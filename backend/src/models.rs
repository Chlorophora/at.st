use crate::middleware::Role;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json;
use sqlx::FromRow;
use std::collections::HashMap;
use validator::{Validate, ValidationError};

#[derive(Debug, FromRow, Serialize, Clone)]
pub struct Post {
    pub id: i32,
    pub title: String,
    pub body: String,
    pub author_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub board_id: Option<i32>,
    pub deleted_at: Option<chrono::DateTime<chrono::Utc>>, // 論理削除日時
    pub user_id: Option<i32>,
    pub archived_at: Option<chrono::DateTime<chrono::Utc>>, // 過去ログ化日時
    pub last_activity_at: DateTime<Utc>,
    pub display_user_id: Option<String>,
    pub permanent_user_hash: Option<String>,
    pub permanent_ip_hash: Option<String>,
    pub permanent_device_hash: Option<String>,
    pub level_at_creation: Option<i32>,
    // 投稿者の現在のレベル。権限や設定によりNoneになる
    pub level: Option<i32>,
    // 現在のレベルが閾値によって隠されているかを示すフラグ
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_current_level_hidden: Option<bool>,
}

// カスタムバリデーション関数:
// 1. 15文字以上の連続した英数字を禁止する
// 2. "!token(...)" 形式の文字列を禁止する
fn validate_no_suspicious_sequences(text: &str) -> Result<(), ValidationError> {
    // 15文字以上の連続した英数字をチェック
    // Unicodeプロパティを使い、より広範な文字種の連続に対応
    static RE_ALPHANUM: Lazy<Regex> = Lazy::new(|| Regex::new(r"[\p{L}\p{N}]{15,}").unwrap());
    if RE_ALPHANUM.is_match(text) {
        let mut error = ValidationError::new("no_long_alphanumeric_sequences");
        error.message = Some("15文字以上の連続した英数字は使用できません。".into());
        return Err(error);
    }

    // "!token(...)" 形式をチェック
    static RE_TOKEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"!token\([a-zA-Z0-9]{32}\)").unwrap());
    if RE_TOKEN.is_match(text) {
        let mut error = ValidationError::new("no_linking_token");
        error.message = Some("連携トークンをこのフィールドに含めることはできません。".into());
        return Err(error);
    }

    Ok(())
}

// 本文（body）専用のカスタムバリデーション関数:
// - 15文字以上の連続した英数字のチェックを *行わない*
// - "!token(...)" 形式の文字列のみを禁止する
fn validate_body_sequences(text: &str) -> Result<(), ValidationError> {
    // "!token(...)" 形式をチェック
    static RE_TOKEN: Lazy<Regex> = Lazy::new(|| Regex::new(r"!token\([a-zA-Z0-9]{32}\)").unwrap());
    if RE_TOKEN.is_match(text) {
        let mut error = ValidationError::new("no_linking_token");
        error.message = Some("連携トークンをこのフィールドに含めることはできません。".into());
        return Err(error);
    }

    Ok(())
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreatePostRequest {
    #[validate(
        length(min = 1, max = 100, message = "文字数エラー!タイトルは1~100字まで"),
        custom(function = "validate_no_suspicious_sequences")
    )]
    pub title: String,
    #[validate(
        length(min = 1, max = 750, message = "文字数エラー!本文は1~750字まで"),
    )]
    pub body: String,
    #[validate(
        length(max = 10, message = "文字数エラー!名前は10字まで"),
        custom(function = "validate_no_suspicious_sequences")
    )]
    pub author_name: Option<String>,
    pub board_id: i32,
    // ブラウザからの投稿時に付与されるフィンガープリント
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "board_moderation_type", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")] // フロントエンドの 'alpha'/'beta' と合わせる
pub enum BoardModerationType {
    Alpha,
    Beta,
}

#[derive(Debug, FromRow, Serialize, Clone)]
pub struct Board {
    pub id: i32,
    pub name: String,
    pub description: String,
    pub default_name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_by: Option<i32>,
    pub max_posts: i32, // 板のスレッド数上限
    pub archived_at: Option<DateTime<Utc>>,
    pub moderation_type: BoardModerationType,
    pub last_activity_at: DateTime<Utc>,
    pub auto_archive_enabled: bool,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateBoardRequest {
    #[validate(
        length(min = 1, max = 20, message = "文字数エラー!板名は1~20文字まで"),
        custom(function = "validate_no_suspicious_sequences")
    )]
    pub name: String,
    #[validate(
        length(min = 1, max = 100, message = "文字数エラー!説明欄は1~100字まで"),
        custom(function = "validate_no_suspicious_sequences")
    )]
    pub description: String,
    #[validate(length(max = 10, message = "文字数エラー!デフォルト名は10文字まで"))]
    pub default_name: Option<String>,
    // ブラウザからの投稿時に付与されるフィンガープリント
    pub fingerprint: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBoardDetailsRequest {
    #[validate(
        length(min = 1, max = 20, message = "文字数エラー!板名は1~20文字まで"),
        custom(function = "validate_no_suspicious_sequences")
    )]
    pub name: Option<String>,
    #[validate(
        length(min = 1, max = 100, message = "文字数エラー!説明欄は1~100字まで"),
        custom(function = "validate_no_suspicious_sequences")
    )]
    pub description: Option<String>,
    #[validate(length(max = 10, message = "文字数エラー!デフォルト名は10文字まで"))]
    pub default_name: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBoardSettingsRequest {
    #[validate(range(min = 1, message = "スレッド数上限は1以上でなければなりません。"))]
    pub max_posts: i32,
}

