use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::PgConnection;

use crate::{errors::ServiceError, middleware::Role, models::ProxyCheckResponse};

// --- Configuration ---
const FINGERPRINT_3_HASH_LOCK_DURATION_HOURS: i64 = 23;
const FINGERPRINT_2_HASH_LOCK_DURATION_HOURS: i64 = 1;

// --- Structs for external APIs ---

#[derive(Deserialize)]
struct CaptchaVerifyResponse {
    success: bool,
    #[serde(default)]
    #[serde(rename = "error-codes")]
    error_codes: Vec<String>,
}

// A temporary struct to hold the result of the combined fingerprint check query.
#[derive(sqlx::FromRow)]
struct FingerprintCheckResult {
    h3_found: bool,
    h2_found: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RejectionType {
    Generic,   // 一般的な失敗 (Captcha失敗, BANなど)
    RateLimit, // レート制限による失敗 (IP/Fingerprint重複)
}

// --- Verification Input & Result ---

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerificationType {
    LevelUp,
    Registration,
    CreateBoard,
    CreatePost,
    CreateComment,
}

pub struct VerificationInput {
    pub verification_type: VerificationType,
    pub user_id: Option<i32>, // Registration時はNone
    pub role: Option<Role>,   // Registration時はNone
    pub ip_address: String, // 切り詰め済みのIPアドレス (ハッシュ生成用)
    pub raw_ip_address: Option<String>, // IP評価用
    // 投稿時は不要なためOptionに変更
    pub captcha_token: Option<String>,
    pub fingerprint_data: Option<Value>,
}

pub struct VerificationResult {
    pub is_success: bool,
    pub rejection_reason: Option<String>,
    pub rejection_type: Option<RejectionType>,
    pub proxycheck_data: Option<ProxyCheckResponse>,
    pub hashes: Option<FingerprintHashes>,
}

#[derive(Debug, Clone)]
pub struct FingerprintHashes {
    pub h3: String,   // webgl + canvas + audio
    pub h_wc: String, // webgl + canvas
    pub h_wa: String, // webgl + audio
    pub h_ca: String, // canvas + audio
}

// --- Main Verification Logic ---

pub async fn perform_verification(
    // プールとトランザクションの両方を受け入れられるようにジェネリックにする
    conn: &mut PgConnection,
    http_client: &reqwest::Client,
    input: VerificationInput,
) -> Result<(VerificationResult, i32), ServiceError> {
    // --- START: 診断ログ ---
    log::info!(
        "[Verification DIAG] === Starting verification process for type: {:?}, IP: {} ===",
        input.verification_type,
        input.ip_address
    );
    log::debug!("[Verification DIAG] Input details: user_id={:?}, role={:?}, captcha_token is_some={}, fingerprint_data is_some={}", input.user_id, input.role, input.captcha_token.is_some(), input.fingerprint_data.is_some());
    // Check if the user is an admin. If so, we can bypass rate-limiting checks.
    let is_admin = matches!(input.role, Some(Role::Admin));

    // 1. Captcha verification (Turnstile or hCaptcha)
    // 認証系のアクションの場合のみ実行
    match input.verification_type {
        VerificationType::LevelUp => {
            log::info!("[Verification DIAG] Performing Turnstile verification...");
            let token = input.captcha_token.as_deref().ok_or_else(|| ServiceError::BadRequest("Captcha token is required.".to_string()))?;
            verify_turnstile(http_client, token, Some(&input.ip_address)).await?;
            log::info!("[Verification DIAG] Turnstile verification successful.");
        }
        VerificationType::Registration => {
            log::info!("[Verification DIAG] Performing hCaptcha verification...");
            let token = input.captcha_token.as_deref().ok_or_else(|| ServiceError::BadRequest("Captcha token is required.".to_string()))?;
            verify_hcaptcha(http_client, token, Some(&input.ip_address)).await?;
            log::info!("[Verification DIAG] hCaptcha verification successful.");
        }
        // 投稿系のアクションではCaptcha検証をスキップ
        VerificationType::CreateBoard
        | VerificationType::CreatePost
        | VerificationType::CreateComment => {
            log::info!("[Verification DIAG] Skipping Captcha verification for post-related action.");
        }
    };

    // 2. Level up ban check (for level up only)
    // `perform_verification` は様々なアクションで呼ばれるようになったため、
    // レベルアップBANチェックはレベルアップ時のみに限定する。
    if let (VerificationType::LevelUp, Some(user_id)) = (&input.verification_type, input.user_id) {
        let is_banned: bool = sqlx::query_scalar!(
            // SQLクエリは変更なし
            "SELECT banned_from_level_up FROM users WHERE id = $1",
            user_id
        )
        .fetch_one(&mut *conn)
        .await
        .unwrap_or(false);

        if is_banned {
            log::warn!("[Verification DIAG] User {:?} is BANNED from level up. Failing verification.", user_id);
            // Banned accounts result in a failed verification.
            // We still need to record this attempt and return its ID to fulfill the function's contract.
            let result = VerificationResult {
                is_success: false,
                rejection_reason: Some("This account is banned from leveling up.".to_string()),
                rejection_type: Some(RejectionType::Generic),
                proxycheck_data: None,
                hashes: None,
            };
            let attempt_id = save_attempt(&mut *conn, &input, &result).await?;
            return Ok((result, attempt_id));
        }
    }

    // --- ここから先は成功・失敗に関わらず情報が記録される ---

    let mut rejection_reason: Option<String> = None;
    let mut rejection_type: Option<RejectionType> = None;

    // --- START: Fingerprint Hash Verification (Moved Up) ---
    // 最初にフィンガープリントをチェックして、ローカルで弾けるリクエストは弾くことで、
    // 外部APIへの不要なリクエストを削減します。
    // 管理者でなく、フィンガープリントデータが提供されている場合にのみ実行します。
    let hashes = if let Some(fp_data) = &input.fingerprint_data {
        let calculated_hashes = calculate_fingerprint_hashes(fp_data);
        log::debug!("[Verification DIAG] [Fingerprint] Calculated hashes: {:?}", &calculated_hashes);
        if !is_admin {
            match verify_fingerprint_hashes(&mut *conn, &calculated_hashes).await {
                Ok(Some(reason)) => {
                    // フィンガープリントが最近使用されているため、リクエストを拒否
                    rejection_reason = Some(reason);
                    log::warn!("[Verification DIAG] [Fingerprint] REJECTED. Reason: {}", rejection_reason.as_ref().unwrap());
                    rejection_type = Some(RejectionType::RateLimit);
                }
                Ok(None) => {
                    // フィンガープリントは問題なし。処理を続行。
                }
                Err(e) => {
                    // データベースエラーは致命的なので、そのままエラーを返す
                    return Err(e.into());
                }
            }
        }
        Some(calculated_hashes)
    } else {
        None
    };

    // --- START: Verification (Conditional) ---
    // アクション種別に応じて、使用する環境変数を切り替える
    let proxycheck_enabled: bool = match input.verification_type {
        VerificationType::LevelUp => {
            std::env::var("PROXYCHECK_ENABLED_LEVEL_UP").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true)
        }
        VerificationType::Registration => {
            std::env::var("PROXYCHECK_ENABLED_REGISTRATION").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true)
        }
        VerificationType::CreateBoard => {
            std::env::var("PROXYCHECK_ENABLED_CREATE_BOARD").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true)
        }
        VerificationType::CreatePost => {
            std::env::var("PROXYCHECK_ENABLED_CREATE_POST").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true)
        }
        VerificationType::CreateComment => {
            std::env::var("PROXYCHECK_ENABLED_CREATE_COMMENT").unwrap_or_else(|_| "true".to_string()).parse().unwrap_or(true)
        }
    };

    // フィンガープリントチェックでまだ拒否されていない場合のみ実行
    let proxycheck_data: Option<ProxyCheckResponse> = if rejection_reason.is_none() && proxycheck_enabled {
        log::info!("[Verification DIAG] [proxycheck] Verification is ENABLED for {:?}. Fetching data...", input.verification_type);
        // 生のIPアドレスを渡す。なければフォールバックする。
        let ip_for_proxycheck = input
            .raw_ip_address
            .as_deref()
            .unwrap_or(&input.ip_address);
        let data = get_proxycheck_data(http_client, ip_for_proxycheck).await?; // 外部APIコール
        log::debug!("[Verification DIAG] [proxycheck] Received data: {:?}", &data); // ログ出力
        // レスポンスを評価 (管理者でない場合のみ)
        if !is_admin { // 管理者チェック
            log::info!("[Verification DIAG] [proxycheck] Not an admin, evaluating response...");
            if let Some(reason) = verify_proxycheck(&data) {
                rejection_reason = Some(reason);
                rejection_type = Some(RejectionType::RateLimit);
                log::warn!("[Verification DIAG] [proxycheck] REJECTED. Reason: {}", rejection_reason.as_ref().unwrap());
            }
        }
        Some(data) // レスポンス(data)をSomeでラップして代入する
    } else {
        None // 無効、または既に拒否されている場合はNone
    };

    // --- START: Early Return on Rejection ---
    // If any of the checks above (proxy, fingerprint) have resulted in a rejection,
    // we must stop here. The transaction will be committed by the calling handler,
    // which saves the failed attempt record. This applies to all verification types.
    if rejection_reason.is_some() {
        log::warn!(
            "[Verification DIAG] Rejection occurred for type: {:?}, IP: {}, Reason: {:?}",
            input.verification_type,
            input.ip_address,
            rejection_reason.as_deref().unwrap_or("N/A")
        );
        let result = VerificationResult {
            is_success: false,
            rejection_reason,
            rejection_type,
            proxycheck_data,
            hashes,
        };
        // Save the failed attempt and return immediately.
        let attempt_id = save_attempt(conn, &input, &result).await?;
        return Ok((result, attempt_id));
    }
    // --- END: Early Return on Rejection ---

    let is_success = rejection_reason.is_none();

    let result = VerificationResult {
        is_success,
        rejection_reason: rejection_reason.clone(),
        rejection_type,
        proxycheck_data,
        hashes,
    };

    // 7. Save attempt information
    let attempt_id = save_attempt(conn, &input, &result).await?;

    log::info!(
        "[Verification DIAG] === Verification process finished. Success: {}. Reason: {:?} ===",
        result.is_success,
        result.rejection_reason
    );
    Ok((result, attempt_id))
}

