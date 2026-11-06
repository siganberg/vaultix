//! Public share links and file requests.
//!
//! - kind "link":   anyone with the URL can view (and optionally download) a file or folder.
//! - kind "upload": anyone with the URL can upload files into a folder, without seeing its contents.
//!
//! Links are 128-bit random tokens. Optional password (Argon2), expiry, download toggle,
//! size limit and a note. A password unlock is remembered by a keyed cookie that becomes
//! invalid when the password changes.

use crate::auth::Auth;
use crate::files;
use crate::util::*;
use crate::S;
use axum::{
    body::Body,
    extract::{Path as UrlPath, Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;

struct Share {
    id: i64,
    token: String,
    user_id: i64,
    owner: String,
    path: String,
    kind: String,
    password_hash: Option<String>,
    expires_at: Option<i64>,
    allow_download: bool,
    max_size: Option<i64>,
    note: Option<String>,
    views: i64,
    uploads: i64,
    created_at: i64,
}

const COLS: &str = "s.id, s.token, s.user_id, u.username, s.path, s.kind, s.password_hash, s.expires_at, s.allow_download, s.max_size, s.note, s.views, s.uploads, s.created_at";

fn map_row(r: &rusqlite::Row) -> rusqlite::Result<Share> {
    Ok(Share {
        id: r.get(0)?,
        token: r.get(1)?,
        user_id: r.get(2)?,
        owner: r.get(3)?,
        path: r.get(4)?,
        kind: r.get(5)?,
        password_hash: r.get(6)?,
        expires_at: r.get(7)?,
        allow_download: r.get(8)?,
        max_size: r.get(9)?,
        note: r.get(10)?,
        views: r.get(11)?,
        uploads: r.get(12)?,
        created_at: r.get(13)?,
    })
}

fn name_of(path: &str) -> String {
    if path.is_empty() { "All files".into() } else { path.rsplit('/').next().unwrap_or(path).to_string() }
}

fn target(s: &S, sh: &Share) -> PathBuf {
    s.user_root(&sh.owner).join(&sh.path)
}

/* =============== owner API =============== */

async fn owner_json(s: &S, sh: &Share) -> Value {
    let md = tokio::fs::metadata(target(s, sh)).await.ok();
    let recent: Vec<Value> = if sh.kind == "upload" {
        let db = s.db.lock().unwrap();
        let mut st = db.prepare("SELECT name, size, uploader, at FROM share_uploads WHERE share_id = ?1 ORDER BY at DESC LIMIT 5").unwrap();
        st.query_map([sh.id], |r| Ok(json!({ "name": r.get::<_, String>(0)?, "size": r.get::<_, i64>(1)?, "uploader": r.get::<_, Option<String>>(2)?, "at": r.get::<_, i64>(3)? })))
            .map(|it| it.filter_map(|x| x.ok()).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };
    json!({
        "id": sh.id,
        "token": sh.token,
        "url": format!("/s/{}", sh.token),
        "path": sh.path,
        "name": name_of(&sh.path),
        "kind": sh.kind,
        "is_dir": md.as_ref().map(|m| m.is_dir()).unwrap_or(false),
        "exists": md.is_some(),
        "has_password": sh.password_hash.is_some(),
        "expires_at": sh.expires_at,
        "expired": sh.expires_at.is_some_and(|e| e <= now()),
        "allow_download": sh.allow_download,
        "max_size": sh.max_size,
        "note": sh.note,
        "views": sh.views,
        "uploads": sh.uploads,
        "recent_uploads": recent,
        "created_at": sh.created_at,
    })
}

fn get_owned(s: &S, uid: i64, id: i64) -> R<Share> {
    s.db.lock()
        .unwrap()
        .query_row(&format!("SELECT {COLS} FROM shares s JOIN users u ON u.id = s.user_id WHERE s.id = ?1 AND s.user_id = ?2"), params![id, uid], map_row)
        .optional()?
        .ok_or_else(AppError::not_found)
}

pub async fn list(auth: Auth, State(s): State<S>) -> R<Json<Value>> {
    let shares: Vec<Share> = {
        let db = s.db.lock().unwrap();
        let mut st = db.prepare(&format!("SELECT {COLS} FROM shares s JOIN users u ON u.id = s.user_id WHERE s.user_id = ?1 ORDER BY s.created_at DESC"))?;
        let v = st.query_map([auth.uid], map_row)?.collect::<Result<_, _>>()?;
        v
    };
    let mut out = Vec::with_capacity(shares.len());
    for sh in &shares {
        out.push(owner_json(&s, sh).await);
    }
    Ok(Json(json!({ "shares": out })))
}

#[derive(Deserialize)]
pub struct ShareReq {
    id: Option<i64>,
    path: Option<String>,
    kind: Option<String>,
    /// None = keep (on update), "" = remove, otherwise set.
    password: Option<String>,
    expires_at: Option<i64>,
    #[serde(default = "yes")]
    allow_download: bool,
    max_size: Option<i64>,
    note: Option<String>,
}
fn yes() -> bool {
    true
}

async fn hash_opt(pw: Option<String>) -> R<Option<Option<String>>> {
    match pw {
        None => Ok(None),
        Some(p) if p.is_empty() => Ok(Some(None)),
        Some(p) => {
            let h = tokio::task::spawn_blocking(move || crate::auth::hash_password(&p)).await??;
            Ok(Some(Some(h)))
        }
    }
}

fn clean_note(n: Option<String>) -> Option<String> {
    n.map(|n| n.trim().chars().take(1000).collect::<String>()).filter(|n| !n.is_empty())
}

pub async fn create(auth: Auth, State(s): State<S>, Json(q): Json<ShareReq>) -> R<Json<Value>> {
    let rel = clean_rel(q.path.as_deref().unwrap_or(""))?;
    let kind = q.kind.as_deref().unwrap_or("link");
    if kind != "link" && kind != "upload" {
        return Err(AppError::bad("Unknown share type"));
    }
    let md = tokio::fs::metadata(s.user_root(&auth.username).join(&rel)).await?;
    if kind == "upload" && !md.is_dir() {
        return Err(AppError::bad("File requests need a folder"));
    }
    if kind == "link" && rel.is_empty() {
        return Err(AppError::bad("Choose a file or folder to share"));
    }
    let pw = hash_opt(q.password.clone()).await?.flatten();
    let token = random_hex(16);
    let id = {
        let db = s.db.lock().unwrap();
        db.execute(
            "INSERT INTO shares (token, user_id, path, kind, password_hash, expires_at, allow_download, max_size, note, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![token, auth.uid, rel, kind, pw, q.expires_at, q.allow_download, q.max_size.filter(|m| *m > 0), clean_note(q.note), now()],
        )?;
        db.last_insert_rowid()
    };
    let sh = get_owned(&s, auth.uid, id)?;
    Ok(Json(owner_json(&s, &sh).await))
}

pub async fn update(auth: Auth, State(s): State<S>, Json(q): Json<ShareReq>) -> R<Json<Value>> {
    let id = q.id.ok_or_else(|| AppError::bad("Missing id"))?;
    get_owned(&s, auth.uid, id)?;
    let pw = hash_opt(q.password.clone()).await?;
    {
        let db = s.db.lock().unwrap();
        db.execute(
            "UPDATE shares SET expires_at = ?1, allow_download = ?2, max_size = ?3, note = ?4 WHERE id = ?5 AND user_id = ?6",
            params![q.expires_at, q.allow_download, q.max_size.filter(|m| *m > 0), clean_note(q.note), id, auth.uid],
        )?;
        if let Some(pw) = pw {
            db.execute("UPDATE shares SET password_hash = ?1 WHERE id = ?2 AND user_id = ?3", params![pw, id, auth.uid])?;
        }
    }
    let sh = get_owned(&s, auth.uid, id)?;
    Ok(Json(owner_json(&s, &sh).await))
}

#[derive(Deserialize)]
pub struct IdReq {
    id: i64,
}

pub async fn revoke(auth: Auth, State(s): State<S>, Json(q): Json<IdReq>) -> R<Json<Value>> {
    s.db.lock().unwrap().execute("DELETE FROM shares WHERE id = ?1 AND user_id = ?2", params![q.id, auth.uid])?;
    Ok(Json(json!({ "ok": true })))
}

/* =============== public side =============== */

fn load(s: &S, token: &str) -> R<Share> {
    if token.len() != 32 || !token.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(AppError::new(StatusCode::NOT_FOUND, "This link doesn't exist or was removed."));
    }
    let sh = s
        .db
        .lock()
        .unwrap()
        .query_row(&format!("SELECT {COLS} FROM shares s JOIN users u ON u.id = s.user_id WHERE s.token = ?1"), [token], map_row)
        .optional()?
        .ok_or_else(|| AppError::new(StatusCode::NOT_FOUND, "This link doesn't exist or was removed."))?;
    if sh.expires_at.is_some_and(|e| e <= now()) {
        return Err(AppError::new(StatusCode::GONE, "This link has expired."));
    }
    Ok(sh)
}

fn unlock_cookie_name(sh: &Share) -> String {
    format!("vxs_{}", &sh.token[..12])
}

fn unlock_value(s: &S, sh: &Share) -> String {
    sha256_hex(&format!("{}:{}:{}", s.secret, sh.token, sh.password_hash.as_deref().unwrap_or("")))
}

fn unlocked(s: &S, sh: &Share, headers: &HeaderMap) -> bool {
    if sh.password_hash.is_none() {
        return true;
    }
    let want = unlock_value(s, sh);
    let name = unlock_cookie_name(sh);
    headers.get_all(header::COOKIE).iter().any(|v| {
        v.to_str().unwrap_or("").split(';').any(|kv| kv.trim().split_once('=').is_some_and(|(k, v)| k == name && v == want))
    })
}

fn require_unlocked(s: &S, sh: &Share, headers: &HeaderMap) -> R<()> {
    if unlocked(s, sh, headers) { Ok(()) } else { Err(AppError::unauth("This link is password protected.")) }
}

/// Mutating public requests must carry our header (cross-site forms can't set it).
fn require_header(headers: &HeaderMap) -> R<()> {
    if headers.contains_key("x-vaultix") { Ok(()) } else { Err(AppError::forbidden("Missing request header")) }
}

/// Resolve a path inside the share, never escaping it.
fn resolve(s: &S, sh: &Share, sub: &str) -> R<PathBuf> {
    let sub = clean_rel(sub)?;
    let base = target(s, sh);
    Ok(if sub.is_empty() { base } else { base.join(sub) })
}

pub async fn info(State(s): State<S>, UrlPath(token): UrlPath<String>, headers: HeaderMap) -> R<Json<Value>> {
    let sh = load(&s, &token)?;
    if !unlocked(&s, &sh, &headers) {
        return Ok(Json(json!({ "locked": true, "kind": sh.kind, "owner": sh.owner })));
    }
    let md = tokio::fs::metadata(target(&s, &sh)).await.map_err(|_| AppError::new(StatusCode::NOT_FOUND, "This item is no longer available."))?;
    s.db.lock().unwrap().execute("UPDATE shares SET views = views + 1 WHERE id = ?1", [sh.id])?;
    Ok(Json(json!({
        "locked": false,
        "kind": sh.kind,
        "owner": sh.owner,
        "name": name_of(&sh.path),
        "is_dir": md.is_dir(),
        "size": if md.is_dir() { 0 } else { md.len() },
        "mtime": mtime_ms(&md),
        "allow_download": sh.allow_download,
        "max_size": sh.max_size,
        "note": sh.note,
        "expires_at": sh.expires_at,
    })))
}

#[derive(Deserialize)]
pub struct PwReq {
    password: String,
}

pub async fn unlock(State(s): State<S>, UrlPath(token): UrlPath<String>, headers: HeaderMap, Json(q): Json<PwReq>) -> R<Response> {
    require_header(&headers)?;
    let sh = load(&s, &token)?;
    let Some(hash) = sh.password_hash.clone() else {
        return Ok(Json(json!({ "ok": true })).into_response());
    };
    let key = format!("share:{}", sh.token);
    let t = now();
    {
        let mut f = s.login_fails.lock().unwrap();
        if let Some((n, first)) = f.get(&key).copied() {
            if t - first > 900 {
                f.remove(&key);
            } else if n >= 10 {
                return Err(AppError::new(StatusCode::TOO_MANY_REQUESTS, "Too many attempts. Try again in 15 minutes."));
            }
        }
    }
    let pw = q.password.clone();
    let ok = tokio::task::spawn_blocking(move || crate::auth::verify_password(&hash, &pw)).await?;
    if !ok {
        s.login_fails.lock().unwrap().entry(key).or_insert((0, t)).0 += 1;
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        return Err(AppError::unauth("Wrong password"));
    }
    let secure = if crate::auth::is_https(&headers, &s) { "; Secure" } else { "" };
    let cookie = format!("{}={}; Path=/; HttpOnly; SameSite=Lax; Max-Age=86400{secure}", unlock_cookie_name(&sh), unlock_value(&s, &sh));
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

#[derive(Deserialize)]
pub struct SubQ {
    #[serde(default)]
    path: String,
    #[serde(default, deserialize_with = "de_flag")]
    download: bool,
}

pub async fn list_dir(State(s): State<S>, UrlPath(token): UrlPath<String>, headers: HeaderMap, Query(q): Query<SubQ>) -> R<Json<Value>> {
    let sh = load(&s, &token)?;
    if sh.kind != "link" {
        return Err(AppError::forbidden("Not allowed"));
    }
    require_unlocked(&s, &sh, &headers)?;
    let dir = resolve(&s, &sh, &q.path)?;
    let base = clean_rel(&q.path)?;
    let entries = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<Value>> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir)? {
            let e = e?;
            let Ok(md) = std::fs::metadata(e.path()) else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            if name == ".DS_Store" || name.starts_with("._") {
                continue;
            }
            let path = if base.is_empty() { name.clone() } else { format!("{base}/{name}") };
            out.push(json!({ "name": name, "path": path, "is_dir": md.is_dir(), "size": if md.is_dir() { 0 } else { md.len() }, "mtime": mtime_ms(&md) }));
        }
        Ok(out)
    })
    .await??;
    Ok(Json(json!({ "entries": entries })))
}

