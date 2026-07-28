use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Maximum query length accepted by the tool. Queries longer than this are rejected before being sent to engines.
const MAX_QUERY_LENGTH: usize = 500;

#[derive(Deserialize, JsonSchema)]
pub struct WebSearchArgs {
    /// The search query.
    pub query: String,

    /// Maximum number of results to return.
    #[serde(default = "default_max_results")]
    pub max_results: usize,

    /// Search type (web, news, images, etc.).
    #[serde(default)]
    pub search_type: WebSearchType,

    /// Language code for the results (e.g., en, fr).
    #[serde(default)]
    pub language: String,

    /// Region/locale to target.
    #[serde(default)]
    pub region: String,
}

fn default_max_results() -> usize {
    10
}

#[derive(Default, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WebSearchType {
    #[default]
    Web,
    News,
    Images,
    Videos,
}

/// Reject queries that are empty or too long. Returns `true` if the query is valid, `false` otherwise.
///
/// Also rejects strings containing control characters (U+0000–U+001F) which could break HTTP parsing on some engines.
pub fn validate_query(query: &str) -> bool {
    let trimmed = query.trim();

    if trimmed.is_empty() {
        return false;
    }

    if trimmed.len() > MAX_QUERY_LENGTH {
        return false;
    }

    // Reject control characters that could corrupt engine HTTP payloads.
    for byte in trimmed.as_bytes() {
        if (0x00..=0x1F).contains(byte) {
            return false;
        }
    }

    true
}
