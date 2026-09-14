"""Just enough PNG decoding to read back ``CaptureFrame`` output in a test.

The server always writes 8-bit RGB, non-interlaced, so this handles exactly that
and refuses anything else — no image library in the dev dependencies for it.
"""
from __future__ import annotations

import struct
import zlib


def decode_rgb(png: bytes) -> tuple[int, int, bytes]:
    """``(width, height, rgb)`` with ``rgb`` as rows of 3-byte pixels, top first."""
    if png[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    pos, idat, header = 8, bytearray(), None
    while pos < len(png):
        (length,) = struct.unpack(">I", png[pos : pos + 4])
        kind, data = png[pos + 4 : pos + 8], png[pos + 8 : pos + 8 + length]
        pos += 12 + length
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", data)
        elif kind == b"IDAT":
            idat += data
    if header is None:
        raise ValueError("PNG has no IHDR")
    width, height, depth, color, _, _, interlace = header
    if (depth, color, interlace) != (8, 2, 0):
        raise ValueError(f"expected 8-bit RGB non-interlaced, got {header}")

    raw, stride, bpp = zlib.decompress(bytes(idat)), width * 3, 3
    out, prev = bytearray(), bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        kind, row = raw[start], bytearray(raw[start + 1 : start + 1 + stride])
        for i in range(stride):
            a = row[i - bpp] if i >= bpp else 0
            b = prev[i]
            c = prev[i - bpp] if i >= bpp else 0
            if kind == 1:
                row[i] = (row[i] + a) & 0xFF
            elif kind == 2:
                row[i] = (row[i] + b) & 0xFF
            elif kind == 3:
                row[i] = (row[i] + (a + b) // 2) & 0xFF
            elif kind == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pred = a if pa <= pb and pa <= pc else (b if pb <= pc else c)
                row[i] = (row[i] + pred) & 0xFF
        out += row
        prev = row
    return width, height, bytes(out)


def pixel(width: int, rgb: bytes, x: int, y: int) -> tuple[int, int, int]:
    i = (y * width + x) * 3
    return rgb[i], rgb[i + 1], rgb[i + 2]
