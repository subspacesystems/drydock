# ADR-006: Bundled map data for the Geolocation globe

Status: Accepted, 2026-10-25

## Context
Tyrel asked for a simple Earth in the Geolocation window that can be turned and clicked to pick a place. That needs coastlines. The license list in `CLAUDE.md` (`app/deny.toml`) covers code dependencies; this is data compiled into the binary.

## Decision
Bundle Natural Earth's 1:110m land polygons (naturalearthdata.com, `nvkelso/natural-earth-vector`, `geojson/ne_110m_land.geojson`). Natural Earth states that all its data is in the public domain, usable for any purpose including commercial, no permission or attribution needed. `tools/land_data.py` converts the GeoJSON to `app/crates/modeler/assets/land_110m.bin` (128 rings, 5001 points, 20 KB, hundredths of a degree); the app embeds it with `include_bytes!`. No new crate.

Nothing from another modeling product is involved; the globe's look (outlines, 30 degree grid, marker) is our own.

## Consequences
- Coarse outlines only (good to a few km at this scale); enough to find a place, then type exact numbers.
- Satellite imagery or detailed maps would need a tile service with its own terms and per-use fees; out of scope until Tyrel decides.
- Credit Natural Earth in the About box when we add third-party notices (courtesy, not required).
