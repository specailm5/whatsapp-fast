use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_autostart::ManagerExt as AutoStartExt;
use tauri_plugin_deep_link::DeepLinkExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use url::Url;

const WHATSAPP_WEB_URL: &str = "https://web.whatsapp.com";

/// Desktop Chrome user-agent override so WhatsApp Web serves the full desktop client.
///
/// MAINTENANCE: the only version-sensitive part is the `Chrome/<major>.0.0.0` token.
/// When WhatsApp Web starts rejecting the shell, bump that major to a recent stable
/// Chrome version; keep the rest looking like a real Windows desktop Chrome.
const DESKTOP_USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";

/// Upper bound for the taskbar badge bitmap (per side), so a remote payload cannot
/// force a large allocation. The badge is rendered at most 64px (32px × dpr 2).
const MAX_BADGE_DIM: u32 = 256;

/// Minimum interval between geometry writes to disk while the window is being
/// dragged or resized. The final geometry is still written on close and on quit.
const GEOMETRY_SAVE_THROTTLE: Duration = Duration::from_millis(500);
const DEFAULT_HOTKEY: &str = "Control+Shift+Space";
const DEFAULT_BLUR_HOTKEY: &str = "Control+Shift+B";
const DEFAULT_PANIC_HOTKEY: &str = "Control+Shift+Z";

/// Shared Tauri IPC bridge helper (exposes window.__WF_INVOKE__ for the scripts below).
const BRIDGE_JS: &str = include_str!("bridge.js");

/// The privacy suite boot script (injected at document-start in the page context).
const PRIVACY_BOOT_JS: &str = include_str!("privacy_boot.js");

/// Embedded "Privacy Suite for WhatsApp Web" default-blur stylesheet (pure CSS that
/// blurs from the first paint). Injected as a <style> at document-start.
const PRIVACY_STYLES_CSS: &str = include_str!("styles.css");

/// Notification-privacy wrapper (wraps window.Notification; reads <html data-wa-notif>).
const PAGE_NOTIFY_JS: &str = include_str!("page-notify.js");

/// Nav-rail Settings entry (adds a gear item into the left rail that opens the
/// separate settings window).
const RAIL_SETTINGS_JS: &str = include_str!("rail_settings.js");

/// Taskbar unread-badge driver. Parses the live unread count out of the page's
/// `document.title` and pushes it (with a rendered badge PNG) to Rust.
const BADGE_JS: &str = include_str!("badge.js");

/// Log a best-effort operation's failure without panicking. Used for operations whose
/// failure is non-fatal but worth recording (persistence, eval, autostart, deep links).
fn log_err<T, E: std::fmt::Debug>(label: &str, result: Result<T, E>) {
    if let Err(e) = result {
        eprintln!("{label}: {e:?}");
    }
}

/// Convert a deep-link (whatsapp:// or https?://wa.me) into a WhatsApp Web send
/// URL. Pure + testable: takes the decoded URL pieces and returns the target or
/// None when the link is not a supported shape (or has nothing useful).
fn build_send_url(
    scheme: &str,
    host: Option<&str>,
    path: &str,
    phone: Option<&str>,
    text: Option<&str>,
) -> Option<String> {
    fn strip_plus(s: &str) -> &str {
        s.trim_start_matches('+')
    }

    let mut params: Vec<(&str, &str)> = Vec::new();
    if scheme == "whatsapp" {
        // Two accepted shapes:
        //   whatsapp://send?phone=12345&text=Hi   (phone as a query param)
        //   whatsapp://1234567890                 (phone as the host, no query)
        // Prefer the query `phone`; fall back to a phone-like host (must contain a
        // digit, so an alpha host such as `chat` is not mistaken for a phone).
        let host_phone = host.filter(|h| *h != "send" && h.bytes().any(|b| b.is_ascii_digit()));
        let p = phone.map(strip_plus).or_else(|| host_phone.map(strip_plus));
        if let Some(p) = p {
            params.push(("phone", p));
        }
        if let Some(t) = text {
            params.push(("text", t));
        }
    } else if (scheme == "https" || scheme == "http") && host == Some("wa.me") {
        // e.g. https://wa.me/1234567890 or https://wa.me/123456?text=Hi
        let raw = path.trim_start_matches('/').trim_end_matches('/');
        let phone = raw.split('/').next().filter(|x| !x.is_empty())?;
        params.push(("phone", strip_plus(phone)));
        if let Some(t) = text {
            params.push(("text", t));
        }
    } else {
        return None;
    }
    if params.is_empty() {
        return None;
    }
    // Serializer percent-encodes the values (so a text message's & = etc. are safe).
    let mut ser = url::form_urlencoded::Serializer::new(String::new());
    for (k, v) in params {
        ser.append_pair(k, v);
    }
    Some(format!("https://web.whatsapp.com/send?{}", ser.finish()))
}

