//! Wayland protocol implementation boilerplate.

pub mod cursor;
pub mod kwin_blur;
pub mod wp_fractional_scaling;
pub mod wp_viewporter;
pub mod xdg_activation;
// Local patch (Drydock, PATCHES.md): xdg-dialog-v1 for modal dialogs.
pub mod xdg_dialog;
