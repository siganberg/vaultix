//! Three-way sync between a local folder and a Vaultix server folder.
//!
//! For every path we compare the local state (L), the remote state (R) and the state
//! both sides had at the end of the last successful sync (B):
//!   L == R                 → in sync, record it
//!   L changed, R == B      → upload
//!   R changed, L == B      → download
//!   both changed           → conflict: keep both (local copy renamed)
//!   L gone, R == B         → delete on server (goes to server Trash)
//!   R gone, L == B         → delete locally (goes to macOS Trash)
//!   gone on one side but modified on the other → the modified version wins

use crate::config::{log, Config};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Meta {
    pub is_dir: bool,
    pub size: u64,
    pub mtime: i64,
    pub hash: String,
}

impl Meta {
    fn dir() -> Meta {
        Meta { is_dir: true, size: 0, mtime: 0, hash: String::new() }
    }
    fn same(&self, o: &Meta) -> bool {
        self.is_dir == o.is_dir && (self.is_dir || self.hash == o.hash)
    }
}

#[derive(Deserialize)]
struct Manifest {
    gen: u64,
    entries: Vec<MEntry>,
}
#[derive(Deserialize)]
struct MEntry {
    path: String,
    is_dir: bool,
    size: u64,
    mtime: i64,
    hash: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Act {
    Same,
    Upload,
    Download,
    MkdirRemote,
    MkdirLocal,
    Conflict,
    DeleteRemote,
    DeleteLocal,
    Covered, // removed as part of a parent folder delete
    Forget,
}

#[derive(Default, Debug)]
pub struct Summary {
    pub uploaded: usize,
    pub downloaded: usize,
    pub deleted: usize,
    pub conflicts: usize,
    pub errors: usize,
    /// Server generation we expect to see if nobody else changed anything.
    pub expected_gen: u64,
}

impl Summary {
    pub fn changed(&self) -> bool {
        self.uploaded + self.downloaded + self.deleted + self.conflicts > 0
    }
}

pub fn ignored(name: &OsStr) -> bool {
    let n = name.to_string_lossy();
    n == ".DS_Store"
        || n.starts_with("._")
        || n == "Icon\r"
        || n.starts_with(".vaultix-part")
        || n.starts_with(".~lock.")
        || n.starts_with("~$")
        || matches!(n.as_ref(), ".Trashes" | ".Spotlight-V100" | ".fseventsd" | ".TemporaryItems" | ".DocumentRevisions-V100" | "Thumbs.db" | "desktop.ini")
}

fn mtime_ms(md: &std::fs::Metadata) -> i64 {
    md.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn hash_file(p: &Path) -> std::io::Result<String> {
    let mut h = blake3::Hasher::new();
    h.update_reader(std::fs::File::open(p)?)?;
    Ok(h.finalize().to_hex().to_string())
}

fn nfc(s: &str) -> String {
    s.nfc().collect()
}

fn unique_suffix() -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
    format!("{:x}{:x}", n, std::process::id())
}

pub fn http() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .timeout(None)
        .user_agent(concat!("vaultix-sync/", env!("CARGO_PKG_VERSION")))
        .pool_max_idle_per_host(4)
        .build()
        .expect("http client")
}

/// Exchange credentials (+ optional 2FA code) for an app token.
pub enum LoginResult {
    Token(String),
    NeedCode,
}
/// macOS returns EHOSTUNREACH ("No route to host", os error 65) when Local Network
/// privacy blocks an app from reaching a LAN address.
pub fn is_local_network_block(e: &anyhow::Error) -> bool {
    e.chain().any(|c| c.downcast_ref::<std::io::Error>().is_some_and(|io| io.raw_os_error() == Some(65)))
        || format!("{e:#}").contains("os error 65")
}

