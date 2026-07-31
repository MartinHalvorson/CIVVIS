"""Write a fresh nonce into every candidate inbound sink, once a second.

The mod's `probeChannels` asks each candidate API what it holds and emits the
answer. That alone proves only that a name resolves. A *channel* is a sink this
process can change from outside such that the mod's next report carries the new
value, so this writes a nonce that changes every second and the reader looks for
that nonce coming back.

⚠ Why the nonce and not a fixed string: a fixed value cannot distinguish "the mod
read what we wrote" from "the mod read a stale copy baked in at install", and this
project has already published one wrong conclusion from exactly that confusion
(`applied = true` on build requests the engine silently discarded).

    python3 tools/civ6_control/probe_channel.py --seconds 240

Then read the answer with:

    python3 tools/civ6_control/probe_channel.py --report <run-dir>
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import civ6_env  # noqa: E402


def options_files() -> list[Path]:
    base = Path(civ6_env.user_dir())
    return [base / "AppOptions.txt", base / "UserOptions.txt"]


def write_option(path: Path, nonce: str) -> str:
    """Put `Decision <nonce>` under a `[Civvis]` section, creating it once.

    The file is the game's, and the game rewrites it on exit and on any options
    change, so the section is re-added rather than assumed to survive.
    """
    if not path.exists():
        return "missing"
    text = path.read_text(errors="replace")
    line = f"Decision {nonce}"
    if "[Civvis]" in text:
        text = re.sub(r"(\[Civvis\]\n)Decision [^\n]*", rf"\g<1>{line}", text)
        if line not in text:  # section present but key absent
            text = text.replace("[Civvis]\n", f"[Civvis]\n{line}\n")
    else:
        text = text.rstrip("\n") + f"\n\n[Civvis]\n{line}\n"
    try:
        path.write_text(text)
    except OSError as exc:
        return f"error:{exc.errno}"
    return "written"


def write_clipboard(nonce: str) -> str:
    try:
        subprocess.run(["pbcopy"], input=nonce.encode(), check=True, timeout=5)
    except (subprocess.SubprocessError, OSError) as exc:
        return f"error:{exc}"
    return "written"


def report(run_dir: Path) -> int:
    """Say, per candidate, whether a nonce we wrote ever came back."""
    events = run_dir / "events.jsonl"
    if not events.exists():
        print(f"no events at {events}")
        return 2
    seen: list[dict] = []
    for raw in events.read_text(errors="replace").splitlines():
        try:
            event = json.loads(raw)
        except ValueError:
            continue
        if event.get("kind") == "channel":
            seen.append(event)
    if not seen:
        print("no `channel` events -- was the mod installed with ProbeChannels?")
        return 2
    print(f"{len(seen)} channel reports, turns {seen[0]['turn']}..{seen[-1]['turn']}")
    print("\n-- what each name resolved to (last report) --")
    last = seen[-1]
    for key in sorted(k for k in last if k.startswith("t_")):
        print(f"  {key[2:]:<20} {last[key]}")
    print("\n-- did any value CHANGE across reports? (a channel must) --")
    fields = sorted(
        k for k in last if not k.startswith("t_") and k not in {"kind", "ctx", "run", "turn"}
    )
    for field in fields:
        values = [str(e.get(field)) for e in seen]
        distinct = sorted(set(values))
        nonces = [v for v in distinct if v.startswith("civvis-nonce-")]
        verdict = "CHANNEL" if nonces else ("varies" if len(distinct) > 1 else "constant")
        shown = ", ".join(distinct[:3]) + (" ..." if len(distinct) > 3 else "")
        print(f"  {field:<12} {verdict:<9} {len(distinct):>3} distinct: {shown}")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--seconds", type=float, default=240.0)
    ap.add_argument("--interval", type=float, default=1.0)
    ap.add_argument("--report", help="run directory to read the verdict from")
    args = ap.parse_args()

    if args.report:
        return report(Path(args.report).expanduser())

    deadline = time.time() + args.seconds
    first = True
    while time.time() < deadline:
        nonce = f"civvis-nonce-{int(time.time())}"
        results = {"clipboard": write_clipboard(nonce)}
        for path in options_files():
            results[path.name] = write_option(path, nonce)
        if first:
            print(json.dumps(results, indent=2, sort_keys=True))
            first = False
        time.sleep(args.interval)
    print("probe writer done")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