/// Convert an incoming deep-link URL (whatsapp:// or https://wa.me) into a WhatsApp Web URL
/// that opens the correct chat, then focus the main window.
fn route_deep_link(app: &AppHandle, url: &Url) {
    let scheme = url.scheme().to_string();
    let host = url.host_str().map(|h| h.to_string());
    let path = url.path().to_string();
    // query_pairs() decodes percent-encoding for us.
    let mut phone: Option<String> = None;
    let mut text: Option<String> = None;
    for (k, v) in url.query_pairs() {
        match k.as_ref() {
            "phone" => phone = Some(v.into_owned()),
            "text" => text = Some(v.into_owned()),
            _ => {}
        }
    }
    if let Some(url_str) = build_send_url(
        &scheme,
        host.as_deref(),
        &path,
        phone.as_deref(),
        text.as_deref(),
    ) {
        if let (Ok(parsed), Some(win)) = (Url::parse(&url_str), app.get_webview_window("main")) {
            log_err("deep-link navigate", win.navigate(parsed));
            log_err("deep-link show", win.show());
            log_err("deep-link unminimize", win.unminimize());
            log_err("deep-link focus", win.set_focus());
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    autostart: bool,
    hotkey: String,
    blur_hotkey: String,
    panic_hotkey: String,
    unread_badge: bool,
    flash_on_new_message: bool,
    /// Close-to-tray instead of quitting when the main window is closed.
    close_to_tray: bool,
    /// Start the main window minimized to the taskbar on launch.
    start_minimized: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            autostart: false,
            hotkey: DEFAULT_HOTKEY.to_string(),
            blur_hotkey: DEFAULT_BLUR_HOTKEY.to_string(),
            panic_hotkey: DEFAULT_PANIC_HOTKEY.to_string(),
            unread_badge: true,
            flash_on_new_message: true,
            close_to_tray: true,
            start_minimized: false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
struct PrivacyCategories {
    contact_name: bool,
    chat_messages: bool,
    last_message: bool,
    media: bool,
    profile_pic: bool,
    voice_notes: bool,
    stickers: bool,
    compose_bar: bool,
    status: bool,
    reactions: bool,
    searchbar: bool,
    communities: bool,
    contact_info: bool,
}

impl Default for PrivacyCategories {
    fn default() -> Self {
        PrivacyCategories {
            contact_name: true,
            chat_messages: true,
            last_message: true,
            media: true,
            profile_pic: true,
            voice_notes: true,
            stickers: true,
            compose_bar: false,
            status: true,
            reactions: true,
            searchbar: true,
            communities: true,
            contact_info: true,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct PrivacyConfig {
    enabled: bool,
    blur: u32,
    categories: PrivacyCategories,
    /// Notification privacy mode: "" (show normally), "redact" ("New message"),
    /// "suppress" (silent). Read by page-notify.js via <html data-wa-notif>.
    notif: String,
    /// Panic-mode unlock PIN, stored as a salted SHA-256 hex hash (never plaintext).
    /// Empty = no lock (Esc reveals). Read by privacy_boot.js.
    panic_pin: String,
    /// Salt used with `panic_pin`. Empty hash+salt => no lock.
    pin_salt: String,
    /// PIN length, so the keypad can render dots and auto-submit on the last digit.
    pin_len: u32,
    /// Whether the PIN was digit-only (selects keypad+dots vs. the text field).
    pin_digits_only: bool,
    /// Show content on hover (pure CSS `html.wa-no-hover` toggles this off).
    hover_reveal: bool,
    /// Blur everything when the main window loses focus (shoulder-surf guard).
    auto_hide_on_focus_loss: bool,
}

impl Default for PrivacyConfig {
    fn default() -> Self {
        PrivacyConfig {
            enabled: false,
            blur: 10,
            categories: PrivacyCategories::default(),
            notif: String::new(),
            panic_pin: String::new(),
            pin_salt: String::new(),
            pin_len: 0,
            pin_digits_only: false,
            hover_reveal: true,
            auto_hide_on_focus_loss: false,
        }
    }
}

#[derive(Default)]
struct AppState {
    settings: Mutex<Settings>,
    settings_path: Mutex<Option<PathBuf>>,
    privacy: Mutex<PrivacyConfig>,
    privacy_path: Mutex<Option<PathBuf>>,
    geometry: Mutex<Option<WindowGeometry>>,
    geometry_path: Mutex<Option<PathBuf>>,
    /// When the geometry was last written to disk. Used to throttle the
    /// resize/move write storm while still capturing the final position on close.
    geometry_last_save: Mutex<Option<Instant>>,
    badge_last_count: Mutex<u32>,
    badge_last_icon: Mutex<Option<BadgeImage>>,
    registered_hotkey: Mutex<Option<Shortcut>>,
    registered_blur_hotkey: Mutex<Option<Shortcut>>,
    registered_panic_hotkey: Mutex<Option<Shortcut>>,
}

/// A rendered taskbar-overlay badge (raw RGBA, row-major, top-to-bottom), produced
/// by badge.js and re-applied whenever the config or count changes.
#[derive(Clone)]
struct BadgeImage {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
}

#[derive(Clone, Serialize, Deserialize)]
struct WindowGeometry {
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    maximized: bool,
}

fn load_settings(app: &AppHandle) -> Settings {
    let mut s = Settings::default();
    if let Ok(dir) = app.path().app_data_dir() {
        let p = dir.join("settings.json");
        if let Ok(text) = fs::read_to_string(&p) {
            if let Ok(parsed) = serde_json::from_str::<Settings>(&text) {
                s = parsed;
            }
        }
        if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut sp) = state.settings_path.lock() {
                *sp = Some(p);
            }
        }
    }
    s
}

fn save_settings(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let s = state.settings.lock().ok().map(|g| g.clone());
    let path = state.settings_path.lock().ok().and_then(|p| p.clone());
    if let (Some(s), Some(path)) = (s, path) {
        if let Some(parent) = path.parent() {
            log_err("create settings directory", fs::create_dir_all(parent));
        }
        match serde_json::to_string_pretty(&s) {
            Ok(json) => log_err("write settings", fs::write(path, json)),
            Err(e) => eprintln!("serialize settings failed: {e}"),
        }
    }
}

fn load_privacy(app: &AppHandle) -> PrivacyConfig {
    let mut p = PrivacyConfig::default();
    if let Ok(dir) = app.path().app_data_dir() {
        let path = dir.join("privacy.json");
        if let Ok(text) = fs::read_to_string(&path) {
            if let Ok(parsed) = serde_json::from_str::<PrivacyConfig>(&text) {
                p = parsed;
            }
        }
        if let Some(state) = app.try_state::<AppState>() {
            if let Ok(mut pp) = state.privacy_path.lock() {
                *pp = Some(path);
            }
        }
    }
    p
}

fn save_privacy(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let p = state.privacy.lock().ok().map(|g| g.clone());
    let path = state.privacy_path.lock().ok().and_then(|x| x.clone());
    if let (Some(p), Some(path)) = (p, path) {
        if let Some(parent) = path.parent() {
            log_err("create privacy directory", fs::create_dir_all(parent));
        }
        match serde_json::to_string_pretty(&p) {
            Ok(json) => log_err("write privacy", fs::write(path, json)),
            Err(e) => eprintln!("serialize privacy failed: {e}"),
        }
    }
}

/// Serialize the current privacy config to a JSON object literal and push it to
/// the live WhatsApp Web page via webview.eval(). Returns false if the main
/// window isn't available.
fn apply_privacy_to_page(app: &AppHandle) -> bool {
    let config = app
        .try_state::<AppState>()
        .and_then(|s| s.privacy.lock().ok().map(|g| g.clone()))
        .unwrap_or_default();
    let json = match serde_json::to_string(&config) {
        Ok(j) => j,
        Err(_) => return false,
    };
    if let Some(win) = app.get_webview_window("main") {
        // Pass a raw JSON object literal; the boot script's apply() accepts it
        // directly (it only JSON.parse's when given a string).
        let script = format!("window.__WF_PRIVACY__ && window.__WF_PRIVACY__.apply({json})");
        log_err("apply privacy to page", win.eval(script));
        true
    } else {
        false
    }
}

fn apply_autostart(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let result = if enabled {
        app.autolaunch().enable()
    } else {
        app.autolaunch().disable()
    };
    result.map_err(|e| e.to_string())
}

/// Register (or re-register) a single global shortcut, remembering it in `slot` so it
/// can be replaced on the next change. The new shortcut is registered BEFORE the previous
/// one is dropped, so a failed change leaves the existing shortcut working; on success the
/// old one is unregistered (otherwise hotkeys would accumulate on every change).
fn register_shortcut(
    app: &AppHandle,
    shortcut: &str,
    slot: &Mutex<Option<Shortcut>>,
    on_press: impl Fn(&AppHandle) + Send + Sync + 'static,
) -> Result<(), String> {
    use std::str::FromStr;
    let key = Shortcut::from_str(shortcut).map_err(|e| e.to_string())?;
    let gs = app.global_shortcut();
    // Re-registering the same shortcut is a no-op. Without this, the "register first,
    // then drop the previous" order below would try to register a duplicate and fail.
    if let Ok(prev) = slot.lock() {
        if *prev == Some(key) {
            return Ok(());
        }
    }
    // Register the NEW shortcut BEFORE unregistering the previous one, so a failure
    // (combination already taken by another app) leaves the existing shortcut working.
    gs.on_shortcut(key, move |app, _shortcut, event| {
        if event.state() == ShortcutState::Pressed {
            on_press(app);
        }
    })
    .map_err(|e| e.to_string())?;
    if let Ok(mut prev) = slot.lock() {
        if let Some(old) = prev.replace(key) {
            log_err("unregister previous shortcut", gs.unregister(old));
        }
    }
    Ok(())
}

/// On-press action for the privacy blur hotkey: flip blur/reveal and summon the window.
fn toggle_blur(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        log_err(
            "toggle blur",
            win.eval("window.__WF_PRIVACY__ && window.__WF_PRIVACY__.toggleAll()"),
        );
    }
    show_main(app);
}

/// On-press action for the panic hotkey: toggle the panic curtain and summon the window.
fn toggle_panic(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        log_err(
            "toggle panic",
            win.eval("window.__WF_PRIVACY__ && window.__WF_PRIVACY__.togglePanic()"),
        );
    }
    show_main(app);
}

fn show_main(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        log_err("show main window", win.show());
        log_err("unminimize main window", win.unminimize());
        log_err("focus main window", win.set_focus());
    }
}

fn open_settings(app: &AppHandle) {
    // If the settings window already exists, focus it; otherwise create it.
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
    let builder =
        WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
            .title("Wisp — Settings")
            .inner_size(720.0, 860.0)
            .min_inner_size(480.0, 600.0)
            .resizable(true)
            .center();
    if let Err(e) = builder.build() {
        eprintln!("failed to open settings window: {e}");
    }
}

/// Open the separate settings window. Callable from the injected nav-rail item.
/// Async so the IPC command runs off the main thread: building a WebView2 window
/// synchronously on the main thread (from a webview-initiated IPC) can deadlock the
/// app's event loop and freeze every window.
#[tauri::command]
async fn open_settings_window(app: AppHandle) -> Result<(), String> {
    open_settings(&app);
    Ok(())
}

/// Snapshot the main window's current geometry into `AppState`. Cheap (no disk I/O),
/// so it is safe to call on every resize/move event.
fn update_geometry_memory(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    if let Some(window) = app.get_webview_window("main") {
        if let (Ok(outer), Ok(pos)) = (window.outer_size(), window.outer_position()) {
            let geo = WindowGeometry {
                width: outer.width,
                height: outer.height,
                x: pos.x,
                y: pos.y,
                maximized: window.is_maximized().unwrap_or(false),
            };
            if let Ok(mut g) = state.geometry.lock() {
                *g = Some(geo);
            }
        }
    }
}

/// Write the last captured geometry to disk so it survives a full app restart, not
/// just a close-to-tray hide within the same process.
fn persist_geometry(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let geo = state.geometry.lock().ok().and_then(|g| g.clone());
    let path = state.geometry_path.lock().ok().and_then(|p| p.clone());
    if let (Some(geo), Some(path)) = (geo, path) {
        if let Some(parent) = path.parent() {
            log_err("create geometry directory", fs::create_dir_all(parent));
        }
        match serde_json::to_string_pretty(&geo) {
            Ok(json) => {
                if let Ok(mut last) = state.geometry_last_save.lock() {
                    *last = Some(Instant::now());
                }
                log_err("write geometry", fs::write(path, json));
            }
            Err(e) => eprintln!("serialize geometry failed: {e}"),
        }
    }
}

/// Capture geometry and force an immediate disk write. Used on close and before an
/// explicit quit, so the final position is never lost to throttling.
fn save_geometry(app: &AppHandle) {
    update_geometry_memory(app);
    persist_geometry(app);
}

/// Capture geometry on every resize/move, but write to disk at most once per
/// `GEOMETRY_SAVE_THROTTLE`. A resize/move drag can fire dozens of events per second;
/// without this the handlers would hammer the filesystem.
fn save_geometry_throttled(app: &AppHandle) {
    update_geometry_memory(app);
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let due = match state.geometry_last_save.lock() {
        Ok(last) => match *last {
            Some(t) => t.elapsed() >= GEOMETRY_SAVE_THROTTLE,
            None => true,
        },
        Err(_) => true,
    };
    if due {
        persist_geometry(app);
    }
}

fn load_geometry(app: &AppHandle) -> Option<WindowGeometry> {
    let dir = app.path().app_data_dir().ok()?;
    let path = dir.join("geometry.json");
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut gp) = state.geometry_path.lock() {
            *gp = Some(path.clone());
        }
    }
    let text = fs::read_to_string(&path).ok()?;
    serde_json::from_str::<WindowGeometry>(&text).ok()
}

