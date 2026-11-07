use crate::util::*;
use crate::S;
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, SaltString},
    Argon2, PasswordHasher, PasswordVerifier,
};
use axum::{
    extract::{FromRequestParts, State},
    http::{header, request::Parts, HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::OnceLock;
use totp_rs::{Algorithm, Secret, TOTP};

pub const COOKIE: &str = "vx_session";
const SESSION_TTL: i64 = 30 * 24 * 3600;
const MAX_FAILS: u32 = 5;
const LOCK_SECS: i64 = 15 * 60;

#[derive(Clone)]
pub struct Auth {
    pub uid: i64,
    pub username: String,
    pub is_admin: bool,
    pub session: Option<String>,
}

pub fn hash_password(pw: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(pw.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .to_string())
}

pub fn verify_password(hash: &str, pw: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| Argon2::default().verify_password(pw.as_bytes(), &h).is_ok())
        .unwrap_or(false)
}

fn dummy_hash() -> &'static str {
    static H: OnceLock<String> = OnceLock::new();
    H.get_or_init(|| hash_password("timing-equaliser").unwrap())
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers.get_all(header::COOKIE).iter().find_map(|v| {
        v.to_str().ok()?.split(';').find_map(|kv| {
            let (k, v) = kv.trim().split_once('=')?;
            (k == name).then(|| v.to_string())
        })
    })
}

fn bearer(headers: &HeaderMap) -> Option<String> {
    let v = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    v.strip_prefix("Bearer ").map(|s| s.trim().to_string())
}

impl FromRequestParts<S> for Auth {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, s: &S) -> Result<Self, AppError> {
        let t = now();
        if let Some(tok) = bearer(&parts.headers) {
            let h = sha256_hex(&tok);
            let db = s.db.lock().unwrap();
            let row: Option<(i64, String, bool)> = db
                .query_row(
                    "SELECT u.id, u.username, u.is_admin FROM app_tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = ?1",
                    [&h],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            let Some((uid, username, is_admin)) = row else {
                return Err(AppError::unauth("Invalid token"));
            };
            db.execute(
                "UPDATE app_tokens SET last_used = ?1 WHERE token_hash = ?2 AND last_used < ?1 - 60",
                params![t, h],
            )?;
            return Ok(Auth { uid, username, is_admin, session: None });
        }

        if let Some(tok) = cookie_value(&parts.headers, COOKIE) {
            // CSRF defence: state-changing cookie requests must carry a custom header,
            // which cross-site forms cannot set. (Cookie is also SameSite=Strict.)
            if !matches!(parts.method, Method::GET | Method::HEAD) && !parts.headers.contains_key("x-vaultix") {
                return Err(AppError::forbidden("Missing request header"));
            }
            let h = sha256_hex(&tok);
            let db = s.db.lock().unwrap();
            let row: Option<(i64, String, bool)> = db
                .query_row(
                    "SELECT u.id, u.username, u.is_admin FROM sessions s JOIN users u ON u.id = s.user_id WHERE s.token_hash = ?1 AND s.expires_at > ?2",
                    params![h, t],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .optional()?;
            if let Some((uid, username, is_admin)) = row {
                db.execute(
                    "UPDATE sessions SET last_seen = ?1, expires_at = ?2 WHERE token_hash = ?3 AND last_seen < ?1 - 300",
                    params![t, t + SESSION_TTL, h],
                )?;
                return Ok(Auth { uid, username, is_admin, session: Some(h) });
            }
        }
        Err(AppError::unauth("Not signed in"))
    }
}

/* ---------- login ---------- */

#[derive(Deserialize)]
pub struct LoginReq {
    username: String,
    password: String,
    code: Option<String>,
    /// For /api/auth/token: device name.
    name: Option<String>,
}

struct UserRow {
    id: i64,
    hash: String,
    totp_enabled: bool,
    secret: Option<String>,
    last_step: i64,
}

fn build_totp(secret_b32: &str, account: &str) -> Option<TOTP> {
    let bytes = Secret::Encoded(secret_b32.to_string()).to_bytes().ok()?;
    TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes, Some("Vaultix".into()), account.to_string()).ok()
}

/// Returns the matched time step if `code` is valid and newer than `last_step` (replay protection).
fn check_totp(secret_b32: &str, account: &str, code: &str, last_step: i64) -> Option<i64> {
    let totp = build_totp(secret_b32, account)?;
    let t = now();
    for off in [-30i64, 0, 30] {
        let ts = (t + off) as u64;
        if totp.generate(ts) == code {
            let step = (ts / 30) as i64;
            if step > last_step {
                return Some(step);
            }
        }
    }
    None
}

