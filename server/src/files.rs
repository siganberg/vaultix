use crate::auth::Auth;
use crate::util::*;
use crate::S;
use axum::{
    body::Body,
    extract::{Query, Request, State},
    http::{header, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use futures_util::StreamExt;
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;
use tower_http::services::ServeFile;

#[derive(Serialize)]
pub struct Entry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    mtime: i64,
}

#[derive(Deserialize)]
pub struct PathQ {
    #[serde(default)]
    path: String,
}

fn join(base: &str, name: &str) -> String {
    if base.is_empty() { name.to_string() } else { format!("{base}/{name}") }
}

fn root(s: &S, auth: &Auth) -> PathBuf {
    s.user_root(&auth.username)
}

/* ---------- listing ---------- */

pub async fn list(auth: Auth, State(s): State<S>, Query(q): Query<PathQ>) -> R<Json<Value>> {
    let rel = clean_rel(&q.path)?;
    let root = root(&s, &auth);
    if rel.is_empty() {
        tokio::fs::create_dir_all(&root).await?;
    }
    let dir = root.join(&rel);
    let entries = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<Entry>> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir)? {
            let e = e?;
            let Ok(md) = std::fs::metadata(e.path()) else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            out.push(Entry {
                path: join(&rel, &name),
                name,
                is_dir: md.is_dir(),
                size: if md.is_dir() { 0 } else { md.len() },
                mtime: mtime_ms(&md),
            });
        }
        Ok(out)
    })
    .await??;
    Ok(Json(json!({ "entries": entries })))
}

/* ---------- download ---------- */

#[derive(Deserialize)]
pub struct GetQ {
    path: String,
    #[serde(default)]
    download: bool,
}

pub async fn download(auth: Auth, State(s): State<S>, Query(q): Query<GetQ>, req: Request) -> R<Response> {
    let rel = clean_rel(&q.path)?;
    let p = root(&s, &auth).join(&rel);
    let md = tokio::fs::metadata(&p).await?;
    if md.is_dir() {
        return Err(AppError::bad("That's a folder"));
    }
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let mut res = ServeFile::new(&p).oneshot(req).await.map_err(|e| AppError::internal(e.to_string()))?.map(Body::new);
    let ct = res.headers().get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    // Active content (html/svg/xml) must never render inline on our origin.
    let risky = ct.contains("html") || ct.contains("svg") || ct.contains("xml") || ct.contains("javascript");
    let disp = if q.download || risky { "attachment" } else { "inline" };
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("{disp}; filename*=UTF-8''{}", utf8_percent_encode(&name, NON_ALPHANUMERIC))).unwrap(),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, no-cache"));
    if !ct.starts_with("application/pdf") {
        h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox; default-src 'none'; img-src 'self'; media-src 'self'; style-src 'unsafe-inline'"));
    }
    Ok(res)
}

/* ---------- upload ---------- */

#[derive(Deserialize)]
pub struct PutQ {
    path: String,
    /// Modification time to preserve (unix ms).
    mtime: Option<i64>,
}

pub async fn upload(auth: Auth, State(s): State<S>, Query(q): Query<PutQ>, body: Body) -> R<Json<Value>> {
    let rel = clean_rel(&q.path)?;
    if rel.is_empty() {
        return Err(AppError::bad("Missing file name"));
    }
    let dest = root(&s, &auth).join(&rel);
    if tokio::fs::metadata(&dest).await.map(|m| m.is_dir()).unwrap_or(false) {
        return Err(AppError::conflict("A folder with that name already exists"));
    }
    if let Some(parent) = dest.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|_| AppError::conflict("Parent path is not a folder"))?;
    }
    let tmp = s.data_dir.join("tmp").join(random_hex(12));
    let mut hasher = blake3::Hasher::new();
    let mut size = 0u64;
    let res: R<()> = async {
        let f = tokio::fs::File::create(&tmp).await?;
        let mut w = tokio::io::BufWriter::with_capacity(1 << 20, f);
        let mut stream = body.into_data_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| AppError::bad(format!("Upload interrupted: {e}")))?;
            hasher.update(&chunk);
            size += chunk.len() as u64;
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
    if let Some(m) = q.mtime {
        let ft = filetime::FileTime::from_unix_time(m.div_euclid(1000), (m.rem_euclid(1000) * 1_000_000) as u32);
        let _ = filetime::set_file_mtime(&tmp, ft);
    }
    if let Err(e) = tokio::fs::rename(&tmp, &dest).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e.into());
    }
    let md = tokio::fs::metadata(&dest).await?;
    let hash = hasher.finalize().to_hex().to_string();
    let mtime = mtime_ms(&md);
    s.db.lock().unwrap().execute(
        "INSERT OR REPLACE INTO hash_cache (user_id, path, size, mtime, hash) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![auth.uid, rel, size as i64, mtime, hash],
    )?;
    s.bump(auth.uid);
    Ok(Json(json!({ "path": rel, "size": size, "mtime": mtime, "hash": hash })))
}

