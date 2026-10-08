# Architecture — WhatsApp Fast

## Overview
A **native Windows desktop application** that hosts the **official WhatsApp Web** client
in a **Tauri 2** WebView2, with a fast, low-resource native shell around it. The goal is a
fast, responsive, native-feeling container — not a reimplementation of WhatsApp's protocol.

## Why this shape
- **ToS-safe:** we load Meta's own `https://web.whatsapp.com` client. No unofficial protocol
  library (Baileys/whatsmeow), no Matrix bridge, no account-ban risk.
- **Fast:** Tauri uses the OS-native WebView2 instead of a bundled Chromium → lower RAM/CPU,
  faster cold start, smaller installer than the official Electron desktop app.
- **Your observation honored:** the chat UI inside WhatsApp Web is already fast; the slow
  part is the *container's* navigation/responsiveness. This project optimizes the container.

## Component map
```
┌────────────────────────────────────────────────────────────┐
│  Tauri 2 native shell (Rust)                               │
│   ├─ WebView2 main window -> https://web.whatsapp.com       │
│   │     desktop Chrome user-agent override                 │
│   ├─ Settings window (local) -> ui/settings.html            │
│   │     auto-start toggle, hotkey, reset window, quit      │
│   ├─ System tray icon + menu (Open / Settings / Quit)       │
│   ├─ Global hotkey (default Ctrl+Shift+Space) to summon     │
│   ├─ Auto-start plugin (launch on Windows sign-in)          │
│   ├─ Single-instance + focus on re-launch                  │
│   ├─ Deep-link (whatsapp://, wa.me) -> open chat + focus  │
│   ├─ Privacy suite (CSS blur + panic curtain) in-page     │
│   ├─ Opener plugin -> external links to OS browser         │
│   ├─ Settings store (app_data_dir/settings.json)           │
│   └─ Persisted window geometry (size/pos/maximized)        │
└────────────────────────────────────────────────────────────┘
```

## Runtime flow
1. **Launch:** the native window restores saved geometry, a tray icon is created, and the
   WebView points at `https://web.whatsapp.com`.
2. **Login:** the user scans a QR code with the WhatsApp app on their phone; the client signs
   in as themselves (we never handle credentials).
3. **Navigation guard:** any navigation whose origin is not WhatsApp Web is handed to the
   OS default browser via `tauri-plugin-opener` — but only well-known `http`/`https`/`mailto`
   links; any other scheme (`file:`, custom handlers) is dropped, not launched.
4. **Close-to-tray:** closing the window hides it to the system tray (quitting only via the
   tray menu), so re-opening is instant.
5. **Hotkey / summon:** pressing the global shortcut (default `Ctrl+Shift+Space`) anywhere
   existing window instead of opening a second one.

## Key decisions
See `docs/decisions/` (ADRs):
- `001` — Tauri over Electron.
- `002` — Wrap the official web client, do not bridge the protocol.
- `003` — Desktop user-agent override and external navigation routing.
- `004` — Settings window, auto-start, and global hotkey.
- `005` — Deep-link handling and WebRTC (call) permissions in the WebView2 shell.

## Directory layout
```
apps/desktop/
  src-tauri/        Rust: main.rs, lib.rs, tauri.conf.json (capabilities inline)
    icons/          generated icon set
  ui/index.html     minimal splash/fallback page (frontendDist)
docs/decisions/     ADR records
```

## Non-goals (explicit)
- No protocol reimplementation, no unofficial account access.
- No live SFU voice/video calls beyond what the web client exposes.
- No remote push or cloud backup (not exposed by the web client).
- We do not read Meta's internal app state; the unread-count badge is driven only by the
  page's own `document.title` (best-effort, see ADR-007).

## Realtime / responsiveness targets
- Cold start well under the official Electron desktop client.
- RAM/CPU visibly lower than the official client (measured).
- Chat switching and window interaction not competing with the OS WebView's main thread.