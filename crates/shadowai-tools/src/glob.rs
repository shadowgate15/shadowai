use globwalk::glob;
use rig::tool::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GlobToolError {
    #[error("Failed to walk directory tree: {0}")]
    Walk(#[from] globwalk::WalkError),
    #[error("Failed to parse glob pattern: {0}")]
    Glob(#[from] globwalk::GlobError),
}

#[derive(Deserialize, JsonSchema)]
pub struct GlobArgs {
    /// The glob pattern to match files against.
    pub pattern: String,
}

#[derive(Serialize)]
pub struct GlobTool;

impl GlobTool {
    pub const DESCRIPTION: &'static str =
        "Lists files matching a glob pattern (e.g., *.rs, src/**/*). Respects .gitignore rules.";
}

impl Tool for GlobTool {
    const NAME: &'static str = "glob";

    type Error = GlobToolError;
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

        // Walk the directory tree while respecting .gitignore files automatically.
        // globwalk respects all parent directories' .gitignore rules when walking.
        for entry in glob(&args.pattern)? {
            let entry = entry?;

            if !entry.file_type().is_file() {
                continue;
            }

            results.push(entry.path().to_string_lossy().into_owned());
        }

        Ok(results)
    }
}
