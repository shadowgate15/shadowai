use rig::{
    Agent,
    client::{AgentClientExt, CompletionClient},
    memory::InMemoryConversationMemory,
};

pub fn build<C>(client: &C) -> Agent<C::CompletionModel>
where
    C: CompletionClient,
{
    client
        .agent("qwen3.5:9b")
        .preamble(include_str!("preamble.md"))
        .max_tokens(256 * 1024)
        .temperature(0.2)
        .default_max_turns(100)
        .tool(shadowai_tools::ReadTool)
        .tool(shadowai_tools::GlobTool)
        .tool(shadowai_tools::EditTool)
        .tool(shadowai_tools::ShellTool)
        .tool(shadowai_tools::WebFetchTool)
        .tool(shadowai_tools::WebSearch)
        .add_hook(shadowai_tools::RepairToolCall)
        .additional_params(serde_json::json!({
            "think": true
        }))
        .memory(InMemoryConversationMemory::new())
        .build()
}
