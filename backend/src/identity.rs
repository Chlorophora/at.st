// c:\Users\sahasahu\Desktop\p\niwatori\backend\src\identity.rs

use chrono::Utc;
use chrono_tz::Asia::Tokyo;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::env;
// base62エンコードのために追加
use base62;

// HMAC-SHA256の型エイリアスを定義
type HmacSha256 = Hmac<Sha256>;

/// 生成された各種ハッシュを保持するための構造体
#[derive(Debug)]
pub struct IdentityHashes {
    pub display_user_id: String,
    pub display_id_user_part: String,
    pub display_id_ip_part: String,
    pub display_id_device_part: String,
    pub permanent_user_hash: String,
    pub permanent_ip_hash: String,
    pub permanent_device_hash: String,
}

/// HMAC-SHA256ハッシュを生成するヘルパー関数
fn create_hmac_hash(key: &[u8], data: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}

/// HMAC-SHA256ハッシュからbase62エンコードされた指定長のIDを生成するヘルパー関数
fn create_base62_id_part(key: &[u8], data: &str, length: usize) -> String {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    let hash_result = mac.finalize().into_bytes();

    // ハッシュ結果の先頭16バイト（128ビット）をu128数値に変換
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&hash_result[..16]);
    let num = u128::from_be_bytes(bytes);

    // u128数値をbase62文字列にエンコード
    let encoded = base62::encode(num);

    // 指定された文字数を切り出して返す
    encoded.chars().take(length).collect()
}

/// ユーザー情報、IP、デバイス情報から日替わりIDと永続ハッシュを生成します。
pub fn generate_identity_hashes(
    user_identifier: &str, // ユーザーを永続的に識別する情報 (例: email)
    ip_address: &str,
    device_info: &str, // User-Agent やブラウザフィンガープリント
) -> IdentityHashes {
    // --- 1. 永続ハッシュの生成 (HMACを使用) ---
    // BANに使われる、時間で変化しないハッシュ。専用のソルト（ペッパー）を使用します。
    let permanent_salt =
        env::var("PERMANENT_HASH_SALT").expect("PERMANENT_HASH_SALT must be set in .env file");
    let permanent_salt_bytes = permanent_salt.as_bytes();

    let permanent_user_hash = create_hmac_hash(permanent_salt_bytes, user_identifier);
    let permanent_ip_hash = create_hmac_hash(permanent_salt_bytes, ip_address);
    let permanent_device_hash = create_hmac_hash(permanent_salt_bytes, device_info);

    // --- 2. 日替わり表示IDの生成 (HMACを使用) ---
    // このIDは毎日変わります。

    let daily_salt = env::var("USER_ID_SALT").expect("USER_ID_SALT must be set in .env file");
    let daily_salt_bytes = daily_salt.as_bytes();
    // JST（日本標準時）の現在時刻を取得し、その日付を文字列に変換します。
    let today = Utc::now()
        .with_timezone(&Tokyo)
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();

    // HMACのメッセージ部分を作成します (キーとしてソルトを使うため、データにソルトを含める必要はありません)
    let daily_user_data = format!("{}-{}", user_identifier, &today);
    let daily_ip_data = format!("{}-{}", ip_address, &today);
    let daily_device_data = format!("{}-{}", device_info, &today);

    let display_id_user_part = create_base62_id_part(daily_salt_bytes, &daily_user_data, 8);
    let display_id_ip_part = create_base62_id_part(daily_salt_bytes, &daily_ip_data, 4);
    let display_id_device_part = create_base62_id_part(daily_salt_bytes, &daily_device_data, 4);

    let display_user_id = format!(
        "{}-{}-{}",
        &display_id_user_part, &display_id_ip_part, &display_id_device_part
    );

    IdentityHashes {
        display_user_id,
        display_id_user_part,
        display_id_ip_part,
        display_id_device_part,
        permanent_user_hash,
        permanent_ip_hash,
        permanent_device_hash,
    }
}
