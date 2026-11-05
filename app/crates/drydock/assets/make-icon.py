#!/usr/bin/env python3
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Subspace Systems
"""Draws assets/drydock.ico: a rounded navy tile with an isometric cube, our own artwork.

Standard library only. Run `python3 assets/make-icon.py` from the crate folder.
"""
import struct
import zlib

SIZES = [16, 32, 48, 256]
SS = 4  # samples per pixel side


def inside_poly(x, y, poly):
    hit = False
    n = len(poly)
    for i in range(n):
        x1, y1 = poly[i]
        x2, y2 = poly[(i + 1) % n]
        if (y1 > y) != (y2 > y) and x < (x2 - x1) * (y - y1) / (y2 - y1) + x1:
            hit = not hit
    return hit


def in_tile(x, y):
    r = 0.18
    cx = min(max(x, r), 1 - r)
    cy = min(max(y, r), 1 - r)
    return (x - cx) ** 2 + (y - cy) ** 2 <= r * r


# Unit-square coordinates: top, left and right faces of a cube.
TOP = [(0.5, 0.16), (0.8, 0.33), (0.5, 0.5), (0.2, 0.33)]
LEFT = [(0.2, 0.33), (0.5, 0.5), (0.5, 0.86), (0.2, 0.69)]
RIGHT = [(0.5, 0.5), (0.8, 0.33), (0.8, 0.69), (0.5, 0.86)]
FACES = [(TOP, (0xF2, 0xF6, 0xFB)), (LEFT, (0x6F, 0xA8, 0xDC)), (RIGHT, (0x3E, 0x7C, 0xB8))]
TILE = (0x14, 0x2B, 0x4A)


def render(size):
    rows = []
    for py in range(size):
        row = bytearray([0])
        for px in range(size):
            acc = [0, 0, 0, 0]
            for sy in range(SS):
                for sx in range(SS):
                    x = (px + (sx + 0.5) / SS) / size
                    y = (py + (sy + 0.5) / SS) / size
                    if not in_tile(x, y):
                        continue
                    color = TILE
                    for poly, c in FACES:
                        if inside_poly(x, y, poly):
                            color = c
                            break
                    acc[0] += color[0]
                    acc[1] += color[1]
                    acc[2] += color[2]
                    acc[3] += 1
            n = SS * SS
            if acc[3] == 0:
                row += bytes([0, 0, 0, 0])
            else:
                row += bytes([acc[0] // acc[3], acc[1] // acc[3], acc[2] // acc[3], 255 * acc[3] // n])
        rows.append(bytes(row))
    return b"".join(rows)


def png(size):
    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(render(size), 9))
        + chunk(b"IEND", b"")
    )


def main():
    images = [png(s) for s in SIZES]
    out = struct.pack("<HHH", 0, 1, len(images))
    offset = 6 + 16 * len(images)
    for s, data in zip(SIZES, images):
        dim = 0 if s >= 256 else s
        out += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    with open("assets/drydock.ico", "wb") as f:
        f.write(out + b"".join(images))
    # The window and desktop icon on Linux: the 256 px frame as a plain PNG.
    with open("assets/drydock-256.png", "wb") as f:
        f.write(images[SIZES.index(256)])


main()
