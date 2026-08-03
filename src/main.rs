mod agents;

use anyhow::Result;
use dialoguer::{
    Editor,
    console::{Style, style},
};
use rig::{completion::Chat, message::Message, providers::ollama};
use shadowai_agent_ui_ipc::{AgentUIIpcMessage, AgentUIIpcSender};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init_tracing();

    if let Some(input) = Editor::new().edit("What would you like to work on?")? {
        let input = input.trim();

        println!("{}", style(input).bold().magenta());

        let (ui_sender, mut ui_receiver) = shadowai_agent_ui_ipc::get_ipc_channel();

        tokio::spawn(async move {
            while let Some(message) = ui_receiver.recv().await {
                match message {
                    AgentUIIpcMessage::ToolResult {
                        metadata: _,
                        name,
                        id: _,
                        result,
                    } => {
                        let style = Style::new().color256(244);

                        println!();
                        println!("{}", style.clone().underlined().bold().apply_to(name));
                        println!("{}", style.apply_to(result));
                        println!();
                    }
                    AgentUIIpcMessage::TextDelta { metadata: _, delta } => {
                        print!("{}", delta);
                    }
                    _ => {}
                }
            }
        });

        run_agent_loop(input, ui_sender).await?;
    }

    Ok(())
}

async fn run_agent_loop(input: &str, sender: AgentUIIpcSender) -> Result<()> {
    let client = ollama::Client::new("not needed")?;
    let ipc_hook = shadowai_agent_ui_ipc::AgentUIIpcHook::new(sender);

    let agent = agents::development::build(&client);

    let mut prompt = input.to_string();

    loop {
        agent
            .runner(prompt)
            .add_hook(ipc_hook.clone())
            .conversation("development")
            .run()
            .await?;

        prompt = dialoguer::Input::<String>::new()
            .with_prompt(format!("{}", style("> ").blue().bold()))
            .interact_text()?;

        if prompt == "exit" || prompt == "quit" {
            break;
        }
    }

    Ok(())
}

fn init_tracing() {
    #[cfg(debug_assertions)]
    let app_name = "shadowai-dev";
    #[cfg(not(debug_assertions))]
    let app_name = "shadowai";

    let project_dirs = directories::ProjectDirs::from("com", "shadowgate15", app_name).unwrap();
    let file_appender = tracing_appender::rolling::daily(project_dirs.data_dir(), "app.log");
    let (non_blocking_file, _guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking_file)
                .with_ansi(false),
        )
        .init();
}
