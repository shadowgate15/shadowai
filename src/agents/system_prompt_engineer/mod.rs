pub static NAME: &str = "system_prompt_engineer";
pub static DESCRIPTION: &str = include_str!("description.md");
pub static EXAMPLE_PROMPTS: &[&str] = &[
    "Write a system prompt for an agent that reads and edits markdown files",
    "Create prompts following the understand-confirm-act workflow",
    "Draft concise imperative instructions under 400 words",
];

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
        .name(NAME)
        .description(DESCRIPTION)
        .preamble(include_str!("preamble.md"))
        .max_tokens(256 * 1024)
        .temperature(0.2)
        .default_max_turns(100)
        .tool(shadowai_tools::ReadFileTool)
        .tool(shadowai_tools::ListMatchingFilesTool)
        .tool(shadowai_tools::WebFetchTool)
        .tool(shadowai_tools::WebSearch)
        .add_hook(shadowai_tools::RepairToolCall)
        .additional_params(serde_json::json!({
            "think": true
        }))
        .memory(InMemoryConversationMemory::new())
        .build()
}