/* ---------- mkdir / move / delete ---------- */

pub async fn mkdir(auth: Auth, State(s): State<S>, Json(q): Json<PathQ>) -> R<Json<Value>> {
    let rel = clean_rel(&q.path)?;
    if rel.is_empty() {
        return Err(AppError::bad("Missing folder name"));
    }
    let p = root(&s, &auth).join(&rel);
    if tokio::fs::metadata(&p).await.map(|m| !m.is_dir()).unwrap_or(false) {
        return Err(AppError::conflict("A file with that name already exists"));
    }
    tokio::fs::create_dir_all(&p).await?;
    s.bump(auth.uid);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct MoveReq {
    from: String,
    to: String,
    #[serde(default)]
    overwrite: bool,
}

pub async fn mv(auth: Auth, State(s): State<S>, Json(q): Json<MoveReq>) -> R<Json<Value>> {
    let from = clean_rel(&q.from)?;
    let to = clean_rel(&q.to)?;
    if from.is_empty() || to.is_empty() {
        return Err(AppError::bad("Invalid path"));
    }
    if from == to {
        return Ok(Json(json!({ "ok": true })));
    }
    if to.starts_with(&format!("{from}/")) {
        return Err(AppError::bad("Can't move a folder into itself"));
    }
    let root = root(&s, &auth);
    let (src, dst) = (root.join(&from), root.join(&to));
    tokio::fs::metadata(&src).await?;
    // Case-only rename (e.g. "a.txt" -> "A.txt") on case-insensitive filesystems is fine.
    if tokio::fs::metadata(&dst).await.is_ok() && from.to_lowercase() != to.to_lowercase() {
        if !q.overwrite {
            return Err(AppError::conflict("Something with that name already exists"));
        }
        trash_path(&s, &auth, &to).await?;
    }
    if let Some(parent) = dst.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::rename(&src, &dst).await?;
    s.db.lock().unwrap().execute(
        "UPDATE OR REPLACE hash_cache SET path = ?3 || substr(path, length(?2) + 1)
         WHERE user_id = ?1 AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
        params![auth.uid, from, to],
    )?;
    s.bump(auth.uid);
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct DeleteReq {
    paths: Vec<String>,
}

fn dir_size(p: &Path) -> u64 {
    walkdir::WalkDir::new(p).into_iter().filter_map(|e| e.ok()).filter_map(|e| e.metadata().ok()).filter(|m| m.is_file()).map(|m| m.len()).sum()
}

async fn trash_path(s: &S, auth: &Auth, rel: &str) -> R<i64> {
    let src = root(s, auth).join(rel);
    let md = tokio::fs::metadata(&src).await?;
    let troot = s.trash_root(&auth.username);
    tokio::fs::create_dir_all(&troot).await?;
    let stored = format!("{}-{}", now_ms(), random_hex(4));
    let size = if md.is_dir() {
        let p = src.clone();
        tokio::task::spawn_blocking(move || dir_size(&p)).await?
    } else {
        md.len()
    };
    tokio::fs::rename(&src, troot.join(&stored)).await?;
    let db = s.db.lock().unwrap();
    db.execute(
        "INSERT INTO trash (user_id, orig_path, stored_name, is_dir, size, deleted_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![auth.uid, rel, stored, md.is_dir(), size as i64, now()],
    )?;
    let id = db.last_insert_rowid();
    db.execute(
        "DELETE FROM hash_cache WHERE user_id = ?1 AND (path = ?2 OR substr(path, 1, length(?2) + 1) = ?2 || '/')",
        params![auth.uid, rel],
    )?;
    Ok(id)
}

pub async fn delete(auth: Auth, State(s): State<S>, Json(q): Json<DeleteReq>) -> R<Json<Value>> {
    let mut ids = Vec::new();
    for p in q.paths {
        let rel = clean_rel(&p)?;
        if rel.is_empty() {
            continue;
        }
        match trash_path(&s, &auth, &rel).await {
            Ok(id) => ids.push(id),
            Err(e) if e.status == axum::http::StatusCode::NOT_FOUND => {}
            Err(e) => return Err(e),
        }
    }
    s.bump(auth.uid);
    Ok(Json(json!({ "deleted": ids.len(), "trash_ids": ids })))
}

/* ---------- trash ---------- */

pub async fn trash_list(auth: Auth, State(s): State<S>) -> R<Json<Value>> {
    let db = s.db.lock().unwrap();
    let mut st = db.prepare("SELECT id, orig_path, is_dir, size, deleted_at FROM trash WHERE user_id = ?1 ORDER BY deleted_at DESC")?;
    let items: Vec<Value> = st
        .query_map([auth.uid], |r| {
            Ok(json!({ "id": r.get::<_, i64>(0)?, "path": r.get::<_, String>(1)?, "is_dir": r.get::<_, bool>(2)?, "size": r.get::<_, i64>(3)?, "deleted_at": r.get::<_, i64>(4)? }))
        })?
        .collect::<Result<_, _>>()?;
    Ok(Json(json!({ "items": items })))
}

#[derive(Deserialize)]
pub struct TrashReq {
    ids: Option<Vec<i64>>,
    #[serde(default)]
    all: bool,
}

fn trash_rows(s: &S, auth: &Auth, req: &TrashReq) -> R<Vec<(i64, String, String)>> {
    let db = s.db.lock().unwrap();
    let mut st = db.prepare("SELECT id, orig_path, stored_name FROM trash WHERE user_id = ?1")?;
    let rows = st.query_map([auth.uid], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?.collect::<Result<Vec<(i64, String, String)>, _>>()?;
    Ok(rows.into_iter().filter(|r| req.all || req.ids.as_ref().is_some_and(|ids| ids.contains(&r.0))).collect())
}

pub async fn trash_restore(auth: Auth, State(s): State<S>, Json(req): Json<TrashReq>) -> R<Json<Value>> {
    let root = root(&s, &auth);
    let troot = s.trash_root(&auth.username);
    for (id, orig, stored) in trash_rows(&s, &auth, &req)? {
        let mut dest = root.join(&orig);
        let mut n = 1;
        while tokio::fs::metadata(&dest).await.is_ok() {
            let p = Path::new(&orig);
            let stem = p.file_stem().map(|x| x.to_string_lossy().into_owned()).unwrap_or_default();
            let ext = p.extension().map(|x| format!(".{}", x.to_string_lossy())).unwrap_or_default();
            dest = root.join(&orig).with_file_name(format!("{stem} (restored {n}){ext}"));
            n += 1;
        }
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::rename(troot.join(&stored), &dest).await?;
        s.db.lock().unwrap().execute("DELETE FROM trash WHERE id = ?1", [id])?;
    }
    s.bump(auth.uid);
    Ok(Json(json!({ "ok": true })))
}

pub async fn trash_purge(auth: Auth, State(s): State<S>, Json(req): Json<TrashReq>) -> R<Json<Value>> {
    let troot = s.trash_root(&auth.username);
    for (id, _, stored) in trash_rows(&s, &auth, &req)? {
        let p = troot.join(&stored);
        let _ = tokio::task::spawn_blocking(move || if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) }).await?;
        s.db.lock().unwrap().execute("DELETE FROM trash WHERE id = ?1", [id])?;
    }
    Ok(Json(json!({ "ok": true })))
}

/// Remove trash items older than `days` for every user.
pub fn purge_old_trash(s: &S, days: i64) {
    let cutoff = now() - days * 86400;
    let rows: Vec<(i64, String, String)> = {
        let db = s.db.lock().unwrap();
        let mut st = match db.prepare("SELECT t.id, u.username, t.stored_name FROM trash t JOIN users u ON u.id = t.user_id WHERE t.deleted_at < ?1") {
            Ok(st) => st,
            Err(_) => return,
        };
        st.query_map([cutoff], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).map(|it| it.filter_map(|r| r.ok()).collect()).unwrap_or_default()
    };
    for (id, user, stored) in rows {
        let p = s.trash_root(&user).join(stored);
        let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        let _ = s.db.lock().unwrap().execute("DELETE FROM trash WHERE id = ?1", [id]);
    }
}

/* ---------- search / usage / version ---------- */

#[derive(Deserialize)]
pub struct SearchQ {
    q: String,
}

pub async fn search(auth: Auth, State(s): State<S>, Query(q): Query<SearchQ>) -> R<Json<Value>> {
    let root = root(&s, &auth);
    let needle = q.q.to_lowercase();
    if needle.trim().is_empty() {
        return Ok(Json(json!({ "entries": [] })));
    }
    let entries = tokio::task::spawn_blocking(move || {
        let mut out = Vec::new();
        for e in walkdir::WalkDir::new(&root).min_depth(1).into_iter().filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy();
            if !name.to_lowercase().contains(&needle) {
                continue;
            }
            let Ok(md) = e.metadata() else { continue };
            let rel = e.path().strip_prefix(&root).unwrap().to_string_lossy().into_owned();
            out.push(Entry { name: name.into_owned(), path: rel, is_dir: md.is_dir(), size: if md.is_dir() { 0 } else { md.len() }, mtime: mtime_ms(&md) });
            if out.len() >= 300 {
                break;
            }
        }
        out
    })
    .await?;
    Ok(Json(json!({ "entries": entries })))
}

