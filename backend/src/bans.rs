use crate::encryption;
use crate::errors::ServiceError;
use crate::middleware::{AuthenticatedUser, Role};
use crate::models::{self, Ban, BanDetails, BanScope, BanType, Board, CreateBanRequest};
use actix_web::{delete, get, post, web, HttpResponse};
use serde::Serialize;
use sqlx::PgPool;
use validator::Validate;

#[derive(sqlx::FromRow)]
struct TargetHashes {
    board_id: Option<i32>,
    post_id: Option<i32>, // スレッドBANのために投稿IDも取得
    permanent_user_hash: Option<String>,
    permanent_ip_hash: Option<String>,
    permanent_device_hash: Option<String>,
}

#[derive(Serialize)]
pub struct PaginatedBansResponse {
    #[serde(rename = "items")]
    // フロントエンドの他のAPIと合わせるため、JSONでは "items" というキーで出力
    bans: Vec<BanDetails>,
    total_count: i64,
}

/// BAN削除時の権限チェッククエリの結果を保持する一時的な構造体
#[derive(sqlx::FromRow)]
struct BanPermissionInfo {
    ban_creator_id: i32,
    // BANが板に紐づいている場合、その板の所有者IDが入る
    board_owner_id: Option<i32>,
}

/// get_admin_bans のための、暗号化された個人情報を含む一時的な構造体
#[derive(sqlx::FromRow)]
struct AdminBanRow {
    id: i32,
    ban_type: BanType,
    hash_value: String,
    board_id: Option<i32>,
    post_id: Option<i32>,
    post_title: Option<String>,
    board_name: Option<String>,
    reason: Option<String>,
    created_by: i32,
    created_by_email: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    source_post_id: Option<i32>,
    source_comment_id: Option<i32>,
    encrypted_source_email: Option<Vec<u8>>,
    encrypted_source_ip: Option<Vec<u8>>,
    encrypted_source_device_info: Option<Vec<u8>>,
}

