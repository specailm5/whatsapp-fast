# ADR-002: Wrap the official WhatsApp Web client, do not bridge the protocol

## Status
Accepted

## Context
The user wants all WhatsApp features and a fast UI but explicitly does **not** want to
trigger WhatsApp Terms of Service, is not a business, and observes that the chat UI itself
is already fast (the slow part is the app container's navigation/responsiveness).

## Decision
Do **not** reimplement the WhatsApp protocol (no Baileys / whatsmeow / Matrix bridge).
Instead, host the **official** `https://web.whatsapp.com` client verbatim in a Tauri
WebView2 and add a fast native shell around it.

## Consequences
+ Zero ToS / account-ban risk from a reverse-engineered protocol; we load Meta's own
   web client and the user signs in as themselves.
+ We preserve WhatsApp's fast chat rendering; we optimize the shell (startup, RAM, CPU,
   navigation responsiveness, OS integration).
\- Feature additions are limited to what the web client exposes (no live SFU voice/video,
   no remote push, no cloud backup).
\- We do not read Meta's internal app state, so an unread-count taskbar badge cannot be
   truthfully driven by the page; it is exposed as a best-effort shell hook.
