use rig::agent::MultiTurnStreamItem;
use rig::providers::ollama;
use rig::streaming::StreamedAssistantContent;
use rig::{client::CompletionClient, prelude::StreamingChat};
use tokio::io;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, LinesCodec};

const MODEL: &str = "ornith";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = ollama::Client::new("not-needed")?;

    // Build an agent: a model plus a system prompt (the "preamble").
    let agent = client
        .agent(MODEL)
        .preamble("**You are an AI coding assistant designed to help users build software.** You do not have access to any tools or file systems on the local computer. When working with code, architecture decisions, or implementation details, please ask the user for clarification, information about their existing setup, or confirmation before proceeding with changes.")
        .build();

    println!(
        "Welcome! Type your message and press Enter. (Multi-line input is supported by wrapping in \"\"\".)"
    );
    println!("Type 'exit' to exit.");
    println!();

    let mut history = vec![];

    while let UserInput::Input(input) = get_user_input().await? {
        println!();
        println!("{} response start {}", "=".repeat(10), "=".repeat(10));
        println!();
        let mut stream = agent.stream_chat(input, &history).await;

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(MultiTurnStreamItem::FinalResponse(fin)) => {
                    history.extend_from_slice(fin.messages().unwrap_or_default());

                    break;
                }
                Ok(MultiTurnStreamItem::StreamAssistantItem(StreamedAssistantContent::Text(
                    text,
                ))) => {
                    print!("{}", text);
                }
                Ok(_other) => { /* Do something with this chunk */ }
                Err(e) => return Err(e.into()),
            }
        }

        println!();
        println!();
        println!("{} response end {}", "=".repeat(10), "=".repeat(10));
        println!();
    }

    Ok(())
}

enum UserInput {
    Input(String),
    Exit,
}

async fn get_user_input() -> Result<UserInput, anyhow::Error> {
    let stdin = io::stdin();
    let mut lines = FramedRead::new(stdin, LinesCodec::new());

    let mut is_multi_line = false;
    let mut input = String::new();
    while let Some(line) = lines.next().await {
        let line = line?;

        if line.trim() == "exit" {
            return Ok(UserInput::Exit);
        } else if line.trim() == "\"\"\"" {
            is_multi_line = !is_multi_line;
        } else {
            input.push_str(&line);

            if is_multi_line {
                input.push('\n');
            }
        }

        if !is_multi_line {
            break;
        }
    }

    Ok(UserInput::Input(input))
}
