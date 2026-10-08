# ADR-007: Taskbar unread badge (read the public document.title)

## Status
Accepted

## Context
A native messaging shell is expected to show an unread-count badge on the Windows
taskbar button and flash when a new message arrives while the app is in the
background. ADR-002 forbids reading Meta's internal app state, so a badge cannot be
driven by the page's internal data. However, WhatsApp Web exposes the unread count in
its own public `document.title` (it becomes `"(N) WhatsApp"` when there are N
conversations with unread messages).

## Decision
Drive the taskbar overlay badge and "flash on new message" from `document.title`,
observed by an injected script (`badge.js`) that runs in the page context. The script
parses the leading `(N)`, renders a small numeric badge as raw RGBA, and pushes
`{ count, rgba, width, height }` to a new Rust command (`badge_set_count`). Rust
builds a `tauri::image::Image` via `Image::new_owned` and applies it with
`WebviewWindow::set_overlay_icon`; it flashes with `request_user_attention`.

The badge and flash are independently toggleable in the Settings window and persisted
in `settings.json` (`unread_badge`, `flash_on_new_message`, both default ON;
`#[serde(default = "...")]` keeps older settings files valid).

## Why this is ToS-safe
The count is taken from a value the page itself renders (the tab title), not from any
internal state, protocol, or data store. If a future build stops putting `(N)` in the
title, the parser returns 0 and no badge is shown — the feature degrades silently
instead of breaking.

## Consequences
+ Native unread-badge + taskbar-flash behaviour with no protocol or data access.
+ Independent toggles; backward-compatible persisted config.
- The badge count reflects what the page publishes in the title (the same count the
  user sees in the browser tab), which is the chat/summary count, not a per-message
  breakdown.
- A WhatsApp build change that alters the title format would disable the badge until
  `badge.js`'s parser is updated (fail-safe, never a crash).
