//! Vaultix desktop sync for Windows and Linux (also runs on macOS for development).
//!
//! The sync engine runs in-process (`vaultix_sync::agent::serve`) behind the same JSON
//! command/event protocol the macOS app uses. This file owns the tray icon, the windows
//! (popover, sign-in, settings) and a snapshot of engine state the UI renders.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_notification::NotificationExt as _;
use tauri_plugin_opener::OpenerExt as _;

const POPOVER_W: f64 = 360.0;
const POPOVER_H: f64 = 480.0;
const RECENT_MAX: usize = 40;

#[derive(Serialize, Deserialize, Clone)]
struct Prefs {
    notifications: bool,
}
impl Default for Prefs {
    fn default() -> Self {
        Prefs { notifications: true }
    }
}

#[derive(Serialize, Clone, Default)]
struct Snap {
    state: Value,
    status: Value,
    transfer: Option<Value>,
    recent: Vec<Value>,
    usage: Option<Value>,
    error: Option<String>,
    platform: &'static str,
    default_dir: String,
    notifications: bool,
    autostart: bool,
}

struct Shared {
    tx: Mutex<mpsc::Sender<String>>,
    snap: Mutex<Snap>,
    dirty: AtomicBool,
    prefs: Mutex<Prefs>,
    quitting: AtomicBool,
    popover_hidden_at: AtomicU64,
    popover_shown_at: AtomicU64,
    pause_item: Mutex<Option<MenuItem<tauri::Wry>>>,
    last_phase: Mutex<String>,
    state_seen: AtomicBool,
}

impl Shared {
    fn send(&self, v: Value) {
        let _ = self.tx.lock().unwrap().send(v.to_string());
    }
}

