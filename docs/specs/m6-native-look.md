# m6-native-look: a desktop look

Tyrel: the interface feels "a bit abnormal" next to Windows and GTK apps. The toolkit
(egui) draws its own widgets and has no native-control mode, so this task is a style pass
toward a desktop look. The module `app/crates/modeler/src/look.rs` has the types and
`unimplemented!()` stubs; tests are in `app/crates/modeler/tests/look.rs`.

## What to build

- `theme_from(system)`: Dark if the OS says dark, otherwise Light (also when it says nothing).
- `style(theme)`: start from egui's light or dark visuals, then:
  - every widget state (and menus) has corner radius at most 3 points (flat, nearly square;
    2 is fine);
  - the hovered fill differs from the resting fill (both `weak_bg_fill` and `bg_fill`);
  - item spacing 4 to 8 points vertically, row height (`interact_size.y`) 20 to 28 points,
    button padding at least 6 points horizontally;
  - menus get tight margins so a hovered menu row highlights across the full menu width
    (check in the real window with `harness/bin/gui-session`; no test).
- `font_candidates(os)`: the OS's UI font files in order of preference, absolute paths to
  `.ttf`/`.otf`/`.ttc`. Windows starts with Segoe UI (`C:\Windows\Fonts\segoeui.ttf`). Mac
  and Linux lists are yours to pick from well-known system locations (Noto Sans, Cantarell,
  Ubuntu, DejaVu on Linux). Fonts are read at run time and never copied into the repo.
- `first_readable_font(paths)`: bytes of the first path that exists and begins with a font
  signature (TrueType `00 01 00 00`, `OTTO`, `true`, `ttcf`); otherwise `None`.
- `font_definitions(system)`: Roboto is always present. With system bytes, a font named
  `System` is first in the Proportional family and Roboto follows as fallback (it covers
  glyphs the system font lacks). Monospace stays Roboto. No egui default fonts.
- `apply(ctx)`: loads the system font, sets the fonts, sets the light and dark styles
  (`set_style_of`) and follows the OS theme (`ThemePreference::System`).

## Wiring

In `app.rs`, `App::new` calls `look::apply(&cc.egui_ctx)` instead of `install_fonts`; delete
`install_fonts` and the `ROBOTO` constant from `app.rs` (the tests that define their own
`install_fonts` are unaffected). Remove the `#[allow(dead_code)]` on `ROBOTO` in `look.rs`.
Anything in the app that hard-codes dark-only colours for panels or toolbars should use the
theme's visuals so light mode is legible (icons already use the theme colour). The 3D
viewport background is not part of this task.

## Out of scope

Replacing egui, GTK/Qt, custom title bars, high-contrast mode.
