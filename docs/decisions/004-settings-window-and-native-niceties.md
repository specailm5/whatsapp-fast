# ADR-004: Settings window, auto-start, and global hotkey

## Status
Accepted

## Context
The shell should offer the desktop conveniences users expect from a native app: launch on
sign-in, a global shortcut to summon the window, and a small settings panel. These must not
interfere with the remote WhatsApp Web window or its ToS-safe wrapper.

## Decision
- Add `tauri-plugin-autostart` for optional launch-at-login (driven from our own Rust command,
  not the remote page).
- Add `tauri-plugin-global-shortcut` registered from Rust with a persisted, user-configurable
  shortcut (default `Ctrl+Shift+Space`) that shows/focuses the main window.
- Build a **separate local settings window** (`ui/settings.html`) from the tray menu, using a
  dedicated local capability. It calls our Rust commands via `__TAURI__.core.invoke`.
- Persist settings (autostart flag + hotkey) in `app_data_dir/settings.json`.

### Why a separate local window, not a route inside WhatsApp Web
The main window loads remote `web.whatsapp.com`, which is Meta's code. Injecting our UI or
hooking its internals would invade the official client. A sibling local window keeps our
chrome native, small, and fully under our control while leaving WhatsApp Web untouched.

### Why Rust commands instead of plugin JS in the settings page
Calling the plugin JS APIs from a local page would grant the page plugin permissions. Driving
autostart and the hotkey from Rust commands keeps the permission surface minimal and keeps
plugin actions server-side and consistent.

## Consequences
+ Native, expected desktop behavior (auto-start, summon hotkey, settings).
+ WhatsApp Web remains a pure remote wrapper — no ToS-relevant change.
+ Settings persist across launches.
\- One more lightweight local window; the shell keeps it tiny and non-competitive with the
   WhatsApp page for CPU.

## Follow-up (Value 4): settings correctness
- **Close-to-tray is now configurable** (`close_to_tray`, default ON) — it replaces the old
  hardcoded `CLOSE_TO_TRAY` const, so a user can opt out and let closing quit the app.
- **Start minimized** (`start_minimized`, default OFF) — the main window starts minimized to
  the taskbar on launch.
- **Real autostart state**: `settings_get` now queries `autolaunch().is_enabled()` so the
  toggle reflects the actual OS state (e.g. if the user disabled it in Task Manager).
- **Debounce**: the blur-strength slider now debounces its `privacy_set` IPC (150ms) so
  dragging the range doesn't flood the page with evals.
Both new settings persist in `settings.json` (backward-compatible via serde defaults).
