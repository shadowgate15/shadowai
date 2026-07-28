use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;

use crate::{WebSearchResult, normalization::{strip_html, unix_to_iso8601}};

const SEARXNG_INSTANCE_URL: &str = "https://searx.be";
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
#[derive(Debug, Deserialize)]
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
pub struct SearxngEngine;

impl SearxngEngine {
    /// Search via SearXNG public instance.
    pub async fn search(&self, query: &str) -> Result<Vec<WebSearchResult>, SearxngError> {
        let client = Client::new();

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

    /// Parse the raw SearXNG JSON response into canonical WebSearchResult entries.
    fn parse_response(&self, body: &str) -> Result<Vec<WebSearchResult>, SearxngError> {
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