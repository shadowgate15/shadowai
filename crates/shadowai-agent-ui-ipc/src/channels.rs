use crate::AgentUIIpcMessage;

pub type AgentUIIpcSender = tokio::sync::mpsc::Sender<AgentUIIpcMessage>;
pub type AgentUIIpcReceiver = tokio::sync::mpsc::Receiver<AgentUIIpcMessage>;

pub fn get_ipc_channel() -> (AgentUIIpcSender, AgentUIIpcReceiver) {
    tokio::sync::mpsc::channel(100)
}
