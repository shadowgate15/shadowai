mod edit;
mod glob;
mod read;
mod shell;
mod web_fetch;

pub use edit::EditTool;
pub use glob::GlobTool;
pub use read::ReadTool;
pub use shell::ShellTool;
pub use web_fetch::WebFetchTool;

/// Repair hook that handles invalid tool calls.
pub struct RepairToolCall;

impl<M: rig::prelude::CompletionModel> rig::agent::AgentHook<M> for RepairToolCall {
    async fn on_event(
        &self,
        _ctx: &rig::agent::HookContext,
        event: rig::agent::StepEvent<'_, M>,
    ) -> rig::agent::Flow {
        match event {
            rig::agent::StepEvent::InvalidToolCall(ctx) if ctx.tool_name == "write_file" => {
                rig::agent::Flow::repair("edit_file")
            }
            rig::agent::StepEvent::InvalidToolCall(ctx) => rig::agent::Flow::retry(format!(
                "Use one of these tools: {:?}",
                ctx.available_tools
            )),
            _ => rig::agent::Flow::cont(),
        }
    }
}
