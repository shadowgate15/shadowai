use rig::tool::{Tool, ToolContext};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use shadowai_shell::{ShellError, execute};

#[derive(Deserialize, JsonSchema)]
pub struct ShellArgs {
    /// The shell command to execute. (e.g., `"ls -la /tmp"`)
    pub command: String,
}

#[derive(Deserialize, Serialize)]
pub struct ExecuteShellCommandTool;

impl ExecuteShellCommandTool {
    pub const DESCRIPTION: &'static str = "Executes a shell command and returns its output.";
}

impl Tool for ExecuteShellCommandTool {
    const NAME: &'static str = "execute_shell_command";

    type Error = ShellError;
    type Args = ShellArgs;
    type Output = String;

    fn description(&self) -> String {
        ExecuteShellCommandTool::DESCRIPTION.to_string()
    }

    fn parameters(&self) -> serde_json::Value {
        schemars::schema_for!(ShellArgs).to_value()
    }

    async fn call(
        &self,
        _ctx: &mut ToolContext,
        args: Self::Args,
    ) -> Result<Self::Output, Self::Error> {
        execute(&args.command).await
    }
}
