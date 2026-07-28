pub mod args;
pub mod error;

use shadowai_search_engines::WebSearchResult;

/// Re-export the public types from sub-modules so they're accessible at the web_search module level.
pub use args::WebSearchArgs;
pub use error::WebSearchError;

/// Wrapper struct that holds engine instances for concurrent execution.
///
/// Each engine is a no-op unit struct from `shadowai-search-engines` — the
/// real work happens inside their `search()` methods (HTTP, retry logic, etc.).
pub struct ShadowWebSearch {
    duckduckgo: shadowai_search_engines::duckduckgo::DuckDuckGoEngine,
    searxng: shadowai_search_engines::searxng::SearxngEngine,
}

impl ShadowWebSearch {
    pub fn new() -> Self {
        Self {
            duckduckgo: shadowai_search_engines::duckduckgo::DuckDuckGoEngine,
            searxng: shadowai_search_engines::searxng::SearxngEngine,
        }
    }

    /// Fire DuckDuckGo with a per-engine timeout. Returns `None` on any failure or timeout.
    async fn run_duckduckgo(&self, query: &str) -> Option<Vec<WebSearchResult>> {
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), self.duckduckgo.search(query)).await;
        match result {
            Ok(Ok(results)) => Some(results),
            _ => None, // timeout or error → skip this engine entirely
        }
    }

    /// Fire SearXNG with a per-engine timeout. Returns `None` on any failure or timeout.
    async fn run_searxng(&self, query: &str) -> Option<Vec<WebSearchResult>> {
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), self.searxng.search(query)).await;
        match result {
            Ok(Ok(results)) => Some(results),
            _ => None, // timeout or error → skip this engine entirely
        }
    }

    /// Fire both engines concurrently and merge/dedup their results.
    async fn execute(&self, args: WebSearchArgs) -> Result<String, WebSearchError> {
        let query = &args.query;

        // Fire both engines in parallel with per-engine timeout (5s each).
        let (ddg_result, searxng_result) = tokio::join!(
            self.run_duckduckgo(query),
            self.run_searxng(query)
        );

        // Each engine returns `None` on timeout or error — skip it silently.
        let ddg_results: Vec<WebSearchResult> = match ddg_result {
            Some(results) => results,
            None => Vec::new(),
        };

        let searxng_results: Vec<WebSearchResult> = match searxng_result {
            Some(results) => results,
            None => Vec::new(),
        };

        // Merge and deduplicate by URL.
        let merged = shadowai_search_engines::normalization::merge_and_dedup(vec![ddg_results, searxng_results]);

        if merged.is_empty() {
            return Err(WebSearchError::EmptyResults { query: args.query });
        }

        Ok(merged)
    }
}

impl rig::tool::Tool for ShadowWebSearch {
    const NAME: &'static str = "web_search";

    type Error = WebSearchError;
    type Args = WebSearchArgs;
    type Output = String;

    fn description(&self) -> String {
        format!(
            "Search the web using multiple search engines (DuckDuckGo, SearXNG). Returns deduplicated results sorted by relevance.",
        )
    }

    fn parameters(&self) -> serde_json::Value {
        schemars::schema_for!(WebSearchArgs).to_value()
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.execute(args).await
    }
}
