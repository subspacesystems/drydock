# Assets

- `land_110m.bin`: land outlines for the Geolocation globe, from Natural Earth 1:110m land (public domain, naturalearthdata.com). Built by `tools/land_data.py`; format in that script. See `docs/adr/006-map-data.md`.
- `drydock.ico`: the Windows app icon, our own artwork from `design/brand/logo` (the app icon and favicon SVGs), embedded in `drydock.exe` by `build.rs`. Holds PNG frames at 16, 32, 48 and 256 px.
- `drydock-256.png`: the 256 px app icon as a PNG, the Linux window icon (`window_icon.rs`) and the source of the Linux hicolor icons.
- The icon files are written by the ignored test `write_icon_files` in `tests/app_icon.rs`: `cargo test -p drydock --test app_icon -- --ignored write_icon_files`.
