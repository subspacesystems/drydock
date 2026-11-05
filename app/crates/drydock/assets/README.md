# Assets

- `land_110m.bin`: land outlines for the Geolocation globe, from Natural Earth 1:110m land (public domain, naturalearthdata.com). Built by `tools/land_data.py`; format in that script. See `docs/adr/006-map-data.md`.
- `drydock.ico`: the app icon (an isometric cube on a navy tile), our own artwork, embedded in `drydock.exe` by `build.rs`. Drawn by `make-icon.py` (standard library only); sizes 16, 32, 48 and 256.
- `drydock-256.png`: the 256 px frame of the icon as a PNG, also written by `make-icon.py`; the Linux window icon (`window_icon.rs`) and, later, the desktop entry's icons.
