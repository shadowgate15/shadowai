use rig::client::CompletionClient;
use rig::completion::Prompt;
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

    // Send a prompt and await the model's reply.
    let response = agent
        .prompt("What is the Rust programming language?")
        .await?;

    println!("{response}");

    Ok(())
}
