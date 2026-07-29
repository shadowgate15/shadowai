use schemars::JsonSchema;
use serde::Deserialize;

/// Maximum query length accepted by the tool. Queries longer than this are rejected before being sent to engines.
const MAX_QUERY_LENGTH: usize = 500;

#[derive(Deserialize, Clone, JsonSchema)]
pub struct WebSearchArgs {
    /// The search query.
    pub query: String,

    /// Maximum number of results to return.
    #[serde(default = "default_max_results")]
    pub max_results: u32,

    /// Language code for the results (e.g., en, fr).
    #[serde(default = "default_language")]
    pub language: String,

    /// Region/locale to target.
    #[serde(default = "default_region")]
    pub region: String,
}

fn default_max_results() -> u32 {
    10
}

fn default_language() -> String {
    "en".to_string()
}

fn default_region() -> String {
    "US".to_string()
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
