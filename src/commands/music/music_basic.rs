use crate::Context;
use crate::Error;

use lavalink_rs::prelude::*;
use poise::serenity_prelude::{Color, CreateEmbed, CreateMessage};
use std::collections::VecDeque;
use std::num::NonZeroU64;

use poise::serenity_prelude as serenity;
use serenity::{Http, Mentionable};

/// ギルドIDから Lavalink 用の GuildId に変換するヘルパー
fn lavalink_guild_id(guild_id: serenity::GuildId) -> lavalink_rs::model::GuildId {
    lavalink_rs::model::GuildId::from(u64::from(guild_id))
}

/// Songbird に接続してプレイヤーコンテキストを作成する
pub async fn _join(
    ctx: &Context<'_>,
    guild_id: serenity::GuildId,
    channel_id: Option<serenity::ChannelId>,
) -> Result<bool, Error> {
    let lava_client = &ctx.data().lavalink;
    let manager = songbird::get(ctx.serenity_context())
        .await
        .ok_or("Songbird not initialized")?
        .clone();

    // まだプレイヤーが存在しなければ接続処理を行う
    if lava_client
        .get_player_context(lavalink_guild_id(guild_id))
        .is_none()
    {
        // 指定がなければ、ユーザーの接続しているチャンネルを取得
        let connect_to = if let Some(ch) = channel_id {
            ch
        } else {
            let guild = ctx.guild().ok_or("Guild not found")?;
            guild
                .voice_states
                .get(&ctx.author().id)
                .and_then(|state| state.channel_id)
                .ok_or("Not in a voice channel")?
        };

        // Songbird で接続
        let join_result = manager
            .join_gateway(
                songbird::id::GuildId::from(
                    NonZeroU64::new(u64::from(guild_id)).ok_or("Invalid guild id")?,
                ),
                connect_to,
            )
            .await;

        match join_result {
            Ok((connection_info, _)) => {
                lava_client
                    .create_player_context_with_data::<(serenity::ChannelId, std::sync::Arc<Http>)>(
                        lavalink_guild_id(guild_id),
                        lavalink_rs::model::player::ConnectionInfo {
                            endpoint: connection_info.endpoint,
                            token: connection_info.token,
                            session_id: connection_info.session_id,
                        },
                        std::sync::Arc::new((
                            ctx.channel_id(),
                            ctx.serenity_context().http.clone(),
                        )),
                    )
                    .await?;
                ctx.say(format!("Joined {}", connect_to.mention())).await?;
                Ok(true)
            }
            Err(why) => {
                ctx.say(format!("Error joining the channel: {}", why))
                    .await?;
                Err(why.into())
            }
        }
    } else {
        Ok(false)
    }
}

/// 曲を再生するコマンド
#[poise::command(prefix_command)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Search term or URL"]
    #[rest]
    term: Option<String>,
) -> Result<(), Error> {
    ctx.defer().await?;
    let guild_id = ctx.guild_id().ok_or("Guild ID not found")?;
    // チャンネルに接続
    let _ = _join(&ctx, guild_id, None).await?;
    let lava_client = &ctx.data().lavalink;

    let player = if let Some(p) = lava_client.get_player_context(lavalink_guild_id(guild_id)) {
        p
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    let query = if let Some(term) = term {
        if term.starts_with("http") {
            term
        } else {
            // ここでは例として Deezer の検索クエリに変換
            SearchEngines::Deezer.to_query(&term)?
        }
    } else {
        // term が指定されていなければ、キューに曲が入っているかチェックし、なければエラー
        let player_data = player.get_player().await?;
        if player_data.track.is_none()
            && player
                .get_queue()
                .get_track(0)
                .await
                .is_ok_and(|x| x.is_some())
        {
            player.skip()?;
        } else {
            ctx.say("The queue is empty.").await?;
        }
        return Ok(());
    };

    let loaded_tracks = lava_client
        .load_tracks(lavalink_guild_id(guild_id), &query)
        .await?;
    let mut playlist_info = None;
    let mut tracks: VecDeque<TrackInQueue> = match loaded_tracks.data {
        Some(TrackLoadData::Track(x)) => {
            let mut v = VecDeque::new();
            v.push_back(TrackInQueue::from(x));
            v
        }
        Some(TrackLoadData::Search(x)) => VecDeque::from([x[0].clone().into()]),
        Some(TrackLoadData::Playlist(x)) => {
            playlist_info = Some(x.info);
            x.tracks.iter().map(|x| x.clone().into()).collect()
        }
        _ => {
            ctx.say(format!("{:?}", loaded_tracks)).await?;
            return Ok(());
        }
    };

    // プレイリストの場合のフィードバック
    if let Some(info) = playlist_info {
        let embed = CreateEmbed::new()
            .color(Color::DARK_BLUE)
            .description(format!("Added playlist to queue: **{}**", info.name));
        let builder = CreateMessage::new().tts(false).embed(embed);
        let _ = ctx.channel_id().send_message(&ctx.http(), builder).await?;
    } else if let Some(track) = tracks.front() {
        if let Some(uri) = &track.track.info.uri {
            let _ = ctx.say(uri).await?;
            let embed = CreateEmbed::new()
                .color(Color::DARK_BLUE)
                .description(format!(
                    "Added to queue: [{} - {}](<{}>)",
                    track.track.info.author, track.track.info.title, uri
                ));
            let builder = CreateMessage::new().tts(false).embed(embed);
            let _ = ctx.channel_id().send_message(&ctx.http(), builder).await?;
        } else {
            let embed = CreateEmbed::new()
                .color(Color::DARK_BLUE)
                .description(format!(
                    "Added to queue: {} - {}",
                    track.track.info.author, track.track.info.title
                ));
            let builder = CreateMessage::new().tts(false).embed(embed);
            let _ = ctx.channel_id().send_message(&ctx.http(), builder).await?;
        }
    }

    // ユーザー情報を各トラックに付与
    for track in &mut tracks {
        track.track.user_data = Some(serde_json::json!({
            "requester_id": ctx.author().id.get()
        }));
    }

    let queue = player.get_queue();
    queue.append(tracks)?;
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    Ok(())
}

/// ボイスチャンネルに接続するコマンド
#[poise::command(prefix_command)]
pub async fn join(ctx: Context<'_>, channel_id: Option<serenity::ChannelId>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Guild ID not found")?;
    let _ = _join(&ctx, guild_id, channel_id).await?;
    Ok(())
}

/// ボイスチャンネルから退出するコマンド
#[poise::command(prefix_command)]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = ctx.guild_id().ok_or("Guild ID not found")?;
    let manager = songbird::get(ctx.serenity_context())
        .await
        .ok_or("Songbird not initialized")?
        .clone();
    let lava_client = &ctx.data().lavalink;

    // Lavalink プレイヤーの削除
    if let Err(err) = lava_client.delete_player(lavalink_guild_id(guild_id)).await {
        println!("Error deleting Lavalink player: {}", err);
    }

    // Songbird からの退出
    if let Some(handler) = manager.get(guild_id) {
        handler.lock().await.leave().await?;
        ctx.say("Left the voice channel.").await?;
    } else {
        ctx.say("Not connected to a voice channel.").await?;
    }
    Ok(())
}
