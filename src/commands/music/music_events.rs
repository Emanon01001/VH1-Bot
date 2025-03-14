use lavalink_rs::{hook, model::events::{self, TrackEndReason}, prelude::*};
use poise::serenity_prelude::{Color, CreateEmbed, CreateMessage};
use tracing::info;

use crate::commands::music::music_basic::PlayerState;


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
    let data = player_context.data::<tokio::sync::Mutex<PlayerState>>().unwrap();
    let state = data.lock().await;
    let channel_id = state.text_channel_id;
    let http = state.http.clone();    
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

    if let Err(e) = channel_id.send_message(&http, message).await {
        println!("Error sending message in track_start hook: {:?}\n\n\n\n", e);
    }
}

#[hook]
pub async fn track_end(client: LavalinkClient, _session_id: String, event: &events::TrackEnd) {
    info!("Track ended: {:?}", event);

    // プレイヤーコンテキストを取得
    if let Some(player_context) = client.get_player_context(event.guild_id) {
        // PlayerStateをMutexから取得
        if let Ok(state) = player_context.data::<tokio::sync::Mutex<PlayerState>>() {
            let state = state.lock().await;
            // リピートがON & 自然終了(Finished)の場合だけもう一度同じ曲をキュー先頭へ入れる
            if state.repeat && event.reason == TrackEndReason::Finished {
                // 再度同じTrackをキューへ積む
                let q = player_context.get_queue();
                // 今回終わった曲(event.track)をもう一度追加
                q.push_to_front(event.track.clone()).unwrap();
            }
        }
    }
}