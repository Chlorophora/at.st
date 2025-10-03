use actix_web::{
    cookie::{time::OffsetDateTime, Cookie, SameSite},
    get, post, web, HttpResponse, Responder,
};
use chrono::{Duration, Utc};
use hex;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use reqwest;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;

use crate::errors::ServiceError;
use crate::middleware::{AuthenticatedUser, Role};
use crate::{
    get_ip_address, models::{self, RegistrationPreflightRequest},
    verification::{self, VerificationInput, VerificationType},
};
use actix_web::HttpRequest;
use rand::{distributions::Alphanumeric, Rng};

// #[derive(Deserialize, Validate)]
// pub struct RequestOtpPayload {
//     #[validate(
//         email(message = "有効なメールアドレスを入力してください。"),
//         length(
//             max = 254,
//             message = "メールアドレスは254文字以下である必要があります。"
//         )
//     )]
//     pub email: String,
//     // preflight_checkから受け取った一時トークン
//     pub preflight_token: String,
// }

// #[derive(Deserialize, Validate)]
// pub struct VerifyOtpPayload {
//     #[validate(
//         email(message = "有効なメールアドレスを入力してください。"),
//         length(
//             max = 254,
//             message = "メールアドレスは254文字以下である必要があります。"
//         )
//     )]
//     pub email: String,
//     #[validate(length(equal = 6, message = "確認コードは6桁である必要があります。"))]
//     pub otp_code: String,
// }

// #[derive(Serialize)]
// struct RequestOtpResponse {
//     message: String,
//     #[serde(skip_serializing_if = "Option::is_none")]
//     otp_for_dev: Option<String>,
// }

// #[derive(Serialize)]
// struct VerifyOtpResponse {
//     message: String,
//     // 専ブラ連携用の一度きりのトークン
//     linking_token: String,
// }

#[derive(Deserialize)]
pub struct CreateAccountPayload {
    // preflight_checkから受け取った一時トークン
    pub preflight_token: String,
}

#[derive(Serialize)]
struct CreateAccountResponse {
    message: String,
    // ユーザーが保存すべき秘密のID
    account_id: String,
    // 専ブラ連携用の一度きりのトークン
    linking_token: String,
}

#[derive(Deserialize)]
pub struct LoginWithAccountIdPayload {
    // ユーザーが入力する秘密のID
    pub account_id: String,
    // preflight_checkから受け取った一時トークン
    pub preflight_token: String,
}

#[derive(Serialize)]
struct LoginSuccessResponse {
    message: String,
    linking_token: String,
}

// JWTのクレーム（ペイロード）を定義
#[derive(Debug, Serialize, Deserialize)]
struct PreflightClaims {
    exp: usize,      // 有効期限 (Unixタイムスタンプ)
    sub: String,     // 識別子 (例: "preflight-passed")
    attempt_id: i32, // level_up_attemptsテーブルのID
}

#[post("/preflight")]
pub async fn preflight_check(
    pool: web::Data<PgPool>,
    req: HttpRequest,
    http_client: web::Data<reqwest::Client>,
    data: web::Json<RegistrationPreflightRequest>,
) -> Result<HttpResponse, ServiceError> {
    // --- START: Refactored Verification Logic ---
    // Use the centralized verification function to handle all checks.
    let (truncated_ip, raw_ip) = get_ip_address(&req);
    let verification_input = VerificationInput {
        verification_type: VerificationType::Registration,
        user_id: None, // Registration doesn't have a user_id yet
        role: None,    // Registration doesn't have a role yet
        ip_address: truncated_ip,
        raw_ip_address: Some(raw_ip),
        // 必須フィールドなのでSome()でラップする
        captcha_token: Some(data.0.hcaptcha_token),
        fingerprint_data: Some(data.0.fingerprint_data.clone()),
    };

    // The perform_verification function now handles all logic, including saving the attempt.
    // We need the ID of the attempt record to generate the JWT.
    // トランザクションが不要な場合は、プールからコネクションを一つ取得して渡します。
    let mut conn = pool.acquire().await?;
    let (result, attempt_id) =
        verification::perform_verification(&mut conn, http_client.get_ref(), verification_input)
            .await?;

    if !result.is_success {
        return Err(ServiceError::BadRequest(
            result
                .rejection_reason
                .unwrap_or_else(|| "Verification failed.".to_string()),
        ));
    }

    // --- 8. Generate JWT with attempt_id ---
    let expiration = Utc::now()
        .checked_add_signed(Duration::minutes(5))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = PreflightClaims {
        exp: expiration,
        sub: "preflight-passed".to_string(),
        attempt_id,
    };
    let secret = std::env::var("JWT_SECRET")
        .map_err(|_| ServiceError::InternalServerError("JWT_SECRET not set".to_string()))?;
    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_ref()),
    )
    .map_err(|_| ServiceError::InternalServerError("Failed to create JWT".to_string()))?;

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "message": "検証に成功しました。アカウントIDを入力または新規発行してください。",
        "preflight_token": token
    })))
    // --- END: Refactored Verification Logic ---
}

