use lavalink_rs::{
    hook,
    model::events::{self, TrackEndReason},
    prelude::*,
};
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
    let data = player_context
        .data::<tokio::sync::Mutex<PlayerState>>()
        .unwrap();
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
    if let Some(player_context) = client.get_player_context(event.guild_id) {
        if let Ok(state) = player_context.data::<tokio::sync::Mutex<PlayerState>>() {
            let state = state.lock().await;
            if state.repeat && event.reason == TrackEndReason::Finished {
                let queue = player_context.get_queue();

                // 1) 元のトラック情報(event.track.info.uriなど)を取得
                let old_info = &event.track.info;
                if let Some(uri) = &old_info.uri {
                    // 2) 同じURIで再ロード
                    let guild_id = player_context.guild_id;
                    let loaded = client.load_tracks(guild_id, uri).await;
                    if let Ok(loaded_tracks) = loaded {
                        if let Some(TrackLoadData::Track(track_data)) = loaded_tracks.data {
                            // 3) 新しいトラックを先頭に追加
                            queue.push_to_front(track_data).unwrap();
                        } else if let Some(TrackLoadData::Search(mut tracks)) = loaded_tracks.data {
                            // 先頭を使用 (YouTube等の検索にヒットした場合)
                            if let Some(first) = tracks.pop() {
                                queue.push_to_front(first).unwrap();
                            }
                        }
                        // TODO: Playlist やその他ケースへの対応
                    }
                }
            }
        }
    }
}
