/* WhatsApp Fast — Privacy controller (replaces the old hardcoded-selector engine).
 *
 * The blur itself is delivered by the embedded "Privacy Suite for WhatsApp Web"
 * DEFAULT-BLUR stylesheet (styles.css), which is pure CSS and blurs by default so
 * a chat is blurred on its FIRST frame (zero-delay — no render→wait→blur flash).
 *
 * This script's only job is to map the saved config onto the <html> hooks that the
 * stylesheet already reads:
 *   html.wa-off              master OFF  -> reveal everything
 *   html.wa-show-<category>  category DISABLED -> reveal it
 *   html style="--wa-scale-<category>:N"  global blur-intensity multiplier
 *   html data-wa-notif       notification mode: "" | redact | suppress (page-notify.js)
 *   html.wa-ready            engine initialised (once settings arrive)
 *
 * Exposes window.__WF_PRIVACY__:
 *   .apply(config)        - set hooks from config (called by Rust on load + on change)
 *   .revealAll() / .blurAll() / .toggleAll()
 *   .activatePanic() / .deactivatePanic() / .togglePanic()
 *   .getState()
 *
 * PANIC MODE
 * ----------
 * The stylesheet ships the panic curtain (html.wa-panic + #wa-panic-overlay) but the
 * CSS only styles it; this controller owns the behaviour:
 *   - togglePanic() (wired to the global hotkey in Rust and the Settings "Test"
 *     button) mounts/removes the overlay and toggles html.wa-panic.
 *   - When a PIN is set (config.panic_pin holds a salted SHA-256 hash), the overlay
 *     requires it to clear: Esc and the hotkey do NOT reveal. A digit-only PIN uses the
 *     on-screen keypad, which fills dots and AUTO-SUBMITS on the last digit; any other
 *     PIN uses the password field + Unlock. The entered value is hashed and compared.
 *   - With no PIN, Esc (and the hotkey) instantly reveal.
 */
