use shadowai_agent::{AgentConfig, run_agent_loop};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
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

    let config = AgentConfig::default();
    let (ui_sender, mut ui_receiver) = shadowai_agent_ui_ipc::get_ipc_channel();

    tokio::spawn(async move {
        while let Some(message) = ui_receiver.recv().await {
            println!("| {:?}", message);
        }
    });
    run_agent_loop(config, ui_sender).await?;

    Ok(())
}
