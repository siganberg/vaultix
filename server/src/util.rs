use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use rand::RngCore;
use serde_json::json;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub msg: String,
    pub totp_required: bool,
}

pub type R<T> = Result<T, AppError>;

impl AppError {
    pub fn new(status: StatusCode, msg: impl Into<String>) -> Self {
        Self { status, msg: msg.into(), totp_required: false }
    }
    pub fn bad(m: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, m)
    }
    pub fn unauth(m: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, m)
    }
    pub fn forbidden(m: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, m)
    }
    pub fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "Not found")
    }
    pub fn conflict(m: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, m)
    }
    pub fn internal(m: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, m)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if self.status.is_server_error() {
            tracing::error!("{}", self.msg);
        }
        let mut body = json!({ "error": self.msg });
        if self.totp_required {
            body["totp_required"] = json!(true);
        }
        (self.status, Json(body)).into_response()
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        use std::io::ErrorKind::*;
        match e.kind() {
            NotFound => Self::not_found(),
            AlreadyExists => Self::conflict("Already exists"),
            PermissionDenied => Self::forbidden("Permission denied"),
            _ => Self::internal(e.to_string()),
        }
    }
}
impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        Self::internal(format!("db: {e}"))
    }
}
impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::internal(e.to_string())
    }
}
impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        Self::internal(e.to_string())
    }
}

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64
}
pub fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

pub fn random_hex(bytes: usize) -> String {
    let mut b = vec![0u8; bytes];
    rand::thread_rng().fill_bytes(&mut b);
    hex::encode(b)
}

pub fn sha256_hex(s: &str) -> String {
    hex::encode(Sha256::digest(s.as_bytes()))
}

/// Normalise a user supplied relative path. Returns "" for the root.
/// Rejects any attempt to escape the user's root.
pub fn clean_rel(p: &str) -> R<String> {
    let mut parts = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => continue,
            ".." => return Err(AppError::bad("Invalid path")),
            c if c.contains('\0') || c.len() > 255 => return Err(AppError::bad("Invalid path")),
            c => parts.push(c),
        }
    }
    Ok(parts.join("/"))
}

pub fn mtime_ms(md: &std::fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn valid_username(u: &str) -> bool {
    !u.is_empty()
        && u.len() <= 32
        && u.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._-".contains(c))
        && !u.starts_with('.')
}

/// Query-string bool accepting `1`/`0`/`true`/`false`/`yes`/`on` (browsers send `download=1`).
pub fn de_flag<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    let s: Option<String> = serde::Deserialize::deserialize(d)?;
    Ok(matches!(s.as_deref().map(str::to_ascii_lowercase).as_deref(), Some("1" | "true" | "yes" | "on")))
}