fn restore_geometry(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    // Prefer the in-memory geometry (set by save_geometry during this run); fall back
    // to the on-disk copy so geometry persists across launches.
    let geo = state
        .geometry
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .or_else(|| load_geometry(app));
    if let (Some(window), Some(geo)) = (app.get_webview_window("main"), geo) {
        let mut x = geo.x;
        let mut y = geo.y;
        // If the saved position no longer overlaps any connected monitor (e.g. the user
        // unplugged a secondary display), re-center it on the primary monitor so the
        // window doesn't open off-screen.
        if let Ok(monitors) = window.available_monitors() {
            let visible = monitors.iter().any(|m| {
                let pos = m.position();
                let size = m.size();
                x < pos.x + size.width as i32
                    && x + geo.width as i32 > pos.x
                    && y < pos.y + size.height as i32
                    && y + geo.height as i32 > pos.y
            });
            if !visible {
                if let Some(m) = monitors.first() {
                    let pos = m.position();
                    let size = m.size();
                    x = pos.x + ((size.width as i32 - geo.width as i32) / 2).max(0);
                    y = pos.y + ((size.height as i32 - geo.height as i32) / 2).max(0);
                }
            }
        }
        log_err(
            "restore window size",
            window.set_size(tauri::PhysicalSize::new(geo.width, geo.height)),
        );
        log_err(
            "restore window position",
            window.set_position(tauri::PhysicalPosition::new(x, y)),
        );
        if geo.maximized {
            log_err("restore maximize", window.maximize());
        }
    }
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Open WhatsApp", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings…", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&show, &settings, &sep, &quit])?;
    TrayIconBuilder::with_id("main-tray")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "settings" => open_settings(app),
            "quit" => {
                save_geometry(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::DoubleClick { .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

/// Commands callable from the (local) settings window.
#[tauri::command]
fn settings_get(app: AppHandle, state: tauri::State<AppState>) -> Result<Settings, String> {
    let mut s = state.settings.lock().map_err(|e| e.to_string())?.clone();
    // Reflect the real OS autostart state (the user may have changed it in Task Manager)
    // and persist the correction so the stored value stays authoritative.
    if let Ok(enabled) = app.autolaunch().is_enabled() {
        if s.autostart != enabled {
            s.autostart = enabled;
            if let Ok(mut stored) = state.settings.lock() {
                stored.autostart = enabled;
            }
            save_settings(&app);
        }
    }
    Ok(s)
}

#[tauri::command]
fn settings_set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    // Apply to the OS first; if that fails, leave the persisted setting untouched and
    // surface the error so the settings UI can revert the toggle.
    apply_autostart(&app, enabled)?;
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut s) = state.settings.lock() {
            s.autostart = enabled;
        }
    }
    save_settings(&app);
    Ok(())
}

