# m10-brand-fonts: Roboto and Roboto Mono for all UI

The app draws every piece of text with Roboto (Regular, Medium) or Roboto Mono
(Regular, Medium), bundled in the binary. It no longer looks for a font on the OS.

## Already on main (skeleton commit)

- The four font files, `LICENSE-RobotoMono`, `assets/fonts/README.md` and `NOTICE` entries are
  committed. Do not touch them.
- `look::ROBOTO`, `ROBOTO_MEDIUM`, `ROBOTO_MONO`, `ROBOTO_MONO_MEDIUM` (the file bytes),
  `look::medium()` (`FontFamily::Name("medium")`) and `look::mono_medium()`
  (`FontFamily::Name("mono-medium")`).
- `look::font_definitions()` takes no argument and is `unimplemented!`; `look::apply` already
  calls it. `font_candidates` and `first_readable_font` are gone; `Os` stays (other code uses it).

## What to build

1. `look::font_definitions()`: `FontDefinitions::empty()` plus four font data entries, keyed
   `"Roboto"`, `"Roboto-Medium"`, `"RobotoMono"`, `"RobotoMono-Medium"`, from the constants.
   Families (exactly these four, one font each, no fallbacks, no egui default fonts):
   - `Proportional` = [`Roboto`]
   - `Monospace` = [`RobotoMono`]
   - `medium()` = [`Roboto-Medium`]
   - `mono_medium()` = [`RobotoMono-Medium`]
2. `look::style(theme)`: set the `family` of `TextStyle::Button` and `TextStyle::Heading` to
   `medium()`. Keep egui's sizes and keep Body, Small and Monospace as they are
   (Proportional / Monospace). Put the mapping in a small helper fn called from `style` with
   one line, because `m10-brand-theme` edits the same function in parallel.
3. Nothing else in the app reads a font from disk. If a build step or other module mentions the
   removed functions, drop it.

Roboto lacks some symbol glyphs egui draws (arrows, check marks). Don't add a fallback font to
cover them; if a screen shows a missing-glyph box, note it in the commit message.

## Tests (already written, `crates/drydock/tests/fonts.rs`)

Families and keys exactly as above; font data equals the constants and each is a TrueType file;
Button and Heading use `medium()` in both themes; text lays out in all four families with
non-zero width; the mono families have equal advance for `iiii` and `WWWW`, the proportional ones
don't; Roboto Medium measures differently from Roboto Regular; `apply` installs the same fonts;
`look.rs` no longer mentions the OS lookup; font files, license and NOTICE entries exist.
Existing tests that loaded `font_definitions(None)` now call `font_definitions()` and must pass.

## Where to look

- `look::font_definitions`, `look::apply`, `look::style`, `look::medium`, `look::mono_medium`,
  `look::ROBOTO_MEDIUM` (`show <name>`).
- `tests/fonts.rs` helper `widths` (lays out text in a bare `egui::Context`).
- `tests/menus_fit.rs` `fonts` helper (how kittest tests install the app's fonts).
