use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GlobError {
    #[error(transparent)]
    GlobError(#[from] glob::GlobError),
    #[error(transparent)]
    PatternError(#[from] glob::PatternError),
}

#[derive(Deserialize, Serialize)]
pub struct GlobTool;

impl GlobTool {
    pub const DESCRIPTION: &'static str =
        "Lists files matching a glob pattern (e.g., *.rs, src/**/*).";
}

impl Tool for GlobTool {
    const NAME: &'static str = "glob";

    type Error = GlobError;
    type Args = String;
    type Output = Vec<String>;

    fn description(&self) -> String {
        GlobTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "string",
            "description": "The glob pattern to match files against.",
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        let mut results = Vec::new();
        for entry in glob::glob(&args)? {
            let path = entry?;
            if path.is_file() {
                results.push(path.to_string_lossy().into_owned());
            }
        }
        Ok(results)
    }
}
