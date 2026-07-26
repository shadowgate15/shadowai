use std::path::{Path, PathBuf};

use rig::agent::MultiTurnStreamItem;
use rig::providers::ollama;
use rig::streaming::StreamedAssistantContent;
use rig::{client::CompletionClient, prelude::StreamingChat};
use serde_json::json;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, LinesCodec};

const MODEL: &str = "ornith";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let client = ollama::Client::new("not-needed")?;

    // Build an agent: a model plus a system prompt (the "preamble").
    let agent = client
        .agent(MODEL)
        .preamble("**You are an AI coding assistant designed to help users build software.** You have access to tools for reading files (e.g., `ReadFile`), listing files via glob patterns (e.g., `GlobFiles`), and writing/editing files (`Edit`). When working on code, architecture decisions, or implementation details, please ask the user for clarification, information about their existing setup, or confirmation before proceeding with changes.")
        .tool(ReadFile)
        .tool(GlobFiles)
        .tool(EditFile)
        .tool(ShellCommand)
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
                Ok(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::ToolCall {
                        tool_call,
                        internal_call_id,
                    },
                )) => {
                    println!();
                    println!(">>> {}({})", tool_call.function.name, internal_call_id);
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&tool_call.function.arguments)?
                    );
                    println!(">>>");
                }
                Ok(MultiTurnStreamItem::StreamAssistantItem(
                    StreamedAssistantContent::Reasoning(reasoning),
                )) => {
                    println!();
                    println!("=== {}", reasoning.id.unwrap_or_default());

                    for reasoning in reasoning.content {
                        match reasoning {
                            rig::message::ReasoningContent::Text { text, signature: _ } => {
                                println!("{}", text)
                            }
                            rig::message::ReasoningContent::Summary(text) => println!("{}", text),
                            _ => {}
                        }
                    }

                    println!("===");
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

#[rig::tool_macro(
    description = "Replace occurrences of old_text with new_text in a file (creates the file if it doesn't exist).",
    required(file),
    required(old_text),
    required(new_text)
)]
async fn edit_file(
    file: PathBuf,
    old_text: String,
    new_text: String,
) -> Result<String, rig::tool::ToolError> {
    _edit_file(file, old_text, new_text)
        .await
        .map_err(|err| err.into_boxed_dyn_error().into())
}

async fn _edit_file(
    file: PathBuf,
    old_text: String,
    new_text: String,
) -> Result<String, anyhow::Error> {
    let parent = file.parent().unwrap_or(Path::new("."));
    tokio::fs::create_dir_all(parent).await?;

    // Try to open existing file first
    match tokio::fs::File::open(&file).await {
        Ok(mut f) => {
            let mut contents = String::new();
            f.read_to_string(&mut contents).await?;

            let modified = contents.replace(old_text.as_str(), new_text.as_str());

            if modified == contents {
                return Err(anyhow::anyhow!(
                    "No occurrences of '{old_text}' found in the file."
                ));
            }

            let mut out = tokio::fs::File::create(&file).await?;
            out.write_all(modified.as_bytes()).await?;
            Ok(modified)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // File doesn't exist — create it fresh with new_text
            let mut f = tokio::fs::File::create(&file).await?;
            f.write_all(new_text.as_bytes()).await?;
            Ok(new_text)
        }
        Err(e) => Err(e.into()), // Some other error
    }
}

#[rig::tool_macro(
    description = "Execute a bash/shell command on the host machine and return its stdout output.",
    required(command)
)]
async fn shell_command(command: String) -> Result<String, rig::tool::ToolError> {
    _shell_command(command)
        .await
        .map_err(|err| err.into_boxed_dyn_error().into())
}

async fn _shell_command(command: String) -> Result<String, anyhow::Error> {
    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(&command)
        .output()?;

    if !output.status.success() {
        return Err(anyhow::anyhow!(
            "Command failed (exit code {}): {}\n{}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
            String::from_utf8_lossy(&output.stdout)
        ));
    }

    let stdout = String::from_utf8(output.stdout)?;
    Ok(stdout.trim().to_string())
}
