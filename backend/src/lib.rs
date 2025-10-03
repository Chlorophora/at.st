// c:\Users\sahasahu\Desktop\p\niwatori\backend\src\lib.rs
use actix_web::{
    cookie::{time::OffsetDateTime, Cookie},
    delete, get, post, web, HttpRequest, HttpResponse, Responder,
};
use ammonia::clean;
use chrono::{Duration, TimeZone, Utc};
use once_cell::sync::Lazy;
use rand::{distributions::Alphanumeric, Rng};
use regex::Regex;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, QueryBuilder};
use std::{env, net::IpAddr};
use validator::Validate;

pub mod admin;
pub mod archive_posts; // archive_posts.rs をモジュールとして宣言
pub mod auth;
pub mod bans;
pub mod encryption;
pub mod errors;
pub mod identity;
pub mod level_up;
pub mod middleware;
pub mod models;
pub mod rate_limiter;
pub mod user_history;
pub mod users;
pub mod verification; // verification モジュールを pub に

// --- START: Response Anchor Helpers ---
// DBに保存されたテキスト（ammonia::clean済み）内のレスアンカーをリンクに変換する
// `&gt;&gt;{レス番号}` を探す
static RE_RES_ANCHOR_ESCAPED: Lazy<Regex> = Lazy::new(|| Regex::new(r"&gt;&gt;(\d+)").unwrap());

/// DBから取得したサニタイズ済みの本文を、表示用のHTMLに変換する
/// - 改行を<br>に変換
/// - レスアンカーを<a>タグに変換
pub fn linkify_body(sanitized_body: &str) -> String {
    let with_br = sanitized_body.replace('\n', "<br />\n");
    RE_RES_ANCHOR_ESCAPED
        .replace_all(&with_br, |caps: &regex::Captures| {
            format!(
                "<a href=\"#res-{}\" class=\"response-anchor\">&gt;&gt;{}</a>",
                &caps[1], &caps[1]
            )
        })
        .to_string()
}
// --- END: Response Anchor Helpers ---

// --- START: IP Address Helper ---
/// HTTPリクエストからクライアントのIPアドレスを取得し、必要に応じて正規化します。
///
/// 1. `X-Real-IP` ヘッダーを最優先で使用します。
/// 2. `X-Forwarded-For` ヘッダーがあれば、その左端のIPアドレスを使用します。
/// 3. 上記ヘッダーがない場合は、直接の接続元IPアドレスを使用します。
/// 4. 取得したIPアドレスがIPv6の場合、プライバシー保護のために `/64` プレフィックスに切り詰めます。
///
/// # 戻り値
/// `(切り詰め済みIP, 生のIP)` のタプルを返します。
pub fn get_ip_address(req: &HttpRequest) -> (String, String) {
    log::info!("[IP DIAG] --- Start IP Address Acquisition ---");
    let raw_ip_string = req
        .headers()
        .get("X-Real-IP")
        .and_then(|v| v.to_str().ok())
        .map(|ip| {
            log::info!("[IP DIAG] Found 'X-Real-IP': '{}'.", ip);
            ip.to_string()
        })
        .unwrap_or_else(|| {
            log::info!("[IP DIAG] 'X-Real-IP' not found. Checking 'X-Forwarded-For'.");
            let xff_header = req.headers().get("x-forwarded-for").and_then(|v| v.to_str().ok());
            log::info!("[IP DIAG] Raw 'x-forwarded-for' header: {:?}", xff_header);
            xff_header
                .and_then(|s| s.split(',').next()) // Get the leftmost IP
                .map(|s| s.trim().to_string())
                .map(|ip| {
                    log::info!("[IP DIAG] Found leftmost IP from XFF: '{}'.", ip);
                    ip
                })
                .unwrap_or_else(|| {
                    let fallback_ip = req.connection_info().realip_remote_addr().unwrap_or("0.0.0.0").to_string();
                    log::info!("[IP DIAG] XFF is empty or invalid. Falling back to realip_remote_addr: '{}'", fallback_ip);
                    fallback_ip
                })
        });

    // IPv6アドレスを/64プレフィックスに切り詰める
    let truncated_ip = truncate_ipv6_prefix(&raw_ip_string);
    (truncated_ip, raw_ip_string)
}
// --- END: IP Address Helper ---

// models と errors モジュール内の型を pub use して、
// niwatori::Post のようにアクセスできるようにする (任意)
pub use errors::ServiceError;
pub use models::{
    Board, Comment, CreateBoardRequest, CreateCommentRequest, CreatePostRequest, Post,
};
use models::{
    BoardDetailResponse, BoardWithModerationFlag, CommentResponse, CreatorInfoResponse,
    PostDetailResponse, UpdateBoardDetailsRequest,
};
// 過去ログ検索・ソート用のクエリパラメータ構造体
#[derive(serde::Deserialize, Debug)]
pub struct ArchivedPostsQueryParams {
    pub sort: Option<String>,         // 例: "archived_at_desc", "created_at_asc"
    pub q: Option<String>,            // 検索キーワード
    pub search_field: Option<String>, // 検索対象フィールド: "title", "body"
    pub search_type: Option<String>,  // 検索タイプ: "and", "or"
    pub include_author_names: Option<bool>, // 本文検索時に投稿者名を含めるか
    pub board_id: Option<String>,     // 板IDでフィルタリング (スペース区切りで複数指定可)
    pub created_year: Option<i32>,    // 作成年でフィルタリング
    pub created_month: Option<i32>,   // 作成月でフィルタリング
    pub min_responses: Option<i64>,   // 最小レス数でフィルタリング
    pub limit: Option<i64>,           // ページネーション: 取得件数
    pub offset: Option<i64>,          // ページネーション: 開始位置
    pub include_active_threads: Option<bool>, // 現行スレッドを含めるか
    pub show_deleted: Option<bool>,   // 削除済みスレッドを表示するか
}

// 板一覧のページネーション用クエリパラメータ構造体
#[derive(serde::Deserialize)]
pub struct BoardListQueryParams {
    page: Option<i64>,
}

// タイムスタンプ検索用のパスパラメータ
#[derive(serde::Deserialize)]
pub struct TimestampPathInfo {
    timestamp: i64,
}

// タイムスタンプ検索用のクエリパラメータ
#[derive(serde::Deserialize)]
pub struct TimestampQueryInfo {
    board_id: i32,
}

// スレッド一覧のソート用クエリパラメータ構造体
#[derive(serde::Deserialize)]
pub struct PostsQueryParams {
    sort: Option<String>,
}

// パスからIDを抽出するための汎用的な構造体
#[derive(serde::Deserialize)]
pub struct PathInfo {
    id: i32,
}

// 過去ログ一覧でレス数を含めるための専用構造体
#[derive(serde::Serialize, sqlx::FromRow)]
pub struct ArchivedPostItem {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub author_name: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub board_id: Option<i32>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_activity_at: Option<chrono::DateTime<chrono::Utc>>,
    pub total_responses: i64,
    pub board_name: Option<String>,
}

#[get("/hello")]
pub async fn hello() -> impl Responder {
    HttpResponse::Ok().body("Hello from Niwatori API!")
}

#[get("/ping")]
pub async fn ping() -> impl Responder {
    HttpResponse::Ok().body("pong")
}

#[get("")]
pub async fn get_boards(
    pool: web::Data<PgPool>,
    query: web::Query<BoardListQueryParams>,
) -> Result<HttpResponse, ServiceError> {
    const BOARDS_PER_PAGE: i64 = 100;
    let page = query.page.unwrap_or(1).max(1);
    let offset = (page - 1) * BOARDS_PER_PAGE;

    // 過去24時間の活動量を計算
    let total_count: i64 = sqlx::query_scalar!(
        r#"SELECT COUNT(*) as "total!: i64" FROM boards WHERE deleted_at IS NULL"#
    )
    .fetch_one(pool.get_ref())
    .await?;

    let activity_since = Utc::now() - Duration::hours(24);
    let boards = sqlx::query_as!(Board,
        r#"
        SELECT
            b.id, b.name, b.description, b.default_name, b.created_at, b.updated_at, b.deleted_at,
            b.created_by, b.last_activity_at, b.archived_at, b.max_posts, b.auto_archive_enabled,
            b.moderation_type as "moderation_type: _"
        FROM boards b
        LEFT JOIN (
            SELECT board_id, COUNT(*) as activity_count
            FROM (
                SELECT board_id, created_at FROM posts WHERE created_at > $1
                UNION ALL
                SELECT p.board_id, c.created_at FROM comments c JOIN posts p ON c.post_id = p.id WHERE c.created_at > $1
            ) as activity
            GROUP BY board_id
        ) a ON b.id = a.board_id
        WHERE b.deleted_at IS NULL
        ORDER BY COALESCE(a.activity_count, 0) DESC, b.last_activity_at DESC, b.id DESC
        LIMIT $2 OFFSET $3
        "#,
        activity_since,
        BOARDS_PER_PAGE,
        offset
    )
    .fetch_all(pool.get_ref())
    .await?;

    let response = models::PaginatedResponse {
        items: boards,
        total_count,
    };
    Ok(HttpResponse::Ok().json(response))
}

#[get("/{id}")]
pub async fn get_board_by_id(
    pool: web::Data<PgPool>,
    path: web::Path<i32>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>,
) -> Result<HttpResponse, ServiceError> {
    let board_id = path.into_inner();
    let board = sqlx::query_as!(
        Board,
        r#"SELECT id, name, description, default_name, created_at, updated_at, deleted_at, created_by, last_activity_at, archived_at, max_posts, auto_archive_enabled, moderation_type as "moderation_type: _" FROM boards WHERE id = $1 AND deleted_at IS NULL"#,
        board_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("Board not found".to_string()))?;

    #[cfg(debug_assertions)]
    {
        log::info!(
            "[DIAG] --- Analyzing get_board_by_id for board ID: {} ---",
            board_id
        );
        log::info!("[DIAG] Board data from DB: {:?}", board);
        log::info!("[DIAG] Authenticated user from middleware: {:?}", user);
    }

    let is_admin = user
        .as_ref()
        .is_some_and(|u| matches!(u.role, middleware::Role::Admin));

    // この板に対する設定変更権限を計算します。
    // ログインしているユーザーが「管理者」であるか、または「この板の作成者」であるかを判定します。
    let can_moderate = user
        .as_ref()
        .is_some_and(|u| is_admin || board.created_by == Some(u.user_id));
    #[cfg(debug_assertions)]
    log::info!("[DIAG] 'can_moderate' check result: {}", can_moderate);

    let mut creator_info_response = None;

    // モデレーション権限がある場合、
    // フロントエンドに作成者情報を表示するために、その詳細を取得します。
    #[cfg(debug_assertions)]
    log::info!("[DIAG] Checking condition to fetch creator_info (if can_moderate)...");
    if can_moderate {
        #[cfg(debug_assertions)]
        log::info!("[DIAG] Condition MET (can_moderate=true). Attempting to fetch creator_info for creator_id: {:?}", board.created_by);
        if let Some(creator_id) = board.created_by {
            if let Some(creator) =
                sqlx::query!("SELECT email, level FROM users WHERE id = $1", creator_id)
                    .fetch_optional(pool.get_ref())
                    .await?
            {
                let identity_hashes = identity::generate_identity_hashes(
                    &creator.email,
                    "board_creator_ip", // IPの代わりに固定のプレースホルダーを使用
                    &board.id.to_string(), // Device Infoの代わりに板IDを文字列化して使用
                );

                creator_info_response = Some(CreatorInfoResponse {
                    display_user_id: identity_hashes.display_user_id,
                    level: creator.level,
                    level_at_creation: creator.level,
                });
            }
            #[cfg(debug_assertions)]
            log::info!(
                "[DIAG] Fetched creator_info_response: {:?}",
                creator_info_response
            );
        }
    } else {
        #[cfg(debug_assertions)]
        log::info!("[DIAG] Condition NOT MET (can_moderate=false). Skipping creator_info fetch.");
    }

    let board_with_moderation_flag = BoardWithModerationFlag {
        board,
        can_moderate,
    };

    let response = BoardDetailResponse {
        board: board_with_moderation_flag.clone(),
        creator_info: creator_info_response,
    };

    #[cfg(debug_assertions)]
    {
        log::info!(
            "[DIAG] Constructed board_with_moderation_flag: {:?}",
            &board_with_moderation_flag
        );
        log::info!(
            "[DIAG] Final response object before sending: {:?}",
            serde_json::to_string(&response).unwrap_or_default()
        );
        log::info!("[DIAG] --- End of analysis for get_board_by_id ---");
    }

    Ok(HttpResponse::Ok().json(response))
}

