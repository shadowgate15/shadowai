use rig::tool::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shadowai_shell::{ShellError, execute};

#[derive(Deserialize, JsonSchema)]
pub struct ShellArgs {
    /// The shell command to execute.
    pub command: String,
}

#[derive(Deserialize, Serialize)]
pub struct ShellTool;

impl ShellTool {
    pub const DESCRIPTION: &'static str = "Executes a shell command and returns its output.";
}

impl Tool for ShellTool {
    const NAME: &'static str = "shell";

    type Error = ShellError;
    type Args = ShellArgs;
    type Output = String;

    fn description(&self) -> String {
        ShellTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        schemars::schema_for!(ShellArgs).to_value()
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        execute(&args.command).await
    }
}
