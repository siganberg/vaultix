mod admin;
mod auth;
mod db;
mod files;
mod shares;
mod util;
mod webdav;

use axum::{
    extract::DefaultBodyLimit,
    http::{header, HeaderValue, Uri},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{any, get, post},
    Router,
};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tower_http::compression::CompressionLayer;
use util::*;

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub cookie_secure: String,
    pub login_fails: Mutex<HashMap<String, (u32, i64)>>,
    /// Server-wide secret for keyed values (share unlock cookies).
    pub secret: String,
    pub dav_cache: Mutex<HashMap<String, (i64, String, i64)>>,
    gens: Mutex<HashMap<i64, u64>>,
    boot: u64,
}
pub type S = Arc<AppState>;

impl AppState {
    pub fn user_root(&self, user: &str) -> PathBuf {
        self.data_dir.join("users").join(user).join("files")
    }
    pub fn trash_root(&self, user: &str) -> PathBuf {
        self.data_dir.join("users").join(user).join("trash")
    }
    /// Change counter per user; lets sync clients cheaply detect remote changes.
    pub fn gen(&self, uid: i64) -> u64 {
        self.boot + *self.gens.lock().unwrap().get(&uid).unwrap_or(&0)
    }
    pub fn bump(&self, uid: i64) {
        *self.gens.lock().unwrap().entry(uid).or_insert(0) += 1;
    }
}

const INDEX: &str = include_str!("../../web/index.html");
const APP_JS: &str = include_str!("../../web/app.js");
const APP_CSS: &str = include_str!("../../web/app.css");
const ICON: &str = include_str!("../../web/icon.svg");
const SHARE_HTML: &str = include_str!("../../web/share.html");
const SHARE_JS: &str = include_str!("../../web/share.js");
const THEME_JS: &str = include_str!("../../web/theme.js");
const FILEICONS_JS: &str = include_str!("../../web/fileicons.js");
const VIEWER_JS: &str = include_str!("../../web/viewer.js");

fn asset(ct: &'static str, body: &'static str, cache: &'static str) -> Response {
    ([(header::CONTENT_TYPE, ct), (header::CACHE_CONTROL, cache)], body).into_response()
}

async fn static_fallback(uri: Uri) -> Response {
    match uri.path() {
        "/app.js" => asset("text/javascript; charset=utf-8", APP_JS, "no-cache"),
        "/app.css" => asset("text/css; charset=utf-8", APP_CSS, "no-cache"),
        "/icon.svg" => asset("image/svg+xml", ICON, "public, max-age=86400"),
        "/fileicons.js" => asset("text/javascript; charset=utf-8", FILEICONS_JS, "no-cache"),
        "/viewer.js" => asset("text/javascript; charset=utf-8", VIEWER_JS, "no-cache"),
        "/theme.js" => asset("text/javascript; charset=utf-8", THEME_JS, "no-cache"),
        "/share.js" => asset("text/javascript; charset=utf-8", SHARE_JS, "no-cache"),
        p if p.starts_with("/s/") => {
            let mut r = asset("text/html; charset=utf-8", SHARE_HTML, "no-cache");
            // Share pages are reached from other sites; never leak the token via Referer.
            r.headers_mut().insert("x-robots-tag", HeaderValue::from_static("noindex, nofollow"));
            r
        }
        p if p.starts_with("/api/") => util::AppError::not_found().into_response(),
        _ => asset("text/html; charset=utf-8", INDEX, "no-cache"),
    }
}

async fn security_headers(req: axum::extract::Request, next: Next) -> Response {
    let mut res = next.run(req).await;
    let h = res.headers_mut();
    h.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert("x-frame-options", HeaderValue::from_static("SAMEORIGIN"));
    h.entry(header::CONTENT_SECURITY_POLICY).or_insert(HeaderValue::from_static(
        "default-src 'self'; img-src 'self' data: blob:; media-src 'self' blob:; style-src 'self' 'unsafe-inline'; frame-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'self'",
    ));
    res
}