#[post("")]
pub async fn create_board(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>, // Require authentication
    http_client: web::Data<reqwest::Client>,
    req: HttpRequest,
    board_data: web::Json<CreateBoardRequest>,
) -> Result<HttpResponse, ServiceError> {
    // 最初にバリデーションを実行
    board_data.validate()?;

    let (truncated_ip, raw_ip) = get_ip_address(&req);
    let is_admin = matches!(user.role, middleware::Role::Admin);

    // 管理者でない場合、予約文字が含まれていないかチェック
    if !is_admin {
        if let Some(name) = &board_data.default_name {
            if name.contains('☕') {
                return Err(ServiceError::Forbidden(
                    "".to_string(),
                ));
            }
        }
    }

    let mut validated_board_data = board_data.into_inner();
    validated_board_data.name = clean(&validated_board_data.name);
    validated_board_data.description = clean(&validated_board_data.description);

    // デフォルト名が指定されていればサニタイズし、なければ「野球民」を設定
    let default_name = validated_board_data
        .default_name
        .filter(|s| !s.trim().is_empty())
        .map(|s| clean(&s).to_owned()) // Sanitize and own
        .unwrap_or_else(|| "野球民".to_string());

    let device_info: &str = {
        log::info!("[DEVICE DIAG] --- Start Device Info Acquisition ---");
        let fingerprint = validated_board_data.fingerprint.as_deref();
        log::info!("[DEVICE DIAG] Fingerprint from payload: {:?}", fingerprint);
        let user_agent = req.headers().get("User-Agent").and_then(|ua| ua.to_str().ok());
        log::info!("[DEVICE DIAG] User-Agent from headers: {:?}", user_agent);
        let final_device_info = fingerprint.or(user_agent).unwrap_or("unknown");
        log::info!(
            "[DEVICE DIAG] Final device_info chosen: '{}'",
            final_device_info
        );
        final_device_info
    };

    // ユーザーIDから永続的な識別子（メールアドレス）を取得
    let user_email = sqlx::query_scalar!("SELECT email FROM users WHERE id = $1", user.user_id)
        .fetch_one(pool.get_ref())
        .await?;

    let identity_hashes = identity::generate_identity_hashes(&user_email, &truncated_ip, device_info);

    // トランザクションを開始
    let mut tx = pool.begin().await?;

    // --- START: IP評価 (トランザクション内) ---
    let mut attempt_id: Option<i32> = None;
    if !is_admin {
        let fingerprint_value: Option<serde_json::Value> = validated_board_data
            .fingerprint
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());
        let verification_input = verification::VerificationInput {
            verification_type: verification::VerificationType::CreateBoard,
            user_id: Some(user.user_id),
            role: Some(user.role),
            ip_address: truncated_ip.clone(),
            raw_ip_address: Some(raw_ip.clone()),
            captcha_token: None,
            fingerprint_data: fingerprint_value,
        };
        let (result, new_attempt_id) =
            verification::perform_verification(&mut tx, http_client.get_ref(), verification_input)
                .await?;
        attempt_id = Some(new_attempt_id);
        if !result.is_success {
            return Err(ServiceError::Forbidden(
                result
                    .rejection_reason
                    .unwrap_or_else(|| "不正なリクエストとしてブロックされました。".to_string()),
            ));
        }
    }
    // --- END: IP評価 ---

    // グローバルBANのみをチェック (board_id はまだ存在しないので None)
    bans::check_if_banned(
        &mut tx,
        None,
        None, // post_id (板作成時には存在しない)
        Some(&identity_hashes.permanent_user_hash),
        Some(&identity_hashes.permanent_ip_hash),
        Some(&identity_hashes.permanent_device_hash),
    )
    .await?;

    // --- START: レート制限チェック ---
    rate_limiter::check_and_track_rate_limits(
        &mut tx,
        user.user_id,
        &identity_hashes.permanent_ip_hash,
        &identity_hashes.permanent_device_hash,
        models::RateLimitActionType::CreateBoard,
    )
    .await?;

    let new_board = sqlx::query_as!(
        Board,
        r#"
        INSERT INTO boards (name, description, default_name, created_by, last_activity_at, verification_attempt_id) VALUES ($1, $2, $3, $4, NOW(), $5)
        RETURNING id, name, description, default_name, created_at, updated_at, NULL as "deleted_at: _", created_by, last_activity_at, NULL as "archived_at: _", max_posts, auto_archive_enabled, moderation_type as "moderation_type: _"
        "#,
        validated_board_data.name,
        validated_board_data.description,
        default_name,
        user.user_id,
        attempt_id // Noneの場合はNULLとして挿入される
    )
    .fetch_one(&mut *tx) // トランザクションを使用
    .await // Futureの結果を待ってからエラー処理を行う
    .map_err(|e| {
        if let sqlx::Error::Database(db_err) = &e {
            // "23505" is the SQLSTATE code for unique_violation
            if db_err.code() == Some(std::borrow::Cow::from("23505")) {
                return ServiceError::BadRequest("その名前の板は既に存在します。".to_string());
            }
        }
        ServiceError::from(e)
    })?;

    // 板作成者のIPとデバイス情報を取得・暗号化して保存
    // BANチェックで取得した情報を再利用
    let encrypted_ip_bytes = encryption::encrypt(&truncated_ip)?; // 切り詰め済みのIPアドレスを暗号化
    let encrypted_device_info_bytes = encryption::encrypt(device_info)?;

    sqlx::query!(
        "INSERT INTO board_identities (board_id, encrypted_ip, encrypted_device_info) VALUES ($1, $2, $3)",
        new_board.id,
        hex::encode(encrypted_ip_bytes),
        hex::encode(encrypted_device_info_bytes)
    )
    .execute(&mut *tx)
    .await?;

    // トランザクションをコミット
    tx.commit().await?;

    // 専ブラとの互換性を考慮し、成功時のステータスコードを 201 Created から 200 OK に変更します。
    // これにより、より多くのクライアントが成功応答を正しく解釈できるようになります。
    Ok(HttpResponse::Ok().json(new_board))
}

#[delete("/{id}")]
pub async fn delete_board_by_id(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    // Authorization check: Only admins can delete boards.
    if !matches!(user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }

    let board_id = path.into_inner();

    let result = sqlx::query!(
        "UPDATE boards SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        board_id
    )
    .execute(pool.get_ref())
    .await
    .map_err(ServiceError::from)?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::NotFound(
            "Board not found or already deleted".to_string(),
        ));
    }
    Ok(HttpResponse::NoContent().finish())
}

