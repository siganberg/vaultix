//! The sync agent: the engine behind a JSON command/event protocol (macOS helper + desktop app).
//!
//! Protocol: newline-delimited JSON.
//!   stdin  (commands): {"cmd":"login","server":..,"username":..,"password":..,"code":..}
//!                      {"cmd":"configure","local_dir":..,"remote_dir":..}
//!                      {"cmd":"sync_now"} {"cmd":"pause","paused":true} {"cmd":"logout"}
//!                      {"cmd":"usage"} {"cmd":"quit"}
//!   stdout (events):   state, status, progress, activity, login_result, usage, error, fatal
//! The agent exits when stdin closes, so it never outlives the app.

use crate::config::{self, log, Config};
use crate::runner::{self, Ctl, Status};
use crate::sync::{self, is_local_network_block, LoginResult, Sink};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::sync::{mpsc::Sender, Arc, Mutex};
use std::time::Duration;

#[derive(Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
enum Cmd {
    Login { server: String, username: String, password: String, code: Option<String> },
    Configure { local_dir: String, remote_dir: String },
    SyncNow,
    Pause { paused: bool },
    Logout,
    Usage,
    Quit,
}

struct Agent {
    cfg: Option<Config>,
    ctl: Option<Sender<Ctl>>,
    /// Signed in but folders not chosen yet: (server, username, token).
    pending: Option<(String, String, String)>,
    status: Arc<Mutex<Status>>,
    out: Sink,
}

impl Agent {
    fn emit_state(&self) {
        let c = self.cfg.as_ref();
        (self.out)(json!({
            "event": "state",
            "configured": c.is_some(),
            "server": c.map(|c| c.server.clone()),
            "username": c.map(|c| c.username.clone()),
            "local_dir": c.map(|c| c.local_dir.to_string_lossy().into_owned()),
            "remote_dir": c.map(|c| c.remote_dir.clone()),
            "paused": c.map(|c| c.paused).unwrap_or(false),
            "version": env!("CARGO_PKG_VERSION"),
            "log_path": config::log_path(),
        }));
    }

    fn stop(&mut self) {
        if let Some(ctl) = self.ctl.take() {
            let _ = ctl.send(Ctl::Quit);
        }
        *self.status.lock().unwrap() = Status::default();
    }

    fn start(&mut self) {
        self.stop();
        if let Some(cfg) = &self.cfg {
            self.ctl = Some(runner::spawn(cfg.clone(), self.status.clone(), Some(self.out.clone())));
        }
    }
}

fn friendly(e: &anyhow::Error, server: &str) -> String {
    if e.chain().any(|c| c.downcast_ref::<reqwest::Error>().is_some_and(|r| r.is_connect() || r.is_timeout())) {
        return format!("Can't reach {server}. Check the address and your connection.");
    }
    e.root_cause().to_string()
}

fn normalize_server(s: &str) -> String {
    let s = s.trim().trim_end_matches('/');
    if s.starts_with("http://") || s.starts_with("https://") { s.to_string() } else { format!("https://{s}") }
}

/// Headless helper for the macOS app: commands on stdin, events on stdout.
pub fn run() -> anyhow::Result<()> {
    let out: Sink = Arc::new(|v: Value| {
        let mut o = std::io::stdout().lock();
        let _ = writeln!(o, "{v}");
        let _ = o.flush();
    });
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                break;
            }
        }
    });
    serve(rx, out)
}

