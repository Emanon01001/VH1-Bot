extern crate tracing;
use crate::sub_command::translate;

mod commands;
mod sub_command;

use once_cell::sync::Lazy;
use poise::serenity_prelude::{
    async_trait, Client, Color, CreateEmbed, CreateMessage, EventHandler, GatewayIntents, GuildId,
    Message, MessageReference, Ready,
};

use lavalink_rs::{model::events, prelude::*};

use serde::Deserialize;
use songbird::{Config, SerenityInit};

use tokio::{fs::OpenOptions, io::AsyncWriteExt, process::Command};

#[derive(Deserialize, Debug)]
struct Database {
    token: Tokens,
    endpoint: Endpoints,
    id: Id,
}

#[derive(Deserialize, Debug)]
struct Tokens {
    token: String,
    api_key: String,
}

#[derive(Deserialize, Debug)]
struct Endpoints {
    api_endpoint: String,
}

#[derive(Deserialize, Debug)]
struct Id {
    translate_ja: u64,
    translate_en: u64,
}

#[derive(Deserialize)]
struct Translation {
    detected_source_language: String,
    text: String,
}

#[derive(Deserialize)]
struct TranslationResponse {
    translations: Vec<Translation>,
}
struct Translate;

struct MessageLog;

struct Data {
    lavalink: LavalinkClient,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

static GLOBAL_DATA: Lazy<Database> = Lazy::new(|| {
    let config_content =
        std::fs::read_to_string("D:/Programming/Rust/VH1-Bot/src/Setting.toml").unwrap();
    let config: Database = toml::from_str(&config_content).unwrap();
    config
});

#[async_trait]
impl EventHandler for Translate {
    async fn ready(&self, _: poise::serenity_prelude::Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
    async fn message(&self, ctx: poise::serenity_prelude::Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        if let Some(guild_id) = msg.guild_id {
            if msg
                .author
                .has_role(&ctx.http, guild_id, GLOBAL_DATA.id.translate_ja)
                .await
                .unwrap()
            {
                let trans = translate(&msg.content, "ja").await;
                if !msg.author.bot {
                    let embed = CreateEmbed::new()
                        .title(&msg.author.name)
                        .color(Color::DARK_BLUE)
                        .description(format!("`{}`: {}", trans.0, trans.1));

                    let msg_ref = MessageReference::from(&msg);
                    let builder = CreateMessage::new()
                        .add_embed(embed)
                        .reference_message(msg_ref);
                    msg.channel_id
                        .send_message(&ctx.http, builder)
                        .await
                        .unwrap();
                }
            }
            if msg
                .author
                .has_role(&ctx.http, guild_id, GLOBAL_DATA.id.translate_en)
                .await
                .unwrap()
            {
                let trans = translate(&msg.content, "en").await;
                if !msg.author.bot {
                    let embed = CreateEmbed::new()
                        .title(&msg.author.name)
                        .color(Color::DARK_BLUE)
                        .description(format!("`{}`: {}", trans.0, trans.1));

                    let msg_ref = MessageReference::from(&msg);
                    let builder = CreateMessage::new()
                        .add_embed(embed)
                        .reference_message(msg_ref);
                    msg.channel_id
                        .send_message(&ctx.http, builder)
                        .await
                        .unwrap();
                }
            }
        }
    }
}
#[async_trait]
impl EventHandler for MessageLog {
    async fn message(&self, _ctx: poise::serenity_prelude::Context, msg: Message) {
        if msg.author.bot {
            return;
        }
        println!("{}: {}", msg.author.name, msg.content);
        let mut file = OpenOptions::new()
            .append(true)
            .open("log.txt")
            .await
            .unwrap();

        let time = {
            let utc = chrono::Utc::now();
            let duration = chrono::Duration::hours(9);
            utc + duration
        };

        let guild_name = match msg.guild_id {
            Some(guild_id) => guild_id
                .to_partial_guild(&_ctx.http)
                .await
                .map(|guild| guild.name)
                .unwrap_or_else(|_| "Not Guild Name".to_string()),
            None => "Not Guild Name".to_string(),
        };

        file.write_all(
            format!(
                "Time: {} |Guild_Name: {} | GuildId: {} | {}: {}\n",
                time,
                guild_name,
                msg.guild_id.unwrap_or_else(|| GuildId::new(1)),
                msg.author.name,
                msg.content
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    }
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![
                sub_command::ping(),
                sub_command::trans(),
                commands::music_basic::play(),
                commands::music_basic::join(),
                commands::music_basic::leave(),
                commands::music_advanced::skip(),
                commands::music_advanced::pause(),
                commands::music_advanced::resume(),
                commands::music_advanced::stop(),
                commands::music_advanced::seek(),
                commands::music_advanced::clear(),
                commands::music_advanced::remove(),
                commands::music_advanced::set_volume(),
                commands::music_advanced::queue(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("s!".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                let events = events::Events {
                    raw: Some(commands::music_events::raw_event),
                    ready: Some(commands::music_events::ready_event),
                    track_start: Some(commands::music_events::track_start),
                    ..Default::default()
                };

                let node_local = NodeBuilder {
                    hostname: "localhost:2333".to_string(),
                    is_ssl: false,
                    events: events::Events::default(),
                    password: "ncfhewau3a2rncu".to_string(),
                    user_id: lavalink_rs::model::UserId::from(u64::from(
                        ctx.cache.current_user().id,
                    )),
                    session_id: None,
                };

                let client = LavalinkClient::new(
                    events,
                    vec![node_local],
                    NodeDistributionStrategy::round_robin(),
                )
                .await;

                Ok(Data { lavalink: client })
            })
        })
        .build();

    let songbird_config = Config::default().decode_mode(songbird::driver::DecodeMode::Decode);

    let mut client = Client::builder(
        &GLOBAL_DATA.token.token,
        GatewayIntents::all() | GatewayIntents::GUILD_VOICE_STATES,
    )
    .event_handler(Translate)
    .event_handler(MessageLog)
    .framework(framework)
    .register_songbird_from_config(songbird_config)
    .await
    .expect("Error creating client");

    let _ = Command::new("pwsh")
        .args([
        "-Command",
        "cd 'D:/Programming/Rust/VH1-Bot/src/Lavalink'; Start-Process -FilePath 'C:/Program Files/Java/jdk-17/bin/java.exe' -ArgumentList '-jar Lavalink_V4_2.2.1.jar'"
        ])
        .spawn()
        .expect("プロセスの起動に失敗しました");

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }

    Ok(())
}
