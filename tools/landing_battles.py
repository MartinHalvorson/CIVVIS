#!/usr/bin/env python3
"""Regenerate the battle catalog embedded in beta/landing.html.

The home page (civvis.ai/home) is a static file, and the Tactics quadrants on
it open a battle picker — the historical scenario library — before they hand
over to the simulator. The catalog itself lives in the engine
(`src/historical_scenarios.rs`), and the page cannot ask a WebAssembly module
for it before the module is loaded, so the page carries a copy: one JSON row
per battle inside `<script id="battle-catalog" type="application/json">`.

A copy drifts, so a test pins it: `server::tests::the_home_page_carries_the_
battle_catalog_the_engine_ships` compares the block against
`historical_scenarios::all()` and fails the moment a battle is added, renamed
or re-dated without this file being run. Then run this:

    python3 tools/landing_battles.py                 # builds and asks the engine
    python3 tools/landing_battles.py --rules r.json  # from a saved /rules answer
    python3 tools/landing_battles.py --url http://127.0.0.1:8765

Without `--rules` or `--url` it builds the native binary (`cargo build
--profile ci --bin civvis`), serves a throwaway spectator on a free port, reads
`/rules`, and stops it. The rows are written one battle per line so a diff
reads as the battle that changed, and `<` is escaped so no summary can ever
close the script element early.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import socket
import subprocess
import sys
import time
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
LANDING = ROOT / "beta" / "landing.html"
OPEN = '<script id="battle-catalog" type="application/json">'
CLOSE = "</script>"


def free_port() -> int:
    with socket.socket() as sock:
        sock.bind(("127.0.0.1", 0))
        return sock.getsockname()[1]


def rules_from_engine(no_build: bool = False) -> dict:
    if not no_build:
        subprocess.run(
            ["cargo", "build", "--profile", "ci", "--locked", "--bin", "civvis"],
            cwd=ROOT, check=True,
        )
    port = free_port()
    server = subprocess.Popen(
        [str(ROOT / "target" / "ci" / "civvis"), "play", "--spectate", "--no-open",
         "--port", str(port), "--players", "2", "--map", "battlefield"],
        cwd=ROOT, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        deadline = time.time() + 120
        while True:
            try:
                return rules_from_url(f"http://127.0.0.1:{port}")
            except OSError:
                if time.time() > deadline:
                    raise SystemExit("the engine never answered /rules")
                time.sleep(0.5)
    finally:
        server.terminate()
        server.wait(timeout=10)


def rules_from_url(url: str) -> dict:
    with urllib.request.urlopen(url.rstrip("/") + "/rules", timeout=5) as answer:
        return json.load(answer)


def catalog_block(rows: list[dict]) -> str:
    lines = [json.dumps(row, ensure_ascii=False, separators=(",", ":")) for row in rows]
    body = "[\n" + ",\n".join(lines) + "\n]"
    return body.replace("<", "\\u003c")


def splice(page: str, block: str) -> str:
    start = page.index(OPEN) + len(OPEN)
    end = page.index(CLOSE, start)
    return page[:start] + "\n" + block + "\n" + page[end:]


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--rules", type=pathlib.Path, help="a saved /rules answer")
    parser.add_argument("--url", help="a running CIVVIS server to ask")
    parser.add_argument("--check", action="store_true",
                        help="exit 1 if the page would change, writing nothing")
    parser.add_argument("--no-build", action="store_true",
                        help="skip `cargo build` and run the target/ci/civvis "
                             "binary that is already there (ignored with "
                             "--rules or --url, which never build)")
    args = parser.parse_args(argv)
    if args.rules:
        rules = json.loads(args.rules.read_text(encoding="utf-8"))
    elif args.url:
        rules = rules_from_url(args.url)
    else:
        rules = rules_from_engine(no_build=args.no_build)
    rows = rules["historical_scenarios"]
    if not isinstance(rows, list) or not rows:
        raise SystemExit("/rules carried no historical_scenarios")
    page = LANDING.read_text(encoding="utf-8")
    updated = splice(page, catalog_block(rows))
    if updated == page:
        print(f"beta/landing.html already carries all {len(rows)} battles")
        return 0
    if args.check:
        print("beta/landing.html is behind the engine's battle catalog", file=sys.stderr)
        return 1
    LANDING.write_text(updated, encoding="utf-8")
    print(f"wrote {len(rows)} battles into beta/landing.html")
    return 0


if __name__ == "__main__":
    sys.exit(main())
