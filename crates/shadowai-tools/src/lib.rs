mod edit;
mod glob;
mod read;
mod shell;
mod web_fetch;
mod web_search;

pub use edit::EditFileTool;
pub use glob::ListMatchingFilesTool;
pub use read::ReadFileTool;
use rig::agent::{AgentHook, HookContext, InvalidToolCallAction, InvalidToolCallContext};
pub use shell::ExecuteShellCommandTool;
pub use web_fetch::WebFetchTool;
pub use web_search::WebSearch;

/// Repair hook that handles invalid tool calls.
pub struct RepairToolCall;

impl AgentHook for RepairToolCall {
    async fn on_invalid_tool_call(
        &self,
        _ctx: &HookContext,
        event: &InvalidToolCallContext,
    ) -> Option<InvalidToolCallAction> {
        match event.tool_name.as_str() {
            "write" => Some(InvalidToolCallAction::Repair {
                tool_name: "edit_file".to_string(),
            }),
            _ => None,
        }
    }
}