pub async fn usage(auth: Auth, State(s): State<S>) -> R<Json<Value>> {
    let root = root(&s, &auth);
    let data = s.data_dir.clone();
    let (used, free, total) = tokio::task::spawn_blocking(move || {
        (dir_size(&root), fs2::available_space(&data).unwrap_or(0), fs2::total_space(&data).unwrap_or(0))
    })
    .await?;
    Ok(Json(json!({ "used": used, "free": free, "total": total })))
}

pub async fn version(auth: Auth, State(s): State<S>) -> Json<Value> {
    Json(json!({ "gen": s.gen(auth.uid) }))
}

/* ---------- sync manifest ---------- */

#[derive(Serialize)]
struct MEntry {
    path: String,
    is_dir: bool,
    size: u64,
    mtime: i64,
    hash: String,
}

fn hash_file(p: &Path) -> std::io::Result<String> {
    let mut h = blake3::Hasher::new();
    h.update_reader(std::fs::File::open(p)?)?;
    Ok(h.finalize().to_hex().to_string())
}

pub async fn manifest(auth: Auth, State(s): State<S>, Query(q): Query<PathQ>) -> R<Json<Value>> {
    let base = clean_rel(&q.path)?;
    let gen = s.gen(auth.uid);
    let root = root(&s, &auth);
    let dir = root.join(&base);
    tokio::fs::create_dir_all(&dir).await?;
    let cache: HashMap<String, (i64, i64, String)> = {
        let db = s.db.lock().unwrap();
        let mut st = db.prepare("SELECT path, size, mtime, hash FROM hash_cache WHERE user_id = ?1")?;
        let it = st.query_map([auth.uid], |r| Ok((r.get::<_, String>(0)?, (r.get(1)?, r.get(2)?, r.get(3)?))))?;
        it.collect::<Result<_, _>>()?
    };
    let (entries, fresh) = tokio::task::spawn_blocking(move || {
        let mut entries = Vec::new();
        let mut fresh = Vec::new();
        for e in walkdir::WalkDir::new(&dir).min_depth(1).into_iter().filter_map(|e| e.ok()) {
            let Ok(md) = e.metadata() else { continue };
            let rel = e.path().strip_prefix(&dir).unwrap().to_string_lossy().into_owned();
            if md.is_dir() {
                entries.push(MEntry { path: rel, is_dir: true, size: 0, mtime: mtime_ms(&md), hash: String::new() });
                continue;
            }
            let full_rel = join(&base, &rel);
            let (size, mtime) = (md.len() as i64, mtime_ms(&md));
            let hash = match cache.get(&full_rel) {
                Some((cs, cm, h)) if *cs == size && *cm == mtime => h.clone(),
                _ => match hash_file(e.path()) {
                    Ok(h) => {
                        fresh.push((full_rel, size, mtime, h.clone()));
                        h
                    }
                    Err(_) => continue,
                },
            };
            entries.push(MEntry { path: rel, is_dir: false, size: size as u64, mtime, hash });
        }
        (entries, fresh)
    })
    .await?;
    if !fresh.is_empty() {
        let mut db = s.db.lock().unwrap();
        let tx = db.transaction()?;
        for (p, size, mtime, h) in fresh {
            tx.execute("INSERT OR REPLACE INTO hash_cache (user_id, path, size, mtime, hash) VALUES (?1, ?2, ?3, ?4, ?5)", params![auth.uid, p, size, mtime, h])?;
        }
        tx.commit()?;
    }
    Ok(Json(json!({ "gen": gen, "entries": entries })))
}

