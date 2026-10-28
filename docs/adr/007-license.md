# ADR-007: License

Status: Accepted, 2026-10-28

## Context
Drydock is free software from Subspace Systems, which holds the copyright in all of it. Contributions from outside the company come under a contributor license agreement.

## Decision
- **App license:** GPL-3.0-or-later, full unmodified text in `LICENSE`.
- **SPDX:** `GPL-3.0-or-later` in `app/Cargo.toml` (shown in Help > About).
- **Source headers:** every `.rs` and `.wgsl` file under `app/` starts with `// SPDX-License-Identifier: GPL-3.0-or-later`. New files get the same line.
- **Dependencies:** the allow list (MIT, Apache-2.0, BSD-2/3, Zlib, ISC, Unicode-3.0, plus BSL-1.0 for two Windows clipboard crates) is all GPLv3-compatible. `app/deny.toml` rejects anything not listed, which covers GPL-2.0-only and other incompatible licenses (checked: a crate set to `GPL-2.0-only` fails the gate). Our own crates have per-crate exceptions for the app's license, so the GPL isn't on the general allow list.
- **Contributions:** outside contributors agree to the contributor license agreement in `CLA.md` (`CONTRIBUTING.md`).
- **Clean-room rules:** unchanged. Copying outside code is banned, GPL or not; the snippet scan rejects copyleft matches.

## Consequences
- Anyone who distributes the app or a modified version must ship its source under GPLv3.
- The Drydock name and logo aren't covered by the license (`TRADEMARKS.md`).