#[derive(Debug, Deserialize, Validate)]
pub struct UpdateBoardModerationTypeRequest {
    pub moderation_type: BoardModerationType,
}

// --- Response Models for Board Details ---

#[derive(Serialize, Debug)]
pub struct CreatorInfoResponse {
    pub display_user_id: String,
    pub level: i32,
    pub level_at_creation: i32,
}

#[derive(Serialize, Debug, Clone)]
pub struct BoardWithModerationFlag {
    // `Board` 構造体のフィールドをインライン展開します。
    #[serde(flatten)]
    pub board: Board,
    // この板に対するモデレーション権限（管理者または作成者）があるかどうかを示します。
    pub can_moderate: bool,
}

#[derive(Serialize, Debug)]
pub struct BoardDetailResponse {
    // モデレーションフラグを含む板情報をネストします。
    pub board: BoardWithModerationFlag,
    // 管理者専用の追加フィールド
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creator_info: Option<CreatorInfoResponse>,
}

#[derive(Debug, FromRow, Serialize, Clone)]
pub struct Comment {
    pub id: i32,
    pub body: String,
    pub post_id: i32,
    pub user_id: Option<i32>,
    pub author_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub display_user_id: Option<String>, // 不要な "" を削除
    pub permanent_user_hash: Option<String>,
    pub permanent_ip_hash: Option<String>,
    pub permanent_device_hash: Option<String>,
    pub level_at_creation: Option<i32>,
    // 投稿者の現在のレベル。権限や設定によりNoneになる
    pub level: Option<i32>,
    // 現在のレベルが閾値によって隠されているかを示すフラグ
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_current_level_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_title: Option<String>,
    // ID検索結果で正しいレスナンバーを付与するために使用
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_number: Option<i64>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateCommentRequest {
    #[validate(
        length(min = 1, max = 300, message = "文字数エラー!本文は1~300字まで")
    )]
    pub body: String,
    #[validate(
        length(max = 10, message = "文字数エラー!名前は10字まで"),
        custom(function = "validate_no_suspicious_sequences")
    )]
    pub author_name: Option<String>,
    pub post_id: i32,
    // ブラウザからの投稿時に付与されるフィンガープリント
    pub fingerprint: Option<String>,
}

// Post詳細ページ用の新しいレスポンスモデル
#[derive(Serialize, Debug)]
pub struct PostDetailResponse {
    pub post: Post,
    // この投稿に対するモデレーション権限があるかどうかを示します。
    pub can_moderate: bool,
    // パンくずリスト表示用に板の名前とIDを追加
    pub board_name: String,
    pub board_id: i32,
}

#[derive(Serialize)]
pub struct CommentResponse {
    pub comment: Comment,
    // このコメントに対するモデレーション権限があるかどうかを示します。
    pub can_moderate: bool,
}

// --- User History Search Models ---

#[derive(Serialize, Debug)]
#[serde(tag = "type", content = "data")] // Corresponds to frontend's HistoryItem
pub enum HistoryItem {
    Post(Post),
    Comment(Comment),
}

#[derive(Serialize, Debug, Default)]
pub struct HistorySummary {
    pub first_seen: Option<DateTime<Utc>>,
    pub last_seen: Option<DateTime<Utc>>,
    pub created_thread_count: i64,
    pub comment_count: i64,
    pub total_contribution_count: i64,
    // Vec<(String, i64)> becomes [["title1", count1], ["title2", count2]] in JSON
    pub created_threads: Vec<(String, i64)>,
    pub commented_in_threads: Vec<(String, i64)>,
}

#[derive(Serialize, Debug)]
pub struct HistoryResponse {
    pub summary: HistorySummary,
    pub items: Vec<HistoryItem>,
}

// --- Admin History & Pagination Models ---

/// ページネーション化されたレスポンス用の汎用構造体
#[derive(Serialize)]
pub struct PaginatedResponse<T> {
    pub items: Vec<T>,
    pub total_count: i64,
}

