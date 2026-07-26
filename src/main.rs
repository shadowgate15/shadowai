use std::path::PathBuf;

use rig::agent::MultiTurnStreamItem;
use rig::providers::ollama;
use rig::streaming::StreamedAssistantContent;
use rig::{client::CompletionClient, prelude::StreamingChat};
use serde_json::json;
use tokio::io::{self, AsyncReadExt};
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
        .tool(ReadFile)
        .tool(GlobFiles)
        .default_max_turns(5)
        .temperature(0.6)
        .additional_params(json!({
            "options": {
                "num_ctx": 32768,
                "num_predict": -1,
                "top_p": 0.95,
                "top_k": 20
            }
        }))
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

                    println!();
                    println!();
                    let usage = fin.usage();
                    println!(
                        "{} rsponse end ({}/{}) {}",
                        "=".repeat(10),
                        usage.input_tokens,
                        usage.output_tokens,
                        "=".repeat(10)
                    );
                    println!();

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

#[rig::tool_macro(description = "Read a file from the filesystem", required(file))]
async fn read_file(file: PathBuf) -> Result<String, rig::tool::ToolError> {
    _read_file(file)
        .await
        .map_err(|err| err.into_boxed_dyn_error().into())
}

async fn _read_file(file: PathBuf) -> Result<String, anyhow::Error> {
    let mut file = tokio::fs::File::open(file).await?;
    let mut contents = String::new();

    file.read_to_string(&mut contents).await?;

    Ok(contents)
}

#[rig::tool_macro(
    description = "List files matching a glob pattern (e.g., *.rs, src/**/*)",
    required(pattern)
)]
async fn glob_files(pattern: String) -> Result<Vec<String>, rig::tool::ToolError> {
    _glob_files(pattern)
        .await
        .map_err(|err| err.into_boxed_dyn_error().into())
}

async fn _glob_files(pattern: String) -> Result<Vec<String>, anyhow::Error> {
    let mut results = Vec::new();
    for entry in glob::glob(&pattern)? {
        let path = entry?;
        if path.is_file() {
            results.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(results)
}
