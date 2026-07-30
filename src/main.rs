use dialoguer::{
    Editor,
    console::{Style, style},
};
use shadowai_agent::{AgentConfig, run_agent_loop};
use shadowai_agent_ui_ipc::AgentUIIpcMessage;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    init_tracing();

    if let Some(input) = Editor::new().edit("What would you like to work on?")? {
        let input = input.trim();

        println!("{}", style(input).bold().magenta());

        let config = AgentConfig::default();
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
        run_agent_loop(config, input.to_string(), ui_sender).await?;
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
