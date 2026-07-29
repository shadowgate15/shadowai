use rig::tool::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Deserialize, JsonSchema)]
pub struct GlobArgs {
    /// The glob pattern to match files against.
    pub pattern: String,
}

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
    type Args = GlobArgs;
    type Output = Vec<String>;

    fn description(&self) -> String {
        GlobTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> Value {
        schemars::schema_for!(GlobArgs).to_value()
    }

    async fn call(
        &self,
        _ctx: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        let mut results = Vec::new();
        for entry in glob::glob(&args.pattern)? {
            let path = entry?;
            if path.is_file() {
                results.push(path.to_string_lossy().into_owned());
            }
        }
        Ok(results)
    }
}
