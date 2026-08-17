#!/usr/bin/env python3
"""Generate and verify the repository's current evaluation status.

The evaluator registry is Rust because it is part of the executable contract,
while the ladder is a JSON ledger written by the live bridge.  Historically
their counts were repeated in prose and in tests, so a new arm or a new live
attempt could leave the paper trail behind without making a build fail.

This tool is the single read-only projection of those two authoritative
sources.  ``--write`` refreshes the checked-in JSON manifest and the compact
status page; ``--check`` is the CI gate and fails if either generated artifact
is stale.  It deliberately does not rewrite the append-only ``docs/EVAL.md``:
that file is evidence, while ``docs/EVAL_STATUS.md`` is the current snapshot.
"""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any


REGISTRY_NAMES = (
    "BUILTIN_AIS",
    "EVAL_ONLY_AIS",
    "LIVE_BRIDGE_TREATMENTS",
    "FIRAXIS_ONLY_TREATMENTS",
    "ENGINE_REPAIR_WAR_TREATMENTS",
    "ENGINE_REPAIR_ECONOMY_TREATMENTS",
    "ENGINE_REPAIR_TREATMENTS",
)

ARRAY_RE = re.compile(
    r"pub const (?P<name>[A-Z][A-Z0-9_]*)\s*:\s*\[&str;\s*(?P<count>\d+)\]\s*=\s*\[(?P<body>.*?)\];",
    re.DOTALL,
)
STRING_RE = re.compile(r'"((?:\\.|[^"\\])*)"')


def _strip_comments(text: str) -> str:
    """Remove Rust comments from a registry body before reading literals."""
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.DOTALL)
    return re.sub(r"//[^\n]*", "", text)


def _rust_string(value: str) -> str:
    # The registry strings are ordinary Rust strings.  JSON's decoder handles
    # the same escapes and keeps this parser independent of a Rust toolchain.
    return json.loads('"' + value + '"')


def read_registry(repo: Path) -> dict[str, dict[str, Any]]:
    source = (repo / "src" / "elo.rs").read_text(encoding="utf-8")
    found: dict[str, dict[str, Any]] = {}
    for match in ARRAY_RE.finditer(source):
        name = match.group("name")
        if name not in REGISTRY_NAMES:
            continue
        body = _strip_comments(match.group("body"))
        items = [_rust_string(value) for value in STRING_RE.findall(body)]
        declared = int(match.group("count"))
        if declared != len(items):
            raise ValueError(
                f"{name}: declaration says {declared}, found {len(items)} strings"
            )
        duplicate = next((item for item in items if items.count(item) > 1), None)
        if duplicate is not None:
            raise ValueError(f"{name}: duplicate registry item {duplicate!r}")
        found[name] = {"declared_count": declared, "items": items}
    missing = [name for name in REGISTRY_NAMES if name not in found]
    if missing:
        raise ValueError(f"src/elo.rs is missing registry constants: {', '.join(missing)}")
    return found


def read_ladder(repo: Path) -> dict[str, Any]:
    path = repo / "docs" / "civ6_ladder.json"
    state = json.loads(path.read_text(encoding="utf-8"))
    attempts = state.get("attempts")
    if not isinstance(attempts, list):
        raise ValueError("docs/civ6_ladder.json: attempts must be a list")
    terminal = [attempt for attempt in attempts if attempt.get("victory") is not None]
    configured = [attempt for attempt in attempts if attempt.get("configured")]
    wins = [attempt for attempt in attempts if attempt.get("configured") and attempt.get("won")]
    latest = max((attempt.get("utc", "") for attempt in attempts), default=None)
    return {
        "attempts": len(attempts),
        "configured": len(configured),
        "terminal": len(terminal),
        "wins": len(wins),
        "latest_utc": latest,
    }


def build_manifest(repo: Path) -> dict[str, Any]:
    registry = read_registry(repo)
    live = registry["LIVE_BRIDGE_TREATMENTS"]["items"]
    firaxis = set(registry["FIRAXIS_ONLY_TREATMENTS"]["items"])
    return {
        "schema_version": 1,
        "source": {
            "registry": "src/elo.rs",
            "ladder": "docs/civ6_ladder.json",
        },
        "registry": registry,
        "derived": {
            "eval_only_count": len(registry["EVAL_ONLY_AIS"]["items"]),
            "live_bridge_count": len(live),
            "firaxis_only_count": len(firaxis),
            "native_engine_repair_count": len(
                registry["ENGINE_REPAIR_TREATMENTS"]["items"]
            ),
            "withholdable_live_count": len(
                [item for item in live if item not in firaxis]
            ),
        },
        "ladder": read_ladder(repo),
    }


