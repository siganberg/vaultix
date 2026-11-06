//! macOS menu-bar app: setup dialogs, tray menu, launch-at-login.

use crate::config::{self, log, Config};
use crate::runner::{self, Ctl, Phase, Status};
use crate::sync::{is_local_network_block, LoginResult};
use anyhow::Result;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tray_icon::{Icon, TrayIconBuilder};

/* ---------- AppleScript dialogs ---------- */

fn osa(lines: &[&str], args: &[&str]) -> Option<String> {
    let mut c = Command::new("osascript");
    for l in lines {
        c.arg("-e").arg(l);
    }
    c.args(args);
    let out = c.output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim_end_matches('\n').to_string())
}

fn ask(prompt: &str, default: &str, hidden: bool) -> Option<String> {
    let dlg = format!(
        "display dialog (item 1 of argv) default answer (item 2 of argv) {} with title \"Vaultix\" buttons {{\"Cancel\", \"Continue\"}} default button \"Continue\" with icon note",
        if hidden { "with hidden answer" } else { "" }
    );
    osa(&["on run argv", "activate", &dlg, "text returned of result", "end run"], &[prompt, default])
}

fn alert(title: &str, msg: &str) {
    osa(&["on run argv", "activate", "display alert (item 1 of argv) message (item 2 of argv) as critical", "end run"], &[title, msg]);
}

fn buttons(msg: &str, b1: &str, b2: &str) -> Option<String> {
    osa(
        &["on run argv", "activate", "display dialog (item 1 of argv) with title \"Vaultix\" buttons {\"Cancel\", item 2 of argv, item 3 of argv} default button 3 with icon note", "button returned of result", "end run"],
        &[msg, b1, b2],
    )
}

fn choose_folder(prompt: &str) -> Option<PathBuf> {
    osa(&["on run argv", "activate", "POSIX path of (choose folder with prompt (item 1 of argv))", "end run"], &[prompt]).map(PathBuf::from)
}

pub fn setup(prev: Option<&Config>) -> Option<Config> {
    let mut server = prev.map(|c| c.server.clone()).unwrap_or_else(|| "https://".into());
    let mut user = prev.map(|c| c.username.clone()).unwrap_or_default();
    loop {
        server = ask("Welcome to Vaultix!\n\nEnter your server address:", &server, false)?.trim().trim_end_matches('/').to_string();
        if !server.starts_with("http://") && !server.starts_with("https://") {
            server = format!("https://{server}");
        }
        user = ask("Username:", &user, false)?.trim().to_lowercase();
        let pass = ask(&format!("Password for {user}:"), "", true)?;
        let token = match login(&server, &user, &pass, None) {
            Ok(LoginResult::Token(t)) => t,
            Ok(LoginResult::NeedCode) => {
                let code = ask("Enter the 6-digit code from your authenticator app\n(or a recovery code):", "", false)?;
                match login(&server, &user, &pass, Some(code.trim())) {
                    Ok(LoginResult::Token(t)) => t,
                    Ok(LoginResult::NeedCode) => {
                        alert("Couldn't sign in", "Invalid 2FA code.");
                        continue;
                    }
                    Err(e) => {
                        alert("Couldn't sign in", &format!("{e:#}"));
                        continue;
                    }
                }
            }
            Err(e) => {
                alert("Couldn't sign in", &format!("{e:#}"));
                continue;
            }
        };

        let default_dir = dirs::home_dir().unwrap_or_default().join("Vaultix");
        let local = match buttons("Where should Vaultix keep your files on this Mac?", "Choose Other…", "Use ~/Vaultix")?.as_str() {
            "Use ~/Vaultix" => {
                let _ = std::fs::create_dir_all(&default_dir);
                default_dir
            }
            _ => choose_folder("Choose the folder to sync with Vaultix")?,
        };
        let local = local.canonicalize().unwrap_or(local);
        let remote = ask("Which server folder should be synced?\nUse “/” for your whole drive.", "/", false)?;
        let remote = remote.trim().trim_matches('/').to_string();
        return Some(Config { server, username: user, token, local_dir: local, remote_dir: remote, paused: false });
    }
}

