use clap::Parser;
use env_logger::Env;

/// NSE stock research CLI (Yahoo Finance data)
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Stock symbol (e.g. RELIANCE or RELIANCE.NS)
    symbol: String,
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    match stocker_core::build_research_report(&args.symbol).await {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap()),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
}
