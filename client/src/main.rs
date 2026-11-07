use vaultix_sync::{agent, config, runner, sync};

use anyhow::{bail, Result};
use config::Config;
use std::io::{BufRead, Write};
use std::sync::{Arc, Mutex};

fn read_line(prompt: &str) -> Result<String> {
    eprint!("{prompt}");
    std::io::stderr().flush()?;
    let mut s = String::new();
    std::io::stdin().lock().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

/// `vaultix-sync login <server> <user> <local-dir> [remote-dir]`
fn cli_login(a: &[String]) -> Result<()> {
    if a.len() < 3 {
        bail!("usage: vaultix-sync login <server> <user> <local-dir> [remote-dir]");
    }
    let server = a[0].trim_end_matches('/').to_string();
    let pass = match std::env::var("VAULTIX_PASSWORD") {
        Ok(p) => p,
        Err(_) => read_line("Password: ")?,
    };
    let token = match sync::login(&server, &a[1], &pass, None)? {
        sync::LoginResult::Token(t) => t,
        sync::LoginResult::NeedCode => {
            let code = match std::env::var("VAULTIX_CODE") {
                Ok(c) => c,
                Err(_) => read_line("2FA code: ")?,
            };
            match sync::login(&server, &a[1], &pass, Some(&code))? {
                sync::LoginResult::Token(t) => t,
                _ => bail!("invalid 2FA code"),
            }
        }
    };
    let local = std::path::PathBuf::from(&a[2]);
    std::fs::create_dir_all(&local)?;
    let cfg = Config {
        server,
        username: a[1].to_lowercase(),
        token,
        local_dir: local.canonicalize()?,
        remote_dir: a.get(3).map(|s| s.trim_matches('/').to_string()).unwrap_or_default(),
        paused: false,
    };
    cfg.save()?;
    println!("Signed in. Config saved to {}", config::dir().display());
    Ok(())
}

fn cli_once() -> Result<()> {
    let Some(cfg) = Config::load() else { bail!("not configured; run `vaultix-sync login` first") };
    let mut eng = sync::Engine::new(cfg);
    eng.progress = Box::new(|s| eprintln!("  {s}"));
    let s = eng.sync_once()?;
    println!("uploaded {}, downloaded {}, deleted {}, conflicts {}, errors {}", s.uploaded, s.downloaded, s.deleted, s.conflicts, s.errors);
    Ok(())
}

fn headless() -> Result<()> {
    let Some(cfg) = Config::load() else { bail!("not configured; run `vaultix-sync login` first") };
    let status = Arc::new(Mutex::new(runner::Status::default()));
    let _ctl = runner::spawn(cfg, status.clone(), None);
    let mut last = String::new();
    loop {
        std::thread::sleep(std::time::Duration::from_millis(500));
        let l = status.lock().unwrap().line();
        if l != last {
            println!("{l}");
            last = l;
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let r = match args.first().map(String::as_str) {
        Some("login") => cli_login(&args[1..]),
        Some("once") => cli_once(),
        Some("run") => headless(),
        Some("agent") => agent::run(),
        Some("logout") => {
            Config::remove();
            println!("Signed out.");
            Ok(())
        }
        Some("help" | "-h" | "--help") => {
            println!("vaultix-sync agent      JSON-lines helper used by Vaultix.app\nvaultix-sync login <server> <user> <local-dir> [remote-dir]\nvaultix-sync once       run a single sync\nvaultix-sync run        sync continuously without UI\nvaultix-sync logout");
            Ok(())
        }
        // The macOS UI is Vaultix.app (SwiftUI); without arguments this helper syncs headlessly.
        _ => headless(),
    };
    if let Err(e) = r {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
