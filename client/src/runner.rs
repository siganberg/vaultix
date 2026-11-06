use crate::config::{log, Config};
use crate::sync::{ignored, Engine};
use notify::{RecursiveMode, Watcher};
use std::sync::{
    mpsc::{self, RecvTimeoutError, Sender},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

pub enum Ctl {
    SyncNow,
    Pause(bool),
    Local,
    Quit,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Phase {
    #[default]
    Starting,
    Idle,
    Syncing,
    Paused,
    Offline,
    Error,
    SignedOut,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub phase: Phase,
    pub detail: String,
    pub last_ok: Option<chrono::DateTime<chrono::Local>>,
}

impl Status {
    pub fn line(&self) -> String {
        let when = self.last_ok.map(|t| format!(" · synced {}", t.format("%H:%M"))).unwrap_or_default();
        match self.phase {
            Phase::Starting => "Starting…".into(),
            Phase::Idle => format!("Up to date{when}"),
            Phase::Syncing => self.detail.clone(),
            Phase::Paused => "Paused".into(),
            Phase::Offline => format!("Offline – {}", self.detail),
            Phase::Error => format!("Error – {}", self.detail),
            Phase::SignedOut => "Signed out – use “Change account…”".into(),
        }
    }
}

const POLL: Duration = Duration::from_secs(15);
const FULL: Duration = Duration::from_secs(300);
const DEBOUNCE: Duration = Duration::from_millis(2000);

pub fn spawn(cfg: Config, status: Arc<Mutex<Status>>) -> Sender<Ctl> {
    let (tx, rx) = mpsc::channel();
    let txw = tx.clone();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            let relevant = ev.paths.iter().any(|p| !p.components().any(|c| ignored(c.as_os_str())));
            if relevant {
                let _ = txw.send(Ctl::Local);
            }
        }
    })
    .ok();
    if let Some(w) = watcher.as_mut() {
        if let Err(e) = w.watch(&cfg.local_dir, RecursiveMode::Recursive) {
            log(&format!("file watcher unavailable ({e}); relying on periodic sync"));
        }
    }
    std::thread::spawn(move || {
        let _keep = watcher;
        run(cfg, status, rx)
    });
    tx
}

fn set(status: &Arc<Mutex<Status>>, phase: Phase, detail: impl Into<String>) {
    let mut s = status.lock().unwrap();
    s.phase = phase;
    s.detail = detail.into();
}

fn run(cfg: Config, status: Arc<Mutex<Status>>, rx: mpsc::Receiver<Ctl>) {
    let mut eng = Engine::new(cfg.clone());
    let st = status.clone();
    eng.progress = Box::new(move |s| set(&st, Phase::Syncing, s));
    let mut paused = cfg.paused;
    let mut need = true;
    let mut last_full = Instant::now();
    let mut last_poll = Instant::now();
    let mut known_gen = 0u64;
    let mut local_dirty: Option<Instant> = None;
    let mut retry_at = Instant::now();
    let mut backoff = 0usize;
    const BACKOFF: [u64; 5] = [5, 15, 60, 180, 300];
    log(&format!("syncing {} <-> {}/{}", cfg.local_dir.display(), cfg.server, cfg.remote_dir));

    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(Ctl::SyncNow) => {
                need = true;
                retry_at = Instant::now();
            }
            Ok(Ctl::Pause(p)) => {
                paused = p;
                need = !p;
            }
            Ok(Ctl::Local) => local_dirty = Some(Instant::now()),
            Ok(Ctl::Quit) | Err(RecvTimeoutError::Disconnected) => return,
            Err(RecvTimeoutError::Timeout) => {}
        }
        if paused {
            set(&status, Phase::Paused, "");
            continue;
        }
        if status.lock().unwrap().phase == Phase::SignedOut {
            continue;
        }
        if local_dirty.is_some_and(|t| t.elapsed() >= DEBOUNCE) {
            local_dirty = None;
            need = true;
        }
        if last_full.elapsed() >= FULL {
            need = true;
        }
        if !need && last_poll.elapsed() >= POLL && Instant::now() >= retry_at {
            last_poll = Instant::now();
            match eng.version() {
                Ok(g) if g != known_gen => need = true,
                Ok(_) => {}
                Err(e) => {
                    let msg = e.to_string();
                    if msg.starts_with("AUTH:") {
                        set(&status, Phase::SignedOut, "");
                    } else {
                        set(&status, Phase::Offline, "can't reach server");
                    }
                }
            }
        }
        if !need || Instant::now() < retry_at || local_dirty.is_some() {
            continue;
        }
        need = false;
        last_full = Instant::now();
        last_poll = Instant::now();
        match eng.sync_once() {
            Ok(sum) => {
                backoff = 0;
                known_gen = sum.expected_gen;
                if sum.changed() || sum.errors > 0 {
                    log(&format!("sync done: ↑{} ↓{} ✕{} conflicts {} errors {}", sum.uploaded, sum.downloaded, sum.deleted, sum.conflicts, sum.errors));
                }
                let mut s = status.lock().unwrap();
                if sum.errors > 0 {
                    s.phase = Phase::Error;
                    s.detail = format!("{} item(s) failed, see log", sum.errors);
                    retry_at = Instant::now() + Duration::from_secs(60);
                    need = true;
                } else {
                    s.phase = Phase::Idle;
                    s.detail.clear();
                }
                s.last_ok = Some(chrono::Local::now());
            }
            Err(e) => {
                let msg = format!("{e:#}");
                log(&format!("sync failed: {msg}"));
                if msg.starts_with("AUTH:") {
                    set(&status, Phase::SignedOut, "");
                } else {
                    let net = e.chain().any(|c| c.downcast_ref::<reqwest::Error>().is_some_and(|r| r.is_connect() || r.is_timeout()));
                    set(&status, if net { Phase::Offline } else { Phase::Error }, if net { "can't reach server".to_string() } else { msg });
                    retry_at = Instant::now() + Duration::from_secs(BACKOFF[backoff.min(BACKOFF.len() - 1)]);
                    backoff += 1;
                    need = true;
                }
            }
        }
    }
}