fn normalize_code(c: &str) -> String {
    c.chars().filter(|c| c.is_ascii_alphanumeric()).collect::<String>().to_lowercase()
}

async fn verify_login(s: &S, req: &LoginReq) -> R<(i64, String)> {
    let username = req.username.trim().to_lowercase();
    let t = now();
    {
        let mut fails = s.login_fails.lock().unwrap();
        if let Some((n, first)) = fails.get(&username).copied() {
            if t - first > LOCK_SECS {
                fails.remove(&username);
            } else if n >= MAX_FAILS {
                return Err(AppError::new(
                    StatusCode::TOO_MANY_REQUESTS,
                    "Too many failed attempts. Try again in 15 minutes.",
                ));
            }
        }
    }
    let row: Option<UserRow> = s
        .db
        .lock()
        .unwrap()
        .query_row(
            "SELECT id, password_hash, totp_enabled, totp_secret, totp_last_step FROM users WHERE username = ?1",
            [&username],
            |r| {
                Ok(UserRow { id: r.get(0)?, hash: r.get(1)?, totp_enabled: r.get(2)?, secret: r.get(3)?, last_step: r.get(4)? })
            },
        )
        .optional()?;

    let fail = |msg: &str| {
        let mut fails = s.login_fails.lock().unwrap();
        let e = fails.entry(username.clone()).or_insert((0, t));
        e.0 += 1;
        AppError::unauth(msg)
    };

    let hash = row.as_ref().map(|r| r.hash.clone()).unwrap_or_else(|| dummy_hash().to_string());
    let pw = req.password.clone();
    let ok = tokio::task::spawn_blocking(move || verify_password(&hash, &pw)).await?;
    let Some(user) = row.filter(|_| ok) else {
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        return Err(fail("Invalid username or password"));
    };

    if user.totp_enabled {
        let code = req.code.as_deref().map(normalize_code).unwrap_or_default();
        if code.is_empty() {
            let mut e = AppError::unauth("Enter your 2FA code");
            e.totp_required = true;
            return Err(e);
        }
        let secret = user.secret.clone().unwrap_or_default();
        let db = s.db.lock().unwrap();
        if code.len() == 6 && code.chars().all(|c| c.is_ascii_digit()) {
            match check_totp(&secret, &username, &code, user.last_step) {
                Some(step) => {
                    db.execute("UPDATE users SET totp_last_step = ?1 WHERE id = ?2", params![step, user.id])?;
                }
                None => {
                    drop(db);
                    let mut e = fail("Invalid 2FA code");
                    e.totp_required = true;
                    return Err(e);
                }
            }
        } else {
            let n = db.execute(
                "DELETE FROM recovery_codes WHERE user_id = ?1 AND code_hash = ?2",
                params![user.id, sha256_hex(&code)],
            )?;
            if n == 0 {
                drop(db);
                let mut e = fail("Invalid 2FA or recovery code");
                e.totp_required = true;
                return Err(e);
            }
        }
    }
    s.login_fails.lock().unwrap().remove(&username);
    Ok((user.id, username))
}

pub fn is_https(headers: &HeaderMap, s: &S) -> bool {
    match s.cookie_secure.as_str() {
        "true" => true,
        "false" => false,
        _ => headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()) == Some("https"),
    }
}

pub async fn login(State(s): State<S>, headers: HeaderMap, Json(req): Json<LoginReq>) -> R<Response> {
    let (uid, _) = verify_login(&s, &req).await?;
    start_session(&s, &headers, uid)
}

/// Create a browser session for `uid` and return the response that sets its cookie.
pub fn start_session(s: &S, headers: &HeaderMap, uid: i64) -> R<Response> {
    let token = random_hex(32);
    let t = now();
    let ua = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).unwrap_or("").chars().take(200).collect::<String>();
    s.db.lock().unwrap().execute(
        "INSERT INTO sessions (token_hash, user_id, created_at, last_seen, expires_at, user_agent) VALUES (?1, ?2, ?3, ?3, ?4, ?5)",
        params![sha256_hex(&token), uid, t, t + SESSION_TTL, ua],
    )?;
    let secure = if is_https(headers, s) { "; Secure" } else { "" };
    let cookie = format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={SESSION_TTL}{secure}");
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