#[tauri::command]
fn settings_set_hotkey(app: AppHandle, hotkey: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    // Register first: if the shortcut is invalid or already taken, keep the previous
    // working shortcut and the persisted value untouched.
    register_shortcut(&app, &hotkey, &state.registered_hotkey, show_main)?;
    if let Ok(mut s) = state.settings.lock() {
        s.hotkey = hotkey;
    }
    save_settings(&app);
    Ok(())
}

#[tauri::command]
fn settings_set_blur_hotkey(app: AppHandle, hotkey: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    register_shortcut(&app, &hotkey, &state.registered_blur_hotkey, toggle_blur)?;
    if let Ok(mut s) = state.settings.lock() {
        s.blur_hotkey = hotkey;
    }
    save_settings(&app);
    Ok(())
}

#[tauri::command]
fn settings_set_panic_hotkey(app: AppHandle, hotkey: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    register_shortcut(&app, &hotkey, &state.registered_panic_hotkey, toggle_panic)?;
    if let Ok(mut s) = state.settings.lock() {
        s.panic_hotkey = hotkey;
    }
    save_settings(&app);
    Ok(())
}

/// Return the persisted privacy config to the settings window.
#[tauri::command]
fn privacy_get(state: tauri::State<AppState>) -> Result<PrivacyConfig, String> {
    state
        .privacy
        .lock()
        .map_err(|e| e.to_string())
        .map(|g| g.clone())
}

/// Persist a new privacy config and immediately apply it to the live page.
#[tauri::command]
fn privacy_set(app: AppHandle, config: PrivacyConfig) -> Result<(), String> {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut p) = state.privacy.lock() {
            *p = config.clone();
        }
    }
    save_privacy(&app);
    apply_privacy_to_page(&app);
    Ok(())
}

/// Toggle the page between "reveal all" and "blur all" without changing the saved
/// config (used by the settings "Blur test" button). The page owns the toggle state and
/// `eval` cannot return it, so callers only learn that the toggle was dispatched.
#[tauri::command]
fn privacy_toggle_all(app: AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("main") {
        log_err(
            "toggle privacy",
            win.eval("window.__WF_PRIVACY__ && window.__WF_PRIVACY__.toggleAll()"),
        );
        Ok(())
    } else {
        Err("main window not available".into())
    }
}

/// Toggle the panic-mode curtain on the live page and bring the window forward.
/// Used by the settings "Test panic" button; the global panic hotkey is wired
/// separately in register_panic_hotkey.
#[tauri::command]
fn panic_toggle(app: AppHandle) -> Result<bool, String> {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.eval("window.__WF_PRIVACY__ && window.__WF_PRIVACY__.togglePanic()");
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        Ok(true)
    } else {
        Err("main window not available".into())
    }
}

#[tauri::command]
fn settings_reset_view(app: AppHandle) -> Result<(), String> {
    // Reset the main window geometry to defaults and refocus it. Use a logical
    // (DPI-independent) size so the reset looks the same on every display.
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.set_size(tauri::LogicalSize::new(1200.0, 800.0));
        let _ = win.center();
        let _ = win.show();
        let _ = win.set_focus();
        // Persist the new geometry immediately so a restart honours the reset.
        save_geometry(&app);
    }
    Ok(())
}

#[tauri::command]
fn summon_window(app: AppHandle) -> Result<(), String> {
    show_main(&app);
    Ok(())
}

#[tauri::command]
fn quit_app(app: AppHandle) -> Result<(), String> {
    // Capture the final geometry before the process goes away.
    save_geometry(&app);
    app.exit(0);
    Ok(())
}

