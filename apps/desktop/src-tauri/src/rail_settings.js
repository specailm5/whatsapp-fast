/* WhatsApp Fast — nav-rail Settings entry.
 *
 * Injected into web.whatsapp.com at document-start. Adds a Settings gear as a real
 * item INSIDE the left nav rail (above the Chats icon) and opens the separate
 * settings window on click.
 *
 * The rail is located via its verified testid ("navbar-primary-section"); the nav
 * items inside carry data-navbar-item="true". The gear is inserted as the FIRST child
 * of the item-list wrapper so it sits above the top nav item and participates in the
 * same layout. Insertion is PERSISTENT: a MutationObserver re-adds the gear whenever
 * React re-render removes it, and the fast path skips the DOM scan once it's attached
 * (that scan-every-mutation behaviour is what froze the app).
 */
(function () {
  'use strict';
  if (window.location.origin !== 'https://web.whatsapp.com') return;

  var ID = 'wfa-rail-settings';

  // Shared Tauri bridge (window.__WF_INVOKE__, injected by bridge.js). The stub only
  // runs if that script somehow didn't load.
  var invoke = window.__WF_INVOKE__ || function (cmd, args) {
    return Promise.reject(new Error('Tauri bridge unavailable'));
  };

  var GEAR = '<svg viewBox="0 0 24 24" width="24" height="24" fill="currentColor"><path d="M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.61l-1.92-3.32a.49.49 0 0 0-.59-.22l-2.39.96a7.03 7.03 0 0 0-1.62-.94l-.36-2.54a.48.48 0 0 0-.48-.41h-3.84a.48.48 0 0 0-.48.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96a.48.48 0 0 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58a.49.49 0 0 0-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.48-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6A3.6 3.6 0 1 1 12 8.4a3.6 3.6 0 0 1 0 7.2z"/></svg>';

  function isVisible(el) {
    if (!el || !el.getBoundingClientRect) return false;
    var s = getComputedStyle(el);
    if (s.display === 'none' || s.visibility === 'hidden') return false;
    var r = el.getBoundingClientRect();
    if (r.width < 10 || r.height < 10) return false;
    if (r.right <= 0 || r.left >= window.innerWidth || r.bottom <= 0 || r.top >= window.innerHeight) return false;
    return true;
  }

  // Verified container: the left nav rail. No geometry heuristics.
  function findRail() {
    var exact = document.querySelectorAll('[data-testid="navbar-primary-section"]');
    for (var i = 0; i < exact.length; i++) {
      if (isVisible(exact[i])) return exact[i];
    }
    // Defensive fallbacks (older/newer builds) — keep the old selector+shape search.
    var sels = [
      '[data-testid="tab-list"]', '[role="tablist"]', '[data-testid="nav-rail"]',
      'nav[aria-label="Navigation"]', 'div[aria-label="Navigation"]', '[aria-label="Navigation"]'
    ];
    var picked = null;
    sels.forEach(function (s) {
      if (picked) return;
      var list = document.querySelectorAll(s);
      for (var j = 0; j < list.length; j++) {
        var r = list[j].getBoundingClientRect();
        if (isVisible(list[j]) && r.left < window.innerWidth * 0.25 && r.height > window.innerHeight * 0.4 && r.width < 160) {
          picked = list[j]; return;
        }
      }
    });
    if (picked) return picked;
    var leaf = document.querySelector('[data-navbar-item="true"], [aria-label="Chats"], [aria-label="Status"]');
    if (leaf) {
      var p = leaf.parentElement;
      for (var k = 0; p && k < 7; k++) {
        if (isVisible(p)) { var tabs = p.querySelectorAll('[data-navbar-item="true"]'); if (tabs.length >= 2) return p; }
        p = p.parentElement;
      }
    }
    return null;
  }

  function makeItem() {
    var muted = 'var(--WDS-content-deemphasized,#8696a0)';
    var base = 'var(--WDS-content-default,#e9edef)';
    var hover = 'var(--WDS-surface-emphasized,rgba(134,150,160,.16))';
    var b = document.createElement('button');
    b.id = ID;
    b.type = 'button';
    b.setAttribute('aria-label', 'Settings');
    b.setAttribute('title', 'Settings');
    b.innerHTML = GEAR;
    b.style.cssText =
      'display:flex;align-items:center;justify-content:center;' +
      'width:40px;height:40px;flex:none;border:none;border-radius:12px;cursor:pointer;' +
      'background:transparent;color:' + muted + ';padding:0;' +
      'transition:background .12s ease,color .12s ease;';
    b.addEventListener('mouseenter', function () { b.style.background = hover; b.style.color = base; });
    b.addEventListener('mouseleave', function () { b.style.background = 'transparent'; b.style.color = muted; });
    b.addEventListener('click', function (e) {
      e.preventDefault();
      e.stopPropagation();
      invoke('open_settings_window').catch(function (err) {
        try { console.error('[wfa] open_settings failed:', err && err.message ? err.message : err); } catch (e2) {}
      });
    });
    return b;
  }

  function ensure() {
    var item = document.getElementById(ID);
    // Fast path: gear already attached -> no DOM scan (this is what stopped the freeze).
    if (item && item.isConnected) return true;
    var rail = findRail();
    if (!rail) return false;
    try {
      item = item || makeItem();
      // The item-list wrapper is the rail's first element child; insert the gear as
      // its first child so it sits ABOVE the top nav item (Chats).
      var listBox = rail.children && rail.children.length ? rail.children[0] : rail;
      var ref = listBox.children && listBox.children.length ? listBox.children[0] : null;
      listBox.insertBefore(item, ref);
      return true;
    } catch (err) {
      try { console.error('[wfa] settings rail insert failed', err); } catch (e) {}
      return false;
    }
  }

  // Persistent: re-insert if React wipes the gear; cheap fast path once attached.
  try {
    var mo = new MutationObserver(function () { ensure(); });
    mo.observe(document.documentElement, { childList: true, subtree: true });
  } catch (e) {}

  var tries = 0;
  var timer = setInterval(function () {
    if (ensure()) clearInterval(timer);
    else if (++tries > 80) clearInterval(timer);
  }, 500);

  window.__WF_RAIL_SETTINGS__ = {
    open: function () {
      invoke('open_settings_window').catch(function (err) {
        try { console.error('[wfa] open_settings failed:', err && err.message ? err.message : err); } catch (e2) {}
      });
    }
  };
})();
