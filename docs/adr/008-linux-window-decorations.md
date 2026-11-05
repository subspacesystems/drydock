# ADR-008: Linux window decorations (sctk-adwaita)

Status: Accepted, 2026-11-05

## Context
On Ubuntu (Wayland, GNOME) the Drydock window showed a bar drawn by the toolkit that did not match other windows and had no title text. GNOME's compositor does not offer server-side decorations, so winit draws its own client-side frame. eframe 0.36 builds winit 0.30 with default features off, which leaves out its `wayland-csd-adwaita` feature; without it winit uses a bare fallback frame with no text.

## Decision
`app/crates/drydock/Cargo.toml` turns on `wayland-csd-adwaita` for winit on Linux. Cargo unifies features, so egui-winit's winit (the one 0.30.x in `Cargo.lock`) gets it. The frame is drawn by `sctk-adwaita` in the GNOME look, with the window title. New crates and their licenses, all inside the allow list of `app/deny.toml`:

| Crate | License |
| --- | --- |
| sctk-adwaita | MIT |
| tiny-skia, tiny-skia-path | BSD-3-Clause |
| arrayref | BSD-2-Clause |
| strict-num | MIT |

`sctk-adwaita` 0.10.1 is patched locally (`app/vendor/sctk-adwaita`, `[patch.crates-io]` in `app/.cargo/config.toml`): winit's feature still asks for its `ab_glyph` title renderer, which pulled `ttf-parser`, flagged unmaintained (RUSTSEC-2026-0192). The local copy draws the title with `skrifa`, already in the lock, so `ab_glyph`, `owned_ttf_parser` and `ttf-parser` are gone. It also leaves out upstream's embedded Cantarell font (OFL-1.1, outside the allow list; kept OFL fonts out of the app): the title uses the system font that `fc-match` finds, and without one the bar shows no text. Its unit tests use our Roboto, in test builds only.

The window also gets an app id (`APP_ID` in `window_icon.rs`, `com.subspacesystems.Drydock`, the name of the desktop entry) and our own icon (`window_icon.rs`), so the dock shows Drydock and not a default. eframe's `persistence` feature is off, so the app id keys no stored state.

## Consequences
- Linux builds gain a few small pure-Rust crates; other systems are unchanged.
- The local patch must be refreshed by hand when winit moves to a newer `sctk-adwaita`.
- The Windows and macOS builds do not take the dependency.
- If winit moves to another minor version, the dependency line must follow it (a test checks that the lock's winit lists sctk-adwaita).
