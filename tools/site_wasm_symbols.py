#!/usr/bin/env python3
"""Bind Binaryen's final function map to the exact published engine bytes.

`beta/publish.sh --symbols-out DIR` calls this after its normal O3 pass. The
sidecar stays outside the site; `site_wasm_profile.py` checks it before naming
an indexed V8 profile. Printing the map does not retain names in the module.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def parse_function_map(text: str) -> dict[int, str]:
    result: dict[int, str] = {}
    for line in text.splitlines():
        index, separator, name = line.partition(":")
        if not separator or not index.isascii() or not index.isdecimal() or not name:
            raise ValueError("malformed Binaryen function map line")
        number = int(index)
        if number in result:
            raise ValueError(f"duplicate function index {number}")
        result[number] = name
    if not result:
        raise ValueError("the function map is empty")
    return result


def record(module: Path, function_map: Path, commit: str) -> dict:
    data = module.read_bytes()
    mapping = function_map.read_bytes()
    parse_function_map(mapping.decode("utf-8"))
    manifest = {
        "format": 1,
        "source_commit": commit,
        "wasm_bytes": len(data),
        "wasm_sha256": hashlib.sha256(data).hexdigest(),
        "function_map_sha256": hashlib.sha256(mapping).hexdigest(),
    }
    (function_map.parent / "manifest.json").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    return manifest


def load(directory: Path, module: Path) -> tuple[dict[int, str], dict]:
    manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
    if not isinstance(manifest, dict) or manifest.get("format") != 1:
        raise ValueError("unsupported function map manifest format")
    data = module.read_bytes()
    if (manifest.get("wasm_bytes") != len(data)
            or manifest.get("wasm_sha256") != hashlib.sha256(data).hexdigest()):
        raise ValueError("function map belongs to a different WebAssembly module")
    mapping = (directory / "functions.map").read_bytes()
    if manifest.get("function_map_sha256") != hashlib.sha256(mapping).hexdigest():
        raise ValueError("function map checksum does not match its manifest")
    return parse_function_map(mapping.decode("utf-8")), manifest


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("module", type=Path)
    parser.add_argument("function_map", type=Path)
    parser.add_argument("commit")
    args = parser.parse_args()
    record(args.module, args.function_map, args.commit)


if __name__ == "__main__":
    main()
