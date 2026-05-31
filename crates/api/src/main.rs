use std::net::SocketAddr;
use std::sync::Arc;

use env_logger::Env;
use stocker_screener::{db::default_db_path, RefreshConfig, ScreenerService};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let db_path = default_db_path();
    log::info!("opening screener DB at {}", db_path.display());
    let service = match ScreenerService::open(&db_path, RefreshConfig::from_env()).await {
        Ok(s) => {
            s.start();
            Some(Arc::new(s))
        }
        Err(e) => {
            log::error!("could not open screener DB ({e}); running without /screener/* routes");
            None
        }
    };

    let addr: SocketAddr = "127.0.0.1:8080".parse().expect("addr");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    log::info!("stocker-api listening on http://{}", addr);
    axum::serve(listener, stocker_api::router(service))
        .await
        .expect("serve");
}
