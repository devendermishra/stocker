#[cfg(all(feature = "web", feature = "desktop"))]
compile_error!("Features `web` and `desktop` are mutually exclusive. For the standalone app use: cargo build -p stocker-web --no-default-features --features desktop");

#[cfg(all(not(feature = "web"), not(feature = "desktop")))]
compile_error!("Enable `web` (default) or `desktop`.");

#[cfg(feature = "web")]
mod web_types;

mod api;
mod app;
mod components;
mod format;
mod portfolio;
mod portfolio_api;
mod portfolio_data_revision;
mod report;
mod routes;
mod screener;
mod screener_api;
mod stocks;
mod sync;
mod sync_api;
mod sync_oauth_modal;
mod types;

fn main() {
    #[cfg(feature = "desktop")]
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.block_on(async {
            match stocker_sync::startup_pull_if_newer().await {
                Ok(Some(action)) => eprintln!("Startup sync: {action:?}"),
                Ok(None) => {}
                Err(e) => eprintln!("Startup sync skipped: {e}"),
            }
        });
    }
    dioxus::launch(app::app);
}
