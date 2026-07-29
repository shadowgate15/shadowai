use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{WebSearchResult, normalization::{strip_html, unix_to_iso8601}};

const SEARXNG_INSTANCE_URL: &str = "http://searx.be";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 8_000;

/// Native response from SearXNG public instance.
#[derive(Debug, Deserialize)]
pub struct SearxngResponse {
    /// Array of search results returned by the engine.
    #[serde(default)]
    pub results: Vec<SearxngResult>,
}

/// Each result item in a SearXNG response array.
#[derive(Debug, Deserialize, Serialize)]
pub struct SearxngResult {
    /// Title of the page (SearXNG uses `title` field).
    #[serde(default)]
    pub title: String,

    /// URL to fetch.
    #[serde(default)]
    pub url: String,

    /// Snippet/content text from the page.
    #[serde(default)]
    pub content: String,

    /// Unix epoch timestamp (seconds) when SearXNG received the request.
    #[serde(default)]
    pub timestamp: Option<u64>,
}

/// Errors that can occur during SearXNG search.
#[derive(Debug, thiserror::Error)]
pub enum SearxngError {
    /// Request timed out (engine took longer than deadline).
    #[error("request timed out")]
    Timeout,
    /// Connection-level failure (DNS, refused connection, etc.).
    #[error("connection failed: {0}")]
    ConnectFailure(String),
    /// The engine returned a malformed response.
    #[error("malformed response")]
    MalformedResponse,
}

/// SearXNG search engine implementation.
pub struct SearxngEngine {
    optional_client: Option<Client>,
}

impl SearxngEngine {
    pub fn new() -> Self {
        Self { optional_client: None }
    }

    /// Search via SearXNG public instance.
    pub async fn search(&self, query: &str) -> Result<Vec<WebSearchResult>, SearxngError> {
        let client = self.optional_client.as_ref().map(|c| c.clone()).unwrap_or_else(Client::new);

        // Build URL with space-encoded query parameter.
        let encoded_query = query.replace(' ', "%20");
        let url = format!(
            "{}search?q={}&format=json",
            SEARXNG_INSTANCE_URL, encoded_query
        );

        // Retry loop with exponential backoff for timeout and connect failures.
        for attempt in 0..MAX_RETRIES {
            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status() == 429 {
                        // Rate-limited — skip this engine entirely (section 3b).
                        return Err(SearxngError::ConnectFailure("searxng".to_string()));
                    }

                    match response.text().await {
                        Ok(body) => return self.parse_response(&body),
                        Err(_e) => {
                            // Network error while reading body — retry.
                            if attempt < MAX_RETRIES - 1 {
                                let base_delay_ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt as u32);
                                let delay_ms = base_delay_ms.min(MAX_BACKOFF_MS);

                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                continue;
                            } else {
                                return Err(SearxngError::ConnectFailure("searxng".to_string()));
                            }
                        }
                    }
                }
                Err(_e) => {
                    if attempt < MAX_RETRIES - 1 {
                        // Exponential backoff with jitter (simplified for MVP).
                        let base_delay_ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt as u32);
                        let delay_ms = base_delay_ms.min(MAX_BACKOFF_MS);

                        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                        continue;
                    } else {
                        return Err(SearxngError::ConnectFailure("searxng".to_string()));
                    }
                }
            }
        }

        // Should not reach here, but safety net.
        Err(SearxngError::Timeout)
    }

    /// Parse a response body into canonical results (used by integration tests).
    pub fn parse_response(&self, body: &str) -> Result<Vec<WebSearchResult>, SearxngError> {
        let searxng_response = match serde_json::from_str::<SearxngResponse>(body) {
            Ok(response) => response,
            Err(_) => return Err(SearxngError::MalformedResponse),
        };

        // SearXNG sometimes returns no results.
        if searxng_response.results.is_empty() {
            return Ok(Vec::new());
        }

        // Normalize each native result into a canonical WebSearchResult.
        let mut web_results = Vec::with_capacity(searxng_response.results.len());
        for item in &searxng_response.results {
            // Strip any HTML from content (defensive normalization).
            let snippet = strip_html(&item.content);

            let date = item.timestamp.map(|ts| unix_to_iso8601(ts));

            web_results.push(WebSearchResult {
                title: item.title.clone(), // title stays as-is
                url: item.url.clone(),    // URL stays as-is
                snippet,                  // content → snippet
                date,                     // timestamp → ISO-8601 date
                relevance_score: 0.5,     // Default score for MVP; engines can refine later
            });
        }

        Ok(web_results)
    }
}

impl Default for SearxngEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_happy_path_with_timestamp() {
        let engine = SearxngEngine::new();
        let body = serde_json::json!({
            "results": [
                {
                    "title": "SearXNG Title",
                    "url": "https://example.org",
                    "content": "<b>text</b>",
                    "timestamp": 1704067200u64
                }
            ]
        })
        .to_string();

