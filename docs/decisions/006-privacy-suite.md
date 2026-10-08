# ADR-006: Privacy suite (blur / hover-reveal) as an injected content script

## Status
Accepted

## Context
Users want the shoulder-surf / screen-share protections that browser extensions such as
the "Privacy Suite Extension for WhatsApp Web" provide: blur contact names, message text,
previews, profile pictures, media, etc., and reveal them on hover. In our shell we load the
official `https://web.whatsapp.com` in a WebView2, so we must bring this behavior into the
shell ourselves. We must also stay within the binding ToS decision (ADR-002): host the
official web client, no protocol reimplementation, no data extraction.

## Decision
Implement the privacy suite as a **client-side, local, purely-visual** blur layer injected
into the WhatsApp Web page. We use Tauri's `WebviewWindowBuilder` with:

- `.initialization_script(PRIVACY_BOOT_JS)` - injects a JS boot script at document-start in
  the page context (the WebView2 equivalent of a content script).
- `webview.eval(...)` from Rust commands and the `.on_page_load` hook - to push the saved
  config into the live page and re-apply it on every full page load.

The blur is the **embedded** `styles.css` from the established "Privacy Suite Extension for
WhatsApp Web" (default-blur engine). It is **pure CSS that blurs by default**, so a chat is
blurred on its FIRST frame — no render→wait→blur flash, and no MutationObserver is needed.

`privacy_boot.js` is now only a thin **controller**: it maps the saved config onto the
`<html>` hooks that `styles.css` already reads — `html.wa-off`, `html.wa-show-<category>`,
the `--wa-scale-<category>` blur-intensity variables, and `<html data-wa-notif>` (read by the
embedded `page-notify.js` notification-privacy wrapper). On load Rust pushes the config via
`webview.eval(...)`.

The combined document-start script is built in Rust and shipped as **one** initialization
script: (1) inject the `<style>` holding `styles.css`, (2) run `page-notify.js` (wraps
`window.Notification`), (3) run the `privacy_boot.js` controller.

Configuration (enabled, per-category booleans, blur strength, notification mode) is persisted
to `privacy.json` and surfaced through Tauri commands to the settings window.

## Why this is ToS-safe
The suite only transforms what is rendered on the **user's own screen**. It does not touch the
WhatsApp protocol, does not read or transmit any WhatsApp data, and does not change what
Meta's servers observe. It is the same class as a laptop privacy-screen filter. No ban risk
is introduced relative to ADR-002.

## The selectors
The extension's `styles.css` ships **verified selectors** for a known WhatsApp build, using
durable `data-testid` hooks (`msg-container`, `cell-frame-title` / `cell-frame-container`,
`media-canvas-img`, `media-url-provider`, `sticker-container`, `author`, `compose-box`,
`drawer-right`, `status-thumbnail`, `_ak4g` / `_ak4n`, `lexical-rich-text-input`, `data-testid="author"`,
etc.). Each rule targets the **most precise element** (the name span, the avatar `<img>`, the
message text bubble, the media tile) rather than a broad container; selectors that stop matching
simply do nothing (fail silently). Reveal-on-hover is per-leaf and instant (`transition: filter 0s`).

Because Meta renames classes on every update, `styles.css` must be re-verified against a live
DOM. A devtools console script (`docs/selector-dump-console.js`) dumps the real selectors; the
verified set is then folded into `styles.css` and the app rebuilt (a one-time developer step).

## Consequences
+ Native, integrated privacy layer; no external extension dependency.
+ Default-blur (CSS-first) means no render→wait→blur flash when a chat opens.
+ Pure CSS is robust against SPA re-renders; reveal-on-hover uses native CSS.
+ Notification privacy via the embedded `page-notify.js` (redact / suppress).
+ Configurable categories + blur strength; quick global hotkey for reveal-all / blur-all.
+ Every blur rule targets a precise leaf (never a broad container), so it never covers more than intended.
- `styles.css` must be re-verified when WhatsApp updates (refreshed via a devtools DOM dump).
- Blur is a visual layer only; it does not (and should not, per ADR-002) capture or store any
  WhatsApp content.

## Follow-up (Value 2): hover-reveal toggle + auto-hide on focus loss
The suite gained two configurable behaviours (both persisted in `privacy.json`):
- **Reveal on hover** (`hover_reveal`, default ON): the controller now sets/removes
  `html.wa-no-hover` on `<html>` from the config. OFF disables reveal-on-hover entirely
  (the CSS already supported the class; the settings toggle now drives it).
- **Auto-hide on focus loss** (`auto_hide_on_focus_loss`, default OFF): when the main
  window loses focus, the shell evaluates `__WF_PRIVACY__.blurAll()` to blur everything
  (shoulder-surf guard); on regaining focus it re-applies the saved config. Wired in the
  Rust `on_window_event` `Focused` handler.

These remain purely visual and local (per ADR-002), and add no new selectors or data access.

## Follow-up (Value 3): hash the panic PIN
The panic PIN is no longer persisted in plaintext. `PrivacyConfig.panic_pin` now holds a
**salted SHA-256 hex hash**, with a new `pin_salt` field alongside it. Hashing is done
client-side with `crypto.subtle` (WebCrypto, available in both the local settings window and
the remote HTTPS page), so no new Rust dependency was needed. This is reflected in:
- Settings shows **Set / Clear PIN** buttons (the password field is a "new PIN" input only).
- The boot script verifies by recomputing sha256(salt + entered) and comparing to the hash. A
  legacy non-digit PIN uses the password field + Unlock; a digit-only PIN uses the keypad and
  auto-submits on the last digit — there is **no OK key** (see the follow-up below).
- An existing pre-hash plaintext `panic_pin` (no `pin_salt`) is migrated to a hash on first
  Settings load; until then it is treated as "no lock" (never a plaintext-active lock).

## Follow-up: restored keypad UX + fixed-length PIN
The hash hid the PIN length, which the original keypad's dot-progress and auto-submit-on-last-
digit behaviour depended on. Two fields were added to the config (`pin_len`, `pin_digits_only`)
so the keypad can fill dots and submit automatically at the PIN length again. The panic PIN is
now enforced as **exactly 4 digits** (numeric) — the standard passcode length, fast to type and
sufficient for a shoulder-surf curtain; any other length is rejected in Settings. The password
field is still shown for non-digit (legacy) PINs as a fallback.

**Do not add an OK/submit button to the keypad.** It was tried and rejected: the button rendered
as an awkward unrelated control and added a tap the user does not need. A digit-only PIN must
auto-submit on the last digit and allow backspace to correct; that is the intended and tested
UX. Any future change must keep auto-submit as the sole submit path for the numeric keypad.

**Migration backfill.** A PIN saved before `pin_len`/`pin_digits_only` were added loads as
`len=0`, which used to disable the keypad and auto-submit (showing the password field instead).
Because the PIN design is now exactly 4 numeric digits, the boot script treats a locked PIN with
no recorded length as the standard 4-digit keypad and auto-submits on the 4th digit. A user with
a genuinely different legacy PIN can Clear and re-set it in Settings.

**Caveat: this is a convenience lock, not cryptographic protection.** The PIN is exactly
4 digits and the salt is stored alongside the hash in `privacy.json`, so the 10,000-value
space is trivially brute-forced by anyone who can read that file. It is meant to stop a
casual shoulder-surfer, not a determined local attacker.
