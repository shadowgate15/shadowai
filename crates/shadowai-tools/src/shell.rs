use rig::tool::Tool;
use serde::{Deserialize, Serialize};
use shadowai_shell::{ShellError, execute};

#[derive(Deserialize, Serialize)]
pub struct ShellTool;

impl ShellTool {
    pub const DESCRIPTION: &'static str = "Executes a shell command and returns its output.";
}

impl Tool for ShellTool {
    const NAME: &'static str = "shell";

    type Error = ShellError;
    type Args = String;
    type Output = String;

    fn description(&self) -> String {
        ShellTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "string",
            "description": "The shell command to execute.",
        })
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        execute(&args).await
    }
}
