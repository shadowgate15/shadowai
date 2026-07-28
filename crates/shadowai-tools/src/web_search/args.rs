use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