const LAN_HELP: &str = "macOS is blocking Vaultix from reaching your server on the local network.\n\nIf a “Vaultix would like to find and connect to devices on your local network” prompt appeared, click Allow. Otherwise turn Vaultix on in System Settings → Privacy & Security → Local Network.\n\nThen click Try Again.";

/// Login, transparently retrying while the user answers the Local Network prompt.
fn login(server: &str, user: &str, pass: &str, code: Option<&str>) -> Result<LoginResult> {
    for attempt in 0.. {
        match crate::sync::login(server, user, pass, code) {
            Err(e) if is_local_network_block(&e) => {
                if attempt < 10 {
                    // The first connection fails while macOS shows its permission prompt.
                    std::thread::sleep(Duration::from_secs(2));
                    continue;
                }
                let again = osa(
                    &["on run argv", "activate", "display dialog (item 1 of argv) with title \"Vaultix\" buttons {\"Cancel\", \"Open Settings\", \"Try Again\"} default button 3 with icon caution", "button returned of result", "end run"],
                    &[LAN_HELP],
                );
                match again.as_deref() {
                    Some("Open Settings") => {
                        let _ = Command::new("open").arg("x-apple.systempreferences:com.apple.preference.security?Privacy_LocalNetwork").spawn();
                        let _ = osa(&["on run argv", "activate", "display dialog \"Turn on Vaultix under Local Network, then click Try Again.\" with title \"Vaultix\" buttons {\"Try Again\"} default button 1", "end run"], &[]);
                    }
                    Some("Try Again") => {}
                    _ => return Err(e),
                }
            }
            r => return r,
        }
    }
    unreachable!()
}

/* ---------- launch at login ---------- */

fn agent_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join("Library/LaunchAgents/app.vaultix.sync.plist")
}

fn login_item_enabled() -> bool {
    agent_path().exists()
}

fn set_login_item(on: bool) {
    let p = agent_path();
    if !on {
        let _ = std::fs::remove_file(p);
        return;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>Label</key><string>app.vaultix.sync</string>
  <key>ProgramArguments</key><array><string>{}</string></array>
  <key>RunAtLoad</key><true/>
  <key>ProcessType</key><string>Interactive</string>
</dict></plist>
"#,
        xml_escape(&exe.to_string_lossy())
    );
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(p, plist);
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/* ---------- tray icon ---------- */

/// Draws the Vaultix "V in a rounded square" as a 44×44 template image.
fn tray_icon(dim: bool) -> Icon {
    const N: usize = 44;
    let mut px = vec![0u8; N * N * 4];
    let seg = |p: (f32, f32), a: (f32, f32), b: (f32, f32)| {
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0);
        ((p.0 - a.0 - t * dx).powi(2) + (p.1 - a.1 - t * dy).powi(2)).sqrt()
    };
    for y in 0..N {
        for x in 0..N {
            let p = (x as f32 + 0.5, y as f32 + 0.5);
            // rounded-square outline
            let (cx, cy, half, r) = (22.0f32, 22.0f32, 18.0f32, 8.0f32);
            let qx = (p.0 - cx).abs() - (half - r);
            let qy = (p.1 - cy).abs() - (half - r);
            let outside = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - r;
            let d_box = outside.abs() - 1.6;
            // the V
            let d_v = seg(p, (14.0, 15.0), (22.0, 30.0)).min(seg(p, (22.0, 30.0), (30.0, 15.0))) - 2.6;
            let d = d_box.min(d_v);
            let a = (0.5 - d).clamp(0.0, 1.0);
            let i = (y * N + x) * 4;
            px[i + 3] = (a * if dim { 110.0 } else { 255.0 }) as u8;
        }
    }
    Icon::from_rgba(px, N as u32, N as u32).expect("icon")
}

/* ---------- app ---------- */

