use lavalink_rs::{hook, model::events, prelude::*};
use poise::serenity_prelude::{ChannelId, Color, CreateEmbed, CreateMessage, Http, Message};
use tracing::info;


#[hook]
pub async fn raw_event(_: LavalinkClient, session_id: String, event: &serde_json::Value) {
    if event["op"].as_str() == Some("event") || event["op"].as_str() == Some("playerUpdate") {
        info!("{:?} -> {:?}", session_id, event);
    }
}

#[hook]
pub async fn ready_event(client: LavalinkClient, session_id: String, event: &events::Ready) {
    client.delete_all_player_contexts().await.unwrap();
    info!("{:?} -> {:?}", session_id, event);
}

#[hook]
pub async fn track_start(client: LavalinkClient, _session_id: String, event: &events::TrackStart) {
    let player_context = client.get_player_context(event.guild_id).unwrap();
    let data = player_context
        .data::<(ChannelId, std::sync::Arc<Http>)>()
        .unwrap();
    let (channel_id, http) = (&data.0, &data.1,);
    let message = {
        let track = &event.track;

        if let Some(uri) = &track.info.uri {

            let embed = CreateEmbed::new()
                .color(Color::DARK_BLUE)
                .title("Started playing")
                .url(uri)
                .field(&track.info.title, &track.info.author, false)
                .timestamp(poise::serenity_prelude::model::Timestamp::now());

            CreateMessage::new().tts(false).embed(embed)
        } else {
            let embed = CreateEmbed::new()
                .color(Color::DARK_BLUE)
                .title("Started playing")
                .field(&track.info.title, &track.info.author, false)
                .timestamp(poise::serenity_prelude::model::Timestamp::now());

            CreateMessage::new().tts(false).embed(embed)
        }
    };

    let _ = channel_id.send_message(&http, message).await.unwrap();
}
