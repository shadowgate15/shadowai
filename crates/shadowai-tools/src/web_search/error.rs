use serde::Serialize;

/// Errors that can occur during a web search.
#[derive(Debug, Clone, Serialize)]
pub enum WebSearchError {
    /// A single engine timed out (e.g., 5s per-engine deadline).
    Timeout(String),
    /// A connect-level failure (DNS, refused connection, etc.).
    ConnectFailure(String),
    /// The engine returned a malformed response.
    MalformedResponse(String),
    /// All engines are unavailable — surfaced as an aggregated error.
    AllEnginesUnavailable,
    /// Every engine failed hard; no results to return.
    EmptyResults { query: String },
}

impl std::fmt::Display for WebSearchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout(engine) => write!(f, "Engine {} timed out", engine),
            Self::ConnectFailure(engine) => write!(f, "Connection failed for {}", engine),
            Self::MalformedResponse(engine) => write!(f, "Malformed response from {}", engine),
            Self::AllEnginesUnavailable => write!(f, "All search engines are currently unavailable"),
            Self::EmptyResults { query } => {
                write!(f, "No results returned for query: {}", query)
            }
        }
    }
}

impl std::error::Error for WebSearchError {}