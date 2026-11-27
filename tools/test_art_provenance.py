#!/usr/bin/env python3
"""Nothing cut from Civilization VI is tracked, and nothing reaches a published build.

`tools/art_provenance.py` is the check; this runs it on the tree, proves it
refuses each way a cut sheet could get in, and pins the places that call it:
`beta/publish.sh` before its WebP pass, the cutters' output directory, and the
native server's local-only override.
"""

from __future__ import annotations

import hashlib
import json
import re
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import art_provenance  # noqa: E402
import own_art  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "web/assets"
CUTTERS = ["civ6_unit_glyphs.py", "civ6_unit_flag_plates.py", "civ6_yield_icons.py",
           "civ6_city_banner_art.py"]


def a_png(width: int = 2, height: int = 2) -> bytes:
    image = own_art.Image(width, height)
    image.over(0, 0, (1.0, 1.0, 1.0), 1.0)
    return image.png()


class TheTree(unittest.TestCase):
    def test_every_tracked_image_is_listed_art_or_an_explained_fixture(self):
        self.assertEqual(art_provenance.check_tree(), [])

    def test_the_sheets_cut_from_the_game_are_gone(self):
        for retired in ("civ6-unit-flags.png", "civ6-unit-flags.json", "civ6-unit-flag-plates.png",
                        "civ6-yield-icons.png", "civ6-city-banner-shields.png",
                        "feature-atlas.png", "environment-feature-atlas.png"):
            self.assertFalse((ASSETS / retired).exists(), retired)

    def test_every_shipped_sheet_is_listed_with_its_bytes(self):
        listed = art_provenance.manifest()
        shipped = sorted(p.name for p in ASSETS.iterdir() if p.name.lower().endswith(art_provenance.MEDIA))
        self.assertEqual(sorted(listed), shipped)
        for name, entry in listed.items():
            data = (ASSETS / name).read_bytes()
            self.assertEqual(entry["sha256"], hashlib.sha256(data).hexdigest(), name)
            self.assertFalse(art_provenance.carries_local_mark(data), name)

    def test_the_drawn_sheets_say_how_they_were_made(self):
        # Every sheet own_art.py writes carries its own provenance text, and the
        # glyph index credits an author and licence for each cell.
        for name in ("unit-glyphs.png", "unit-plates.png", "yield-icons.png", "city-wall-mark.png"):
            self.assertIn(b"civvis-provenance\0drawn by tools/own_art.py", (ASSETS / name).read_bytes(), name)
        index = json.loads((ASSETS / "unit-glyphs.json").read_text(encoding="utf-8"))
        credits = (ASSETS / "ART_CREDITS.md").read_text(encoding="utf-8")
        for row in index["units"]:
            self.assertEqual(row["licence"], "CC BY 3.0", row["type"])
            self.assertIn(row["author"], own_art.AUTHORS.values(), row["type"])
            self.assertIn(row["url"], credits, row["type"])

    def test_every_unit_has_an_icon_whose_paths_are_kept(self):
        self.assertEqual(sorted(own_art.UNIT_ICONS), own_art.roster())
        kept = json.loads(own_art.ICON_DATA.read_text(encoding="utf-8"))
        self.assertEqual(kept["commit"], own_art.ICONS_COMMIT)
        used = set(own_art.UNIT_ICONS.values()) | {name for *_, name in own_art.YIELDS}
        self.assertEqual(sorted(kept["icons"]), sorted(used))
        for name, icon in kept["icons"].items():
            self.assertEqual(icon["licence"], "CC BY 3.0", name)
            self.assertTrue(icon["paths"], name)

    def test_the_local_directory_is_ignored(self):
        self.assertEqual(art_provenance.LOCAL_ART, ROOT / "local/civ6-art")
        self.assertIn("\n/local/\n", (ROOT / ".gitignore").read_text(encoding="utf-8"))


