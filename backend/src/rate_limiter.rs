use actix_web::{delete, get, post, put, web, HttpResponse};
use chrono::{Duration, Utc};
use sqlx::{FromRow, PgConnection, PgPool, Postgres, QueryBuilder};
use validator::Validate;

use crate::{
    errors::ServiceError,
    middleware::{AuthenticatedUser, Role},
    models::{
        self, CreateRateLimitRuleRequest, RateLimitRule, RateLimitRuleResponse, RateLimitTarget,
        UpdateRateLimitRuleRequest,
    },
};
use serde::Serialize;

/// [管理者用] ロック情報をフロントエンドに返すための構造体
#[derive(FromRow, Serialize, Debug)]
pub struct RateLimitLockInfo {
    pub target_key: String,
    pub expires_at: chrono::DateTime<chrono::Utc>,
    pub rule_id: i32,
    pub rule_name: Option<String>, // ルールが削除されている可能性を考慮してOption
}

/// [管理者用] レート制限ルールを作成します。
#[post("")]
pub async fn create_rate_limit_rule(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    data: web::Json<CreateRateLimitRuleRequest>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }
    data.validate()?;

    let new_rule = sqlx::query_as!(
        RateLimitRule,
        r#"
        INSERT INTO rate_limit_rules (name, target, action_type, threshold, time_frame_seconds, lockout_seconds, is_enabled, created_by)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING id, name, target as "target: _", action_type as "action_type: _", threshold, time_frame_seconds, lockout_seconds, is_enabled, created_at, updated_at, created_by
        "#,
        data.name,
        data.target as _,
        data.action_type as _,
        data.threshold,
        data.time_frame_seconds,
        data.lockout_seconds,
        data.is_enabled,
        user.user_id
    )
    .fetch_one(pool.get_ref())
    .await?;

    Ok(HttpResponse::Created().json(new_rule))
}

/// [管理者用] 全てのレート制限ルールを取得します。
#[get("")]
pub async fn get_rate_limit_rules(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }

    // A temporary struct to hold the flat query result.
    // This avoids issues with sqlx's macro expansion for nested structs.
    #[derive(sqlx::FromRow)]
    struct AdminRateLimitRuleRow {
        id: i32,
        name: String,
        target: RateLimitTarget,
        action_type: models::RateLimitActionType,
        threshold: i32,
        time_frame_seconds: i32,
        lockout_seconds: i32,
        is_enabled: bool,
        created_at: chrono::DateTime<Utc>,
        updated_at: chrono::DateTime<Utc>,
        created_by: i32,
        created_by_email: Option<String>,
    }

    let rule_rows = sqlx::query_as!(
        AdminRateLimitRuleRow,
        r#"
        SELECT
            r.id, r.name, r.target as "target: _", r.action_type as "action_type: _", r.threshold, r.time_frame_seconds,
            r.lockout_seconds, r.is_enabled, r.created_at, r.updated_at, r.created_by,
            u.email as "created_by_email?"
        FROM rate_limit_rules r
        LEFT JOIN users u ON r.created_by = u.id
        ORDER BY r.created_at DESC
        "#
    )
    .fetch_all(pool.get_ref())
    .await?;

    // Manually map the flat rows to the nested response structure.
    let rules: Vec<RateLimitRuleResponse> = rule_rows
        .into_iter()
        .map(|row| RateLimitRuleResponse {
            rule: RateLimitRule {
                id: row.id,
                name: row.name,
                target: row.target,
                action_type: row.action_type,
                threshold: row.threshold,
                time_frame_seconds: row.time_frame_seconds,
                lockout_seconds: row.lockout_seconds,
                is_enabled: row.is_enabled,
                created_at: row.created_at,
                updated_at: row.updated_at,
                created_by: row.created_by,
            },
            created_by_email: row.created_by_email,
        })
        .collect();

    Ok(HttpResponse::Ok().json(rules))
}

