use thiserror::Error;

#[derive(Error, Debug)]
pub enum StockerError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid symbol: {0}")]
    InvalidSymbol(String),
    #[error("No quote data for symbol")]
    NoQuoteData,
    #[error("Forbidden remote data source: {0}")]
    ForbiddenDataSource(String),
}

pub type Result<T> = std::result::Result<T, StockerError>;