/// Re-apply the taskbar overlay icon from the last known count + rendered RGBA.
/// Called whenever the badge config or the page's unread count changes.
fn update_badge_icon(app: &AppHandle) {
    let Some(state) = app.try_state::<AppState>() else {
        return;
    };
    let enabled = state
        .settings
        .lock()
        .ok()
        .map(|s| s.unread_badge)
        .unwrap_or(true);
    let count = state.badge_last_count.lock().ok().map(|g| *g).unwrap_or(0);
    let icon = state.badge_last_icon.lock().ok().and_then(|g| g.clone());
    if let Some(win) = app.get_webview_window("main") {
        if !enabled || count == 0 {
            log_err("clear overlay icon", win.set_overlay_icon(None));
        } else if let Some(bi) = icon {
            let img = tauri::image::Image::new_owned(bi.rgba, bi.width, bi.height);
            log_err("set overlay icon", win.set_overlay_icon(Some(img)));
        } else {
            log_err("clear overlay icon", win.set_overlay_icon(None));
        }
    }
}

/// Rule for whether a new unread count should flash the taskbar. Pure + testable:
/// only flashes on an *increase* while the window is unfocused and flash is enabled.
fn should_flash_for_count(prev: u32, next: u32, focused: bool, enabled: bool) -> bool {
    enabled && next > prev && !focused
}

/// Minimal standard-alphabet Base64 decoder (whitespace tolerated, `=` padding
/// ignored). The badge bitmap is sent as base64 rather than a JSON array of numbers,
/// which keeps the IPC payload roughly 4x smaller. Returns `None` on malformed input.
fn decode_base64(input: &str) -> Option<Vec<u8>> {
    fn value(c: u8) -> Option<u32> {
        match c {
            b'A'..=b'Z' => Some((c - b'A') as u32),
            b'a'..=b'z' => Some((c - b'a') as u32 + 26),
            b'0'..=b'9' => Some((c - b'0') as u32 + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    // Trailing `=` padding carries no data.
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b'=' {
        end -= 1;
    }
    let data = &bytes[..end];
    let mut out = Vec::with_capacity(data.len() / 4 * 3 + 3);
    for chunk in data.chunks(4) {
        // A lone trailing character can never be valid base64.
        if chunk.len() < 2 {
            return None;
        }
        let mut v = [0u32; 4];
        for (i, &b) in chunk.iter().enumerate() {
            v[i] = value(b)?;
        }
        out.push(((v[0] << 2) | (v[1] >> 4)) as u8);
        if chunk.len() > 2 {
            out.push(((v[1] << 4) | (v[2] >> 2)) as u8);
        }
        if chunk.len() > 3 {
            out.push(((v[2] << 6) | v[3]) as u8);
        }
    }
    Some(out)
}

/// Called by badge.js when the page's unread count (from document.title) changes.
///
/// `rename_all = "snake_case"` matches the argument keys badge.js sends (Tauri's
/// default would expect camelCase).
#[tauri::command(rename_all = "snake_case")]
fn badge_set_count(
    app: AppHandle,
    state: tauri::State<AppState>,
    count: u32,
    rgba_b64: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
) -> Result<(), String> {
    // Read the previous count and store the new one under a single lock.
    let prev = {
        let mut guard = state.badge_last_count.lock().map_err(|e| e.to_string())?;
        let prev = *guard;
        *guard = count;
        prev
    };

    // Validate the rendered badge before trusting it: the payload comes from the remote
    // page, so cap the dimensions and require the exact RGBA byte length.
    let badge = match (rgba_b64, width, height) {
        (Some(encoded), Some(width), Some(height))
            if width > 0 && height > 0 && width <= MAX_BADGE_DIM && height <= MAX_BADGE_DIM =>
        {
            decode_base64(&encoded)
                .filter(|rgba| rgba.len() == (width * height * 4) as usize)
                .map(|rgba| BadgeImage {
                    rgba,
                    width,
                    height,
                })
        }
        _ => None,
    };
    if let Ok(mut i) = state.badge_last_icon.lock() {
        *i = badge;
    }

    let flash = state
        .settings
        .lock()
        .ok()
        .map(|s| s.flash_on_new_message)
        .unwrap_or(true);
    if let Some(win) = app.get_webview_window("main") {
        if should_flash_for_count(prev, count, win.is_focused().unwrap_or(false), flash)
            && win.is_visible().unwrap_or(false)
        {
            log_err(
                "request user attention",
                win.request_user_attention(Some(tauri::UserAttentionType::Informational)),
            );
        }
    }

    update_badge_icon(&app);
    Ok(())
}

/// Whether the taskbar overlay badge is shown (toggled from the settings window).
#[tauri::command]
fn settings_set_unread_badge(app: AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut s) = state.settings.lock() {
            s.unread_badge = enabled;
        }
    }
    save_settings(&app);
    update_badge_icon(&app);
    Ok(())
}

/// Whether the taskbar flashes when a new message arrives while unfocused.
#[tauri::command]
fn settings_set_flash(app: AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut s) = state.settings.lock() {
            s.flash_on_new_message = enabled;
        }
    }
    save_settings(&app);
    Ok(())
}

#[tauri::command]
fn settings_set_close_to_tray(app: AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut s) = state.settings.lock() {
            s.close_to_tray = enabled;
        }
    }
    save_settings(&app);
    Ok(())
}

#[tauri::command]
fn settings_set_start_minimized(app: AppHandle, enabled: bool) -> Result<(), String> {
    if let Some(state) = app.try_state::<AppState>() {
        if let Ok(mut s) = state.settings.lock() {
            s.start_minimized = enabled;
        }
    }
    save_settings(&app);
    Ok(())
}

fn register_initial_hotkey(app: &AppHandle, hotkey: &str) {
    let state = app.state::<AppState>();
    if let Err(e) = register_shortcut(app, hotkey, &state.registered_hotkey, show_main) {
        eprintln!("hotkey registration failed: {e}");
    }
}

fn register_initial_blur_hotkey(app: &AppHandle, shortcut: &str) {
    let state = app.state::<AppState>();
    if let Err(e) = register_shortcut(app, shortcut, &state.registered_blur_hotkey, toggle_blur) {
        eprintln!("blur hotkey registration failed: {e}");
    }
}

