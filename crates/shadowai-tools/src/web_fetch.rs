use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use fetchkit::FetchError;
use fetchkit::FetchRequest;
use fetchkit::fetch;

static FETCH_TOOL: std::sync::LazyLock<fetchkit::Tool> =
    std::sync::LazyLock::new(|| fetchkit::Tool::builder().build());

#[derive(Deserialize)]
pub struct WebFetchArgs {
    /// The URL to fetch.
    pub url: String,
}

/// Fetches web content for an LLM agent.
#[derive(Deserialize, Serialize)]
pub struct WebFetchTool;

impl WebFetchTool {
    pub fn description() -> &'static str {
        FETCH_TOOL.description()
    }
}

impl Tool for WebFetchTool {
    const NAME: &'static str = "web_fetch";

    type Error = FetchError;
    type Args = WebFetchArgs;
    type Output = String;

    fn description(&self) -> String {
        FETCH_TOOL.description().to_string()
    }

    fn parameters(&self) -> Value {
        FETCH_TOOL.input_schema()
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let req = FetchRequest::new(&args.url).as_markdown();
        let response = fetch(req).await?;

        Ok(response.content.unwrap_or_default())
    }
}
