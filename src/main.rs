use rig::client::CompletionClient;
use rig::integrations::cli_chatbot::ChatBotBuilder;
use rig::providers::ollama;

const MODEL: &str = "ornith";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = ollama::Client::new("not-needed")?;

    // Build an agent: a model plus a system prompt (the "preamble").
    let agent = client
        .agent(MODEL)
        .preamble("**You are an AI coding assistant designed to help users build software.** You do not have access to any tools or file systems on the local computer. When working with code, architecture decisions, or implementation details, please ask the user for clarification, information about their existing setup, or confirmation before proceeding with changes.")
        .build();

    let chatbot = ChatBotBuilder::new().agent(agent).show_usage().build();
    chatbot.run().await?;

    Ok(())
}
