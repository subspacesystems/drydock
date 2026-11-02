# m8-rename-config: settings folder becomes Drydock

The product is Drydock. Its settings folder and the variable that overrides it take the new name. The old folder and variable are ignored: no copy, no move, no fallback. Settings start fresh.

## Change

`config_dir` in the app crate's `src/recent.rs` reads, in this order:

1. `DRYDOCK_CONFIG_DIR`, used as given. `MODELER_CONFIG_DIR` is no longer read.
2. On Windows only, `APPDATA` joined with `Drydock` (capital D).
3. An absolute `XDG_CONFIG_HOME` joined with `drydock`.
4. `HOME` joined with `.config` and `drydock`.

An empty value counts as unset for every variable, and a relative `XDG_CONFIG_HOME` is skipped, as today. With none of them set the result is `None`.

Nothing reads, moves or deletes the old `modeler` / `Modeler` folder. Don't add migration code.

`recent_file`, `settings_file` (print setup), `toolbars_file` and `panels_file` already build on `config_dir`; they need no change of their own. Update the doc comment of `config_dir` and any other comment or doc text that names the old variable or folder (grep the repo for `MODELER_CONFIG_DIR`).

## Tests

The tests are already changed in the spec commit; don't edit them:

- `tests/recent.rs`: the `config_dir_*` and `recent_file_*` tests use `DRYDOCK_CONFIG_DIR` and the `drydock` / `Drydock` folder names; `the_old_override_is_not_read` checks that `MODELER_CONFIG_DIR` alone is ignored and the path falls through to `HOME` (or `None` without it).
- `tests/print_setup.rs`, `tests/toolbar_layout.rs`, `tests/dock_layout.rs`: their `var` closures set `DRYDOCK_CONFIG_DIR`.

The crate rename (`m8-rename-crates`) has merged first, so the crate and path names on main may differ from those written here (for example `use modeler::recent` may read `use drydock::recent` in the merged tests); keep what main has.

## Where to look

- `show config_dir`, `show recent_file` (`src/recent.rs`)
- `show settings_file` (print setup), `show toolbars_file`, `show panels_file`: callers, to confirm nothing else names the folder
- `grep -rn MODELER_CONFIG_DIR app docs` for stray mentions
- tests: `env` helper in `tests/recent.rs`