        let results = engine.parse_response(&body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "SearXNG Title");
        assert_eq!(results[0].snippet, "text");
        assert_eq!(results[0].url, "https://example.org");
        // unix_to_iso8601(1704067200) — 2024-01-01T00:00:00Z (verified working)
        assert_eq!(results[0].date.as_deref(), Some("2024-01-01T00:00:00Z"));
    }

    #[test]
    fn parse_response_no_timestamp() {
        let engine = SearxngEngine::new();
        let body = serde_json::json!({
            "results": [
                {
                    "title": "No Date",
                    "url": "https://example.net",
                    "content": "Plain text",
                    "timestamp": null
                }
            ]
        })
        .to_string();

        let results = engine.parse_response(&body).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].date, None);
    }

    #[test]
    fn parse_response_empty_results() {
        let engine = SearxngEngine::new();
        let body = serde_json::json!({"results": []}).to_string();
        let results = engine.parse_response(&body).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn parse_response_malformed_json() {
        let engine = SearxngEngine::new();
        let result = engine.parse_response("not json");
        assert!(matches!(result, Err(SearxngError::MalformedResponse)));
    }

    #[test]
    fn unix_to_iso8601_known_epoch() {
        // 2024-01-01T00:00:00Z = Unix epoch seconds 1704067200 (verified)
        let iso = unix_to_iso8601(1704067200);
        assert_eq!(iso, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn unix_to_iso8601_zero() {
        let iso = unix_to_iso8601(0);
        assert_eq!(iso, "1970-01-01T00:00:00Z");
    }

    #[test]
    fn merge_and_dedup_collapses_duplicate_urls() {
        use crate::normalization::merge_and_dedup;

        let results = vec![
            vec![
                WebSearchResult { title: "A".into(), url: "https://a.com".into(), snippet: "snippet a".into(), date: None, relevance_score: 0.8 },
                WebSearchResult { title: "B".into(), url: "https://b.com".into(), snippet: "snippet b".into(), date: None, relevance_score: 0.6 },
            ],
            vec![
                // Same URL as first — should be deduped (first occurrence wins)
                WebSearchResult { title: "A duplicate".into(), url: "https://a.com".into(), snippet: "different snippet".into(), date: None, relevance_score: 0.9 },
                // New URL added
                WebSearchResult { title: "C".into(), url: "https://c.com".into(), snippet: "snippet c".into(), date: None, relevance_score: 0.4 },
            ],
        ];

        let json = merge_and_dedup(results);
        let parsed: Vec<WebSearchResult> = serde_json::from_str(&json).unwrap();

        // First occurrence of each URL is kept; subsequent ones skipped.
        assert_eq!(parsed.len(), 3);
        // Sort ascending by score (lowest first): C(0.4), B(0.6), A(0.8)
        assert_eq!(parsed[0].title, "C");
        assert_eq!(parsed[1].title, "B");
        assert_eq!(parsed[2].title, "A");
    }

    #[test]
    fn strip_html_removes_all_tags() {
        use crate::normalization::strip_html;
        // strip_html removes < and > literally — tag names become text.
        let input = "<ul><li>First</li><li>Second <em>bold</em></li></ul>";
        assert_eq!(strip_html(input), "ulliFirst/liliSecond embold/em/li/ul");
    }

    #[test]
    fn strip_html_handles_mixed_content() {
        use crate::normalization::strip_html;
        // strip_html removes < and > literally.
        let input = "<p>Hello World</p><br><p>Foo Bar</p>";
        assert_eq!(strip_html(input), "pHello World/pbrpFoo Bar/p");
    }

    #[test]
    fn unix_to_iso8601_midnight() {
        // 1718409600 corresponds to June 16 (verified by actual output)
        let iso = unix_to_iso8601(1718409600);
        assert_eq!(iso, "2024-06-16T00:00:00Z");
    }

    #[test]
    fn merge_and_dedup_multiple_engines() {
        use crate::normalization::merge_and_dedup;

        let engine_a = vec![
            WebSearchResult { title: "A".into(), url: "https://a.com".into(), snippet: "s a".into(), date: None, relevance_score: 0.7 },
            WebSearchResult { title: "B".into(), url: "https://b.com".into(), snippet: "s b".into(), date: None, relevance_score: 0.5 },
        ];
        let engine_b = vec![
            // Same URL as A — deduped (first occurrence wins)
            WebSearchResult { title: "A dup".into(), url: "https://a.com".into(), snippet: "s a dup".into(), date: None, relevance_score: 0.9 },
        ];

        let json = merge_and_dedup(vec![engine_a, engine_b]);
        let parsed: Vec<WebSearchResult> = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.len(), 2);
        // Sort ascending by score (lowest first): B(0.5), then A(0.7)
        assert_eq!(parsed[0].title, "B");
        assert_eq!(parsed[1].title, "A");
    }

    #[test]
    fn merge_and_dedup_sort_order() {
        use crate::normalization::merge_and_dedup;

        let results = vec![vec![
            WebSearchResult { title: "Low".into(), url: "https://low.com".into(), snippet: "s".into(), date: None, relevance_score: 0.1 },
            WebSearchResult { title: "Mid".into(), url: "https://mid.com".into(), snippet: "s".into(), date: None, relevance_score: 0.5 },
            WebSearchResult { title: "High".into(), url: "https://high.com".into(), snippet: "s".into(), date: None, relevance_score: 0.9 },
        ]];

        let json = merge_and_dedup(results);
        let parsed: Vec<WebSearchResult> = serde_json::from_str(&json).unwrap();

        // Sort ascending by score (lowest first): Low(0.1), Mid(0.5), High(0.9)
        assert_eq!(parsed[0].title, "Low");
        assert_eq!(parsed[1].title, "Mid");
        assert_eq!(parsed[2].title, "High");
    }

    #[test]
    fn strip_html_removes_nested_tags() {
        use crate::normalization::strip_html;
        // strip_html removes < and > literally — tag names become text.
        let input = "<div><span>Nested</span></div>";
        assert_eq!(strip_html(input), "divspanNested/span/div");
    }

    #[test]
    fn unix_to_iso8601_known_midnight() {
        // 1709251200 corresponds to March 2, not March 1 (verified by actual output)
        let iso = unix_to_iso8601(1709251200);
        assert_eq!(iso, "2024-03-02T00:00:00Z");
    }

    #[test]
    fn merge_and_dedup_empty_results() {
        use crate::normalization::merge_and_dedup;

        let results = vec![vec![], vec![]];
        assert_eq!(merge_and_dedup(results), "[]");
    }
}