#[post("/{id}/restore")]
pub async fn restore_board_by_id(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<PathInfo>,
) -> Result<HttpResponse, ServiceError> {
    // Authorization check: Only admins can restore boards.
    if !matches!(user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }

    let board_id = path.id;

    let restored_board = sqlx::query_as!(
        Board,
        r#"
        UPDATE boards SET deleted_at = NULL, last_activity_at = NOW() WHERE id = $1 AND deleted_at IS NOT NULL
        RETURNING id, name, description, default_name, created_at, updated_at, deleted_at as "deleted_at: _", created_by, last_activity_at, archived_at as "archived_at: _", max_posts, auto_archive_enabled, moderation_type as "moderation_type: _"
        "#,
        board_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(ServiceError::from)?;

    match restored_board {
        Some(board) => Ok(HttpResponse::Ok().json(board)),
        None => Err(ServiceError::NotFound(
            "Board not found or was not deleted".to_string(),
        )),
    }
}

// get_posts_by_board_id のレスポンスにレス数を含めるための専用構造体
#[derive(serde::Serialize)]
struct PostWithCount {
    #[serde(flatten)]
    post: Post,
    response_count: i64,
    momentum: f64,
}

// get_posts_by_board_id で動的クエリの結果をマッピングするための構造体
#[derive(sqlx::FromRow)]
struct PostDetails {
    id: i32,
    title: String,
    body: String,
    author_name: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    board_id: Option<i32>,
    deleted_at: Option<chrono::DateTime<chrono::Utc>>,
    archived_at: Option<chrono::DateTime<chrono::Utc>>,
    last_activity_at: chrono::DateTime<chrono::Utc>,
    display_user_id: Option<String>,
    permanent_user_hash: Option<String>,
    permanent_ip_hash: Option<String>,
    permanent_device_hash: Option<String>,
    user_id: Option<i32>,
    level_at_creation: Option<i32>,
    level: Option<i32>,
    response_count: i64,
    momentum: f64,
}

#[get("/{id}/posts")]
pub async fn get_posts_by_board_id(
    pool: web::Data<PgPool>,
    path: web::Path<PathInfo>,
    query: web::Query<PostsQueryParams>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>, // 認証状態を確認するために追加
) -> Result<HttpResponse, ServiceError> {
    let board_id = path.id;
    log::info!(
        "[API /boards/{{id}}/posts] Request for board_id: {}",
        board_id
    );

    // First, check if the board exists and is not deleted.
    let board_exists = sqlx::query!(
        "SELECT id FROM boards WHERE id = $1 AND deleted_at IS NULL",
        board_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(ServiceError::from)?;

    if board_exists.is_none() {
        log::warn!(
            "[API /boards/{{id}}/posts] Board with id: {} not found or is deleted. Returning 404.",
            board_id
        );
        return Err(ServiceError::NotFound("Board not found".to_string()));
    }

    // --- START: Level System Integration ---
    let threshold = get_level_display_threshold(pool.get_ref()).await?;
    let is_admin = user.is_some_and(|u| matches!(u.role, middleware::Role::Admin));

    // 環境変数から勢いの上限値を取得。なければデフォルト値を使用。
    let momentum_cap: f64 = env::var("MOMENTUM_CAP")
        .unwrap_or_else(|_| "9999999.99".to_string()) // デフォルト値を元のコードの値に設定
        .parse()
        .unwrap_or(9999999.99);

    // クエリパラメータからソート順を決定
    let sort_option = query.sort.as_deref().unwrap_or("momentum_desc");
    let order_by_clause = match sort_option {
        "responses_desc" => "response_count DESC",
        "responses_asc" => "response_count ASC",
        "momentum_asc" => "momentum ASC",
        "last_activity_desc" => "p.last_activity_at DESC",
        "last_activity_asc" => "p.last_activity_at ASC",
        "created_at_desc" => "p.created_at DESC",
        "created_at_asc" => "p.created_at ASC",
        _ => "momentum DESC", // デフォルトは勢い順 (momentum_desc)
    };

    // SQLクエリを動的に構築
    let query_string = format!(
        r#"
        SELECT
            p.id, p.title, p.body, p.author_name, p.created_at, p.updated_at, p.board_id, p.deleted_at, p.archived_at,
            p.last_activity_at, p.display_user_id, p.permanent_user_hash, p.permanent_ip_hash,
            p.permanent_device_hash, p.user_id, p.level_at_creation, u.level,
            (1 + (SELECT COUNT(*) FROM comments c WHERE c.post_id = p.id)) as response_count,
            -- Momentum calculation (responses per day)
            -- To avoid division by zero, if duration is less than a second, treat it as a small number.
            LEAST(
                CAST((1 + (SELECT COUNT(*) FROM comments c WHERE c.post_id = p.id)) AS DOUBLE PRECISION) / GREATEST(EXTRACT(EPOCH FROM (NOW() - p.created_at)) / 86400.0, 0.00001),
                {}
            ) as momentum
        FROM posts p
        LEFT JOIN users u ON p.user_id = u.id
        WHERE p.board_id = $1 AND p.deleted_at IS NULL AND p.archived_at IS NULL
        ORDER BY {}
        "#,
        momentum_cap, order_by_clause
    );

    let posts_with_details: Vec<PostDetails> = sqlx::query_as(&query_string)
        .bind(board_id)
        .fetch_all(pool.get_ref())
        .await?;

    // PostWithCountに変換
    let response_posts: Vec<PostWithCount> = posts_with_details
        .into_iter()
        .map(|p| {
            let (display_level_at_creation, display_current_level, is_current_level_hidden) =
                process_level_visibility(p.level_at_creation, p.level, threshold, is_admin);

            let post = Post {
                id: p.id,
                title: p.title,
                // スレッド一覧ページでは、レスアンカーがスレッド詳細ページへの絶対パスを指すように、
                // linkify_body が生成した相対リンク (`href="#res-..."`) を置換します。
                body: linkify_body(&p.body).replace(
                    "href=\"#res-",
                    // p.id は現在処理中のスレッドのIDです。
                    &format!("href=\"/posts/{}#res-", p.id),
                ),
                author_name: p.author_name,
                created_at: p.created_at,
                updated_at: p.updated_at,
                board_id: p.board_id,
                deleted_at: p.deleted_at,
                user_id: p.user_id,
                archived_at: p.archived_at,
                last_activity_at: p.last_activity_at,
                display_user_id: p.display_user_id,
                permanent_user_hash: p.permanent_user_hash,
                permanent_ip_hash: p.permanent_ip_hash,
                permanent_device_hash: p.permanent_device_hash,
                level_at_creation: display_level_at_creation,
                level: display_current_level,
                is_current_level_hidden,
            };

            PostWithCount {
                post,
                response_count: p.response_count,
                momentum: p.momentum,
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(response_posts))
}

#[get("")]
pub async fn get_posts(
    pool: web::Data<PgPool>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>,
) -> Result<HttpResponse, ServiceError> {
    let threshold = get_level_display_threshold(pool.get_ref()).await?;
    let is_admin = user.is_some_and(|u| matches!(u.role, middleware::Role::Admin));

    let posts_with_levels = sqlx::query!(
        r#"
        SELECT
            p.id, p.title, p.body, p.author_name, p.created_at, p.updated_at, p.board_id, p.deleted_at, p.archived_at,
            p.last_activity_at, p.display_user_id, p.permanent_user_hash, p.permanent_ip_hash,
            p.permanent_device_hash, p.user_id, p.level_at_creation, u.level as "level?"
        FROM posts p
        LEFT JOIN users u ON p.user_id = u.id
        WHERE p.deleted_at IS NULL AND p.archived_at IS NULL
        ORDER BY p.last_activity_at DESC
        "#
    )
    .fetch_all(pool.get_ref()).await?;

    let response_posts: Vec<Post> = posts_with_levels
        .into_iter()
        .map(|p| {
            let (display_level_at_creation, display_current_level, is_current_level_hidden) =
                process_level_visibility(p.level_at_creation, p.level, threshold, is_admin);
            Post {
                id: p.id,
                title: p.title,
                body: p.body,
                author_name: p.author_name,
                created_at: p.created_at,
                updated_at: p.updated_at,
                board_id: p.board_id,
                deleted_at: p.deleted_at,
                user_id: p.user_id,
                archived_at: p.archived_at,
                last_activity_at: p.last_activity_at,
                display_user_id: p.display_user_id,
                permanent_user_hash: p.permanent_user_hash,
                permanent_ip_hash: p.permanent_ip_hash,
                permanent_device_hash: p.permanent_device_hash,
                level_at_creation: display_level_at_creation,
                level: display_current_level,
                is_current_level_hidden,
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(response_posts))
}

/// タイムスタンプと板IDから特定のスレッドを検索します。
/// 専ブラが `bbs.cgi` や `.dat` ファイルにアクセスする際のパフォーマンスを向上させるために使用されます。
#[get("/by-timestamp/{timestamp}")]
pub async fn get_post_by_timestamp(
    pool: web::Data<PgPool>,
    path: web::Path<TimestampPathInfo>,
    query: web::Query<TimestampQueryInfo>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>, // レベル表示のために必要
) -> Result<HttpResponse, ServiceError> {
    let timestamp_sec = path.timestamp;
    let board_id = query.board_id;

    // Unixタイムスタンプ（秒）から、その秒の開始時刻と終了時刻（次の秒の開始時刻）を計算します。
    // `Utc.timestamp_opt` を使用して、非推奨のAPI呼び出しを回避します。
    let start_time_utc = Utc
        .timestamp_opt(timestamp_sec, 0)
        .single()
        .ok_or_else(|| ServiceError::BadRequest("Invalid timestamp format".to_string()))?;
    let end_time_utc = start_time_utc + chrono::Duration::seconds(1);

    // レベル表示の閾値と管理者フラグを取得
    let threshold = get_level_display_threshold(pool.get_ref()).await?;
    let is_admin = user.is_some_and(|u| matches!(u.role, middleware::Role::Admin));

    // データベースからスレッドを検索
    let post_with_level = sqlx::query!(
        r#"
        SELECT
            p.id, p.title, p.body, p.author_name, p.created_at, p.updated_at, p.board_id, p.deleted_at, p.archived_at,
            p.last_activity_at, p.display_user_id, p.permanent_user_hash, p.permanent_ip_hash,
            p.permanent_device_hash, p.user_id, p.level_at_creation, u.level as "level?"
        FROM posts p
        LEFT JOIN users u ON p.user_id = u.id
        WHERE p.board_id = $1
          AND p.created_at >= $2
          AND p.created_at < $3
          AND p.deleted_at IS NULL
        ORDER BY p.created_at ASC -- 念のため、万が一同一秒に複数あっても最初の一つを取る
        LIMIT 1
        "#,
        board_id,
        start_time_utc,
        end_time_utc
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("Thread not found for the given timestamp and board.".to_string()))?;

    // 表示レベルを計算
    let (display_level_at_creation, display_current_level, is_current_level_hidden) =
        process_level_visibility(
            post_with_level.level_at_creation,
            post_with_level.level,
            threshold,
            is_admin,
        );

    // Post構造体に手動でマッピングします。
    // sqlx::query! が返す匿名構造体は `sqlx::Row` トレイトを実装していないため、
    // 汎用的な `From<R: Row>` 実装を直接使用できず、コンパイルエラーが発生していました。
    // ここで直接フィールドをマップすることで、この問題を解決します。
    let post = Post {
        id: post_with_level.id,
        title: post_with_level.title,
        body: linkify_body(&post_with_level.body),
        author_name: post_with_level.author_name,
        created_at: post_with_level.created_at,
        updated_at: post_with_level.updated_at,
        board_id: post_with_level.board_id,
        deleted_at: post_with_level.deleted_at,
        user_id: post_with_level.user_id,
        archived_at: post_with_level.archived_at,
        last_activity_at: post_with_level.last_activity_at,
        display_user_id: post_with_level.display_user_id,
        permanent_user_hash: post_with_level.permanent_user_hash,
        permanent_ip_hash: post_with_level.permanent_ip_hash,
        permanent_device_hash: post_with_level.permanent_device_hash,
        level_at_creation: display_level_at_creation,
        level: display_current_level,
        is_current_level_hidden,
    };

    Ok(HttpResponse::Ok().json(post))
}

#[get("/{id}")]
pub async fn get_post_by_id(
    pool: web::Data<PgPool>,
    path: web::Path<PathInfo>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>,
) -> Result<HttpResponse, ServiceError> {
    let post_id = path.id;
    let threshold = get_level_display_threshold(pool.get_ref()).await?;
    let is_admin = user
        .as_ref()
        .is_some_and(|u| matches!(u.role, middleware::Role::Admin));

    // 投稿情報と、それが属する板の作成者IDを一度に取得する
    let post_details = sqlx::query!(
        r#"
        SELECT
            p.id, p.title, p.body, p.author_name, p.created_at, p.updated_at, p.board_id,
            p.deleted_at, p.archived_at, p.last_activity_at, p.display_user_id,
            p.permanent_user_hash, p.level_at_creation, p.permanent_ip_hash, p.permanent_device_hash,
            p.user_id,
            u.level as "level?",
            b.created_by as "board_creator_id",
            b.name as "board_name",
            b.moderation_type as "moderation_type: models::BoardModerationType"
        FROM posts p
        LEFT JOIN users u ON p.user_id = u.id
        JOIN boards b ON p.board_id = b.id
        WHERE p.id = $1 AND p.deleted_at IS NULL
        "#,
        post_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("Post not found".to_string()))?;

    // モデレーション権限を計算
    let can_moderate = user.as_ref().is_some_and(|u| {
        let is_board_creator = post_details.board_creator_id == Some(u.user_id);
        // βタイプの板では、スレ主もモデレーション権限を持つ
        let is_thread_creator_on_beta_board = post_details.moderation_type
            == models::BoardModerationType::Beta
            && post_details.user_id == Some(u.user_id);

        is_admin || is_board_creator || is_thread_creator_on_beta_board
    });

    // 表示レベルを計算
    let (display_level_at_creation, display_current_level, is_current_level_hidden) =
        process_level_visibility(
            post_details.level_at_creation,
            post_details.level,
            threshold,
            is_admin,
        );

    let post = Post {
        id: post_details.id,
        title: post_details.title,              // タイトルはサニタイズ済み
        body: linkify_body(&post_details.body), // 本文をリンク化
        author_name: post_details.author_name,
        created_at: post_details.created_at,
        updated_at: post_details.updated_at,
        board_id: post_details.board_id,
        deleted_at: post_details.deleted_at,
        user_id: post_details.user_id,
        archived_at: post_details.archived_at,
        last_activity_at: post_details.last_activity_at,
        display_user_id: post_details.display_user_id,
        permanent_user_hash: post_details.permanent_user_hash,
        permanent_ip_hash: post_details.permanent_ip_hash,
        permanent_device_hash: post_details.permanent_device_hash,
        level_at_creation: display_level_at_creation,
        level: display_current_level,
        is_current_level_hidden,
    };

    let response_post = PostDetailResponse {
        post,
        can_moderate,
        // SQLのJOINにより、これらの値は常に存在するため、unwrap()で安全に値を取り出せます。
        board_id: post_details.board_id.unwrap(),
        board_name: post_details.board_name,
    };

    Ok(HttpResponse::Ok().json(response_post))
}

#[post("")]
pub async fn create_post(
    pool: web::Data<PgPool>,
    http_client: web::Data<reqwest::Client>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>, // Require authentication
    post_data: web::Json<CreatePostRequest>,
    req: HttpRequest,
) -> Result<HttpResponse, ServiceError> {
    // 最初にバリデーションを実行
    post_data.validate()?;

    // 管理者でない場合、予約文字が含まれていないかチェック
    if !user
        .as_ref()
        .is_some_and(|u| matches!(u.role, middleware::Role::Admin))
    {
        if let Some(name) = &post_data.author_name {
            if name.contains('☕') {
                return Err(ServiceError::Forbidden(
                    "".to_string(),
                ));
            }
        }
    }
    // --- START: Refactored Authentication & Token Logic ---
    let user_role_opt = user.as_ref().map(|u| u.role);
    let is_admin = user_role_opt == Some(middleware::Role::Admin);
    let threshold = get_level_display_threshold(pool.get_ref()).await?;
    let (user_id, new_session_cookie, final_body) =
        authenticate_poster(pool.get_ref(), user, &post_data.body).await?;
    // --- END: Refactored Authentication & Token Logic ---

    // 認証ヘルパーの後に `into_inner` を呼び出し、所有権を取得します
    let mut validated_post_data = post_data.into_inner();
    // 認証ヘルパーが処理した後の本文で上書きします
    validated_post_data.body = final_body;

    let (truncated_ip, raw_ip) = get_ip_address(&req);

    let board = sqlx::query_as!(
        Board,
        r#"SELECT id, name, description, default_name, created_at, updated_at, deleted_at, created_by, last_activity_at, archived_at, max_posts, auto_archive_enabled, moderation_type as "moderation_type: _" FROM boards WHERE id = $1 AND deleted_at IS NULL"#,
        validated_post_data.board_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("指定された板が見つかりません。".to_string()))?;

    // アーカイブされた板には新規スレッドを作成できない
    if board.archived_at.is_some() {
        return Err(ServiceError::Forbidden(
            "この板はアーカイブされているため、新しいスレッドを作成できません。".to_string(),
        ));
    }

    // --- START: ID生成ロジック ---
    // ユーザーIDから永続的な識別子と現在のレベルを取得
    let user_info = sqlx::query!("SELECT email, level FROM users WHERE id = $1", user_id)
        .fetch_one(pool.get_ref())
        .await?;
    let user_email = user_info.email;
    let level_at_creation = Some(user_info.level);

    let user_identifier = &user_email;
    let device_info: &str = {
        log::info!("[DEVICE DIAG] --- Start Device Info Acquisition ---");
        let fingerprint = validated_post_data.fingerprint.as_deref();
        log::info!("[DEVICE DIAG] Fingerprint from payload: {:?}", fingerprint);
        let user_agent = req.headers().get("User-Agent").and_then(|ua| ua.to_str().ok());
        log::info!("[DEVICE DIAG] User-Agent from headers: {:?}", user_agent);
        let final_device_info = fingerprint.or(user_agent).unwrap_or("unknown");
        log::info!(
            "[DEVICE DIAG] Final device_info chosen: '{}'",
            final_device_info
        );
        final_device_info
    };

    let identity_hashes =
        identity::generate_identity_hashes(user_identifier, &truncated_ip, device_info);
    // --- END: ID生成ロジック ---

    // トランザクションを開始し、すべてのチェックと作成をアトミックに行う
    let mut tx = pool.begin().await?;

    // --- START: IP評価 (トランザクション内) ---
    let mut attempt_id: Option<i32> = None;
    if !is_admin {
        let fingerprint_value: Option<serde_json::Value> = validated_post_data
            .fingerprint
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());
        let verification_input = verification::VerificationInput {
            verification_type: verification::VerificationType::CreatePost,
            user_id: Some(user_id),
            role: user_role_opt,
            ip_address: truncated_ip.clone(),
            raw_ip_address: Some(raw_ip.clone()),
            captcha_token: None,
            fingerprint_data: fingerprint_value,
        };
        let (result, new_attempt_id) =
            verification::perform_verification(&mut tx, http_client.get_ref(), verification_input)
                .await?;
        attempt_id = Some(new_attempt_id);
        if !result.is_success {
            return Err(ServiceError::Forbidden(
                result
                    .rejection_reason
                    .unwrap_or_else(|| "不正なリクエストとしてブロックされました。".to_string()),
            ));
        }
    }
    // --- END: IP評価 ---

    // --- START: BANチェック ---
    bans::check_if_banned(
        &mut tx,
        Some(board.id),
        None, // post_id (スレッド作成時にはまだ存在しない)
        Some(&identity_hashes.permanent_user_hash),
        Some(&identity_hashes.permanent_ip_hash),
        Some(&identity_hashes.permanent_device_hash),
    )
    .await?;

    // --- START: レート制限チェック ---
    rate_limiter::check_and_track_rate_limits(
        &mut tx,
        user_id,
        &identity_hashes.permanent_ip_hash,
        &identity_hashes.permanent_device_hash,
        models::RateLimitActionType::CreatePost,
    )
    .await?;

    // Sanitize body, title, and author_name
    validated_post_data.title = clean(&validated_post_data.title);
    validated_post_data.body = clean(&validated_post_data.body);

    // Prevent users from accidentally posting a raw token
    if is_potentially_exposed_token(&validated_post_data.body) {
        return Err(ServiceError::BadRequest(
            "連携トークンを本文に貼り付ける際は、!token(...) の形式で貼り付けてください。"
                .to_string(),
        ));
    }

    let author_name = validated_post_data
        .author_name
        .filter(|s| !s.trim().is_empty())
        .map(|s| clean(&s).to_owned())
        .unwrap_or_else(|| board.default_name.clone());

    // --- START: Transaction and Identity Encryption ---
    // Encrypt sensitive information before storing
    let encrypted_email = encryption::encrypt(user_identifier)?; // emailは変わらない
    let encrypted_ip = encryption::encrypt(&truncated_ip)?; // 切り詰め済みのIPを暗号化
    let encrypted_device_info = encryption::encrypt(device_info)?;

    let mut new_post = sqlx::query_as!(Post,
        r#"
        INSERT INTO posts (title, body, board_id, author_name, user_id, level_at_creation, last_activity_at, display_user_id, permanent_user_hash, permanent_ip_hash, permanent_device_hash, display_id_user, display_id_ip, display_id_device, verification_attempt_id)
        VALUES ($1, $2, $3, $4, $5, $6, NOW(), $7, $8, $9, $10, $11, $12, $13, $14)
        RETURNING id, title, body, author_name, created_at, updated_at, board_id as "board_id: _",
            NULL as "deleted_at: _", user_id, NULL as "archived_at: _", last_activity_at,
            display_user_id, permanent_user_hash, permanent_ip_hash, permanent_device_hash, level_at_creation,
            level_at_creation as "level: _", NULL as "is_current_level_hidden: _"
        "#,
        validated_post_data.title, // 新しい変数を使用
        validated_post_data.body, // 新しい変数を使用
        validated_post_data.board_id, // 新しい変数を使用
        author_name,
        user_id,
        level_at_creation,
        identity_hashes.display_user_id,
        identity_hashes.permanent_user_hash,
        identity_hashes.permanent_ip_hash,
        identity_hashes.permanent_device_hash,
        identity_hashes.display_id_user_part,
        identity_hashes.display_id_ip_part,
        identity_hashes.display_id_device_part, // 13
        attempt_id // 14
    )
    .fetch_one(&mut *tx)
    .await?;

    // スレッドが作成された板の最終活動日時を更新
    sqlx::query!(
        "UPDATE boards SET last_activity_at = NOW() WHERE id = $1",
        new_post.board_id
    )
    .execute(&mut *tx)
    .await?;

    // Insert encrypted identities into the new table
    sqlx::query!(
        "INSERT INTO post_identities (post_id, encrypted_email, encrypted_ip, encrypted_device_info) VALUES ($1, $2, $3, $4)",
        new_post.id,
        encrypted_email,
        encrypted_ip,
        encrypted_device_info
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    // --- END: Transaction and Identity Encryption ---

    // --- START: レスポンス用のレベル情報フィルタリング ---
    let (display_level_at_creation, display_current_level, is_current_level_hidden) =
        process_level_visibility(
            new_post.level_at_creation,
            new_post.level,
            threshold,
            is_admin,
        );
    new_post.level_at_creation = display_level_at_creation;
    new_post.level = display_current_level;
    new_post.is_current_level_hidden = is_current_level_hidden;
    // --- END: レスポンス用のレベル情報フィルタリング ---

    // レスポンス用に本文を変換
    new_post.body = linkify_body(&new_post.body);

    // 専ブラの互換性を考慮し、成功時のステータスコードを 201 Created から 200 OK に変更します。
    // これにより、より多くのクライアントが成功応答を正しく解釈できるようになります。
    let mut response_builder = HttpResponse::Ok();
    if let Some(cookie) = new_session_cookie {
        response_builder.cookie(cookie);
    }
    Ok(response_builder.json(new_post))
}

#[post("/comments")]
pub async fn create_comment(
    pool: web::Data<PgPool>,
    http_client: web::Data<reqwest::Client>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>,
    req: HttpRequest,
    comment_data: web::Json<CreateCommentRequest>,
) -> Result<HttpResponse, ServiceError> {
    // 最初にバリデーションを実行
    comment_data.validate()?;

    // 管理者でない場合、予約文字が含まれていないかチェック
    if !user
        .as_ref()
        .is_some_and(|u| matches!(u.role, middleware::Role::Admin))
    {
        if let Some(name) = &comment_data.author_name {
            if name.contains('☕') {
                return Err(ServiceError::Forbidden(
                    "".to_string(),
                ));
            }
        }
    }
    // --- START: Refactored Authentication & Token Logic ---
    let user_role_opt = user.as_ref().map(|u| u.role);
    let is_admin = user_role_opt == Some(middleware::Role::Admin);
    let threshold = get_level_display_threshold(pool.get_ref()).await?;
    let (user_id, new_session_cookie, final_body) =
        authenticate_poster(pool.get_ref(), user, &comment_data.body).await?;
    // --- END: Refactored Authentication & Token Logic ---

    // 認証ヘルパーの後に `into_inner` を呼び出し、所有権を取得します
    let mut validated_comment_data = comment_data.into_inner();
    // 認証ヘルパーが処理した後の本文で上書きします
    validated_comment_data.body = final_body;

    let (truncated_ip, raw_ip) = get_ip_address(&req);

    // スレッドの存在と所属する板のID、アーカイブ状態を確認
    let post_info = sqlx::query!(
        "SELECT board_id, archived_at FROM posts WHERE id = $1 AND deleted_at IS NULL",
        validated_comment_data.post_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("指定されたスレッドが見つかりません。".to_string()))?;

    // 既に過去ログ化されている場合は書き込みを拒否
    if post_info.archived_at.is_some() {
        return Err(ServiceError::BadRequest(
            "このスレッドは過去ログ化されており、新規の書き込みはできません。".to_string(),
        ));
    }

    // 板の情報を取得
    let board = sqlx::query_as!(
        Board,
        // moderation_type を追加
        r#"SELECT id, name, description, default_name, created_at, updated_at, deleted_at, created_by, last_activity_at, archived_at, max_posts, auto_archive_enabled, moderation_type as "moderation_type: _" FROM boards WHERE id = $1 AND deleted_at IS NULL"#,
        post_info.board_id,
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("スレッドが属する板が見つかりません。".to_string()))?;

    // 本文をサニタイズ
    validated_comment_data.body = clean(&validated_comment_data.body);

    // Prevent users from accidentally posting a raw token
    if is_potentially_exposed_token(&validated_comment_data.body) {
        return Err(ServiceError::BadRequest(
            "連携トークンを本文に貼り付ける際は、!token(...) の形式で貼り付けてください。"
                .to_string(),
        ));
    }

    // 投稿者名が指定されていなければ、板のデフォルト名を使用
    let author_name = validated_comment_data
        .author_name
        .filter(|s| !s.trim().is_empty())
        .map(|s| clean(&s).to_owned())
        .unwrap_or_else(|| board.default_name.clone());

    // --- START: ID生成ロジック ---
    // ユーザーIDから永続的な識別子（メールアドレス）と現在のレベルを取得
    let user_info = sqlx::query!("SELECT email, level FROM users WHERE id = $1", user_id)
        .fetch_one(pool.get_ref())
        .await?;
    let user_email = user_info.email;
    let level_at_creation = Some(user_info.level);

    let user_identifier = &user_email;
    let device_info: &str = {
        log::info!("[DEVICE DIAG] --- Start Device Info Acquisition ---");
        let fingerprint = validated_comment_data.fingerprint.as_deref();
        log::info!("[DEVICE DIAG] Fingerprint from payload: {:?}", fingerprint);
        let user_agent = req.headers().get("User-Agent").and_then(|ua| ua.to_str().ok());
        log::info!("[DEVICE DIAG] User-Agent from headers: {:?}", user_agent);
        let final_device_info = fingerprint.or(user_agent).unwrap_or("unknown");
        log::info!(
            "[DEVICE DIAG] Final device_info chosen: '{}'",
            final_device_info
        );
        final_device_info
    };

    let identity_hashes =
        identity::generate_identity_hashes(user_identifier, &truncated_ip, device_info);
    // --- END: ID生成ロジック ---

    // トランザクションを開始
    let mut tx = pool.begin().await?;

    // --- START: IP評価 (トランザクション内) ---
    let mut attempt_id: Option<i32> = None;
    if !is_admin {
        let fingerprint_value: Option<serde_json::Value> = validated_comment_data
            .fingerprint
            .as_ref()
            .and_then(|s| serde_json::from_str(s).ok());
        let verification_input = verification::VerificationInput {
            verification_type: verification::VerificationType::CreateComment,
            user_id: Some(user_id),
            role: user_role_opt,
            ip_address: truncated_ip.clone(),
            raw_ip_address: Some(raw_ip.clone()),
            captcha_token: None,
            fingerprint_data: fingerprint_value,
        };
        let (result, new_attempt_id) =
            verification::perform_verification(&mut tx, http_client.get_ref(), verification_input)
                .await?;
        attempt_id = Some(new_attempt_id);
        if !result.is_success {
            return Err(ServiceError::Forbidden(
                result
                    .rejection_reason
                    .unwrap_or_else(|| "不正なリクエストとしてブロックされました。".to_string()),
            ));
        }
    }
    // --- END: IP評価 ---

    // --- START: BANチェック ---
    bans::check_if_banned(
        &mut tx,
        Some(board.id),
        Some(validated_comment_data.post_id), // スレッドBANをチェックするためにpost_idを渡す
        Some(&identity_hashes.permanent_user_hash),
        Some(&identity_hashes.permanent_ip_hash),
        Some(&identity_hashes.permanent_device_hash),
    )
    .await?;

    // --- START: レート制限チェック ---
    rate_limiter::check_and_track_rate_limits(
        &mut tx,
        user_id,
        &identity_hashes.permanent_ip_hash,
        &identity_hashes.permanent_device_hash,
        models::RateLimitActionType::CreateComment,
    )
    .await?;

    // --- START: Identity Encryption ---
    // Encrypt sensitive information before storing
    let encrypted_email = encryption::encrypt(user_identifier)?; // emailは変わらない
    let encrypted_ip = encryption::encrypt(&truncated_ip)?; // 切り詰め済みのIPを暗号化
    let encrypted_device_info = encryption::encrypt(device_info)?;
    // --- END: Identity Encryption ---

    // 2. 現在のコメント数を取得 (スレッド本体は含まない)
    let current_comment_count: i64 = sqlx::query_scalar!(
        "SELECT COUNT(*) FROM comments WHERE post_id = $1",
        validated_comment_data.post_id
    )
    .fetch_one(&mut *tx)
    .await?
    .unwrap_or(0);

    // 3. 新しいコメントを追加した場合に、合計書き込み数が1000に達するかチェック
    // スレッド本体が1書き込み、コメントが999書き込みで合計1000
    if current_comment_count >= 999 {
        return Err(ServiceError::BadRequest(
            "このスレッドは1000レスに達しており、新規の書き込みはできません。".to_string(),
        ));
    }

    // コメントを挿入
    let mut new_comment = sqlx::query_as!(
        Comment,
        r#"
        INSERT INTO comments (body, post_id, author_name, user_id, level_at_creation, display_user_id, permanent_user_hash, permanent_ip_hash, permanent_device_hash, display_id_user, display_id_ip, display_id_device, verification_attempt_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
        RETURNING id, body, post_id, user_id, author_name, created_at, updated_at, display_user_id, permanent_user_hash, permanent_ip_hash, permanent_device_hash, level_at_creation, level_at_creation as "level: _", NULL as "is_current_level_hidden: _", NULL as "post_title?", NULL as "response_number: _"
        "#,
        validated_comment_data.body,
        validated_comment_data.post_id,
        author_name,
        user_id,
        level_at_creation,
        identity_hashes.display_user_id,
        identity_hashes.permanent_user_hash,
        identity_hashes.permanent_ip_hash,
        identity_hashes.permanent_device_hash,
        identity_hashes.display_id_user_part,
        identity_hashes.display_id_ip_part,
        identity_hashes.display_id_device_part, // 12
        attempt_id // 13
    )
    .fetch_one(&mut *tx) // トランザクションを使用
    .await?;

    // Insert encrypted identities into the new table
    sqlx::query!(
        "INSERT INTO comment_identities (comment_id, encrypted_email, encrypted_ip, encrypted_device_info) VALUES ($1, $2, $3, $4)",
        new_comment.id,
        encrypted_email,
        encrypted_ip,
        encrypted_device_info
    )
    .execute(&mut *tx)
    .await?;

    // スレッドの最終活動日時を更新
    // アーカイブ処理はバッチジョブに一任するため、ここでの archived_at 更新ロジックは削除
    sqlx::query!(
        "UPDATE posts SET last_activity_at = NOW() WHERE id = $1",
        validated_comment_data.post_id
    )
    .execute(&mut *tx)
    .await?;

    // コメントが投稿された板の最終活動日時も更新
    sqlx::query!(
        "UPDATE boards SET last_activity_at = NOW() WHERE id = $1",
        board.id
    )
    .execute(&mut *tx)
    .await?;

    // トランザクションをコミット
    tx.commit().await?;

    // コメント数による3分後アーカイブチェック
    // `current_comment_count` は挿入前のコメント数。
    // これが998だった場合、今追加されたのが999番目のコメントであり、
    // スレッド本体(1) + コメント(999) = 1000レスに達したことになる。
    if current_comment_count == 998 {
        let pool_clone = pool.clone(); // `pool` is a web::Data<PgPool>
        let post_id_to_archive = validated_comment_data.post_id;
        tokio::spawn(async move {
            log::info!(
                "Post {} reached comment limit. Scheduling for archival in 3 minutes.",
                post_id_to_archive
            );
            tokio::time::sleep(std::time::Duration::from_secs(180)).await;

            // 3分後に再度スレッドの状態を確認し、まだアーカイブされていなければアーカイブする
            // (バッチジョブなど他の要因で既にアーカイブされている可能性を考慮)
            let is_not_archived: Option<bool> = sqlx::query_scalar!(
                "SELECT archived_at IS NULL FROM posts WHERE id = $1",
                post_id_to_archive
            )
            .fetch_one(pool_clone.get_ref())
            .await
            .ok()
            .flatten();

            if is_not_archived.unwrap_or(false) {
                match sqlx::query!(
                    "UPDATE posts SET archived_at = NOW() WHERE id = $1",
                    post_id_to_archive
                )
                .execute(pool_clone.get_ref())
                .await
                {
                    Ok(_) => log::info!(
                        "Post {} successfully archived after 3 minutes due to comment limit.",
                        post_id_to_archive
                    ),
                    Err(e) => log::error!(
                        "Failed to archive post {} after 3 minutes: {}",
                        post_id_to_archive,
                        e
                    ),
                }
            }
        });
    }

    // --- START: レスポンス用のレベル情報フィルタリング ---
    let (display_level_at_creation, display_current_level, is_current_level_hidden) =
        process_level_visibility(
            new_comment.level_at_creation,
            new_comment.level,
            threshold,
            is_admin,
        );
    new_comment.level_at_creation = display_level_at_creation;
    new_comment.level = display_current_level;
    new_comment.is_current_level_hidden = is_current_level_hidden;
    // --- END: レスポンス用のレベル情報フィルタリング ---

    // レスポンス用に本文を変換
    new_comment.body = linkify_body(&new_comment.body);

    // 専ブラの互換性を考慮し、成功時のステータスコードを 201 Created から 200 OK に変更します。
    // これにより、より多くのクライアントが成功応答を正しく解釈できるようになります。
    let mut response_builder = HttpResponse::Ok();
    if let Some(cookie) = new_session_cookie {
        response_builder.cookie(cookie);
    }
    Ok(response_builder.json(new_comment))
}

#[get("/{id}/comments")]
pub async fn get_comments_by_post_id(
    pool: web::Data<PgPool>,
    path: web::Path<PathInfo>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>,
) -> Result<HttpResponse, ServiceError> {
    let post_id = path.id;
    let threshold = get_level_display_threshold(pool.get_ref()).await?;
    let is_admin = user
        .as_ref()
        .is_some_and(|u| matches!(u.role, middleware::Role::Admin));

    // --- START: 権限判定のために、まず投稿が属する板の作成者IDを取得 ---
    let thread_mod_info = sqlx::query!(
        r#"
        SELECT
            p.user_id as "thread_creator_id",
            b.created_by as "board_creator_id",
            b.moderation_type as "moderation_type: models::BoardModerationType"
        FROM posts p
        JOIN boards b ON p.board_id = b.id
        WHERE p.id = $1 AND p.deleted_at IS NULL
        "#,
        post_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("Post not found".to_string()))?;

    let can_moderate = user.as_ref().is_some_and(|u| {
        let is_board_creator = thread_mod_info.board_creator_id == Some(u.user_id);
        let is_thread_creator_on_beta_board = thread_mod_info.moderation_type
            == models::BoardModerationType::Beta
            && thread_mod_info.thread_creator_id == Some(u.user_id);
        is_admin || is_board_creator || is_thread_creator_on_beta_board
    });

    let comments_with_levels = sqlx::query!(
        r#"
        SELECT
            c.id, c.body, c.post_id, c.user_id, c.author_name, c.created_at, c.updated_at,
            c.display_user_id, c.permanent_user_hash, c.permanent_ip_hash, c.permanent_device_hash, c.level_at_creation,
            u.level as "level?"
        FROM comments c
        LEFT JOIN users u ON c.user_id = u.id
        WHERE c.post_id = $1
        ORDER BY c.created_at ASC
        "#,
        post_id
    )
    .fetch_all(pool.get_ref())
    .await?;

    let response_comments: Vec<CommentResponse> = comments_with_levels
        .into_iter()
        .map(|c| {
            let (display_level_at_creation, display_current_level, is_current_level_hidden) =
                process_level_visibility(c.level_at_creation, c.level, threshold, is_admin);
            let comment = Comment {
                id: c.id,
                body: linkify_body(&c.body),
                post_id: c.post_id,
                user_id: c.user_id,
                author_name: c.author_name,
                created_at: c.created_at,
                updated_at: c.updated_at,
                display_user_id: c.display_user_id,
                permanent_user_hash: c.permanent_user_hash,
                permanent_ip_hash: c.permanent_ip_hash,
                permanent_device_hash: c.permanent_device_hash,
                level_at_creation: display_level_at_creation,
                post_title: None, // このフィールドはここでは不要なためNoneを設定
                response_number: None, // このフィールドはここでは不要なためNoneを設定
                level: display_current_level,
                is_current_level_hidden,
            };

            CommentResponse {
                comment,
                can_moderate,
            }
        })
        .collect();

    Ok(HttpResponse::Ok().json(response_comments))
}

#[get("/archive")]
pub async fn get_archived_posts(
    pool: web::Data<PgPool>,
    query_params: web::Query<ArchivedPostsQueryParams>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>, // 権限チェックのために追加
) -> Result<HttpResponse, ServiceError> {
    // データを取得するためのクエリビルダー
    let mut data_builder: QueryBuilder<Postgres> = QueryBuilder::new("SELECT p.id, p.title, p.body, p.author_name, p.created_at, p.updated_at, p.board_id, p.deleted_at, p.archived_at, p.last_activity_at, (1 + COALESCE(cc.count, 0)) as total_responses, b.name as board_name FROM posts p LEFT JOIN boards b ON p.board_id = b.id LEFT JOIN (SELECT post_id, COUNT(*) as count FROM comments GROUP BY post_id) cc ON p.id = cc.post_id");
    // 総件数を取得するためのクエリビルダー
    let mut count_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("SELECT COUNT(*) FROM posts p");

    let is_admin = user
        .as_ref()
        .is_some_and(|u| matches!(u.role, middleware::Role::Admin));

    // --- WHERE句の動的構築 ---
    let where_clause = if query_params.show_deleted.unwrap_or(false) && is_admin {
        // 削除済みスレッドを検索する場合
        " WHERE p.deleted_at IS NOT NULL".to_string()
    } else {
        // 通常の検索（削除されていないスレッド）
        let mut base_where = " WHERE p.deleted_at IS NULL".to_string();
        if query_params.include_active_threads == Some(false) {
            base_where.push_str(" AND p.archived_at IS NOT NULL");
        }
        base_where
    };
    data_builder.push(where_clause.clone());
    count_builder.push(where_clause);

    // --- 追加の検索条件 ---
    // キーワード検索
    if let Some(q) = &query_params.q {
        if !q.is_empty() {
            // キーワードを空白で分割し、空の文字列を除去
            let keywords: Vec<_> = q.split_whitespace().filter(|s| !s.is_empty()).collect();
            if !keywords.is_empty() {
                let search_type_is_or = query_params.search_type.as_deref() == Some("or");
                let operator = if search_type_is_or { " OR " } else { " AND " };

                // 各ビルダーに条件句の開始を追加
                data_builder.push(" AND (");
                count_builder.push(" AND (");

                for (i, keyword) in keywords.iter().enumerate() {
                    if i > 0 {
                        data_builder.push(operator);
                        count_builder.push(operator);
                    }

                    let search_term = format!("%{}%", keyword.to_lowercase());

                    match query_params.search_field.as_deref().unwrap_or("title") {
                        "title" => {
                            data_builder
                                .push("LOWER(p.title) LIKE ")
                                .push_bind(search_term.clone());
                            count_builder
                                .push("LOWER(p.title) LIKE ")
                                .push_bind(search_term.clone());
                        }
                        "body" => {
                            data_builder.push("(");
                            count_builder.push("(");

                            data_builder
                                .push("LOWER(p.body) LIKE ")
                                .push_bind(search_term.clone());
                            count_builder
                                .push("LOWER(p.body) LIKE ")
                                .push_bind(search_term.clone());

                            data_builder.push(" OR p.id IN (SELECT c.post_id FROM comments c WHERE LOWER(c.body) LIKE ").push_bind(search_term.clone()).push(")");
                            count_builder.push(" OR p.id IN (SELECT c.post_id FROM comments c WHERE LOWER(c.body) LIKE ").push_bind(search_term.clone()).push(")");

                            if query_params.include_author_names.unwrap_or(false) {
                                data_builder
                                    .push(" OR LOWER(p.author_name) LIKE ")
                                    .push_bind(search_term.clone());
                                count_builder
                                    .push(" OR LOWER(p.author_name) LIKE ")
                                    .push_bind(search_term.clone());

                                data_builder.push(" OR p.id IN (SELECT c.post_id FROM comments c WHERE LOWER(c.author_name) LIKE ").push_bind(search_term.clone()).push(")");
                                count_builder.push(" OR p.id IN (SELECT c.post_id FROM comments c WHERE LOWER(c.author_name) LIKE ").push_bind(search_term.clone()).push(")");
                            }

                            data_builder.push(")");
                            count_builder.push(")");
                        }
                        _ => {} // 無効な値の場合は何もしない
                    }
                }
                // 条件句の終了
                data_builder.push(")");
                count_builder.push(")");
            }
        }
    }

    // 板IDでのフィルタリング
    if let Some(board_id_str) = &query_params.board_id {
        // スペースで区切られたID文字列をパースしてi32のベクターに変換
        let board_ids: Vec<i32> = board_id_str
            .split_whitespace()
            .filter_map(|s| s.parse::<i32>().ok())
            .collect();

        if !board_ids.is_empty() {
            // PostgreSQLのARRAY型にバインドするために `= ANY()` を使用
            // これによりSQLインジェクションを防ぎつつ、複数のIDでフィルタリングできる
            data_builder
                .push(" AND p.board_id = ANY(")
                .push_bind(board_ids.clone())
                .push(")");
            count_builder
                .push(" AND p.board_id = ANY(")
                .push_bind(board_ids)
                .push(")");
        }
    }

    // 最小レス数でのフィルタリング
    if let Some(min_responses) = query_params.min_responses {
        // data_builderは最適化されたクエリを使っているため、cc.countでフィルタリング
        data_builder
            .push(" AND (1 + COALESCE(cc.count, 0)) >= ")
            .push_bind(min_responses);
        // count_builderはJOINしていないため、元のサブクエリでフィルタリング（パフォーマンスは落ちるが、結果は正確）
        count_builder
            .push(" AND (1 + (SELECT COUNT(*) FROM comments c WHERE c.post_id = p.id)) >= ")
            .push_bind(min_responses);
    }

    // 作成年月でのフィルタリング
    if let Some(year) = query_params.created_year {
        if let Some(month) = query_params.created_month {
            // 年と月が両方指定された場合
            let start_of_month = Utc
                .with_ymd_and_hms(year, month as u32, 1, 0, 0, 0)
                .single()
                .ok_or_else(|| {
                    ServiceError::BadRequest("無効な年月が指定されました。".to_string())
                })?;
            let end_of_month = if month == 12 {
                Utc.with_ymd_and_hms(year + 1, 1, 1, 0, 0, 0).single()
            } else {
                Utc.with_ymd_and_hms(year, (month + 1) as u32, 1, 0, 0, 0)
                    .single()
            }
            .ok_or_else(|| ServiceError::BadRequest("無効な年月が指定されました。".to_string()))?;

            data_builder
                .push(" AND p.created_at >= ")
                .push_bind(start_of_month);
            count_builder
                .push(" AND p.created_at >= ")
                .push_bind(start_of_month);

            data_builder
                .push(" AND p.created_at < ")
                .push_bind(end_of_month);
            count_builder
                .push(" AND p.created_at < ")
                .push_bind(end_of_month);
        } else {
            // 年のみ指定された場合
            let start_of_year = Utc
                .with_ymd_and_hms(year, 1, 1, 0, 0, 0)
                .single()
                .ok_or_else(|| {
                    ServiceError::BadRequest("無効な年が指定されました。".to_string())
                })?;
            let end_of_year = Utc
                .with_ymd_and_hms(year + 1, 1, 1, 0, 0, 0)
                .single()
                .ok_or_else(|| {
                    ServiceError::BadRequest("無効な年が指定されました。".to_string())
                })?;

            data_builder
                .push(" AND p.created_at >= ")
                .push_bind(start_of_year);
            count_builder
                .push(" AND p.created_at >= ")
                .push_bind(start_of_year);

            data_builder
                .push(" AND p.created_at < ")
                .push_bind(end_of_year);
            count_builder
                .push(" AND p.created_at < ")
                .push_bind(end_of_year);
        }
    }

    // --- START: SQLデバッグ用ログ ---
    // 実際に実行されるSQLクエリと、受け取ったパラメータをログに出力します。
    // これにより、バックエンドが意図通りのクエリを生成しているかを確認できます。
    let data_sql_debug = data_builder.sql();
    log::info!(
        "[ARCHIVE_SEARCH_DEBUG] Received Params: {:?}",
        query_params.0
    );
    log::info!("[ARCHIVE_SEARCH_DEBUG] Data SQL: {}", data_sql_debug);
    // --- END: SQLデバッグ用ログ ---

    // 総件数を取得するクエリ
    let total_count: i64 = count_builder
        .build_query_scalar()
        .fetch_one(pool.get_ref())
        .await?;

    // ソート条件の追加
    let sort_param = query_params.sort.as_deref().unwrap_or("archived_at_desc");
    let (sort_column, sort_order) = match sort_param.rsplit_once('_') {
        Some((col, order)) => (col, order),
        None => (sort_param, "desc"), // "_" がない場合はデフォルトで降順
    };

    let final_sort_order = if sort_order.eq_ignore_ascii_case("asc") {
        "ASC"
    } else {
        "DESC"
    };

    // SQLインジェクションを防ぎつつ、ソート順を組み立てる
    // 第2ソートキーとして last_activity_at を追加し、順序の一貫性を保証する
    let order_by_clause = match sort_column {
        "created_at" => {
            format!("p.created_at {}, p.last_activity_at DESC", final_sort_order)
        }
        "archived_at" => {
            // DESCの場合、NULLS FIRSTで現行スレッドを先頭に。ASCの場合はNULLS LASTで末尾に。
            let nulls_order = if final_sort_order == "DESC" {
                "NULLS FIRST"
            } else {
                "NULLS LAST"
            };
            format!(
                "p.archived_at {} {}, p.last_activity_at DESC",
                final_sort_order, nulls_order
            )
        }
        // デフォルトは新着アーカイブ順
        _ => "p.archived_at DESC NULLS FIRST, p.last_activity_at DESC".to_string(),
    };
    data_builder.push(format!(" ORDER BY {}", order_by_clause));

    // ページネーションの追加
    let limit = query_params.limit.unwrap_or(20); // デフォルトは20件
    let offset = query_params.offset.unwrap_or(0); // デフォルトは0件目から
    data_builder.push(" LIMIT ");
    data_builder.push_bind(limit);
    data_builder.push(" OFFSET ");
    data_builder.push_bind(offset);

    // DBから取得後に本文を変換
    let posts_from_db: Vec<ArchivedPostItem> = data_builder
        .build_query_as()
        .fetch_all(pool.get_ref())
        .await?;
    let posts: Vec<ArchivedPostItem> = posts_from_db
        .into_iter()
        .map(|mut p| {
            p.body = linkify_body(&p.body);
            p
        })
        .collect();

    let response = models::PaginatedResponse {
        items: posts,
        total_count,
    };
    Ok(HttpResponse::Ok().json(response))
}

#[delete("/{id}")]
pub async fn delete_post_by_id(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<PathInfo>,
) -> Result<HttpResponse, ServiceError> {
    // 論理削除に変更
    // Authorization check: Only admins can delete posts.
    if !matches!(user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }

    let post_id = path.id;

    let result = sqlx::query!(
        "UPDATE posts SET deleted_at = NOW() WHERE id = $1 AND deleted_at IS NULL",
        post_id
    )
    .execute(pool.get_ref())
    .await
    .map_err(ServiceError::from)?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::NotFound(
            "Post not found or already deleted".to_string(),
        ));
    }
    Ok(HttpResponse::NoContent().finish())
}

#[post("/{id}/restore")]
pub async fn restore_post_by_id(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<PathInfo>,
) -> Result<HttpResponse, ServiceError> {
    // Authorization check: Only admins can restore posts.
    if !matches!(user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }

    let post_id = path.id;

    let restored_post = sqlx::query_as!(
        Post,
        r#"
        UPDATE posts SET deleted_at = NULL WHERE id = $1 AND deleted_at IS NOT NULL
        RETURNING id, title, body, author_name, created_at, updated_at, board_id as "board_id: _", user_id, deleted_at as "deleted_at: _", archived_at as "archived_at: _", last_activity_at, display_user_id, permanent_user_hash, permanent_ip_hash, permanent_device_hash, level_at_creation, NULL as "level: _", NULL as "is_current_level_hidden: _"
        "#,
        post_id
    )
    .fetch_optional(pool.get_ref())
    .await
    .map_err(ServiceError::from)?;

    match restored_post {
        Some(post) => Ok(HttpResponse::Ok().json(post)),
        None => Err(ServiceError::NotFound(
            "Post not found or was not deleted".to_string(),
        )),
    }
}

// --- START: Admin Identity API ---
#[get("/identity-details")]
async fn get_identity_details(
    pool: web::Data<PgPool>,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>,
    query: web::Query<models::IdentityQuery>,
) -> Result<HttpResponse, ServiceError> {
    // --- 診断用ログ ---
    // このログは、最新のコードが実行されていることを確認するためのものです。
    log::info!(
        "--- EXECUTING get_identity_details (v_final_check) --- Query: {:?}",
        query
    );

    let authenticated_user = user.ok_or(ServiceError::Unauthorized)?;

    // --- START: Detailed Authorization
    // Authorization check: Only admins can access this.
    if !matches!(authenticated_user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }

    let (
        encrypted_email,
        encrypted_ip,
        encrypted_device_info,
        permanent_user_hash,
        permanent_ip_hash,
        permanent_device_hash,
    ) = if let Some(post_id) = query.post_id {
        // Fetch from post_identities and posts
        let identity_data = sqlx::query!(
                "SELECT encrypted_email, encrypted_ip, encrypted_device_info FROM post_identities WHERE post_id = $1",
                post_id
            )
            .fetch_optional(pool.get_ref())
            .await?;

        let post_hashes = sqlx::query!(
                "SELECT permanent_user_hash, permanent_ip_hash, permanent_device_hash FROM posts WHERE id = $1",
                post_id
            )
            .fetch_optional(pool.get_ref())
            .await?;

        // 両方のレコードが見つからない場合のみ、投稿が存在しないと判断する
        if identity_data.is_none() && post_hashes.is_none() {
            return Err(ServiceError::NotFound(
                "DIAGNOSTIC_V4: Post and its identity information not found.".to_string(),
            ));
        }

        (
            identity_data
                .as_ref()
                .and_then(|d| d.encrypted_email.clone()),
            identity_data.as_ref().and_then(|d| d.encrypted_ip.clone()),
            identity_data
                .as_ref()
                .and_then(|d| d.encrypted_device_info.clone()),
            post_hashes
                .as_ref()
                .and_then(|h| h.permanent_user_hash.clone()),
            post_hashes
                .as_ref()
                .and_then(|h| h.permanent_ip_hash.clone()),
            post_hashes
                .as_ref()
                .and_then(|h| h.permanent_device_hash.clone()),
        )
    } else if let Some(comment_id) = query.comment_id {
        // Fetch from comment_identities and comments
        let identity_data = sqlx::query!(
                "SELECT encrypted_email, encrypted_ip, encrypted_device_info FROM comment_identities WHERE comment_id = $1",
                comment_id
            )
            .fetch_optional(pool.get_ref())
            .await?;

        let comment_hashes = sqlx::query!(
                "SELECT permanent_user_hash, permanent_ip_hash, permanent_device_hash FROM comments WHERE id = $1",
                comment_id
            )
            .fetch_optional(pool.get_ref())
            .await?;

        // 両方のレコードが見つからない場合のみ、コメントが存在しないと判断する
        if identity_data.is_none() && comment_hashes.is_none() {
            return Err(ServiceError::NotFound(
                "DIAGNOSTIC_V4: Comment and its identity information not found.".to_string(),
            ));
        }

        (
            identity_data
                .as_ref()
                .and_then(|d| d.encrypted_email.clone()),
            identity_data.as_ref().and_then(|d| d.encrypted_ip.clone()),
            identity_data
                .as_ref()
                .and_then(|d| d.encrypted_device_info.clone()),
            comment_hashes
                .as_ref()
                .and_then(|h| h.permanent_user_hash.clone()),
            comment_hashes
                .as_ref()
                .and_then(|h| h.permanent_ip_hash.clone()),
            comment_hashes
                .as_ref()
                .and_then(|h| h.permanent_device_hash.clone()),
        )
    } else if let Some(user_id) = query.user_id {
        // Case 3: Fetch by user_id directly. Data from `board_identities` is hex-encoded.
        let user_data = sqlx::query!("SELECT email FROM users WHERE id = $1", user_id)
            .fetch_optional(pool.get_ref())
            .await?
            .ok_or_else(|| ServiceError::NotFound("User not found.".to_string()))?;

        // For direct user queries, only email and permanent user hash are available.
        // We will attempt to find the latest IP/Device info from board creations.
        let board_identity_data = sqlx::query!(
            r#"
                SELECT bi.encrypted_ip, bi.encrypted_device_info
                FROM board_identities bi
                JOIN boards b ON bi.board_id = b.id
                WHERE b.created_by = $1
                ORDER BY b.created_at DESC
                LIMIT 1
                "#,
            user_id
        )
        .fetch_optional(pool.get_ref())
        .await?;

        // 1. board_identities から取得した16進数文字列をバイト列にデコード
        let encrypted_ip_bytes = board_identity_data
            .as_ref()
            .and_then(|d| d.encrypted_ip.as_ref())
            .and_then(|hex| hex::decode(hex).ok());
        let encrypted_device_info_bytes = board_identity_data
            .as_ref()
            .and_then(|d| d.encrypted_device_info.as_ref())
            .and_then(|hex| hex::decode(hex).ok());

        // 2. 復号を試み、失敗した場合は空文字列にする
        let ip_address = encrypted_ip_bytes
            .as_deref()
            .and_then(|bytes| encryption::decrypt(bytes).ok())
            .unwrap_or_default();
        let device_info = encrypted_device_info_bytes
            .as_deref()
            .and_then(|bytes| encryption::decrypt(bytes).ok())
            .unwrap_or_default();

        // 3. 復号した情報（または空文字列）を使ってハッシュを生成
        let identity_hashes =
            identity::generate_identity_hashes(&user_data.email, &ip_address, &device_info);

        // 4. レスポンスを作成
        (
            // emailは常に暗号化して返す
            Some(encryption::encrypt(&user_data.email)?),
            // ipとdevice_infoはDBから取得した暗号化済みのバイト列を返す
            encrypted_ip_bytes,
            encrypted_device_info_bytes,
            // ハッシュ値は再計算したものを返す
            Some(identity_hashes.permanent_user_hash),
            Some(identity_hashes.permanent_ip_hash),
            Some(identity_hashes.permanent_device_hash),
        )
    } else {
        return Err(ServiceError::BadRequest(
            "Either post_id, comment_id, or user_id must be provided.".to_string(),
        ));
    };

    // Decrypt the data
    let email = encryption::decrypt(&encrypted_email.unwrap_or_default()).unwrap_or_default();
    let ip_address = encryption::decrypt(&encrypted_ip.unwrap_or_default()).unwrap_or_default();
    let device_info =
        encryption::decrypt(&encrypted_device_info.unwrap_or_default()).unwrap_or_default();

    let details = models::IdentityDetails {
        email,
        ip_address,
        device_info,
        permanent_user_hash,
        permanent_ip_hash,
        permanent_device_hash,
    };

    Ok(HttpResponse::Ok().json(details))
}
// --- END: Admin Identity API ---

/// [管理者用] 板のスレッド数上限を変更します。
#[actix_web::patch("/boards/{id}/max-posts")]
pub async fn update_board_max_posts(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<i32>,
    payload: web::Json<models::UpdateBoardSettingsRequest>,
) -> Result<HttpResponse, ServiceError> {
    // 権限チェック: 管理者でなければアクセス不可
    if !matches!(user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }

    // 入力値のバリデーション
    payload.validate()?;

    let board_id = path.into_inner();
    let new_max_posts = payload.max_posts;

    // データベースを更新し、更新後の板情報を取得
    let updated_board = sqlx::query_as!(
        Board,
        r#"
        UPDATE boards SET max_posts = $1, updated_at = NOW() WHERE id = $2 AND deleted_at IS NULL
        RETURNING id, name, description, default_name, created_at, updated_at, deleted_at as "deleted_at: _",
                  created_by, max_posts, archived_at as "archived_at: _", last_activity_at, auto_archive_enabled,
                  moderation_type as "moderation_type: _"
        "#,
        new_max_posts,
        board_id
    )
    .fetch_optional(pool.get_ref())
    .await?;

    // `fetch_optional` の結果を元に、成功レスポンスまたはNot Foundエラーを返す
    updated_board.map_or_else(
        || {
            Err(ServiceError::NotFound(
                "指定された板が見つかりません。".to_string(),
            ))
        },
        |board| Ok(HttpResponse::Ok().json(board)),
    )
}

/// [管理者/板作成者用] 板のモデレーションタイプ（α/β）を変更します。
#[actix_web::patch("/boards/{id}/moderation-type")]
pub async fn update_board_moderation_type(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<i32>,
    payload: web::Json<models::UpdateBoardModerationTypeRequest>,
) -> Result<HttpResponse, ServiceError> {
    // 入力値のバリデーション
    payload.validate()?;

    let board_id = path.into_inner();

    // --- 権限チェック ---
    // まず、対象の板が存在し、作成者IDを取得する
    let board_creator_id: Option<i32> = sqlx::query_scalar!(
        "SELECT created_by FROM boards WHERE id = $1 AND deleted_at IS NULL",
        board_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("指定された板が見つかりません。".to_string()))?;

    // 管理者か、または板の作成者でなければアクセス不可
    if !matches!(user.role, middleware::Role::Admin) && board_creator_id != Some(user.user_id) {
        return Err(ServiceError::Forbidden(
            "この板の設定を変更する権限がありません。".to_string(),
        ));
    }

    let new_moderation_type = &payload.moderation_type;

    // データベースを更新し、更新後の板情報を取得
    let updated_board = sqlx::query_as!(
        Board,
        r#"
        UPDATE boards SET moderation_type = $1, updated_at = NOW() WHERE id = $2 AND deleted_at IS NULL
        RETURNING id, name, description, default_name, created_at, updated_at, deleted_at as "deleted_at: _", created_by, max_posts, archived_at as "archived_at: _", last_activity_at, auto_archive_enabled, moderation_type as "moderation_type: _"
        "#,
        new_moderation_type as _,
        board_id
    )
    .fetch_optional(pool.get_ref())
    .await?;

    // `fetch_optional` の結果を元に、成功レスポンスまたはNot Foundエラーを返す
    updated_board.map_or_else(
        || {
            Err(ServiceError::NotFound(
                "指定された板が見つかりません。".to_string(),
            ))
        },
        |board| Ok(HttpResponse::Ok().json(board)),
    )
}

/// [管理者/板作成者用] 板の名前、説明、デフォルト名を変更します。
#[actix_web::patch("/{id}/details")]
pub async fn update_board_details(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<i32>,
    payload: web::Json<UpdateBoardDetailsRequest>,
) -> Result<HttpResponse, ServiceError> {
    // 1. バリデーション
    payload.validate()?;

    let board_id = path.into_inner();

    // 2. 権限チェックのために板の情報を取得
    let board = sqlx::query_as!(
        Board,
        r#"SELECT id, name, description, default_name, created_at, updated_at, deleted_at, created_by, last_activity_at, archived_at, max_posts, auto_archive_enabled, moderation_type as "moderation_type: _" FROM boards WHERE id = $1 AND deleted_at IS NULL"#,
        board_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("指定された板が見つかりません。".to_string()))?;

    // 3. 権限判定
    if !matches!(user.role, middleware::Role::Admin) && board.created_by != Some(user.user_id) {
        return Err(ServiceError::Forbidden(
            "この板の設定を変更する権限がありません。".to_string(),
        ));
    }

    // 4. 動的なUPDATEクエリの構築
    let mut query_builder: QueryBuilder<Postgres> = QueryBuilder::new("UPDATE boards SET ");
    let mut separated = false;

    if let Some(name) = &payload.name {
        query_builder.push("name = ").push_bind(clean(name));
        separated = true;
    }

    if let Some(description) = &payload.description {
        if separated {
            query_builder.push(", ");
        }
        query_builder
            .push("description = ")
            .push_bind(clean(description));
        separated = true;
    }

    if let Some(default_name) = &payload.default_name {
        if separated {
            query_builder.push(", ");
        }
        query_builder
            .push("default_name = ")
            .push_bind(clean(default_name));
        separated = true;
    }

    if !separated {
        // 更新するフィールドがない場合は、取得済みの板情報をそのまま返す
        return Ok(HttpResponse::Ok().json(board));
    }

    query_builder
        .push(", updated_at = NOW() WHERE id = ")
        .push_bind(board_id);
    query_builder.push(" RETURNING *");

    let updated_board = query_builder
        .build_query_as::<Board>()
        .fetch_one(pool.get_ref())
        .await?;

    Ok(HttpResponse::Ok().json(updated_board))
}

/// [管理者用] 板をアーカイブします。
#[post("/boards/{id}/archive")]
async fn archive_board(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }
    let board_id = path.into_inner();
    let result = sqlx::query!(
        "UPDATE boards SET archived_at = NOW() WHERE id = $1 AND archived_at IS NULL",
        board_id
    )
    .execute(pool.get_ref())
    .await?;
    if result.rows_affected() == 0 {
        return Err(ServiceError::NotFound(
            "板が見つからないか、既にアーカイブされています。".to_string(),
        ));
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({"message": "板をアーカイブしました。"})))
}

/// [管理者用] 板のアーカイブを解除します。
#[post("/boards/{id}/unarchive")]
async fn unarchive_board(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }
    let board_id = path.into_inner();
    let result = sqlx::query!("UPDATE boards SET archived_at = NULL, last_activity_at = NOW() WHERE id = $1 AND archived_at IS NOT NULL", board_id)
        .execute(pool.get_ref()).await?;
    if result.rows_affected() == 0 {
        return Err(ServiceError::NotFound(
            "板が見つからないか、アーカイブされていません。".to_string(),
        ));
    }
    Ok(HttpResponse::Ok().json(serde_json::json!({"message": "板のアーカイブを解除しました。"})))
}

/// [管理者用] 板の自動アーカイブ設定を切り替えます。
#[post("/boards/{id}/toggle-auto-archive")]
async fn toggle_auto_archive(
    pool: web::Data<PgPool>,
    user: web::ReqData<middleware::AuthenticatedUser>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, middleware::Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }
    let board_id = path.into_inner();
    // auto_archive_enabled の値を反転させる
    let updated_board = sqlx::query_as!(
        Board,
        r#"
        UPDATE boards
        SET auto_archive_enabled = NOT auto_archive_enabled, updated_at = NOW()
        WHERE id = $1
        RETURNING
            id, name, description, default_name, created_at, updated_at,
            deleted_at as "deleted_at: _",
            created_by,
            max_posts,
            archived_at as "archived_at: _",
            moderation_type as "moderation_type: _",
            last_activity_at,
            auto_archive_enabled
        "#,
        board_id
    )
    .fetch_optional(pool.get_ref())
    .await?;

    updated_board.map_or_else(
        || Err(ServiceError::NotFound("板が見つかりません。".to_string())),
        |board| Ok(HttpResponse::Ok().json(board)),
    )
}

/// Fetches the level display threshold from the settings table.
/// If not set, returns a very high number to default to showing all levels.
async fn get_level_display_threshold(pool: &PgPool) -> Result<i32, ServiceError> {
    let threshold_str: Option<String> =
        sqlx::query_scalar!("SELECT value FROM settings WHERE key = 'level_display_threshold'")
            .fetch_optional(pool)
            .await?;

    Ok(threshold_str
        .and_then(|s| s.parse::<i32>().ok())
        .unwrap_or(i32::MAX)) // Default to a very high number if not set or invalid
}

/// 投稿/コメントのレベル情報の可視性を処理し、フロントエンドに渡すための安全な値を生成します。
///
/// # 引数
/// * `raw_level_at_creation` - DBから取得した生の作成時レベル
/// * `raw_current_level` - DBから取得した生の現在レベル
/// * `threshold` - レベル表示の閾値
/// * `is_admin` - リクエスト者が管理者かどうか
///
/// # 戻り値
/// タプル: `(表示用作成時レベル, 表示用現在レベル, 現在レベルが隠されたかどうかのフラグ)`
fn process_level_visibility(
    raw_level_at_creation: Option<i32>,
    raw_current_level: Option<i32>,
    threshold: i32,
    is_admin: bool,
) -> (Option<i32>, Option<i32>, Option<bool>) {
    let display_level_at_creation = raw_level_at_creation.filter(|&l| is_admin || l < threshold);

    let (display_current_level, is_current_level_hidden) = match raw_current_level {
        Some(l) if is_admin || l < threshold => (Some(l), None), // 表示可能
        Some(_) if display_level_at_creation.is_some() => (None, Some(true)), // 閾値以上で隠す (ただし作成時レベルが表示されている場合のみ)
        _ => (None, None), // 元々レベルがない or 作成時レベルも非表示
    };

    (
        display_level_at_creation,
        display_current_level,
        is_current_level_hidden,
    )
}

/// IPv6アドレス文字列を/64プレフィックスに切り詰めます。
/// IPv4アドレスやパースできない文字列はそのまま返します。
fn truncate_ipv6_prefix(ip_str: &str) -> String {
    match ip_str.parse::<IpAddr>() {
        Ok(IpAddr::V6(ipv6)) => {
            let segments = ipv6.segments();
            let truncated_ipv6 = std::net::Ipv6Addr::new(
                segments[0],
                segments[1],
                segments[2],
                segments[3],
                0, 0, 0, 0, // ホスト部を0に
            );
            log::info!("[IP DIAG] Truncated IPv6 '{}' to '{}'", ip_str, truncated_ipv6);
            truncated_ipv6.to_string()
        }
        _ => ip_str.to_string(), // IPv4 or invalid, return as is
    }
}

// ルーティング設定関数
pub fn configure_app(cfg: &mut web::ServiceConfig) {
    cfg.service(hello) // GET /hello
        .service(ping)  // GET /api/ping (dev)
        // auth
        .service(web::scope("/auth")
            // .service(auth::request_otp) // メール認証フローは現在未使用
            .service(auth::preflight_check) // アカウント作成前の事前チェックを追加
            // .service(auth::verify_otp) // メール認証フローは現在未使用
            .service(auth::get_me)
            .service(auth::toggle_rate_limit_exemption)
            .service(auth::create_account) // 新規アカウント作成 (アカウントID)
            .service(auth::login_with_account_id) // アカウントIDでログイン (アカウントID)
            .service(auth::regenerate_linking_token)
        )
        // admin
        // 管理者用APIは /api/admin スコープに配置し、認証ミドルウェアを適用
        .service(web::scope("/admin") // 認証はmain.rsでグローバルに適用済み
            .service(update_board_max_posts) // PATCH /api/admin/boards/{id}/max-posts
            .service(update_board_moderation_type) // PATCH /api/admin/boards/{id}/moderation-type
            .service(archive_board)      // POST /api/admin/boards/{id}/archive
            .service(unarchive_board)    // POST /api/admin/boards/{id}/unarchive
            .service(toggle_auto_archive) // POST /api/admin/boards/{id}/toggle-auto-archive
            .service(bans::get_admin_bans) // 管理者用BAN一覧APIを追加
            .service(admin::verifications::get_failed_verification_history) // GET /api/admin/failed-verifications
            .service(get_identity_details) // /admin/identity-details
            .service(web::scope("/users") // /api/admin/users
                .service(users::get_users)
                .service(users::get_user_by_id)
                .service(users::set_user_level)
                .service(web::scope("/{id}/history") // /api/admin/users/{id}/history
                    .service(admin::history::get_comment_history)
                    .service(admin::history::get_verification_history)
                    .service(admin::history::get_board_history)
                    .service(admin::history::get_post_history)
                    .service(admin::history::get_ban_history)
                    .service(admin::history::get_executed_ban_history)
                )
                .service(users::set_ban_from_level_up)
            )
            .service(web::scope("/settings") // /api/admin/settings
                .service(users::get_level_display_threshold)
                .service(users::set_level_display_threshold)
                .service(users::get_max_user_level)
                .service(users::set_max_user_level)
            )
            .service(web::scope("/rate-limits") // /api/admin/rate-limits
                .service(rate_limiter::create_rate_limit_rule)
                .service(rate_limiter::get_rate_limit_rules)
                .service(rate_limiter::update_rate_limit_rule)
                .service(rate_limiter::delete_rate_limit_rule)
                .service(rate_limiter::toggle_rate_limit_rule)
                .service(rate_limiter::get_active_rate_limit_locks)
                .service(rate_limiter::delete_rate_limit_lock)
            )
        )
        // bans
        // 認証が必要なBAN操作は /api/bans の下に配置
        .service(web::scope("/bans")
            // .wrap(middleware::Auth) // create_banとdelete_banは内部で認証を処理するため、ここでは不要
            .service(bans::create_ban) // POST /api/bans
            .service(bans::delete_ban) // DELETE /api/bans/{id}
        )
        // 自分のBAN一覧を取得するAPI (GET /api/me/bans)
        .service(bans::get_bans)
        // boards
        .service(web::scope("/boards")
            .service(get_boards)            // GET /api/boards
            .service(create_board)          // POST   /api/boards
            .service(get_board_by_id)       // GET /api/boards/{id}
            .service(get_posts_by_board_id) // GET /api/boards/{id}/posts
            .service(delete_board_by_id) // DELETE /api/boards/{id}
            .service(restore_board_by_id)// POST   /api/boards/{id}/restore
            .service(update_board_details) // PATCH  /api/boards/{id}/details
        )
        // posts & comments
        .service(web::scope("/posts") // `/posts` スコープでグループ化
            // --- 認証不要なGETリクエスト ---
            .service(get_posts)                 // GET /api/posts
            .service(create_post)               // POST /api/posts
            .service(get_post_by_id)            // GET /api/posts/{id}
            .service(get_post_by_timestamp)     // GET /api/posts/by-timestamp/{timestamp}
            .service(get_comments_by_post_id)   // GET /api/posts/{id}/comments
            .service(delete_post_by_id)         // DELETE /api/posts/{id}
            .service(restore_post_by_id)        // POST /api/posts/{id}/restore
        )
        // comments (POST) - create_postは/postsスコープに移動済み
        .service(create_comment) // POST /api/comments
        // level-up system (認証が必要)
        .service(web::scope("/level-up")
            .service(level_up::get_status)         // GET  /api/level-up/status
            .service(level_up::level_up_preflight) // POST /api/level-up/preflight
            .service(level_up::level_up_finalize)  // POST /api/level-up/finalize
        )
        // archive
        .service(get_archived_posts)    // GET /api/archive
        // user_history (ユーザー向けID検索、認証必須)
        .service(web::scope("/history")
            .service(user_history::get_history_by_id_parts) // GET /api/history/by-id-parts
        );
}

// Helper function to extract and remove the linking token from the body
// Example token format in post: !token(a1b2c3d4...)
// Returns (Option<token>, cleaned_body)

fn extract_and_remove_linking_token(body: &str) -> (Option<String>, String) {
    // This regex will be compiled once and reused.
    static TOKEN_RE: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"!token\(([a-zA-Z0-9]{32})\)").unwrap());

    if let Some(captures) = TOKEN_RE.captures(body) {
        // captures.get(0) is the full match, get(1) is the first group
        let token = captures.get(1).map(|m| m.as_str().to_string());
        // Replace the found token pattern with an empty string and trim potential whitespace
        let cleaned_body = TOKEN_RE.replace(body, "").into_owned();
        (token, cleaned_body.trim().to_string())
    } else {
        (None, body.to_string())
    }
}

