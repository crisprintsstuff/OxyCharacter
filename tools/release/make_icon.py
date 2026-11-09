#!/usr/bin/env python3
"""Draw tools/release/oxycharacter.png.

Original mark for the AppImage and the macOS bundle: a dark rounded square,
a blue disc and a simple standing figure. No third-party artwork. The colours
match the editor chrome (near-black panels, accent blue).
"""

import struct
import zlib
from pathlib import Path

W = H = 256
BG = (14, 14, 16)
ACCENT = (20, 114, 226)
FIGURE = (236, 237, 240)


def inside_round_rect(x: float, y: float, radius: float) -> bool:
    inset = 18
    left, top, right, bottom = inset, inset, W - 1 - inset, H - 1 - inset
    if x < left or x > right or y < top or y > bottom:
        return False
    cx = min(max(x, left + radius), right - radius)
    cy = min(max(y, top + radius), bottom - radius)
    return (x - cx) ** 2 + (y - cy) ** 2 <= radius ** 2


def in_circle(x: float, y: float, cx: float, cy: float, r: float) -> bool:
    return (x - cx) ** 2 + (y - cy) ** 2 <= r ** 2


def pixel(x: int, y: int) -> tuple[int, int, int]:
    fx, fy = x + 0.5, y + 0.5
    if not inside_round_rect(fx, fy, 48):
        return BG
    # Disc behind the figure, sitting a little above centre.
    if in_circle(fx, fy, 128, 118, 78):
        colour = ACCENT
    else:
        colour = BG
    # Head.
    if in_circle(fx, fy, 128, 78, 18):
        colour = FIGURE
    # Torso: a tapered column.
    if 112 <= fx <= 144 and 96 <= fy <= 168:
        half = 16 - (fy - 96) * 0.06
        if abs(fx - 128) <= half:
            colour = FIGURE
    # Arms.
    if 78 <= fx <= 178 and 108 <= fy <= 122:
        colour = FIGURE
    # Legs.
    if 108 <= fx <= 122 and 160 <= fy <= 198:
        colour = FIGURE
    if 134 <= fx <= 148 and 160 <= fy <= 198:
        colour = FIGURE
    return colour


def chunk(tag: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def main() -> None:
    rows = bytearray()
    for y in range(H):
        rows.append(0)
        for x in range(W):
            rows.extend(pixel(x, y))
    ihdr = struct.pack(">IIBBBBB", W, H, 8, 2, 0, 0, 0)
    blob = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(rows), 9))
        + chunk(b"IEND", b"")
    )
    out = Path(__file__).with_name("oxycharacter.png")
    out.write_bytes(blob)
    print(f"wrote {out} ({len(blob)} bytes)")


if __name__ == "__main__":
    main()
