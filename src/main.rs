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
        .preamble("You are a coding assistant that helps the user build software.")
        .build();

    let chatbot = ChatBotBuilder::new().agent(agent).show_usage().build();
    chatbot.run().await?;

    Ok(())
}
