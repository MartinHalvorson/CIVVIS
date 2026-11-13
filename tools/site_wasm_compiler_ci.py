#!/usr/bin/env python3
"""Observe compilation during two complete qualified normal games on remote CI.

The control uses the normal driver unchanged. The second process adds only
V8's compilation logging flag. Original saves and entire publication traces
must match before the diagnostic completes. Reported compilation durations
can overlap across threads, are quantized, and are not CPU or speed results.
Compiler allocation counters are not the WebAssembly heap. Tests play no game.
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
from collections import defaultdict
from pathlib import Path

import site_wasm_pair as pair
import site_wasm_profile as reader
import site_wasm_profile_ci as guard
import site_wasm_symbols as symbols

TRACE_FLAG = "--trace-wasm-compilation-times"
EVENT = re.compile(
    r"^Compiled function (?P<module>0x[0-9a-fA-F]+)#(?P<index>\d+) "
    r"using (?P<tier>Liftoff|TurboFan|Turboshaft), took (?P<ms>\d+(?:\.\d+)?) ms "
    r"and (?P<bytes>\d+)(?: / (?P<total>\d+) max/total)? bytes; "
    r"bodysize (?P<body>\d+) codesize (?P<code>\d+)(?: name (?P<name>.*))?$"
)


def function_sizes(data: bytes) -> dict[int, int]:
    """Read exact code-body byte lengths; qualified engines have no imports."""
    if data[:8] != b"\0asm\1\0\0\0":
        raise ValueError("not a WebAssembly 1 module")

    def number(pos: int) -> tuple[int, int]:
        value = 0
        for shift in range(0, 35, 7):
            if pos >= len(data):
                raise ValueError("truncated WebAssembly integer")
            byte = data[pos]
            pos += 1
            value |= (byte & 127) << shift
            if byte < 128:
                if value >= 2 ** 32:
                    break
                return value, pos
        raise ValueError("invalid WebAssembly integer")

    pos, sizes, declarations = 8, None, None
    while pos < len(data):
        section = data[pos]
        length, start = number(pos + 1)
        end = start + length
        if end > len(data):
            raise ValueError("truncated WebAssembly section")
        if section == 2:
            imports, _ = number(start)
            if imports:
                raise ValueError("qualified engine must not import host state")
        elif section == 3:
            declarations, _ = number(start)
        elif section == 10:
            if sizes is not None:
                raise ValueError("duplicate code section")
            count, cursor = number(start)
            sizes = {}
            for index in range(count):
                size, cursor = number(cursor)
                if size < 2 or cursor + size > end:
                    raise ValueError("truncated or empty function body")
                sizes[index] = size
                cursor += size
            if cursor != end:
                raise ValueError("code section has trailing bytes")
        pos = end
    if not sizes or declarations != len(sizes):
        raise ValueError("missing or inconsistent function bodies")
    return sizes


def compilation_events(text: str, mapping: dict[int, str], sizes: dict[int, int]) -> list[dict]:
    if set(mapping) != set(sizes):
        raise ValueError("function map does not cover exact module bodies")
    events, modules = [], set()
    for line in text.splitlines():
        if not line.startswith("Compiled function "):
            continue
        match = EVENT.fullmatch(line)
        if not match:
            raise ValueError("unrecognized compilation record: " + line[:240])
        index, body = int(match["index"]), int(match["body"])
        duration = float(match["ms"])
        if (index not in sizes or sizes[index] != body or not math.isfinite(duration)
                or duration < 0 or int(match["code"]) <= 0):
            raise ValueError("compilation record differs from bound module")
        modules.add(match["module"])
        event = {"function_index": index, "symbol": mapping[index],
                 "function": reader.readable(mapping[index]), "tier": match["tier"],
                 "reported_elapsed_ms": duration, "body_bytes": body,
                 "generated_code_bytes": int(match["code"]), "trace_name": match["name"]}
        if match["total"] is None:
            event["compiler_single_reported_bytes"] = int(match["bytes"])
        else:
            maximum, total = int(match["bytes"]), int(match["total"])
            if maximum > total:
                raise ValueError("compiler max/total byte counters are inconsistent")
            event.update(compiler_max_bytes=maximum, compiler_total_bytes=total)
        events.append(event)
    if not events or len(modules) != 1 or not any(x["tier"] == "Liftoff" for x in events):
        raise ValueError("missing compilation records or multiple modules observed")
    return events


def summarize(events: list[dict]) -> dict:
    tiers = defaultdict(lambda: {"events": 0, "reported_elapsed_ms_sum": 0.0,
                                "generated_code_bytes_sum": 0})
    functions = {}
    for event in events:
        tier = tiers[event["tier"]]
        tier["events"] += 1
        tier["reported_elapsed_ms_sum"] += event["reported_elapsed_ms"]
        tier["generated_code_bytes_sum"] += event["generated_code_bytes"]
        index = event["function_index"]
        row = functions.setdefault(index, {"function_index": index,
                                          "function": event["function"],
                                          "body_bytes": event["body_bytes"],
                                          "events": 0, "reported_elapsed_ms_sum": 0.0,
                                          "tiers": {}})
        row["events"] += 1
        row["reported_elapsed_ms_sum"] += event["reported_elapsed_ms"]
        row["tiers"][event["tier"]] = row["tiers"].get(event["tier"], 0) + 1
    return {"event_count": len(events), "tier_totals": dict(tiers),
            "functions_by_reported_time": sorted(functions.values(),
                key=lambda row: (-row["reported_elapsed_ms_sum"], -row["body_bytes"],
                                 row["function_index"])),
            "largest_events": sorted(events, key=lambda event:
                (-event["reported_elapsed_ms"], -event["body_bytes"]))[:40],
            "scope": "Reported compilation durations are quantized, can overlap across "
                     "threads, and are not CPU shares, critical-path time or a speed result. "
                     "Compiler byte counters are not the WebAssembly heap."}


def final_game(text: str) -> dict:
    rows = [json.loads(line) for line in text.splitlines() if line.startswith("{")]
    final = [row for row in rows if row.get("harnessVersion") == 7
             and row.get("finished") is True and "moduleSha256" in row]
    if len(final) != 1:
        raise ValueError("expected exactly one complete game record")
    return final[0]


def observe(qualified: Path, source: str, out: Path, case: str, run_id: int,
            repo: str, node: str = "node", run=subprocess.run) -> dict:
    if case not in guard.CASES or run_id <= 0 or not re.fullmatch(r"[\w.-]+/[\w.-]+", repo):
        raise ValueError("invalid qualification, repository or normal case")
    if out.exists():
        raise FileExistsError("diagnostic exists; inspect original evidence")
    module, qualification = pair.qualified(qualified, source)
    provenance = json.loads(run(["gh", "api", f"repos/{repo}/actions/runs/{run_id}"],
                                check=True, text=True, capture_output=True).stdout)
    guard.check_run(provenance, run_id, repo, qualification)
    runtime = json.loads(run([node, "-p", "JSON.stringify(process.versions)"],
                            check=True, text=True, capture_output=True).stdout)
    if runtime.get("node") != guard.NODE_VERSION:
        raise ValueError("diagnostic runtime must match Node " + guard.NODE_VERSION)
    out.mkdir(parents=True)
    (out / "qualification").mkdir()
    shutil.copyfile(module, out / "qualification/civvis.wasm")
    shutil.copytree(qualified / "wasm-symbols", out / "wasm-symbols")
    shutil.copyfile(qualified / "qualification-info.json", out / "qualification-info.json")
    module, _ = pair.qualified(out, source)
    mapping, _ = symbols.load(out / "wasm-symbols", module)
    sizes = function_sizes(module.read_bytes())
    if set(mapping) != set(sizes):
        raise ValueError("function map does not cover qualified code")
    roster, seed = guard.CASES[case]
    result = {"phase": "observing", "source": source, "case": case,
              "qualification": qualification, "qualification_run": run_id,
              "qualification_url": provenance.get("html_url"), "runtime": runtime,
              "tooling_source": os.environ.get("GITHUB_SHA"),
              "driver_sha256": hashlib.sha256(pair.DRIVER.read_bytes()).hexdigest(),
              "games": {}, "commands": {},
              "compiler_event_window": "Whole traced process, including module startup "
                                       "and final save outside the driver's game timing bracket.",
              "scope": "Two complete normal games: uninstrumented control and compiler "
                       "logging only. No CPU profiler or optimization/tiering flags. "
                       "Diagnostic timings establish no performance gain or website milestone."}

    def record() -> None:
        (out / "receipt.json").write_text(json.dumps(result, indent=2) + "\n")

    record()
    try:
        for arm in ("control", "traced"):
            save = out / (arm + ".save.json")
            command = [node] + ([TRACE_FLAG] if arm == "traced" else []) + [
                str(pair.DRIVER.resolve()), str(module.resolve()), str(seed), roster,
                str(save.resolve())]
            result["commands"][arm] = command
            result["phase"] = "original " + arm + " full normal game running"
            record()
            with (out / (arm + ".log")).open("w") as log:
                run(command, check=True, text=True, stdout=log,
                    stderr=subprocess.STDOUT, timeout=900)
            row = final_game((out / (arm + ".log")).read_text())
            guard.check_game(row, save, roster, seed, qualification["engine"]["wasm_sha256"])
            run([node, "--input-type=module", "-e", guard.SETTINGS_CHECK,
                 pair.DRIVER.resolve().as_uri(), json.dumps(row["settings"]), roster],
                check=True, text=True, capture_output=True)
            result["games"][arm] = row
            record()
        pair.same_game(result["games"]["control"], result["games"]["traced"])
        for suffix in (".save.json", ".save.json.trace.jsonl"):
            if (out / ("control" + suffix)).read_bytes() != (out / ("traced" + suffix)).read_bytes():
                raise ValueError("original save or entire trace differs between arms")
        events = compilation_events((out / "traced.log").read_text(), mapping, sizes)
        (out / "compilation-events.json").write_text(json.dumps(events, indent=2) + "\n")
        report = summarize(events)
        (out / "compilation-report.json").write_text(json.dumps(report, indent=2) + "\n")
        control = result["games"]["control"]
        result.update(phase="complete", compilation_events=len(events),
                      unprofiled_background_cpu_lower_bound_ms=max(
                          0.0, control["cpuMs"] - control["wallMs"]),
                      original_save_and_entire_trace_bytes_identical=True)
        record()
        return result
    except BaseException as error:
        result.update(phase="failed; original partial evidence retained", error=str(error))
        record()
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--qualified", type=Path, required=True)
    parser.add_argument("--source", required=True)
    parser.add_argument("--run", type=int, required=True)
    parser.add_argument("--repo", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--case", choices=sorted(guard.CASES), default="default-7")
    args = parser.parse_args()
    result = observe(args.qualified.resolve(), args.source, args.out.resolve(), args.case,
                     args.run, args.repo)
    print(json.dumps({key: result[key] for key in ("phase", "source", "case", "compilation_events")}))


if __name__ == "__main__":
    main()