#[post("")]
pub async fn create_ban(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    ban_data: web::Json<CreateBanRequest>,
) -> Result<HttpResponse, ServiceError> {
    // 1. BAN対象のハッシュ値、発生源ID、板IDを特定する
    let (hash_to_ban, source_post_id, source_comment_id, target_board_id, target_post_id) =
        if let Some(post_id) = ban_data.post_id.filter(|_| ban_data.hash_value.is_none()) {
            // post_idからハッシュ値を取得
            let post = sqlx::query_as!(
            TargetHashes,
            "SELECT id as post_id, board_id, permanent_user_hash, permanent_ip_hash, permanent_device_hash FROM posts WHERE id = $1",
            post_id,
        )
        .fetch_optional(pool.get_ref()).await?
        .ok_or_else(|| ServiceError::NotFound("指定された投稿が見つかりません。".to_string()))?;

            let hash = match ban_data.ban_type {
                BanType::User => post.permanent_user_hash,
                BanType::Ip => post.permanent_ip_hash,
                BanType::Device => post.permanent_device_hash,
            }
            .ok_or_else(|| {
                ServiceError::BadRequest(
                    "この投稿には指定されたBANタイプに必要なハッシュがありません。".to_string(),
                )
            })?;
            (hash, Some(post_id), None, post.board_id, post.post_id)
        } else if let Some(comment_id) = ban_data
            .comment_id
            .filter(|_| ban_data.hash_value.is_none())
        {
            // comment_idからハッシュ値を取得
            let comment = sqlx::query_as!(
            TargetHashes,
            r#"
            SELECT p.board_id, c.post_id, c.permanent_user_hash, c.permanent_ip_hash, c.permanent_device_hash
            FROM comments c
            INNER JOIN posts p ON c.post_id = p.id
            WHERE c.id = $1
            "#, comment_id
        ).fetch_optional(pool.get_ref()).await?
         .ok_or_else(|| ServiceError::NotFound("指定されたコメントが見つかりません。".to_string()))?;

            let hash = match ban_data.ban_type {
                BanType::User => comment.permanent_user_hash,
                BanType::Ip => comment.permanent_ip_hash,
                BanType::Device => comment.permanent_device_hash,
            }
            .ok_or_else(|| {
                ServiceError::BadRequest(
                    "このコメントには指定されたBANタイプに必要なハッシュがありません。".to_string(),
                )
            })?;
            (
                hash,
                None,
                Some(comment_id),
                comment.board_id,
                comment.post_id,
            )
        } else if let Some(hash_value) = &ban_data.hash_value {
            // ハッシュ値を直接指定
            if !matches!(user.role, Role::Admin) {
                return Err(ServiceError::Forbidden(
                    "ハッシュ値を直接指定したBANは管理者のみ実行できます。".to_string(),
                ));
            }
            // ハッシュ直接指定の場合、発生源はない。対象の板/スレはリクエストのboard_id/post_idから取得する
            (
                hash_value.clone(),
                None,
                None,
                ban_data.board_id,
                ban_data.post_id,
            )
        } else {
            return Err(ServiceError::BadRequest(
                "BAN対象 (post_id, comment_id, hash_value) を指定してください。".to_string(),
            ));
        };

    // 2. 権限チェック
    let is_admin = matches!(user.role, Role::Admin);

    // デバイスBANは管理者のみ
    if ban_data.ban_type == BanType::Device && !is_admin {
        return Err(ServiceError::Forbidden(
            "デバイスBANは管理者のみ実行できます。".to_string(),
        ));
    }

    // スコープに応じた権限チェックと、DBに保存するIDを決定
    let (board_id_for_db, post_id_for_db) = match ban_data.scope {
        BanScope::Global => {
            if !is_admin {
                return Err(ServiceError::Forbidden(
                    "グローバルBANは管理者のみ実行できます。".to_string(),
                ));
            }
            (None, None)
        }
        BanScope::Board => {
            // post_id/comment_id由来の板IDか、リクエストで直接指定された板IDを使用
            let board_id = target_board_id.or(ban_data.board_id).ok_or_else(|| {
                ServiceError::BadRequest(
                    "板BANを行うには、対象の投稿/コメント、またはboard_idの直接指定が必要です。"
                        .to_string(),
                )
            })?;
            if !is_admin {
                let is_board_owner: bool = sqlx::query_scalar!(
                    "SELECT EXISTS(SELECT 1 FROM boards WHERE id = $1 AND created_by = $2)",
                    board_id,
                    user.user_id
                )
                .fetch_one(pool.get_ref())
                .await?
                .unwrap_or(false);
                if !is_board_owner {
                    return Err(ServiceError::Forbidden(
                        "この板を管理する権限がありません。".to_string(),
                    ));
                }
            }
            (Some(board_id), None)
        }
        BanScope::Thread => {
            // 発生源のスレIDか、リクエストで直接指定されたスレIDを使用
            let post_id = target_post_id.or(ban_data.post_id).ok_or_else(|| {
                ServiceError::BadRequest("スレッドBANを行うには、対象の投稿/コメント、またはpost_idの直接指定が必要です。".to_string())
            })?;

            // スレッドIDから板IDを取得する。発生源から取得できている場合はそれを使う。
            let board_id = if let Some(b_id) = target_board_id {
                b_id
            } else {
                let board_id_from_post: Option<i32> =
                    sqlx::query_scalar!("SELECT board_id FROM posts WHERE id = $1", post_id)
                        .fetch_optional(pool.get_ref())
                        .await?
                        .flatten();
                board_id_from_post.ok_or_else(|| {
                    ServiceError::NotFound("指定されたスレッドが見つかりません。".to_string())
                })?
            };

            if !is_admin {
                // 板の所有者か、またはβタイプの板のスレ主かチェック
                let board: Option<Board> = sqlx::query_as!(
                    Board,
                    r#"
                    SELECT
                        id, name, description, default_name, created_at, updated_at,
                        deleted_at as "deleted_at: _",
                        created_by,
                        max_posts,
                        archived_at as "archived_at: _",
                        moderation_type as "moderation_type: _",
                        last_activity_at,
                        auto_archive_enabled
                    FROM boards WHERE id = $1
                    "#,
                    board_id
                )
                .fetch_optional(pool.get_ref())
                .await?;
                let board = board.ok_or_else(|| {
                    ServiceError::NotFound("対象の板が見つかりません。".to_string())
                })?;

                if board.created_by == Some(user.user_id) {
                    // 板の所有者なのでOK
                } else if board.moderation_type == models::BoardModerationType::Beta {
                    // βタイプの板の場合、スレ主かチェック
                    let is_thread_creator: bool = sqlx::query_scalar!(
                        "SELECT EXISTS(SELECT 1 FROM posts WHERE id = $1 AND user_id = $2)",
                        post_id,
                        user.user_id
                    )
                    .fetch_one(pool.get_ref())
                    .await?
                    .unwrap_or(false);
                    if !is_thread_creator {
                        return Err(ServiceError::Forbidden(
                            "このスレッドを管理する権限がありません。".to_string(),
                        ));
                    }
                } else {
                    return Err(ServiceError::Forbidden(
                        "このスレッドを管理する権限がありません。".to_string(),
                    ));
                }
            }
            (Some(board_id), Some(post_id))
        }
    };

    // 3. 既存のBANがないかチェック
    let existing_ban: Option<(i32,)> = sqlx::query_as(
        r#"SELECT id FROM bans WHERE ban_type = $1 AND hash_value = $2
           AND (
             (post_id IS NULL AND board_id IS NULL) -- Global
             OR (post_id IS NULL AND board_id = $3) -- Board
             OR (post_id = $4) -- Thread
           )"#,
    )
    .bind(ban_data.ban_type)
    .bind(&hash_to_ban)
    .bind(board_id_for_db)
    .bind(post_id_for_db)
    .fetch_optional(pool.get_ref())
    .await?;

    if existing_ban.is_some() {
        let scope = match ban_data.scope {
            BanScope::Thread => "このスレッド",
            BanScope::Board => "この板",
            BanScope::Global => "グローバル",
        };
        return Err(ServiceError::BadRequest(format!(
            "このユーザー/IP/デバイスは既に{}でBANされています。",
            scope
        )));
    }

    // 4. バリデーションとDBへの挿入
    ban_data.validate()?;

    // 暗号化
    let encrypted_source_email = match &ban_data.source_email {
        Some(email) if !email.is_empty() => Some(encryption::encrypt(email)?),
        _ => None,
    };
    let encrypted_source_ip = match &ban_data.source_ip_address {
        Some(ip) if !ip.is_empty() => Some(encryption::encrypt(ip)?),
        _ => None,
    };
    let encrypted_source_device_info = match &ban_data.source_device_info {
        Some(device) if !device.is_empty() => Some(encryption::encrypt(device)?),
        _ => None,
    };

    let new_ban = sqlx::query_as!(
        Ban,
        r#"
        INSERT INTO bans (ban_type, hash_value, board_id, post_id, reason, created_by, source_post_id, source_comment_id, encrypted_source_email, encrypted_source_ip, encrypted_source_device_info)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        RETURNING id, ban_type as "ban_type: _", hash_value, board_id, post_id, reason, created_by, created_at, expires_at,
                  source_post_id, source_comment_id, encrypted_source_email, encrypted_source_ip, encrypted_source_device_info
        "#,
        ban_data.ban_type as _,
        hash_to_ban,
        board_id_for_db,
        post_id_for_db,
        ban_data.reason,
        user.user_id,
        source_post_id,
        source_comment_id,
        encrypted_source_email,
        encrypted_source_ip,
        encrypted_source_device_info
    )
    .fetch_one(pool.get_ref())
    .await?;

    Ok(HttpResponse::Created().json(new_ban))
}

