use rig::agent::{
    AgentHook, CompletionCallAction, CompletionCallEvent, CompletionResponseEvent, HookContext,
    ModelTurnAction, ModelTurnFinished, ObservationAction, StepEventKind, StreamResponseFinish,
    TextDelta, ToolCall, ToolCallAction, ToolCallDelta, ToolResultAction, ToolResultEvent,
};

use crate::{AgentUIIpcMessage, AgentUIIpcSender};

#[derive(Clone)]
pub struct AgentUIIpcHook(AgentUIIpcSender);

impl AgentHook for AgentUIIpcHook {
    async fn on_completion_call(
        &self,
        ctx: &HookContext,
        _event: CompletionCallEvent<'_>,
    ) -> CompletionCallAction {
        self.send(AgentUIIpcMessage::Start {
            metadata: ctx.into(),
        });

        CompletionCallAction::Continue
    }

    async fn on_completion_response(
        &self,
        ctx: &HookContext,
        _event: CompletionResponseEvent<'_>,
    ) -> ObservationAction {
        self.send(AgentUIIpcMessage::Finish {
            metadata: ctx.into(),
        });

        ObservationAction::Continue
    }

    async fn on_model_turn_finished(
        &self,
        ctx: &HookContext,
        _event: ModelTurnFinished<'_>,
    ) -> ModelTurnAction {
        self.send(AgentUIIpcMessage::ModelTurnFinished {
            metadata: ctx.into(),
        });

        ModelTurnAction::Continue
    }

    async fn on_tool_call(&self, ctx: &HookContext, event: ToolCall<'_>) -> ToolCallAction {
        self.send(AgentUIIpcMessage::ToolCall {
            metadata: ctx.into(),
            name: event.tool_name.to_string(),
            id: event.internal_call_id.to_string(),
        });

        ToolCallAction::Run
    }

    async fn on_tool_result(
        &self,
        ctx: &HookContext,
        event: ToolResultEvent<'_>,
    ) -> ToolResultAction {
        self.send(AgentUIIpcMessage::ToolResult {
            metadata: ctx.into(),
            name: event.tool_name.to_string(),
            id: event.internal_call_id.to_string(),
            result: event.presentation.render(),
        });

        ToolResultAction::Keep
    }

    async fn on_text_delta(&self, ctx: &HookContext, event: TextDelta<'_>) -> ObservationAction {
        self.send(AgentUIIpcMessage::TextDelta {
            metadata: ctx.into(),
            delta: event.delta.to_string(),
        });

        ObservationAction::Continue
    }

    async fn on_tool_call_delta(
        &self,
        ctx: &HookContext,
        event: ToolCallDelta<'_>,
    ) -> ObservationAction {
        self.send(AgentUIIpcMessage::ToolCallDelta {
            metadata: ctx.into(),
            id: event.internal_call_id.to_string(),
            delta: event.delta.to_string(),
        });

        ObservationAction::Continue
    }

    async fn on_stream_response_finish(
        &self,
        ctx: &HookContext,
        _event: StreamResponseFinish<'_>,
    ) -> ObservationAction {
        self.send(AgentUIIpcMessage::Finish {
            metadata: ctx.into(),
        });

        ObservationAction::Continue
    }

    fn observes(&self, kind: StepEventKind) -> bool {
        kind != StepEventKind::InvalidToolCall
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
