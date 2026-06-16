use std::net::SocketAddr;
use std::sync::Arc;

use env_logger::Env;
use stocker_mf::{db::default_db_path as mf_db_path, MfService};
use stocker_portfolio::{db::default_db_path as portfolio_db_path, PortfolioService};
use stocker_screener::{db::default_db_path, RefreshConfig, ScreenerService};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let db_path = default_db_path();
    log::info!("opening screener DB at {}", db_path.display());
    let screener = match ScreenerService::open(&db_path, RefreshConfig::from_env()).await {
        Ok(s) => {
            s.start();
            Some(Arc::new(s))
        }
        Err(e) => {
            log::error!("could not open screener DB ({e}); running without /screener/* routes");
            None
        }
    };

    let mf_db = mf_db_path();
    log::info!("opening mutual fund DB at {}", mf_db.display());
    let mf = match MfService::open(&mf_db).await {
        Ok(s) => Some(Arc::new(s)),
        Err(e) => {
            log::error!("could not open mf DB ({e}); MF NAV unavailable in portfolio");
            None
        }
    };

    let port_db = portfolio_db_path();
    log::info!("opening portfolio DB at {}", port_db.display());
    let portfolio = match PortfolioService::open(&port_db, screener.clone(), mf).await {
        Ok(s) => Some(Arc::new(s)),
        Err(e) => {
            log::error!("could not open portfolio DB ({e}); running without /portfolio/* routes");
            None
        }
    };

    let addr: SocketAddr = "127.0.0.1:8080".parse().expect("addr");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    log::info!("stocker-api listening on http://{}", addr);
    axum::serve(listener, stocker_api::router(screener, portfolio))
        .await
        .expect("serve");
}