// --- Helper Functions ---

/// Verifies a Cloudflare Turnstile token.
/// Returns Ok(()) on success, or an Err(ServiceError) on failure.
pub async fn verify_turnstile(
    client: &reqwest::Client,
    token: &str,
    remote_ip: Option<&str>,
) -> Result<(), ServiceError> {
    let secret_key = std::env::var("CLOUDFLARE_TURNSTILE_SECRET_KEY").map_err(|_| {
        ServiceError::InternalServerError("CLOUDFLARE_TURNSTILE_SECRET_KEY is not set.".to_string())
    })?;

    let mut params = std::collections::HashMap::new();
    params.insert("secret", secret_key);
    params.insert("response", token.to_string());
    if let Some(ip) = remote_ip {
        params.insert("remoteip", ip.to_string());
    }

    let res = client
        .post("https://challenges.cloudflare.com/turnstile/v0/siteverify")
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            ServiceError::InternalServerError(format!(
                "Failed to contact Turnstile verification server: {}",
                e
            ))
        })?;

    if !res.status().is_success() {
        return Err(ServiceError::InternalServerError(
            "Turnstile verification returned a non-success status.".to_string(),
        ));
    }

    let verification_response: CaptchaVerifyResponse = res.json().await.map_err(|e| {
        ServiceError::InternalServerError(format!("Failed to parse Turnstile response: {}", e))
    })?;

    if !verification_response.success {
        let error_codes = verification_response.error_codes.join(", ");
        log::warn!("Turnstile verification failed with errors: {}", error_codes);
        return Err(ServiceError::BadRequest(format!(
            "Turnstile verification failed. Error codes: {}",
            error_codes
        )));
    }

    Ok(())
}

