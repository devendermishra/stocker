//! User registration, login, and session management.

use std::time::{SystemTime, UNIX_EPOCH};

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use chrono::Utc;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};
use sqlx::{Row, SqlitePool};

use crate::error::{Error, Result};
use crate::models::User;

const SESSION_TTL_SECS: i64 = 30 * 24 * 60 * 60; // 30 days

#[derive(Debug, Clone, serde::Serialize)]
pub struct AuthSession {
    pub token: String,
    pub user: User,
    pub expires_at: i64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

pub fn hash_password(password: &str) -> Result<String> {
    if password.len() < 6 {
        return Err(Error::InvalidInput(
            "password must be at least 6 characters".into(),
        ));
    }
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| Error::Other(format!("hash password: {e}")))?
        .to_string();
    Ok(hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(password_hash)
        .map_err(|e| Error::Other(format!("parse password hash: {e}")))?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    hex::encode(bytes)
}

fn normalize_email(email: &str) -> Result<String> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') || email.len() < 5 {
        return Err(Error::InvalidInput("invalid email".into()));
    }
    Ok(email)
}

pub const LOCAL_USER_EMAIL: &str = "local@stocker";

/// Single local user for offline portfolio use (no login required).
pub async fn ensure_local_user(pool: &SqlitePool) -> Result<User> {
    if let Some(id) = sqlx::query_scalar::<_, i64>("SELECT id FROM users WHERE email = ?")
        .bind(LOCAL_USER_EMAIL)
        .fetch_optional(pool)
        .await?
    {
        return get_user_by_id(pool, id).await;
    }
    let now = Utc::now().timestamp();
    let res = sqlx::query(
        "INSERT INTO users (email, password_hash, display_name, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(LOCAL_USER_EMAIL)
    .bind("!")
    .bind("Local")
    .bind(now)
    .execute(pool)
    .await?;
    get_user_by_id(pool, res.last_insert_rowid()).await
}

pub async fn register(pool: &SqlitePool, req: &RegisterRequest) -> Result<AuthSession> {
    let email = normalize_email(&req.email)?;
    let password_hash = hash_password(&req.password)?;
    let now = Utc::now().timestamp();

    let existing: Option<i64> =
        sqlx::query_scalar("SELECT id FROM users WHERE email = ?")
            .bind(&email)
            .fetch_optional(pool)
            .await?;
    if existing.is_some() {
        return Err(Error::Conflict("email already registered".into()));
    }

    let res = sqlx::query(
        "INSERT INTO users (email, password_hash, display_name, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&email)
    .bind(&password_hash)
    .bind(req.display_name.as_deref())
    .bind(now)
    .execute(pool)
    .await?;

    let user_id = res.last_insert_rowid();
    create_session(pool, user_id).await
}

pub async fn login(pool: &SqlitePool, req: &LoginRequest) -> Result<AuthSession> {
    let email = normalize_email(&req.email)?;
    let row = sqlx::query(
        "SELECT id, email, password_hash, display_name, created_at FROM users WHERE email = ?",
    )
    .bind(&email)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::Unauthorized)?;

    let password_hash: String = row.try_get("password_hash")?;
    if !verify_password(&req.password, &password_hash)? {
        return Err(Error::Unauthorized);
    }

    let user_id: i64 = row.try_get("id")?;
    create_session(pool, user_id).await
}

async fn create_session(pool: &SqlitePool, user_id: i64) -> Result<AuthSession> {
    let token = generate_token();
    let token_hash = hash_token(&token);
    let now = Utc::now().timestamp();
    let expires_at = now + SESSION_TTL_SECS;

    sqlx::query(
        "INSERT INTO sessions (user_id, token_hash, expires_at, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(user_id)
    .bind(&token_hash)
    .bind(expires_at)
    .bind(now)
    .execute(pool)
    .await?;

    let user = get_user_by_id(pool, user_id).await?;
    Ok(AuthSession {
        token,
        user,
        expires_at,
    })
}

pub async fn logout(pool: &SqlitePool, token: &str) -> Result<()> {
    let token_hash = hash_token(token);
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(&token_hash)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn authenticate(pool: &SqlitePool, token: &str) -> Result<User> {
    let token_hash = hash_token(token);
    let now = Utc::now().timestamp();
    let row = sqlx::query(
        "SELECT s.user_id, s.expires_at, u.id, u.email, u.display_name, u.created_at
         FROM sessions s
         JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ?",
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?
    .ok_or(Error::Unauthorized)?;

    let expires_at: i64 = row.try_get("expires_at")?;
    if expires_at < now {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(&token_hash)
            .execute(pool)
            .await?;
        return Err(Error::Unauthorized);
    }

    Ok(User {
        id: row.try_get("id")?,
        email: row.try_get("email")?,
        display_name: row.try_get("display_name")?,
        created_at: row.try_get("created_at")?,
    })
}

pub async fn get_user_by_id(pool: &SqlitePool, user_id: i64) -> Result<User> {
    let row = sqlx::query("SELECT id, email, display_name, created_at FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(Error::NotFound)?;
    Ok(User {
        id: row.try_get("id")?,
        email: row.try_get("email")?,
        display_name: row.try_get("display_name")?,
        created_at: row.try_get("created_at")?,
    })
}

/// Purge expired sessions (maintenance helper).
pub async fn purge_expired_sessions(pool: &SqlitePool) -> Result<u64> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let res = sqlx::query("DELETE FROM sessions WHERE expires_at < ?")
        .bind(now)
        .execute(pool)
        .await?;
    Ok(res.rows_affected())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hash_roundtrip() {
        let hash = hash_password("secret123").unwrap();
        assert!(verify_password("secret123", &hash).unwrap());
        assert!(!verify_password("wrong", &hash).unwrap());
    }
}
