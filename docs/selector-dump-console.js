// WhatsApp Fast — selector dump (run in the WhatsApp Web F12 console)
// Paste this into DevTools Console on web.whatsapp.com. A small panel appears.
// Choose a category, make sure "Capture on click" is ON, then click a representative
// element in WhatsApp (e.g. one avatar, one name, one message bubble, one sticker...).
// Use "Copy JSON" and paste the result back. We bake the real selectors into CATS.
(function () {
  'use strict';
  if (window.location.origin !== 'https://web.whatsapp.com') { console.log('Run this on https://web.whatsapp.com'); return; }

  var CATS = [
    ['contactName', 'Contact/group name'],
    ['chatMessages', 'Message bubble text'],
    ['lastMessage', 'Chat-list preview text'],
    ['media', 'Photo/video in a chat'],
    ['profilePic', 'Profile picture <img>'],
    ['voiceNotes', 'Voice-note element'],
    ['stickers', 'Sticker element'],
    ['composeBar', 'Composer input area'],
    ['status', 'Status element'],
    ['reactions', 'Message reaction'],
    ['searchbar', 'Search bar'],
    ['communities', 'Community element'],
    ['contactInfo', 'Contact info panel'],
  ];
  var out = {};
  CATS.forEach(function (c) { out[c[0]] = []; });

  function cssEsc(s) {
    return String(s).replace(/[^a-zA-Z0-9_-]/g, function (c) { return '\\' + c.charCodeAt(0).toString(16) + ' '; });
  }

  // General selector: tag + every class in the element. Matches all identical components.
  function classSel(el) {
    if (!el.className || typeof el.className !== 'string') return null;
    var cls = el.className.trim().split(/\s+/).filter(Boolean);
    if (!cls.length) return null;
    return el.tagName.toLowerCase() + cls.map(function (c) { return '.' + cssEsc(c); }).join('');
  }

  // Nearest ancestor (or self) with a data-testid.
  function testidSel(el) {
    var n = el;
    while (n && n.nodeType === 1) {
      var dt = n.getAttribute && n.getAttribute('data-testid');
      if (dt) return { sel: '[data-testid="' + cssEsc(dt) + '"]', onNode: n.tagName.toLowerCase() };
      n = n.parentElement;
    }
    return null;
  }

  // Precise nth-of-type path (matches exactly this one element).
  function pathSel(el) {
    var parts = [], node = el, limit = 10;
    while (node && node.nodeType === 1 && limit-- > 0) {
      var t = node.tagName.toLowerCase();
      if (t === 'body' || t === 'html') break;
      var seg = t;
      var dt = node.getAttribute && node.getAttribute('data-testid');
      if (dt) seg += '[data-testid="' + cssEsc(dt) + '"]';
      else if (node.hasAttribute && node.hasAttribute('role')) seg += '[role="' + node.getAttribute('role') + '"]';
      var idx = 1, sib = node.previousElementSibling;
      while (sib) { if (sib.tagName === node.tagName) idx++; sib = sib.previousElementSibling; }
      parts.unshift(seg + ':nth-of-type(' + idx + ')');
      var cand = parts.join(' > ');
      try { if (document.querySelectorAll(cand).length === 1) return cand; } catch (e) {}
      node = node.parentElement;
    }
    return null;
  }

  // ---- panel ----
  var panel = document.createElement('div');
  panel.id = 'wfd-panel';
  panel.style.cssText = 'position:fixed;top:8px;right:8px;z-index:999999;background:#111b21;color:#d1d7db;padding:12px 14px;border-radius:10px;font:13px/1.4 system-ui;box-shadow:0 6px 20px rgba(0,0,0,.6);max-width:420px;';
  panel.innerHTML =
    '<div style="font-weight:600;margin-bottom:6px">Selector capture</div>' +
    '<div>Category: <select id="wfd-cat" style="background:#202c33;color:#d1d7db;border:1px solid #2a3942;border-radius:6px;padding:4px;font-size:12px">' +
    CATS.map(function (c) { return '<option value="' + c[0] + '">' + c[1] + '</option>'; }).join('') +
    '</select></div>' +
    '<div style="margin-top:6px"><label style="display:flex;align-items:center;gap:5px"><input type="checkbox" id="wfd-arm" checked style="accent-color:#25d366"> Capture on click (Esc = OFF)</label></div>' +
    '<div style="margin-top:8px;display:flex;gap:6px"><button id="wfd-copy" style="background:#25d366;color:#0b141a;border:none;border-radius:6px;padding:6px 10px;font-weight:600;cursor:pointer">Copy JSON</button><button id="wfd-clear" style="background:#202c33;color:#d1d7db;border:1px solid #2a3942;border-radius:6px;padding:6px 10px;cursor:pointer">Clear</button></div>' +
    '<pre id="wfd-log" style="white-space:pre-wrap;max-height:260px;overflow:auto;margin-top:8px;background:#0b141a;padding:8px;border-radius:6px;font-size:11px"></pre>';
  document.documentElement.appendChild(panel);

  var catSel = panel.querySelector('#wfd-cat');
  var armEl = panel.querySelector('#wfd-arm');
  var logEl = panel.querySelector('#wfd-log');
  var hl = null;

  function isPanel(t) { return t && panel.contains(t); }

  function onHover(e) {
    if (!armEl.checked || isPanel(e.target)) return;
    if (hl) hl.style.outline = '';
    hl = e.target;
    hl.style.outline = '3px solid #25d366';
  }
  function onClick(e) {
    if (!armEl.checked) return;
    if (isPanel(e.target)) return;
    e.preventDefault(); e.stopPropagation();
    var el = e.target;
    var cat = catSel.value;
    var info = { cat: cat, tag: el.tagName.toLowerCase() };
    var role = el.getAttribute && el.getAttribute('role');
    if (role) info.role = role;
    var dt = el.getAttribute && el.getAttribute('data-testid');
    if (dt) info.dataTestid = dt;
    var cs = classSel(el);
    if (cs) { info.classSel = cs; info.classMatches = document.querySelectorAll(cs).length; }
    var ts = testidSel(el);
    if (ts) { info.testidSel = ts.sel; info.testidMatches = document.querySelectorAll(ts.sel).length; info.testidOn = ts.onNode; }
    var ps = pathSel(el);
    if (ps) info.uniquePathSel = ps;
    out[cat].push(info);
    var last = out[cat][out[cat].length - 1];
    logEl.textContent = JSON.stringify(out, null, 1);
    console.log('[wfd]' + cat + ':', last);
  }
  function onKey(e) {
    if (e.key === 'Escape') { armEl.checked = false; if (hl) hl.style.outline = ''; hl = null; }
  }

  document.addEventListener('mousemove', onHover, true);
  document.addEventListener('click', onClick, true);
  window.addEventListener('keydown', onKey, true);

  panel.querySelector('#wfd-copy').addEventListener('click', function () {
    var text = JSON.stringify(out, null, 1);
    (navigator.clipboard ? navigator.clipboard.writeText(text) : Promise.reject(new Error('no clipboard')))
      .then(function () { logEl.textContent = text + '\n\n[COPIED]'; })
      .catch(function () { window.prompt('Copy this JSON:', text); });
  });
  panel.querySelector('#wfd-clear').addEventListener('click', function () {
    out = {}; CATS.forEach(function (c) { out[c[0]] = []; });
    logEl.textContent = '';
  });

  console.log('[wfd] Ready. Choose a category, then click the matching element. Esc disables capture.');
  console.log('[wfd] Hint: click the element you WANT blurred (the avatar img, the name span, a message bubble, a sticker).');
})();
