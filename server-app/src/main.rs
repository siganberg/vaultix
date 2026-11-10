//! Vaultix Server: host a Vaultix drive on this computer without Docker.
//!
//! A tray app that runs the server in-process (`vaultix_server::open` + `serve`) and shows one
//! small window: status, the address other devices should use, where files are kept, the port.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_opener::OpenerExt as _;

const DEFAULT_PORT: u16 = 8085;

#[derive(Serialize, Deserialize, Clone)]
struct Config {
    data_dir: PathBuf,
    port: u16,
    /// Reachable from other devices on the network (false = this computer only).
    lan: bool,
}

fn config_dir() -> PathBuf {
    std::env::var_os("VAULTIX_SERVER_CONFIG").map(PathBuf::from).unwrap_or_else(|| dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("VaultixServer"))
}
fn config_path() -> PathBuf {
    config_dir().join("config.json")
}
fn log_path() -> PathBuf {
    config_dir().join("server.log")
}
fn default_data_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join("Vaultix Server")
}
fn load_config() -> Option<Config> {
    serde_json::from_slice(&std::fs::read(config_path()).ok()?).ok()
}
fn save_config(c: &Config) -> anyhow::Result<()> {
    std::fs::create_dir_all(config_dir())?;
    let tmp = config_path().with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(c)?)?;
    std::fs::rename(tmp, config_path())?;
    Ok(())
}

#[derive(Clone, PartialEq)]
enum Status {
    Stopped,
    Starting,
    Running,
    Error(String),
}

struct Running {
    stop: tokio::sync::oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
    state: vaultix_server::S,
}

struct App {
    rt: tokio::runtime::Runtime,
    cfg: Mutex<Option<Config>>,
    status: Mutex<Status>,
    running: Mutex<Option<Running>>,
    quitting: AtomicBool,
    toggle_item: Mutex<Option<MenuItem<tauri::Wry>>>,
}

/// The address other devices on the network should use: this machine's primary LAN IP.
fn lan_ip() -> Option<String> {
    // No packets are sent; connecting a UDP socket just selects the outgoing interface.
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_loopback() && !ip.is_unspecified()).then(|| ip.to_string())
}

fn friendly_error(e: &anyhow::Error, port: u16) -> String {
    let s = format!("{e:#}");
    let l = s.to_lowercase();
    if l.contains("in use") || l.contains("os error 48") || l.contains("os error 98") || l.contains("os error 10048") {
        format!("Port {port} is already used by another app. Choose a different port.")
    } else if l.contains("permission denied") || l.contains("os error 13") || l.contains("os error 10013") {
        if port < 1024 {
            format!("Port {port} needs administrator rights. Choose a port above 1024, like 8085.")
        } else {
            format!("Vaultix isn't allowed to use this folder or port: {s}")
        }
    } else {
        s
    }
}

/* ---------------- server lifecycle ---------------- */

fn set_status(app: &AppHandle, st: Status) {
    *app.state::<Arc<App>>().status.lock().unwrap() = st;
    refresh(app);
}

fn start_server(app: &AppHandle) {
    let a = app.state::<Arc<App>>().inner().clone();
    if a.running.lock().unwrap().is_some() {
        return;
    }
    let Some(cfg) = a.cfg.lock().unwrap().clone() else { return };
    set_status(app, Status::Starting);
    let opts = vaultix_server::Options {
        data_dir: cfg.data_dir.clone(),
        bind: format!("{}:{}", if cfg.lan { "0.0.0.0" } else { "127.0.0.1" }, cfg.port),
        cookie_secure: "auto".into(),
        trash_days: 30,
        watch: true,
    };
    let state = match vaultix_server::open(&opts) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("can't open data folder: {e:#}");
            return set_status(app, Status::Error(format!("Can't use the folder {}: {e}", cfg.data_dir.display())));
        }
    };
    let (stop_tx, stop_rx) = tokio::sync::oneshot::channel::<()>();
    let (app2, st2, mine, port) = (app.clone(), state.clone(), state.clone(), cfg.port);
    let task = a.rt.spawn(async move {
        let serving = vaultix_server::serve(st2, opts, async {
            let _ = stop_rx.await;
        });
        tokio::pin!(serving);
        // If it's still serving after a moment, the port was bound successfully.
        let early = tokio::select! {
            r = &mut serving => Some(r),
            _ = tokio::time::sleep(Duration::from_millis(400)) => None,
        };
        let result = match early {
            Some(r) => r,
            None => {
                set_status(&app2, Status::Running);
                serving.await
            }
        };
        let a = app2.state::<Arc<App>>();
        {
            // Clear our own entry only (a restart may already have put a new server here).
            let mut r = a.running.lock().unwrap();
            if r.as_ref().is_some_and(|r| Arc::ptr_eq(&r.state, &mine)) {
                r.take();
            }
        }
        match result {
            Ok(()) => set_status(&app2, Status::Stopped),
            Err(e) => {
                tracing::error!("server stopped: {e:#}");
                set_status(&app2, Status::Error(friendly_error(&e, port)));
            }
        }
    });
    *a.running.lock().unwrap() = Some(Running { stop: stop_tx, task, state });
}