// メール認証フローは使用しないが、コードは残しておく
// #[post("/request-otp")]
// pub async fn request_otp(
//     pool: web::Data<PgPool>,
//     payload: web::Json<RequestOtpPayload>,
//     http_client: web::Data<reqwest::Client>, // http_clientを引数で受け取る
// ) -> Result<impl Responder, ServiceError> {
//     // リクエストボディのバリデーション
//     payload.validate()?;

//     // --- JWT検証 ---
//     let secret = std::env::var("JWT_SECRET")
//         .map_err(|_| ServiceError::InternalServerError("JWT_SECRET not set".to_string()))?;
//     let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
//     let claims = decode::<PreflightClaims>(
//         &payload.preflight_token,
//         &DecodingKey::from_secret(secret.as_ref()),
//         &validation,
//     )
//     .map_err(|e| {
//         log::warn!("Preflight JWT validation failed: {}", e);
//         ServiceError::BadRequest(
//             "事前検証トークンが無効です。ページをリロードしてやり直してください。".to_string(),
//         )
//     })?;
//     // --- JWT検証完了 ---

//     let email = payload.email.to_lowercase();

//     // 1. ユーザーを検索し、存在しない場合は新規作成する
//     let user_id = match sqlx::query!("SELECT id FROM users WHERE email = $1", email.clone())
//         .fetch_optional(pool.get_ref())
//         .await?
//     {
//         Some(user) => user.id,
//         None => {
//             let new_user =
//                 sqlx::query!("INSERT INTO users (email) VALUES ($1) RETURNING id", email)
//                     .fetch_one(pool.get_ref())
//                     .await?
//                     .id;
//             new_user
//         }
//     };

//     // --- 事前検証で保存したレコードに、今作成/取得したユーザーIDを紐付ける ---
//     sqlx::query!(
//         "UPDATE level_up_attempts SET user_id = $1 WHERE id = $2",
//         user_id,
//         claims.claims.attempt_id
//     )
//     .execute(pool.get_ref())
//     .await?;

//     // 2. 6桁のOTPを生成
//     let mut rng = rand::thread_rng(); // rand 0.8ではこのままでOK
//     let otp_code: String = rng.gen_range(100_000..1_000_000).to_string();

//     // 3. OTPをハッシュ化
//     let salt = SaltString::generate(&mut rng);
//     let argon2 = Argon2::default();
//     let otp_code_hash = argon2
//         .hash_password(otp_code.as_bytes(), &salt)
//         .map_err(|_| ServiceError::InternalServerError("OTPハッシュ化エラー".to_string()))?
//         .to_string();

//     // 4. ハッシュ化したOTPをデータベースに保存 (有効期限は10分)
//     let expires_at = Utc::now() + Duration::minutes(10);
//     sqlx::query!(
//         "INSERT INTO otp_tokens (user_id, otp_code_hash, expires_at) VALUES ($1, $2, $3)",
//         user_id,
//         otp_code_hash,
//         expires_at
//     )
//     .execute(pool.get_ref())
//     .await?;

