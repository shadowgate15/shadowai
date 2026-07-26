use rig::client::CompletionClient;
use rig::prelude::StreamingChat;
use rig::streaming::StreamedAssistantContent;
use rig::{agent::MultiTurnStreamItem, providers::ollama};

use anyhow::Result;
use serde_json::json;
use tokio::io;
use tokio_stream::StreamExt;
use tokio_util::codec::{FramedRead, LinesCodec};

/// Configuration for the agent loop.
pub struct AgentConfig {
    pub model: String,
    pub system_prompt: String,
    pub temperature: f64,
    pub default_max_turns: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            model: "ornith".to_string(),
            system_prompt: "**You are an AI coding assistant designed to help users build software.** You have access to tools for reading files (e.g., `ReadFile`), listing files via glob patterns (e.g., `GlobFiles`), and writing/editing files (`Edit`). When working on code, architecture decisions, or implementation details, please ask the user for clarification, information about their existing setup, or confirmation before proceeding with changes.".to_string(),
            temperature: 0.6,
            default_max_turns: 100,
        }
    }
}

/// Internal representation of a user's input line (single-line or multi-line).
enum UserInput {
    Input(String),
    Exit,
}

/// Manages the message history for the conversation.
pub struct ConversationHistory {
    messages: Vec<rig::message::Message>,
}

impl ConversationHistory {
    pub fn new() -> Self {
        Self { messages: vec![] }
    }

    /// Returns a reference to the current message slice used by rig.
    pub fn messages(&self) -> &[rig::message::Message] {
        &self.messages
    }

    /// Extend the history with new assistant/user messages from a finished stream.
    pub fn extend_from_slice(&mut self, msgs: &[rig::message::Message]) {
        self.messages.extend_from_slice(msgs);
    }

    /// Reset history (clears all stored messages).
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

impl Default for ConversationHistory {
    fn default() -> Self {
        Self::new()
    }
}

/// Get a single line of user input from stdin, supporting multi-line with `"""`.
async fn get_user_input() -> Result<UserInput> {
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

/// Run the agent conversation loop. This builds a rig `Agent` with the given config,
/// registers tools from `shadowai-tools`, adds the repair hook, and drives the
/// multi-turn streaming chat loop until the user types "exit".
pub async fn run_agent_loop(config: AgentConfig) -> Result<()> {
    let client = ollama::Client::new("not-needed")?;

    // Build the agent with the configured model + preamble.
    let agent = client
        .agent(&config.model)
        .preamble(&config.system_prompt)
        .tool(shadowai_tools::ReadFile)
        .tool(shadowai_tools::GlobFiles)
        .tool(shadowai_tools::EditFile)
        .tool(shadowai_tools::ShellCommand)
        .add_hook(shadowai_tools::RepairToolCall)
        .default_max_turns(config.default_max_turns)
        .temperature(config.temperature)
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

    let mut history = ConversationHistory::new();

    while let UserInput::Input(input) = get_user_input().await? {
        println!();
        println!("{} response start {}", "=".repeat(10), "=".repeat(10));
        println!();

        let mut stream = agent.stream_chat(input, &history.messages).await;

        while let Some(msg) = stream.next().await {
            match msg {
                Ok(MultiTurnStreamItem::FinalResponse(fin)) => {
                    history.extend_from_slice(fin.messages().unwrap_or_default());

                    println!();
                    println!();
                    let usage = fin.usage();
                    println!(
                        "{} response end ({}/{}) {}",
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

                    for reasoning in &reasoning.content {
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

// Re-export key items from shadowai-tools so downstream crates can use them directly.
pub use shadowai_tools::{edit_file, glob_files, read_file, shell_command};