pub fn login(server: &str, user: &str, pass: &str, code: Option<&str>) -> Result<LoginResult> {
    // An app password (created in the web app) is already a token: just verify it.
    if pass.starts_with("vx_") {
        let res = http()
            .get(format!("{}/api/me", server.trim_end_matches('/')))
            .bearer_auth(pass)
            .timeout(Duration::from_secs(30))
            .send()
            .with_context(|| format!("Can't reach {server}"))?;
        if !res.status().is_success() {
            bail!("That app password isn't valid (it may have been revoked)");
        }
        let v: serde_json::Value = res.json().unwrap_or_default();
        if v["username"].as_str().is_some_and(|u| !u.eq_ignore_ascii_case(user)) {
            bail!("That app password belongs to a different user");
        }
        return Ok(LoginResult::Token(pass.to_string()));
    }
    let host = hostname::get().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|_| "Mac".into());
    let res = http()
        .post(format!("{}/api/auth/token", server.trim_end_matches('/')))
        .timeout(Duration::from_secs(30))
        .json(&json!({ "username": user, "password": pass, "code": code, "name": format!("Desktop sync – {host}") }))
        .send()
        .with_context(|| format!("Can't reach {server}"))?;
    let status = res.status();
    let v: serde_json::Value = res.json().unwrap_or_default();
    if status.is_success() {
        return Ok(LoginResult::Token(v["token"].as_str().unwrap_or_default().to_string()));
    }
    if v["totp_required"].as_bool() == Some(true) && code.is_none() {
        return Ok(LoginResult::NeedCode);
    }
    bail!("{}", v["error"].as_str().unwrap_or(status.as_str()))
}

/// Receives structured events (progress, activity) for the desktop app.
pub type Sink = std::sync::Arc<dyn Fn(serde_json::Value) + Send + Sync>;

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64
}

/// Wraps a reader and reports bytes read, throttled.
struct ProgressReader<R> {
    inner: R,
    done: u64,
    total: u64,
    last: std::time::Instant,
    report: Box<dyn FnMut(u64, u64) + Send>,
}
impl<R: Read> Read for ProgressReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.done += n as u64;
        if self.last.elapsed() >= Duration::from_millis(150) || n == 0 {
            self.last = std::time::Instant::now();
            (self.report)(self.done, self.total);
        }
        Ok(n)
    }
}

pub struct Engine {
    pub events: Option<Sink>,
    cur: (usize, usize),
    pub cfg: Config,
    http: reqwest::blocking::Client,
    base: BTreeMap<String, Meta>,
    pub progress: Box<dyn Fn(String) + Send + Sync>,
}

/// Uploads/downloads run on this many threads at once.
const PARALLEL: usize = 6;

/// Everything a transfer needs, without the sync state, so workers can share it.
struct Ctx<'a> {
    http: &'a reqwest::blocking::Client,
    cfg: &'a Config,
    events: &'a Option<Sink>,
    progress: &'a (dyn Fn(String) + Send + Sync),
    total: usize,
}

struct LocalEntry {
    meta: Meta,
    path: PathBuf,
}

impl Engine {
    pub fn new(cfg: Config) -> Engine {
        let base = std::fs::read_to_string(cfg.state_file()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        Engine { events: None, cur: (0, 0), cfg, http: http(), base, progress: Box::new(|_| {}) }
    }

    fn emit(&self, v: serde_json::Value) {
        if let Some(s) = &self.events {
            s(v);
        }
    }

    fn activity(&self, op: &str, k: &str, is_dir: bool) {
        self.emit(json!({ "event": "activity", "op": op, "path": k, "is_dir": is_dir, "at": now_ms() }));
    }

    fn step(&mut self, what: &str, k: &str) {
        self.cur.0 += 1;
        let name = Path::new(k).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        (self.progress)(format!("{what} {name} ({}/{})", self.cur.0, self.cur.1));
        self.emit(json!({ "event": "progress", "op": what, "path": k, "index": self.cur.0, "total": self.cur.1, "bytes": 0, "size": 0 }));
    }

    fn ctx(&self) -> Ctx<'_> {
        Ctx { http: &self.http, cfg: &self.cfg, events: &self.events, progress: &*self.progress, total: self.cur.1 }
    }

