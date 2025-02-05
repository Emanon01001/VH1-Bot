use poise::serenity_prelude as serenity;
use poise::serenity_prelude::colours::roles::DARK_BLUE;
use poise::serenity_prelude::CreateEmbed;
use poise::serenity_prelude::CreateMessage;
use std::time::Duration;

use crate::Context;
use crate::Error;

/// Skip the current song.
#[poise::command(slash_command)]
pub async fn skip(ctx: Context<'_>, number: Option<usize>) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    let now_playing = player.get_player().await?.track;

    if let Some(np) = now_playing {
        match number {
            Some(n) => {
                for _ in 0..n {
                    player.skip()?;
                }
            }
            None => {
                player.skip()?;
                ctx.say(format!("Skipped {}", np.info.title)).await?;
            }
        }
    } else {
        ctx.say("Nothing to skip").await?;
    }

    Ok(())
}

/// Pause the current song.
#[poise::command(slash_command)]
pub async fn pause(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    player.set_pause(true).await?;

    ctx.say("Paused").await?;

    Ok(())
}

/// Resume playing the current song.
#[poise::command(slash_command)]
pub async fn resume(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    player.set_pause(false).await?;

    ctx.say("Resumed playback").await?;

    Ok(())
}

/// Stops the playback of the current song.
#[poise::command(slash_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    let now_playing = player.get_player().await?.track;

    if let Some(np) = now_playing {
        player.stop_now().await?;
        ctx.say(format!("Stopped {}", np.info.title)).await?;
    } else {
        ctx.say("Nothing to stop").await?;
    }

    Ok(())
}

/// Jump to a specific time in the song, in seconds.
#[poise::command(slash_command)]
pub async fn seek(
    ctx: Context<'_>,
    #[description = "Time to jump to (in seconds)"] time: u64,
) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    let now_playing = player.get_player().await?.track;

    if now_playing.is_some() {
        player.set_position(Duration::from_secs(time)).await?;
        ctx.say(format!("Jumped to {}s", time)).await?;
    } else {
        ctx.say("Nothing is playing").await?;
    }

    Ok(())
}

/// Remove a specific song from the queue.
#[poise::command(slash_command)]
pub async fn remove(
    ctx: Context<'_>,
    #[description = "Queue item index to remove"] index: usize,
) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    player.get_queue().remove(index)?;

    ctx.say("Removed successfully").await?;

    Ok(())
}

/// Clear the current queue.
#[poise::command(slash_command)]
pub async fn clear(ctx: Context<'_>) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    player.get_queue().clear()?;

    ctx.say("Queue cleared successfully").await?;

    Ok(())
}

#[poise::command(slash_command)]
pub async fn set_volume(ctx: Context<'_>, volume: u16) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    match player.set_volume(volume).await {
        Ok(n) => {
            ctx.say(format!("Set Volume: {}", n.volume)).await?;
        }
        Err(err) => {
            ctx.say(format!("Err: {}", err)).await?;
        }
    }

    Ok(())
}

#[poise::command(slash_command)]
pub async fn queue(ctx: Context<'_>, n: usize) -> Result<(), Error> {
    let guild_id = lavalink_guild_id(ctx.guild_id().unwrap());

    let lava_client = ctx.data().lavalink.clone();

    let Some(player) = lava_client.get_player_context(guild_id) else {
        ctx.say("Join the bot to a voice channel first.").await?;
        return Ok(());
    };

    let queue = player.get_queue().get_queue().await?;

    let mut fields: Vec<(String, String, bool)> = Vec::default();

    for (i, v) in queue.iter().take(n).enumerate() {
        let track_info = &v.track.info;

        // 曲の詳細をEmbedのフィールドとして追加
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
        println!("{}", e)
    }

    Ok(())
}

fn lavalink_guild_id(guild_id: serenity::GuildId) -> lavalink_rs::model::GuildId {
    lavalink_rs::model::GuildId::from(u64::from(guild_id))
}
