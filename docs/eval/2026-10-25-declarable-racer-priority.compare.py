#!/usr/bin/env python3
"""Run the preregistered 24 internal games; preserve nonactivation and failures."""
import argparse
from concurrent.futures import ThreadPoolExecutor
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import time


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    temporary = path.with_suffix(path.suffix + ".tmp")
    temporary.write_text(json.dumps(value, indent=2) + "\n")
    temporary.replace(path)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--build", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    args = parser.parse_args()
    source = Path(__file__).resolve()
    driver = source.with_suffix(".rs")
    genome = source.with_name("2026-10-25-declarable-racer-priority.genome.json")
    binary, build, out = args.binary.resolve(), args.build.resolve(), args.out.resolve()
    receipt = json.loads(build.read_text())
    assert receipt["binary_sha256"] == sha(binary), "frozen binary differs"
    assert receipt["driver_sha256"] == sha(driver), "driver differs from build"
    assert receipt["genome_sha256"] == sha(genome), "registered genome differs"
    out.mkdir()
    assignments = []
    arms = ["off", "v1", "v2"]
    for index, seed in enumerate(range(59002800, 59002808)):
        ordered = arms[index % 3:] + arms[:index % 3]
        start_index = len(assignments)
        assignments.extend({"index": start_index + offset, "seed": seed, "arm": arm}
                           for offset, arm in enumerate(ordered))
    registration = {"scope": "internal development comparison, not native verification",
                    "jobs": 2, "deadline_seconds": 7200, "assignments": assignments,
                    "driver_sha256": sha(driver), "controller_sha256": sha(source),
                    "genome_sha256": sha(genome), "build": receipt,
                    "registered_at": time.time()}
    save(out / "registration.json", registration)

    def run(case):
        name = f"{case['index']:02d}-{case['seed']}-{case['arm']}"
        directory = out / name
        command = [str(binary), str(genome), str(case["seed"]), case["arm"], str(directory)]
        result = {**case, "command": command, "started_at": time.time(),
                  "directory": str(directory)}
        with (out / f"{name}.log").open("wb") as log:
            process = subprocess.Popen(command, stdout=log, stderr=subprocess.STDOUT,
                                       start_new_session=True)
            result["pid"] = process.pid
            save(out / f"{name}.process.json", result)
            deadline = time.monotonic() + 7200
            timed_out = False
            while True:
                pid, status, usage = os.wait4(process.pid, os.WNOHANG)
                if pid:
                    break
                if time.monotonic() >= deadline:
                    timed_out = True
                    os.killpg(process.pid, signal.SIGKILL)
                    _, status, usage = os.wait4(process.pid, 0)
                    break
                time.sleep(1)
            process.returncode = os.waitstatus_to_exitcode(status)
            result.update(exit_code=process.returncode, timed_out=timed_out,
                          finished_at=time.time(), user_cpu_seconds=usage.ru_utime,
                          system_cpu_seconds=usage.ru_stime, log_sha256=sha(out / f"{name}.log"))
        endpoint = directory / "result.json"
        if process.returncode == 0 and endpoint.exists():
            result["status"] = "complete"
            result["result"] = json.loads(endpoint.read_text())
            result["endpoint_sha256"] = sha(endpoint)
            result["artifact_hashes"] = {p.name: sha(p) for p in directory.iterdir() if p.is_file()}
        else:
            result["status"] = "failed"
            result["partial_artifact_hashes"] = {p.name: sha(p) for p in directory.iterdir() if p.is_file()} if directory.exists() else {}
        save(out / f"{name}.process.json", result)
        print(json.dumps({k: result[k] for k in ("index", "seed", "arm", "status", "exit_code", "timed_out")}), flush=True)
        return result

    with ThreadPoolExecutor(max_workers=2) as pool:
        results = list(pool.map(run, assignments))
    save(out / "results.json", {"registration": registration, "cases": results,
                                "status": "complete", "finished_at": time.time()})


if __name__ == "__main__":
    main()
