use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    pub server: String,
    pub username: String,
    pub token: String,
    pub local_dir: PathBuf,
    /// Folder on the server to mirror; "" = whole drive.
    #[serde(default)]
    pub remote_dir: String,
    #[serde(default)]
    pub paused: bool,
}

pub fn dir() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("Vaultix")
}

fn path() -> PathBuf {
    dir().join("config.json")
}

impl Config {
    pub fn load() -> Option<Config> {
        let s = std::fs::read_to_string(path()).ok()?;
        let c: Config = serde_json::from_str(&s).ok()?;
        (!c.token.is_empty() && !c.server.is_empty()).then_some(c)
    }

    pub fn save(&self) -> Result<()> {
        std::fs::create_dir_all(dir())?;
        let tmp = dir().join("config.json.tmp");
        {
            let mut f = std::fs::File::create(&tmp)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
            }
            f.write_all(serde_json::to_string_pretty(self)?.as_bytes())?;
        }
        std::fs::rename(tmp, path()).context("saving config")?;
        Ok(())
    }

    pub fn remove() {
        let _ = std::fs::remove_file(path());
    }

    /// Per-account/folder sync state file, so changing setup starts fresh.
    pub fn state_file(&self) -> PathBuf {
        let key = format!("{}|{}|{}|{}", self.server, self.username, self.remote_dir, self.local_dir.display());
        let h = blake3::hash(key.as_bytes()).to_hex();
        dir().join(format!("state-{}.json", &h[..16]))
    }
}

pub fn log(msg: &str) {
    let line = format!("{} {msg}\n", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
    eprint!("{line}");
    let p = log_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // Keep the log from growing without bound.
    if std::fs::metadata(&p).map(|m| m.len() > 5_000_000).unwrap_or(false) {
        let _ = std::fs::rename(&p, p.with_extension("log.1"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn log_path() -> PathBuf {
    #[cfg(target_os = "macos")]
    {
        dirs::home_dir().unwrap_or_default().join("Library/Logs/Vaultix/sync.log")
    }
    #[cfg(not(target_os = "macos"))]
    {
        dir().join("sync.log")
    }
}
