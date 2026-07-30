use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentUIIpcMessage {
    /// Request to LLM has started. This is sent when the agent starts processing a request.
    Start {
        metadata: AgentUIIpcMessageMetadata,
    },
    /// Request to LLM has finished. This is sent when the agent finishes processing a request.
    Finish {
        metadata: AgentUIIpcMessageMetadata,
    },
    ModelTurnFinished {
        metadata: AgentUIIpcMessageMetadata,
    },
    ToolCall {
        metadata: AgentUIIpcMessageMetadata,
        name: String,
        id: String,
    },
    ToolResult {
        metadata: AgentUIIpcMessageMetadata,
        name: String,
        id: String,
        result: String,
    },
    TextDelta {
        metadata: AgentUIIpcMessageMetadata,
        delta: String,
    },
    ToolCallDelta {
        metadata: AgentUIIpcMessageMetadata,
        id: String,
        delta: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentUIIpcMessageMetadata {
    /// Name of the agent that is running. This is optional because some agents may not have a name.
    agent_name: Option<String>,
    /// Run ID of the agent that is running. This is used to correlate messages from the same run.
    run_id: String,
}

#[cfg(feature = "hook")]
impl From<&rig::agent::HookContext> for AgentUIIpcMessageMetadata {
    fn from(ctx: &rig::agent::HookContext) -> Self {
        Self {
            agent_name: ctx.agent_name().map(|s| s.to_string()),
            run_id: ctx.run_id().to_string(),
        }
    }
}
