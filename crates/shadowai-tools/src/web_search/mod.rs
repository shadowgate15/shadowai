pub mod args;
pub mod cache;
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
    pub const DESCRIPTION: &'static str = "Search the web using multiple search engines (DuckDuckGo, SearXNG). Returns deduplicated results sorted by relevance.";

    /// Fire DuckDuckGo with a per-engine timeout. Returns `None` on any failure or timeout.
    async fn run_duckduckgo(&self, query: &str) -> Option<Vec<WebSearchResult>> {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.duckduckgo.search(query),
        )
        .await;
        match result {
            Ok(Ok(results)) => Some(results),
            _ => None, // timeout or error → skip this engine entirely
        }
    }

    /// Fire SearXNG with a per-engine timeout. Returns `None` on any failure or timeout.
    async fn run_searxng(&self, query: &str) -> Option<Vec<WebSearchResult>> {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.searxng.search(query),
        )
        .await;
        match result {
            Ok(Ok(results)) => Some(results),
            _ => None, // timeout or error → skip this engine entirely
        }
    }

    /// Fire both engines concurrently and merge/dedup their results.
    async fn execute(&self, args: WebSearchArgs) -> Result<String, WebSearchError> {
        let query = &args.query;

        // Check cache first — if we have a fresh result for this query, return it directly.
        if let Some(cached) = crate::web_search::cache::check(query) {
            return Ok(cached);
        }

        // Fire both engines in parallel with per-engine timeout (5s each).
        let (ddg_result, searxng_result) =
            tokio::join!(self.run_duckduckgo(query), self.run_searxng(query));

        // Each engine returns `None` on timeout or error — skip it silently.
        let ddg_results: Vec<WebSearchResult> = ddg_result.unwrap_or_default();

        let searxng_results: Vec<WebSearchResult> = searxng_result.unwrap_or_default();

        // Merge and deduplicate by URL.
        let merged = shadowai_search_engines::normalization::merge_and_dedup(vec![
            ddg_results,
            searxng_results,
        ]);

        if merged.is_empty() {
            return Err(WebSearchError::EmptyResults { query: args.query });
        }

        // Store the successful result in cache for future lookups.
        crate::web_search::cache::store(query.as_str(), merged.clone());

        Ok(merged)
    }
}

impl Default for ShadowWebSearch {
    fn default() -> Self {
        Self {
            duckduckgo: shadowai_search_engines::duckduckgo::DuckDuckGoEngine,
            searxng: shadowai_search_engines::searxng::SearxngEngine,
        }
    }
}

impl rig::tool::Tool for ShadowWebSearch {
    const NAME: &'static str = "web_search";

    type Error = WebSearchError;
    type Args = WebSearchArgs;
    type Output = String;

    fn description(&self) -> String {
        ShadowWebSearch::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        schemars::schema_for!(WebSearchArgs).to_value()
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        self.execute(args).await
    }
}