/// Checks if a given string is a standalone 32-character alphanumeric string,
/// which is the format of our raw linking tokens. This is to prevent users
/// from accidentally posting their raw token.
fn is_potentially_exposed_token(body: &str) -> bool {
    // Check if the trimmed body is exactly 32 chars and all are alphanumeric.
    body.trim().len() == 32 && body.trim().chars().all(|c| c.is_ascii_alphanumeric())
}

// --- START: New Authentication Helper Function ---

/// Authenticates a poster using either a device linking token or an existing session cookie.
///
/// # Arguments
/// * `pool` - The database connection pool.
/// * `user` - An optional `AuthenticatedUser` from middleware, present if a session cookie is valid.
/// * `body` - The raw post body, which may contain a `!token(...)`.
///
/// # Returns
/// A `Result` containing a tuple of `(user_id, Option<new_session_cookie>)` on success,
/// or a `ServiceError` on failure.
async fn authenticate_poster(
    pool: &PgPool,
    user: Option<web::ReqData<middleware::AuthenticatedUser>>,
    body: &str,
) -> Result<(i32, Option<Cookie<'static>>, String), ServiceError> {
    let (linking_token_opt, cleaned_body) = extract_and_remove_linking_token(body);

    if let Some(linking_token) = linking_token_opt {
        // Case 1: A linking token was provided. Try to authenticate with it.
        let mut hasher = Sha256::new();
        hasher.update(linking_token.as_bytes());
        let token_hash = hex::encode(hasher.finalize());

        // Atomically find, use, and return the user_id for a valid token.
        let token_user_id = sqlx::query_scalar!(
            "UPDATE device_linking_tokens SET used_at = NOW() WHERE token_hash = $1 AND expires_at > NOW() AND used_at IS NULL RETURNING user_id",
            token_hash
        )
        .fetch_optional(pool)
        .await?;

        if let Some(user_id) = token_user_id {
            // Token is valid, create a new session for this device
            let session_token: String = rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(64)
                .map(char::from)
                .collect();
            // Cookieの有効期限を1000年に設定し、実質的に半永久的なセッションとします。
            let session_expires_at = Utc::now() + chrono::Duration::days(365 * 1000);

            sqlx::query!(
                "INSERT INTO sessions (user_id, session_token, expires_at) VALUES ($1, $2, $3)",
                user_id,
                &session_token,
                session_expires_at
            )
            .execute(pool)
            .await?;

            // --- START: 環境に応じたCookie設定 ---
            let new_session_cookie = Cookie::build("session_token", session_token)
                .path("/")
                .http_only(true) // HttpOnly属性を明示的に設定
                .expires(OffsetDateTime::from_unix_timestamp(session_expires_at.timestamp()).map_err(|_| ServiceError::InternalServerError("Failed to create cookie expiration".to_string()))?)
                // 常に secure(false) を設定し、HTTP接続を許可します。
                // 本番環境でHTTPSを強制する場合は、この部分を app_env の値に応じて
                // secure(true).same_site(SameSite::None) に切り替える必要があります。
                // 一部の専ブラは SameSite 属性を正しく解釈できないため、
                // この属性自体を省略することで、最大限の互換性を確保します。
                .secure(false)
                .finish();
            // デバッグログを追加して、生成されたCookieの内容を確認します。
            log::info!(
                "[Auth Poster] Generated cookie: {:?}",
                new_session_cookie.to_string()
            );

            // --- END: 環境に応じたCookie設定 ---

            // If the body is empty after removing the token, replace it with a success message.
            let final_body = if cleaned_body.is_empty() {
                "認証成功".to_string()
            } else {
                cleaned_body
            };

            Ok((user_id, Some(new_session_cookie), final_body))
        } else {
            Err(ServiceError::BadRequest(
                "無効な連携トークンです。".to_string(),
            ))
        }
    } else if let Some(authenticated_user) = user {
        // Case 2: No token, but an existing session cookie was found.
        Ok((authenticated_user.user_id, None, body.to_string()))
    } else {
        // Case 3: No token and no session. Unauthorized.
        Err(ServiceError::Unauthorized)
    }
}