/// Verifies an hCaptcha token.
/// Returns Ok(()) on success, or an Err(ServiceError) on failure.
pub async fn verify_hcaptcha(
    client: &reqwest::Client,
    token: &str,
    remote_ip: Option<&str>,
) -> Result<(), ServiceError> {
    let secret_key = std::env::var("HCAPTCHA_SECRET_KEY").map_err(|_| {
        ServiceError::InternalServerError("HCAPTCHA_SECRET_KEY is not set.".to_string())
    })?;

    let mut params = std::collections::HashMap::new();
    params.insert("secret", secret_key);
    params.insert("response", token.to_string());
    if let Some(ip) = remote_ip {
        params.insert("remoteip", ip.to_string());
    }

    let res = client
        .post("https://hcaptcha.com/siteverify")
        .form(&params)
        .send()
        .await
        .map_err(|e| {
            ServiceError::InternalServerError(format!(
                "Failed to contact hCaptcha verification server: {}",
                e
            ))
        })?;

    if !res.status().is_success() {
        return Err(ServiceError::InternalServerError(
            "hCaptcha verification returned a non-success status.".to_string(),
        ));
    }

    let verification_response: CaptchaVerifyResponse = res.json().await.map_err(|e| {
        ServiceError::InternalServerError(format!("Failed to parse hCaptcha response: {}", e))
    })?;

    if !verification_response.success {
        let error_codes = verification_response.error_codes.join(", ");
        log::warn!("hCaptcha verification failed with errors: {}", error_codes);
        return Err(ServiceError::BadRequest(format!(
            "hCaptcha verification failed. Error codes: {}",
            error_codes
        )));
    }

    Ok(())
}