/// The agent loop, driven by any command source (stdin for the Mac helper, a channel for the
/// embedded desktop app). Returns when the source closes or a `quit` command arrives.
pub fn serve(cmds: std::sync::mpsc::Receiver<String>, out: Sink) -> anyhow::Result<()> {
    std::fs::create_dir_all(config::dir())?;
    let lock = std::fs::File::create(config::dir().join("agent.lock"))?;
    if fs2::FileExt::try_lock_exclusive(&lock).is_err() {
        out(json!({ "event": "fatal", "message": "Another Vaultix sync process is already running." }));
        return Ok(());
    }

    let status = Arc::new(Mutex::new(Status::default()));
    let agent = Arc::new(Mutex::new(Agent { cfg: Config::load(), ctl: None, pending: None, status: status.clone(), out: out.clone() }));
    {
        let mut a = agent.lock().unwrap();
        a.emit_state();
        a.start();
    }

    // Status ticker: forward status changes.
    {
        let (status, out) = (status.clone(), out.clone());
        std::thread::spawn(move || {
            let mut last = String::new();
            loop {
                std::thread::sleep(Duration::from_millis(250));
                let s = status.lock().unwrap().clone();
                let v = json!({
                    "event": "status",
                    "phase": format!("{:?}", s.phase).to_lowercase(),
                    "detail": s.detail,
                    "line": s.line(),
                    "last_ok": s.last_ok.map(|t| t.timestamp_millis()),
                });
                let key = v.to_string();
                if key != last {
                    out(v);
                    last = key;
                }
            }
        });
    }

    for line in cmds {
        if line.trim().is_empty() {
            continue;
        }
        let cmd: Cmd = match serde_json::from_str(&line) {
            Ok(c) => c,
            Err(e) => {
                out(json!({ "event": "error", "message": format!("bad command: {e}") }));
                continue;
            }
        };
        match cmd {
            Cmd::Login { server, username, password, code } => {
                let (agent, out) = (agent.clone(), out.clone());
                std::thread::spawn(move || {
                    let server = normalize_server(&server);
                    let user = username.trim().to_lowercase();
                    let code = code.filter(|c| !c.trim().is_empty());
                    let mut tries = 0;
                    let res = loop {
                        let r = sync::login(&server, &user, &password, code.as_deref());
                        match &r {
                            // macOS fails the first LAN connection while it shows the permission prompt.
                            Err(e) if is_local_network_block(e) && tries < 15 => {
                                tries += 1;
                                std::thread::sleep(Duration::from_secs(2));
                            }
                            _ => break r,
                        }
                    };
                    match res {
                        Ok(LoginResult::Token(t)) => {
                            agent.lock().unwrap().pending = Some((server.clone(), user.clone(), t));
                            out(json!({ "event": "login_result", "ok": true, "server": server, "username": user }));
                        }
                        Ok(LoginResult::NeedCode) => out(json!({ "event": "login_result", "ok": false, "need_code": true })),
                        Err(e) => {
                            let lan = is_local_network_block(&e);
                            let bad_code = code.is_some() && format!("{e}").to_lowercase().contains("2fa");
                            out(json!({
                                "event": "login_result", "ok": false,
                                "error": if lan { "Your system is blocking Vaultix from the local network.".to_string() } else { friendly(&e, &server) },
                                "local_network": lan,
                                "need_code": bad_code,
                            }));
                        }
                    }
                });
            }
            Cmd::Configure { local_dir, remote_dir } => {
                let mut a = agent.lock().unwrap();
                let creds = a.pending.take().or_else(|| a.cfg.as_ref().map(|c| (c.server.clone(), c.username.clone(), c.token.clone())));
                let Some((server, username, token)) = creds else {
                    out(json!({ "event": "error", "message": "Sign in first." }));
                    continue;
                };
                let dir = std::path::PathBuf::from(&local_dir);
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    out(json!({ "event": "error", "message": format!("Can't use {local_dir}: {e}") }));
                    continue;
                }
                let cfg = Config {
                    server,
                    username,
                    token,
                    local_dir: dir.canonicalize().unwrap_or(dir),
                    remote_dir: remote_dir.trim().trim_matches('/').to_string(),
                    paused: false,
                };
                if let Err(e) = cfg.save() {
                    out(json!({ "event": "error", "message": format!("Couldn't save settings: {e}") }));
                    continue;
                }
                log(&format!("configured: {} -> {}/{}", cfg.local_dir.display(), cfg.server, cfg.remote_dir));
                a.cfg = Some(cfg);
                a.start();
                a.emit_state();
            }
            Cmd::SyncNow => {
                if let Some(c) = &agent.lock().unwrap().ctl {
                    let _ = c.send(Ctl::SyncNow);
                }
            }
            Cmd::Pause { paused } => {
                let mut a = agent.lock().unwrap();
                if let Some(cfg) = a.cfg.as_mut() {
                    cfg.paused = paused;
                    let _ = cfg.save();
                }
                if let Some(c) = &a.ctl {
                    let _ = c.send(Ctl::Pause(paused));
                }
                a.emit_state();
            }
            Cmd::Logout => {
                let mut a = agent.lock().unwrap();
                a.stop();
                a.cfg = None;
                a.pending = None;
                Config::remove();
                log("signed out");
                a.emit_state();
            }
            Cmd::Usage => {
                let cfg = agent.lock().unwrap().cfg.clone();
                if let Some(cfg) = cfg {
                    let out = out.clone();
                    std::thread::spawn(move || {
                        let r = sync::http()
                            .get(format!("{}/api/usage", cfg.server))
                            .bearer_auth(&cfg.token)
                            .timeout(Duration::from_secs(60))
                            .send()
                            .and_then(|r| r.error_for_status())
                            .and_then(|r| r.json::<Value>());
                        if let Ok(v) = r {
                            out(json!({ "event": "usage", "used": v["used"], "free": v["free"], "total": v["total"] }));
                        }
                    });
                }
            }
            Cmd::Quit => break,
        }
    }
    agent.lock().unwrap().stop();
    drop(lock);
    Ok(())
}
