#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod config;
#[cfg(debug_assertions)]
mod demo;
mod search;
mod steam;
mod vdf;

use config::{log, Config, APP_NAME};
use search::{Hit, Searcher};
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use steam::Game;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewWindow, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

const WINDOW: &str = "search";
const TRAY: &str = "main";
/// 40rem at the default 16px font size.
const WINDOW_WIDTH: f64 = 640.0;
const VERSION: &str = match option_env!("APP_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

#[derive(Default)]
struct Library {
    loaded: bool,
    steam_found: bool,
    games: Vec<Game>,
}

struct AppState {
    library: Mutex<Library>,
    searcher: Mutex<Searcher>,
    config: Mutex<Config>,
    scanning: AtomicBool,
    /// Logical height requested by the page, reused when the window moves to another monitor.
    height: Mutex<f64>,
    shown_at: Mutex<Option<Instant>>,
}

#[derive(Serialize)]
struct SearchResult {
    /// "loading", "no-steam", "no-games", "no-match" or "ok".
    status: &'static str,
    hits: Vec<Hit>,
}

#[tauri::command]
fn search(query: String, state: State<AppState>) -> SearchResult {
    let library = state.library.lock().unwrap();
    let status = if !library.loaded {
        "loading"
    } else if !library.steam_found {
        "no-steam"
    } else if library.games.is_empty() {
        "no-games"
    } else {
        "ok"
    };
    if status != "ok" || query.trim().is_empty() {
        return SearchResult { status, hits: Vec::new() };
    }
    let limit = state.config.lock().unwrap().max_results;
    let hits = state.searcher.lock().unwrap().search(&library.games, &query, limit);
    SearchResult { status: if hits.is_empty() { "no-match" } else { "ok" }, hits }
}

#[tauri::command]
fn launch(appid: u32, app: AppHandle) -> Result<(), String> {
    let url = format!("steam://rungameid/{appid}");
    let result = app.opener().open_url(&url, None::<&str>).map_err(|e| e.to_string());
    match &result {
        Ok(()) => log(&format!("launched appid {appid}")),
        Err(e) => log(&format!("cannot launch appid {appid}: {e}")),
    }
    hide_search(&app);
    result
}

#[tauri::command]
fn hide(app: AppHandle) {
    hide_search(&app);
}

/// Resizes the window to the page content height (logical pixels), keeping its width.
#[tauri::command]
fn fit(height: f64, window: WebviewWindow, state: State<AppState>) {
    let height = height.clamp(1.0, 2000.0);
    *state.height.lock().unwrap() = height;
    if let (Ok(size), Ok(scale)) = (window.inner_size(), window.scale_factor()) {
        let _ = window.set_size(PhysicalSize::new(size.width, (height * scale).round() as u32));
    }
}

/// Called by the page once the search field has focus after "shown".
#[tauri::command]
fn ready(state: State<AppState>) {
    if let Some(start) = state.shown_at.lock().unwrap().take() {
        log(&format!("search field ready {:.1} ms after activation", start.elapsed().as_secs_f64() * 1000.0));
    }
}

fn show_search(app: &AppHandle) {
    let Some(window) = app.get_webview_window(WINDOW) else { return };
    let state = app.state::<AppState>();
    *state.shown_at.lock().unwrap() = Some(Instant::now());
    place_on_cursor_monitor(app, &window, *state.height.lock().unwrap());
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit_to(WINDOW, "shown", ());
    start_scan(app);
}

fn hide_search(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WINDOW) {
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            let _ = app.emit_to(WINDOW, "hidden", ());
        }
    }
}

fn toggle_search(app: &AppHandle) {
    let visible = app.get_webview_window(WINDOW).and_then(|w| w.is_visible().ok()).unwrap_or(false);
    if visible {
        hide_search(app);
    } else {
        show_search(app);
    }
}

/// Centers the window horizontally in the work area of the monitor under the cursor,
/// in the upper third, sized for that monitor's scale factor.
fn place_on_cursor_monitor(app: &AppHandle, window: &WebviewWindow, height: f64) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|c| app.monitor_from_point(c.x, c.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let width = ((WINDOW_WIDTH * scale).round() as u32).min(area.size.width * 9 / 10);
    let x = area.position.x + (area.size.width - width) as i32 / 2;
    let y = area.position.y + area.size.height as i32 / 5;
    // Move first so a DPI change between monitors is applied before sizing.
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(width, (height * scale).round() as u32));
}

