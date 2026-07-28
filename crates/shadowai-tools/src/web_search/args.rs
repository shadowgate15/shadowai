use serde::{Deserialize, Serialize};
use schemars::JsonSchema;

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

fn default_max_results() -> usize { 10 }

#[derive(Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum WebSearchType {
    Web,
    News,
    Images,
    Videos,
}

impl Default for WebSearchType {
    fn default() -> Self {
        Self::Web
    }
}