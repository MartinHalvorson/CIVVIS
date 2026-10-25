"""Retain the next frozen native attempt's live process context, without edits."""
from pathlib import Path
import argparse
import datetime
import hashlib
import json
import re
import shlex
import subprocess
import time


def utc():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def stems(root):
    pattern = re.compile(r"^(civvis-\d{8}T\d{6}Z)(?:-cont\d+)?$")
    return {match[1] for p in root.iterdir() if p.is_dir() and (match := pattern.fullmatch(p.name))}


def write(path, data):
    with path.open("x") as stream:
        json.dump(data, stream, indent=2)
        stream.write("\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=False)
    snapshots = Path.home() / "civvis-civ6-runs/decider-snapshots"
    baseline = stems(snapshots)
    write(args.out / "registration.json", {"registered_at_utc": utc(), "baseline_stems": sorted(baseline),
          "selection": "first new frozen attempt stem, irrespective of gameplay or outcome",
          "observer_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest()})
    print("registered next frozen attempt context", flush=True)
    selected, deadline = None, time.monotonic() + 14400
    while time.monotonic() < deadline:
        new = sorted(stems(snapshots) - baseline)
        if selected is None and new:
            selected = new[0]
            write(args.out / "selection.json", {"selected_at_utc": utc(), "case": selected,
                  "all_new_stems_at_selection": new})
            deadline = time.monotonic() + 1200
            print("fixed case", selected, flush=True)
        if selected:
            binary = snapshots / selected / "civvis_orders"
            matches = []
            for line in subprocess.check_output(["ps", "-axo", "pid=,ppid=,command="], text=True).splitlines():
                parts = line.strip().split(None, 2)
                if len(parts) != 3:
                    continue
                try:
                    words = shlex.split(parts[2])
                except ValueError:
                    continue
                if words and words[0] == str(binary):
                    try:
                        raw_cwd = subprocess.check_output(["lsof", "-p", parts[0], "-a", "-d", "cwd", "-Fn"], text=True)
                    except subprocess.CalledProcessError:
                        continue
                    cwd = [row[1:] for row in raw_cwd.splitlines() if row.startswith("n")]
                    if len(cwd) == 1:
                        matches.append({"pid": int(parts[0]), "parent": int(parts[1]), "argv": words,
                                        "raw_command": parts[2], "cwd": cwd[0], "cwd_readback": raw_cwd})
            if len(matches) == 1:
                provenance = json.loads(binary.with_suffix(".provenance.json").read_text())
                digest = hashlib.sha256(binary.read_bytes()).hexdigest()
                if digest != provenance["sha256"]:
                    raise ValueError("native binary differs from its frozen provenance")
                write(args.out / "actual-native-context.json", {"recorded_at_utc": utc(), "case": selected,
                      "binary_sha256": digest, "provenance": provenance, "processes": matches})
                print("actual context retained", selected, matches[0]["pid"], flush=True)
                return 0
            if any(tag > selected for tag in new):
                write(args.out / "coverage-failure.json", {"recorded_at_utc": utc(), "case": selected,
                      "error": "a later attempt exists but no unique live context was retained for the fixed case"})
                return 2
        time.sleep(1)
    write(args.out / "coverage-failure.json", {"recorded_at_utc": utc(), "case": selected,
          "error": "no unique live context within the preregistered collection window"})
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
