# WhatsApp Fast — Brand & Logo Kit

The logo for **WhatsApp Fast**: a fast, low-memory **native Windows shell** for the official
WhatsApp Web client.

## The idea

A **speech bubble with a lightning bolt knocked out of it** — _messaging_ + _speed_ in one
form. The bolt is negative space (a real hole in the bubble), so the mark stays a single bold
silhouette that survives from a 16 px favicon up to signage.

- **Category:** messaging / chat — anchored by the green (kept from the previous icon) and the bubble.
- **Distinct from:** WhatsApp's handset-in-bubble, Telegram's paper plane, Signal's plain bubble.
- **One idea:** "the fast way to WhatsApp."

## Files

Production-ready (vector, no live text):

| File | Use |
| --- | --- |
| `whatsapp-fast-symbol.svg` | Primary symbol, brand green, bolt knocked out. |
| `whatsapp-fast-symbol-mono.svg` | One-colour master — recolour to any solid ink. |
| `whatsapp-fast-symbol-small.svg` | Small-size cut (blunter tail, thicker bolt) for ≤24 px. |
| `whatsapp-fast-app-icon.svg` | Full-bleed green tile + white bubble (app/Windows icon). |
| `exports/` | Black / white / brand-mono, square, favicon and app-icon SVG + PNG variants, plus the full web/PWA icon set (`favicon.ico`, `icon-*.png`, `maskable-512.png`, `site.webmanifest`, `head-snippet.html`). |

**Provisional (placeholder type, not production):** `whatsapp-fast-lockup-horizontal.svg`,
`whatsapp-fast-lockup-stacked.svg`, `whatsapp-fast-wordmark.svg`. These use a system sans
(Segoe UI / Arial) as a stand-in. Before release, set the wordmark in a licensed typeface and
**outline it to paths** (no live `<text>` in shipped vectors).

The application icon set in `apps/desktop/src-tauri/icons/` and `apps/desktop/app-icon.png`
were regenerated from `whatsapp-fast-app-icon.svg`.

## Colour

| Role | HEX | RGB | Notes |
| --- | --- | --- | --- |
| Brand green | `#1DAA61` | 29, 170, 97 | Primary. Matches the app's `--wfa-accent`. |
| Deep green | `#0E7A47` | 14, 122, 71 | Optional darker tone for contrast / hover states. |
| Ink | `#0B1319` | 11, 19, 25 | Wordmark and neutral text on light. |
| White | `#FFFFFF` | 255, 255, 255 | The bolt (negative space) and the reversed mark. |

Reproduction: the mark works as **green-on-light**, **white-on-dark** (reversed), or **single
colour** (black / one brand ink). No gradients are required. On dark UI, prefer the reversed
white symbol; on the green tile, the bolt reads through as the tile colour.

## Clear space & minimum sizes

- **Clear space:** keep a margin of at least the width of the **bolt's waist** (roughly 1/6 of the
  bubble's height) on every side. Nothing intrudes on it.
- **Minimum size:** symbol **16 px** (use `whatsapp-fast-symbol-small.svg` at ≤24 px); horizontal
  lockup **≥120 px** wide; stacked lockup **≥96 px** wide.

## Backgrounds

- ✅ Green symbol on white / light grey; white symbol on dark; one-colour on any flat colour that
  keeps contrast.
- ✅ App tile: white bubble on the green tile.
- 🚫 Don't place the green symbol on the brand green (no contrast).
- 🚫 Don't add drop shadows, bevels, outlines or gradients to rescue legibility.

## Misuse

- Don't stretch, rotate, skew or re-colour the mark outside the palette.
- Don't recompose the tile (keep the corner radius and padding).
- Don't swap the bolt for another icon, or add effects.
- Don't use the provisional lockups as-is; outline the wordmark first.

## Trademark note

"WhatsApp" is a trademark of Meta Platforms, Inc. This app is an unofficial, ToS-safe **shell for
the official WhatsApp Web client** and is not affiliated with or endorsed by Meta. If the product
is distributed publicly, confirm the name and its use of the word "WhatsApp" with a trademark
professional, and keep the "unofficial" disclaimer visible in the README/about.