fn data_dir() -> PathBuf {
    vaultix_sync::config::dir()
}
fn now_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}
fn load_json<T: for<'a> Deserialize<'a> + Default>(name: &str) -> T {
    std::fs::read(data_dir().join(name)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}
fn save_json<T: Serialize>(name: &str, v: &T) {
    let p = data_dir().join(name);
    let tmp = p.with_extension("tmp");
    if std::fs::write(&tmp, serde_json::to_vec(v).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(tmp, p);
    }
}

fn platform() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/* ---------------- windows ---------------- */

fn popover(app: &AppHandle) -> Option<tauri::WebviewWindow> {
    if let Some(w) = app.get_webview_window("popover") {
        return Some(w);
    }
    let w = WebviewWindowBuilder::new(app, "popover", WebviewUrl::App("popover.html".into()))
        .title("Vaultix")
        .inner_size(POPOVER_W, POPOVER_H)
        .resizable(false)
        .decorations(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .shadow(true)
        .visible(false)
        .build()
        .ok()?;
    Some(w)
}

/// Show the popover next to the tray icon (Windows/macOS), or in the top-right corner when the
/// tray can't tell us where it is (Linux app indicators).
fn show_popover(app: &AppHandle, anchor: Option<tauri::Rect>) {
    let shared = app.state::<Arc<Shared>>();
    let Some(w) = popover(app) else { return };
    if w.is_visible().unwrap_or(false) {
        let _ = w.hide();
        return;
    }
    // A click on the tray first blurs (hides) the popover; don't immediately reopen it.
    if now_ms().saturating_sub(shared.popover_hidden_at.load(Ordering::Relaxed)) < 250 {
        return;
    }
    let scale = w.scale_factor().unwrap_or(1.0);
    let (pw, ph) = ((POPOVER_W * scale) as i32, (POPOVER_H * scale) as i32);
    let margin = (8.0 * scale) as i32;
    let pos = match anchor {
        Some(r) => {
            let p = r.position.to_physical::<i32>(scale);
            let s = r.size.to_physical::<i32>(scale);
            let mon = app.monitor_from_point(p.x as f64, p.y as f64).ok().flatten().or_else(|| w.primary_monitor().ok().flatten());
            let (mx, my, mw, mh) = mon
                .map(|m| { let a = m.work_area(); (a.position.x, a.position.y, a.size.width as i32, a.size.height as i32) })
                .unwrap_or((0, 0, 1920, 1080));
            let x = (p.x + s.width / 2 - pw / 2).clamp(mx + margin, mx + mw - pw - margin);
            // Taskbar at the bottom → open upwards; menu bar / top panel → open downwards.
            let y = if p.y > my + mh / 2 { p.y - ph - margin } else { p.y + s.height + margin };
            PhysicalPosition::new(x, y.clamp(my + margin, my + mh - ph - margin))
        }
        None => {
            let mon = w.primary_monitor().ok().flatten();
            let (mx, my, mw) = mon.map(|m| { let a = m.work_area(); (a.position.x, a.position.y, a.size.width as i32) }).unwrap_or((0, 0, 1920));
            PhysicalPosition::new(mx + mw - pw - margin * 2, my + margin * 2)
        }
    };
    let _ = w.set_position(pos);
    shared.popover_shown_at.store(now_ms(), Ordering::Relaxed);
    let _ = w.show();
    let _ = w.set_focus();
    let _ = w.emit("popover-shown", ());
}

fn show_window(app: &AppHandle, name: &str) {
    if name == "popover" {
        show_popover(app, None);
        return;
    }
    if let Some(p) = app.get_webview_window("popover") {
        let _ = p.hide();
    }
    if let Some(w) = app.get_webview_window(name) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let (url, title, w, h) = match name {
        "setup" => ("setup.html", "Sign in to Vaultix", 440.0, 600.0),
        _ => ("settings.html", "Vaultix Settings", 560.0, 460.0),
    };
    let _ = WebviewWindowBuilder::new(app, name, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(w, h)
        .resizable(false)
        .maximizable(false)
        .minimizable(name != "setup")
        .center()
        .focused(true)
        .build();
}

fn on_main(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || f(&a));
}

/* ---------------- engine events ---------------- */

fn notify(app: &AppHandle, title: &str, body: &str) {
    if !app.state::<Arc<Shared>>().prefs.lock().unwrap().notifications {
        return;
    }
    let _ = app.notification().builder().title(title).body(body).show();
}

fn tray_update(app: &AppHandle) {
    let shared = app.state::<Arc<Shared>>();
    let (phase, line, paused) = {
        let s = shared.snap.lock().unwrap();
        (
            s.status["phase"].as_str().unwrap_or("").to_string(),
            s.status["line"].as_str().unwrap_or("Vaultix").to_string(),
            s.state["paused"].as_bool().unwrap_or(false),
        )
    };
    let icon: &[u8] = match phase.as_str() {
        _ if paused => include_bytes!("../icons/tray-paused.png"),
        "syncing" | "scanning" => include_bytes!("../icons/tray-sync.png"),
        "error" | "signedout" | "offline" => include_bytes!("../icons/tray-error.png"),
        _ => include_bytes!("../icons/tray.png"),
    };
    if let Some(t) = app.tray_by_id("main") {
        let _ = t.set_tooltip(Some(format!("Vaultix — {line}")));
        if let Ok(img) = Image::from_bytes(icon) {
            let _ = t.set_icon(Some(img));
        }
    }
    let item = shared.pause_item.lock().unwrap().clone();
    if let Some(item) = item {
        let _ = item.set_text(if paused { "Resume Syncing" } else { "Pause Syncing" });
    }
}

fn handle_event(app: &AppHandle, v: Value) {
    let shared = app.state::<Arc<Shared>>().inner().clone();
    let ev = v["event"].as_str().unwrap_or("").to_string();
    match ev.as_str() {
        "state" => {
            let configured = v["configured"].as_bool().unwrap_or(false);
            shared.snap.lock().unwrap().state = v;
            if configured {
                shared.send(json!({ "cmd": "usage" }));
            }
            if !shared.state_seen.swap(true, Ordering::Relaxed) && !configured {
                on_main(app, |a| show_window(a, "setup"));
            }
            on_main(app, tray_update);
        }
        "status" => {
            let phase = v["phase"].as_str().unwrap_or("idle").to_string();
            let line = v["line"].as_str().unwrap_or("").to_string();
            {
                let mut s = shared.snap.lock().unwrap();
                if phase != "syncing" {
                    s.transfer = None;
                }
                s.status = v;
            }
            let prev = std::mem::replace(&mut *shared.last_phase.lock().unwrap(), phase.clone());
            if prev != phase {
                if prev == "syncing" && phase == "idle" {
                    shared.send(json!({ "cmd": "usage" }));
                    save_json("desktop-recent.json", &shared.snap.lock().unwrap().recent);
                }
                let configured = shared.snap.lock().unwrap().state["configured"].as_bool().unwrap_or(false);
                if configured && (phase == "error" || phase == "signedout") {
                    notify(app, if phase == "signedout" { "Vaultix is signed out" } else { "Vaultix sync problem" }, &line);
                }
                on_main(app, tray_update);
            }
        }
        "progress" => shared.snap.lock().unwrap().transfer = Some(v),
        "activity" => {
            if v["op"] == "conflict" {
                let name = v["original"].as_str().or(v["path"].as_str()).unwrap_or("").rsplit('/').next().unwrap_or("").to_string();
                notify(app, "Edited in two places", &format!("Vaultix kept both versions of “{name}”."));
            }
            let mut s = shared.snap.lock().unwrap();
            let mut a = v;
            if a["at"].is_null() {
                a["at"] = json!(now_ms());
            }
            s.recent.retain(|r| r["path"] != a["path"]);
            s.recent.insert(0, a);
            s.recent.truncate(RECENT_MAX);
        }
        "usage" => shared.snap.lock().unwrap().usage = Some(v),
        "login_result" => {
            let _ = app.emit("login_result", v);
            return;
        }
        "error" | "fatal" => {
            shared.snap.lock().unwrap().error = v["message"].as_str().map(String::from);
            let _ = app.emit("engine_error", v);
        }
        _ => return,
    }
    shared.dirty.store(true, Ordering::Relaxed);
}

fn snapshot(app: &AppHandle) -> Snap {
    let shared = app.state::<Arc<Shared>>();
    let mut s = shared.snap.lock().unwrap().clone();
    s.notifications = shared.prefs.lock().unwrap().notifications;
    s.autostart = app.autolaunch().is_enabled().unwrap_or(false);
    s
}

/* ---------------- commands ---------------- */

#[tauri::command]
fn get_snapshot(app: AppHandle) -> Snap {
    snapshot(&app)
}

#[tauri::command]
fn engine(app: AppHandle, cmd: Value) {
    let shared = app.state::<Arc<Shared>>();
    if cmd["cmd"] == "logout" {
        let mut s = shared.snap.lock().unwrap();
        s.recent.clear();
        s.usage = None;
        save_json("desktop-recent.json", &s.recent);
    }
    if cmd["cmd"] == "login" || cmd["cmd"] == "configure" {
        shared.snap.lock().unwrap().error = None;
    }
    shared.send(cmd);
}

#[tauri::command]
async fn open_window(app: AppHandle, name: String) {
    on_main(&app, move |a| show_window(a, &name));
}

#[tauri::command]
fn close_window(window: tauri::WebviewWindow) {
    if window.label() == "popover" {
        let _ = window.hide();
    } else {
        let _ = window.close();
    }
}

fn cfg_str(app: &AppHandle, key: &str) -> String {
    app.state::<Arc<Shared>>().snap.lock().unwrap().state[key].as_str().unwrap_or("").to_string()
}

#[tauri::command]
fn open_target(app: AppHandle, what: String, path: Option<String>) -> Result<(), String> {
    let o = app.opener();
    let r = match what.as_str() {
        "folder" => o.open_path(cfg_str(&app, "local_dir"), None::<&str>),
        "web" => o.open_url(cfg_str(&app, "server"), None::<&str>),
        "log" => o.open_path(vaultix_sync::config::log_path().to_string_lossy(), None::<&str>),
        "reveal" => {
            let rel = path.unwrap_or_default();
            let p = PathBuf::from(cfg_str(&app, "local_dir")).join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
            if p.exists() { o.reveal_item_in_dir(p) } else { o.open_path(cfg_str(&app, "local_dir"), None::<&str>) }
        }
        _ => return Err("unknown target".into()),
    };
    if let Some(p) = app.get_webview_window("popover") {
        let _ = p.hide();
    }
    r.map_err(|e| e.to_string())
}

#[tauri::command]
async fn pick_folder(app: AppHandle, start: Option<String>) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut d = app.dialog().file().set_title("Choose your Vaultix folder");
    if let Some(s) = start.filter(|s| !s.is_empty()) {
        let p = PathBuf::from(&s);
        d = d.set_directory(if p.exists() { p } else { p.parent().map(|p| p.to_path_buf()).unwrap_or(p) });
    }
    d.pick_folder(move |f| {
        let _ = tx.send(f.and_then(|f| f.into_path().ok()).map(|p| p.to_string_lossy().into_owned()));
    });
    rx.recv().ok().flatten()
}

#[tauri::command]
fn set_pref(app: AppHandle, key: String, value: bool) -> Result<(), String> {
    let shared = app.state::<Arc<Shared>>();
    match key.as_str() {
        "autostart" => {
            let al = app.autolaunch();
            if value { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
        }
        "notifications" => {
            let mut p = shared.prefs.lock().unwrap();
            p.notifications = value;
            save_json("desktop.json", &*p);
        }
        _ => return Err("unknown setting".into()),
    }
    shared.dirty.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn quit(app: AppHandle) {
    quit_app(&app);
}

fn quit_app(app: &AppHandle) {
    let shared = app.state::<Arc<Shared>>();
    if shared.quitting.swap(true, Ordering::Relaxed) {
        return;
    }
    save_json("desktop-recent.json", &shared.snap.lock().unwrap().recent);
    shared.send(json!({ "cmd": "quit" }));
    let a = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(700));
        a.exit(0);
    });
}