/// Ask the server to stop and wait (briefly) until it has.
fn stop_server(app: &AppHandle) {
    let a = app.state::<Arc<App>>().inner().clone();
    let Some(r) = a.running.lock().unwrap().take() else { return };
    let _ = r.stop.send(());
    let task = r.task;
    drop(r.state);
    // Open downloads can hold graceful shutdown; don't wait forever.
    let _ = a.rt.block_on(async { tokio::time::timeout(Duration::from_secs(4), task).await });
    if !matches!(*a.status.lock().unwrap(), Status::Error(_)) {
        set_status(app, Status::Stopped);
    }
}

/* ---------------- state for the UI ---------------- */

#[derive(Serialize, Clone)]
struct View {
    configured: bool,
    status: &'static str,
    error: Option<String>,
    data_dir: String,
    port: u16,
    lan: bool,
    local_url: String,
    lan_url: Option<String>,
    host_url: Option<String>,
    users: i64,
    autostart: bool,
    version: &'static str,
    platform: &'static str,
}

fn view(app: &AppHandle) -> View {
    let a = app.state::<Arc<App>>();
    let cfg = a.cfg.lock().unwrap().clone();
    let configured = cfg.is_some();
    let c = cfg.unwrap_or(Config { data_dir: default_data_dir(), port: DEFAULT_PORT, lan: true });
    let (status, error) = match &*a.status.lock().unwrap() {
        Status::Stopped => ("stopped", None),
        Status::Starting => ("starting", None),
        Status::Running => ("running", None),
        Status::Error(e) => ("error", Some(e.clone())),
    };
    let users = a.running.lock().unwrap().as_ref().map(|r| vaultix_server::user_count(&r.state)).unwrap_or(-1);
    let host = hostname::get().ok().map(|h| h.to_string_lossy().into_owned()).filter(|h| !h.is_empty() && h != "localhost");
    View {
        configured,
        status,
        error,
        data_dir: c.data_dir.to_string_lossy().into_owned(),
        port: c.port,
        lan: c.lan,
        local_url: format!("http://localhost:{}", c.port),
        lan_url: c.lan.then(lan_ip).flatten().map(|ip| format!("http://{ip}:{}", c.port)),
        host_url: if c.lan { host.map(|h| format!("http://{}:{}", h.to_lowercase(), c.port)) } else { None },
        users,
        autostart: app.autolaunch().is_enabled().unwrap_or(false),
        version: env!("CARGO_PKG_VERSION"),
        platform: if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "macos" } else { "linux" },
    }
}

