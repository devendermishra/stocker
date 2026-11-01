use thiserror::Error;

#[derive(Error, Debug)]
pub enum DomainError {
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),
}