/// [管理者用] 特定のレート制限ルールを更新します。
#[put("/{id}")]
pub async fn update_rate_limit_rule(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    path: web::Path<i32>,
    data: web::Json<UpdateRateLimitRuleRequest>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }
    data.validate()?;
    let rule_id = path.into_inner();

    let updated_rule = sqlx::query_as!(
        RateLimitRule,
        r#"
        UPDATE rate_limit_rules
        SET name = $1, target = $2, action_type = $3, threshold = $4, time_frame_seconds = $5, lockout_seconds = $6, is_enabled = $7, updated_at = NOW()
        WHERE id = $8
        RETURNING id, name, target as "target: _", action_type as "action_type: _", threshold, time_frame_seconds, lockout_seconds, is_enabled, created_at, updated_at, created_by
        "#,
        data.name,
        data.target as _,
        data.action_type as _,
        data.threshold,
        data.time_frame_seconds,
        data.lockout_seconds,
        data.is_enabled,
        rule_id
    )
    .fetch_optional(pool.get_ref())
    .await?;

    match updated_rule {
        Some(rule) => Ok(HttpResponse::Ok().json(rule)),
        None => Err(ServiceError::NotFound("Rule not found".to_string())),
    }
}

/// [管理者用] 特定のレート制限ルールを削除します。
#[delete("/{id}")]
pub async fn delete_rate_limit_rule(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }
    let rule_id = path.into_inner();

    // トランザクションを開始し、ルールと関連ロックを不可分に削除する
    let mut tx = pool.begin().await?;

    // 1. このルールによって生成されたロックを削除
    sqlx::query!("DELETE FROM rate_limit_locks WHERE rule_id = $1", rule_id)
        .execute(&mut *tx)
        .await?;

    // 2. ルール自体を削除
    let result = sqlx::query!("DELETE FROM rate_limit_rules WHERE id = $1", rule_id)
        .execute(&mut *tx)
        .await?;

    // ルールが見つからなかった場合はロールバックしてエラーを返す
    if result.rows_affected() == 0 {
        tx.rollback().await?;
        return Err(ServiceError::NotFound("Rule not found".to_string()));
    }

    // トランザクションをコミット
    tx.commit().await?;

    Ok(HttpResponse::NoContent().finish())
}

/// [管理者用] 特定のレート制限ルールの有効/無効を切り替えます。
#[post("/{id}/toggle")]
pub async fn toggle_rate_limit_rule(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    path: web::Path<i32>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }
    let rule_id = path.into_inner();

    let updated_rule = sqlx::query_as!(
        RateLimitRule,
        r#"
        UPDATE rate_limit_rules
        SET is_enabled = NOT is_enabled, updated_at = NOW()
        WHERE id = $1
        RETURNING id, name, target as "target: _", action_type as "action_type: _", threshold, time_frame_seconds, lockout_seconds, is_enabled, created_at, updated_at, created_by
        "#,
        rule_id
    )
    .fetch_optional(pool.get_ref())
    .await?;

    updated_rule.map_or_else(
        || Err(ServiceError::NotFound("Rule not found".to_string())),
        |rule| Ok(HttpResponse::Ok().json(rule)),
    )
}

/// [管理者用] 現在有効なすべてのレート制限ロックを取得します。
#[get("/locks")]
pub async fn get_active_rate_limit_locks(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }

    let locks = sqlx::query_as!(
        RateLimitLockInfo,
        r#"
        SELECT
            l.target_key,
            l.expires_at,
            l.rule_id,
            r.name as "rule_name?"
        FROM rate_limit_locks l
        LEFT JOIN rate_limit_rules r ON l.rule_id = r.id
        WHERE l.expires_at > NOW()
        ORDER BY l.expires_at ASC
        "#
    )
    .fetch_all(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(locks))
}