    fn url(&self, p: &str) -> String {
        self.ctx().url(p)
    }

    fn remote_path(&self, rel: &str) -> String {
        self.ctx().remote_path(rel)
    }

    fn check(&self, res: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        self.ctx().check(res)
    }

    fn save_state(&self) {
        save_base(&self.cfg, &self.base);
    }

    pub fn version(&self) -> Result<u64> {
        let r = self.check(self.http.get(self.url("/api/version")).bearer_auth(&self.cfg.token).timeout(Duration::from_secs(20)).send()?)?;
        Ok(r.json::<serde_json::Value>()?["gen"].as_u64().unwrap_or(0))
    }

    fn fetch_remote(&self) -> Result<(u64, BTreeMap<String, Meta>)> {
        let r = self.check(
            self.http
                .get(self.url("/api/manifest"))
                .query(&[("path", self.cfg.remote_dir.trim_matches('/'))])
                .bearer_auth(&self.cfg.token)
                .timeout(Duration::from_secs(600))
                .send()?,
        )?;
        let m: Manifest = r.json()?;
        let map = m
            .entries
            .into_iter()
            .filter(|e| !e.path.split('/').any(|c| ignored(OsStr::new(c))))
            .map(|e| (nfc(&e.path), Meta { is_dir: e.is_dir, size: e.size, mtime: e.mtime, hash: e.hash }))
            .collect();
        Ok((m.gen, map))
    }

    fn scan_local(&self) -> Result<BTreeMap<String, LocalEntry>> {
        let root = &self.cfg.local_dir;
        let mut out = BTreeMap::new();
        let walker = walkdir::WalkDir::new(root).min_depth(1).follow_links(false).into_iter().filter_entry(|e| !ignored(e.file_name()));
        for e in walker {
            let Ok(e) = e else { continue };
            if e.file_type().is_symlink() {
                continue;
            }
            let rel = nfc(&e.path().strip_prefix(root)?.to_string_lossy());
            let Ok(md) = e.metadata() else { continue };
            if md.is_dir() {
                out.insert(rel, LocalEntry { meta: Meta::dir(), path: e.path().to_path_buf() });
                continue;
            }
            let (size, mtime) = (md.len(), mtime_ms(&md));
            let hash = match self.base.get(&rel) {
                Some(b) if !b.is_dir && b.size == size && b.mtime == mtime => b.hash.clone(),
                _ => match hash_file(e.path()) {
                    Ok(h) => h,
                    Err(err) => {
                        log(&format!("skip {rel}: {err}"));
                        continue;
                    }
                },
            };
            out.insert(rel, LocalEntry { meta: Meta { is_dir: false, size, mtime, hash }, path: e.path().to_path_buf() });
        }
        Ok(out)
    }