(function () {
  'use strict';
  if (window.location.origin !== 'https://web.whatsapp.com') return;

  var state = { revealAll: false };

  // The extension's category slugs (from styles.css). Our config category key IS the
  // same slug. Checkbox TRUE = blur (no wa-show-<cat>); FALSE = reveal (add it).
  var CATS = ['contactName', 'chatMessages', 'lastMessage', 'media', 'profilePic',
    'voiceNotes', 'stickers', 'composeBar', 'status', 'reactions', 'searchbar',
    'communities', 'contactInfo'];

  function root() { return document.documentElement; }

  // Salted-SHA-256 hex digest via WebCrypto. The panic PIN is stored as this hash,
  // so verifying requires recomputing it from the salt + entered value.
  function sha256Hex(str) {
    if (!window.crypto || !window.crypto.subtle || !window.crypto.subtle.digest) {
      return Promise.reject(new Error('WebCrypto unavailable'));
    }
    var bytes = new TextEncoder().encode(str);
    return window.crypto.subtle.digest('SHA-256', bytes).then(function (buf) {
      var arr = new Uint8Array(buf);
      var hex = '';
      for (var i = 0; i < arr.length; i++) hex += ('0' + arr[i].toString(16)).slice(-2);
      return hex;
    });
  }

  // ---------------- Panic-mode runtime state ----------------
  var panic = { active: false, pin: '', pin_salt: '', pin_len: 0, pin_digits_only: false, pinBuf: '', locked: false, overlay: null };

  function apply(configObj) {
    var cfg = configObj || {};
    var html = root();
    var enabled = !!cfg.enabled;

    // Master switch.
    html.classList.toggle('wa-off', !enabled);

    // Per-category reveal + intensity scale (one pass over all extension slugs).
    var blur = (typeof cfg.blur === 'number' ? cfg.blur : 10);
    var scale = blur / 10;
    CATS.forEach(function (c) {
      var hide = !(cfg.categories && cfg.categories[c] === false);
      html.classList.toggle('wa-show-' + c, !hide);
      html.style.setProperty('--wa-scale-' + c, String(scale));
    });

    // Hover-reveal: `html.wa-no-hover` disables reveal-on-hover entirely.
    html.classList.toggle('wa-no-hover', !(cfg.hover_reveal !== false));

    // Notification privacy (read lazily by page-notify.js on each notify()).
    var notif = cfg.notif || '';
    if (notif) html.setAttribute('data-wa-notif', notif);
    else html.removeAttribute('data-wa-notif');

    // Panic-mode PIN. panic_pin is a salted SHA-256 hash (never plaintext); the lock
    // engages only when a non-empty salt is also present. pin_len / pin_digits_only let
    // the keypad render dots and auto-submit on the last digit.
    panic.pin = cfg.panic_pin || '';
    panic.pin_salt = cfg.pin_salt || '';
    panic.pin_len = cfg.pin_len || 0;
    panic.pin_digits_only = !!cfg.pin_digits_only;
    panic.locked = !!(panic.pin && panic.pin_salt);
    // Backfill: a PIN saved before the pin_len field existed loads as len=0, which disables
    // the keypad and auto-submit. The app's PIN design is exactly 4 numeric digits, so treat a
    // locked PIN with no recorded length as the standard 4-digit keypad (auto-submit on the 4th
    // digit). A user with a genuinely different legacy PIN can Clear + re-set it in Settings.
    if (panic.locked && panic.pin_len === 0) {
      panic.pin_len = 4;
      panic.pin_digits_only = true;
    }

    html.classList.add('wa-ready');
    if (state.revealAll) html.classList.add('wa-off');
    return true;
  }

  function revealAll() { root().classList.add('wa-off'); state.revealAll = true; }
  function blurAll() { root().classList.remove('wa-off'); state.revealAll = false; }
  function toggleAll() { if (state.revealAll) blurAll(); else revealAll(); return state.revealAll; }

  // ========================= PANIC MODE =========================

  function buildOverlay() {
    var host = document.body || document.documentElement;
    if (!host) return null;
    var ov = document.createElement('div');
    ov.id = 'wa-panic-overlay';
    ov.innerHTML =
      '<div class="wa-panic-card">' +
        '<div class="wa-panic-shield" aria-hidden="true">' +
          '<svg viewBox="0 0 24 24" width="72" height="72" fill="currentColor" ' +
            'style="display:inline-block"><path d="M12 1L3 5v6c0 5.55 3.84 10.74 9 12 ' +
            '5.16-1.26 9-6.45 9-12V5l-9-4z"/></svg>' +
        '</div>' +
        '<div class="wa-panic-title">WhatsApp hidden</div>' +
        '<div class="wa-panic-sub" id="wa-panic-sub">Press <b>Esc</b> to reveal</div>' +
        '<div class="wa-panic-dots" id="wa-panic-dots"></div>' +
        '<div class="wa-panic-keypad" id="wa-panic-keypad"></div>' +
        '<div class="wa-panic-pass" id="wa-panic-pass">' +
          '<input type="password" id="wa-panic-input" autocomplete="off" ' +
            'aria-label="Unlock PIN" />' +
          '<button type="button" class="wa-panic-unlock" id="wa-panic-unlock">Unlock</button>' +
        '</div>' +
        '<div class="wa-panic-err" id="wa-panic-err"></div>' +
      '</div>';
    host.appendChild(ov);
    buildKeypad(ov);

    // Typed password path (works alongside the keypad).
    var input = ov.querySelector('#wa-panic-input');
    var unlock = ov.querySelector('#wa-panic-unlock');
    input.addEventListener('keydown', function (e) {
      // Allow the user to type; Enter submits. Escape is handled globally below.
      if (e.key === 'Enter') { e.preventDefault(); submitPin(input.value); }
    });
    unlock.addEventListener('click', function () { submitPin(input.value); });
    return ov;
  }

  function buildKeypad(ov) {
    var kp = ov.querySelector('#wa-panic-keypad');
    // No OK key: a digit-only PIN auto-submits once the last digit is entered.
    var layout = ['1', '2', '3', '4', '5', '6', '7', '8', '9', 'back', '0', ''];
    layout.forEach(function (k) {
      if (k === '') {
        kp.appendChild(document.createElement('div')).className = 'wa-key-spacer';
        return;
      }
      var b = document.createElement('button');
      b.type = 'button';
      b.className = 'wa-key' + (k === 'back' ? ' wa-key-aux' : '');
      b.setAttribute('data-key', k);
      b.textContent = k === 'back' ? '⌫' : k;
      kp.appendChild(b);
    });
    kp.addEventListener('click', function (ev) {
      var btn = ev.target && ev.target.closest ? ev.target.closest('.wa-key') : null;
      if (!btn) return;
      keypadPress(btn.getAttribute('data-key'));
    });
  }

  function renderPanicUI() {
    var ov = panic.overlay;
    if (!ov) return;
    var sub = ov.querySelector('#wa-panic-sub');
    var keypad = ov.querySelector('#wa-panic-keypad');
    var pass = ov.querySelector('#wa-panic-pass');
    var dots = ov.querySelector('#wa-panic-dots');
    if (panic.locked) {
      // Digit-only PIN -> on-screen keypad + dot progress + auto-submit on the last
      // digit. Any other PIN -> typed password field + Unlock.
      var digitOnly = panic.pin_len > 0 && panic.pin_digits_only;
      if (sub) sub.innerHTML = digitOnly ? 'Enter your PIN' : 'Enter your PIN to reveal WhatsApp';
      if (keypad) keypad.style.display = digitOnly ? '' : 'none';
      if (dots) dots.style.display = digitOnly ? '' : 'none';
      if (pass) pass.style.display = digitOnly ? 'none' : '';
      if (digitOnly) renderDots();
    } else {
      if (sub) sub.innerHTML = 'Press <b>Esc</b> to reveal';
      if (keypad) keypad.style.display = 'none';
      if (dots) dots.style.display = 'none';
      if (pass) pass.style.display = 'none';
    }
  }

  function renderDots() {
    var ov = panic.overlay;
    if (!ov || !panic.locked) return;
    var wrap = ov.querySelector('#wa-panic-dots');
    if (!wrap) return;
    var n = Math.max(panic.pin_len, 4);
    var cur = panic.pinBuf.length;
    var html = '';
    for (var i = 0; i < n; i++) {
      html += '<div class="wa-dot' + (i < cur ? ' filled' : '') + '"></div>';
    }
    wrap.innerHTML = html;
  }

  function setErr(msg) {
    var ov = panic.overlay;
    if (!ov) return;
    var err = ov.querySelector('#wa-panic-err');
    if (err) err.textContent = msg;
  }

  function keypadPress(k) {
    if (!panic.locked || !panic.active) return;
    setErr('');
    if (k === 'back') {
      panic.pinBuf = panic.pinBuf.slice(0, -1);
      renderDots();
    } else if (/^[0-9]$/.test(k)) {
      if (panic.pinBuf.length < Math.max(panic.pin_len, 8)) {
        panic.pinBuf += k;
        renderDots();
        // Auto-submit on the last digit, so the user doesn't need to press OK.
        if (panic.pin_len > 0 && panic.pinBuf.length >= panic.pin_len) {
          var buf = panic.pinBuf;
          panic.pinBuf = '';
          setTimeout(function () {
            if (panic.active && panic.locked) submitPin(buf);
          }, 90);
        }
      }
    }
  }

  function submitPin(value) {
    if (!panic.active || !panic.locked) return;
    setErr('');
    sha256Hex(panic.pin_salt + value).then(function (hash) {
      if (hash === panic.pin) {
        deactivatePanic();
        return;
      }
      panic.pinBuf = '';
      renderDots();
      setErr('Wrong PIN');
      var ov = panic.overlay;
      if (ov) {
        var inp = ov.querySelector('#wa-panic-input');
        if (inp) inp.value = '';
        var card = ov.querySelector('.wa-panic-card');
        if (card) {
          card.classList.remove('wa-shake');
          void card.offsetWidth; // reflow to restart the shake animation
          card.classList.add('wa-shake');
        }
      }
    }).catch(function () {
      setErr('PIN check failed');
    });
  }

  function onPanicDocKey(e) {
    if (e.key === 'Escape') {
      // With a PIN set, Esc must NOT reveal — the lock holds.
      if (!panic.locked) {
        e.preventDefault();
        e.stopPropagation();
        deactivatePanic();
      }
    }
    // Every other key is left alone so typing in the password field works.
  }

  function activatePanic() {
    if (panic.active) return;
    panic.active = true;
    panic.pinBuf = '';
    root().classList.add('wa-panic');
    if (!panic.overlay) panic.overlay = buildOverlay();
    if (panic.overlay) {
      renderPanicUI();
      // Convenience: focus the unlock field so the PIN can be typed immediately.
      if (panic.locked) {
        var input = panic.overlay.querySelector('#wa-panic-input');
        if (input) input.focus();
      }
    }
    root().addEventListener('keydown', onPanicDocKey, true);
  }

  function deactivatePanic() {
    if (!panic.active) return;
    panic.active = false;
    root().classList.remove('wa-panic');
    root().removeEventListener('keydown', onPanicDocKey, true);
    if (panic.overlay) {
      panic.overlay.remove();
      panic.overlay = null;
    }
  }

  function togglePanic() {
    if (panic.active) {
      // A PIN lock means the curtain only clears by entering the PIN; the hotkey
      // (and Esc) are deliberately disabled while locked.
      if (panic.locked) return;
      deactivatePanic();
    } else {
      activatePanic();
    }
  }

  window.__WF_PRIVACY__ = {
    apply: apply,
    revealAll: revealAll,
    blurAll: blurAll,
    toggleAll: toggleAll,
    activatePanic: activatePanic,
    deactivatePanic: deactivatePanic,
    togglePanic: togglePanic,
    getState: function () {
      return {
        revealAll: state.revealAll,
        panicActive: panic.active,
        panicLocked: panic.locked
      };
    }
  };
})();
