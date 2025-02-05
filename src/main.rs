mod commands;
mod sub_command;

use lavalink_rs::{model::events, prelude::*};
use once_cell::sync::Lazy;
use poise::serenity_prelude::{
    async_trait, Client, Color, CreateEmbed, CreateMessage, EventHandler, GatewayIntents, GuildId,
    Message, MessageReference, Ready,
};
use serde::Deserialize;
use songbird::{Config, SerenityInit};
use sub_command::{log_message, translate};
use tokio::process::Command;

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

/// 翻訳処理の結果を返す型（例：検出言語と翻訳テキスト）
type TranslationResult = (String, String);

struct Translate;
struct MessageLog;

struct Data {
    lavalink: LavalinkClient,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

/// 設定ファイルの読み込み（※同期処理となっています。必要に応じて非同期版に変更してください）
static GLOBAL_DATA: Lazy<Database> = Lazy::new(|| {
    let config_content = std::fs::read_to_string("D:/Programming/Rust/VH1-Bot/src/Setting.toml")
        .expect("設定ファイルの読み込みに失敗しました");
    toml::from_str(&config_content).expect("設定ファイルのパースに失敗しました")
});

#[async_trait]
impl EventHandler for Translate {
    async fn ready(&self, _ctx: poise::serenity_prelude::Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }

    async fn message(&self, ctx: poise::serenity_prelude::Context, msg: Message) {
        // Bot自身のメッセージは無視
        if msg.author.bot {
            return;
        }
        // ギルド内のメッセージのみ処理
        if let Some(guild_id) = msg.guild_id {
            // ユーザーが翻訳対象のロールを持っているか確認（両方の場合はまとめて処理）
            let has_translate_ja = msg
                .author
                .has_role(&ctx.http, guild_id, GLOBAL_DATA.id.translate_ja)
                .await
                .unwrap_or(false);
            let has_translate_en = msg
                .author
                .has_role(&ctx.http, guild_id, GLOBAL_DATA.id.translate_en)
                .await
                .unwrap_or(false);

            // 翻訳すべき場合のみ実施
            if has_translate_ja || has_translate_en {
                let mut description = String::new();
                // 日本語翻訳を実行
                if has_translate_ja {
                    let result: TranslationResult = translate(&msg.content, "ja").await;
                    description
                        .push_str(&format!("**日本語翻訳**\n`{}`: {}\n", result.0, result.1));
                }
                // 英語翻訳を実行
                if has_translate_en {
                    let result: TranslationResult = translate(&msg.content, "en").await;
                    description.push_str(&format!("**英語翻訳**\n`{}`: {}\n", result.0, result.1));
                }
                // 翻訳結果がある場合は、まとめて返信
                if !description.is_empty() {
                    let embed = CreateEmbed::new()
                        .title(&msg.author.name)
                        .color(Color::DARK_BLUE)
                        .description(description);
                    let msg_ref = MessageReference::from(&msg);
                    let builder = CreateMessage::new()
                        .add_embed(embed)
                        .reference_message(msg_ref);
                    if let Err(err) = msg.channel_id.send_message(&ctx.http, builder).await {
                        eprintln!("メッセージ送信エラー: {:?}", err);
                    }
                }
            }
        }
    }
}

#[async_trait]
impl EventHandler for MessageLog {
    async fn message(&self, ctx: poise::serenity_prelude::Context, msg: Message) {
        // Bot自身のメッセージは無視
        if msg.author.bot {
            return;
        }
        println!("{}: {}", msg.author.name, msg.content);

        // ファイル書き込みエラーはログ出力のみ行い、パニックを回避
        if let Err(err) = log_message(&ctx, &msg).await {
            eprintln!("ログ書き込みエラー: {:?}", err);
        }
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
                commands::music::music_basic::play(),
                commands::music::music_basic::join(),
                commands::music::music_basic::leave(),
                commands::music::music_advanced::skip(),
                commands::music::music_advanced::pause(),
                commands::music::music_advanced::resume(),
                commands::music::music_advanced::stop(),
                commands::music::music_advanced::seek(),
                commands::music::music_advanced::clear(),
                commands::music::music_advanced::remove(),
                commands::music::music_advanced::set_volume(),
                commands::music::music_advanced::queue(),
            ],
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some("s!".to_string()),
                ..Default::default()
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            Box::pin(async move {
                // グローバルコマンドの登録
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;

                // Lavalink のイベント設定（必要に応じてイベント処理を実装してください）
                let events = events::Events {
                    raw: Some(commands::music::music_events::raw_event),
                    ready: Some(commands::music::music_events::ready_event),
                    track_start: Some(commands::music::music_events::track_start),
                    ..Default::default()
                };

                // Lavalink ノードの設定
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

    // Songbird の設定（必要に応じて設定値を変更してください）
    let songbird_config = Config::default().decode_mode(songbird::driver::DecodeMode::Decode);

    // Discord Client の作成。エラーハンドリングを強化
    let mut client = Client::builder(
        &GLOBAL_DATA.token.token,
        GatewayIntents::all() | GatewayIntents::GUILD_VOICE_STATES,
    )
    .event_handler(Translate)
    .event_handler(MessageLog)
    .framework(framework)
    .register_songbird_from_config(songbird_config)
    .await
    .expect("Clientの作成に失敗しました");

    // Lavalink のプロセス起動（ハードコードされたパス等は環境変数等で管理することを推奨）
    let process = Command::new("pwsh")
        .args([
            "-Command",
            "cd 'D:/Programming/Rust/VH1-Bot/src/Lavalink'; Start-Process -FilePath 'C:/Program Files/Java/jdk-17/bin/java.exe' -ArgumentList '-jar Lavalink_V4_2.2.1.jar'"
        ])
        .spawn();

    match process {
        Ok(_) => println!("Lavalinkプロセスを起動しました。"),
        Err(err) => eprintln!("Lavalinkプロセスの起動に失敗しました: {:?}", err),
    }

    if let Err(why) = client.start().await {
        eprintln!("Client error: {:?}", why);
    }

    Ok(())
}