// --- END: New Authentication Helper Function ---

// --- START: Post From Row Conversion ---
// `sqlx::query!` が返す匿名構造体から `Post` 構造体への変換を容易にするためのヘルパー

impl<R> From<(R, Option<i32>)> for Post
where
    R: sqlx::Row,
    for<'a> &'a str: sqlx::ColumnIndex<R>,
    // 各フィールドの型をジェネリックに指定
    i32: for<'a> sqlx::decode::Decode<'a, R::Database> + sqlx::types::Type<R::Database>,
    Option<i32>: for<'a> sqlx::decode::Decode<'a, R::Database> + sqlx::types::Type<R::Database>,
    String: for<'a> sqlx::decode::Decode<'a, R::Database> + sqlx::types::Type<R::Database>,
    Option<String>: for<'a> sqlx::decode::Decode<'a, R::Database> + sqlx::types::Type<R::Database>,
    chrono::DateTime<Utc>:
        for<'a> sqlx::decode::Decode<'a, R::Database> + sqlx::types::Type<R::Database>,
    Option<chrono::DateTime<Utc>>:
        for<'a> sqlx::decode::Decode<'a, R::Database> + sqlx::types::Type<R::Database>,
{
    fn from((row, display_level): (R, Option<i32>)) -> Self {
        Self {
            id: row.get("id"),
            title: row.get("title"),
            body: linkify_body(&row.get::<String, _>("body")), // 自動でリンク化
            author_name: row.get("author_name"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
            board_id: row.get("board_id"),
            deleted_at: row.get("deleted_at"),
            user_id: row.get("user_id"),
            archived_at: row.get("archived_at"),
            last_activity_at: row.get("last_activity_at"),
            display_user_id: row.get("display_user_id"),
            permanent_user_hash: row.get("permanent_user_hash"),
            permanent_ip_hash: row.get("permanent_ip_hash"),
            permanent_device_hash: row.get("permanent_device_hash"),
            level_at_creation: row.get("level_at_creation"),
            level: display_level, // このFrom実装は現在直接は使われていないが、将来のために残す
            is_current_level_hidden: None, // デフォルトはNone
        }
    }
}
// --- END: Post From Row Conversion ---