/// ユーザーのレス投稿履歴の各項目を表す構造体
#[derive(Debug, FromRow, Serialize)]
pub struct CommentHistoryItem {
    pub id: i32,
    pub body_snippet: Option<String>,
    pub post_id: i32,
    pub post_title: Option<String>,
    pub board_id: Option<i32>,
    pub board_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub proxycheck_json: Option<serde_json::Value>,
}

/// ユーザーの認証・レベルアップ履歴の各項目を表す構造体
#[derive(Debug, FromRow, Serialize)]
pub struct VerificationHistoryItem {
    pub id: i32,
    pub attempt_type: Option<String>,
    pub is_success: bool,
    pub ip_address: Option<String>,
    pub created_at: DateTime<Utc>,
    pub rejection_reason: Option<String>,
    pub fingerprint_json: Option<serde_json::Value>,
    pub proxycheck_json: Option<serde_json::Value>,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProxyCheckDetections {
    pub proxy: bool,
    pub vpn: bool,
    pub tor: bool,
    pub hosting: bool,
    pub compromised: bool,
    pub scraper: bool,
    pub anonymous: bool,
    pub risk: i64,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProxyCheckIpDetails {
    // network, location, device_estimate, operator などの他のフィールドも
    // 必要に応じてここに追加できますが、今回はdetectionsのみ定義します。
    pub detections: Option<ProxyCheckDetections>,
    // 他のフィールドは汎用的に受け取る
    #[serde(flatten)]
    pub other_fields: HashMap<String, serde_json::Value>,
}

/// ユーザーの板作成履歴の各項目を表す構造体
#[derive(Debug, FromRow, Serialize)]
pub struct BoardHistoryItem {
    pub id: i32,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub proxycheck_json: Option<serde_json::Value>,
}

/// ユーザーのスレッド作成履歴の各項目を表す構造体
#[derive(Debug, FromRow, Serialize)]
pub struct PostHistoryItem {
    pub id: i32,
    pub title: String,
    pub board_id: Option<i32>,
    pub board_name: String,
    pub created_at: DateTime<Utc>,
    pub proxycheck_json: Option<serde_json::Value>,
}

// --- Verification / Level Up / Registration Models ---

#[derive(Deserialize)]
pub struct LevelUpRequest {
    pub turnstile_token: String,
    #[serde(rename = "fingerprintData")]
    pub fingerprint_data: serde_json::Value,
}

#[derive(Deserialize)]
pub struct RegistrationPreflightRequest {
    pub hcaptcha_token: String,
    pub turnstile_token: String, // Turnstileトークンを追加
    #[serde(rename = "fingerprintData")]
    pub fingerprint_data: serde_json::Value,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ProxyCheckResponse {
    pub status: String,
    // レスポンスはIPアドレスをキーにした動的なフィールドを持つため、
    // HashMapを使用してIPアドレスごとの詳細情報を受け取ります。
    #[serde(flatten)]
    pub ip_details: HashMap<String, ProxyCheckIpDetails>,
    #[serde(default)]
    pub query_time: Option<i64>,
}

// --- User Models ---

#[derive(Debug, FromRow, Serialize)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub role: Role,
    pub created_at: DateTime<Utc>,
    // Level system fields
    pub level: i32,
    pub last_level_up_at: Option<DateTime<Utc>>,
    pub last_level_up_ip: Option<String>,
    pub level_up_failure_count: i32,
    pub last_level_up_attempt_at: Option<DateTime<Utc>>,
    pub banned_from_level_up: bool,
    pub is_rate_limit_exempt: bool,
    // 専ブラ連携トークンの最終発行日時
    pub last_linking_token_generated_at: Option<DateTime<Utc>>,
}

// --- Settings Models ---

#[derive(Debug, FromRow, Serialize)]
pub struct Setting {
    pub key: String,
    pub value: String,
}

// --- BAN Models ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "ban_type", rename_all = "lowercase")]
#[serde(rename_all = "PascalCase")] // JSON出力時に "User", "Ip", "Device" となるように設定
pub enum BanType {
    User,
    Ip,
    Device,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum BanScope {
    Global,
    Board,
    Thread,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct Ban {
    pub id: i32,
    pub ban_type: BanType,
    pub hash_value: String,
    pub board_id: Option<i32>,
    pub reason: Option<String>,
    pub created_by: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,
    // BANの発生源を記録するため、Ban構造体にもフィールドを追加
    pub source_post_id: Option<i32>,
    pub source_comment_id: Option<i32>,
    pub post_id: Option<i32>,
    // BAN作成時に記録された暗号化PII。
    // create_banの返り値として必要だが、JSONレスポンスには含めない。
    #[serde(skip_serializing)]
    pub encrypted_source_email: Option<Vec<u8>>,
    #[serde(skip_serializing)]
    pub encrypted_source_ip: Option<Vec<u8>>,
    #[serde(skip_serializing)]
    pub encrypted_source_device_info: Option<Vec<u8>>,
}

#[derive(serde::Deserialize, Validate)]
pub struct CreateBanRequest {
    // IDを指定してBANする場合
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>,

    // ハッシュ値を直接指定してBANする場合
    #[validate(length(equal = 64))]
    pub hash_value: Option<String>,

    pub ban_type: BanType,
    pub scope: BanScope,

    #[validate(length(max = 255))]
    pub reason: Option<String>,

    // ハッシュ直接指定で板BAN/スレッドBANを行う場合に使用
    pub board_id: Option<i32>,

    // BANの発生源となったユーザーの個人情報 (フロントエンドから送信される)
    // これらは暗号化されてDBに保存される
    #[validate(length(max = 254))]
    pub source_email: Option<String>,
    #[validate(length(max = 45))] // IPv6を考慮
    pub source_ip_address: Option<String>,
    #[validate(length(max = 512))]
    pub source_device_info: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, sqlx::FromRow)]
pub struct BanDetails {
    pub id: i32,
    pub ban_type: BanType,
    pub hash_value: String,
    pub scope: String,              // "Global", "Board", "Thread"
    pub scope_display_name: String, // "グローバル", "板内", "スレッド内"
    pub board_id: Option<i32>,
    pub post_id: Option<i32>,
    pub board_name: Option<String>,
    pub post_title: Option<String>,
    pub reason: Option<String>,
    pub created_by: i32,
    pub created_by_email: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub expires_at: Option<chrono::DateTime<chrono::Utc>>,

    // BANの発生源を記録するフィールド
    pub source_post_id: Option<i32>,
    pub source_comment_id: Option<i32>,

    // 管理者向けに表示する復号化された情報 (JSONシリアライズ時にNoneなら省略)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_email: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip_address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_device_info: Option<String>,
    // 発生源となったユーザーのID (フロントエンドでのリンク生成用)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_user_id: Option<i32>,
}

// --- Admin Identity Models ---

#[derive(serde::Deserialize, Debug)]
pub struct IdentityQuery {
    pub post_id: Option<i32>,
    pub comment_id: Option<i32>,
    pub user_id: Option<i32>,
}

#[derive(serde::Serialize)]
pub struct IdentityDetails {
    pub email: String,
    pub ip_address: String,
    pub device_info: String,
    pub permanent_user_hash: Option<String>,
    pub permanent_ip_hash: Option<String>,
    pub permanent_device_hash: Option<String>,
}

// --- Rate Limiter Models ---

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "rate_limit_action_type", rename_all = "PascalCase")]
#[serde(rename_all = "PascalCase")]
pub enum RateLimitActionType {
    CreateBoard,
    CreatePost,
    CreateComment,
    SearchHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, serde::Serialize, serde::Deserialize)]