//     let mut response_body = RequestOtpResponse {
//         message: "確認コードを送信しました。メールを確認してください。".to_string(),
//         otp_for_dev: None,
//     };

//     // --- メール送信処理 ---
//     // リリースビルド（本番環境）の場合のみ、Mailerサービスにリクエストを送信
//     #[cfg(not(debug_assertions))]
//     {
//         let mailer_url = std::env::var("MAILER_SERVICE_URL")
//             .map_err(|_| ServiceError::InternalServerError("MAILER_SERVICE_URL is not set".to_string()))?;

//         let res = http_client.post(mailer_url)
//             .json(&serde_json::json!({
//                 "email": &email,
//                 "otp_code": &otp_code
//             }))
//             .send()
//             .await;
        
//         match res {
//             Ok(response) if response.status().is_success() => {
//                 log::info!("Successfully requested OTP email for {} via mailer service.", &email);
//             }
//             Ok(response) => {
//                 let status = response.status();
//                 let text = response.text().await.unwrap_or_else(|_| "No body".to_string());
//                 log::error!("Mailer service returned an error. Status: {}, Body: {}", status, text);
//                 return Err(ServiceError::InternalServerError("Failed to send confirmation email.".to_string()));
//             }
//             Err(e) => {
//                 log::error!("Failed to send request to mailer service: {}", e);
//                 // ユーザーには一般的なエラーを返し、詳細はログで確認します。
//                 return Err(ServiceError::InternalServerError("Failed to send confirmation email.".to_string()));
//             }
//         }
//     }

//     // デバッグビルドの場合のみ、レスポンスにOTPを含める
//     #[cfg(debug_assertions)]
//     {
//         response_body.otp_for_dev = Some(otp_code);
//     }

//     Ok(HttpResponse::Ok().json(response_body))
// }

// メール認証フローは使用しないが、コードは残しておく
// #[post("/verify-otp")]
// pub async fn verify_otp(
//     pool: web::Data<PgPool>,
//     payload: web::Json<VerifyOtpPayload>,
//     existing_user: Option<web::ReqData<AuthenticatedUser>>, // 既存のセッション情報をオプショナルで受け取る
// ) -> Result<impl Responder, ServiceError> {
//     payload.validate()?;

//     let email = payload.email.to_lowercase();

//     // 1. Find the user by email
//     let user = sqlx::query!("SELECT id FROM users WHERE email = $1", email)
//         .fetch_optional(pool.get_ref())
//         .await?
//         .ok_or_else(|| {
//             ServiceError::BadRequest("ユーザーが見つからないか、確認コードが無効です。".to_string())
//         })?;

//     // 2. Find the most recent, unexpired, and unused OTP for that user
//     let token_record = sqlx::query!(
//         "SELECT id, otp_code_hash FROM otp_tokens WHERE user_id = $1 AND expires_at > NOW() AND used_at IS NULL ORDER BY created_at DESC LIMIT 1",
//         user.id
//     )
//     .fetch_optional(pool.get_ref())
//     .await?
//     .ok_or_else(|| ServiceError::BadRequest("確認コードが無効か、有効期限が切れています。".to_string()))?;

//     // 3. Check if the provided OTP hash matches
//     let parsed_hash = PasswordHash::new(&token_record.otp_code_hash)
//         .map_err(|_| ServiceError::InternalServerError("OTPハッシュ解析エラー".to_string()))?;

//     if Argon2::default()
//         .verify_password(payload.otp_code.as_bytes(), &parsed_hash)
//         .is_err()
//     {
//         return Err(ServiceError::BadRequest(
//             "確認コードが正しくありません。".to_string(),
//         ));
//     }

//     // --- OTP is valid, create a session ---

//     // 4. Generate a secure session token
//     let session_token: String = rand::thread_rng()
//         .sample_iter(&Alphanumeric)
//         .take(64)
//         .map(char::from)
//         .collect();

//     // トランザクションを開始
//     let mut tx = pool.begin().await?;

