use actix_web::{error::Error as ActixError, http};
use actix_web::{http::StatusCode, HttpResponse, ResponseError};
use actix_web::error::PayloadError;
use derive_more::Display;
use log;
use serde_json;
use sqlx::Error as SqlxError;
use validator::ValidationErrors;

#[derive(Debug, Display)]
pub enum ServiceError {
    #[display(fmt = "Internal Server Error: {}", _0)]
    InternalServerError(String),

    #[allow(dead_code)] // このバリアントは将来使用する可能性があるため警告を抑制
    #[display(fmt = "BadRequest: {}", _0)]
    BadRequest(String),

    #[display(fmt = "NotFound: {}", _0)]
    NotFound(String),

    #[display(fmt = "Unauthorized")]
    Unauthorized,

    #[display(fmt = "Forbidden: {}", _0)]
    Forbidden(String),

    #[display(fmt = "Too Many Requests: {}", _0)]
    TooManyRequests(String),

    #[display(fmt = "Input validation failed")]
    ValidationFailed(ValidationErrors),
    // 他のエラーケース
}

impl ResponseError for ServiceError {
    fn status_code(&self) -> StatusCode {
        match *self {
            ServiceError::InternalServerError(_) => StatusCode::INTERNAL_SERVER_ERROR,
            ServiceError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServiceError::NotFound(_) => StatusCode::NOT_FOUND,
            ServiceError::Unauthorized => StatusCode::UNAUTHORIZED,
            ServiceError::Forbidden(_) => StatusCode::FORBIDDEN,
            ServiceError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            ServiceError::ValidationFailed(_) => StatusCode::BAD_REQUEST,
        }
    }

    fn error_response(&self) -> HttpResponse {
        let status = self.status_code();

        // デバッグビルドの場合、InternalServerErrorの詳細をレスポンスに含める
        // これにより、フロントエンド開発中に500エラーの原因を特定しやすくなる
        #[cfg(debug_assertions)]
        if let ServiceError::InternalServerError(details) = self {
            return HttpResponse::build(status).json(serde_json::json!({
                "error": self.to_string(),
                "details": details,
            }));
        }

        match self {
            ServiceError::ValidationFailed(val_errors) => {
                let mut details = serde_json::Map::new();
                for (field, field_errors) in val_errors.field_errors() {
                    let messages: Vec<String> = field_errors
                        .iter()
                        .filter_map(|e| e.message.as_ref().map(|cow_str| cow_str.to_string()))
                        .collect();
                    if !messages.is_empty() {
                        details.insert(field.to_string(), serde_json::Value::from(messages));
                    }
                }
                HttpResponse::build(status).json(serde_json::json!({
                    "error": self.to_string(), // "Input validation failed"
                    "details": details
                }))
            }
            _ => HttpResponse::build(status).json(serde_json::json!({
                "error": self.to_string()
            })),
        }
    }
}

// sqlx::Error から ServiceError への変換
impl From<SqlxError> for ServiceError {
    fn from(error: SqlxError) -> ServiceError {
        log::error!("Database error: {:?}", error); // ログには詳細なエラーを残す
        match error {
            SqlxError::RowNotFound => ServiceError::NotFound("Resource not found".to_string()),
            // 他のsqlxエラーに対する具体的なマッピングも可能
            _ => ServiceError::InternalServerError(
                "An unexpected database error occurred.".to_string(),
            ),
        }
    }
}

// validator::ValidationErrors から ServiceError への変換
impl From<ValidationErrors> for ServiceError {
    fn from(errors: ValidationErrors) -> ServiceError {
        ServiceError::ValidationFailed(errors)
    }
}

// actix_web::cookie::CookieBuilder::finish が返す http::Error を変換できるようにする
impl From<http::Error> for ServiceError {
    fn from(err: http::Error) -> Self {
        ServiceError::InternalServerError(format!("Cookie setting error: {}", err))
        // この行は変更なしでOK
    }
}

// actix_web::HttpResponse::add_cookie が返す actix_web::error::Error を変換できるようにする
impl From<ActixError> for ServiceError {
    fn from(error: ActixError) -> ServiceError {
        ServiceError::InternalServerError(error.to_string())
    }
}

// actix_web::web::Payload からのストリームエラーを変換できるようにする
impl From<PayloadError> for ServiceError {
    fn from(error: PayloadError) -> ServiceError {
        log::error!("Payload error: {:?}", error);
        ServiceError::BadRequest(format!("Failed to read request body: {}", error))
    }
}

// serde_json::Error から ServiceError への変換
impl From<serde_json::Error> for ServiceError {
    fn from(error: serde_json::Error) -> ServiceError {
        // JSONのパースエラーは予期せぬ挙動の可能性が高いためInternalServerErrorとして扱う
        ServiceError::InternalServerError(format!(
            "JSON serialization/deserialization error: {}",
            error
        ))
    }
}
