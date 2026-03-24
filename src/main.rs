mod app;
mod auth;
mod config;
mod discord;
mod store;
mod token_file;
mod tui;
mod utils;

use config::Config;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install rustls crypto provider");

    let config = Config::load()?;

    // Init tracing to file
    let data_dir = Config::data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let file_appender = tracing_appender::rolling::daily(&data_dir, "tiscord.log");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_env_filter("tiscord=debug,twilight_gateway=debug")
        .init();

    // Check for --clear-token flag
    if std::env::args().any(|a| a == "--clear-token") {
        auth::clear_token()?;
        eprintln!("Token cleared.");
        return Ok(());
    }

    let token = auth::get_token()?;

    // Create channels
    let (discord_event_tx, discord_event_rx) = tokio::sync::mpsc::unbounded_channel();
    let (action_tx, action_rx) = tokio::sync::mpsc::unbounded_channel();

    // Create HTTP client and spawn action handler
    let http = discord::client::create_http_client(&token);
    tokio::spawn(discord::actions::run_action_handler(
        http,
        action_rx,
        discord_event_tx.clone(),
    ));

    // Spawn gateway
    tokio::spawn(discord::client::run_gateway(token, discord_event_tx));

    // Detect terminal graphics capabilities
    let terminal_caps = tui::terminal_caps::TerminalCapabilities::detect();
    tracing::info!("Terminal graphics: {:?}", terminal_caps.graphics);

    // Create store and app
    let mut initial_store = store::Store::new();
    initial_store.supports_images = terminal_caps.supports_images();
    let store = std::sync::Arc::new(std::sync::RwLock::new(initial_store));
    let mut app = app::App::new(store, action_tx, discord_event_rx, config, terminal_caps);

    // Init terminal and run
    let mut terminal = tui::terminal::init()?;
    let result = app.run(&mut terminal).await;
    tui::terminal::restore()?;
    result
}
