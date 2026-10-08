/* Wisp — Taskbar unread-badge driver.
 *
 * The native shell wants a taskbar overlay badge (and a taskbar flash on a new
 * message while the window is unfocused), but per ADR-002 we never read Meta's
 * internal state. The one public, page-own signal that carries the unread count is
 * WhatsApp Web's document.title, which becomes "(N) WhatsApp" when there are N
 * conversations with unread messages.
 *
 * This script:
 *   - parses the leading (N) out of document.title,
 *   - renders a small red numeric badge into a transparent canvas and sends the
 *     pixels back as base64 RGBA (row-major), which Rust decodes into a
 *     tauri::image::Image via set_overlay_icon(),
 *   - pushes the count so Rust can flash the taskbar when N increases.
 *
 * It degrades gracefully: if a future WhatsApp build stops putting (N) in the
 * title, we simply send count 0 and no badge is shown — fail-safe, never a crash.
 */
(function () {
  'use strict';
  if (window.location.origin !== 'https://web.whatsapp.com') return;

  // Shared Tauri bridge (window.__WF_INVOKE__, injected by bridge.js). The stub only
  // runs if that script somehow didn't load.
  var invoke = window.__WF_INVOKE__ || function (cmd, args) {
    return Promise.reject(new Error('Tauri bridge unavailable'));
  };

  var lastCount = -1; // sentinel so the very first read is always pushed
  var timer = null;

  // "(N) WhatsApp" or "(N, M) WhatsApp" -> N (conversations with unread).
  function parseTitle(t) {
    if (!t) return 0;
    var m = t.match(/^\s*\((\d+)(?:\s*,\s*\d+)?\)/);
    if (!m) return 0;
    var n = parseInt(m[1], 10);
    return isFinite(n) ? n : 0;
  }

  // Render the badge into a canvas and return raw RGBA (no decoding needed in Rust).
  function drawBadge(count) {
    var dpr = Math.min(window.devicePixelRatio || 1, 2); // cap so the IPC payload stays small
    var size = Math.round(32 * dpr);
    var c = document.createElement('canvas');
    c.width = size;
    c.height = size;
    var g = c.getContext('2d');
    if (!g) return null;
    g.clearRect(0, 0, size, size);

    // Red disc with a hair of margin so the OS doesn't crop the edge.
    g.beginPath();
    g.arc(size / 2, size / 2, size / 2 - 1 * dpr, 0, 2 * Math.PI);
    g.fillStyle = '#e53935';
    g.fill();

    var label = count > 9 ? '9+' : String(count);
    g.fillStyle = '#ffffff';
    g.font = 'bold ' + Math.round(size * 0.5) + 'px system-ui, "Segoe UI", sans-serif';
    g.textAlign = 'center';
    g.textBaseline = 'middle';
    g.fillText(label, size / 2, size / 2 + size * 0.02);

    var im = g.getImageData(0, 0, size, size);
    // Base64 keeps the IPC payload ~4x smaller than a JSON array of byte values.
    var bytes = new Uint8Array(im.data.buffer, im.data.byteOffset, im.data.length);
    var binary = '';
    for (var i = 0; i < bytes.length; i++) binary += String.fromCharCode(bytes[i]);
    return { rgba_b64: btoa(binary), width: im.width, height: im.height };
  }

  function push(count) {
    if (count === lastCount) return;
    lastCount = count;
    var payload = { count: count };
    if (count > 0) {
      var badge = drawBadge(count);
      if (badge) {
        payload.rgba_b64 = badge.rgba_b64;
        payload.width = badge.width;
        payload.height = badge.height;
      }
    }
    invoke('badge_set_count', payload).catch(function (_e) {});
  }

  function onTitle() {
    var n = parseTitle(document.title);
    if (timer) clearTimeout(timer);
    timer = setTimeout(function () { push(n); }, 200);
  }

  function attach() {
    var t = document.querySelector('title');
    if (!t) return false;
    if (t.__wfaAttached) return true;
    t.__wfaAttached = true;
    try {
      new MutationObserver(onTitle).observe(t, { childList: true, subtree: true, characterData: true });
    } catch (_e) {}
    return true;
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', function () {
      if (!attach()) setTimeout(attach, 300);
    });
  } else {
    attach();
  }

  // If a future build swaps out the <title> node entirely, re-attach.
  try {
    new MutationObserver(function () { attach(); })
      .observe(document.head || document.documentElement, { childList: true });
  } catch (_e) {}

  onTitle();
})();