def render_status(manifest: dict[str, Any]) -> str:
    derived = manifest["derived"]
    ladder = manifest["ladder"]
    registry = manifest["registry"]
    rows = [
        ("Built-in agents", len(registry["BUILTIN_AIS"]["items"])),
        ("Evaluator-only agents", derived["eval_only_count"]),
        ("Live-bridge treatments", derived["live_bridge_count"]),
        ("Firaxis-only treatments", derived["firaxis_only_count"]),
        ("Native engine-repair treatments", derived["native_engine_repair_count"]),
        ("Withholdable live treatments", derived["withholdable_live_count"]),
    ]
    lines = [
        "# Current evaluation status",
        "",
        "<!-- GENERATED FILE: python3 tools/eval_manifest.py --write -->",
        "",
        "This page is generated from `src/elo.rs` and `docs/civ6_ladder.json`.",
        "The append-only experiment evidence remains in `docs/EVAL.md`; this",
        "page is the current inventory and live-bridge snapshot.",
        "",
        "## Registry",
        "",
        "| inventory | count |",
        "|---|---:|",
    ]
    lines.extend(f"| {label} | {count} |" for label, count in rows)
    lines += [
        "",
        "## Live ladder",
        "",
        f"- Attempts recorded: **{ladder['attempts']}**",
        f"- Configured attempts: **{ladder['configured']}**",
        f"- Terminal outcomes: **{ladder['terminal']}**",
        f"- Configured wins: **{ladder['wins']}**",
        f"- Latest ledger entry: **{ladder['latest_utc'] or '—'}**",
        "",
        "Regenerate with `python3 tools/eval_manifest.py --write`; CI runs",
        "`--check` so registry or ledger changes cannot silently leave this",
        "snapshot stale.",
        "",
    ]
    return "\n".join(lines)


def canonical_json(value: Any) -> str:
    return json.dumps(value, indent=2, sort_keys=True) + "\n"


def write_outputs(repo: Path, manifest: dict[str, Any]) -> None:
    (repo / "docs" / "eval_manifest.json").write_text(
        canonical_json(manifest), encoding="utf-8"
    )
    (repo / "docs" / "EVAL_STATUS.md").write_text(
        render_status(manifest), encoding="utf-8"
    )


def check_outputs(repo: Path, manifest: dict[str, Any]) -> int:
    expected_json = canonical_json(manifest)
    expected_status = render_status(manifest)
    actual_json = (repo / "docs" / "eval_manifest.json").read_text(encoding="utf-8") \
        if (repo / "docs" / "eval_manifest.json").exists() else None
    actual_status = (repo / "docs" / "EVAL_STATUS.md").read_text(encoding="utf-8") \
        if (repo / "docs" / "EVAL_STATUS.md").exists() else None
    failures = []
    if actual_json != expected_json:
        failures.append("docs/eval_manifest.json")
    if actual_status != expected_status:
        failures.append("docs/EVAL_STATUS.md")
    if failures:
        print("stale generated evaluation outputs: " + ", ".join(failures), file=sys.stderr)
        print("run: python3 tools/eval_manifest.py --write", file=sys.stderr)
        return 1
    print(
        f"evaluation manifest current: {manifest['derived']['eval_only_count']} evaluator arms, "
        f"{manifest['ladder']['attempts']} ladder attempts"
    )
    return 0


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parent.parent)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--write", action="store_true", help="write generated outputs")
    mode.add_argument("--check", action="store_true", help="fail when outputs are stale")
    args = parser.parse_args(argv)
    try:
        manifest = build_manifest(args.repo)
        if args.write:
            write_outputs(args.repo, manifest)
            print(
                f"wrote docs/eval_manifest.json and docs/EVAL_STATUS.md "
                f"({manifest['derived']['eval_only_count']} evaluator arms)"
            )
            return 0
        return check_outputs(args.repo, manifest)
    except (OSError, ValueError, json.JSONDecodeError) as error:
        print(f"eval manifest: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
