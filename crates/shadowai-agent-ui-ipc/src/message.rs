pub enum AgentUIIpcMessage {
    /// Request to LLM has started. This is sent when the agent starts processing a request.
    Start {
        /// Name of the agent that is running. This is optional because some agents may not have a name.
        agent_name: Option<String>,
        /// Run ID of the agent that is running. This is used to correlate messages from the same run.
        run_id: String,
    },
    /// Request to LLM has finished. This is sent when the agent finishes processing a request.
    Finish {
        /// Name of the agent that is running. This is optional because some agents may not have a name.
        agent_name: Option<String>,
        /// Run ID of the agent that is running. This is used to correlate messages from the same run.
        run_id: String,
    },
}