//     // もしリクエストに既存のセッション情報（別のアカウントのもの）が含まれていたら、
//     // そのセッションをDBから削除する。これにより、古いアカウントのセッションが残らないようにする。
//     if let Some(prev_user) = existing_user {
//         // これからログインするユーザーと、既存セッションのユーザーが違う場合も考慮し、
//         // とにかくリクエストに含まれていた古いセッションは削除する。
//         sqlx::query!("DELETE FROM sessions WHERE user_id = $1", prev_user.user_id)
//             .execute(&mut *tx)
//             .await?;
//     }

//     // 5. このユーザーIDに紐づく既存のセッションをすべて削除する (1ユーザー1セッションを強制)
//     sqlx::query!("DELETE FROM sessions WHERE user_id = $1", user.id)
//         .execute(&mut *tx)
//         .await?;

//     // 6. 新しいセッショントークンをデータベースに保存する
//     let session_expires_at = Utc::now() + chrono::Duration::days(365 * 1000);
//     sqlx::query!(
//         "INSERT INTO sessions (user_id, session_token, expires_at) VALUES ($1, $2, $3)",
//         user.id,
//         &session_token,
//         session_expires_at
//     )
//     .execute(&mut *tx)
//     .await?;

//     // 7. OTPを使用済みにする
//     sqlx::query!(
//         "UPDATE otp_tokens SET used_at = NOW() WHERE id = $1",
//         token_record.id,
//     )
//     .execute(&mut *tx)
//     .await?;

//     let linking_token = generate_and_save_linking_token(&mut tx, user.id).await?;

//     // トランザクションをコミット
//     tx.commit().await?;

//     // --- START: 環境に応じたCookie設定 ---
//     let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string());
//     let mut cookie_builder = Cookie::build("session_token", session_token)
//         .path("/")
//         .http_only(true)
//         .expires(
//             OffsetDateTime::from_unix_timestamp(session_expires_at.timestamp()).map_err(|_| {
//                 ServiceError::InternalServerError("Failed to create cookie expiration".to_string())
//             })?,
//         );

//     if app_env == "production" {
//         // 本番環境では、HTTPSが必須なのでSecure属性を付け、クロスドメインを許容する
//         cookie_builder = cookie_builder.secure(true).same_site(SameSite::None);
//     } else {
//         // 開発環境(HTTP)では、Secure属性を外します。
//         // また、一部の専ブラは SameSite 属性を正しく解釈できないため、
//         // この属性自体を省略することで、最大限の互換性を確保します。
//         cookie_builder = cookie_builder.secure(false);
//     }

//     let cookie = cookie_builder.finish().into_owned();
//     // --- END: 環境に応じたCookie設定 ---

//     // HttpResponseBuilderのcookieメソッドを使用するように統一
//     Ok(HttpResponse::Ok().cookie(cookie).json(VerifyOtpResponse {
//         message: "認証に成功しました。".to_string(),
//         linking_token,
//     }))
// }

