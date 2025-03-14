use poise::serenity_prelude::Color;
use poise::serenity_prelude::CreateEmbed;
use poise::serenity_prelude::Message;
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;

use crate::Context;
use crate::Error;
use crate::TranslationResponse;
use crate::GLOBAL_DATA;

#[poise::command(slash_command, prefix_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let latency = ctx.ping().await;
    ctx.say(format!("{:?}ms", latency.as_millis())).await?;
    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn trans(ctx: Context<'_>, language: String, word: Vec<String>) -> Result<(), Error> {
    let text_to_translate = word.join(" ");
    let translate_language = language;

    let trans = translate(text_to_translate.as_str(), translate_language.as_str()).await;

    if !ctx.author().bot {
        let embed = CreateEmbed::new()
            .title(&ctx.author().name)
            .color(Color::DARK_BLUE)
            .description(format!("`{}`: {}", trans.0, trans.1));

        let reply = poise::CreateReply::default().reply(true).embed(embed);

        ctx.send(reply).await?;
    }

    Ok(())
}

pub async fn translate(text_to_translate: &str, translate_language: &str) -> (String, String) {
    let client = reqwest::Client::new();
    let response = client
        .post(GLOBAL_DATA.endpoint.api_endpoint.as_str())
        .header(
            "Authorization",
            format!("DeepL-Auth-Key {}", GLOBAL_DATA.token.api_key),
        )
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({
            "text": [text_to_translate.replace("\"", "\\\"")],
            "target_lang": translate_language
        }))
        .send()
        .await
        .unwrap()
        .json::<TranslationResponse>()
        .await
        .unwrap();

    let trans = {
        let trans = response.translations[0].text.trim().to_string();
        let language = response.translations[0]
            .detected_source_language
            .trim()
            .to_string();
        (language, trans)
    };
    trans
}

/// ログメッセージを非同期でファイルに出力するヘルパー関数
pub async fn log_message(
    ctx: &poise::serenity_prelude::Context,
    msg: &Message,
) -> Result<(), Error> {
    let mut file = OpenOptions::new().append(true).open("log.txt").await?;
    // 日本標準時（UTC+9）のタイムスタンプを取得
    let time = chrono::Utc::now() + chrono::Duration::hours(9);
    // ギルド名の取得（失敗した場合はデフォルト文字列）
    let guild_name = if let Some(guild_id) = msg.guild_id {
        guild_id
            .to_partial_guild(&ctx.http)
            .await
            .map(|guild| guild.name)
            .unwrap_or_else(|_| "Unknown Guild".to_string())
    } else {
        "Unknown Guild".to_string()
    };
    let log_line = format!(
        "Time: {} | Guild_Name: {} | GuildId: {} | {}: {}\n",
        time,
        guild_name,
        msg.guild_id.map_or("None".to_string(), |id| id.to_string()),
        msg.author.name,
        msg.content
    );
    file.write_all(log_line.as_bytes()).await?;
    Ok(())
}
