//! Optional disk watcher: when files under `users/<name>/files` change outside the API
//! (someone edits the data folder directly), bump that user's change counter so sync
//! clients pick the change up on their next poll instead of the next full sync.

use crate::S;
use notify::{RecursiveMode, Watcher};
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub fn spawn(state: S) -> Option<notify::RecommendedWatcher> {
    let users = state.data_dir.join("users");
    let root = users.canonicalize().unwrap_or(users.clone());
    let dirty: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));
    let (d2, r2) = (dirty.clone(), root.clone());
    let mut w = notify::recommended_watcher(move |ev: notify::Result<notify::Event>| {
        let Ok(ev) = ev else { return };
        if matches!(ev.kind, notify::EventKind::Access(_)) {
            return;
        }
        for p in ev.paths {
            // users/<name>/files/...  (ignore the per-user trash and anything else)
            let Ok(rel) = p.strip_prefix(&r2) else { continue };
            let mut c = rel.components();
            let (Some(user), Some(kind)) = (c.next(), c.next()) else { continue };
            if kind.as_os_str() == "files" {
                d2.lock().unwrap().insert(user.as_os_str().to_string_lossy().into_owned());
            }
        }
    })
    .map_err(|e| tracing::warn!("disk watcher unavailable: {e}"))
    .ok()?;
    if let Err(e) = w.watch(&root, RecursiveMode::Recursive) {
        tracing::warn!("disk watcher unavailable: {e}");
        return None;
    }
    // Debounce: at most one bump per user per second.
    let weak = Arc::downgrade(&state);
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(1));
        let Some(state) = weak.upgrade() else { break };
        let names: Vec<String> = dirty.lock().unwrap().drain().collect();
        for name in names {
            let uid: Option<i64> = state.db.lock().unwrap().query_row("SELECT id FROM users WHERE username = ?1", [&name], |r| r.get(0)).ok();
            if let Some(uid) = uid {
                state.bump_external(uid);
            }
        }
    });
    tracing::info!("watching {} for direct file changes", root.display());
    Some(w)
}
