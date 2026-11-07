//! First-run setup and user management from the web app (no terminal needed).

use crate::auth::{self, Auth};
use crate::util::*;
use crate::S;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};

fn user_count(s: &S) -> R<i64> {
    Ok(s.db.lock().unwrap().query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?)
}

/// `GET /api/setup` — whether this server still needs its first (admin) account.
pub async fn setup_status(State(s): State<S>) -> R<Json<Value>> {
    Ok(Json(json!({ "needed": user_count(&s)? == 0 })))
}

#[derive(Deserialize)]
pub struct SetupReq {
    username: String,
    password: String,
}

/// `POST /api/setup` — create the first admin and sign them in. Only works while no users exist.
pub async fn setup(State(s): State<S>, headers: HeaderMap, Json(req): Json<SetupReq>) -> R<Response> {
    if !headers.contains_key("x-vaultix") {
        return Err(AppError::forbidden("Missing request header"));
    }
    // Serialize concurrent attempts: only the first one can win.
    static ONCE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
    let _g = ONCE.lock().await;
    if user_count(&s)? != 0 {
        return Err(AppError::forbidden("This server is already set up. Sign in instead."));
    }
    crate::create_user(&s, &req.username, &req.password, true).map_err(|e| AppError::bad(e.to_string()))?;
    let uid: i64 = s.db.lock().unwrap().query_row("SELECT id FROM users WHERE username = ?1", [req.username.trim().to_lowercase()], |r| r.get(0))?;
    tracing::info!("first-run setup: created admin '{}'", req.username.trim().to_lowercase());
    auth::start_session(&s, &headers, uid)
}

fn require_admin(a: &Auth) -> R<()> {
    if a.is_admin { Ok(()) } else { Err(AppError::forbidden("Only admins can manage users")) }
}

/// `GET /api/admin/users`
pub async fn list_users(State(s): State<S>, auth: Auth) -> R<Json<Value>> {
    require_admin(&auth)?;
    let db = s.db.lock().unwrap();
    let mut st = db.prepare(
        "SELECT u.username, u.is_admin, u.totp_enabled, u.created_at,
                (SELECT COUNT(*) FROM app_tokens t WHERE t.user_id = u.id),
                (SELECT MAX(last_seen) FROM sessions x WHERE x.user_id = u.id)
         FROM users u ORDER BY u.username",
    )?;
    let users = st
        .query_map([], |r| {
            Ok(json!({
                "username": r.get::<_, String>(0)?,
                "is_admin": r.get::<_, bool>(1)?,
                "totp_enabled": r.get::<_, bool>(2)?,
                "created_at": r.get::<_, i64>(3)?,
                "devices": r.get::<_, i64>(4)?,
                "last_seen": r.get::<_, Option<i64>>(5)?,
            }))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Json(json!({ "users": users })))
}

#[derive(Deserialize)]
pub struct CreateReq {
    username: String,
    password: String,
    #[serde(default)]
    is_admin: bool,
}

/// `POST /api/admin/users`
pub async fn create_user(State(s): State<S>, auth: Auth, Json(req): Json<CreateReq>) -> R<Json<Value>> {
    require_admin(&auth)?;
    let name = req.username.trim().to_lowercase();
    let exists: Option<i64> = s.db.lock().unwrap().query_row("SELECT id FROM users WHERE username = ?1", [&name], |r| r.get(0)).optional()?;
    if exists.is_some() {
        return Err(AppError::bad("That username is taken"));
    }
    crate::create_user(&s, &name, &req.password, req.is_admin).map_err(|e| AppError::bad(e.to_string()))?;
    tracing::info!("{} created user '{name}'", auth.username);
    Ok(Json(json!({ "ok": true, "username": name })))
}

#[derive(Deserialize)]
pub struct UpdateReq {
    username: String,
    password: Option<String>,
    is_admin: Option<bool>,
    #[serde(default)]
    reset_2fa: bool,
}

/// `POST /api/admin/users/update` — reset password / 2FA, change admin flag.
pub async fn update_user(State(s): State<S>, auth: Auth, Json(req): Json<UpdateReq>) -> R<Json<Value>> {
    require_admin(&auth)?;
    let name = req.username.trim().to_lowercase();
    let uid: i64 = s
        .db
        .lock()
        .unwrap()
        .query_row("SELECT id FROM users WHERE username = ?1", [&name], |r| r.get(0))
        .optional()?
        .ok_or_else(AppError::not_found)?;
    if let Some(pw) = &req.password {
        if pw.len() < 8 {
            return Err(AppError::bad("Password must be at least 8 characters"));
        }
        let h = auth::hash_password(pw).map_err(|e| AppError::bad(e.to_string()))?;
        let db = s.db.lock().unwrap();
        db.execute("UPDATE users SET password_hash = ?1 WHERE id = ?2", params![h, uid])?;
        // A reset password signs the user out of browsers (app passwords keep working).
        db.execute("DELETE FROM sessions WHERE user_id = ?1", [uid])?;
    }
    if req.reset_2fa {
        let db = s.db.lock().unwrap();
        db.execute("UPDATE users SET totp_enabled = 0, totp_secret = NULL WHERE id = ?1", [uid])?;
        db.execute("DELETE FROM recovery_codes WHERE user_id = ?1", [uid])?;
    }
    if let Some(adm) = req.is_admin {
        if !adm && uid == auth.uid {
            return Err(AppError::bad("You can't remove your own admin rights"));
        }
        s.db.lock().unwrap().execute("UPDATE users SET is_admin = ?1 WHERE id = ?2", params![adm, uid])?;
    }
    tracing::info!("{} updated user '{name}'", auth.username);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DeleteReq {
    username: String,
}

/// `POST /api/admin/users/delete` — removes the account; files stay on disk under data/users/<name>.
pub async fn delete_user(State(s): State<S>, auth: Auth, Json(req): Json<DeleteReq>) -> R<Json<Value>> {
    require_admin(&auth)?;
    let name = req.username.trim().to_lowercase();
    if name == auth.username {
        return Err(AppError::bad("You can't delete your own account"));
    }
    let n = s.db.lock().unwrap().execute("DELETE FROM users WHERE username = ?1", [&name])?;
    if n == 0 {
        return Err(AppError::not_found());
    }
    tracing::info!("{} deleted user '{name}' (files kept)", auth.username);
    Ok(Json(json!({ "ok": true, "files_kept": true })))
}