    pub fn sync_once(&mut self) -> Result<Summary> {
        let root = self.cfg.local_dir.clone();
        if !root.is_dir() {
            bail!("Sync folder {} is missing", root.display());
        }
        (self.progress)("Checking for changes…".into());
        let (gen, remote) = self.fetch_remote()?;
        let local = self.scan_local()?;

        // Safety net: never mirror a wipe caused by an unmounted disk or a wiped server.
        let base_files = self.base.values().filter(|m| !m.is_dir).count();
        if base_files >= 10 && local.values().all(|l| l.meta.is_dir) && !remote.is_empty() {
            bail!("The local folder looks empty. Refusing to delete {base_files} files from the server; if this is intended, delete them in the web app.");
        }
        if base_files >= 10 && remote.is_empty() && !local.is_empty() {
            bail!("The server folder looks empty. Refusing to delete {base_files} local files; check the server.");
        }

        // ---- plan ----
        let keys: BTreeSet<String> = local.keys().chain(remote.keys()).chain(self.base.keys()).cloned().collect();
        let mut acts: BTreeMap<String, Act> = BTreeMap::new();
        for k in &keys {
            let (l, r, b) = (local.get(k).map(|x| &x.meta), remote.get(k), self.base.get(k));
            let a = match (l, r) {
                (Some(l), Some(r)) if l.same(r) => Act::Same,
                (Some(l), Some(r)) if l.is_dir != r.is_dir => Act::Conflict,
                (Some(l), Some(r)) => {
                    let lc = b.is_none_or(|b| !l.same(b));
                    let rc = b.is_none_or(|b| !r.same(b));
                    match (lc, rc) {
                        (true, false) => Act::Upload,
                        (false, true) => Act::Download,
                        _ => Act::Conflict,
                    }
                }
                (Some(l), None) => match b {
                    Some(b) if l.same(b) => Act::DeleteLocal,
                    _ if l.is_dir => Act::MkdirRemote,
                    _ => Act::Upload,
                },
                (None, Some(r)) => match b {
                    Some(b) if r.same(b) => Act::DeleteRemote,
                    _ if r.is_dir => Act::MkdirLocal,
                    _ => Act::Download,
                },
                (None, None) => Act::Forget,
            };
            acts.insert(k.clone(), a);
        }
        // Folder deletes: only if everything inside is also being deleted; then do it in one go.
        let keys_vec: Vec<String> = acts.keys().cloned().collect();
        for k in keys_vec.iter() {
            let a = acts[k].clone();
            if !matches!(a, Act::DeleteLocal | Act::DeleteRemote) {
                continue;
            }
            let is_dir = local.get(k).map(|l| l.meta.is_dir).or(remote.get(k).map(|r| r.is_dir)).unwrap_or(false);
            if !is_dir {
                continue;
            }
            let prefix = format!("{k}/");
            let kids: Vec<String> = acts.range(prefix.clone()..).take_while(|(kk, _)| kk.starts_with(&prefix)).map(|(kk, _)| kk.clone()).collect();
            // Parents are visited before children (sorted keys), so `kids` still hold their planned actions.
            let blocked = kids.iter().any(|kk| acts[kk] != a && !matches!(acts[kk], Act::Covered | Act::Forget));
            if blocked {
                acts.insert(k.clone(), if a == Act::DeleteLocal { Act::MkdirRemote } else { Act::MkdirLocal });
            } else {
                for kk in kids {
                    if acts[&kk] == a {
                        acts.insert(kk, Act::Covered);
                    }
                }
            }
        }
        let work = acts.values().filter(|a| !matches!(a, Act::Same | Act::Forget | Act::Covered)).count();
        let mut sum = Summary { expected_gen: gen, ..Default::default() };
        self.cur = (0, work);

        // ---- execute: folders & conflicts, then transfers, then deletions ----
        for (k, a) in acts.iter() {
            let res: Result<()> = match a {
                Act::Same => {
                    let l = &local[k].meta;
                    self.base.insert(k.clone(), l.clone());
                    Ok(())
                }
                Act::Forget | Act::Covered => {
                    self.base.remove(k);
                    Ok(())
                }
                Act::MkdirLocal => std::fs::create_dir_all(root.join(k)).map(|_| {
                    self.base.insert(k.clone(), Meta::dir());
                }).map_err(Into::into),
                Act::MkdirRemote => {
                    self.step("Creating", k);
                    self.mkdir_remote(k).map(|_| {
                        sum.expected_gen += 1;
                        self.base.insert(k.clone(), Meta::dir());
                    })
                }
                Act::Conflict => {
                    self.step("Resolving conflict", k);
                    sum.conflicts += 1;
                    self.conflict(k, &local[k], &remote[k])
                }
                _ => Ok(()),
            };
            if let Err(e) = res {
                if e.to_string().starts_with("AUTH:") {
                    self.save_state();
                    return Err(e);
                }
                sum.errors += 1;
                log(&format!("error {k}: {e:#}"));
            }
        }
        // Transfers in parallel; results are applied here as they arrive.
        {
            let jobs: Vec<(&String, bool)> = acts.iter().filter(|(_, a)| matches!(a, Act::Upload | Act::Download)).map(|(k, a)| (k, *a == Act::Upload)).collect();
            let queue = std::sync::Mutex::new(jobs.into_iter());
            let started = std::sync::atomic::AtomicUsize::new(self.cur.0);
            let abort = std::sync::atomic::AtomicBool::new(false);
            let ctx = Ctx { http: &self.http, cfg: &self.cfg, events: &self.events, progress: &*self.progress, total: work };
            let base = &mut self.base;
            let (local, remote) = (&local, &remote);
            let mut auth_err = None;
            let mut finished = 0usize;
            std::thread::scope(|sc| {
                let (tx, rx) = std::sync::mpsc::channel::<(String, bool, Result<Option<Meta>>)>();
                for _ in 0..PARALLEL {
                    let (tx, ctx, queue, started, abort) = (tx.clone(), &ctx, &queue, &started, &abort);
                    sc.spawn(move || loop {
                        if abort.load(std::sync::atomic::Ordering::Relaxed) {
                            break;
                        }
                        let Some((k, up)) = queue.lock().unwrap().next() else { break };
                        let idx = started.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                        let res = if up {
                            ctx.announce("Uploading", k, idx);
                            ctx.upload(k, &local[k], idx)
                        } else {
                            ctx.announce("Downloading", k, idx);
                            ctx.download(k, &remote[k], idx).map(Some)
                        };
                        if res.as_ref().is_err_and(|e| e.to_string().starts_with("AUTH:")) {
                            abort.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                        if tx.send((k.clone(), up, res)).is_err() {
                            break;
                        }
                    });
                }
                drop(tx);
                for (k, up, res) in rx {
                    match res {
                        Ok(meta) => {
                            if up {
                                sum.uploaded += 1;
                                sum.expected_gen += 1; // server accepted the upload
                            } else {
                                sum.downloaded += 1;
                            }
                            if let Some(m) = meta {
                                base.insert(k, m);
                            }
                            finished += 1;
                            if finished % 50 == 0 {
                                save_base(ctx.cfg, base);
                            }
                        }
                        Err(e) if e.to_string().starts_with("AUTH:") => auth_err = Some(e),
                        Err(e) => {
                            sum.errors += 1;
                            log(&format!("error {k}: {e:#}"));
                        }
                    }
                }
            });
            self.cur.0 = started.into_inner();
            if let Some(e) = auth_err {
                self.save_state();
                return Err(e);
            }
        }
        // Remote deletes in one batch.
        let rdel: Vec<&String> = acts.iter().filter(|(_, a)| **a == Act::DeleteRemote).map(|(k, _)| k).collect();
        if !rdel.is_empty() {
            (self.progress)(format!("Deleting {} item(s) on server…", rdel.len()));
            let paths: Vec<String> = rdel.iter().map(|k| self.remote_path(k)).collect();
            match self.http.post(self.url("/api/delete")).bearer_auth(&self.cfg.token).json(&json!({ "paths": paths })).send().map_err(anyhow::Error::from).and_then(|r| self.check(r)) {
                Ok(_) => {
                    sum.expected_gen += 1;
                    for k in &rdel {
                        log(&format!("deleted on server: {k}"));
                        self.activity("deleted_remote", k, false);
                        self.base.remove(*k);
                        sum.deleted += 1;
                    }
                }
                Err(e) => {
                    sum.errors += 1;
                    log(&format!("error deleting on server: {e:#}"));
                }
            }
        }
        // Local deletes, deepest first, into the system Trash.
        for (k, a) in acts.iter().rev() {
            if *a != Act::DeleteLocal {
                continue;
            }
            let p = &local[k].path;
            let r = if p.is_dir() && dir_only_junk(p) {
                std::fs::remove_dir_all(p).map_err(anyhow::Error::from)
            } else {
                move_to_trash(p)
            };
            match r {
                Ok(_) => {
                    log(&format!("deleted locally: {k}"));
                    self.activity("deleted_local", k, false);
                    self.base.remove(k);
                    sum.deleted += 1;
                }
                Err(e) => {
                    sum.errors += 1;
                    log(&format!("error deleting {k}: {e:#}"));
                }
            }
        }
        self.save_state();
        Ok(sum)
    }

    fn mkdir_remote(&self, k: &str) -> Result<()> {
        self.check(self.http.post(self.url("/api/mkdir")).bearer_auth(&self.cfg.token).json(&json!({ "path": self.remote_path(k) })).send()?)?;
        Ok(())
    }

    /// Keep both: rename the local copy, then take the server version.
    fn conflict(&mut self, k: &str, l: &LocalEntry, r: &Meta) -> Result<()> {
        let host = hostname::get().map(|h| h.to_string_lossy().into_owned()).unwrap_or_else(|_| "Mac".into());
        let host = host.split('.').next().unwrap_or("Mac");
        let stamp = chrono::Local::now().format("%Y-%m-%d %H%M");
        let p = &l.path;
        let stem = p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let ext = p.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
        let (stem, ext) = if l.meta.is_dir { (p.file_name().unwrap().to_string_lossy().into_owned(), String::new()) } else { (stem, ext) };
        let newp = p.with_file_name(format!("{stem} (conflict {host} {stamp}){ext}"));
        std::fs::rename(p, &newp)?;
        log(&format!("conflict on {k}: kept local copy as {}", newp.display()));
        let kept = Path::new(k).with_file_name(newp.file_name().unwrap_or_default()).to_string_lossy().into_owned();
        self.emit(json!({ "event": "activity", "op": "conflict", "path": kept, "original": k, "is_dir": l.meta.is_dir, "at": now_ms() }));
        self.base.remove(k);
        if r.is_dir {
            std::fs::create_dir_all(p)?;
            self.base.insert(k.to_string(), Meta::dir());
            Ok(())
        } else {
            let m = self.ctx().download(k, r, self.cur.0)?;
            self.base.insert(k.to_string(), m);
            Ok(())
        }
    }
}

fn dir_only_junk(p: &Path) -> bool {
    walkdir::WalkDir::new(p).min_depth(1).into_iter().filter_map(|e| e.ok()).all(|e| e.file_type().is_dir() || ignored(e.file_name()))
}

fn move_to_trash(p: &Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut ctx = trash::TrashContext::default();
        // NSFileManager: no Finder automation prompt, no Finder sound.
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx.delete(p).map_err(|e| anyhow!("{e}"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::delete(p).map_err(|e| anyhow!("{e}"))
    }
}

fn save_base(cfg: &Config, base: &BTreeMap<String, Meta>) {
    let p = cfg.state_file();
    let tmp = p.with_extension("tmp");
    if std::fs::write(&tmp, serde_json::to_vec(base).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, p);
    }
}

impl Ctx<'_> {
    fn url(&self, p: &str) -> String {
        format!("{}{}", self.cfg.server.trim_end_matches('/'), p)
    }