fn router(state: S) -> Router {
    use auth::*;
    let json_api = Router::new()
        .route("/api/auth/login", post(login))
        .route("/api/auth/token", post(issue_token))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/password", post(change_password))
        .route("/api/me", get(me))
        .route("/api/setup", get(admin::setup_status).post(admin::setup))
        .route("/api/admin/users", get(admin::list_users).post(admin::create_user))
        .route("/api/admin/users/update", post(admin::update_user))
        .route("/api/admin/users/delete", post(admin::delete_user))
        .route("/api/2fa/setup", post(totp_setup))
        .route("/api/2fa/enable", post(totp_enable))
        .route("/api/2fa/disable", post(totp_disable))
        .route("/api/devices", get(list_devices))
        .route("/api/devices/revoke", post(revoke_device))
        .route("/api/devices/create", post(create_device))
        .route("/api/list", get(files::list))
        .route("/api/mkdir", post(files::mkdir))
        .route("/api/move", post(files::mv))
        .route("/api/delete", post(files::delete))
        .route("/api/trash", get(files::trash_list))
        .route("/api/trash/restore", post(files::trash_restore))
        .route("/api/trash/purge", post(files::trash_purge))
        .route("/api/search", get(files::search))
        .route("/api/usage", get(files::usage))
        .route("/api/version", get(files::version))
        .route("/api/manifest", get(files::manifest))
        .route("/api/shares", get(shares::list).post(shares::create))
        .route("/api/shares/update", post(shares::update))
        .route("/api/shares/revoke", post(shares::revoke))
        .route("/api/public/{token}", get(shares::info))
        .route("/api/public/{token}/unlock", post(shares::unlock))
        .route("/api/public/{token}/list", get(shares::list_dir))
        .fallback(static_fallback)
        .layer(CompressionLayer::new());
    // File streams: no compression (keeps Range requests & throughput intact), no body limit.
    let file_api = Router::new()
        .route("/api/file", get(files::download).put(files::upload))
        .route("/api/zip", get(files::zip))
        .route("/api/public/{token}/file", get(shares::file))
        .route("/api/public/{token}/zip", get(shares::zip))
        .route("/api/public/{token}/upload", axum::routing::put(shares::upload))
        .layer(DefaultBodyLimit::disable());
    let dav = Router::new()
        .route("/dav", any(webdav::handle))
        .route("/dav/", any(webdav::handle))
        .route("/dav/{*p}", any(webdav::handle))
        .route("/remote.php/webdav", any(webdav::handle))
        .route("/remote.php/webdav/{*p}", any(webdav::handle))
        .route("/remote.php/webdav/", any(webdav::handle))
        .route("/remote.php/dav/files/{user}", any(webdav::handle))
        .route("/remote.php/dav/files/{user}/", any(webdav::handle))
        .route("/remote.php/dav/files/{user}/{*p}", any(webdav::handle))
        .layer(DefaultBodyLimit::disable());
    json_api.merge(file_api).merge(dav).layer(middleware::from_fn(security_headers)).with_state(state)
}

pub(crate) fn create_user(s: &AppState, username: &str, password: &str, admin: bool) -> anyhow::Result<()> {
    let username = username.trim().to_lowercase();
    anyhow::ensure!(valid_username(&username), "username may only contain a-z, 0-9, '.', '_' and '-'");
    anyhow::ensure!(password.len() >= 8, "password must be at least 8 characters");
    let hash = auth::hash_password(password)?;
    s.db.lock().unwrap().execute(
        "INSERT INTO users (username, password_hash, is_admin, created_at) VALUES (?1, ?2, ?3, ?4)",
        params![username, hash, admin, now()],
    )?;
    std::fs::create_dir_all(s.user_root(&username))?;
    std::fs::create_dir_all(s.trash_root(&username))?;
    Ok(())
}

fn prompt_password() -> anyhow::Result<String> {
    if let Ok(p) = std::env::var("VAULTIX_PASSWORD") {
        return Ok(p);
    }
    let a = rpassword::prompt_password("New password: ")?;
    let b = rpassword::prompt_password("Repeat password: ")?;
    anyhow::ensure!(a == b, "passwords do not match");
    Ok(a)
}