pub async fn get_proxycheck_data(
    client: &reqwest::Client,
    ip: &str,
) -> Result<ProxyCheckResponse, ServiceError> {
    let api_key = std::env::var("PROXYCHECK_API_KEY")
        .map_err(|_| ServiceError::InternalServerError("PROXYCHECK_API_KEY not set".to_string()))?;
    let base_url = std::env::var("PROXYCHECK_API_URL")
        .map_err(|_| ServiceError::InternalServerError("PROXYCHECK_API_URL not set".to_string()))?;
    let url = format!("{}/{}?key={}", base_url, ip, api_key);
    log::info!("[proxycheck] Requesting data for IP: {} from URL: {}", ip, url);
    let response = client.get(&url).send().await.map_err(|e| {
        log::error!(
            "[proxycheck] API request failed. Full error details: {:?}",
            e
        );
        ServiceError::InternalServerError(format!("Failed to contact API: {}", e))
    })?;

    if !response.status().is_success() {
        let status = response.status();
        let body_text = response.text().await.unwrap_or_else(|_| "N/A".to_string());
        log::error!(
            "[proxycheck] API returned non-success status: {}. Body: {}",
            status,
            body_text
        );
        return Err(ServiceError::InternalServerError(format!("proxycheck API error: {}", status)));
    }

    // レスポンスを新しいProxyCheckResponse構造体にデシリアライズします。
    let proxy_data: ProxyCheckResponse = response.json().await.map_err(|e| ServiceError::InternalServerError(format!("Failed to parse proxycheck response: {}", e)))?;
    Ok(proxy_data)
}

pub fn calculate_fingerprint_hashes(fingerprint_data: &Value) -> FingerprintHashes {
    let get_component = |key| fingerprint_data["components"][key].to_string();
    let hash_str = |s: &str| -> String { hex::encode(Sha256::digest(s.as_bytes())) };
    let (webgl, canvas, audio) = (
        get_component("webgl"),
        get_component("canvas"),
        get_component("audio"),
    );
    FingerprintHashes {
        h3: hash_str(&format!("{}{}{}", webgl, canvas, audio)),
        h_wc: hash_str(&format!("{}{}", webgl, canvas)),
        h_wa: hash_str(&format!("{}{}", webgl, audio)),
        h_ca: hash_str(&format!("{}{}", canvas, audio)),
    }
}

/// レスポンスを評価する
fn verify_proxycheck(data: &ProxyCheckResponse) -> Option<String> {
    // `ip_details`はHashMapなので、最初の（そして唯一の）エントリの値を取得します。
    log::debug!("[verify_proxycheck] Evaluating proxycheck response...");
    if let Some(details) = data.ip_details.values().next() {
        log::debug!("[verify_proxycheck] Found IP details block.");
        // --- START: 検出ロジックの強化 ---
        // APIレスポンスでは、`detections`オブジェクトが存在することがあります。
        if let Some(detections) = &details.detections {
            log::debug!("[verify_proxycheck] Found 'detections' object: {:?}", detections);
            if detections.proxy
                || detections.vpn
                || detections.tor
                || detections.hosting
                || detections.compromised
                || detections.scraper
                || detections.anonymous
            {
                let reason = format!("Detection flag was true: proxy={}, vpn={}, tor={}, hosting={}, compromised={}, scraper={}, anonymous={}", detections.proxy, detections.vpn, detections.tor, detections.hosting, detections.compromised, detections.scraper, detections.anonymous);
                log::warn!("[verify_proxycheck] REJECTED due to detection flag. Details: {}", reason);
                return Some("検証に失敗しました。VPN・プロキシ等を使用している場合はオフにして再度お試しください。".to_string());
            }
        } else {
            log::debug!("[verify_proxycheck] 'detections' object not found. Checking other fields.");
        }

        // `detections`オブジェクトがない、または古い形式のレスポンスも考慮します。
        // `other_fields`に直接 "proxy": "yes" のようなキーと値のペアが含まれているかチェックします。
        for key in ["proxy", "vpn", "tor", "hosting", "compromised", "scraper", "anonymous"] {
            if let Some(value) = details.other_fields.get(key) {
                if value.as_str().map_or(false, |s| s.eq_ignore_ascii_case("yes")) {
                    log::warn!("[verify_proxycheck] REJECTED due to flag: '{}': '{}'", key, value);
                    return Some("検証に失敗しました。VPN・プロキシ等を使用している場合はオフにして再度お試しください。".to_string());
                }
            }
        }
        // --- END: 検出ロジックの強化 ---
    }
    None
}

