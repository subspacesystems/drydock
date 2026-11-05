# ADR-008: Linux window decorations (sctk-adwaita)

Status: Accepted, 2026-11-05

## Context
On Ubuntu (Wayland, GNOME) the Drydock window showed a bar drawn by the toolkit that did not match other windows and had no title text. GNOME's compositor does not offer server-side decorations, so winit draws its own client-side frame. eframe 0.36 builds winit 0.30 with default features off, which leaves out its `wayland-csd-adwaita` feature; without it winit uses a bare fallback frame with no text.

## Decision
`app/crates/drydock/Cargo.toml` turns on `wayland-csd-adwaita` for winit on Linux. Cargo unifies features, so egui-winit's winit (the one 0.30.x in `Cargo.lock`) gets it. The frame is drawn by `sctk-adwaita` in the GNOME look, with the window title. New crates and their licenses, all inside the allow list of `app/deny.toml`:

| Crate | License |
| --- | --- |
| sctk-adwaita | MIT |
| ab_glyph, ab_glyph_rasterizer, owned_ttf_parser | Apache-2.0 |
| ttf-parser | MIT OR Apache-2.0 |
| tiny-skia, tiny-skia-path | BSD-3-Clause |
| arrayref | BSD-2-Clause |
| strict-num | MIT |

The window also gets an app id (`APP_ID` in `window_icon.rs`, `com.subspacesystems.Drydock`, the name of the desktop entry) and our own icon (`window_icon.rs`), so the dock shows Drydock and not a default. eframe's `persistence` feature is off, so the app id keys no stored state.

## Consequences
- Linux builds gain about nine small pure-Rust crates; other systems are unchanged.
- The Windows and macOS builds do not take the dependency.
- If winit moves to another minor version, the dependency line must follow it (a test checks that the lock's winit lists sctk-adwaita).
