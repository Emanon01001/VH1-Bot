use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
};
use tokio::{sync::oneshot, task::JoinHandle};

use axum::{
    body::Body,
    extract::Path as AxumPath,
    http::{Request, StatusCode, Uri},
    response::Response,
    routing::get,
    Router,
};
use tower_http::services::ServeDir;

/// サーバ停止用のハンドル
pub struct LocalHttpServerHandle {
    pub shutdown_tx: Option<oneshot::Sender<()>>,
    pub join_handle: JoinHandle<()>,
}

/// セキュリティ対策: 公開を許可する拡張子のリスト
const ALLOWED_EXTENSIONS: &[&str] = &["mp3", "m4a", "flac", "wav"];

/// ディレクトリトラバーサル禁止 + 拡張子チェック
fn sanitize_path(base_dir: &Path, file_name: &str) -> Option<PathBuf> {
    // 1) 相対パスを結合
    let mut full_path = base_dir.join(file_name);

    // 2) 正規化 (canonicalize) は Windows などで失敗する場合があるので注意
    //    ここでは簡易チェックとして `base_dir` をprefixに持つか確認
    //    ただし symbolic link などあると厳密ではない
    if let Ok(canon) = full_path.canonicalize() {
        if !canon.starts_with(base_dir) {
            // ディレクトリトラバーサル
            return None;
        }
        // 拡張子をチェック
        if let Some(ext) = canon.extension() {
            let ext_str = ext.to_string_lossy().to_lowercase();
            if ALLOWED_EXTENSIONS.contains(&ext_str.as_str()) {
                return Some(canon);
            }
        }
    }
    None
}

/// 個別ファイル用ハンドラ
async fn file_handler(
    AxumPath(file_name): AxumPath<String>,
    req: Request<Body>,
    base_dir: &Path,
) -> Result<Response<Body>, StatusCode> {
    // 例: GET /mysong.mp3 → file_name="mysong.mp3"
    // ディレクトリトラバーサル防止 & 拡張子チェック
    if let Some(valid_path) = sanitize_path(base_dir, &file_name) {
        // ServeDirを使わず、ServeFileなどで単一ファイルを返す方法もある
        match tower_http::services::ServeFile::new(valid_path).oneshot(req).await {
            Ok(resp) => Ok(resp),
            Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
        }
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

/// サーバ起動
pub async fn start_local_server(
    bind_addr: SocketAddr,
    base_dir: PathBuf,
) -> LocalHttpServerHandle {
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // ルーティング：GET /file/<filename> を file_handler で返却
    // "list directory" は行わず、指定ファイルのみ許可(要URLに /file/xxx )
    let router = Router::new()
        .route("/file/:filename", get(|path, req| async move {
            file_handler(path, req, &base_dir).await
        }))
        .route("/", get(|| async {
            // ルートにアクセスされた場合の簡易レスポンス
            "Local file server is running. Use /file/<filename> to fetch a file."
        }));

    let server = axum::Server::bind(&bind_addr)
        .serve(router.into_make_service())
        .with_graceful_shutdown(async {
            let _ = shutdown_rx.await;
        });

    let join_handle = tokio::spawn(async move {
        if let Err(e) = server.await {
            eprintln!("Local HTTP Server error: {:?}", e);
        }
    });

    LocalHttpServerHandle {
        shutdown_tx: Some(shutdown_tx),
        join_handle,
    }
}