/* ---------------- main ---------------- */

fn main() {
    let (tx, rx) = mpsc::channel::<String>();
    let background = std::env::args().any(|a| a == "--background");
    let mut recent: Vec<Value> = load_json("desktop-recent.json");
    recent.truncate(RECENT_MAX);
    let shared = Arc::new(Shared {
        tx: Mutex::new(tx),
        snap: Mutex::new(Snap {
            recent,
            platform: platform(),
            default_dir: dirs::home_dir().unwrap_or_default().join("Vaultix").to_string_lossy().into_owned(),
            ..Default::default()
        }),
        dirty: AtomicBool::new(false),
        prefs: Mutex::new(load_json("desktop.json")),
        quitting: AtomicBool::new(false),
        popover_hidden_at: AtomicU64::new(0),
        popover_shown_at: AtomicU64::new(0),
        pause_item: Mutex::new(None),
        last_phase: Mutex::new(String::new()),
        state_seen: AtomicBool::new(false),
    });

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let configured = app.state::<Arc<Shared>>().snap.lock().unwrap().state["configured"].as_bool().unwrap_or(false);
            show_window(app, if configured { "popover" } else { "setup" });
        }))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(shared.clone())
        .invoke_handler(tauri::generate_handler![get_snapshot, engine, open_window, close_window, open_target, pick_folder, set_pref, quit])
        .setup(move |app| {
            let h = app.handle().clone();
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // Tray
            let open = MenuItem::with_id(app, "open", "Open Vaultix", true, None::<&str>)?;
            let sync = MenuItem::with_id(app, "sync", "Sync Now", true, None::<&str>)?;
            let pause = MenuItem::with_id(app, "pause", "Pause Syncing", true, None::<&str>)?;
            let folder = MenuItem::with_id(app, "folder", "Open Folder", true, None::<&str>)?;
            let web = MenuItem::with_id(app, "web", "Open in Browser", true, None::<&str>)?;
            let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit Vaultix", true, None::<&str>)?;
            let sep = || PredefinedMenuItem::separator(app);
            let menu = Menu::with_items(app, &[&open, &sep()?, &sync, &pause, &sep()?, &folder, &web, &settings, &sep()?, &quit])?;
            *shared.pause_item.lock().unwrap() = Some(pause);
            TrayIconBuilder::with_id("main")
                .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .tooltip("Vaultix")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| {
                    let s = app.state::<Arc<Shared>>().inner().clone();
                    match e.id.as_ref() {
                        "open" => {
                            let configured = s.snap.lock().unwrap().state["configured"].as_bool().unwrap_or(false);
                            show_window(app, if configured { "popover" } else { "setup" })
                        }
                        "sync" => s.send(json!({ "cmd": "sync_now" })),
                        "pause" => {
                            let paused = s.snap.lock().unwrap().state["paused"].as_bool().unwrap_or(false);
                            s.send(json!({ "cmd": "pause", "paused": !paused }));
                        }
                        "folder" => { let _ = open_target(app.clone(), "folder".into(), None); }
                        "web" => { let _ = open_target(app.clone(), "web".into(), None); }
                        "settings" => show_window(app, "settings"),
                        "quit" => quit_app(app),
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = e {
                        let app = tray.app_handle();
                        let configured = app.state::<Arc<Shared>>().snap.lock().unwrap().state["configured"].as_bool().unwrap_or(false);
                        if configured { show_popover(app, Some(rect)) } else { show_window(app, "setup") }
                    }
                })
                .build(app)?;

            // Pre-create the popover so it opens instantly.
            popover(&h);

            // Sync engine, in-process.
            let sink_app = h.clone();
            let sink: vaultix_sync::sync::Sink = Arc::new(move |v: Value| handle_event(&sink_app, v));
            std::thread::spawn(move || {
                if let Err(e) = vaultix_sync::agent::serve(rx, sink) {
                    vaultix_sync::config::log(&format!("agent stopped: {e:#}"));
                }
            });

            // Push UI updates at most 4×/s: big syncs emit thousands of events per second.
            let fl = h.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(250));
                let s = fl.state::<Arc<Shared>>();
                if s.dirty.swap(false, Ordering::Relaxed) {
                    let _ = fl.emit("snapshot", snapshot(&fl));
                }
                if s.quitting.load(Ordering::Relaxed) {
                    break;
                }
            });

            // Persist the recent list now and then.
            let pr = h.clone();
            std::thread::spawn(move || {
                let mut last = Instant::now();
                loop {
                    std::thread::sleep(Duration::from_secs(5));
                    if last.elapsed() > Duration::from_secs(30) {
                        save_json("desktop-recent.json", &pr.state::<Arc<Shared>>().snap.lock().unwrap().recent);
                        last = Instant::now();
                    }
                }
            });

            if !background {
                // Launched by hand: show something (setup appears automatically when signed out).
                let h2 = h.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(600));
                    let configured = h2.state::<Arc<Shared>>().snap.lock().unwrap().state["configured"].as_bool().unwrap_or(false);
                    if configured {
                        on_main(&h2, |a| show_window(a, "popover"));
                    }
                });
            }
            if let Ok(v) = std::env::var("VAULTIX_SHOW") {
                // Testing aid: VAULTIX_SHOW=settings opens that window at launch.
                let h3 = h.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(900));
                    on_main(&h3, move |a| show_window(a, &v));
                });
            }
            Ok(())
        })
        .on_window_event(|w, e| match e {
            WindowEvent::Focused(false) if w.label() == "popover" => {
                let s = w.app_handle().state::<Arc<Shared>>();
                // Some Linux window managers report a focus loss right as the window maps.
                if now_ms().saturating_sub(s.popover_shown_at.load(Ordering::Relaxed)) < 400 {
                    return;
                }
                let _ = w.hide();
                s.popover_hidden_at.store(now_ms(), Ordering::Relaxed);
            }
            WindowEvent::CloseRequested { api, .. } if w.label() == "popover" => {
                api.prevent_close();
                let _ = w.hide();
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("failed to start Vaultix");

    app.run(|app, e| {
        if let RunEvent::ExitRequested { api, code, .. } = e {
            // Closing the last window keeps Vaultix running in the tray; only Quit exits.
            if code.is_none() && !app.state::<Arc<Shared>>().quitting.load(Ordering::Relaxed) {
                api.prevent_exit();
            }
        }
    });
}
