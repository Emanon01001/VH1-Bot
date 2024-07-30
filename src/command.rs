use poise::serenity_prelude::Color;
use poise::serenity_prelude::CreateEmbed;

use crate::Context;
use crate::Error;
use crate::TranslationResponse;
use crate::GLOBAL_DATA;

#[poise::command(prefix_command, slash_command)]
pub async fn ping(ctx: Context<'_>) -> Result<(), Error> {
    let latency = ctx.ping().await;
    ctx.say(format!("{:?}ms", latency.as_millis())).await?;
    Ok(())
}

#[poise::command(prefix_command, slash_command)]
pub async fn trans(ctx: Context<'_>, language: String, word: String) -> Result<(), Error> {
    let text_to_translate = word;
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
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("auth_key", GLOBAL_DATA.token.api_key.as_str()),
            ("text", text_to_translate),
            ("target_lang", translate_language),
        ])
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
