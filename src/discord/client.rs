use anyhow::Result;
use tokio::sync::mpsc;
use twilight_gateway::{EventTypeFlags, Intents, Shard, ShardId, StreamExt as _};
use twilight_http::Client as HttpClient;

use super::events::{translate_event, DiscordEvent};

pub fn create_http_client(token: &str) -> HttpClient {
    HttpClient::new(token.to_string())
}

pub fn required_intents() -> Intents {
    Intents::GUILDS
        | Intents::GUILD_MESSAGES
        | Intents::GUILD_MEMBERS
        | Intents::DIRECT_MESSAGES
        | Intents::MESSAGE_CONTENT
}

pub async fn run_gateway(
    token: String,
    event_tx: mpsc::UnboundedSender<DiscordEvent>,
) -> Result<()> {
    let intents = required_intents();
    let mut shard = Shard::new(ShardId::ONE, token, intents);

    while let Some(item) = shard.next_event(EventTypeFlags::all()).await {
        let event = match item {
            Ok(event) => event,
            Err(e) => {
                tracing::error!("gateway error: {e}");
                let _ = event_tx.send(DiscordEvent::GatewayDisconnect);
                continue;
            }
        };

        if let Some(discord_event) = translate_event(event) {
            let _ = event_tx.send(discord_event);
        }
    }

    Ok(())
}
