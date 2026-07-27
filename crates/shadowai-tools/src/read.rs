use std::path::PathBuf;

use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use shadowai_filesystem::read_file;

#[derive(Deserialize, Serialize)]
pub struct ReadTool;

impl ReadTool {
    pub const DESCRIPTION: &'static str = "Reads the contents of a file at the given path.";
}

impl Tool for ReadTool {
    const NAME: &'static str = "read";

    type Error = std::io::Error;
    type Args = PathBuf;
    type Output = String;

    fn description(&self) -> String {
        ReadTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "string",
            "description": "The path to the file to read.",
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        read_file(args).await
    }
}
