//! WebDAV access to each user's files.
//!
//! Mounted at `/dav/` plus Nextcloud-compatible aliases so existing clients can be
//! pointed at Vaultix unchanged:
//!   /remote.php/dav/files/<user>/...
//!   /remote.php/webdav/...
//!
//! Auth is HTTP Basic: username + app password (created in Settings → Devices).
//! The account password is also accepted when 2FA is off.

use crate::util::*;
use crate::S;
use axum::{
    body::Body,
    extract::{Request, State},
    http::{header, HeaderValue, Method, StatusCode},
    response::{IntoResponse, Response},
};
use base64::Engine;
use dav_server::{fakels::FakeLs, localfs::LocalFs, DavHandler};
use rusqlite::{params, OptionalExtension};

const CACHE_SECS: i64 = 300;

fn challenge() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [(header::WWW_AUTHENTICATE, HeaderValue::from_static("Basic realm=\"Vaultix\", charset=\"UTF-8\""))],
        "Authentication required",
    )
        .into_response()
}

/// Validate Basic credentials. Returns (uid, username).
async fn basic_auth(s: &S, authz: Option<String>) -> Option<(i64, String)> {
    let v = authz?;
    let raw = base64::engine::general_purpose::STANDARD.decode(v.strip_prefix("Basic ")?.trim()).ok()?;
    let raw = String::from_utf8(raw).ok()?;
    let (user, pass) = raw.split_once(':')?;
    let user = user.trim().to_lowercase();
    let key = sha256_hex(&format!("{user}\0{pass}"));
    let t = now();
    if let Some((uid, name, exp)) = s.dav_cache.lock().unwrap().get(&key).cloned() {
        if exp > t {
            return Some((uid, name));
        }
    }

    let found: Option<(i64, String)> = if pass.starts_with("vx_") {
        let db = s.db.lock().unwrap();
        let r = db
            .query_row(
                "SELECT u.id, u.username FROM app_tokens t JOIN users u ON u.id = t.user_id WHERE t.token_hash = ?1 AND u.username = ?2",
                params![sha256_hex(pass), user],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .ok()?;
        if r.is_some() {
            let _ = db.execute("UPDATE app_tokens SET last_used = ?1 WHERE token_hash = ?2", params![t, sha256_hex(pass)]);
        }
        r
    } else {
        // Account password only when 2FA is off; otherwise an app password is required.
        let row: Option<(i64, String, bool)> = s
            .db
            .lock()
            .unwrap()
            .query_row("SELECT id, password_hash, totp_enabled FROM users WHERE username = ?1", [&user], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .optional()
            .ok()?;
        match row {
            Some((uid, hash, false)) => {
                let pass = pass.to_string();
                let ok = tokio::task::spawn_blocking(move || crate::auth::verify_password(&hash, &pass)).await.ok()?;
                ok.then(|| (uid, user.clone()))
            }
            _ => None,
        }
    };

    match found {
        Some((uid, name)) => {
            let mut c = s.dav_cache.lock().unwrap();
            if c.len() > 1000 {
                c.retain(|_, v| v.2 > t);
            }
            c.insert(key, (uid, name.clone(), t + CACHE_SECS));
            Some((uid, name))
        }
        None => {
            tokio::time::sleep(std::time::Duration::from_millis(400)).await;
            None
        }
    }
}

pub async fn handle(State(s): State<S>, req: Request) -> Response {
    let path = req.uri().path().to_string();
    // Work out the URL prefix this request was addressed to.
    let (prefix, url_user): (String, Option<String>) = if let Some(rest) = path.strip_prefix("/remote.php/dav/files/") {
        let user = rest.split('/').next().unwrap_or("").to_string();
        (format!("/remote.php/dav/files/{user}"), Some(percent_encoding::percent_decode_str(&user).decode_utf8_lossy().to_lowercase()))
    } else if path.starts_with("/remote.php/webdav") {
        ("/remote.php/webdav".into(), None)
    } else {
        ("/dav".into(), None)
    };

    // Let clients discover the server without credentials.
    if req.method() == Method::OPTIONS && req.headers().get(header::AUTHORIZATION).is_none() {
        return (
            StatusCode::OK,
            [("dav", "1, 2"), ("allow", "OPTIONS, GET, HEAD, PUT, DELETE, PROPFIND, PROPPATCH, MKCOL, COPY, MOVE, LOCK, UNLOCK")],
        )
            .into_response();
    }

    let authz = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).map(String::from);
    let Some((uid, username)) = basic_auth(&s, authz).await else {
        return challenge();
    };
    if url_user.is_some_and(|u| u != username) {
        return (StatusCode::FORBIDDEN, "Wrong user in URL").into_response();
    }

    let root = s.user_root(&username);
    let _ = tokio::fs::create_dir_all(&root).await;
    let mutating = !matches!(req.method().as_str(), "GET" | "HEAD" | "OPTIONS" | "PROPFIND" | "LOCK" | "UNLOCK");

    let dav = DavHandler::builder()
        .filesystem(LocalFs::new(&root, false, false, true))
        .locksystem(FakeLs::new())
        .strip_prefix(prefix)
        .hide_symlinks(true)
        .build_handler();
    let res = dav.handle(req).await;
    if mutating && res.status().is_success() {
        s.bump(uid);
    }
    res.map(Body::new)
}