pub async fn file(State(s): State<S>, UrlPath(token): UrlPath<String>, Query(q): Query<SubQ>, req: Request) -> R<Response> {
    let sh = load(&s, &token)?;
    if sh.kind != "link" {
        return Err(AppError::forbidden("Not allowed"));
    }
    require_unlocked(&s, &sh, req.headers())?;
    if q.download && !sh.allow_download {
        return Err(AppError::forbidden("Downloads are turned off for this link."));
    }
    let p = resolve(&s, &sh, &q.path)?;
    files::serve_file(p, q.download, req).await
}

#[derive(Deserialize)]
pub struct ZipQ {
    #[serde(default)]
    paths: Option<String>,
}

pub async fn zip(State(s): State<S>, UrlPath(token): UrlPath<String>, headers: HeaderMap, Query(q): Query<ZipQ>) -> R<Response> {
    let sh = load(&s, &token)?;
    if sh.kind != "link" || !sh.allow_download {
        return Err(AppError::forbidden("Downloads are turned off for this link."));
    }
    require_unlocked(&s, &sh, &headers)?;
    let base = target(&s, &sh);
    let subs: Vec<String> = match q.paths.as_deref() {
        Some(p) if !p.is_empty() => serde_json::from_str::<Vec<String>>(p).map_err(|_| AppError::bad("Invalid paths"))?,
        _ => vec![],
    };
    let name = name_of(&sh.path);
    if subs.is_empty() {
        // Whole shared item, zipped relative to its parent.
        let parent = base.parent().map(Path::to_path_buf).unwrap_or(base.clone());
        let leaf = base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        return files::zip_response(&s, parent, vec![leaf], &name).await;
    }
    let rels = subs.iter().map(|p| clean_rel(p)).collect::<R<Vec<_>>>()?;
    files::zip_response(&s, base, rels, &name).await
}

