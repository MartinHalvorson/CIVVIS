"""Seal all 48 assigned cases after both registered generation jobs finish."""
import argparse
import hashlib
import json
from pathlib import Path


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, required=True)
    args = parser.parse_args()
    exits = json.loads((args.root / "sim-buffered-generation-exits.json").read_text())
    assert [(r["seed"], r["exit_code"]) for r in exits] == [(59002700, 0), (59002701, 0)]
    assert all(r["byte_equal"] for r in exits[0]["old_completed_cases"])
    genome = args.root / "sim-genome.json"
    build = json.loads((args.root / "sim-generate-buffered-build.json").read_text())
    assert sha(genome) == build["genome_sha256"]
    result = {"scope": __doc__, "genome_file": str(genome), "genome_sha256": sha(genome),
              "generation_build": build, "generation_exits": exits, "cases": []}
    for seed in (59002700, 59002701):
        directory = args.root / f"sim-buffered-{seed}"
        generation = json.loads((directory / "result.json").read_text())
        cases = generation["cases"]
        assert [(c["turn"], c["pid"]) for c in cases] == [
            (turn, pid) for turn in (50, 100, 150, 200) for pid in range(6)]
        for case in cases:
            entry = {**case, "tag": f"{seed}-p{case['pid']}"}
            if case["status"] == "board_saved":
                path = directory / case["file"]
                entry.update(status="input_verified", input_file=str(path),
                             input_sha256=sha(path), input_bytes=path.stat().st_size)
            result["cases"].append(entry)
    assert len(result["cases"]) == 48
    with (args.root / "sim-preparation.json").open("x") as output:
        json.dump(result, output, indent=2)
        output.write("\n")
    print("verified inputs", sum(c["status"] == "input_verified" for c in result["cases"]),
          "assigned", len(result["cases"]))


if __name__ == "__main__":
    main()