// 管理者専用: 全てのBAN情報を取得する
#[get("/bans")]
pub async fn get_admin_bans(
    pool: web::Data<PgPool>,
    user: Option<web::ReqData<AuthenticatedUser>>,
    query: web::Query<models::PaginationParams>,
) -> Result<HttpResponse, ServiceError> {
    // ユーザーが認証されているか手動でチェックし、されていなければUnauthorizedエラーを返す
    let authenticated_user = user.ok_or(ServiceError::Unauthorized)?;

    // 管理者でなければアクセス不可
    if !matches!(authenticated_user.role, Role::Admin) {
        return Err(ServiceError::Forbidden(
            "管理者権限が必要です。".to_string(),
        ));
    }

    // BANの総件数を取得
    let total_count: i64 = sqlx::query_scalar!("SELECT count(*) FROM bans")
        .fetch_one(pool.get_ref())
        .await?
        .unwrap_or(0);

    // ページネーションのためのオフセットを計算
    let offset = (query.page - 1) * query.limit;

    // N+1問題を解決するため、1回のクエリでBAN情報と関連情報をJOINして取得
    let ban_rows = sqlx::query_as!(
        AdminBanRow,
        r#"
        SELECT
            b.id,
            b.ban_type as "ban_type: BanType",
            b.hash_value,
            b.board_id,
            b.post_id,
            bo.name as "board_name?",
            p.title as "post_title?",
            b.reason,
            b.created_by,
            u.email as "created_by_email?",
            b.created_at,
            b.expires_at,
            b.source_post_id,
            b.source_comment_id,
            b.encrypted_source_email,
            b.encrypted_source_ip,
            b.encrypted_source_device_info
        FROM bans b
        LEFT JOIN boards bo ON b.board_id = bo.id
        LEFT JOIN posts p ON b.post_id = p.id
        LEFT JOIN users u ON b.created_by = u.id
        ORDER BY b.created_at DESC
        LIMIT $1 OFFSET $2
        "#,
        query.limit,
        offset
    )
    .fetch_all(pool.get_ref())
    .await?;

    // 取得したデータをBanDetailsに変換し、スコープを判定し、個人情報を復号
    let bans: Vec<BanDetails> = ban_rows
        .into_iter()
        .map(|row| {
            let (scope, scope_display_name) = if row.post_id.is_some() {
                ("Thread".to_string(), "スレッド内".to_string())
            } else if row.board_id.is_some() {
                ("Board".to_string(), "板内".to_string())
            } else {
                ("Global".to_string(), "グローバル".to_string())
            };

            BanDetails {
                id: row.id,
                ban_type: row.ban_type,
                hash_value: row.hash_value,
                board_id: row.board_id,
                post_id: row.post_id,
                board_name: row.board_name,
                post_title: row.post_title,
                reason: row.reason,
                created_by: row.created_by,
                created_by_email: row.created_by_email,
                scope,
                scope_display_name,
                created_at: row.created_at,
                expires_at: row.expires_at,
                source_post_id: row.source_post_id,
                source_comment_id: row.source_comment_id,
                source_email: row
                    .encrypted_source_email
                    .as_deref()
                    .and_then(|e| encryption::decrypt(e).ok()),
                source_ip_address: row
                    .encrypted_source_ip
                    .as_deref()
                    .and_then(|e| encryption::decrypt(e).ok()),
                source_device_info: row
                    .encrypted_source_device_info
                    .as_deref()
                    .and_then(|e| encryption::decrypt(e).ok()),
                source_user_id: None, // This field is not populated by AdminBanRow
            }
        })
        .collect();

    let response = PaginatedBansResponse { bans, total_count };

    Ok(HttpResponse::Ok().json(response))
}

