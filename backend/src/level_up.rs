use actix_web::{get, post, web, HttpRequest, HttpResponse};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::{
    bans, get_ip_address,
    errors::ServiceError,
    identity,
    middleware::AuthenticatedUser,
    models,
    users,
    verification::{self, VerificationInput, VerificationType},
};

const LEVEL_UP_SUCCESS_LOCK_HOURS: i64 = 23;
const MAX_FAILURE_ATTEMPTS: i32 = 3;
const FAILURE_LOCKOUT_MINUTES: i64 = 5;

/// preflightリクエスト用の構造体
#[derive(Deserialize)]
pub struct LevelUpPreflightRequest {
    pub turnstile_token: String,
    #[serde(rename = "fingerprintData")]
    pub fingerprint_data: serde_json::Value,
}

/// finalizeリクエスト用の構造体
#[derive(Deserialize)]
pub struct LevelUpFinalizeRequest {
    pub level_up_token: String,
}

#[derive(Serialize, Clone)]
struct LevelUpStatusResponse {
    can_attempt: bool,
    is_locked: bool,
    lock_expires_in_seconds: Option<i64>,
    message: String,
}

/// レベルアップ用JWTのクレーム
#[derive(Debug, Serialize, Deserialize)]
struct LevelUpClaims {
    exp: usize,      // 有効期限 (Unixタイムスタンプ)
    sub: String,     // 識別子 (例: "level-up-preflight-passed")
    attempt_id: i32, // level_up_attemptsテーブルのID
}

/// ヘルパー関数: ユーザーレコードからレベルアップステータスを計算する (DBアクセスなし)
fn calculate_level_up_status(user: &models::User) -> LevelUpStatusResponse {
    if user.role == crate::middleware::Role::Admin {
        return LevelUpStatusResponse {
            can_attempt: true,
            is_locked: false,
            lock_expires_in_seconds: None,
            message: "管理者権限により、いつでもレベル上げが可能です。".to_string(),
        };
    }

    // 1. 成功後のロックアウトを確認
    if let Some(last_success) = user.last_level_up_at {
        let lock_until = last_success + Duration::hours(LEVEL_UP_SUCCESS_LOCK_HOURS);
        if Utc::now() < lock_until {
            let remaining = lock_until.signed_duration_since(Utc::now());
            return LevelUpStatusResponse {
                can_attempt: false,
                is_locked: true,
                lock_expires_in_seconds: Some(remaining.num_seconds()),
                message: format!(
                    "次にレベル上げできるまであと: {}",
                    format_duration(remaining)
                ),
            };
        }
    }

    // 2. 失敗回数によるロックアウトを確認
    if user.level_up_failure_count >= MAX_FAILURE_ATTEMPTS {
        if let Some(last_attempt) = user.last_level_up_attempt_at {
            let lock_until = last_attempt + Duration::minutes(FAILURE_LOCKOUT_MINUTES);
            if Utc::now() < lock_until {
                let remaining = lock_until.signed_duration_since(Utc::now());
                return LevelUpStatusResponse {
                    can_attempt: false,
                    is_locked: true,
                    lock_expires_in_seconds: Some(remaining.num_seconds()),
                    message: format!(
                        "試行回数が上限に達しました。あと {} で再試行できます。",
                        format_duration(remaining)
                    ),
                };
            }
        }
    }

    LevelUpStatusResponse {
        can_attempt: true,
        is_locked: false,
        lock_expires_in_seconds: None,
        message: "レベル上げが可能です。".to_string(),
    }
}

#[get("/status")]
pub async fn get_status(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
) -> Result<HttpResponse, ServiceError> {
    let user_record = sqlx::query_as!(
        models::User,
        r#"
        SELECT id, email, role as "role: _", created_at, level, last_level_up_at, last_level_up_ip, level_up_failure_count, last_level_up_attempt_at, banned_from_level_up, is_rate_limit_exempt, last_linking_token_generated_at
        FROM users WHERE id = $1
        "#,
        user.user_id
    )
    .fetch_one(pool.get_ref())
    .await?;

    let status = calculate_level_up_status(&user_record);
    Ok(HttpResponse::Ok().json(status))
}