    fn remote_path(&self, rel: &str) -> String {
        let r = self.cfg.remote_dir.trim_matches('/');
        if r.is_empty() { rel.to_string() } else { format!("{r}/{rel}") }
    }

    fn check(&self, res: reqwest::blocking::Response) -> Result<reqwest::blocking::Response> {
        let st = res.status();
        if st.is_success() {
            return Ok(res);
        }
        let msg = res.json::<serde_json::Value>().ok().and_then(|v| v["error"].as_str().map(String::from)).unwrap_or_else(|| st.to_string());
        if st == reqwest::StatusCode::UNAUTHORIZED {
            bail!("AUTH: {msg}");
        }
        bail!("{msg}")
    }

    fn emit(&self, v: serde_json::Value) {
        if let Some(s) = self.events {
            s(v);
        }
    }

    fn announce(&self, what: &str, k: &str, idx: usize) {
        let name = Path::new(k).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        (self.progress)(format!("{what} {name} ({idx}/{})", self.total));
        self.emit(json!({ "event": "progress", "op": what, "path": k, "index": idx, "total": self.total, "bytes": 0, "size": 0 }));
    }

    fn progress_cb(&self, op: &'static str, k: &str, idx: usize) -> Box<dyn FnMut(u64, u64) + Send> {
        let sink = self.events.clone();
        let (k, n) = (k.to_string(), self.total);
        Box::new(move |done, total| {
            if let Some(s) = &sink {
                s(json!({ "event": "progress", "op": op, "path": k, "index": idx, "total": n, "bytes": done, "size": total }));
            }
        })
    }