/// 新しいアカウントIDを発行します。
#[post("/create-account")]
pub async fn create_account(
    pool: web::Data<PgPool>,
    payload: web::Json<CreateAccountPayload>,
    existing_user: Option<web::ReqData<AuthenticatedUser>>, // 既存のセッション情報をオプショナルで受け取る
) -> Result<impl Responder, ServiceError> {
    // --- 1. JWT検証 ---
    let secret = std::env::var("JWT_SECRET")
        .map_err(|_| ServiceError::InternalServerError("JWT_SECRET not set".to_string()))?;
    let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    let claims = decode::<PreflightClaims>(
        &payload.preflight_token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )
    .map_err(|e| {
        log::warn!("Preflight JWT validation failed for create_account: {}", e);
        ServiceError::BadRequest(
            "事前検証トークンが無効です。ページをリロードしてやり直してください。".to_string(),
        )
    })?;

    // --- 2. 新しいユーザーとアカウントIDを作成 ---
    let mut tx = pool.begin().await?;

    // 32文字のランダムな英数字でアカウントIDを生成
    let account_id: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect();

    // 新しいユーザーをDBに挿入
    // emailはNULL許容になったと仮定
    // 要件通り、emailカラムにアカウントIDを保存する
    let new_user = sqlx::query!(
        "INSERT INTO users (email) VALUES ($1) RETURNING id",
        &account_id
    )
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| {
        if let Some(db_err) = e.as_database_error() {
            if db_err.is_unique_violation() {
                return ServiceError::InternalServerError(
                    "アカウントIDの生成に失敗しました。もう一度お試しください。".to_string(),
                );
            }
        }
        e.into()
    })?;

    let new_user_id = new_user.id;

    // --- 3. 事前検証レコードにユーザーIDを紐付け ---
    sqlx::query!(
        "UPDATE level_up_attempts SET user_id = $1 WHERE id = $2",
        new_user_id,
        claims.claims.attempt_id
    )
    .execute(&mut *tx)
    .await?;

    // --- 4. 新しいセッションを作成 ---
    // もしリクエストに既存のセッション情報が含まれていたら、そのセッションをDBから削除
    if let Some(prev_user) = existing_user {
        sqlx::query!("DELETE FROM sessions WHERE user_id = $1", prev_user.user_id)
            .execute(&mut *tx)
            .await?;
    }

    // 新しいセッショントークンを生成
    let session_token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    let session_expires_at = Utc::now() + chrono::Duration::days(365 * 1000);
    sqlx::query!(
        "INSERT INTO sessions (user_id, session_token, expires_at) VALUES ($1, $2, $3)",
        new_user_id,
        &session_token,
        session_expires_at
    )
    .execute(&mut *tx)
    .await?;

    // --- 5. 専ブラ連携用トークンを生成 ---
    let linking_token = generate_and_save_linking_token(&mut *tx, new_user_id).await?;

    // --- 6. トランザクションをコミット ---
    tx.commit().await?;

    // --- 7. セッションCookieを生成 ---
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string());
    let mut cookie_builder = Cookie::build("session_token", session_token)
        .path("/")
        .http_only(true)
        .expires(OffsetDateTime::from_unix_timestamp(session_expires_at.timestamp()).map_err(|_| ServiceError::InternalServerError("Failed to create cookie expiration".to_string()))?);

    if app_env == "production" {
        cookie_builder = cookie_builder.secure(true).same_site(SameSite::None);
    } else {
        cookie_builder = cookie_builder.secure(false);
    }
    let cookie = cookie_builder.finish().into_owned();

    // --- 8. レスポンスを返す ---
    let response_body = CreateAccountResponse {
        message: "新しいアカウントが発行されました。".to_string(),
        account_id,
        linking_token,
    };

    Ok(HttpResponse::Ok().cookie(cookie).json(response_body))
}