// get_bans のための、個人情報を含まない一時的な構造体
#[derive(sqlx::FromRow)]
struct MyBanRow {
    id: i32,
    ban_type: BanType,
    hash_value: String,
    board_id: Option<i32>,
    post_id: Option<i32>,
    board_name: Option<String>,
    post_title: Option<String>,
    reason: Option<String>,
    created_by: i32,
    created_by_email: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    source_post_id: Option<i32>,
    source_comment_id: Option<i32>,
}

#[get("/me/bans")]
pub async fn get_bans(
    pool: web::Data<PgPool>,
    user: Option<web::ReqData<AuthenticatedUser>>,
    query: web::Query<models::PaginationParams>,
) -> Result<HttpResponse, ServiceError> {
    let authenticated_user = match user {
        Some(u) => {
            log::info!(
                "[get_bans handler v3] User authenticated with ID: {}",
                u.user_id
            );
            u
        }
        None => {
            // 認証されていない場合は、ここで処理を中断し、401 Unauthorizedエラーを返す
            log::warn!("[get_bans handler v3] User not authenticated. Aborting with 401.");
            return Err(ServiceError::Unauthorized);
        }
    };

    // このユーザーが作成したBANの総件数を取得
    let total_count: i64 = sqlx::query_scalar!(
        "SELECT count(*) FROM bans WHERE created_by = $1",
        authenticated_user.user_id
    )
    .fetch_one(pool.get_ref())
    .await?
    .unwrap_or(0);

    // ページネーションのためのオフセットを計算
    let offset = (query.page - 1) * query.limit;

    // このエンドポイントは、ログインしているユーザーが作成したBANのみを返す。
    let ban_rows = sqlx::query_as!(
        MyBanRow,
        r#"
        SELECT
            b.id,
            b.ban_type as "ban_type: BanType",
            b.hash_value,
            b.board_id,
            b.post_id,
            bo.name as "board_name?",
            p.title as "post_title?",
            b.reason,
            b.created_by,
            u.email as "created_by_email?",
            b.created_at,
            b.expires_at,
            b.source_post_id,
            b.source_comment_id
        FROM bans b
        LEFT JOIN boards bo ON b.board_id = bo.id
        LEFT JOIN posts p ON b.post_id = p.id
        LEFT JOIN users u ON b.created_by = u.id
        WHERE b.created_by = $1
        ORDER BY b.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
        authenticated_user.user_id,
        query.limit,
        offset
    )
    .fetch_all(pool.get_ref())
    .await
    .map_err(|e| {
        // DBクエリでエラーが発生した場合もログに残す
        log::error!("[get_bans handler v3] Database query failed: {}", e);
        ServiceError::from(e)
    })?;

    // 取得したデータを、フロントエンドが期待するBanDetails形式に変換する
    let bans: Vec<BanDetails> = ban_rows
        .into_iter()
        .map(|row| {
            let (scope, scope_display_name) = if row.post_id.is_some() {
                ("Thread".to_string(), "スレッド内".to_string())
            } else if row.board_id.is_some() {
                ("Board".to_string(), "板内".to_string())
            } else {
                ("Global".to_string(), "グローバル".to_string())
            };

            BanDetails {
                id: row.id,
                ban_type: row.ban_type,
                hash_value: row.hash_value,
                board_id: row.board_id,
                post_id: row.post_id,
                board_name: row.board_name,
                post_title: row.post_title,
                reason: row.reason,
                created_by: row.created_by,
                created_by_email: row.created_by_email,
                scope,
                scope_display_name,
                created_at: row.created_at,
                expires_at: row.expires_at,
                source_post_id: row.source_post_id,
                source_comment_id: row.source_comment_id,
                source_email: None, // This endpoint does not decrypt PII
                source_ip_address: None,
                source_device_info: None,
                source_user_id: None,
            }
        })
        .collect();

    log::info!(
        "[get_bans handler v3] Successfully fetched {} bans for user ID {}.",
        bans.len(),
        authenticated_user.user_id
    );

    let response = PaginatedBansResponse { bans, total_count };

    Ok(HttpResponse::Ok().json(response))
}