pub fn app() -> Result<()> {
    std::fs::create_dir_all(config::dir())?;
    let lock = std::fs::File::create(config::dir().join("app.lock"))?;
    if fs2::FileExt::try_lock_exclusive(&lock).is_err() {
        // Already running; nothing to do.
        return Ok(());
    }

    let mut cfg = match Config::load() {
        Some(c) => c,
        None => match setup(None) {
            Some(c) => {
                c.save()?;
                set_login_item(true);
                c
            }
            None => return Ok(()),
        },
    };

    let status = Arc::new(Mutex::new(Status::default()));
    let mut ctl = runner::spawn(cfg.clone(), status.clone());

    let mut event_loop = EventLoopBuilder::new().build();
    event_loop.set_activation_policy(ActivationPolicy::Accessory);

    let status_item = MenuItem::new("Starting…", false, None);
    let account_item = MenuItem::new(format!("{} @ {}", cfg.username, host_of(&cfg.server)), false, None);
    let open_folder = MenuItem::new("Open Vaultix Folder", true, None);
    let open_web = MenuItem::new("Open in Browser", true, None);
    let sync_now = MenuItem::new("Sync Now", true, None);
    let pause = MenuItem::new(if cfg.paused { "Resume Syncing" } else { "Pause Syncing" }, true, None);
    let login_item = CheckMenuItem::new("Start at Login", true, login_item_enabled(), None);
    let show_log = MenuItem::new("Show Log", true, None);
    let change = MenuItem::new("Change Account or Folder…", true, None);
    let quit = MenuItem::new("Quit Vaultix", true, None);
    let menu = Menu::new();
    menu.append_items(&[
        &status_item,
        &account_item,
        &PredefinedMenuItem::separator(),
        &open_folder,
        &open_web,
        &PredefinedMenuItem::separator(),
        &sync_now,
        &pause,
        &PredefinedMenuItem::separator(),
        &login_item,
        &show_log,
        &change,
        &PredefinedMenuItem::separator(),
        &quit,
    ])?;

    let mut tray = None;
    let mut menu_opt = Some(menu);
    let mut last_line = String::new();
    let mut last_dim = false;

    event_loop.run(move |event, _, cf| {
        *cf = ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(400));
        if let Event::NewEvents(StartCause::Init) = event {
            if let Some(menu) = menu_opt.take() {
                tray = TrayIconBuilder::new()
                    .with_menu(Box::new(menu))
                    .with_icon(tray_icon(false))
                    .with_icon_as_template(true)
                    .with_tooltip("Vaultix")
                    .build()
                    .ok();
            }
        }

        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            let id = ev.id;
            if id == open_folder.id() {
                let _ = Command::new("open").arg(&cfg.local_dir).spawn();
            } else if id == open_web.id() {
                let _ = Command::new("open").arg(&cfg.server).spawn();
            } else if id == sync_now.id() {
                let _ = ctl.send(Ctl::SyncNow);
            } else if id == pause.id() {
                cfg.paused = !cfg.paused;
                let _ = cfg.save();
                pause.set_text(if cfg.paused { "Resume Syncing" } else { "Pause Syncing" });
                let _ = ctl.send(Ctl::Pause(cfg.paused));
            } else if id == login_item.id() {
                set_login_item(login_item.is_checked());
            } else if id == show_log.id() {
                let _ = Command::new("open").args(["-a", "Console"]).arg(config::log_path()).spawn();
            } else if id == change.id() {
                if let Some(new) = setup(Some(&cfg)) {
                    let _ = ctl.send(Ctl::Quit);
                    if let Err(e) = new.save() {
                        alert("Couldn't save settings", &e.to_string());
                    }
                    cfg = new;
                    account_item.set_text(format!("{} @ {}", cfg.username, host_of(&cfg.server)));
                    pause.set_text("Pause Syncing");
                    *status.lock().unwrap() = Status::default();
                    ctl = runner::spawn(cfg.clone(), status.clone());
                    log("configuration changed");
                }
            } else if id == quit.id() {
                let _ = ctl.send(Ctl::Quit);
                *cf = ControlFlow::Exit;
            }
        }

        let st = status.lock().unwrap().clone();
        let line = st.line();
        if line != last_line {
            status_item.set_text(&line);
            if let Some(t) = &tray {
                let _ = t.set_tooltip(Some(format!("Vaultix – {line}")));
            }
            last_line = line;
        }
        let dim = matches!(st.phase, Phase::Paused | Phase::Offline | Phase::SignedOut | Phase::Error);
        if dim != last_dim {
            if let Some(t) = &tray {
                let _ = t.set_icon_with_as_template(Some(tray_icon(dim)), true);
            }
            last_dim = dim;
        }
        let _ = &lock; // hold the single-instance lock for the app's lifetime
    });
}

fn host_of(url: &str) -> String {
    url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or(url).to_string()
}