/// アカウントIDでログインします。
#[post("/login-with-account-id")]
pub async fn login_with_account_id(
    pool: web::Data<PgPool>,
    payload: web::Json<LoginWithAccountIdPayload>,
    existing_user: Option<web::ReqData<AuthenticatedUser>>,
) -> Result<impl Responder, ServiceError> {
    // --- 1. JWT検証 ---
    let secret = std::env::var("JWT_SECRET")
        .map_err(|_| ServiceError::InternalServerError("JWT_SECRET not set".to_string()))?;
    let validation = Validation::new(jsonwebtoken::Algorithm::HS256);
    let claims = decode::<PreflightClaims>(
        &payload.preflight_token,
        &DecodingKey::from_secret(secret.as_ref()),
        &validation,
    )
    .map_err(|e| {
        log::warn!("Preflight JWT validation failed for login_with_account_id: {}", e);
        ServiceError::BadRequest(
            "事前検証トークンが無効です。ページをリロードしてやり直してください。".to_string(),
        )
    })?;

    // --- 2. アカウントIDでユーザーを検索 ---
    // 要件通り、emailカラムをアカウントIDとして検索する
    let user = sqlx::query!(
        "SELECT id FROM users WHERE email = $1",
        payload.account_id
    )
        .fetch_optional(pool.get_ref())
        .await?
        .ok_or_else(|| {
            ServiceError::BadRequest("アカウントIDが見つかりません。".to_string())
        })?;

    let user_id = user.id;

    // --- 3. 新しいセッションを作成 ---
    let mut tx = pool.begin().await?;

    // 事前検証レコードにユーザーIDを紐付け
    sqlx::query!(
        "UPDATE level_up_attempts SET user_id = $1 WHERE id = $2",
        user_id,
        claims.claims.attempt_id
    )
    .execute(&mut *tx)
    .await?;

    // 既存のセッションがあれば削除
    if let Some(prev_user) = existing_user {
        sqlx::query!("DELETE FROM sessions WHERE user_id = $1", prev_user.user_id)
            .execute(&mut *tx)
            .await?;
    }

    // このユーザーIDに紐づく他のセッションもすべて削除 (1ユーザー1セッションを強制)
    sqlx::query!("DELETE FROM sessions WHERE user_id = $1", user_id)
        .execute(&mut *tx)
        .await?;

    // 新しいセッショントークンを生成して保存
    let session_token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(64)
        .map(char::from)
        .collect();

    let session_expires_at = Utc::now() + chrono::Duration::days(365 * 1000);
    sqlx::query!(
        "INSERT INTO sessions (user_id, session_token, expires_at) VALUES ($1, $2, $3)",
        user_id,
        &session_token,
        session_expires_at
    )
    .execute(&mut *tx)
    .await?;

    // --- 4. 専ブラ連携用トークンを生成 ---
    let linking_token = generate_and_save_linking_token(&mut *tx, user_id).await?;

    // --- 5. トランザクションをコミット ---
    tx.commit().await?;

    // --- 6. セッションCookieを生成 ---
    let app_env = std::env::var("APP_ENV").unwrap_or_else(|_| "production".to_string());
    let mut cookie_builder = Cookie::build("session_token", session_token)
        .path("/")
        .http_only(true)
        .expires(OffsetDateTime::from_unix_timestamp(session_expires_at.timestamp()).map_err(|_| ServiceError::InternalServerError("Failed to create cookie expiration".to_string()))?);

    if app_env == "production" {
        cookie_builder = cookie_builder.secure(true).same_site(SameSite::None);
    } else {
        cookie_builder = cookie_builder.secure(false);
    }
    let cookie = cookie_builder.finish().into_owned();

    // --- 7. レスポンスを返す ---
    let response_body = LoginSuccessResponse {
        message: "ログインに成功しました。".to_string(),
        linking_token,
    };

    Ok(HttpResponse::Ok().cookie(cookie).json(response_body))
}

// This struct defines the shape of the JSON response for the /me endpoint.
// It should match the `User` type defined in the frontend's `app.d.ts`.
#[derive(Serialize)]
struct UserResponse {
    user_id: i32,
    email: Option<String>,
    role: String,
    level: i32,
    is_rate_limit_exempt: bool,
}