pub async fn verify_fingerprint_hashes(
    conn: &mut PgConnection,
    hashes: &FingerprintHashes,
) -> Result<Option<String>, sqlx::Error> {
    // --- Development Bypass for Rate Limiting ---
    // 環境変数 `DEV_MODE_DISABLE_RATE_LIMIT` が "true" の場合、レート制限をスキップします。
    // これにより、開発中に同じブラウザで繰り返しテストを行うことが容易になります。
    let dev_mode_bypass_enabled = std::env::var("DEV_MODE_DISABLE_RATE_LIMIT")
        .unwrap_or_else(|_| "false".to_string())
        .to_lowercase()
        == "true";

    if dev_mode_bypass_enabled {
        log::warn!("[DEV_MODE] Fingerprint rate limit check is BYPASSED.");
        return Ok(None);
    }

    let h3_lock_time = Utc::now() - Duration::hours(FINGERPRINT_3_HASH_LOCK_DURATION_HOURS);
    let h2_lock_time = Utc::now() - Duration::hours(FINGERPRINT_2_HASH_LOCK_DURATION_HOURS);

    // Combine the two checks into a single, more efficient database query.
    let check_result = sqlx::query_as!(
        FingerprintCheckResult,
        r#"
        SELECT
            EXISTS (
                SELECT 1 FROM level_up_attempts
                WHERE hash_webgl_canvas_audio = $1 AND created_at > $2
            ) as "h3_found!",
            EXISTS (
                SELECT 1 FROM level_up_attempts
                WHERE (hash_webgl_canvas = $3 OR hash_webgl_audio = $4 OR hash_canvas_audio = $5)
                  AND created_at > $6
            ) as "h2_found!"
        "#,
        &hashes.h3,
        h3_lock_time,
        &hashes.h_wc,
        &hashes.h_wa,
        &hashes.h_ca,
        h2_lock_time
    )
    .fetch_one(conn)
    .await?;

    if check_result.h3_found {
        return Ok(Some(
            "Fingerprint (3-hash) has been used recently.".to_string(),
        ));
    }
    if check_result.h2_found {
        return Ok(Some(
            "Fingerprint (2-hash) has been used recently.".to_string(),
        ));
    }
    Ok(None)
}

pub async fn save_attempt(
    conn: &mut PgConnection,
    input: &VerificationInput,
    result: &VerificationResult,
) -> Result<i32, sqlx::Error> {
    // Note: The `level_up_attempts` table stores verification attempts for BOTH
    // level-up and registration processes. A more accurate name might be
    // `verification_attempts`, but it's used consistently throughout the system.
    let attempt_type_str = match input.verification_type {
        VerificationType::LevelUp => "level_up",
        VerificationType::Registration => "registration",
        VerificationType::CreateBoard => "create_board",
        VerificationType::CreatePost => "create_post",
        VerificationType::CreateComment => "create_comment",
    };
    let proxycheck_json = result
        .proxycheck_data
        .as_ref()
        .and_then(|s| serde_json::to_value(s).ok());

    let fingerprint_json = input.fingerprint_data.as_ref().and_then(|fp| serde_json::to_value(fp).ok());
    let (h3, h_wc, h_wa, h_ca) = result
        .hashes
        .as_ref()
        .map_or((None, None, None, None), |h| {
            (
                Some(h.h3.clone()),
                Some(h.h_wc.clone()),
                Some(h.h_wa.clone()),
                Some(h.h_ca.clone()),
            )
        });

    let attempt_id = sqlx::query_scalar!(
        r#"
        INSERT INTO level_up_attempts (user_id, attempt_type, is_success, ip_address, proxycheck_json, fingerprint_json, hash_webgl_canvas_audio, hash_webgl_canvas, hash_webgl_audio, hash_canvas_audio, rejection_reason)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11) RETURNING id
        "#,
        input.user_id, attempt_type_str, result.is_success, &input.ip_address, proxycheck_json, fingerprint_json, h3, h_wc, h_wa, h_ca, result.rejection_reason
    ).fetch_one(&mut *conn).await?;

    // --- START: Update user failure count on level-up failure ---
    // If the verification was for a level-up and it failed, increment the user's failure counter.
    // This ensures the failure count is always updated atomically with the attempt record.
    if let (Some(user_id), VerificationType::LevelUp, false) =
        (input.user_id, input.verification_type, result.is_success)
    {
        sqlx::query!(
            "UPDATE users SET level_up_failure_count = level_up_failure_count + 1, last_level_up_attempt_at = NOW() WHERE id = $1",
            user_id
        ).execute(&mut *conn).await?;
    }
    // --- END: Update user failure count on level-up failure ---

    Ok(attempt_id)
}
