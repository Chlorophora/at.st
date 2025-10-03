use actix_cors::Cors;
use actix_web::{error, http, middleware::Logger, web, App, HttpResponse, HttpServer};
use log;
use niwatori::archive_posts::archive_posts_batch;
use niwatori::{configure_app, middleware::Auth};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{env, path::Path}; // Path をインポート

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // .env ファイルを明示的に読み込み、結果をログに出力します。
    match dotenvy::from_path(Path::new(".env")) {
        Ok(_) => {
            // .env ファイルが読み込めた場合、ロギングを初期化してから成功メッセージを表示します。
            env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
            log::info!("✅ Successfully loaded variables from .env file.");
        }
        Err(e) => {
            // .env ファイルが見つからないのはエラーではない場合もあるため、先にロガーを初期化してから警告を出します。
            env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));
            log::warn!(
                "⚠️ Could not load .env file: {}. Relying on system environment variables.",
                e
            );
        }
    }

    // 必須の環境変数をチェックします。設定されていない場合はここでパニックします。
    let database_url =
        env::var("DATABASE_URL").expect("🔥 DATABASE_URL must be set in your .env file.");
    env::var("USER_ID_SALT").expect("🔥 USER_ID_SALT must be set in your .env file.");
    env::var("PERMANENT_HASH_SALT").expect("🔥 PERMANENT_HASH_SALT must be set in your .env file.");

    // データベース接続プールを作成
    let pool = PgPoolOptions::new()
        .max_connections(10) // Increase max connections
        .connect(&database_url)
        .await
        .unwrap_or_else(|err| {
            eprintln!("🔥 Failed to connect to the database.");
            eprintln!("Please check that the database is running and the DATABASE_URL in your .env file is correct.");
            eprintln!("--------------------------------------------------");
            eprintln!("DATABASE_URL: {}", database_url);
            eprintln!("Error: {}", err);
            eprintln!("--------------------------------------------------");
            std::process::exit(1);
        });

    // アーカイブバッチジョブをバックグラウンドで実行
    let pool_for_scheduler = pool.clone();
    tokio::spawn(async move {
        let interval_minutes_str =
            env::var("ARCHIVE_INTERVAL_MINUTES").unwrap_or_else(|_| "60".to_string());
        let interval_minutes: u64 = interval_minutes_str.parse().unwrap_or_else(|_| {
            log::warn!(
                "Invalid ARCHIVE_INTERVAL_MINUTES value '{}'. Defaulting to 60.",
                interval_minutes_str
            );
            60
        });

        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(interval_minutes * 60));
        log::info!(
            "Archiving scheduler started. Running every {} minutes.",
            interval_minutes
        );
        loop {
            interval.tick().await;
            log::info!("Running archive batch job...");
            if let Err(e) = archive_posts_batch(&pool_for_scheduler).await {
                log::error!("Failed to run archive batch job: {}", e);
            }
        }
    });

    let server_address = "0.0.0.0:8000";
    log::info!("Starting HTTP server at http://{}", server_address);

    // `move`クロージャを避けるため、クロージャ内で使用する変数を事前にクローンします。
    // これにより、コンパイラの型推論が正しく機能し、`trusted_proxies`メソッドが見つかるようになります。
    let pool_for_app = pool.clone();

    HttpServer::new(move || { // `move`クロージャを避けるため、クロージャ内で使用する変数を事前にクローンします。
        // JSONペイロードのパースエラー時に、構造化されたJSONエラーレスポンスを返すための設定
        let json_config = web::JsonConfig::default().error_handler(|err, _req| {
            let error_message = format!("Invalid JSON payload: {}", err);
            log::warn!("{}", &error_message);
            let error_response = json!({
                "error": "Json deserialization error",
                "details": err.to_string()
            });
            error::InternalError::from_response(
                err,
                HttpResponse::BadRequest().json(error_response),
            )
            .into()
        });

        // 外部API通信用のHTTPクライアントを作成
        let http_client = reqwest::Client::new();
        // 環境変数からフロントエンドのオリジンを読み込む。設定がなければlocalhostをデフォルト値とする。
        let frontend_origin = env::var("FRONTEND_URL").unwrap_or_else(|_| {
            log::warn!("FRONTEND_URL not set in .env, defaulting to http://localhost:5173");
            "http://localhost:5173".to_string()
        });
        let cors = Cors::default()
            .allowed_origin(&frontend_origin) // 環境変数から読み込んだオリジンを許可
            .allowed_methods(vec!["GET", "POST", "DELETE", "PUT", "PATCH"])
            .allowed_headers(vec![
                http::header::AUTHORIZATION,
                http::header::ACCEPT,
                http::header::CONTENT_TYPE,
            ])
            .supports_credentials() // Cookieを含むリクエストを許可するために必須
            .max_age(3600);

        App::new()
            .app_data(json_config) // カスタムJSONエラーハンドラを登録
            .app_data(web::Data::new(pool_for_app.clone()))
            .app_data(web::Data::new(http_client.clone())) // HTTPクライアントをアプリケーションデータとして登録
            .wrap(Logger::default()) // リクエストロガーを最初に追加
            .wrap(cors)
            .wrap(Auth) // 認証ミドルウェアを登録
            .service(web::scope("/api").configure(configure_app)) // Apply the /api scope here
    })
    .bind(("0.0.0.0", 8000))?
    .run()
    .await
}