fn cli(s: &AppState, args: &[String]) -> anyhow::Result<()> {
    let a: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let db = || s.db.lock().unwrap();
    match a.as_slice() {
        ["user", "add", name, rest @ ..] => {
            create_user(s, name, &prompt_password()?, rest.contains(&"--admin"))?;
            println!("Created user {name}");
        }
        ["user", "passwd", name] => {
            let h = auth::hash_password(&prompt_password()?)?;
            let n = db().execute("UPDATE users SET password_hash = ?1 WHERE username = ?2", params![h, name])?;
            anyhow::ensure!(n == 1, "no such user");
            db().execute("DELETE FROM sessions WHERE user_id = (SELECT id FROM users WHERE username = ?1)", [name])?;
            println!("Password updated; sessions signed out.");
        }
        ["user", "reset-2fa", name] => {
            let n = db().execute("UPDATE users SET totp_enabled = 0, totp_secret = NULL WHERE username = ?1", [name])?;
            anyhow::ensure!(n == 1, "no such user");
            println!("2FA disabled for {name}");
        }
        ["user", "delete", name] => {
            let n = db().execute("DELETE FROM users WHERE username = ?1", [name])?;
            anyhow::ensure!(n == 1, "no such user");
            println!("Deleted {name}. Files kept at {}", s.data_dir.join("users").join(name).display());
        }
        ["user", "list"] => {
            let d = db();
            let mut st = d.prepare("SELECT username, is_admin, totp_enabled FROM users ORDER BY username")?;
            for r in st.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, bool>(1)?, r.get::<_, bool>(2)?)))? {
                let (u, adm, t) = r?;
                println!("{u}{}{}", if adm { "  [admin]" } else { "" }, if t { "  [2fa]" } else { "" });
            }
        }
        _ => {
            eprintln!(
                "Vaultix server\n\nUsage:\n  vaultix-server                      start the server\n  vaultix-server user add <name> [--admin]\n  vaultix-server user passwd <name>\n  vaultix-server user reset-2fa <name>\n  vaultix-server user delete <name>\n  vaultix-server user list\n\nEnv: VAULTIX_DATA (./data), VAULTIX_BIND (0.0.0.0:8080), VAULTIX_ADMIN_USER/VAULTIX_ADMIN_PASSWORD (first run),\n     VAULTIX_COOKIE_SECURE (auto|true|false), VAULTIX_TRASH_DAYS (30)"
            );
            std::process::exit(2);
        }
    }
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = tokio::signal::ctrl_c();
    #[cfg(unix)]
    {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        tokio::select! { _ = ctrl_c => {}, _ = term.recv() => {} }
    }
    #[cfg(not(unix))]
    ctrl_c.await.ok();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_string());
    if std::env::args().nth(1).as_deref() == Some("health") {
        // Used by the Docker HEALTHCHECK (the image ships no curl).
        let port = env("VAULTIX_BIND", "0.0.0.0:8080").rsplit(':').next().unwrap_or("8080").to_string();
        let mut s = std::net::TcpStream::connect(format!("127.0.0.1:{port}"))?;
        use std::io::{Read, Write};
        s.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
        s.write_all(b"GET /icon.svg HTTP/1.0\r\n\r\n")?;
        let mut buf = [0u8; 12];
        s.read_exact(&mut buf)?;
        anyhow::ensure!(buf.ends_with(b"200"), "unhealthy");
        return Ok(());
    }
    let data_dir = PathBuf::from(env("VAULTIX_DATA", "./data"));
    std::fs::create_dir_all(data_dir.join("users"))?;
    std::fs::create_dir_all(data_dir.join("tmp"))?;
    let conn = db::open(&data_dir.join("vaultix.db"))?;
    let secret: String = match conn.query_row("SELECT value FROM settings WHERE key = 'secret'", [], |r| r.get(0)).optional()? {
        Some(s) => s,
        None => {
            let s = random_hex(32);
            conn.execute("INSERT INTO settings (key, value) VALUES ('secret', ?1)", [&s])?;
            s
        }
    };
    let state = Arc::new(AppState {
        db: Mutex::new(conn),
        data_dir: data_dir.clone(),
        secret,
        cookie_secure: env("VAULTIX_COOKIE_SECURE", "auto"),
        login_fails: Mutex::new(HashMap::new()),
        dav_cache: Mutex::new(HashMap::new()),
        gens: Mutex::new(HashMap::new()),
        boot: now_ms() as u64,
    });

    let args: Vec<String> = std::env::args().skip(1).collect();
    if !args.is_empty() {
        return cli(&state, &args);
    }

    // First run: create admin from env.
    let users: i64 = state.db.lock().unwrap().query_row("SELECT COUNT(*) FROM users", [], |r| r.get(0))?;
    if users == 0 {
        match (std::env::var("VAULTIX_ADMIN_USER"), std::env::var("VAULTIX_ADMIN_PASSWORD")) {
            (Ok(u), Ok(p)) => {
                create_user(&state, &u, &p, true)?;
                tracing::info!("created admin user '{u}'");
            }
            _ => tracing::warn!("no users yet: open the web app to create the admin account"),
        }
    }
    // Clear interrupted uploads.
    for e in std::fs::read_dir(data_dir.join("tmp"))?.flatten() {
        let _ = std::fs::remove_file(e.path());
    }
    let _: Option<i64> = state.db.lock().unwrap().query_row("DELETE FROM sessions WHERE expires_at < ?1 RETURNING 0", [now()], |r| r.get(0)).optional().unwrap_or(None);

    let trash_days: i64 = env("VAULTIX_TRASH_DAYS", "30").parse().unwrap_or(30);
    let st = state.clone();
    tokio::spawn(async move {
        loop {
            let s2 = st.clone();
            let _ = tokio::task::spawn_blocking(move || files::purge_old_trash(&s2, trash_days)).await;
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    });

    let bind = env("VAULTIX_BIND", "0.0.0.0:8080");
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    tracing::info!("Vaultix listening on http://{bind}  (data: {})", data_dir.display());
    axum::serve(listener, router(state)).with_graceful_shutdown(shutdown_signal()).await?;
    Ok(())
}
