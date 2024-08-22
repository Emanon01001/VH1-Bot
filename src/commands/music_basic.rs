use crate::Context;
use crate::Error;

use hound::WavSpec;
use hound::WavWriter;
use lavalink_rs::prelude::*;
use poise::serenity_prelude::Color;
use poise::serenity_prelude::CreateEmbed;
use poise::serenity_prelude::CreateMessage;
use std::collections::VecDeque;
use std::num::NonZeroU64;
use std::ops::Deref;

use poise::serenity_prelude as serenity;
use serenity::{model::id::ChannelId, Http, Mentionable};
use songbird::CoreEvent;

use super::voice_receive::Receiver;

pub async fn _join(
    ctx: &Context<'_>,
    guild_id: serenity::GuildId,
    channel_id: Option<serenity::ChannelId>,
    receive: Option<String>,
) -> Result<bool, Error> {
    let lava_client = ctx.data().lavalink.clone();

    let manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();

    if lava_client
        .get_player_context(lavalink_guild_id(guild_id))
        .is_none()
    {
        let connect_to = match channel_id {
            Some(x) => x,
            None => {
                let guild = ctx.guild().unwrap().deref().clone();
                let user_channel_id = guild
                    .voice_states
                    .get(&ctx.author().id)
                    .and_then(|voice_state| voice_state.channel_id);

                match user_channel_id {
                    Some(channel) => channel,
                    None => {
                        return Err("Not in a voice channel".into());
                    }
                }
            }
        };

        match receive {
            Some(word) => {
                if word.as_str() == "receive" {
                    let _ = WavWriter::create(
                        "output.wav",
                        WavSpec {
                            channels: 2,
                            sample_rate: 48000,
                            bits_per_sample: 16,
                            sample_format: hound::SampleFormat::Int,
                        },
                    )
                    .unwrap();

                    let handler = manager
                        .join(
                            songbird::id::GuildId::from(NonZeroU64::from(guild_id)),
                            connect_to,
                        )
                        .await;

                    match handler {
                        Ok(handler_lock) => {
                            let mut handler = handler_lock.lock().await;
                            let receiver = Receiver::new();

                            handler.add_global_event(CoreEvent::VoiceTick.into(), receiver.clone());
                            handler.add_global_event(
                                CoreEvent::ClientDisconnect.into(),
                                receiver.clone(),
                            );

                            let _ = ctx.say(format!("Joined {}", connect_to.mention())).await?;

                            return Ok(true);
                        }
                        Err(why) => {
                            let _ = ctx
                                .say(format!("Error joining the channel: {}", why))
                                .await?;
                            return Err(why.into());
                        }
                    }
                }
            }
            None => {
                let handler = manager
                    .join_gateway(
                        songbird::id::GuildId::from(NonZeroU64::from(guild_id)),
                        connect_to,
                    )
                    .await;

                match handler {
                    Ok((connection_info, _)) => {
                        lava_client
                            .create_player_context_with_data::<(ChannelId, std::sync::Arc<Http>)>(
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

                        let _ = ctx.say(format!("Joined {}", connect_to.mention())).await?;

                        return Ok(true);
                    }
                    Err(why) => {
                        let _ = ctx
                            .say(format!("Error joining the channel: {}", why))
                            .await?;
                        return Err(why.into());
                    }
                }
            }
        }
    }

    Ok(false)
}

/// Play a song in the voice channel you are connected in.
#[poise::command(slash_command)]
pub async fn play(
    ctx: Context<'_>,
    #[description = "Search term or URL"]
    #[rest]
    term: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let _connect = _join(&ctx, guild_id, None, None).await?;
    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(lavalink_guild_id(guild_id)) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    let query = if let Some(term) = term {
        if term.starts_with("http") {
            term
        } else {
            SearchEngines::Deezer.to_query(&term)?
        }
    } else {
        if let Ok(player_data) = player.get_player().await {
            let queue = player.get_queue();

            if player_data.track.is_none() && queue.get_track(0).await.is_ok_and(|x| x.is_some()) {
                player.skip()?;
            } else {
                ctx.say("The queue is empty.").await?;
            }
        }

        return Ok(());
    };
    let loaded_tracks = lava_client
        .load_tracks(lavalink_guild_id(guild_id), &query)
        .await?;

    let mut playlist_info = None;

    let mut tracks: VecDeque<TrackInQueue> = match loaded_tracks.data {
        Some(TrackLoadData::Track(x)) => {
            let mut v: VecDeque<TrackInQueue> = VecDeque::new();
            v.push_back(TrackInQueue::from(x));
            v
        }
        Some(TrackLoadData::Search(x)) => vec![x[0].clone().into()].into(),
        Some(TrackLoadData::Playlist(x)) => {
            println!("Playlist");
            playlist_info = Some(x.info);
            x.tracks.iter().map(|x| x.clone().into()).collect()
        }
        _ => {
            ctx.say(format!("{:?}", loaded_tracks)).await?;
            return Ok(());
        }
    };

    if let Some(info) = playlist_info {
        let embed = CreateEmbed::new()
            .color(Color::DARK_BLUE)
            .description(format!("Added playlist to queue: **{}**", info.name,));
        let builder = CreateMessage::new().tts(false).embed(embed);

        let _ = ctx.channel_id().send_message(&ctx.http(), builder).await?;
    } else {
        let track = &tracks[0].track;

        if let Some(uri) = &track.info.uri {
            let _ = ctx.say(uri).await.unwrap();

            let embed = CreateEmbed::new()
                .color(Color::DARK_BLUE)
                .description(format!(
                    "Added to queue: [{} - {}](<{}>)",
                    track.info.author, track.info.title, uri
                ));
            let builder = CreateMessage::new().tts(false).embed(embed);

            let _ = ctx.channel_id().send_message(&ctx.http(), builder).await?;
        } else {
            let embed = CreateEmbed::new()
                .color(Color::DARK_BLUE)
                .description(format!(
                    "Added to queue: {} - {}",
                    track.info.author, track.info.title
                ));
            let builder = CreateMessage::new().tts(false).embed(embed);

            let _ = ctx.channel_id().send_message(&ctx.http(), builder).await?;
        }
    }

    for i in &mut tracks {
        i.track.user_data = Some(serde_json::json!({"requester_id": ctx.author().id.get()}));
    }

    let queue = player.get_queue();
    queue.append(tracks)?;

    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

    if let Ok(_player_data) = player.get_player().await {
        match &queue.get_track(0).await {
            Ok(_track_queue) => (),
            Err(err) => println!("\n\n\n{}", err),
        }
    }
    Ok(())
}

#[poise::command(slash_command)]
pub async fn join(
    ctx: Context<'_>,
    channel_id: Option<serenity::ChannelId>,
    receive: Option<String>,
) -> Result<(), Error> {
    let guild_id = ctx.guild_id().unwrap();

    let _ = _join(&ctx, guild_id, channel_id, receive).await?;

    Ok(())
}
/// Leave the current voice channel.
#[poise::command(slash_command)]
pub async fn leave(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = match ctx.guild_id() {
        Some(guild_id) => guild_id,
        None => {
            ctx.say("Could not find the guild ID.").await?;
            return Ok(());
        }
    };

    let manager = songbird::get(ctx.serenity_context()).await.unwrap().clone();
    let lava_client = ctx.data().lavalink.clone();

    // Lavalinkのプレイヤー削除
    match lava_client.delete_player(lavalink_guild_id(guild_id)).await {
        Ok(_) => (),
        Err(err) => {
            println!("Error deleting Lavalink player: {}", err);
        }
    }

    // Songbirdのボイスチャンネルからの退出
    if let Some(handler) = manager.get(guild_id) {
        let _ = handler.lock().await.leave().await;
        ctx.say("Left the voice channel.").await?;
    } else {
        ctx.say("Not connected to a voice channel.").await?;
    }

    Ok(())
}

fn lavalink_guild_id(guild_id: serenity::GuildId) -> lavalink_rs::model::GuildId {
    lavalink_rs::model::GuildId::from(u64::from(guild_id))
}
