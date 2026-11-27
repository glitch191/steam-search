#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod autostart;
mod config;
#[cfg(debug_assertions)]
mod demo;
mod fonts;
mod search;
mod steam;
mod vdf;

use config::{log, Config, APP_NAME};
use search::{Hit, Searcher};
use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Instant;
use steam::Game;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_opener::OpenerExt;

const WINDOW: &str = "search";
const SETTINGS: &str = "settings";
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
    /// Physical bottom edge to keep fixed while resizing, when the window sits above the taskbar.
    bottom_edge: Mutex<Option<i32>>,
    shown_at: Mutex<Option<Instant>>,
    /// True while the settings window waits for a new hotkey; the global hotkey is unregistered meanwhile.
    capturing: AtomicBool,
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
        let new_height = (height * scale).round() as u32;
        // Above a bottom taskbar the window grows upward: keep its bottom edge in place.
        if let (Some(bottom), Ok(pos)) = (*state.bottom_edge.lock().unwrap(), window.outer_position()) {
            let _ = window.set_position(PhysicalPosition::new(pos.x, bottom - new_height as i32));
        }
        let _ = window.set_size(PhysicalSize::new(size.width, new_height));
    }
}

/// Applies a hotkey captured by the settings window. On failure the previous hotkey is kept
/// and the error is returned for display; capture mode continues.
#[tauri::command]
fn set_hotkey(hotkey: String, app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    if config::parse_hotkey(&hotkey).is_none() {
        return Err(format!("{hotkey} is not a supported shortcut."));
    }
    let previous = std::mem::replace(&mut state.config.lock().unwrap().hotkey, hotkey.clone());
    if !apply_hotkey(&app) {
        state.config.lock().unwrap().hotkey = previous;
        // Keep waiting for another combination: the hotkey stays unregistered while capturing.
        let _ = app.global_shortcut().unregister_all();
        return Err(format!("{hotkey} is already used by another application. Try another shortcut."));
    }
    state.capturing.store(false, Ordering::SeqCst);
    config::save(&state.config.lock().unwrap());
    log(&format!("hotkey changed to {hotkey} from the settings window"));
    Ok(())
}

/// Releases the global hotkey so that pressing the current combination reaches the settings window.
#[tauri::command]
fn begin_hotkey_capture(app: AppHandle) {
    app.state::<AppState>().capturing.store(true, Ordering::SeqCst);
    let _ = app.global_shortcut().unregister_all();
}

/// Leaves capture mode without a new hotkey: the current one is registered again.
#[tauri::command]
fn cancel_hotkey_capture(app: AppHandle) {
    end_hotkey_capture(&app);
}

fn end_hotkey_capture(app: &AppHandle) {
    if app.state::<AppState>().capturing.swap(false, Ordering::SeqCst) {
        apply_hotkey(app);
    }
}

/// What the settings window shows. Field names match config.json.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SettingsView {
    hotkey: String,
    max_results: usize,
    steam_path: String,
    /// Actual state of the startup entry, which Settings or Task Manager may have turned off.
    start_with_windows: bool,
    font_family: String,
    font_size: u32,
    /// Steam folder in use (configured or detected), empty when none is found.
    steam_found_at: String,
    /// A few installed game names for the font preview.
    sample_games: Vec<String>,
}

fn settings_view(app: &AppHandle) -> SettingsView {
    let state = app.state::<AppState>();
    let config = state.config.lock().unwrap().clone();
    let steam_found_at =
        steam::find_steam_path(&config.steam_path).map(|p| p.display().to_string()).unwrap_or_default();
    let mut sample_games: Vec<String> = state.library.lock().unwrap().games.iter().map(|g| g.name.clone()).collect();
    sample_games.sort_by_key(|n| n.to_lowercase());
    sample_games.truncate(4);
    SettingsView {
        hotkey: config.hotkey,
        max_results: config.max_results,
        steam_path: config.steam_path,
        start_with_windows: autostart::is_enabled(),
        font_family: config.font_family,
        font_size: config.font_size,
        steam_found_at,
        sample_games,
    }
}

#[tauri::command]
fn get_settings(app: AppHandle) -> SettingsView {
    settings_view(&app)
}

/// Installed font families. Async so the DirectWrite enumeration runs off the main thread.
#[tauri::command]
async fn font_families() -> Vec<String> {
    fonts::installed_families()
}

/// Applies and saves the settings in `patch` (config.json key names, hotkey excluded).
/// Returns the settings as stored, so the window shows values that fell back to their default.
#[tauri::command]
fn save_settings(patch: Value, app: AppHandle) -> SettingsView {
    let state = app.state::<AppState>();
    let (old, new) = {
        let mut config = state.config.lock().unwrap();
        let mut new = config::merge(&config, &patch);
        new.hotkey = config.hotkey.clone();
        (std::mem::replace(&mut *config, new.clone()), new)
    };
    config::save(&new);
    if patch.get("startWithWindows").is_some() {
        // Turned on from here, the entry is enabled even if Task Manager had disabled it.
        match autostart::set_enabled(new.start_with_windows, true) {
            Ok(()) => log(&format!("start with Windows {}", if new.start_with_windows { "enabled" } else { "disabled" })),
            Err(e) => log(&format!("cannot change start with Windows: {e}")),
        }
    }
    if (&old.font_family, old.font_size) != (&new.font_family, new.font_size) {
        let _ = app.emit_to(WINDOW, "appearance-changed", ());
    }
    if old.steam_path != new.steam_path {
        start_scan(&app);
    }
    settings_view(&app)
}

