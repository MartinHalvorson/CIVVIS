#!/usr/bin/env python3
"""Profile one qualified production browser game on an isolated CI runner.

The normal full-game driver checks the roster and rules. This wrapper binds
the qualification run, module, symbols, complete save and frame hashes to the
CPU samples. Sampling includes process/module startup; instrumented timing
is diagnostic evidence, never a paired speed result. Unit tests start no game.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import re
import shutil
import subprocess
from pathlib import Path

import site_wasm_pair as pair
import site_wasm_profile as reader
import site_wasm_symbols as symbols

NODE_VERSION = "24.21.0"
CASES = {f"{roster}-{seed}": (roster, seed) for roster, seed in pair.SEEDS}
SETTINGS_CHECK = """
const [url, text, roster] = process.argv.slice(1);
process.argv[1] = '';
const {validateSettings} = await import(url);
validateSettings(JSON.parse(text), roster);
"""


def check_run(run: dict, run_id: int, repo: str, receipt: dict) -> None:
    if (run.get("id") != run_id or run.get("status") != "completed"
            or run.get("conclusion") != "success"
            or run.get("path", "").split("@", 1)[0] != ".github/workflows/wasm-qualify.yml"
            or run.get("repository", {}).get("full_name") != repo
            or run.get("head_sha") != receipt["publisher_commit"]):
        raise ValueError("input must be a successful normal qualification from this publisher")


def check_game(row: dict, save: Path, roster: str, seed: int, module_sha: str) -> None:
    pair.validate_run(row, "profile", roster, seed, module_sha)
    if hashlib.sha256(save.read_bytes()).hexdigest() != row["saveHash"]:
        raise ValueError("raw save bytes differ from the completed game")
    frames = [json.loads(line) for line in save.with_suffix(save.suffix + ".trace.jsonl")
              .read_text().splitlines() if line]
    if (len(frames) != row["frames"] + 1 or frames[-1].get("finished") is not True
            or frames[-1].get("turn") != row["turn"]):
        raise ValueError("frame evidence does not cover the complete game")
    digest = hashlib.sha256()
    for frame in frames:
        if not re.fullmatch(r"[0-9a-f]{64}", frame.get("hash", "")):
            raise ValueError("missing full-frame hash")
        digest.update((frame["hash"] + "\n").encode())
    if (digest.hexdigest() != row["traceHash"]
            or any(b["sequence"] != a["sequence"] + 1 for a, b in zip(frames, frames[1:]))):
        raise ValueError("frame hashes or publication sequence changed")


def check_samples(profile: dict) -> int:
    samples, deltas = profile.get("samples", []), profile.get("timeDeltas", [])
    ids = {node["id"] for node in profile.get("nodes", [])}
    if (not samples or len(samples) != len(deltas) or any(node not in ids for node in samples)
            or any(not isinstance(delta, (int, float)) or not math.isfinite(delta)
                   or delta < 0 for delta in deltas) or sum(deltas) <= 0
            or not any(re.fullmatch(r"wasm-function\[\d+\]", node.get("callFrame", {})
                                   .get("functionName", "")) for node in profile.get("nodes", []))):
        raise ValueError("missing or incomplete WebAssembly CPU samples")
    return len(samples)


def profile_game(qualified: Path, source: str, out: Path, case: str, run_id: int,
                 repo: str, node: str = "node", run=subprocess.run) -> dict:
    if case not in CASES or run_id <= 0 or not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        raise ValueError("invalid qualification run, repository or normal game case")
    if out.exists():
        raise FileExistsError("profile output already exists; inspect the original run")
    module, receipt = pair.qualified(qualified, source)
    provenance = json.loads(run(["gh", "api", f"repos/{repo}/actions/runs/{run_id}"],
                                check=True, text=True, capture_output=True).stdout)
    check_run(provenance, run_id, repo, receipt)
    runtime = json.loads(run([node, "-p", "JSON.stringify(process.versions)"], check=True,
                             text=True, capture_output=True).stdout)
    if runtime.get("node") != NODE_VERSION:
        raise ValueError(f"profile runtime must match Node {NODE_VERSION}")
    out.mkdir(parents=True)
    (out / "qualification").mkdir()
    shutil.copyfile(module, out / "qualification/civvis.wasm")
    shutil.copytree(qualified / "wasm-symbols", out / "wasm-symbols")
    shutil.copyfile(qualified / "qualification-info.json", out / "qualification-info.json")
    module, _ = pair.qualified(out, source)
    roster, seed = CASES[case]
    save = out / "game.save.json"
    command = [node, "--cpu-prof", "--cpu-prof-interval=500", f"--cpu-prof-dir={out}",
               "--cpu-prof-name=engine.cpuprofile", str(pair.DRIVER.resolve()),
               str(module.resolve()), str(seed), roster, str(save.resolve())]
    result = {"phase": "profiling", "source": source, "qualification": receipt,
              "qualification_run": run_id, "qualification_url": provenance.get("html_url"),
              "tooling_source": os.environ.get("GITHUB_SHA"), "runtime": runtime,
              "driver_sha256": hashlib.sha256(pair.DRIVER.read_bytes()).hexdigest(),
              "case": case, "command": command,
              "scope": "CPU diagnostic of a full normal game; sampling includes startup. "
                       "Instrumented timings are not a speed comparison. No UI painting measured."}
    receipt_path = out / "receipt.json"

    def record() -> None:
        receipt_path.write_text(json.dumps(result, indent=2) + "\n")

    record()
    try:
        with (out / "game.log").open("w") as log:
            run(command, check=True, text=True, stdout=log, stderr=subprocess.STDOUT)
        rows = [json.loads(line) for line in (out / "game.log").read_text().splitlines()
                if line.startswith("{")]
        final = rows[-1]
        check_game(final, save, roster, seed, receipt["engine"]["wasm_sha256"])
        # Clearing argv[1] before importing prevents the driver's CLI entry
        # point from starting a second game. Only its pure settings check runs.
        run([node, "--input-type=module", "-e", SETTINGS_CHECK, pair.DRIVER.resolve().as_uri(),
             json.dumps(final["settings"]), roster], check=True, text=True, capture_output=True)
        samples = json.loads((out / "engine.cpuprofile").read_text())
        count = check_samples(samples)
        mapping, _ = symbols.load(out / "wasm-symbols", module)
        totals = reader.self_time(samples, mapping)
        with (out / "report.txt").open("w") as report:
            reader.report(totals, top=80, out=report)
        result.update(phase="complete", game=final, samples=count)
        record()
        return result
    except BaseException as error:
        result.update(phase="failed; partial diagnostic retained", error=str(error))
        record()
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qualified", type=Path, required=True)
    parser.add_argument("--source", required=True)
    parser.add_argument("--run", type=int, required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--case", choices=sorted(CASES), default="six-7311003")
    args = parser.parse_args()
    result = profile_game(args.qualified.resolve(), args.source, args.out.resolve(),
                          args.case, args.run, args.repo)
    print(json.dumps({key: result[key] for key in ("phase", "source", "case", "samples")}))


if __name__ == "__main__":
    main()
