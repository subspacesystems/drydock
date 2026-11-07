# m10-theme-linux-portal: follow the Linux desktop's light/dark setting

Feedback fix. On Ubuntu (GNOME) Settings > Appearance has "Default" (light) and "Dark";
other apps follow it instantly. Drydock always opens Dark and never changes: winit 0.30 reports
no theme on Linux, so egui uses `fallback_theme`, which `look::apply` sets to `theme_from(None)`
(Dark).

## Decision: how to talk to D-Bus

Not `zbus`: it is not in Cargo.lock and pulls in a large tree (async runtime, many crates),
each needing a license check in `app/deny.toml`. Instead spawn GLib's `gdbus` (present on every
GNOME desktop) and parse its text output with small pure functions. If `gdbus` is missing the
result is the same as "the desktop doesn't say": Dark. No new dependencies.

## Behavior

- Mapping (`portal::theme_for_scheme`): portal `color-scheme` 1 (prefer dark) -> Dark;
  2 (prefer light) -> Light; 0 (no preference, GNOME "Default") -> Light; any other number ->
  Light; `None` (no portal, no session bus, error, timeout, unparseable) -> Dark (Tyrel's choice
  stays for "the OS doesn't say").
- At start (Linux only): `look::apply` asks `portal::read_scheme()` and sets egui's
  `fallback_theme` from `theme_for_scheme`. Windows and macOS are unchanged (winit reports the
  theme and `ThemePreference::System` follows it); `read_scheme` returns `None` there and
  `watch` does nothing, so `look::apply` must not change those platforms' result.
- While running (Linux only): `App::new` calls `portal::watch(ctx)` once after `look::apply`.
  The thread runs `gdbus monitor --session --dest org.freedesktop.portal.Desktop`, reads stdout
  lines, and for each line `parse_signal_line` accepts calls
  `look::set_fallback_theme(&ctx, theme_for_scheme(Some(n)))` (sets
  `ctx.options_mut(|o| o.fallback_theme = ...)` and `ctx.request_repaint()`). A missing gdbus or
  a bus that goes away ends the thread quietly.
- `read_scheme`: run `gdbus call --session --dest org.freedesktop.portal.Desktop --object-path
  /org/freedesktop/portal/desktop --method org.freedesktop.portal.Settings.ReadOne
  org.freedesktop.appearance color-scheme`; on failure the same with method `...Settings.Read`
  (older portals, reply `(<<uint32 N>>,)`). Poll the child with a deadline of about 2 s total
  and kill it on timeout, so a stuck bus never blocks start-up. Do not use a shell.
- Parsers (`parse_read_reply`, `parse_signal_line`) accept the formats in their doc comments and
  return `None` for everything else (see tests); they never panic.

## Everything that reads the theme follows a change

Check each, and fix if it does not switch live:
- `App::ui` already runs `look::apply_viewport_theme(&mut self.style, theme_of(visuals))` every
  frame (app.rs `fn ui`): keep it, the viewport must redraw (`self.dirty`) on a change. (The
  viewport colors later come from the model's background, row `m10-background-model-info`; keep
  it working until then.)
- `paint_viewport`, `paint_overlays` and `icons.rs` (tool icons are tinted by theme) read the
  theme per frame: confirm icon textures do not cache a single theme; if they do, key the cache
  by theme.
- The About box (`about_lockup` test shows the lockup per theme): confirm it re-reads the theme
  each frame.
Tyrel checks the real desktop on Ubuntu at sign-off; Xvfb has no portal, so tests cover the
fallback (Dark), the parsers, and `set_fallback_theme` switching the visuals.

## Tests

`app/crates/drydock/tests/portal.rs` (already written; do not edit). Skeletons are on main
(`src/portal.rs`, `look::set_fallback_theme`): replace the `unimplemented!` bodies, wire
`look::apply` and `App::new`. Existing `look_default_dark.rs` must keep passing.

## Where to look

`show look::apply`, `show look::theme_from`, `show look::theme_of`, `show look::set_fallback_theme`,
`show look::apply_viewport_theme`, `show App::new`, `outline src/icons.rs` (theme use near
`look::palette(look::theme_of`), `tests/look_default_dark.rs` (frame helper), `tests/about_lockup.rs`.
