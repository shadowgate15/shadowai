use std::path::PathBuf;

use rig::tool::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shadowai_filesystem::read_file;

#[derive(Deserialize, JsonSchema)]
pub struct ReadArgs {
    /// The path to the file to read.
    pub file: PathBuf,
}

#[derive(Deserialize, Serialize)]
pub struct ReadFileTool;

impl ReadFileTool {
    pub const DESCRIPTION: &'static str = "Reads the contents of a file at the given path.";
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";

    type Error = std::io::Error;
    type Args = ReadArgs;
    type Output = String;

    fn description(&self) -> String {
        ReadFileTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> Value {
        schemars::schema_for!(ReadArgs).to_value()
    }

    async fn call(
        &self,
        _ctx: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        read_file(args.file).await
    }
}
