pub mod args;
pub mod cache;
pub mod error;

use serde::{Deserialize, Serialize};

/// Re-export the public types from sub-modules so they're accessible at the web_search module level.
pub use args::WebSearchArgs;
pub use error::WebSearchError;
use websearch::{providers::DuckDuckGoProvider, web_search};

/// Wrapper struct that holds engine instances for concurrent execution.
///
/// Each engine is a no-op unit struct from `shadowai-search-engines` — the
/// real work happens inside their `search()` methods (HTTP, retry logic, etc.).
pub struct WebSearch;

impl WebSearch {
    pub const DESCRIPTION: &'static str = "Search the web using multiple search engines (DuckDuckGo, SearXNG). Returns deduplicated results sorted by relevance.";

    /// Fire web_search
    async fn execute(&self, args: WebSearchArgs) -> Result<String, WebSearchError> {
        let query = &args.query;

        // Reject empty / malformed queries before touching cache or engines.
        if !crate::web_search::args::validate_query(query) {
            return Err(WebSearchError::MalformedQuery(query.to_string()));
        }

        // Check cache first — if we have a fresh result for this query, return it directly.
        if let Some(cached) = crate::web_search::cache::check(query) {
            return Ok(cached);
        }

        let provider = DuckDuckGoProvider::new();

        let result = web_search(websearch::SearchOptions {
            query: query.to_string(),
            max_results: Some(args.max_results),
            language: Some(args.language),
            region: Some(args.region),
            provider: Box::new(provider),
            timeout: Some(std::time::Duration::from_secs(5).as_millis().try_into()?), // Per-engine timeout (5s)
            ..Default::default()
        })
        .await?
        .into_iter()
        .map(SearchResult::from)
        .collect::<Vec<SearchResult>>();

        let merged = serde_json::to_string(&result)?;

        println!(
            "web_search: Merged {} results for query '{}'",
            result.len(),
            query
        );

        if merged.is_empty() {
            return Err(WebSearchError::EmptyResults(args.query));
        }

        // Store the successful result in cache for future lookups.
        crate::web_search::cache::store(query.as_str(), merged.clone());

        Ok(merged)
    }
}

impl rig::tool::Tool for WebSearch {
    const NAME: &'static str = "web_search";

    type Error = WebSearchError;
    type Args = WebSearchArgs;
    type Output = String;

    fn description(&self) -> String {
        WebSearch::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        schemars::schema_for!(WebSearchArgs).to_value()
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.execute(args).await
    }
}

// Represents a web search result returned by any search provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    /// URL of the search result
    pub url: String,
    /// Title of the web page
    pub title: String,
    /// Snippet/description of the web page
    pub snippet: Option<String>,
    /// The source website domain
    pub domain: Option<String>,
    /// When the result was published or last updated
    pub published_date: Option<String>,
}

impl From<websearch::SearchResult> for SearchResult {
    fn from(result: websearch::SearchResult) -> Self {
        Self {
            url: result.url,
            title: result.title,
            snippet: result.snippet,
            domain: result.domain,
            published_date: result.published_date,
        }
    }
}
