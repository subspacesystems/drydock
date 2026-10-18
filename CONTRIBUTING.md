# Contributing to Drydock

Thanks for helping. Drydock is maintained by Subspace Systems: issues and pull requests get
reviewed and updates get released, but response times vary.

## Reporting a bug

Open an issue with:

- what you did, what you expected, and what happened instead;
- the Drydock version or commit, and your operating system;
- a small model file that shows the problem, if one is involved and you can share it.

Security problems are different: see [SECURITY.md](SECURITY.md) and don't open a public issue.

## Contributor License Agreement

Before a pull request can be merged, you need to agree to the
[Contributor License Agreement](CLA.md). A check on your first pull request walks you through it;
you only do it once. The CLA lets Subspace Systems keep offering Drydock under other terms as
well as the GPL. You keep the copyright in your work.

## Clean-room rule

Drydock is written from scratch. Don't contribute code, icons, cursors, art, materials, sample
models or help text taken from another 3D modeler, decompiled from one, or derived from its SDK or
scripting API. Describing how a tool behaves in your own words is fine; copying is not. Third-party
libraries must use one of the licenses `app/deny.toml` allows.

## Code

- The app is a Rust workspace in `app/`. Format with `cargo fmt` (settings in `app/rustfmt.toml`).
- Every new source file starts with these two lines:

  ```rust
  // SPDX-License-Identifier: GPL-3.0-or-later
  // Copyright (C) 2026 Subspace Systems
  ```

- Keep changes focused: one fix or feature per pull request, with a short description of what it
  changes and why.

## Tests

From `app/`, before opening a pull request:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check licenses
```

New behavior comes with tests. A bug fix comes with a test that fails without the fix.

## License

By contributing you agree that your contribution is licensed under the GNU General Public License,
version 3 or later ([LICENSE](LICENSE)), and under the terms of the [CLA](CLA.md).