/// Opens the settings window, or brings it to the front when it is already open.
fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return;
    }
    let result = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("steam-search settings")
        .inner_size(720.0, 640.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .center()
        .focused(true)
        .build();
    if let Err(e) = result {
        log(&format!("cannot open the settings window: {e}"));
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Appearance {
    font_family: String,
    font_size: u32,
}

/// Font settings of the game list, read by the page at startup and after "appearance-changed".
#[tauri::command]
fn appearance(state: State<AppState>) -> Appearance {
    let config = state.config.lock().unwrap();
    Appearance { font_family: config.font_family.clone(), font_size: config.font_size }
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
    place_near_tray(app, &window, *state.height.lock().unwrap());
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

/// Places the window just above the tray icon (or below it when the taskbar is at the top),
/// inside the work area of the monitor that holds the tray, sized for that monitor's scale factor.
/// Falls back to the bottom right corner of the primary monitor when the icon position is unknown.
fn place_near_tray(app: &AppHandle, window: &WebviewWindow, height: f64) {
    let icon = app.tray_by_id(TRAY).and_then(|t| t.rect().ok().flatten()).map(|r| {
        let pos = r.position.to_physical::<i32>(1.0);
        let size = r.size.to_physical::<u32>(1.0);
        (pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2)
    });
    let monitor = icon
        .and_then(|(x, y)| app.monitor_from_point(x as f64, y as f64).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let area = monitor.work_area();
    let scale = monitor.scale_factor();
    let margin = (8.0 * scale).round() as i32;
    let width = ((WINDOW_WIDTH * scale).round() as u32).min(area.size.width * 9 / 10);
    let height_px = (height * scale).round() as u32;
    let (left, top) = (area.position.x, area.position.y);
    let (right, bottom) = (left + area.size.width as i32, top + area.size.height as i32);

    let (center_x, icon_y) = icon.unwrap_or((right, bottom));
    let x = (center_x - width as i32 / 2).clamp(left + margin, right - width as i32 - margin);
    let taskbar_at_top = icon_y < top + area.size.height as i32 / 2;
    let (y, bottom_edge) = if taskbar_at_top {
        (top + margin, None)
    } else {
        (bottom - margin - height_px as i32, Some(bottom - margin))
    };
    *app.state::<AppState>().bottom_edge.lock().unwrap() = bottom_edge;
    // Move first so a DPI change between monitors is applied before sizing.
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.set_size(PhysicalSize::new(width, height_px));
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

/// Registers the configured hotkey, shows it in the tray menu, and reports failures in the
/// tray tooltip and the log. Returns true when the hotkey is registered.
fn apply_hotkey(app: &AppHandle) -> bool {
    let hotkey = app.state::<AppState>().config.lock().unwrap().hotkey.clone();
    let shortcuts = app.global_shortcut();
    let _ = shortcuts.unregister_all();
    let result = config::parse_hotkey(&hotkey).map(|s| shortcuts.register(s));
    let registered = matches!(result, Some(Ok(())));
    let tooltip = match result {
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
    registered
}

/// Applies startWithWindows in release builds only, so dev builds never register themselves.
fn sync_start_with_windows(config: &Config) {
    if cfg!(not(debug_assertions)) {
        config::apply_start_with_windows(config);
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "open", "Open search", true, None::<&str>)?,
            &MenuItem::with_id(app, "rescan", "Rescan library", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "settings", "Settings...", true, None::<&str>)?,
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
            "settings" => show_settings(app),
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
            bottom_edge: Mutex::new(None),
            shown_at: Mutex::new(None),
            capturing: AtomicBool::new(false),
        })
        .invoke_handler(tauri::generate_handler![
            search,
            launch,
            hide,
            fit,
            ready,
            appearance,
            get_settings,
            font_families,
            save_settings,
            set_hotkey,
            begin_hotkey_capture,
            cancel_hotkey_capture
        ])
        .setup(|app| {
            log(&format!("{APP_NAME} {VERSION} started"));
            sync_start_with_windows(&app.state::<AppState>().config.lock().unwrap());
            build_tray(app.handle())?;
            apply_hotkey(app.handle());
            start_scan(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| match (window.label(), event) {
            (WINDOW, WindowEvent::Focused(false)) => hide_search(window.app_handle()),
            (WINDOW, WindowEvent::CloseRequested { api, .. }) => {
                api.prevent_close();
                hide_search(window.app_handle());
            }
            // Closing the settings window during a hotkey change cancels it.
            (SETTINGS, WindowEvent::Destroyed) => end_hotkey_capture(window.app_handle()),
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running the application");
}