/// Push the current state to the window and the tray.
fn refresh(app: &AppHandle) {
    let v = view(app);
    let a = app.state::<Arc<App>>();
    if let Some(t) = app.tray_by_id("main") {
        let (icon, tip): (&[u8], String) = match v.status {
            "running" => (include_bytes!("../icons/tray.png"), format!("Vaultix Server: running on port {}", v.port)),
            "error" => (include_bytes!("../icons/tray-error.png"), "Vaultix Server: problem, click for details".into()),
            "starting" => (include_bytes!("../icons/tray.png"), "Vaultix Server: starting…".into()),
            _ => (include_bytes!("../icons/tray-stopped.png"), "Vaultix Server: stopped".into()),
        };
        let _ = t.set_tooltip(Some(tip));
        if let Ok(img) = Image::from_bytes(icon) {
            let _ = t.set_icon(Some(img));
        }
    }
    let item = a.toggle_item.lock().unwrap().clone();
    if let Some(item) = item {
        let _ = item.set_text(if matches!(v.status, "running" | "starting") { "Stop Server" } else { "Start Server" });
    }
    let _ = app.emit("state", v);
}

fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = WebviewWindowBuilder::new(app, "main", WebviewUrl::App("index.html".into()))
        .title("Vaultix Server")
        .inner_size(460.0, 720.0)
        .resizable(false)
        .maximizable(false)
        .center()
        .focused(true)
        .build();
}

fn on_main(app: &AppHandle, f: impl FnOnce(&AppHandle) + Send + 'static) {
    let a = app.clone();
    let _ = app.run_on_main_thread(move || f(&a));
}

/* ---------------- commands ---------------- */

#[tauri::command]
fn get_state(app: AppHandle) -> View {
    view(&app)
}

#[derive(Deserialize)]
struct Settings {
    data_dir: String,
    port: u16,
    lan: bool,
}

