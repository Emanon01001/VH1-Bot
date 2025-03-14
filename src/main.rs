#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod sub_command;

use eframe::{egui, App, NativeOptions};
use std::process::Command as SysCommand;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tokio::{runtime::Runtime, sync::oneshot};

// --- ここから先は、従来のBotコードを「run_bot」関数にまとめるイメージ --- //
use lavalink_rs::{model::events, prelude::*};
use once_cell::sync::Lazy;
use poise::serenity_prelude::{
    async_trait, Client, Color, CreateEmbed, CreateMessage, EventHandler, GatewayIntents, Message,
    MessageReference, Ready,
};
use serde::Deserialize;
use songbird::{Config, SerenityInit};
use sub_command::{log_message, translate};

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

#[derive(Deserialize, Debug)]
struct Translations {
    detected_source_language: String,
    text: String,
}

#[derive(Deserialize, Debug)]
struct TranslationResponse {
    translations: Vec<Translations>,
}

/// 翻訳処理の結果を返す型
type TranslationResult = (String, String);

struct Translate;
struct MessageLog;

struct Data {
    lavalink: LavalinkClient,
}

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;

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
        if msg.author.bot {
            return;
        }

        if let Some(guild_id) = msg.guild_id {
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

            if has_translate_ja || has_translate_en {
                let mut description = String::new();
                if has_translate_ja {
                    let result: TranslationResult = translate(&msg.content, "ja").await;
                    description
                        .push_str(&format!("**日本語翻訳**\n`{}`: {}\n", result.0, result.1));
                }
                if has_translate_en {
                    let result: TranslationResult = translate(&msg.content, "en").await;
                    description.push_str(&format!("**英語翻訳**\n`{}`: {}\n", result.0, result.1));
                }

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
        if msg.author.bot {
            return;
        }
        println!("{}: {}", msg.author.name, msg.content);
        if let Err(err) = log_message(&ctx, &msg).await {
            eprintln!("ログ書き込みエラー: {:?}", err);
        }
    }
}

