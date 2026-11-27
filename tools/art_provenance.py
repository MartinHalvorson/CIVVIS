#!/usr/bin/env python3
"""Keeps art that is not CIVVIS's own out of the repository and off civvis.ai.

    python3 tools/art_provenance.py                  # every image git tracks
    python3 tools/art_provenance.py --bundle DIR     # every image in a publish bundle

The site used to ship sprite sheets cut out of an installed Civilization VI
(the unit glyphs, unit flags, yield signs and the city-wall shields). Those
cutters still exist, so that somebody who owns the game can see its own art on
their own machine, but they now write into `LOCAL_ART`, which git ignores and
which only the native server reads (`LOCAL_CIV6_ART` in src/server.rs). This
module is the fence around that arrangement:

1. Every image, audio, video or font file git tracks is either listed in
   `web/assets/art-manifest.json` with these exact bytes (sha256), or named in
   `NOT_SHIPPED` with the reason it is in the tree at all.
2. No tracked file, and no file in a bundle, carries `LOCAL_MARK`: the PNG
   text chunk every cutter stamps on what it reads out of the install. A cut
   sheet copied into `web/assets` by hand is refused by its marker even before
   its hash is looked up.
3. Every image in a publish bundle is in the manifest by name and hash.
   `beta/publish.sh` runs this on the assembled lane before it converts the
   sheets to WebP, and fails the build on any problem.
4. `LOCAL_ART` is ignored by git, so `git add -A` cannot pick a cut up.

`tools/test_art_provenance.py` runs all four with the other `tools/test_*.py`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import subprocess
import sys
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "web/assets/art-manifest.json"
#: Where the `tools/civ6_*` cutters write. Git ignores it; nothing publishes it.
LOCAL_ART = ROOT / "local/civ6-art"
LOCAL_MARK_KEY = "civvis-provenance"
LOCAL_MARK = "cut from an installed Civilization VI: local only, never commit or publish"

MEDIA = (".png", ".jpg", ".jpeg", ".gif", ".webp", ".avif", ".svg", ".bmp", ".ico",
         ".dds", ".tga", ".blp", ".mp3", ".ogg", ".wav", ".m4a", ".flac", ".mp4",
         ".webm", ".mov", ".ttf", ".otf", ".woff", ".woff2", ".eot")

#: Tracked media that is not shipped art, and why it is in the tree.
NOT_SHIPPED = {
    "tools/fixtures/deal_refuse_live.jpg":
        "a screenshot of a live game: the popup reader's test frame; never published",
    "tools/fixtures/ordinary_map_live.jpg":
        "a screenshot of a live game: the popup reader's control frame; never published",
    "tools/fixtures/game_modes_scaled.png":
        "a screenshot crop of the game's setup screen: the mode-box reader's test frame; never published",
}


def manifest() -> dict:
    return json.loads(MANIFEST.read_text(encoding="utf-8"))["files"]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def carries_local_mark(data: bytes) -> bool:
    return (LOCAL_MARK_KEY.encode() + b"\0" + LOCAL_MARK.encode()) in data


def mark_local(png: bytes) -> bytes:
    """`png` with the local-only marker as a tEXt chunk right after IHDR."""
    if png[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    ihdr_end = 8 + 12 + struct.unpack(">I", png[8:12])[0]
    body = LOCAL_MARK_KEY.encode("latin-1") + b"\0" + LOCAL_MARK.encode("latin-1")
    chunk = (struct.pack(">I", len(body)) + b"tEXt" + body
             + struct.pack(">I", zlib.crc32(b"tEXt" + body) & 0xFFFFFFFF))
    return png[:ihdr_end] + chunk + png[ihdr_end:]


def local_destination(name: str) -> Path:
    """Where a cutter writes `name`, creating the directory."""
    LOCAL_ART.mkdir(parents=True, exist_ok=True)
    return LOCAL_ART / name


def tracked_files() -> list[str]:
    out = subprocess.run(["git", "-C", str(ROOT), "ls-files", "-z"],
                         capture_output=True, check=True).stdout
    return [name for name in out.decode("utf-8").split("\0") if name]


def check_tree() -> list[str]:
    problems = []
    listed = manifest()
    for name in tracked_files():
        path = ROOT / name
        if not path.is_file():
            continue          # deleted in the working tree
        data = path.read_bytes()
        if carries_local_mark(data):
            problems.append(f"{name}: carries the local-only marker of art cut from the game")
        if not name.lower().endswith(MEDIA):
            continue
        if name in NOT_SHIPPED:
            continue
        entry = listed.get(Path(name).name) if name.startswith("web/assets/") else None
        if entry is None:
            problems.append(f"{name}: an image or media file that art-manifest.json does not list "
                            "and NOT_SHIPPED does not explain")
        elif entry["sha256"] != sha256(data):
            problems.append(f"{name}: its bytes are not the ones art-manifest.json lists")
    for name in listed:
        if not (ROOT / "web/assets" / name).is_file():
            problems.append(f"web/assets/{name}: listed in art-manifest.json but missing")
    ignored = subprocess.run(["git", "-C", str(ROOT), "check-ignore", "-q",
                              str(LOCAL_ART / "unit-glyphs.png")])
    if ignored.returncode != 0:
        problems.append(f"{LOCAL_ART.relative_to(ROOT)}/ is not ignored by git")
    return problems


def check_bundle(directory: Path) -> list[str]:
    """Every media file under a publish bundle is listed art with its bytes."""
    problems = []
    listed = manifest()
    files = [path for path in sorted(directory.rglob("*")) if path.is_file()]
    for path in files:
        relative = path.relative_to(directory)
        data = path.read_bytes()
        if carries_local_mark(data):
            problems.append(f"{relative}: art cut from the game, marked local-only")
            continue
        if not path.name.lower().endswith(MEDIA):
            continue
        entry = listed.get(path.name)
        if entry is None:
            problems.append(f"{relative}: not in web/assets/art-manifest.json")
        elif entry["sha256"] != sha256(data):
            problems.append(f"{relative}: not the bytes web/assets/art-manifest.json lists")
    return problems


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--bundle", type=Path, help="a publish bundle to check instead of the tree")
    args = parser.parse_args(argv)
    problems = check_bundle(args.bundle) if args.bundle else check_tree()
    for problem in problems:
        print(f"art provenance: {problem}", file=sys.stderr)
    if problems:
        print("Only CIVVIS's own art, listed in web/assets/art-manifest.json, may be committed "
              "or published (docs/CIV6_ART.md).", file=sys.stderr)
        return 1
    print("art provenance: every image is listed art"
          + (f" in {args.bundle}" if args.bundle else " in the tree"))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
