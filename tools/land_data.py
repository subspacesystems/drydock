#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Subspace Systems

"""Convert Natural Earth's 1:110m land polygons (public domain, naturalearthdata.com)
into the globe's outline asset, app/crates/drydock/assets/land_110m.bin.

Usage: python3 -I tools/land_data.py ne_110m_land.geojson OUT.bin

Format, little-endian: u16 ring count; per ring a u16 point count, then that many
(i16 longitude, i16 latitude) pairs in hundredths of a degree. Rings are not closed
(the first point is not repeated at the end).
"""
import json
import struct
import sys


def rings(geometry):
    if geometry["type"] == "Polygon":
        yield from geometry["coordinates"]
    elif geometry["type"] == "MultiPolygon":
        for polygon in geometry["coordinates"]:
            yield from polygon


def main(src, dst):
    with open(src, encoding="utf-8") as f:
        data = json.load(f)
    out = []
    for feature in data["features"]:
        for ring in rings(feature["geometry"]):
            points = [(round(lon * 100), round(lat * 100)) for lon, lat, *_ in ring]
            if len(points) > 1 and points[0] == points[-1]:
                points.pop()
            deduped = [p for i, p in enumerate(points) if i == 0 or p != points[i - 1]]
            if len(deduped) >= 3:
                out.append(deduped)
    blob = bytearray(struct.pack("<H", len(out)))
    for ring in out:
        blob += struct.pack("<H", len(ring))
        for lon, lat in ring:
            assert -18000 <= lon <= 18000 and -9000 <= lat <= 9000
            blob += struct.pack("<hh", lon, lat)
    with open(dst, "wb") as f:
        f.write(blob)
    print(f"{len(out)} rings, {sum(map(len, out))} points, {len(blob)} bytes")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
