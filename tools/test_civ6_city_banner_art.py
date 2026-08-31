#!/usr/bin/env python3
"""Offline contract tests for the city-banner wall art and its delivery path."""

from __future__ import annotations

import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import civ6_city_banner_art as art


REPO = Path(__file__).resolve().parent.parent
APP = REPO / "web/assets/app.js"
SERVER = REPO / "src/server.rs"


def png_rgba(path: Path) -> tuple[int, int, bytes]:
    """The simple filter-0 PNGs this extractor writes, decoded without Pillow."""
    data = path.read_bytes()
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise AssertionError(f"{path} is not a PNG")
    width = height = None
    payload = bytearray()
    at = 8
    while at < len(data):
        length = struct.unpack_from(">I", data, at)[0]
        kind = data[at + 4:at + 8]
        body = data[at + 8:at + 8 + length]
        if kind == b"IHDR":
            width, height, depth, color = struct.unpack_from(">IIBB", body)
            if (depth, color) != (8, 6):
                raise AssertionError(f"{path} is not 8-bit RGBA")
        elif kind == b"IDAT":
            payload.extend(body)
        at += 12 + length
    if width is None or height is None:
        raise AssertionError(f"{path} has no PNG header")
    raw = zlib.decompress(payload)
    stride = width * 4
    rows = []
    for row in range(height):
        start = row * (stride + 1)
        if raw[start] != 0:
            raise AssertionError("the extractor must write unfiltered PNG rows")
        rows.append(raw[start + 1:start + 1 + stride])
    return width, height, b"".join(rows)


class CityBannerWallArt(unittest.TestCase):
    def test_committed_sprite_keeps_the_games_authored_dimensions_and_ink(self):
        width, height, pixels = png_rgba(art.OUTPUT)
        self.assertEqual((width, height), art.DIMENSIONS)
        self.assertGreater(sum(alpha > 0 for alpha in pixels[3::4]), 0)

    def test_writer_preserves_a_source_rgba_sprite(self):
        source = (2, 1, bytearray([1, 2, 3, 4, 5, 6, 7, 8]))
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary) / "sprite.png"
            art.write(output, source)
            self.assertEqual(png_rgba(output), (2, 1, bytes(source[2])))

    def test_renderer_loads_the_game_sprite_and_server_serves_it(self):
        app = APP.read_text(encoding="utf-8")
        server = SERVER.read_text(encoding="utf-8")
        self.assertIn('CIV6_CITY_BANNER_WALL_SHIELDS.src = "/assets/civ6-city-banner-shields.png"', app)
        self.assertIn("drawCiv6CityBannerWallShields(cx", app)
        self.assertIn("(c.wall_max || 0) > 0", app)
        self.assertIn('include_bytes!("../web/assets/civ6-city-banner-shields.png")', server)
        self.assertIn('("GET", "/assets/civ6-city-banner-shields.png")', server)


if __name__ == "__main__":
    unittest.main()
