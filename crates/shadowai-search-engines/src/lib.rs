pub mod duckduckgo;
pub mod normalization;
pub mod searxng;

use serde::{Deserialize, Serialize};

/// Canonical search result returned to the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebSearchResult {
    /// Display title of the page.
    pub title: String,

    /// Canonical URL (the one the agent should fetch).
    pub url: String,

    /// Plain-text snippet extracted from the result.
    pub snippet: String,

    /// When available — ISO-8601 timestamp or None if the engine doesn't expose freshness metadata.
    pub date: Option<String>,

    /// Relevance score (used for sorting after merge). Higher = more relevant.
    pub relevance_score: f32,
}