/// Rescans the libraries on a background thread. The UI keeps using the cached list meanwhile.
fn start_scan(app: &AppHandle) {
    let state = app.state::<AppState>();
    if state.scanning.swap(true, Ordering::SeqCst) {
        return;
    }
    let steam_path = state.config.lock().unwrap().steam_path.clone();
    let app = app.clone();
    std::thread::spawn(move || {
        let (steam_found, games) = scan(&steam_path);
        let state = app.state::<AppState>();
        let changed = {
            let mut library = state.library.lock().unwrap();
            let changed = !library.loaded || library.steam_found != steam_found || library.games != games;
            if changed {
                log(&match steam_found {
                    true => format!("library scan: {} installed games", games.len()),
                    false => "Steam installation not found".to_string(),
                });
                *library = Library { loaded: true, steam_found, games };
            }
            changed
        };
        state.scanning.store(false, Ordering::SeqCst);
        if changed {
            let _ = app.emit_to(WINDOW, "library-updated", ());
        }
    });
}

fn scan(steam_path: &str) -> (bool, Vec<Game>) {
    #[cfg(debug_assertions)]
    if std::env::var_os("STEAM_SEARCH_DEMO").is_some() {
        return (true, demo::games());
    }
    match steam::find_steam_path(steam_path) {
        Some(path) => (true, steam::installed_games(&path)),
        None => (false, Vec::new()),
    }
}

/// Registers the configured hotkey and reports failures in the tray tooltip and the log.
fn apply_hotkey(app: &AppHandle) {
    let hotkey = app.state::<AppState>().config.lock().unwrap().hotkey.clone();
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    let tooltip = match config::parse_hotkey(&hotkey).map(|s| shortcuts.register(s)) {
        Some(Ok(())) => {
            log(&format!("hotkey {hotkey} registered"));
            format!("{APP_NAME} ({hotkey})")
        }
        Some(Err(e)) => {
            log(&format!("cannot register hotkey {hotkey}, it may be used by another application: {e}"));
            format!("{APP_NAME}: hotkey {hotkey} is unavailable (used by another application?)")
        }
        None => {
            log(&format!("invalid hotkey {hotkey}"));
            format!("{APP_NAME}: invalid hotkey {hotkey}")
        }
    };
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_tooltip(Some(tooltip));
    }
}

fn reload_settings(app: &AppHandle) {
    let config = config::load();
    log("settings reloaded");
    *app.state::<AppState>().config.lock().unwrap() = config;
    apply_hotkey(app);
    start_scan(app);
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let autostart_item =
        CheckMenuItem::with_id(app, "autostart", "Start with Windows", true, autostart::is_enabled(), None::<&str>)?;
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", "Open search", true, None::<&str>)?,
            &MenuItem::with_id(app, "rescan", "Rescan library", true, None::<&str>)?,
            &autostart_item,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "config", "Open config file", true, None::<&str>)?,
            &MenuItem::with_id(app, "reload", "Reload settings", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "version", format!("Version {VERSION}"), false, None::<&str>)?,
            &MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
        ],
    )?;
    TrayIconBuilder::with_id(TRAY)
        .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
        .tooltip(APP_NAME)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => show_search(app),
            "rescan" => start_scan(app),
            "autostart" => {
                let wanted = autostart_item.is_checked().unwrap_or(false);
                match autostart::set_enabled(wanted) {
                    Ok(()) => log(&format!("start with Windows {}", if wanted { "enabled" } else { "disabled" })),
                    Err(e) => {
                        log(&format!("cannot change start with Windows: {e}"));
                        let _ = autostart_item.set_checked(autostart::is_enabled());
                    }
                }
            }
            "config" => {
                let _ = config::load(); // creates the file if it was deleted
                if let Err(e) = app.opener().open_path(config::config_path().to_string_lossy(), None::<&str>) {
                    log(&format!("cannot open config.json: {e}"));
                }
            }
            "reload" => reload_settings(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_search(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_search(app)))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state() == ShortcutState::Pressed {
                        toggle_search(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .manage(AppState {
            library: Mutex::new(Library::default()),
            searcher: Mutex::new(Searcher::new()),
            config: Mutex::new(config::load()),
            scanning: AtomicBool::new(false),
            height: Mutex::new(58.0),
            shown_at: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![search, launch, hide, fit, ready])
        .setup(|app| {
            log(&format!("{APP_NAME} {VERSION} started"));
            build_tray(app.handle())?;
            apply_hotkey(app.handle());
            start_scan(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::Focused(false) => hide_search(window.app_handle()),
            WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                hide_search(window.app_handle());
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
