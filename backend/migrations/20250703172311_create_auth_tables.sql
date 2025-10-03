-- Add up migration script here
-- ユーザー情報を格納するテーブル
CREATE TABLE users (
    id SERIAL PRIMARY KEY,
    email VARCHAR(255) UNIQUE NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ワンタイムパスワード(OTP)を一時的に保存するテーブル
CREATE TABLE otp_tokens (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    otp_code_hash VARCHAR(255) NOT NULL, -- OTPは必ずハッシュ化して保存します
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    used_at TIMESTAMPTZ -- OTPが使用された日時。一度使ったOTPは再利用不可にする
);

-- セッショントークンを保存するテーブル
CREATE TABLE sessions (
    id SERIAL PRIMARY KEY,
    user_id INTEGER NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    session_token_hash VARCHAR(255) UNIQUE NOT NULL, -- ハッシュ値にはVARCHAR(255)がより適切です
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
