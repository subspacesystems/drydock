# ADR-009: `unsafe` for the Windows update transport

Status: Accepted, 2026-11-15

## Context
`unsafe_code = "forbid"` holds workspace-wide (ADR-001; exceptions need an ADR). Windows auto-update (M13) needs an HTTPS client that uses the Windows certificate store and the system proxy, because dev releases sit on a host with a private CA that the desktop trusts, and because a Rust TLS stack would pull `webpki-roots` (CDLA-Permissive-2.0, outside the allowed licenses). WinHTTP does both. Its API is raw handles and buffers, so every call through the `windows` crate is `unsafe`.

## Decision
- Only `drydock-update/src/winhttp.rs` (compiled on Windows only) may use `unsafe`. The crate sets `unsafe_code = "deny"` in its own `[lints.rust]` (a crate cannot override an inherited `forbid`), repeats the workspace clippy lints, and the one module carries `#![allow(unsafe_code)]`.
- The unsafe code is limited to WinHTTP calls and the handle wrapper that closes handles on drop. Everything that decides something (URL scheme check, header formatting, size limit, error mapping) is safe code in `net.rs` with tests on every platform.
- New dependency: `windows` 0.62 (MIT OR Apache-2.0), already in `Cargo.lock`; features `Win32_Foundation` and `Win32_Networking_WinHttp`, `cfg(windows)` only.

## Consequences
- Linux builds are unchanged; the other crates keep `forbid`.
- The transport is verified by the `winclippy` gate (compile and lint), the sign-off under Wine and a run on Windows.