fn register_initial_panic_hotkey(app: &AppHandle, shortcut: &str) {
    let state = app.state::<AppState>();
    if let Err(e) = register_shortcut(app, shortcut, &state.registered_panic_hotkey, toggle_panic) {
        eprintln!("panic hotkey registration failed: {e}");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_deep_link::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            settings_get,
            settings_set_autostart,
            settings_set_hotkey,
            settings_set_blur_hotkey,
            settings_set_panic_hotkey,
            settings_reset_view,
            summon_window,
            open_settings_window,
            privacy_get,
            privacy_set,
            privacy_toggle_all,
            panic_toggle,
            badge_set_count,
            settings_set_unread_badge,
            settings_set_flash,
            settings_set_close_to_tray,
            settings_set_start_minimized,
            quit_app,
        ])
        .setup(|app| {
            // Load persisted settings and apply OS state.
            let settings = load_settings(app.handle());
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut s) = state.settings.lock() { *s = settings.clone(); }
            }
            if let Err(e) = apply_autostart(app.handle(), settings.autostart) {
                eprintln!("apply autostart at startup failed: {e}");
            }
            register_initial_hotkey(app.handle(), &settings.hotkey);
            register_initial_blur_hotkey(app.handle(), &settings.blur_hotkey);
            register_initial_panic_hotkey(app.handle(), &settings.panic_hotkey);

            // Load persisted privacy config and push it into AppState.
            let privacy = load_privacy(app.handle());
            if let Some(state) = app.try_state::<AppState>() {
                if let Ok(mut p) = state.privacy.lock() { *p = privacy; }
            }

            // Register what'sapp and wa.me deep links; route them into the WhatsApp Web window.
            let _ = app.deep_link().register_all();
            let app_handle = app.handle().clone();
            app.deep_link().on_open_url(move |event| {
                for url in event.urls() {
                    route_deep_link(&app_handle, &url);
                }
            });

            // Build the document-start scripts. They are injected SEPARATELY so a
            // failure in one (e.g. document.head not ready yet) can NEVER break the
            // others — the controller must always define window.__WF_PRIVACY__.
            let css_literal = serde_json::to_string(PRIVACY_STYLES_CSS).unwrap_or_else(|_| String::from("\"\""));
            let css_style_js = format!(
                "(function(){{if(window.location.origin!=='https://web.whatsapp.com')return;var css={css_literal};function go(){{var h=document.head||document.documentElement;if(!h){{setTimeout(go,0);return;}}if(document.getElementById('wa-default-blur'))return;var s=document.createElement('style');s.id='wa-default-blur';s.textContent=css;h.appendChild(s);}}go();}})();"
            );

            // Main WhatsApp Web window (desktop user-agent + navigation guard).
            let win = WebviewWindowBuilder::new(app, "main", WebviewUrl::External(WHATSAPP_WEB_URL.parse().expect("valid url")))
                .title("Wisp")
                .inner_size(1200.0, 800.0)
                .min_inner_size(360.0, 600.0)
                .center()
                .user_agent(DESKTOP_USER_AGENT)
                // Inject (bridge, style, notification wrapper, controller, rail entry, badge)
                // at document-start. The bridge must come first so rail/badge can share it.
                .initialization_script(BRIDGE_JS)
                .initialization_script(css_style_js)
                .initialization_script(PAGE_NOTIFY_JS)
                .initialization_script(PRIVACY_BOOT_JS)
                .initialization_script(RAIL_SETTINGS_JS)
                .initialization_script(BADGE_JS)
                .on_navigation(|url| {
                    // Allow only the real WhatsApp Web origin (exact scheme + host), never a
                    // spoofed host like web.whatsapp.com.evil.example.
                    if url.scheme() == "https" && url.host_str() == Some("web.whatsapp.com") {
                        return true;
                    }
                    // Everything else leaves the app, but only well-known web/mail schemes
                    // are handed to the OS opener. Anything else (`file:`, `ms-settings:`,
                    // arbitrary custom schemes) is dropped so a link in a message cannot ask
                    // ShellExecute to launch an arbitrary handler.
                    if matches!(url.scheme(), "http" | "https" | "mailto") {
                        let _ = tauri_plugin_opener::open_url(url.as_str(), None::<&str>);
                    }
                    false
                })
                // Re-apply the saved privacy config on every full page load.
                .on_page_load(|window, _payload| {
                    let app = window.app_handle().clone();
                    apply_privacy_to_page(&app);
                })
                .build()
                .expect("failed to build main window");
            restore_geometry(app.handle());
            if settings.start_minimized {
                let _ = win.minimize();
            }
            let _ = build_tray(app.handle());
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            match event {
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    // Only the main (WhatsApp) window hides to tray; the settings window closes normally.
                    if window.label() == "main" {
                        save_geometry(app);
                        let close_to_tray = app
                            .try_state::<AppState>()
                            .and_then(|s| s.settings.lock().ok().map(|g| g.close_to_tray))
                            .unwrap_or(true);
                        if close_to_tray {
                            api.prevent_close();
                            let _ = window.hide();
                        }
                    }
                }
                tauri::WindowEvent::Focused(focused) if window.label() == "main" => {
                    // Shoulder-surf guard: blur everything when the main window loses
                    // focus; restore the saved config when it regains focus.
                    let auto = app
                        .try_state::<AppState>()
                        .and_then(|s| s.privacy.lock().ok().map(|p| p.auto_hide_on_focus_loss))
                        .unwrap_or(false);
                    if auto {
                        if *focused {
                            apply_privacy_to_page(app);
                        } else if let Some(win) = app.get_webview_window("main") {
                            let _ = win.eval("window.__WF_PRIVACY__ && window.__WF_PRIVACY__.blurAll()");
                        }
                    }
                }
                tauri::WindowEvent::Resized(_) | tauri::WindowEvent::Moved(_) if window.label() == "main" => {
                    // Throttled: a drag fires these continuously; the last position is
                    // flushed by save_geometry on close and before an explicit quit.
                    save_geometry_throttled(app);
                }
                _ => {},
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_flash_for_count_only_on_increase_when_unfocused_and_enabled() {
        // Increase while unfocused + enabled -> flash.
        assert!(should_flash_for_count(0, 1, false, true));
        assert!(should_flash_for_count(3, 8, false, true));
        // No flash when the count decreases (the user read the messages).
        assert!(!should_flash_for_count(5, 4, false, true));
        assert!(!should_flash_for_count(0, 0, false, true));
        // No flash when the window is focused.
        assert!(!should_flash_for_count(0, 1, true, true));
        // No flash when the feature is disabled.
        assert!(!should_flash_for_count(0, 1, false, false));
    }

    #[test]
    fn settings_deserialize_backwards_compatible() {
        // A settings.json written by an older build (before the badge settings)
        // must still parse, defaulting the new fields to ON.
        let old_json = r#"{"autostart":true,"hotkey":"Ctrl+Shift+Space","blur_hotkey":"Ctrl+Shift+B","panic_hotkey":"Ctrl+Shift+Z"}"#;
        let s: Settings = serde_json::from_str(old_json).expect("old settings should parse");
        assert!(s.unread_badge);
        assert!(s.flash_on_new_message);
        assert!(s.autostart);
        assert_eq!(s.hotkey, "Ctrl+Shift+Space");
        assert!(s.close_to_tray, "close_to_tray should default ON");
        assert!(!s.start_minimized, "start_minimized should default OFF");
    }

    #[test]
    fn settings_deserialize_missing_hotkeys_defaults() {
        // The very first settings.json only had autostart + hotkey; blur_hotkey and
        // panic_hotkey were added later. A missing field must NOT reset the whole file
        // to defaults — it should fall back to that field's app default.
        let old_json = r#"{"autostart":true,"hotkey":"Ctrl+Shift+Space"}"#;
        let s: Settings = serde_json::from_str(old_json).expect("old settings should parse");
        assert!(s.autostart);
        assert_eq!(s.hotkey, "Ctrl+Shift+Space");
        assert_eq!(s.blur_hotkey, DEFAULT_BLUR_HOTKEY);
        assert_eq!(s.panic_hotkey, DEFAULT_PANIC_HOTKEY);
        assert!(s.unread_badge);
        assert!(s.flash_on_new_message);
        assert!(s.close_to_tray);
        assert!(!s.start_minimized);
    }

    #[test]
    fn settings_toggle_round_trip() {
        let s = Settings {
            unread_badge: false,
            flash_on_new_message: false,
            ..Settings::default()
        };
        let json = serde_json::to_string(&s).expect("serialize");
        let back: Settings = serde_json::from_str(&json).expect("deserialize");
        assert!(!back.unread_badge);
        assert!(!back.flash_on_new_message);
        assert!(back.autostart == false); // Default impl held for the rest
    }

    #[test]
    fn privacy_config_deserialize_backwards_compatible() {
        // A privacy.json written before hover/auto-hide existed must still parse,
        // defaulting hover-reveal ON and auto-hide OFF (from Default).
        let old_json = r#"{"enabled":true,"blur":12,"notif":"redact","panic_pin":"","categories":{"contactName":true}}"#;
        let p: PrivacyConfig = serde_json::from_str(old_json).expect("old privacy should parse");
        assert!(p.enabled);
        assert_eq!(p.blur, 12);
        assert_eq!(p.notif, "redact");
        assert!(p.hover_reveal, "hover_reveal should default ON");
        assert!(
            !p.auto_hide_on_focus_loss,
            "auto_hide_on_focus_loss should default OFF"
        );
        // pin_salt/pin_len/pin_digits_only were added later; an old file without them
        // must default to no lock (empty salt, zero length, not digit-only).
        assert_eq!(p.pin_salt, "", "pin_salt should default to empty");
        assert_eq!(p.pin_len, 0, "pin_len should default to 0");
        assert!(
            !p.pin_digits_only,
            "pin_digits_only should default to false"
        );
        // Omitted category keys default to the configured value (true), not false.
        assert!(p.categories.contact_name);
    }

    #[test]
    fn privacy_config_pin_salt_round_trip() {
        // The panic PIN is stored as a salted hash (not plaintext); the salt field
        // must survive a serialize/deserialize round trip.
        let p = PrivacyConfig {
            panic_pin: "a1b2c3...".into(),
            pin_salt: "deadbeefdeadbeef".into(),
            ..PrivacyConfig::default()
        };
        let json = serde_json::to_string(&p).expect("serialize");
        let back: PrivacyConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.panic_pin, "a1b2c3...");
        assert_eq!(back.pin_salt, "deadbeefdeadbeef");
    }

    #[test]
    fn privacy_config_toggle_round_trip() {
        let p = PrivacyConfig {
            hover_reveal: false,
            auto_hide_on_focus_loss: true,
            ..PrivacyConfig::default()
        };
        let json = serde_json::to_string(&p).expect("serialize");
        let back: PrivacyConfig = serde_json::from_str(&json).expect("deserialize");
        assert!(!back.hover_reveal);
        assert!(back.auto_hide_on_focus_loss);
        assert!(!back.enabled); // Default held for the rest
    }

    #[test]
    fn build_send_url_whatsapp_scheme() {
        // phone only
        assert_eq!(
            build_send_url("whatsapp", Some("send"), "", Some("123456"), None),
            Some("https://web.whatsapp.com/send?phone=123456".to_string())
        );
        // text only (leading + preserved for a message, not stripped)
        assert_eq!(
            build_send_url("whatsapp", Some("send"), "", None, Some("hi")),
            Some("https://web.whatsapp.com/send?text=hi".to_string())
        );
        // phone AND text combined + percent-encoded
        assert_eq!(
            build_send_url(
                "whatsapp",
                Some("send"),
                "",
                Some("+123456"),
                Some("hi there & more")
            ),
            Some("https://web.whatsapp.com/send?phone=123456&text=hi+there+%26+more".to_string())
        );
        // nothing useful -> None
        assert_eq!(
            build_send_url("whatsapp", Some("send"), "", None, None),
            None
        );
        // phone as the HOST (whatsapp://1234567890) with no query param
        assert_eq!(
            build_send_url("whatsapp", Some("1234567890"), "", None, None),
            Some("https://web.whatsapp.com/send?phone=1234567890".to_string())
        );
        // host phone + query text combined
        assert_eq!(
            build_send_url(
                "whatsapp",
                Some("1234567890"),
                "",
                None,
                Some("hello world")
            ),
            Some("https://web.whatsapp.com/send?phone=1234567890&text=hello+world".to_string())
        );
        // leading '+' stripped from a host phone
        assert_eq!(
            build_send_url("whatsapp", Some("+15551234567"), "", None, None),
            Some("https://web.whatsapp.com/send?phone=15551234567".to_string())
        );
        // a bare non-send, non-numeric host with no phone/text -> None
        assert_eq!(
            build_send_url("whatsapp", Some("chat"), "", None, None),
            None
        );
    }

    #[test]
    fn build_send_url_wa_me() {
        assert_eq!(
            build_send_url("https", Some("wa.me"), "/1234567890", None, None),
            Some("https://web.whatsapp.com/send?phone=1234567890".to_string())
        );
        // phone + text
        assert_eq!(
            build_send_url("https", Some("wa.me"), "/12345", None, Some("hi")),
            Some("https://web.whatsapp.com/send?phone=12345&text=hi".to_string())
        );
        // leading '+' in path stripped
        assert_eq!(
            build_send_url("https", Some("wa.me"), "/+123", None, None),
            Some("https://web.whatsapp.com/send?phone=123".to_string())
        );
        // http scheme accepted too (and the + stripped)
        assert_eq!(
            build_send_url("http", Some("wa.me"), "/+123", None, None),
            Some("https://web.whatsapp.com/send?phone=123".to_string())
        );
        // empty path -> None
        assert_eq!(
            build_send_url("https", Some("wa.me"), "/", None, None),
            None
        );
        // wrong host on a wa.me-looking scheme -> None
        assert_eq!(
            build_send_url("https", Some("api.whatsapp.com"), "/send", None, None),
            None
        );
    }

    #[test]
    fn build_send_url_rejects_other_shapes() {
        assert_eq!(
            build_send_url("https", Some("example.com"), "/x", None, None),
            None
        );
        assert_eq!(build_send_url("mailto", Some("a@b"), "", None, None), None);
    }

    #[test]
    fn decode_base64_decodes_and_rejects_malformed() {
        assert_eq!(decode_base64(""), Some(Vec::new()));
        assert_eq!(decode_base64("TWFu"), Some(b"Man".to_vec()));
        assert_eq!(decode_base64("TWE="), Some(b"Ma".to_vec()));
        assert_eq!(decode_base64("TQ=="), Some(b"M".to_vec()));
        assert_eq!(decode_base64("AAAA"), Some(vec![0, 0, 0]));
        assert_eq!(decode_base64("//8="), Some(vec![255, 255]));
        // Whitespace (e.g. wrapped lines) is tolerated.
        assert_eq!(decode_base64("TW\nFu"), Some(b"Man".to_vec()));
        // Malformed input is rejected rather than producing a partial badge.
        assert_eq!(decode_base64("A"), None);
        assert_eq!(decode_base64("!!!!"), None);
    }

    // ---------- ACL three-way sync guard ----------
    // A Tauri custom command must be registered in THREE places that all agree or the app
    // silently loses the ability to invoke it (or to grant it to a remote origin):
    //   1. build.rs `APP_COMMANDS`           -> drives the autogenerated allow-<kebab> ACL
    //   2. the `generate_handler!` list      -> the actual dispatcher (src/lib.rs)
    //   3. the `allow-<kebab>` permission    -> granted in a capability in tauri.conf.json
    // This guard fails the build if they drift. Sources are embedded via include_str! so the
    // comparison reads the real files (not a hand-copied duplicate).
    const BUILD_RS_SRC: &str = include_str!("../build.rs");
    const LIB_RS_SRC: &str = include_str!("lib.rs");
    const TAURI_CONF_SRC: &str = include_str!("../tauri.conf.json");

    /// Snapshot the `"name",` entries in build.rs `APP_COMMANDS = &[...]`.
    fn app_commands_from_build_rs(src: &str) -> Vec<String> {
        let start = src.find("const APP_COMMANDS").expect("APP_COMMANDS decl");
        // The type is `&[&str]`, so find the `= &[` that opens the value array (not the
        // `[` in the array type), otherwise we'd parse `&str` as a command name.
        let arr = src[start..].find("= &[").expect("array init") + start + "= &[".len();
        let end = src[arr..].find(']').expect("]") + arr;
        src[arr..end]
            .split(',')
            .map(|s| s.trim().trim_matches('"').to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Snapshot the bare identifiers inside `generate_handler![ ... ]`.
    fn handler_commands_from_lib(src: &str) -> Vec<String> {
        let start = src.find("generate_handler![").expect("generate_handler![");
        let arr = src[start..].find('[').expect("[") + start;
        let end = src[arr..].find(']').expect("]") + arr;
        src[arr + 1..end]
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Snapshot the app's own `allow-*` permissions from every capability and map kebab->snake
    /// to the command name they grant. Namespaced plugin/core perms ("plugin:..."/"core:...") are
    /// excluded because they contain ':' and are not our custom commands.
    fn capability_command_names(src: &str) -> Vec<String> {
        let v: serde_json::Value = serde_json::from_str(src).expect("parse tauri.conf.json");
        let mut cmds = Vec::new();
        if let Some(caps) = v["app"]["security"]["capabilities"].as_array() {
            for cap in caps {
                if let Some(perms) = cap["permissions"].as_array() {
                    for p in perms {
                        if let Some(ps) = p.as_str() {
                            if ps.starts_with("allow-") && !ps.contains(':') {
                                cmds.push(ps.trim_start_matches("allow-").replace('-', "_"));
                            }
                        }
                    }
                }
            }
        }
        cmds
    }

    #[test]
    fn acl_sync_build_handler_and_capabilities_agree() {
        use std::collections::BTreeSet;
        let build: BTreeSet<String> = app_commands_from_build_rs(BUILD_RS_SRC)
            .into_iter()
            .collect();
        let handler: BTreeSet<String> = handler_commands_from_lib(LIB_RS_SRC).into_iter().collect();
        let caps: BTreeSet<String> = capability_command_names(TAURI_CONF_SRC)
            .into_iter()
            .collect();

        assert!(!build.is_empty(), "build.rs APP_COMMANDS is empty");
        assert!(
            build == handler,
            "ACL sync broken: build.rs APP_COMMANDS != generate_handler! in lib.rs\n\
             only-in-APP_COMMANDS: {:?}\n\
             only-in-generate_handler: {:?}\n\
             missing-from-APP_COMMANDS: {:?}",
            build.difference(&handler).collect::<Vec<_>>(),
            handler.difference(&build).collect::<Vec<_>>(),
            build.symmetric_difference(&handler).collect::<Vec<_>>()
        );
        assert!(
            build == caps,
            "ACL sync broken: build.rs APP_COMMANDS != capability allow-* in tauri.conf.json\n\
             only-in-APP_COMMANDS: {:?}\n\
             only-in-capability: {:?}",
            build.difference(&caps).collect::<Vec<_>>(),
            caps.difference(&build).collect::<Vec<_>>()
        );
    }
}