/* ---------- file requests (anonymous upload) ---------- */

#[derive(Deserialize)]
pub struct UpQ {
    name: String,
    #[serde(default)]
    uploader: Option<String>,
}

fn safe_name(n: &str) -> R<String> {
    let n = n.rsplit(['/', '\\']).next().unwrap_or("").trim();
    if n.is_empty() || n == "." || n == ".." || n.len() > 255 || n.contains('\0') {
        return Err(AppError::bad("Invalid file name"));
    }
    Ok(n.to_string())
}

/// "photo.jpg" → "photo (1).jpg" … until free.
fn unique_dest(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    if !p.exists() {
        return p;
    }
    let path = Path::new(name);
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let ext = path.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    (1..).map(|i| dir.join(format!("{stem} ({i}){ext}"))).find(|p| !p.exists()).unwrap()
}

pub async fn upload(State(s): State<S>, UrlPath(token): UrlPath<String>, headers: HeaderMap, Query(q): Query<UpQ>, body: Body) -> R<Json<Value>> {
    require_header(&headers)?;
    let sh = load(&s, &token)?;
    if sh.kind != "upload" {
        return Err(AppError::forbidden("This link doesn't accept uploads."));
    }
    require_unlocked(&s, &sh, &headers)?;
    let name = safe_name(&q.name)?;
    let limit = sh.max_size.map(|m| m as u64);
    let declared = headers.get(header::CONTENT_LENGTH).and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    if let (Some(l), Some(d)) = (limit, declared) {
        if d > l {
            return Err(AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "That file is larger than this link allows."));
        }
    }
    let dir = target(&s, &sh);
    if !tokio::fs::metadata(&dir).await.map(|m| m.is_dir()).unwrap_or(false) {
        return Err(AppError::new(StatusCode::NOT_FOUND, "This upload folder is no longer available."));
    }
    let tmp = s.data_dir.join("tmp").join(random_hex(12));
    let mut size = 0u64;
    let res: R<()> = async {
        let f = tokio::fs::File::create(&tmp).await?;
        let mut w = tokio::io::BufWriter::with_capacity(1 << 20, f);
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::bad(format!("Upload interrupted: {e}")))?;
            size += chunk.len() as u64;
            if limit.is_some_and(|l| size > l) {
                return Err(AppError::new(StatusCode::PAYLOAD_TOO_LARGE, "That file is larger than this link allows."));
            }
            w.write_all(&chunk).await?;
        }
        w.flush().await?;
        w.into_inner().sync_data().await?;
        Ok(())
    }
    .await;
    if let Err(e) = res {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    // Pick a free name and move into place (never overwrites).
    let dest = {
        let dir = dir.clone();
        let name = name.clone();
        tokio::task::spawn_blocking(move || unique_dest(&dir, &name)).await?
    };
    if let Err(e) = tokio::fs::rename(&tmp, &dest).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e.into());
    }
    let saved = dest.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or(name);
    let uploader = q.uploader.map(|u| u.trim().chars().take(80).collect::<String>()).filter(|u| !u.is_empty());
    {
        let db = s.db.lock().unwrap();
        db.execute("INSERT INTO share_uploads (share_id, name, size, uploader, at) VALUES (?1, ?2, ?3, ?4, ?5)", params![sh.id, saved, size as i64, uploader, now()])?;
        db.execute("UPDATE shares SET uploads = uploads + 1 WHERE id = ?1", [sh.id])?;
    }
    s.bump(sh.user_id);
    tracing::info!("file request {}: received {saved} ({size} bytes)", &sh.token[..8]);
    Ok(Json(json!({ "ok": true, "name": saved, "size": size })))
}
