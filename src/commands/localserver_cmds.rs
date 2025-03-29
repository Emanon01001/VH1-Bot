// 例: src/commands/localserver_cmds.rs
use crate::{Context, Error, Data};
use crate::localserver::{start_local_server, LocalHttpServerHandle};
use std::{
    net::SocketAddr,
    path::PathBuf,
    sync::Mutex,
};
use poise::serenity_prelude::{CreateEmbed};
use poise::serenity_prelude::colours::roles::DARK_BLUE;

#[poise::command(slash_command, prefix_command)]
pub async fn start_localserver(ctx: Context<'_>) -> Result<(), Error> {
    let data = ctx.data();
    let mut guard = data.local_server.lock().unwrap();
    if guard.is_some() {
        ctx.say("すでにローカルサーバーは起動しています。").await?;
        return Ok(());
    }

    // ポートやディレクトリは適宜変更
    let bind_addr = SocketAddr::from(([127, 0, 0, 1], 8080));
    let base_dir = PathBuf::from("D:/Music");

    let handle = start_local_server(bind_addr, base_dir).await;
    *guard = Some(handle);

    let embed = CreateEmbed::new()
        .title("Local Server Started")
        .color(DARK_BLUE)
        .description("ローカルファイル配信サーバーを起動しました。\n例: http://127.0.0.1:8080/file/hoge.mp3");
    ctx.send(|m| m.embed(|_| embed)).await?;

    Ok(())
}

#[poise::command(slash_command, prefix_command)]
pub async fn stop_localserver(ctx: Context<'_>) -> Result<(), Error> {
    let data = ctx.data();
    let mut guard = data.local_server.lock().unwrap();
    if let Some(mut handle) = guard.take() {
        // シャットダウン
        if let Some(tx) = handle.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Err(e) = handle.join_handle.await {
            eprintln!("サーバ終了時にエラー: {:?}", e);
        }

        let embed = CreateEmbed::new()
            .title("Local Server Stopped")
            .color(DARK_BLUE)
            .description("ローカルサーバーを停止しました。");
        ctx.send(|m| m.embed(|_| embed)).await?;
    } else {
        ctx.say("ローカルサーバーは起動していません。").await?;
    }
    Ok(())
}