/// Save settings (first run, or a change) and (re)start the server with them.
#[tauri::command]
async fn apply_settings(app: AppHandle, settings: Settings) -> Result<(), String> {
    let dir = PathBuf::from(settings.data_dir.trim());
    if settings.data_dir.trim().is_empty() || !dir.is_absolute() {
        return Err("Choose a folder for your files.".into());
    }
    if settings.port == 0 {
        return Err("Choose a port between 1 and 65535 (8085 is a good default).".into());
    }
    std::fs::create_dir_all(&dir).map_err(|e| format!("Can't create {}: {e}", dir.display()))?;
    let cfg = Config { data_dir: dir.canonicalize().unwrap_or(dir), port: settings.port, lan: settings.lan };
    save_config(&cfg).map_err(|e| format!("Couldn't save settings: {e}"))?;
    let a = app.state::<Arc<App>>().inner().clone();
    let app2 = app.clone();
    tokio::task::spawn_blocking(move || {
        stop_server(&app2);
        *a.cfg.lock().unwrap() = Some(cfg);
        start_server(&app2);
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn server(app: AppHandle, action: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || match action.as_str() {
        "start" => start_server(&app),
        "stop" => stop_server(&app),
        _ => {
            stop_server(&app);
            start_server(&app);
        }
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_target(app: AppHandle, what: String) -> Result<(), String> {
    let v = view(&app);
    let o = app.opener();
    match what.as_str() {
        "web" => o.open_url(v.local_url, None::<&str>),
        "data" => o.open_path(v.data_dir, None::<&str>),
        "files" => {
            // One account → straight to its files; otherwise the folder that holds everyone's.
            let users = PathBuf::from(&v.data_dir).join("users");
            let dirs: Vec<PathBuf> = std::fs::read_dir(&users).map(|d| d.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect()).unwrap_or_default();
            let target = if dirs.len() == 1 { dirs[0].join("files") } else { users };
            o.open_path(target.to_string_lossy(), None::<&str>)
        }
        "log" => o.open_path(log_path().to_string_lossy(), None::<&str>),
        "help" => o.open_url("https://github.com/siganberg/vaultix#making-it-reachable-from-anywhere-https", None::<&str>),
        "apps" => o.open_url("https://github.com/siganberg/vaultix/releases/latest", None::<&str>),
        _ => return Err("unknown target".into()),
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn pick_folder(app: AppHandle, start: Option<String>) -> Option<String> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = std::sync::mpsc::channel();
    let mut d = app.dialog().file().set_title("Where should Vaultix keep its files?");
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
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    let al = app.autolaunch();
    if enabled { al.enable() } else { al.disable() }.map_err(|e| e.to_string())?;
    refresh(&app);
    Ok(())
}

#[tauri::command]
fn hide_window(window: tauri::WebviewWindow) {
    let _ = window.hide();
}

#[tauri::command]
async fn quit(app: AppHandle) {
    quit_app(&app);
}

fn quit_app(app: &AppHandle) {
    let a = app.state::<Arc<App>>().inner().clone();
    if a.quitting.swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        stop_server(&app);
        app.exit(0);
    });
}

/* ---------------- main ---------------- */

fn init_logging() {
    let _ = std::fs::create_dir_all(config_dir());
    let p = log_path();
    if std::fs::metadata(&p).map(|m| m.len() > 5_000_000).unwrap_or(false) {
        let _ = std::fs::rename(&p, p.with_extension("log.1"));
    }
    if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).open(&p) {
        let _ = tracing_subscriber::fmt().with_ansi(false).with_writer(Mutex::new(f)).with_env_filter(tracing_subscriber::EnvFilter::new("info")).try_init();
    }
}

fn main() {
    init_logging();
    let background = std::env::args().any(|a| a == "--background");
    let state = Arc::new(App {
        rt: tokio::runtime::Builder::new_multi_thread().enable_all().thread_name("vaultix-server").build().expect("tokio runtime"),
        cfg: Mutex::new(load_config()),
        status: Mutex::new(Status::Stopped),
        running: Mutex::new(None),
        quitting: AtomicBool::new(false),
        toggle_item: Mutex::new(None),
    });

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_window(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, Some(vec!["--background"])))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![get_state, apply_settings, server, open_target, pick_folder, set_autostart, hide_window, quit])
        .setup(move |app| {
            let h = app.handle().clone();
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let open = MenuItem::with_id(app, "open", "Open Vaultix in Browser", true, None::<&str>)?;
            let show = MenuItem::with_id(app, "show", "Server Settings…", true, None::<&str>)?;
            let toggle = MenuItem::with_id(app, "toggle", "Stop Server", true, None::<&str>)?;
            let files = MenuItem::with_id(app, "files", "Open Files Folder", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit (stops the server)", true, None::<&str>)?;
            let sep = || PredefinedMenuItem::separator(app);
            let menu = Menu::with_items(app, &[&open, &show, &sep()?, &toggle, &files, &sep()?, &quit])?;
            *state.toggle_item.lock().unwrap() = Some(toggle);
            TrayIconBuilder::with_id("main")
                .icon(Image::from_bytes(include_bytes!("../icons/tray-stopped.png"))?)
                .tooltip("Vaultix Server")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => {
                        let _ = open_target(app.clone(), "web".into());
                    }
                    "show" => show_window(app),
                    "files" => {
                        let _ = open_target(app.clone(), "files".into());
                    }
                    "toggle" => {
                        let app = app.clone();
                        std::thread::spawn(move || {
                            let running = app.state::<Arc<App>>().running.lock().unwrap().is_some();
                            if running { stop_server(&app) } else { start_server(&app) }
                        });
                    }
                    "quit" => quit_app(app),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        show_window(tray.app_handle());
                    }
                })
                .build(app)?;

            let configured = state.cfg.lock().unwrap().is_some();
            if configured {
                let h2 = h.clone();
                std::thread::spawn(move || start_server(&h2));
            }
            // First run always shows the window; later launches only when started by hand.
            if !configured || !background {
                show_window(&h);
            }

            // Keep the user count and addresses fresh (network changes, first account created).
            let h3 = h.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_secs(3));
                if h3.state::<Arc<App>>().quitting.load(Ordering::Relaxed) {
                    break;
                }
                if h3.get_webview_window("main").and_then(|w| w.is_visible().ok()).unwrap_or(false) {
                    on_main(&h3, |a| {
                        let _ = a.emit("state", view(a));
                    });
                }
            });
            Ok(())
        })
        .on_window_event(|w, e| {
            // Closing the window keeps the server running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = e {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("failed to start Vaultix Server");

    app.run(|app, e| {
        if let RunEvent::ExitRequested { api, code, .. } = e {
            if code.is_none() && !app.state::<Arc<App>>().quitting.load(Ordering::Relaxed) {
                api.prevent_exit();
            }
        }
    });
}