///
/// Botの起動処理をまとめた非同期関数。
/// GUI（egui/eframe）の別スレッド/別ランタイムから呼び出す想定。
///
/// - `pid_holder`: Lavalink のプロセスIDを格納するためのArc<Mutex<Option<u32>>>。
///
async fn run_bot(
    mut shutdown_rx: oneshot::Receiver<()>,
    log_buffer: Arc<Mutex<Vec<String>>>,
    pid_holder: Arc<Mutex<Option<u32>>>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // フレームワークの生成
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
                commands::music::music_advanced::shuffle(),
                commands::music::music_advanced::repeat(),
                commands::test::button_test(),
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

                // Lavalink のイベント設定
                let events = events::Events {
                    raw: Some(commands::music::music_events::raw_event),
                    ready: Some(commands::music::music_events::ready_event),
                    track_start: Some(commands::music::music_events::track_start),
                    track_end: Some(commands::music::music_events::track_end),
                    ..Default::default()
                };

                // Lavalinkノードの設定
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

    // Songbirdの設定
    let songbird_config = Config::default().decode_mode(songbird::driver::DecodeMode::Decode);

    // Discord Clientの作成
    let mut client = Client::builder(
        &GLOBAL_DATA.token.token,
        GatewayIntents::all() | GatewayIntents::GUILD_VOICE_STATES,
    )
    .event_handler(MessageLog)
    .event_handler(Translate)
    .framework(framework)
    .register_songbird_from_config(songbird_config)
    .await
    .expect("Clientの作成に失敗しました");

    // Lavalinkプロセスの起動部分 (java.exe を直接起動)
    let mut lavalink_child = {
        #[cfg(windows)]
        fn spawn_java_hidden() -> std::io::Result<tokio::process::Child> {
            const CREATE_NO_WINDOW: u32 = 0x08000000;

            let mut cmd = tokio::process::Command::new(
                "D:/Programming/Rust/VH1-Bot/src/Lavalink/jdk-17/bin/java.exe",
            );
            cmd.args(["-jar", "Lavalink.jar"])
                .current_dir("D:/Programming/Rust/VH1-Bot/src/Lavalink")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .creation_flags(CREATE_NO_WINDOW);
            cmd.spawn()
        }
        spawn_java_hidden()
    };

    match &mut lavalink_child {
        Ok(child) => {
            // ★ ここで PID を取得して pid_holder にセット
            if let Some(pid) = child.id() {
                let mut holder = pid_holder.lock().unwrap();
                *holder = Some(pid);
                println!("[INFO] Lavalinkプロセスを起動しました。PID={}", pid);
            } else {
                println!("[WARN] LavalinkプロセスのPIDを取得できませんでした。");
            }

            if let Some(stdout) = child.stdout.take() {
                let log_buffer_for_task = Arc::clone(&log_buffer);
                // 非同期タスクでログの読み取り
                tokio::spawn(async move {
                    use tokio::io::{AsyncBufReadExt, BufReader};
                    let mut reader = BufReader::new(stdout);
                    let mut line = String::new();

                    while let Ok(bytes_read) = reader.read_line(&mut line).await {
                        if bytes_read == 0 {
                            // EOF (子プロセス終了など)
                            break;
                        }
                        {
                            let mut buf = log_buffer_for_task.lock().unwrap();
                            buf.push(line.trim_end().to_string());
                            if buf.len() > 1000 {
                                buf.remove(0);
                            }
                        }
                        line.clear();
                    }
                });
            }
        }
        Err(err) => eprintln!("Lavalinkプロセスの起動に失敗しました: {:?}", err),
    }

    // shutdownシグナル待ちとDiscord Clientの起動を並行処理する
    tokio::select! {
        res = client.start() => {
            if let Err(err) = res {
                eprintln!("Client error: {:?}", err);
                return Err(err.into());
            }
        },
        _ = &mut shutdown_rx => {
            println!("停止要求を受信しました。");
            // Discord Clientの停止
            client.shard_manager.shutdown_all().await;

            // Lavalinkプロセスの停止（kill() は await する必要があります）
            if let Ok(child) = &mut lavalink_child {
                if let Err(err) = child.kill().await {
                    eprintln!("Lavalinkプロセスの停止に失敗しました: {:?}", err);
                } else {
                    println!("Lavalinkプロセスに終了要求を送りました。");
                    match child.wait().await {
                        Ok(status) => println!("Lavalinkプロセスが終了しました。終了コード: {:?}", status.code()),
                        Err(err) => eprintln!("Lavalinkプロセスの終了待機に失敗しました: {:?}", err),
                    }
                }
            }
        },
    }

    Ok(())
}

/*  --- ここから下はGUI周りのコード ---
    GUIアプリケーションの状態を保持する構造体
*/
struct MyEguiApp {
    /// Botが現在起動中かどうか
    bot_running: Arc<AtomicBool>,
    /// Tokioランタイム(ボタンクリックで生成)を保持しておくなど
    runtime: Option<Runtime>,

    shutdown_tx: Option<oneshot::Sender<()>>,
    lavalink_logs: Arc<Mutex<Vec<String>>>,
    /// LavalinkプロセスのPIDを保持する変数
    lavalink_pid: Arc<Mutex<Option<u32>>>,
}

impl MyEguiApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        setup_custom_fonts(&cc.egui_ctx);

        Self {
            bot_running: Arc::new(AtomicBool::new(false)),
            runtime: None,
            shutdown_tx: None,
            lavalink_logs: Arc::new(Mutex::new(Vec::new())),
            lavalink_pid: Arc::new(Mutex::new(None)),
        }
    }
}

fn setup_custom_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "meiryo".to_owned(),
        egui::FontData::from_static(include_bytes!("./meiryo.ttc")).into(),
    );
    if let Some(proportional) = fonts.families.get_mut(&egui::FontFamily::Proportional) {
        proportional.insert(0, "meiryo".to_owned());
    }
    if let Some(monospace) = fonts.families.get_mut(&egui::FontFamily::Monospace) {
        monospace.insert(0, "meiryo".to_owned());
    }
    ctx.set_fonts(fonts);
}

