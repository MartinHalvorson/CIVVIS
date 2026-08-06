#!/usr/bin/env python3
from __future__ import annotations

import pathlib
import tempfile
import unittest

from cache_bust import content_version, version_lane


SHIM = """(function () {
  const here = new URL(".", document.currentScript.src);
  const WASM_URL = new URL("civvis.wasm", here).href;
  const WORKER_URL = new URL("worker.js", here).href;
})();
"""

PAGE = """<!doctype html><html><head>
<link rel="preload" href="civvis.wasm" as="fetch" type="application/wasm">
<script src="shim.js"></script>
</head><body><script>
const atlas = "assets/atlas.webp";
const second = 'assets/second.webp';
</script></body></html>
"""


APP_JS_PAGE = """<!doctype html><html><head>
<link rel="preload" href="civvis.wasm" as="fetch" type="application/wasm">
<script src="shim.js"></script>
</head><body><script src="assets/app.js"></script></body></html>
"""

APP_JS = """const atlas = "assets/atlas.webp";
const second = 'assets/second.webp';
"""


class CacheBustTests(unittest.TestCase):
    # The plain lane models a revision from before the viewer's script moved
    # into assets/app.js (#1289) — its atlas references live inline in the
    # page. The stable lane is routinely pinned to such a revision, so both
    # shapes must stay publishable.
    def make_lane(self, root: pathlib.Path) -> pathlib.Path:
        lane = root / "lane"
        (lane / "assets").mkdir(parents=True)
        (lane / "index.html").write_text(PAGE, encoding="utf-8")
        (lane / "shim.js").write_text(SHIM, encoding="utf-8")
        (lane / "worker.js").write_text("worker-v1\n", encoding="utf-8")
        (lane / "civvis.wasm").write_bytes(b"wasm-v1")
        (lane / "assets" / "atlas.webp").write_bytes(b"atlas-v1")
        (lane / "assets" / "second.webp").write_bytes(b"atlas-v2")
        return lane

    def make_app_js_lane(self, root: pathlib.Path) -> pathlib.Path:
        lane = self.make_lane(root)
        (lane / "index.html").write_text(APP_JS_PAGE, encoding="utf-8")
        (lane / "assets" / "app.js").write_text(APP_JS, encoding="utf-8")
        return lane

    def test_versions_the_atlases_inside_the_viewer_script(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lane = self.make_app_js_lane(pathlib.Path(directory))
            atlas_version = content_version(lane / "assets" / "atlas.webp")
            second_version = content_version(lane / "assets" / "second.webp")

            version_lane(lane)

            appjs = (lane / "assets" / "app.js").read_text(encoding="utf-8")
            self.assertIn(f'"assets/atlas.webp?v={atlas_version}"', appjs)
            self.assertIn(f"'assets/second.webp?v={second_version}'", appjs)
            # The page requests the script by the hash of its rewritten bytes,
            # so an atlas change rolls the script's URL too.
            page = (lane / "index.html").read_text(encoding="utf-8")
            rewritten_version = content_version(lane / "assets" / "app.js")
            self.assertIn(f'src="assets/app.js?v={rewritten_version}"', page)

    def test_versions_every_build_dependency_by_its_content(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lane = self.make_lane(pathlib.Path(directory))
            wasm_version = content_version(lane / "civvis.wasm")
            worker_version = content_version(lane / "worker.js")
            atlas_version = content_version(lane / "assets" / "atlas.webp")
            second_version = content_version(lane / "assets" / "second.webp")

            version_lane(lane)

            shim = (lane / "shim.js").read_text(encoding="utf-8")
            self.assertIn(f'civvis.wasm?v={wasm_version}', shim)
            self.assertIn(f'worker.js?v={worker_version}', shim)
            shim_version = content_version(lane / "shim.js")

            page = (lane / "index.html").read_text(encoding="utf-8")
            self.assertIn(f'href="civvis.wasm?v={wasm_version}"', page)
            self.assertIn(f'src="shim.js?v={shim_version}"', page)
            self.assertIn(f'"assets/atlas.webp?v={atlas_version}"', page)
            self.assertIn(f"'assets/second.webp?v={second_version}'", page)

    def test_dependency_change_rolls_the_generated_shim_url(self) -> None:
        with tempfile.TemporaryDirectory() as first, tempfile.TemporaryDirectory() as second:
            first_lane = self.make_lane(pathlib.Path(first))
            second_lane = self.make_lane(pathlib.Path(second))
            (second_lane / "worker.js").write_text("worker-v2\n", encoding="utf-8")

            version_lane(first_lane)
            version_lane(second_lane)

            first_page = (first_lane / "index.html").read_text(encoding="utf-8")
            second_page = (second_lane / "index.html").read_text(encoding="utf-8")
            self.assertNotEqual(first_page, second_page)
            self.assertNotEqual(
                content_version(first_lane / "shim.js"),
                content_version(second_lane / "shim.js"),
            )

    def test_missing_referenced_asset_fails_closed(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            lane = self.make_lane(pathlib.Path(directory))
            (lane / "assets" / "atlas.webp").unlink()
            with self.assertRaisesRegex(FileNotFoundError, "assets/atlas.webp"):
                version_lane(lane)


if __name__ == "__main__":
    unittest.main()