/// Desktop client login: same credentials + 2FA, returns a long-lived app token.
pub async fn issue_token(State(s): State<S>, Json(req): Json<LoginReq>) -> R<Json<Value>> {
    let (uid, username) = verify_login(&s, &req).await?;
    let token = format!("vx_{}", random_hex(32));
    let name = req.name.clone().unwrap_or_else(|| "Desktop".into()).chars().take(80).collect::<String>();
    s.db.lock().unwrap().execute(
        "INSERT INTO app_tokens (user_id, name, token_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![uid, name, sha256_hex(&token), now()],
    )?;
    Ok(Json(json!({ "token": token, "username": username })))
}

pub async fn logout(State(s): State<S>, auth: Auth) -> R<Response> {
    if let Some(h) = auth.session {
        s.db.lock().unwrap().execute("DELETE FROM sessions WHERE token_hash = ?1", [h])?;
    }
    let cookie = format!("{COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0");
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

pub async fn me(State(s): State<S>, auth: Auth) -> R<Json<Value>> {
    let totp: bool = s.db.lock().unwrap().query_row("SELECT totp_enabled FROM users WHERE id = ?1", [auth.uid], |r| r.get(0))?;
    let recovery_left: i64 = s.db.lock().unwrap().query_row("SELECT COUNT(*) FROM recovery_codes WHERE user_id = ?1", [auth.uid], |r| r.get(0))?;
    Ok(Json(json!({
        "username": auth.username,
        "is_admin": auth.is_admin,
        "totp_enabled": totp,
        "recovery_codes_left": recovery_left,
    })))
}

/* ---------- password ---------- */

#[derive(Deserialize)]
pub struct PwReq {
    current: String,
    new: String,
}

async fn check_current_pw(s: &S, uid: i64, pw: &str) -> R<()> {
    let hash: String = s.db.lock().unwrap().query_row("SELECT password_hash FROM users WHERE id = ?1", [uid], |r| r.get(0))?;
    let pw = pw.to_string();
    if !tokio::task::spawn_blocking(move || verify_password(&hash, &pw)).await? {
        return Err(AppError::forbidden("Current password is incorrect"));
    }
    Ok(())
}

pub async fn change_password(State(s): State<S>, auth: Auth, Json(req): Json<PwReq>) -> R<Json<Value>> {
    if req.new.len() < 8 {
        return Err(AppError::bad("Password must be at least 8 characters"));
    }
    check_current_pw(&s, auth.uid, &req.current).await?;
    let h = tokio::task::spawn_blocking(move || hash_password(&req.new)).await??;
    let db = s.db.lock().unwrap();
    db.execute("UPDATE users SET password_hash = ?1 WHERE id = ?2", params![h, auth.uid])?;
    // Sign out every other browser session.
    db.execute(
        "DELETE FROM sessions WHERE user_id = ?1 AND token_hash != ?2",
        params![auth.uid, auth.session.unwrap_or_default()],
    )?;
    Ok(Json(json!({ "ok": true })))
}

/* ---------- 2FA ---------- */

pub async fn totp_setup(State(s): State<S>, auth: Auth) -> R<Json<Value>> {
    let db = s.db.lock().unwrap();
    let enabled: bool = db.query_row("SELECT totp_enabled FROM users WHERE id = ?1", [auth.uid], |r| r.get(0))?;
    if enabled {
        return Err(AppError::bad("2FA is already enabled"));
    }
    let secret = match Secret::generate_secret().to_encoded() {
        Secret::Encoded(s) => s,
        Secret::Raw(_) => unreachable!(),
    };
    db.execute("UPDATE users SET totp_secret = ?1 WHERE id = ?2", params![secret, auth.uid])?;
    let totp = build_totp(&secret, &auth.username).ok_or_else(|| AppError::internal("totp"))?;
    let qr = totp.get_qr_base64().map_err(AppError::internal)?;
    Ok(Json(json!({
        "secret": secret,
        "url": totp.get_url(),
        "qr": format!("data:image/png;base64,{qr}"),
    })))
}

#[derive(Deserialize)]
pub struct CodeReq {
    code: String,
}

pub async fn totp_enable(State(s): State<S>, auth: Auth, Json(req): Json<CodeReq>) -> R<Json<Value>> {
    let db = s.db.lock().unwrap();
    let (secret, enabled): (Option<String>, bool) =
        db.query_row("SELECT totp_secret, totp_enabled FROM users WHERE id = ?1", [auth.uid], |r| Ok((r.get(0)?, r.get(1)?)))?;
    if enabled {
        return Err(AppError::bad("2FA is already enabled"));
    }
    let secret = secret.ok_or_else(|| AppError::bad("Start 2FA setup first"))?;
    let step = check_totp(&secret, &auth.username, &normalize_code(&req.code), 0)
        .ok_or_else(|| AppError::bad("That code didn't match. Check your authenticator's clock and try again."))?;
    db.execute("UPDATE users SET totp_enabled = 1, totp_last_step = ?1 WHERE id = ?2", params![step, auth.uid])?;
    db.execute("DELETE FROM recovery_codes WHERE user_id = ?1", [auth.uid])?;
    let mut codes = Vec::new();
    for _ in 0..10 {
        let c = random_hex(5); // 10 hex chars
        db.execute("INSERT INTO recovery_codes (user_id, code_hash) VALUES (?1, ?2)", params![auth.uid, sha256_hex(&c)])?;
        codes.push(format!("{}-{}", &c[..5], &c[5..]));
    }
    Ok(Json(json!({ "ok": true, "recovery_codes": codes })))
}

#[derive(Deserialize)]
pub struct PwOnly {
    password: String,
}

pub async fn totp_disable(State(s): State<S>, auth: Auth, Json(req): Json<PwOnly>) -> R<Json<Value>> {
    check_current_pw(&s, auth.uid, &req.password).await?;
    let db = s.db.lock().unwrap();
    db.execute("UPDATE users SET totp_enabled = 0, totp_secret = NULL WHERE id = ?1", [auth.uid])?;
    db.execute("DELETE FROM recovery_codes WHERE user_id = ?1", [auth.uid])?;
    Ok(Json(json!({ "ok": true })))
}

/* ---------- devices: app tokens & sessions ---------- */

pub async fn list_devices(State(s): State<S>, auth: Auth) -> R<Json<Value>> {
    let db = s.db.lock().unwrap();
    let mut st = db.prepare("SELECT id, name, created_at, last_used FROM app_tokens WHERE user_id = ?1 ORDER BY created_at DESC")?;
    let tokens: Vec<Value> = st
        .query_map([auth.uid], |r| {
            Ok(json!({ "id": r.get::<_, i64>(0)?, "name": r.get::<_, String>(1)?, "created_at": r.get::<_, i64>(2)?, "last_used": r.get::<_, i64>(3)? }))
        })?
        .collect::<Result<_, _>>()?;
    let mut st = db.prepare(
        "SELECT token_hash, user_agent, created_at, last_seen FROM sessions WHERE user_id = ?1 AND expires_at > ?2 ORDER BY last_seen DESC",
    )?;
    let cur = auth.session.clone().unwrap_or_default();
    let sessions: Vec<Value> = st
        .query_map(params![auth.uid, now()], |r| {
            let h: String = r.get(0)?;
            Ok(json!({ "id": h[..16].to_string(), "current": h == cur, "user_agent": r.get::<_, Option<String>>(1)?, "created_at": r.get::<_, i64>(2)?, "last_seen": r.get::<_, i64>(3)? }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({ "tokens": tokens, "sessions": sessions })))
}

#[derive(Deserialize)]
pub struct RevokeReq {
    token_id: Option<i64>,
    session_id: Option<String>,
}

pub async fn revoke_device(State(s): State<S>, auth: Auth, Json(req): Json<RevokeReq>) -> R<Json<Value>> {
    let db = s.db.lock().unwrap();
    if let Some(id) = req.token_id {
        db.execute("DELETE FROM app_tokens WHERE id = ?1 AND user_id = ?2", params![id, auth.uid])?;
    }
    if let Some(sid) = req.session_id.filter(|s| s.len() == 16 && s.chars().all(|c| c.is_ascii_hexdigit())) {
        db.execute(
            "DELETE FROM sessions WHERE user_id = ?1 AND substr(token_hash, 1, 16) = ?2",
            params![auth.uid, sid],
        )?;
    }
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct NameReq {
    name: String,
}

/// Create an app password from a signed-in browser session (for WebDAV apps, sync clients).
pub async fn create_device(State(s): State<S>, auth: Auth, Json(req): Json<NameReq>) -> R<Json<Value>> {
    let name = req.name.trim().chars().take(80).collect::<String>();
    if name.is_empty() {
        return Err(AppError::bad("Name required"));
    }
    let token = format!("vx_{}", random_hex(32));
    s.db.lock().unwrap().execute(
        "INSERT INTO app_tokens (user_id, name, token_hash, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![auth.uid, name, sha256_hex(&token), now()],
    )?;
    Ok(Json(json!({ "token": token })))
}
