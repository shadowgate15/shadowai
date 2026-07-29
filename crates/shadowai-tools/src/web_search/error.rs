/// Errors that can occur during a web search.
#[derive(Debug, thiserror::Error)]
pub enum WebSearchError {
    #[error("Query is too long or empty: {0}")]
    MalformedQuery(String),
    #[error(transparent)]
    TryFromIntError(#[from] std::num::TryFromIntError),
    #[error(transparent)]
    SearchError(#[from] websearch::SearchError),
    #[error(transparent)]
    SerializationError(#[from] serde_json::Error),
    #[error("No results found for query: {0}")]
    EmptyResults(String),
}