impl App for MyEguiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Bot起動制御パネル");

            if !self.bot_running.load(Ordering::SeqCst) {
                // 起動していない場合
                if ui.button("Botを起動").clicked() {
                    let bot_flag = self.bot_running.clone();
                    bot_flag.store(true, Ordering::SeqCst);

                    let (shutdown_tx, shutdown_rx) = oneshot::channel();
                    self.shutdown_tx = Some(shutdown_tx);

                    let rt = Runtime::new().expect("Tokioランタイムの生成に失敗");

                    let lavalink_logs = Arc::clone(&self.lavalink_logs);
                    let pid_holder = Arc::clone(&self.lavalink_pid);
                    rt.spawn(async move {
                        if let Err(e) = run_bot(shutdown_rx, lavalink_logs, pid_holder).await {
                            eprintln!("Bot error: {:?}", e);
                        }
                        bot_flag.store(false, Ordering::SeqCst);
                    });
                    self.runtime = Some(rt);
                }
            } else {
                // 起動中
                ui.label("Botは起動中");
                if ui.button("Botを停止").clicked() {
                    // 1) 普通のシャットダウン処理を送信
                    if let Some(tx) = self.shutdown_tx.take() {
                        let _ = tx.send(());
                    }
                    // 2) Tokioランタイムをシャットダウン
                    if let Some(rt) = self.runtime.take() {
                        rt.shutdown_background();
                    }
                    // 3) 強制終了（taskkill）を試す
                    if let Some(pid) = *self.lavalink_pid.lock().unwrap() {
                        // Windowsのtaskkillコマンドで強制終了
                        // "/PID" とPIDを渡して /Fフラグで強制終了
                        let output = SysCommand::new("taskkill")
                            .args(["/F", "/PID", &pid.to_string()])
                            .output();
                        match output {
                            Ok(o) => {
                                println!(
                                    "[INFO] taskkill output: {}",
                                    String::from_utf8_lossy(&o.stdout)
                                );
                            }
                            Err(err) => {
                                eprintln!("taskkillエラー: {:?}", err);
                            }
                        }
                    }
                    // PIDもリセット
                    *self.lavalink_pid.lock().unwrap() = None;
                    // 状態を停止に
                    self.bot_running.store(false, Ordering::SeqCst);
                }
            }
        });

        // 右側にログパネル
        egui::SidePanel::right("LogPanel")
            .resizable(true)
            .min_width(800.0)
            .max_width(800.0)
            .show(ctx, |ui| {
                ui.heading("Logs");
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let logs = self.lavalink_logs.lock().expect("Mutex lock failed");
                    // 新しいログが下に溜まるので、上から見る場合は逆順に表示
                    for log in logs.iter().rev() {
                        ui.label(log);
                    }
                });
            });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(pid) = *self.lavalink_pid.lock().unwrap() {
            // Windowsのtaskkillコマンドで強制終了
            // "/PID" とPIDを渡して /Fフラグで強制終了
            let output = SysCommand::new("taskkill")
                .args(["/F", "/PID", &pid.to_string()])
                .output();
            match output {
                Ok(o) => {
                    println!(
                        "[INFO] taskkill output: {}",
                        String::from_utf8_lossy(&o.stdout)
                    );
                }
                Err(err) => {
                    eprintln!("taskkillエラー: {:?}", err);
                }
            }
        }
    }
}

///
/// エントリーポイント（コンソールアプリではなくGUIアプリとして起動）
///
fn main() {
    let _ = tracing_subscriber::fmt::try_init();

    let native_options = NativeOptions {
        vsync: true,
        multisampling: 0,
        depth_buffer: 0,
        stencil_buffer: 0,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        renderer: eframe::Renderer::default(),
        run_and_return: false,
        event_loop_builder: None,
        window_builder: None,
        shader_version: None,
        centered: true,
        persist_window: false,
        persistence_path: None,
        dithering: false,
        viewport: egui::ViewportBuilder::default().with_inner_size([1000f32, 500f32]),
    };

    let _ = eframe::run_native(
        "Discord Bot Control",
        native_options,
        Box::new(|cc| Ok(Box::new(MyEguiApp::new(cc)))),
    );
}