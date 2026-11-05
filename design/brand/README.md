# Drydock brand assets

Source material for the Drydock brand update. The full rule book is the
**Subspace Systems design system** (Tyrel has the link); this folder holds
everything the app needs from it. Tracked by the "Apply the Subspace brand to
Drydock" issue.

| Path | What |
|---|---|
| `tokens.json` | Every color (dark + light), type style, spacing and radius token. Color tokens may alias others as `"{name}"`. |
| `icons/*.svg` | 24 icons: 18 tools + 6 materials-panel icons. 20×20 grid, white ink, meant to be tinted at runtime. |
| `icons/generate_icons.py` | The script that draws the icons (Python 3.12+). Edit it and rerun it; don't hand-edit the SVGs. |
| `logo/drydock-app-icon.svg` | App icon tile, 1000×1000 (window icon, `.ico`, Linux hicolor, `assets/drydock-256.png`). |
| `logo/drydock-favicon.svg` | Simplified icon for 16 and 32 px. |
| `logo/drydock-lockup-h-dark.svg` | Horizontal "Subspace Drydock" lockup for dark backgrounds (About box, splash). For the light theme, make `drydock-lockup-h-light.svg` by changing `#EEF7FF` and `#1FD5FF` to `#091126`; the orange stays. |

## Icons

- One file per `Tool` variant (`select`, `line`, `rectangle`, `circle`, `arc`,
  `arc3`, `pushpull`, `offset`, `followme`, `move`, `rotate`, `scale`, `tape`,
  `erase`, `paint`, `orbit`, `pan`, `zoom`) and per panel `Icon` (`list`,
  `tiles`, `new`, `edit`, `purge`, `eyedropper`).
- Drawn at 20×20 with a 1.75 px stroke and mitred joins. Drawing tools mark the
  point they act on with a small **ring node**, the Subspace logo's motif.
- White ink (`#FFFFFF`) so a runtime tint gives the theme's color. Fills at 35%
  opacity are intentional.
- The paint bucket pours to the left, as Tyrel asked.

## Colors in the app

Map `tokens.json` onto egui `Visuals`, one set per theme:

| egui | Token | Dark | Light |
|---|---|---|---|
| `panel_fill` | `surface` | `#0D162D` | `#FFFFFF` |
| `window_fill` | `surface-raised` | `#161F37` | `#FFFFFF` |
| `extreme_bg_color` | `bg-sunken` | `#050A19` | `#E2E8F4` |
| `faint_bg_color` | `bg` | `#091126` | `#EEF7FF` |
| `window_stroke`, `noninteractive.bg_stroke` | `border` | `#263047` | `#CBD1DF` |
| `inactive.bg_stroke` (control outlines) | `border-control` | `#5F697E` | `#848C9F` |
| text (`fg_stroke`) | `ink` | `#EEF7FF` | `#091126` |
| secondary text | `ink-muted` | `#AAB1C1` | `#3D475E` |
| hover / selected fill, `selection.bg_fill` | `accent-soft` | cyan, alpha 41 | `#00739C`, alpha 31 |
| `selection.stroke`, `hyperlink_color`, focus | `accent` | `#1FD5FF` | `#00739C` |
| primary dialog button fill / label | `action` / `on-action` | `#8E83FC` / `#091126` | `#5247C9` / `#FFFFFF` |
| active-tool marker | `product` | `#FF8A3D` | `#A84A14` |
| `warn_fg_color` / `error_fg_color` | `warning` / `danger` | `#FFD24D` / `#FF5C7A` | `#8A6400` / `#B3203F` |

**Viewport**

| Token | Dark | Light |
|---|---|---|
| `viewport-bg` | `#050A19` | `#F4F8FC` |
| `viewport-grid` | cyan, alpha 10% | navy, alpha 8% |
| `edge` | `#EEF7FF` | `#091126` |
| `face-front` | `#3D475E` | `#E2E8F4` |
| `face-back` | `#B04A80` | `#C25A8C` |
| `axis-x` / `axis-y` / `axis-z` | `#FF4F64` / `#33D17A` / `#6FA3FF` | `#D62D45` / `#13994F` / `#2F6FE0` |

All text pairs above meet WCAG 4.5:1 and all marks meet 3:1, in both themes.

## Fonts

- **Roboto** for the whole UI: Regular 400 for text, Medium 500 for button
  labels and panel headers. Roboto replaces the OS font; Tyrel decided this on
  2026-11-05.
- **Roboto Mono** Regular and Medium for numbers the user types or measures:
  the Measurements box, Entity Info values, dimensions. Always show units.
- **Orbitron** Bold only for the splash and the About box headline. Never in
  working UI, never under 19 px, never as the logo.
- Sources: `google/fonts` on GitHub (`ofl/roboto`, `apache/robotomono`,
  `ofl/orbitron`). egui needs static instances; cut them from the variable files
  with fontTools `varLib.instancer`. Commit each license file next to its font.

## Shapes

- Controls: corner radius 4. Menus and windows: 8.
- The Measurements box uses asymmetric corners: 2 on the left, 8 on the right
  (`CornerRadius { nw: 2, sw: 2, ne: 8, se: 8 }`).
- Flat: separate regions with `border`; only menus and popups get a shadow.