#[sqlx(type_name = "rate_limit_target_type", rename_all = "PascalCase")]
#[serde(rename_all = "PascalCase")]
pub enum RateLimitTarget {
    UserId,
    IpAddress,
    DeviceId,
    UserAndIp,
    UserAndDevice,
    IpAndDevice,
    All,
}

#[derive(Debug, FromRow, Serialize)]
pub struct RateLimitRule {
    pub id: i32,
    pub name: String,
    pub target: RateLimitTarget,
    pub action_type: RateLimitActionType,
    pub threshold: i32,
    pub time_frame_seconds: i32,
    pub lockout_seconds: i32,
    pub is_enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub created_by: i32,
}

#[derive(Debug, Serialize)]
pub struct RateLimitRuleResponse {
    #[serde(flatten)]
    pub rule: RateLimitRule,
    pub created_by_email: Option<String>,
}

#[derive(Debug, Deserialize, Validate)]
pub struct CreateRateLimitRuleRequest {
    #[validate(length(min = 1, max = 255))]
    pub name: String,
    pub target: RateLimitTarget,
    pub action_type: RateLimitActionType,
    #[validate(range(min = 1))]
    pub threshold: i32,
    #[validate(range(min = 1))]
    pub time_frame_seconds: i32,
    #[validate(range(min = 1))]
    pub lockout_seconds: i32,
    pub is_enabled: bool,
}

pub type UpdateRateLimitRuleRequest = CreateRateLimitRuleRequest;

/// ページネーション用の汎用クエリパラメータ
#[derive(Debug, Deserialize)]
pub struct PaginationParams {
    pub page: i64,
    pub limit: i64,
}
