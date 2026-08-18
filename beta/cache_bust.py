#!/usr/bin/env python3
"""Give every published build dependency a content-addressed URL.

A lane's HTML is a moving pointer and is always revalidated. Once that fresh
page arrives, these query versions make the browser request a new URL for every
JS, WASM, or atlas whose bytes changed. Unchanged files keep the same URL and
remain safely reusable from cache.

The lane is the whole unit: the viewer at its root and the landing page's
photographs under home/ are versioned by the same pass, so no page in the lane
carries a hand-maintained hash. A reference that already has a `?v=` (an older
pinned revision's hand-written one) is re-derived from the actual bytes.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re


ASSET_REFERENCE = re.compile(r'(["\'])(assets/[^"\'?#]+)(?:\?v=[0-9a-f]+)?\1')


def content_version(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def replace_once(text: str, needle: str, replacement: str, label: str) -> str:
    count = text.count(needle)
    if count != 1:
        raise ValueError(f"expected exactly one unversioned {label}; found {count}")
    return text.replace(needle, replacement, 1)


def version_references(page_path: pathlib.Path, base: pathlib.Path) -> int:
    """Rewrite every assets/ reference in one file to a content-addressed URL.

    `base` is the directory the file's relative references resolve against —
    the file's own directory for a page, the lane root for the viewer script.
    Returns how many references were versioned; a reference to a file that
    does not exist on disk fails the build.
    """
    versioned = 0

    def version_asset(match: re.Match[str]) -> str:
        nonlocal versioned
        quote, relative = match.groups()
        asset_path = base / relative
        if not asset_path.is_file():
            raise FileNotFoundError(f"{page_path.name} references missing {relative}")
        versioned += 1
        return f"{quote}{relative}?v={content_version(asset_path)}{quote}"

    text = page_path.read_text(encoding="utf-8")
    text = ASSET_REFERENCE.sub(version_asset, text)
    page_path.write_text(text, encoding="utf-8")
    return versioned


def version_lane(lane: pathlib.Path) -> None:
    page_path = lane / "index.html"
    shim_path = lane / "shim.js"
    worker_path = lane / "worker.js"
    wasm_path = lane / "civvis.wasm"
    # The viewer's scripts moved out of the page into assets/*.js (#1289, and
    # further carves since). A lane pinned to a revision from before that — the
    # stable lane routinely is — keeps the script inline in index.html and ships
    # none of them, so an empty list is a shape, not an error.
    #
    # ⚠ EVERY SCRIPT, NOT `app.js`. Naming one file meant a later carve's atlas
    # references were never content-versioned, so a stale atlas could be served
    # from cache after the art changed — the exact staleness this module exists
    # to prevent.
    script_paths = sorted((lane / "assets").glob("*.js")) if (lane / "assets").is_dir() else []
    for required in (page_path, shim_path, worker_path, wasm_path):
        if not required.is_file():
            raise FileNotFoundError(f"published lane is missing {required.name}")

    wasm_version = content_version(wasm_path)
    worker_version = content_version(worker_path)

    # The page versions shim.js, while the shim versions its own two build
    # dependencies. Including those dependency hashes in the generated shim
    # also changes the shim's hash whenever either dependency changes.
    shim = shim_path.read_text(encoding="utf-8")
    shim = replace_once(
        shim,
        'const WASM_URL = new URL("civvis.wasm", here).href;',
        f'const WASM_URL = new URL("civvis.wasm?v={wasm_version}", here).href;',
        "WASM URL in shim.js",
    )
    shim = replace_once(
        shim,
        'const WORKER_URL = new URL("worker.js", here).href;',
        f'const WORKER_URL = new URL("worker.js?v={worker_version}", here).href;',
        "worker URL in shim.js",
    )
    shim_path.write_text(shim, encoding="utf-8")
    shim_version = content_version(shim_path)

    page = page_path.read_text(encoding="utf-8")
    page = replace_once(
        page,
        'href="civvis.wasm"',
        f'href="civvis.wasm?v={wasm_version}"',
        "WASM preload in index.html",
    )
    page = replace_once(
        page,
        'src="shim.js"',
        f'src="shim.js?v={shim_version}"',
        "shim script in index.html",
    )
    page_path.write_text(page, encoding="utf-8")

    # The viewer's scripts carry the atlas references now. Version them inside
    # each script first, so a script's own bytes — and therefore the hash the
    # page requests it by — change whenever any atlas it names changes: the same
    # dependency-hash chaining the generated shim uses for the wasm and worker.
    # In a pre-app.js lane the references sit in the page and the page's own
    # pass covers them.
    referenced_assets = 0
    for script_path in script_paths:
        referenced_assets += version_references(script_path, lane)
    referenced_assets += version_references(page_path, lane)
    if referenced_assets == 0:
        raise ValueError("published viewer contains no unversioned atlas references")

    # The other pages of the lane. The landing page's photographs live in
    # home/assets and are referenced relative to the page; the downloads page
    # references no local assets today, and versioning zero of them is a
    # shape, not an error — verify.py asserts the files themselves ship.
    for page_dir in ("home", "download"):
        lane_page = lane / page_dir / "index.html"
        if lane_page.is_file():
            version_references(lane_page, lane / page_dir)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lane", type=pathlib.Path)
    args = parser.parse_args()
    version_lane(args.lane)
    print(f"   content-versioned build files in {args.lane}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
