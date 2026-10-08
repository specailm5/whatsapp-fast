# ADR-001: Tauri over Electron for the Windows shell

## Status
Accepted

## Context
We need a native Windows desktop shell that hosts the official WhatsApp Web client.
The dominant path is Electron (which the official WhatsApp desktop uses), but Electron
bundles a full Chromium runtime per app: high RAM, high CPU, slow cold start, and a large
installer.

## Decision
Use **Tauri 2** (Rust backend + the OS-native WebView2 on Windows). The shell loads
`https://web.whatsapp.com` in the WebView2.

## Consequences
+ Low memory and CPU footprint (OS WebView, no bundled Chromium).
+ Fast cold start and small installer (NSIS).
+ Native tray, notifications, shortcuts, and windowing via Tauri plugins.
\- The embedded engine is the OS WebView2, so behavior is tied to the installed WebView2
   runtime (bootstrapped on Windows 10; ships on Windows 11).
\- Some Chromium-only APIs available in Electron are not present in WebView2; we avoid
   depending on them.