    /// Returns the meta to record, or None if the file changed while uploading (retried next pass).
    fn upload(&self, k: &str, l: &LocalEntry, idx: usize) -> Result<Option<Meta>> {
        let f = std::fs::File::open(&l.path)?;
        let size = f.metadata()?.len();
        let r = self.check(
            self.http
                .put(self.url("/api/file"))
                .query(&[("path", self.remote_path(k)), ("mtime", l.meta.mtime.to_string())])
                .bearer_auth(&self.cfg.token)
                .body(reqwest::blocking::Body::sized(
                    ProgressReader { inner: f, done: 0, total: size, last: std::time::Instant::now(), report: self.progress_cb("Uploading", k, idx) },
                    size,
                ))
                .send()?,
        )?;
        let v: serde_json::Value = r.json()?;
        if v["hash"].as_str() == Some(l.meta.hash.as_str()) {
            log(&format!("uploaded {k}"));
            self.emit(json!({ "event": "activity", "op": "uploaded", "path": k, "is_dir": false, "at": now_ms() }));
            Ok(Some(l.meta.clone()))
        } else {
            log(&format!("{k} changed while uploading; will retry"));
            Ok(None)
        }
    }

    fn download(&self, k: &str, r: &Meta, idx: usize) -> Result<Meta> {
        let dest = self.cfg.local_dir.join(k);
        if let Some(p) = dest.parent() {
            std::fs::create_dir_all(p)?;
        }
        let tmp = dest.with_file_name(format!(".vaultix-part-{}", unique_suffix()));
        let res = (|| -> Result<String> {
            let mut resp = self.check(self.http.get(self.url("/api/file")).query(&[("path", self.remote_path(k))]).bearer_auth(&self.cfg.token).send()?)?;
            let mut report = self.progress_cb("Downloading", k, idx);
            let (mut got, mut last) = (0u64, std::time::Instant::now());
            let mut f = std::io::BufWriter::with_capacity(1 << 20, std::fs::File::create(&tmp)?);
            let mut h = blake3::Hasher::new();
            let mut buf = vec![0u8; 1 << 20];
            loop {
                let n = resp.read(&mut buf)?;
                if n == 0 {
                    break;
                }
                h.update(&buf[..n]);
                f.write_all(&buf[..n])?;
                got += n as u64;
                if last.elapsed() >= Duration::from_millis(150) {
                    last = std::time::Instant::now();
                    report(got, r.size);
                }
            }
            f.into_inner().map_err(|e| anyhow!("{e}"))?.sync_all()?;
            Ok(h.finalize().to_hex().to_string())
        })();
        let hash = match res {
            Ok(h) => h,
            Err(e) => {
                let _ = std::fs::remove_file(&tmp);
                return Err(e);
            }
        };
        if hash != r.hash {
            let _ = std::fs::remove_file(&tmp);
            bail!("file changed on server during download; will retry");
        }
        let ft = filetime::FileTime::from_unix_time(r.mtime.div_euclid(1000), (r.mtime.rem_euclid(1000) * 1_000_000) as u32);
        filetime::set_file_mtime(&tmp, ft)?;
        std::fs::rename(&tmp, &dest)?;
        let md = std::fs::metadata(&dest)?;
        log(&format!("downloaded {k}"));
        self.emit(json!({ "event": "activity", "op": "downloaded", "path": k, "is_dir": false, "at": now_ms() }));
        Ok(Meta { is_dir: false, size: md.len(), mtime: mtime_ms(&md), hash })
    }
}
