# ADR-005: Deep-link handling and WebRTC (call) permissions in the WebView2 shell

## Status
Accepted

## Context
Users expect clicking a `whatsapp://…` link (or a contact/chat link) to open straight into
the app, and they expect WhatsApp Web voice/video calls to work inside the native shell.

## Deep-link handling
### Decision
Register the **`whatsapp` custom scheme** (and accept `https://wa.me/<phone>`) via
`tauri-plugin-deep-link`. On receiving a deep link, translate it into the equivalent
`https://web.whatsapp.com/send?phone=…` URL, navigate the main WebView there, and focus the
window.

### Why only a custom scheme on Windows
Tauri's deep-link plugin registers **URL schemes** (`whatsapp://`) reliably on Windows. The
https/universal-link `domains` option is **macOS-only** (needs a signed app + provisioning
profile). Capturing arbitrary `https://wa.me` clicks as our own protocol handler requires the
OS/browser to make us the default handler for that domain, which Windows doesn't automate. We
therefore handle `wa.me` URLs only when they are delivered to us, and register `whatsapp://`
as a first-class protocol.

## WebRTC / call (mic & camera) permissions
### Findings
wry's WebView2 layer only auto-allows the **clipboard** permission; it does not register a
handler for mic/camera. WebView2's default behavior is to show a permission prompt, but in a
Tauri embedded context prompts can be suppressed/denied. Tauri/wry do **not** expose a public
permission-grant API — granting mic/camera requires raw WebView2 COM interop via
`with_webview` + the `webview2-com` crate.

### Decision
Do **not** ship unverified unsafe COM interop. Verify on-device that WebView2 prompts for
mic/camera when WhatsApp Web starts a call. If it does (expected), calls work with no code
change. If it is suppressed, apply the documented fallback (below) inside a `with_webview`
block on the main window.

**Outcome (verified on-device):** the WebView2 permission prompt appears for mic/camera when
WhatsApp Web starts a call, so **voice/video calls work with no code change**. The unsafe
`webview2-com` COM fallback was therefore **not implemented** and remains documented only as a
contingency for a future build on which prompts are suppressed.

### Documented fallback (only if on-device testing shows prompts suppressed)
```text
main_win.with_webview(|pw| { /* downcast to the WebView2 controller and add a
PermissionRequested handler that ALLOWs Microphone and Camera kinds */ });
```
This requires `webview2-com` as a dependency and unsafe COM calls. It is intentionally
left as a follow-up so the shipped binary stays safe and verifiable.

## Consequences
+ `whatsapp://send?phone=…` and delivered `https://wa.me/…` links open the right chat and
  focus the app.
+ Calls rely on the standard WebView2 permission prompt (verified on-device) rather than a
  risky override.
\- `https://wa.me` links clicked in a browser are not silently captured as our protocol;
  only the `whatsapp://` scheme is first-class on Windows.

## Follow-up (Value 5): refactor to a pure, tested URL builder
The link-to-URL translation was pulled out of `route_deep_link` into a pure, unit-tested
function `build_send_url(scheme, host, path, phone, text)`, and `route_deep_link` now just
extracts the decoded pieces (`Url::query_pairs`) and calls it. This:
- combines `phone` AND `text` (previously it took only one);
- percent-encodes values via `url::form_urlencoded::Serializer` (safe text);
- strips leading `+` from the phone; and
- drops the earlier dead `if let Some((_, q)) = ... { let _ = q; }` no-op.

Two real gaps found during verification of the pure function (the original unit tests were
green but never exercised them) were fixed:
- `whatsapp://<number>` where the phone is the **host** (no `phone` query param) now falls
  back to a phone-like host (one containing a digit); a host with no digits, such as `chat`,
  is not mistaken for a phone.
- `http://wa.me/...` is accepted alongside `https://wa.me/...`.

Covered by unit tests: whatsapp:// via query phone/text/both, whatsapp:// phone-as-host (+text),
leading-`+` stripping, wa.me via http and https, empty-path and unsupported-scheme rejection.
The pure function handles only these two schemes; any other scheme/host returns `None`.
