use env_logger::Env;
use std::net::SocketAddr;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    let addr: SocketAddr = "127.0.0.1:8080".parse().expect("addr");
    let listener = tokio::net::TcpListener::bind(addr).await.expect("bind");
    log::info!("stocker-api listening on http://{}", addr);
    axum::serve(listener, stocker_api::router())
        .await
        .expect("serve");
}