#[delete("/{id}")]
pub async fn delete_ban(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    let ban_id = path.into_inner();

    // 1. 権限チェックに必要な情報を1回のクエリで取得
    let perm_info = sqlx::query_as!(
        BanPermissionInfo,
        r#"
        SELECT
            b.created_by as "ban_creator_id!",
            bo.created_by as "board_owner_id"
        FROM bans b
        LEFT JOIN boards bo ON b.board_id = bo.id
        WHERE b.id = $1
        "#,
        ban_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("指定されたBANが見つかりません。".to_string()))?;

    // 2. 権限を判定
    let is_admin = matches!(user.role, Role::Admin);
    let is_ban_creator = perm_info.ban_creator_id == user.user_id;
    let is_board_owner = perm_info.board_owner_id == Some(user.user_id);

    // 権限がない場合はエラーを返す (管理者、BAN作成者、板オーナーのいずれでもない)
    if !is_admin && !is_ban_creator && !is_board_owner {
        return Err(ServiceError::Forbidden(
            "このBANを削除する権限がありません。".to_string(),
        ));
    }

    // 3. BANを削除
    let result = sqlx::query!("DELETE FROM bans WHERE id = $1", ban_id)
        .execute(pool.get_ref())
        .await?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::NotFound(
            "削除対象のBANが見つかりませんでした。".to_string(),
        ));
    }

    Ok(HttpResponse::NoContent().finish())
}

/// Checks if a user is banned from posting on a specific board.
///
/// This function checks for both board-specific and global bans based on the
/// provided user, IP, and device hashes.
///
/// # Arguments
/// * `pool` - The database connection pool.
/// * `board_id` - The ID of the board where the post is being made.
/// * `user_hash` - The permanent user hash of the poster.
/// * `ip_hash` - The permanent IP hash of the poster.
/// * `device_hash` - The permanent device hash of the poster.
///
/// # Returns
/// * `Ok(())` if the user is not banned.
/// * `Err(ServiceError::Forbidden)` if the user is banned. The error message is generic
///   to avoid revealing the ban status directly to the user.
pub async fn check_if_banned(
    conn: &mut sqlx::PgConnection,
    board_id: Option<i32>,
    post_id: Option<i32>,
    user_hash: Option<&str>,
    ip_hash: Option<&str>,
    device_hash: Option<&str>,
) -> Result<(), ServiceError> {
    let is_banned: bool = sqlx::query_scalar!(
        r#"
        SELECT EXISTS (
            SELECT 1 FROM bans
            WHERE
                -- Check for a matching hash
                (
                    (ban_type = 'user' AND hash_value = $3) OR
                    (ban_type = 'ip' AND hash_value = $4) OR
                    (ban_type = 'device' AND hash_value = $5)
                )
                -- And check if the scope applies
                AND (
                    (board_id IS NULL AND post_id IS NULL) -- Global Ban
                    OR (board_id = $1 AND post_id IS NULL)    -- Board Ban
                    OR (post_id = $2)                       -- Thread Ban
                )
        )
        "#,
        board_id,
        post_id,
        user_hash,
        ip_hash,
        device_hash
    )
    .fetch_one(conn)
    .await?
    .unwrap_or(false);

    if is_banned {
        Err(ServiceError::Forbidden("".to_string()))
    } else {
        Ok(())
    }
}