class TheCheckRefuses(unittest.TestCase):
    """Each way a cut could reach the site, built in a scratch bundle."""

    def bundle(self) -> Path:
        directory = Path(tempfile.mkdtemp())
        self.addCleanup(shutil.rmtree, directory)
        shutil.copytree(ASSETS, directory / "assets")
        return directory

    def test_our_own_assets_pass(self):
        self.assertEqual(art_provenance.check_bundle(self.bundle()), [])

    def test_a_marked_cut_under_any_name(self):
        bundle = self.bundle()
        (bundle / "assets/unit-glyphs.png").write_bytes(art_provenance.mark_local(a_png()))
        problems = art_provenance.check_bundle(bundle)
        self.assertTrue(any("cut from the game" in problem for problem in problems), problems)

    def test_an_unlisted_image(self):
        bundle = self.bundle()
        (bundle / "assets/civ6-unit-flags.png").write_bytes(a_png())
        problems = art_provenance.check_bundle(bundle)
        self.assertTrue(any("not in web/assets/art-manifest.json" in p for p in problems), problems)

    def test_a_listed_name_with_other_bytes(self):
        bundle = self.bundle()
        (bundle / "assets/yield-icons.png").write_bytes(a_png())
        problems = art_provenance.check_bundle(bundle)
        self.assertTrue(any("not the bytes" in p for p in problems), problems)

    def test_an_image_anywhere_in_the_bundle(self):
        bundle = self.bundle()
        (bundle / "home/assets").mkdir(parents=True)
        (bundle / "home/assets/portrait.jpg").write_bytes(b"\xff\xd8\xff")
        self.assertTrue(art_provenance.check_bundle(bundle))

    def test_the_marker_survives_the_round_trip_the_cutters_use(self):
        marked = art_provenance.mark_local(a_png())
        self.assertTrue(art_provenance.carries_local_mark(marked))
        self.assertEqual(marked[:8], b"\x89PNG\r\n\x1a\n")
        # Still a valid PNG to the decoder the cutters' own checks use.
        import civ6_unit_glyphs
        self.assertEqual(civ6_unit_glyphs.read_png(marked)[:2], (2, 2))


class TheCallers(unittest.TestCase):
    def test_publish_checks_the_bundle_before_the_webp_pass(self):
        publish = (ROOT / "beta/publish.sh").read_text(encoding="utf-8")
        call = publish.index('python3 "$source_tree/tools/art_provenance.py" --bundle "$out"')
        self.assertLess(call, publish.index("LOSSY_QUALITY = {"), "the check runs before WebP drops the marker")
        self.assertLess(call, publish.index('python3 - "$source_tree/web/index.html" "$out/index.html"'))
        self.assertGreater(call, publish.index('cp -R "$source_tree/web/assets" "$out/assets"'))

    def test_every_cutter_writes_marked_art_into_the_local_directory(self):
        for name in CUTTERS:
            source = (ROOT / "tools" / name).read_text(encoding="utf-8")
            self.assertIn("art_provenance", source, name)
            self.assertNotRegex(source, r'(?:ROOT / |")web/assets/civ6-', name)
        writers = [(ROOT / "tools" / name).read_text(encoding="utf-8")
                   for name in ("civ6_unit_flag_plates.py", "civ6_yield_icons.py")]
        for source in writers:
            self.assertIn("png = art_provenance.mark_local(png)", source)

    def test_the_native_server_alone_reads_the_local_cut(self):
        server = (ROOT / "src/server.rs").read_text(encoding="utf-8")
        self.assertIn('const LOCAL_CIV6_ART: &str = "local/civ6-art";', server)
        self.assertEqual(len(re.findall(r"map_sheet\(\"[a-z-]+\.png\", EMBEDDED_[A-Z_]+\)", server)), 4)
        wasm = (ROOT / "src/wasm.rs").read_text(encoding="utf-8")
        self.assertNotIn("local/civ6-art", wasm)
        shim = (ROOT / "beta/shim.js").read_text(encoding="utf-8")
        self.assertNotIn("local/civ6-art", shim)


if __name__ == "__main__":
    unittest.main()
