#!/usr/bin/env python3
"""Give every published build dependency a content-addressed URL.

The lane's HTML is a moving pointer and is always revalidated. Once that fresh
page arrives, these query versions make the browser request a new URL for every
JS, WASM, or atlas whose bytes changed. Unchanged files keep the same URL and
remain safely reusable from cache.
"""

from __future__ import annotations

import argparse
import hashlib
import pathlib
import re


ASSET_REFERENCE = re.compile(r'(["\'])(assets/[^"\'?#]+)\1')


def content_version(path: pathlib.Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def replace_once(text: str, needle: str, replacement: str, label: str) -> str:
    count = text.count(needle)
    if count != 1:
        raise ValueError(f"expected exactly one unversioned {label}; found {count}")
    return text.replace(needle, replacement, 1)


def version_lane(lane: pathlib.Path) -> None:
    page_path = lane / "index.html"
    shim_path = lane / "shim.js"
    worker_path = lane / "worker.js"
    wasm_path = lane / "civvis.wasm"
    app_js_path = lane / "assets" / "app.js"
    for required in (page_path, shim_path, worker_path, wasm_path, app_js_path):
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

    referenced_assets = 0

    def version_asset(match: re.Match[str]) -> str:
        nonlocal referenced_assets
        quote, relative = match.groups()
        asset_path = lane / relative
        if not asset_path.is_file():
            raise FileNotFoundError(f"published page references missing {relative}")
        referenced_assets += 1
        return f"{quote}{relative}?v={content_version(asset_path)}{quote}"

    # The viewer's script (`assets/app.js`) carries the atlas references now.
    # Version them inside the script first and write it back, so the script's
    # own bytes — and therefore the hash the page requests it by — change
    # whenever any atlas it names changes: the same dependency-hash chaining
    # the generated shim uses for the wasm and worker.
    appjs = app_js_path.read_text(encoding="utf-8")
    appjs = ASSET_REFERENCE.sub(version_asset, appjs)
    app_js_path.write_text(appjs, encoding="utf-8")

    page = ASSET_REFERENCE.sub(version_asset, page)
    if referenced_assets == 0:
        raise ValueError("published page contains no unversioned atlas references")
    page_path.write_text(page, encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("lane", type=pathlib.Path)
    args = parser.parse_args()
    version_lane(args.lane)
    print(f"   content-versioned build files in {args.lane}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
