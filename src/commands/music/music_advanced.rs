use lavalink_rs::prelude::PlayerContext;
use poise::serenity_prelude as serenity;
use poise::serenity_prelude::colours::roles::DARK_BLUE;
use poise::serenity_prelude::{CreateEmbed, CreateMessage};
use std::time::Duration;

use crate::Context;
use crate::Error;

/// ヘルパー関数：ギルドIDの取得およびプレイヤーコンテキストの取得を行う
async fn get_player_context_from_ctx(ctx: &Context<'_>) -> Option<PlayerContext> {
    let guild_id = ctx.guild_id()?;
    let lava_client = &ctx.data().lavalink;
    lava_client.get_player_context(lavalink_guild_id(guild_id))
}

/// Skip the current song.
#[poise::command(prefix_command)]
pub async fn skip(ctx: Context<'_>, number: Option<usize>) -> Result<(), Error> {
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        let now_playing = player.get_player().await?.track;
        if let Some(np) = now_playing {
            if let Some(n) = number {
                for _ in 0..n {
                    player.skip()?;
                }
                ctx.say(format!("Skipped {} tracks.", n)).await?;
            } else {
                player.skip()?;
                ctx.say(format!("Skipped: {}", np.info.title)).await?;
            }
        } else {
            ctx.say("Nothing to skip.").await?;
        }
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }
    Ok(())
}

/// Pause the current song.
#[poise::command(prefix_command)]
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        player.set_pause(true).await?;
        ctx.say("Paused.").await?;
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }
    Ok(())
}

/// Resume playing the current song.
#[poise::command(prefix_command)]
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        player.set_pause(false).await?;
        ctx.say("Resumed playback.").await?;
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }
    Ok(())
}

/// Stop the current song.
#[poise::command(prefix_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        let now_playing = player.get_player().await?.track;
        if let Some(np) = now_playing {
            player.stop_now().await?;
            ctx.say(format!("Stopped: {}", np.info.title)).await?;
        } else {
            ctx.say("Nothing to stop.").await?;
        }
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }
    Ok(())
}

/// Jump to a specific time in the song, in seconds.
#[poise::command(prefix_command)]
pub async fn seek(
    ctx: Context<'_>,
    #[description = "Time to jump to (in seconds)"] time: u64,
) -> Result<(), Error> {
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        if player.get_player().await?.track.is_some() {
            player.set_position(Duration::from_secs(time)).await?;
            ctx.say(format!("Jumped to {}s", time)).await?;
        } else {
            ctx.say("Nothing is playing.").await?;
        }
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }
    Ok(())
}

/// Remove a specific song from the queue.
#[poise::command(prefix_command)]
pub async fn remove(
    ctx: Context<'_>,
    #[description = "Queue item index to remove"] index: usize,
) -> Result<(), Error> {
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        player.get_queue().remove(index)?;
        ctx.say("Removed successfully.").await?;
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }
    Ok(())
}

/// Clear the current queue.
#[poise::command(prefix_command)]
pub async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        player.get_queue().clear()?;
        ctx.say("Queue cleared successfully.").await?;
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }
    Ok(())
}

/// Set the volume of the current player.
#[poise::command(prefix_command)]
pub async fn set_volume(ctx: Context<'_>, volume: u16) -> Result<(), Error> {
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        match player.set_volume(volume).await {
            Ok(n) => {
                ctx.say(format!("Set volume to: {}", n.volume)).await?;
            }
            Err(err) => {
                ctx.say(format!("Error: {}", err)).await?;
            }
        }
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }
    Ok(())
}

/// Display the current queue.
#[poise::command(prefix_command)]
pub async fn queue(ctx: Context<'_>, n: usize) -> Result<(), Error> {
    ctx.defer().await?;
    if let Some(player) = get_player_context_from_ctx(&ctx).await {
        let queue = player.get_queue().get_queue().await?;
        let mut fields: Vec<(String, String, bool)> = Vec::new();
        for (i, v) in queue.iter().take(n).enumerate() {
            let track_info = &v.track.info;
            let title = format!("{}. {} - {}", i + 1, track_info.author, track_info.title);
            let value = if let Some(uri) = &track_info.uri {
                format!("[Link to track]({})", uri)
            } else {
                "No URL available".to_string()
            };
            fields.push((title, value, false));
        }
        let embed = CreateEmbed::new()
            .title("Current Queue")
            .color(DARK_BLUE)
            .fields(fields);
        let builder = CreateMessage::new().tts(false).embed(embed);
        if let Err(e) = ctx.channel_id().send_message(&ctx.http(), builder).await {
            println!("Error sending queue message: {}", e);
        }
    } else {
        ctx.say("Join the bot to a voice channel first.").await?;
    }

    Ok(())
}

/// lavalink の GuildId への変換ヘルパー
fn lavalink_guild_id(guild_id: serenity::GuildId) -> lavalink_rs::model::GuildId {
    lavalink_rs::model::GuildId::from(u64::from(guild_id))
}
