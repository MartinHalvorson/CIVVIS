"""Preserve the native Caravel/Builder holds and preceding exported facts.

This extracts observations; it does not replay a host or infer saved turns.
"""
import argparse
import hashlib
import json
from pathlib import Path


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--events", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    raw = args.events.read_bytes()
    tiles, state, selected, cases = {}, None, {}, []
    for number, line in enumerate(raw.splitlines(keepends=True), 1):
        row = json.loads(line)
        if row.get("kind") == "tiles":
            for tile in row["plots"]:
                tiles[(tile["x"], tile["y"])] = (tile, number, line)
        if row.get("kind") == "state":
            state = (row, number, line)
        if row.get("kind") != "builder_barbarian_capture_hold" or row.get("hostile_type") != "UNIT_CARAVEL":
            continue
        assert state is not None
        x, y = row["sent"]
        tile, tile_line, tile_raw = tiles[(x, y)]
        native, state_line, state_raw = state
        cities = [c for c in native["cities"] if (c["x"], c["y"]) == (x, y)]
        districts = [d for c in native["cities"] for d in c.get("districts", [])
                     if (d.get("x"), d.get("y")) == (x, y)]
        cases.append({"turn": row["turn"], "hold": row, "hold_line": number,
                      "tile": tile, "tile_line": tile_line, "state_line": state_line,
                      "state_turn": native["turn"], "city_centers": cities,
                      "owned_districts_on_destination": districts,
                      "owned_plain_land": tile.get("o") == 0 and tile.get("w") is False
                      and not cities and not districts})
        selected.update({number: line, tile_line: tile_raw, state_line: state_raw})
    assert [(c["turn"], c["hold"]["sent"]) for c in cases] == [
        (96, [27, 31]), (97, [27, 31]), (108, [24, 37])]
    sources = []
    for number, line in sorted(selected.items()):
        name = f"line-{number:06d}.jsonl"
        (args.out / name).write_bytes(line)
        sources.append({"line": number, "file": name, "bytes": len(line), "sha256": sha(line)})
    report = {"run": "civvis-20261102T202907Z", "source": str(args.events),
              "source_bytes": len(raw), "source_sha256": sha(raw),
              "scope": "Native observation slices, not host execution or a production/strength estimate.",
              "cases": cases, "sources": sources}
    (args.out / "observations.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"cases": len(cases), "owned_plain_land": sum(c["owned_plain_land"] for c in cases)}))


if __name__ == "__main__":
    main()