/// [管理者用] 特定のレート制限ロックを解除します。
#[delete("/locks/{target_key}")]
pub async fn delete_rate_limit_lock(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    path: web::Path<String>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, Role::Admin) {
        return Err(ServiceError::Unauthorized);
    }
    let target_key_to_delete = path.into_inner();

    let result = sqlx::query!(
        "DELETE FROM rate_limit_locks WHERE target_key = $1",
        target_key_to_delete
    )
    .execute(pool.get_ref())
    .await?;

    if result.rows_affected() == 0 {
        return Err(ServiceError::NotFound(
            "指定されたロックが見つかりません。".to_string(),
        ));
    }

    Ok(HttpResponse::NoContent().finish())
}

/// 投稿者のID情報を受け取り、レート制限に違反していないかチェックし、今回の投稿イベントを記録します。
pub async fn check_and_track_rate_limits(
    conn: &mut PgConnection,
    user_id: i32,
    ip_hash: &str,
    device_hash: &str,
    action_type: models::RateLimitActionType,
) -> Result<(), ServiceError> {
    // --- START: Admin Exemption Check ---
    // First, check if the user is an admin exempt from rate limiting.
    struct UserInfo {
        role: Role,
        is_rate_limit_exempt: bool,
    }

    let user_info = sqlx::query_as!(
        UserInfo,
        r#"SELECT role as "role: _", is_rate_limit_exempt FROM users WHERE id = $1"#,
        user_id
    )
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| {
        ServiceError::InternalServerError("User not found during rate limit check.".to_string())
    })?;

    if matches!(user_info.role, Role::Admin) && user_info.is_rate_limit_exempt {
        log::info!(
            "[Rate Limiter] Skipping check for exempt admin user_id: {}",
            user_id
        );
        return Ok(());
    }
    // --- END: Admin Exemption Check ---

    let all_keys = get_all_target_keys(user_id, ip_hash, device_hash);

    // 1. まず、いずれかのキーがロックされていないかチェックする
    let now = Utc::now();
    let lock_check: Option<(String,)> = sqlx::query_as(
        "SELECT target_key FROM rate_limit_locks WHERE target_key = ANY($1) AND expires_at > $2 LIMIT 1",
    )
    .bind(&all_keys)
    .bind(now)
    .fetch_optional(&mut *conn)
    .await?;

    if lock_check.is_some() {
        return Err(ServiceError::TooManyRequests(
            "レート制限により、現在投稿できません。".to_string(),
        ));
    }

    // 2. 有効なルールをすべて取得
    let rules = sqlx::query_as!(
        RateLimitRule,
        r#"SELECT id, name, target as "target: _", action_type as "action_type: _", threshold, time_frame_seconds, lockout_seconds, is_enabled, created_at, updated_at, created_by FROM rate_limit_rules WHERE is_enabled = true AND action_type = $1"#,
        action_type as _
    )
        .fetch_all(&mut *conn)
        .await?;

    if rules.is_empty() {
        return Ok(()); // ルールがなければチェック不要
    }

    // 3. 各ルールについて違反がないかチェック
    for rule in &rules {
        let target_key = get_target_key_for_rule(&rule.target, user_id, ip_hash, device_hash);
        let time_window_start = now - Duration::seconds(rule.time_frame_seconds as i64);

        let count: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM rate_limit_tracker WHERE rule_id = $1 AND target_key = $2 AND created_at > $3",
            rule.id,
            target_key,
            time_window_start
        )
        .fetch_one(&mut *conn)
        .await?
        .unwrap_or(0);

        // 閾値に達した場合（今回の投稿で閾値以上になるので >=）
        if count >= rule.threshold as i64 {
            let expires_at = now + Duration::seconds(rule.lockout_seconds as i64);
            sqlx::query!(
                "INSERT INTO rate_limit_locks (rule_id, target_key, expires_at) VALUES ($1, $2, $3) ON CONFLICT (target_key) DO UPDATE SET expires_at = $3",
                rule.id,
                target_key,
                expires_at
            )
            .execute(&mut *conn)
            .await?;

            log::warn!(
                "Rate limit triggered for rule '{}' (ID: {}) by key '{}'. Locked until {}.",
                rule.name,
                rule.id,
                target_key,
                expires_at
            );

            return Err(ServiceError::TooManyRequests(
                "レート制限により、現在投稿できません。".to_string(),
            ));
        }
    }

    // 4. 違反がなければ、今回の投稿イベントを記録する
    // 1回の投稿で、関連するすべてのルールに対して記録を残す
    let mut query_builder: QueryBuilder<Postgres> =
        QueryBuilder::new("INSERT INTO rate_limit_tracker (rule_id, target_key) ");

    if !rules.is_empty() {
        query_builder.push_values(rules.iter(), |mut b, rule| {
            let target_key = get_target_key_for_rule(&rule.target, user_id, ip_hash, device_hash);
            b.push_bind(rule.id).push_bind(target_key);
        });

        let query = query_builder.build();
        query.execute(conn).await?;
    }

    Ok(())
}

