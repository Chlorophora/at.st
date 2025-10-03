use std::fmt;
use std::future::{ready, Ready};
use std::rc::Rc;

use actix_web::{
    dev::{self, Service, ServiceRequest, ServiceResponse, Transform},
    web, Error, HttpMessage,
};
use futures_util::future::LocalBoxFuture;
use log;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type, Serialize, Deserialize)]
#[sqlx(type_name = "user_role", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Moderator,
    User,
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Moderator => write!(f, "moderator"),
            Role::User => write!(f, "user"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub user_id: i32,
    pub role: Role,
    pub level: i32,
}

/// Middleware factory for authentication.
pub struct Auth;

impl<S, B> Transform<S, ServiceRequest> for Auth
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = AuthMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(AuthMiddleware {
            service: Rc::new(service),
        }))
    }
}

/// The actual authentication middleware.
pub struct AuthMiddleware<S> {
    service: Rc<S>,
}

impl<S, B> Service<ServiceRequest> for AuthMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    dev::forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Clone the necessary parts from the request before it's consumed by the next service.
        let pool_option = req.app_data::<web::Data<PgPool>>().cloned();
        let session_cookie = req.cookie("session_token");
        let session_token_option = session_cookie.as_ref().map(|c| c.value().to_string());
        let srv = self.service.clone();

        Box::pin(async move {
            // The request needs to be mutable to attach extensions, so we make it mutable here.
            let req = req; // この行はRustの最近のバージョンでは不要な場合がありますが、actix-webのバージョンによっては必要です。警告が出ているなら削除して問題ありません。
                           // ログクレートを使用した構造化されたログ出力
            log::trace!("[AuthMiddleware] Path: {}", req.path());

            if let (Some(pool), Some(session_token)) = (pool_option, session_token_option) {
                log::debug!("[AuthMiddleware] Found session_token, querying database...");
                // Check the database for a valid, non-expired session.
                // u.roleを"role: Role"としてマッピングすることで、sqlxがDBのenumをRustのenumに直接変換します。
                let user_session = sqlx::query!(
                    r#"
                    SELECT s.user_id, u.role as "role: Role", u.level
                    FROM sessions s
                    JOIN users u ON s.user_id = u.id
                    WHERE s.session_token = $1 AND s.expires_at > NOW()
                    "#,
                    session_token
                )
                .fetch_optional(pool.get_ref())
                .await;

                match user_session {
                    Ok(Some(session)) => {
                        let user = AuthenticatedUser {
                            user_id: session.user_id,
                            role: session.role,
                            level: session.level,
                        };
                        log::info!("[AuthMiddleware] ✅ Auth successful for user_id: {}. Role: {:?}, Level: {}", user.user_id, user.role, user.level);
                        req.extensions_mut().insert(user);
                    }
                    Ok(None) => {
                        log::info!("[AuthMiddleware] ❌ Session token found, but no valid session in database.");
                    }
                    Err(e) => {
                        log::error!(
                            "[AuthMiddleware] ❌ Database query failed for session token: {}",
                            e
                        );
                    }
                }
            } else {
                log::trace!("[AuthMiddleware] ⚠️ 'session_token' cookie NOT found in request.");
            }

            // Call the next service in the chain.
            srv.call(req).await
        })
    }
}
