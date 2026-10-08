# Wisp

A fast, low-memory **native Windows shell** for the **official WhatsApp Web** client, built
with **Tauri 2** (Rust + OS WebView2).

> **Why:** the WhatsApp chat UI is already fast. What's slow in the official Electron desktop
> app is the *container* — startup, RAM/CPU, and navigation responsiveness. This app swaps
> that heavy container for a native WebView2 shell while keeping the official client intact.
> It is **ToS-safe** (it loads Meta's own web client; no unofficial protocol library).

## Features
- Native Windows window hosting `https://web.whatsapp.com`.
- Desktop Chrome user-agent override so the full client loads.
- System tray icon + menu (Open / Settings / Quit) and double-click to restore.
- Close-to-tray warm start (re-open is instant).
- Single-instance: re-launch focuses the existing window.
- External links open in your default browser, not the app.
- Notifications raised by the web client are passed through, and the privacy suite can redact or silence them.
- Persisted window geometry across launches.
- **Taskbar unread badge** — shows the unread-chat count on the taskbar button and
  flashes the taskbar when a new message arrives while the window is in the
  background. The count is read from the page's own `document.title`, so it stays
  ToS-safe (no internal state). Toggle in Settings.
- **Launch at startup** (optional; toggle in Settings).
- **Global hotkey** (default `Ctrl+Shift+Space`) to summon & focus the window from anywhere.
- **Settings window** — auto-start toggle, change the hotkey, reset the window view, quit.
- **System tray "Settings…" menu item**.
- **Deep links** — `whatsapp://send?phone=…` and delivered `https://wa.me/…` links open the right chat and focus the app.
- **Privacy suite** (optional) — blur message text, previews, contact/group names, profile
  pictures, media, and the composer; hover any blurred item to reveal it. Category toggles +
  blur-strength slider in Settings, plus a global quick-toggle hotkey (default
  `Ctrl+Shift+B`) to reveal-all / blur-all. Hotkeys are set by clicking <b>Record</b> and
  pressing the real key combo (a capture popup; <b>Esc</b> cancels) — no manual typing. If an
  element stops matching (or over-blurs), the selectors are refreshed by dumping the live DOM
  from the WhatsApp devtools console and baking the real <code>data-testid</code>/class selectors
  back in. Purely local & visual; no WhatsApp data leaves the device (see ADR-006).
- **Panic mode** — a global hotkey (default Ctrl+Shift+Z) instantly hides every chat behind a
  full-screen curtain. Optionally set an unlock PIN in Settings: with a PIN the curtain only
  clears after entering it (reveal via the on-screen keypad or typed password field); without one
  Esc or the hotkey reveals instantly. Configurable hotkey + PIN in Settings.

## Requirements
- Windows 10/11 with the **WebView2 Runtime** (Win11 ships it; the installer bootstraps it on
  Win10).
- Rust toolchain + the `tauri` prerequisites to build from source.
- pnpm + Node.js to run the Tauri CLI.

## Build
```bash
pnpm install
pnpm --filter @wisp/desktop tauri dev      # run in dev
pnpm --filter @wisp/desktop tauri build    # produce NSIS installer (.exe)
```

The NSIS installer is written to:
  `apps/desktop/src-tauri/target/release/bundle/nsis/Wisp_0.1.0_x64-setup.exe`
  (confirmed ~2.3 MB). The release binary is `apps/desktop/src-tauri/target/release/wisp.exe`.

## Verification / CI

Every change is gated by fast, deterministic checks (also run by `.github/workflows/ci.yml` on
pull requests and pushes to `main`):

```bash
cd apps/desktop/src-tauri
cargo fmt -- --check          # rustfmt
cargo clippy --message-format short -- -D warnings
cargo test --lib              # unit tests
cd ../../..
node --check apps/desktop/src-tauri/src/bridge.js
node --check apps/desktop/src-tauri/src/page-notify.js
node --check apps/desktop/src-tauri/src/rail_settings.js
node --check apps/desktop/src-tauri/src/badge.js
node --check apps/desktop/src-tauri/src/privacy_boot.js
```

The suite includes an **ACL three-way sync guard** (`acl_sync_build_handler_and_capabilities_agree`):
Tauri custom commands must be registered in three places that agree — `build.rs` `APP_COMMANDS`,
the `generate_handler!` list, and the matching `allow-*` permission in a capability in
`tauri.conf.json`. If a command is added/renamed/removed in one place but not the others, the app
silently loses the ability to invoke or grant it; this test fails the build on such a drift.

The injected JavaScript files (and the inline `<script>` in `apps/desktop/ui/settings.html`) are
syntax-checked with `node --check`, and the CI runs a full `pnpm build` on `windows-latest` to
produce the NSIS installer.

## ToS / integrity note
This app only hosts Meta's **official** WhatsApp Web client; it does not reverse-engineer or
abuse the WhatsApp protocol. The user scans a QR code with their own phone and signs in as
themselves. Please respect WhatsApp's Terms of Service and use the app for personal, lawful
messaging.

Wisp is an independent, unofficial client. It is **not affiliated with, sponsored by, or endorsed
by Meta Platforms, Inc.** "WhatsApp" is a trademark of Meta Platforms, Inc., used here only to
describe the service this client connects to.

## Architecture
See [ARCHITECTURE.md](ARCHITECTURE.md) and `docs/decisions/`.