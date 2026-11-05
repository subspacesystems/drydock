# m10-theme-default-dark: Dark when the OS doesn't say

Tyrel chose Dark: when the OS reports neither light nor dark, Drydock starts in Dark.
When the OS says Light or Dark, that still wins.

## Change
- `drydock::look::theme_from(None)` returns `Theme::Dark`. `Some(Dark)` -> Dark and
  `Some(Light)` -> Light stay. Update its doc comment ("else Dark").
- `drydock::look::apply` must make the same default hold in egui: before
  `set_theme(ThemePreference::System)`, set `ctx.options_mut(|o| o.fallback_theme = ...)` to
  the egui theme that corresponds to `theme_from(None)`. egui's own default is already Dark,
  but the app should not depend on that; one source of truth is `theme_from`.
- Nothing else calls `theme_from` today; don't add callers beyond `apply`.

## Tests (already written, do not edit)
- `tests/look.rs::theme_follows_the_os_and_defaults_to_dark` (renamed, now pins `None` -> Dark).
- `tests/look_default_dark.rs`: a fresh `egui::Context` after `look::apply` and one frame with
  no OS theme is in dark visuals (`look::theme_of(&ctx.style().visuals) == Theme::Dark`) and
  its fallback theme is Dark; with the OS saying Light (`RawInput::system_theme`) it is Light.

## Where to look
- `show theme_from`, `show apply` (both in `app/crates/drydock/src/look.rs`)
- `show theme_of` (visuals -> Theme)
- egui `Options::fallback_theme`, `RawInput::system_theme`