/// ルールの監視対象に応じて、DBに保存する一意なキーを生成する
fn get_target_key_for_rule(
    target: &RateLimitTarget,
    user_id: i32,
    ip_hash: &str,
    device_hash: &str,
) -> String {
    match target {
        RateLimitTarget::UserId => format!("user:{}", user_id),
        RateLimitTarget::IpAddress => format!("ip:{}", ip_hash),
        RateLimitTarget::DeviceId => format!("device:{}", device_hash),
        RateLimitTarget::UserAndIp => format!("user_ip:{}:{}", user_id, ip_hash),
        RateLimitTarget::UserAndDevice => format!("user_device:{}:{}", user_id, device_hash),
        RateLimitTarget::IpAndDevice => format!("ip_device:{}:{}", ip_hash, device_hash),
        RateLimitTarget::All => format!("all:{}:{}:{}", user_id, ip_hash, device_hash),
    }
}

/// 投稿者に関連する全ての可能性のあるキーをリストで返す
fn get_all_target_keys(user_id: i32, ip_hash: &str, device_hash: &str) -> Vec<String> {
    vec![
        get_target_key_for_rule(&RateLimitTarget::UserId, user_id, ip_hash, device_hash),
        get_target_key_for_rule(&RateLimitTarget::IpAddress, user_id, ip_hash, device_hash),
        get_target_key_for_rule(&RateLimitTarget::DeviceId, user_id, ip_hash, device_hash),
        get_target_key_for_rule(&RateLimitTarget::UserAndIp, user_id, ip_hash, device_hash),
        get_target_key_for_rule(
            &RateLimitTarget::UserAndDevice,
            user_id,
            ip_hash,
            device_hash,
        ),
        get_target_key_for_rule(&RateLimitTarget::IpAndDevice, user_id, ip_hash, device_hash),
        get_target_key_for_rule(&RateLimitTarget::All, user_id, ip_hash, device_hash),
    ]
}

/// 古いレート制限データをクリーンアップするバッチ処理
pub async fn cleanup_rate_limiter_tables(conn: &mut PgConnection) -> Result<(), sqlx::Error> {
    // 1. 古いトラッカーデータを削除 (最も古いルールでも、その倍の期間だけ保持すれば十分)
    let max_time_frame: i32 =
        sqlx::query_scalar("SELECT COALESCE(MAX(time_frame_seconds), 3600) FROM rate_limit_rules")
            .fetch_one(&mut *conn)
            .await
            .unwrap_or(3600);

    let tracker_cutoff = Utc::now() - Duration::seconds(max_time_frame as i64 * 2);
    let tracker_result = sqlx::query!(
        "DELETE FROM rate_limit_tracker WHERE created_at < $1",
        tracker_cutoff
    )
    .execute(&mut *conn)
    .await?;

    // 2. 期限切れのロックを削除
    let lock_result = sqlx::query!("DELETE FROM rate_limit_locks WHERE expires_at < NOW()")
        .execute(conn)
        .await?;

    if tracker_result.rows_affected() > 0 || lock_result.rows_affected() > 0 {
        log::info!(
            "[RateLimiter Cleanup] Deleted {} tracker entries and {} lock entries.",
            tracker_result.rows_affected(),
            lock_result.rows_affected()
        );
    }

    Ok(())
}
