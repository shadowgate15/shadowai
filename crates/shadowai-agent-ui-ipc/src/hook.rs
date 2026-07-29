use std::ops::Deref;

use rig::{
    agent::{AgentHook, Flow, HookContext, StepEvent},
    prelude::CompletionModel,
};

use crate::{AgentUIIpcMessage, AgentUIIpcSender};

pub struct AgentUIIpcHook(AgentUIIpcSender);

impl<M: CompletionModel> AgentHook<M> for AgentUIIpcHook {
    async fn on_event(&self, ctx: &HookContext, event: StepEvent<'_, M>) -> Flow {
        match event {
            StepEvent::CompletionCall {
                prompt,
                history,
                turn,
            } => self.send(AgentUIIpcMessage::Start {
                agent_name: ctx.agent_name().map(|s| s.to_string()),
                run_id: ctx.run_id().to_string(),
            }),
            StepEvent::CompletionResponse { prompt, response } => {
                self.send(AgentUIIpcMessage::Finish {
                    agent_name: ctx.agent_name().map(|s| s.to_string()),
                    run_id: ctx.run_id().to_string(),
                })
            }
            StepEvent::ModelTurnFinished {
                turn,
                content,
                usage,
            } => todo!(),
            StepEvent::InvalidToolCall(invalid_tool_call_context) => todo!(),
            StepEvent::ToolCall {
                tool_name,
                tool_call_id,
                internal_call_id,
                args,
            } => todo!(),
            StepEvent::ToolResult {
                tool_name,
                tool_call_id,
                internal_call_id,
                args,
                result,
                outcome,
                extensions,
            } => todo!(),
            StepEvent::TextDelta { delta, aggregated } => todo!(),
            StepEvent::ToolCallDelta {
                tool_call_id,
                internal_call_id,
                tool_name,
                delta,
            } => todo!(),
            StepEvent::StreamResponseFinish { prompt, response } => todo!(),
            _ => todo!(),
        };

        Flow::cont()
    }
}

impl AgentUIIpcHook {
    pub fn new(sender: AgentUIIpcSender) -> Self {
        Self(sender)
    }

    fn send(&self, message: AgentUIIpcMessage) {
        let sender = self.0.clone();
        tokio::spawn(async move {
            if let Err(e) = sender.send(message).await {
                tracing::error!("Failed to send message to AgentUIIpcHook: {}", e);
            }
        });
    }
}
