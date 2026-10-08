# ADR-003: Desktop user-agent override and external navigation routing

## Status
Accepted

## Context
The OS WebView2's default user-agent does not look like a real desktop browser, which can
make WhatsApp Web report an 'unsupported browser' and refuse to load the full client.
Also, links inside the client should open in the user's default browser, not inside the app.

## Decision
Set a current desktop Chrome user-agent on the main WebView (configurable in one place).
Route any navigation whose origin is not `https://web.whatsapp.com` to the OS browser via the
`tauri-plugin-opener`.

## Consequences
+ WhatsApp Web loads with the full desktop experience.
+ External links never escape into the app and never cause an in-webview navigation away
   from WhatsApp.
+ The user-agent is centralized so a Meta web-client change can be addressed with a single
   edit, not a rewrite.
