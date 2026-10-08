/* Wisp — notification-privacy wrapper.
 *
 * Wraps window.Notification so the privacy suite's "notification mode" (carried by
 * <html data-wa-notif>, set by privacy_boot.js) can control what the web client's
 * notifications show, without reading WhatsApp's internal state:
 *
 *   data-wa-notif="suppress" -> a silent, instantly-closed notification (no sound/popup)
 *   data-wa-notif="redact"   -> the body is replaced with a generic "New message"
 *   otherwise                -> passthrough to the native Notification
 *
 * The wrapper preserves the native prototype plus the static `permission`,
 * `requestPermission` and `maxActions`, so the page behaves as if
 * window.Notification had never been replaced.
 */
(function () {
  'use strict';
  var Native = window.Notification;
  if (typeof Native !== 'function') return;

  function Wrapped(title, options) {
    try {
      // Only apply privacy rules when the OS has granted notification permission;
      // otherwise fall straight through to the normal constructor.
      var granted = (function () {
        try { return Native.permission === 'granted'; } catch (e) { return false; }
      })();
      var mode = granted
        ? (function () {
            try { return document.documentElement.getAttribute('data-wa-notif') || 'off'; }
            catch (e) { return 'off'; }
          })()
        : 'off';

      if (mode === 'suppress') {
        var suppressed = new Native('', { silent: true, tag: 'wa-privacy-suppressed', body: '' });
        try { suppressed.close(); } catch (e) {}
        return suppressed;
      }

      if (mode === 'redact') {
        var opts = {};
        if (options && typeof options === 'object') {
          for (var key in options) {
            if (Object.prototype.hasOwnProperty.call(options, key)) opts[key] = options[key];
          }
        }
        opts.body = 'New message';
        delete opts.image;
        return new Native('WhatsApp', opts);
      }

      return new Native(title, options);
    } catch (err) {
      try { return new Native(title, options); } catch (e) { return; }
    }
  }

  try {
    Wrapped.prototype = Native.prototype;
    Wrapped.requestPermission = Native.requestPermission
      ? Native.requestPermission.bind(Native)
      : undefined;
    Object.defineProperty(Wrapped, 'permission', {
      get: function () {
        try { return Native.permission; } catch (e) { return 'default'; }
      },
      configurable: true
    });
    if (Native.maxActions !== undefined) {
      try { Wrapped.maxActions = Native.maxActions; } catch (e) {}
    }
    window.Notification = Wrapped;
  } catch (e) {}
})();