#[get("/me")]
pub async fn get_me(
    user: Option<web::ReqData<AuthenticatedUser>>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ServiceError> {
    let authenticated_user = user.ok_or(ServiceError::Unauthorized)?;

    // ユーザーの完全な情報を取得
    let user_details = sqlx::query!(
        r#"SELECT email, is_rate_limit_exempt FROM users WHERE id = $1"#,
        authenticated_user.user_id
    )
    .fetch_optional(pool.get_ref())
    .await?
    .ok_or_else(|| ServiceError::NotFound("User not found".to_string()))?;

    Ok(HttpResponse::Ok().json(UserResponse {
        user_id: authenticated_user.user_id,
        // user_details.email にはアカウントIDが入っているが、フィールド名はemailのまま返す
        email: Some(user_details.email),
        role: authenticated_user.role.to_string(),
        level: authenticated_user.level,
        is_rate_limit_exempt: user_details.is_rate_limit_exempt,
    }))
}

/// [管理者用] 自身のレート制限免除設定を切り替えます。
#[post("/me/toggle-rate-limit-exemption")]
pub async fn toggle_rate_limit_exemption(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
) -> Result<HttpResponse, ServiceError> {
    if !matches!(user.role, Role::Admin) {
        return Err(ServiceError::Forbidden(
            "Only admins can change this setting.".to_string(),
        ));
    }

    // is_rate_limit_exempt の値を反転させて更新します。
    // `models::User` に `RETURNING` の結果をマッピングします。
    let updated_user = sqlx::query_as!(
        models::User,
        r#"
        UPDATE users SET is_rate_limit_exempt = NOT is_rate_limit_exempt, updated_at = NOW()
        WHERE id = $1
        RETURNING id, email, role as "role: _", created_at, level, last_level_up_at, last_level_up_ip, level_up_failure_count, last_level_up_attempt_at, banned_from_level_up, is_rate_limit_exempt, last_linking_token_generated_at
        "#,
        user.user_id
    )
    .fetch_one(pool.get_ref())
    .await?;

    Ok(HttpResponse::Ok().json(updated_user))
}

/// [認証必須] 専ブラ連携用のトークンを再発行します。
#[post("/me/regenerate-linking-token")]
pub async fn regenerate_linking_token(
    pool: web::Data<PgPool>,
    user: web::ReqData<AuthenticatedUser>,
) -> Result<HttpResponse, ServiceError> {
    const COOLDOWN_SECONDS: i64 = 60;

    // トランザクションを開始し、チェックと更新をアトミックに行う
    let mut tx = pool.begin().await?;

    // ユーザーの最終発行日時を取得
    let last_generated_at: Option<chrono::DateTime<Utc>> = sqlx::query_scalar!(
        "SELECT last_linking_token_generated_at FROM users WHERE id = $1",
        user.user_id
    )
    .fetch_one(&mut *tx)
    .await?;

    // クールダウン期間中かチェック
    if let Some(last_time) = last_generated_at {
        let elapsed = Utc::now().signed_duration_since(last_time).num_seconds();
        if elapsed < COOLDOWN_SECONDS {
            let remaining = COOLDOWN_SECONDS - elapsed;
            return Err(ServiceError::TooManyRequests(format!(
                "トークンを再発行するには、あと {} 秒待つ必要があります。",
                remaining
            )));
        }
    }

    // 最終発行日時を更新
    sqlx::query!(
        "UPDATE users SET last_linking_token_generated_at = NOW() WHERE id = $1",
        user.user_id
    )
    .execute(&mut *tx)
    .await?;

    // 新しいトークンを生成してDBに保存
    let linking_token = generate_and_save_linking_token(&mut tx, user.user_id).await?;

    // トランザクションをコミット
    tx.commit().await?;

    log::info!("User {} regenerated a device linking token.", user.user_id);

    Ok(HttpResponse::Ok().json(serde_json::json!({
        "message": "新しい連携トークンを発行しました。10分以内に使用してください。",
        "linking_token": linking_token,
    })))
}

/// 連携トークンを生成し、ハッシュ化してDBに保存するヘルパー関数。
/// 生のトークンを返す。
async fn generate_and_save_linking_token(
    conn: &mut sqlx::PgConnection,
    user_id: i32,
) -> Result<String, ServiceError> {
    // 1. 安全な一度きりの専ブラ連携トークンを生成する
    let linking_token: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32) // 32文字のトークン
        .map(char::from)
        .collect();

    // 2. 連携トークンを保存用にハッシュ化する (SHA-256)
    let mut hasher = Sha256::new();
    hasher.update(linking_token.as_bytes());
    let linking_token_hash = hex::encode(hasher.finalize());

    // 3. ハッシュ化した連携トークンをデータベースに保存する (有効期限10分)
    let linking_token_expires_at = Utc::now() + Duration::minutes(10);
    sqlx::query!(
        "INSERT INTO device_linking_tokens (user_id, token_hash, expires_at) VALUES ($1, $2, $3)",
        user_id,
        linking_token_hash,
        linking_token_expires_at
    )
    .execute(conn)
    .await?;

    Ok(linking_token)
}