/* ---------- zip download ---------- */

#[derive(Deserialize)]
pub struct ZipQ {
    /// JSON array of paths.
    paths: String,
}

pub async fn zip(auth: Auth, State(s): State<S>, Query(q): Query<ZipQ>) -> R<Response> {
    let paths: Vec<String> = serde_json::from_str(&q.paths).map_err(|_| AppError::bad("Invalid paths"))?;
    let rels = paths.iter().map(|p| clean_rel(p)).collect::<R<Vec<_>>>()?;
    let root = root(&s, &auth);
    let tmp = s.data_dir.join("tmp").join(format!("{}.zip", random_hex(12)));
    let tmp2 = tmp.clone();
    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        use zip::write::SimpleFileOptions;
        let mut zw = zip::ZipWriter::new(std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(&tmp2)?));
        let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored).large_file(true);
        let mut buf = vec![0u8; 1 << 20];
        for rel in rels {
            let src = root.join(&rel);
            // Entries are named relative to the item's parent folder.
            let parent = src.parent().unwrap_or(&root).to_path_buf();
            for e in walkdir::WalkDir::new(&src).into_iter().filter_map(|e| e.ok()) {
                let name = e.path().strip_prefix(&parent)?.to_string_lossy().into_owned();
                let opts = match e.metadata().ok().and_then(|m| m.modified().ok()).and_then(|t| zip::DateTime::try_from(time::OffsetDateTime::from(t)).ok()) {
                    Some(dt) => opts.last_modified_time(dt),
                    None => opts,
                };
                if e.file_type().is_dir() {
                    zw.add_directory(name, opts)?;
                } else if e.file_type().is_file() {
                    zw.start_file(name, opts)?;
                    let mut f = std::fs::File::open(e.path())?;
                    loop {
                        let n = f.read(&mut buf)?;
                        if n == 0 { break; }
                        zw.write_all(&buf[..n])?;
                    }
                }
            }
        }
        zw.finish()?.flush()?;
        Ok(())
    })
    .await??;
    let file = tokio::fs::File::open(&tmp).await?;
    let len = file.metadata().await?.len();
    let _ = tokio::fs::remove_file(&tmp).await; // unlinked; stays readable while open
    let name = if paths.len() == 1 {
        Path::new(&paths[0]).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "files".into())
    } else {
        "vaultix-files".into()
    };
    let disp = format!("attachment; filename*=UTF-8''{}.zip", utf8_percent_encode(&name, NON_ALPHANUMERIC));
    Ok((
        [(header::CONTENT_TYPE, "application/zip".to_string()), (header::CONTENT_LENGTH, len.to_string()), (header::CONTENT_DISPOSITION, disp)],
        Body::from_stream(tokio_util::io::ReaderStream::with_capacity(file, 1 << 20)),
    )
        .into_response())
}
