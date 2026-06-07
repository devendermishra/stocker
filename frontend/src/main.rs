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
mod report;
mod routes;
mod screener;
mod screener_api;
mod stocks;
mod types;

fn main() {
    dioxus::launch(app::app);
}
