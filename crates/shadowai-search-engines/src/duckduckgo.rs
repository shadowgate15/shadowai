use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::{WebSearchResult, normalization::strip_html};

const DDG_API_URL: &str = "http://api.duckduckgo.com/";
const MAX_RETRIES: u32 = 3;
const INITIAL_BACKOFF_MS: u64 = 100;
const MAX_BACKOFF_MS: u64 = 8_000;

/// Native response from DuckDuckGo Instant Answer API.
#[derive(Debug, Deserialize)]
pub struct DuckDuckGoResponse {
    /// Top-level heading (from first result or overall).
    #[serde(default)]
    pub heading: String,

    /// Results array — each item has Heading, Body, Source, Url fields.
    #[serde(default)]
    pub results: Vec<DuckDuckGoResult>,
}

/// Each result item in the DDG response array.
#[derive(Debug, Deserialize, Serialize)]
pub struct DuckDuckGoResult {
    /// Title of the page (DDG uses "Heading" field).
    pub heading: String,

    /// Description/snippet text from the page.
    pub body: String,

    /// Source domain of the result.
    pub source: String,

    /// URL to fetch.
    pub url: String,
}

/// Errors that can occur during DuckDuckGo search.
#[derive(Debug, thiserror::Error)]
pub enum DuckDuckGoError {
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

/// DuckDuckGo search engine implementation.
pub struct DuckDuckGoEngine {
    optional_client: Option<Client>,
}

impl DuckDuckGoEngine {
    pub fn new() -> Self {
        Self { optional_client: None }
    }

    /// Search via DuckDuckGo's Instant Answer API.
    pub async fn search(&self, query: &str) -> Result<Vec<WebSearchResult>, DuckDuckGoError> {
        let client = self.optional_client.as_ref().map(|c| c.clone()).unwrap_or_else(Client::new);

        // Build URL with space-encoded query parameter
        let encoded_query = query.replace(' ', "%20");
        let url = format!("{}?q={}&format=jsonf&no_html=1", DDG_API_URL, encoded_query);

        // Retry loop with exponential backoff for timeout and connect failures.
        for attempt in 0..MAX_RETRIES {
            match client.get(&url).send().await {
                Ok(response) => {
                    if response.status() == 429 {
                        // Rate-limited — skip this engine entirely (section 3b).
                        return Err(DuckDuckGoError::ConnectFailure("duckduckgo".to_string()));
                    }

                    match response.text().await {
                        Ok(body) => return self.parse_response(&body, query),
                        Err(_e) => {
                            // Network error while reading body — retry.
                            if attempt < MAX_RETRIES - 1 {
                                let base_delay_ms = INITIAL_BACKOFF_MS * 2u64.pow(attempt as u32);
                                let delay_ms = base_delay_ms.min(MAX_BACKOFF_MS);

                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                continue;
                            } else {
                                return Err(DuckDuckGoError::ConnectFailure(
                                    "duckduckgo".to_string(),
                                ));
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
                        return Err(DuckDuckGoError::ConnectFailure("duckduckgo".to_string()));
                    }
                }
            }
        }

        // Should not reach here, but safety net.
        Err(DuckDuckGoError::Timeout)
    }

    /// Parse a response body into canonical results (used by integration tests).
    pub fn parse_response(
        &self,
        body: &str,
        _query: &str,
    ) -> Result<Vec<WebSearchResult>, DuckDuckGoError> {
        let ddg_response = match serde_json::from_str::<DuckDuckGoResponse>(body) {
            Ok(response) => response,
            Err(_) => return Err(DuckDuckGoError::MalformedResponse),
        };

        // DDG sometimes returns no results (e.g., instant answer only).
        if ddg_response.results.is_empty() {
            return Ok(Vec::new());
        }

        // Normalize each native result into a canonical WebSearchResult.
        let mut web_results = Vec::with_capacity(ddg_response.results.len());
        for item in &ddg_response.results {
            // Strip any HTML from body (defensive normalization).
            let snippet = strip_html(&item.body);

            web_results.push(WebSearchResult {
                title: item.heading.clone(), // heading → title
                url: item.url.clone(),       // URL stays as-is
                snippet,                     // body → snippet
                date: None,                  // DDG doesn't expose freshness metadata
                relevance_score: 0.5,        // Default score for MVP; engines can refine later
            });
        }

        Ok(web_results)
    }
}

impl Default for DuckDuckGoEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_happy_path() {
        let engine = DuckDuckGoEngine::new();
        let body = serde_json::json!({
            "heading": "",
            "results": [
                { "heading": "Test Title", "body": "<p>Snippet text</p>", "source": "example.com", "url": "https://example.com" }
            ]
        })
        .to_string();

        let results = engine.parse_response(&body, "").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Test Title");
        assert_eq!(results[0].snippet, "pSnippet text/p");
        assert_eq!(results[0].url, "https://example.com");
        assert!((results[0].relevance_score - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn parse_response_empty_results() {
        let engine = DuckDuckGoEngine::new();
        let body = serde_json::json!({
            "heading": "",
            "results": []
        })
        .to_string();
        let results = engine.parse_response(&body, "").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn parse_response_malformed_json() {
        let engine = DuckDuckGoEngine::new();
        let result = engine.parse_response("not json", "");
        assert!(matches!(result, Err(DuckDuckGoError::MalformedResponse)));
    }

    #[test]
    fn default_engine_is_new_instance() {
        // Default is a new Client — not shared across tests.
        let _ = DuckDuckGoEngine::default();
    }
}