/// ステップ1: レベルアップの事前検証を行い、成功すればトークンを発行する
#[post("/preflight")]
pub async fn level_up_preflight(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    req: HttpRequest,
    http_client: web::Data<reqwest::Client>,
    data: web::Json<LevelUpPreflightRequest>,
) -> Result<HttpResponse, ServiceError> {
    let (truncated_ip, raw_ip) = get_ip_address(&req);
    let verification_input = VerificationInput {
        verification_type: VerificationType::LevelUp,
        user_id: Some(user.user_id),
        role: Some(user.role),
        ip_address: truncated_ip,
        raw_ip_address: Some(raw_ip),
        captcha_token: Some(data.0.turnstile_token),
        fingerprint_data: Some(data.0.fingerprint_data.clone()),
    };

    let mut conn = pool.acquire().await?;
    let (result, attempt_id) =
        verification::perform_verification(&mut conn, http_client.get_ref(), verification_input)
            .await?;

    if !result.is_success {
        let reason = result.rejection_reason.unwrap_or_else(|| "検証に失敗しました。".to_string());
        return Err(ServiceError::BadRequest(reason));
    }

    // 検証成功、JWTを生成
    let expiration = Utc::now()
        .checked_add_signed(Duration::minutes(5))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = LevelUpClaims {
        exp: expiration,
        sub: "level-up-preflight-passed".to_string(),
        attempt_id,
    };
    let secret = std::env::var("JWT_SECRET")
        .map_err(|_| ServiceError::InternalServerError("JWT_SECRET not set".to_string()))?;
    let token = encode(&Header::default(), &claims, &EncodingKey::from_secret(secret.as_ref()))
        .map_err(|_| ServiceError::InternalServerError("Failed to create JWT".to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "検証に成功しました。レベルアップを完了してください。",
        "level_up_token": token
    })))
}

/// ステップ2: トークンを使ってレベルアップを最終実行する
#[post("/finalize")]
pub async fn level_up_finalize(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
    data: web::Json<LevelUpFinalizeRequest>,
) -> Result<HttpResponse, ServiceError> {
    // --- 1. JWT検証 ---
    let secret = std::env::var("JWT_SECRET")
        .map_err(|_| ServiceError::InternalServerError("JWT_SECRET not set".to_string()))?;
    let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    let claims = decode::<LevelUpClaims>(
        &data.level_up_token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )
    .map_err(|e| {
        log::warn!("Level-up JWT validation failed: {}", e);
        ServiceError::BadRequest(
            "レベルアップトークンが無効です。もう一度やり直してください。".to_string(),
        )
    })?;

    // --- START: トランザクションと行ロック ---
    let mut tx = pool.begin().await?;

    let user_record = sqlx::query_as!(
        models::User,
        r#"
        SELECT id, email, role as "role: _", created_at, level, last_level_up_at, last_level_up_ip, level_up_failure_count, last_level_up_attempt_at, banned_from_level_up, is_rate_limit_exempt, last_linking_token_generated_at
        FROM users WHERE id = $1 FOR UPDATE
        "#,
        user.user_id
    ).fetch_one(&mut *tx).await?;

    // --- START: 上限レベルチェック ---
    let max_level = users::get_max_user_level_value(pool.get_ref()).await?;
    if user_record.level >= max_level {
        return Err(ServiceError::Forbidden(
            "既に上限レベルに達しています。".to_string(),
        ));
    }
    // --- END: 上限レベルチェック ---

    let status = calculate_level_up_status(&user_record);

    if !status.can_attempt {
        return Err(ServiceError::TooManyRequests(status.message));
    }

    // --- 2. BANチェック ---
    // トークン生成時のIP/デバイス情報を使ってBANチェックを行う
    let attempt_info = sqlx::query!(
        "SELECT ip_address, fingerprint_json FROM level_up_attempts WHERE id = $1",
        claims.claims.attempt_id
    ).fetch_optional(&mut *tx).await?.ok_or_else(|| ServiceError::BadRequest("検証履歴が見つかりません。".to_string()))?;

    let ip_address = attempt_info.ip_address.unwrap_or_default();
    let device_info = attempt_info.fingerprint_json.map(|v| v.to_string()).unwrap_or_default();

    let identity_hashes = identity::generate_identity_hashes(&user_record.email, &ip_address, &device_info);
    bans::check_if_banned(&mut tx, None, None, Some(&identity_hashes.permanent_user_hash), Some(&identity_hashes.permanent_ip_hash), Some(&identity_hashes.permanent_device_hash)).await?;

    // --- 3. レベルアップ実行 ---
    sqlx::query!(
        "UPDATE users SET level = level + 1, last_level_up_at = NOW(), level_up_failure_count = 0 WHERE id = $1",
        user_record.id,
    ).execute(&mut *tx).await?;

    tx.commit().await?;

    Ok(HttpResponse::Ok().json(serde_json::json!({ "success": true, "message": "レベルアップに成功しました！" })))
}

fn format_duration(duration: Duration) -> String {
    let hours = duration.num_hours();
    let minutes = duration.num_minutes() % 60;
    let seconds = duration.num_seconds() % 60;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}
