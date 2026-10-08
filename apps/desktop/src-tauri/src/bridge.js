/* Wisp — shared Tauri IPC bridge helper.
 *
 * Injected at document-start on web.whatsapp.com (before the scripts that use it).
 * Resolves the Tauri `invoke` bridge lazily — it may not exist yet at document-start —
 * and exposes it as window.__WF_INVOKE__ for rail_settings.js and badge.js to share.
 *
 * Remote pages sometimes only expose the low-level __TAURI_INTERNALS__ bridge, so
 * several resolution paths are tried in order before rejecting.
 */
(function () {
  'use strict';
  if (window.location.origin !== 'https://web.whatsapp.com') return;

  function invoke(cmd, args) {
    var tauri = window.__TAURI__;
    if (tauri && tauri.core && tauri.core.invoke) return tauri.core.invoke(cmd, args || {});
    if (tauri && tauri.invoke) return tauri.invoke(cmd, args || {});
    var internals = window.__TAURI_INTERNALS__;
    if (internals && internals.invoke) return internals.invoke(cmd, args || {});
    return Promise.reject(new Error('Tauri bridge unavailable'));
  }

  window.__WF_INVOKE__ = invoke;
})();
