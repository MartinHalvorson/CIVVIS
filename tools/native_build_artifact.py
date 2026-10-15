#!/usr/bin/env python3
"""Build a fixed native candidate and verify its source and artifact receipts."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess

FAST_PROFILE = {"CARGO_PROFILE_RELEASE_CODEGEN_UNITS": "16",
                "CARGO_PROFILE_RELEASE_LTO": "false", "CARGO_INCREMENTAL": "1"}


def arm64_executable(path: Path) -> bool:
    # Apple SDK: mach-o/loader.h MH_MAGIC_64; mach/machine.h CPU_TYPE_ARM64.
    with path.open("rb") as stream:
        header = stream.read(8)
    return len(header) == 8 and struct.unpack("<II", header) == (0xfeedfacf, 0x0100000c)


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        value = hashlib.sha256()
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(block)
    return value.hexdigest()


def source_manifest(repo: Path, expected: str) -> dict:
    if re.fullmatch(r"[0-9a-f]{40}", expected) is None:
        raise ValueError("source SHA must be exactly forty lowercase hexadecimal characters")
    head = subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
    if head != expected:
        raise ValueError(f"source HEAD {head} differs from requested {expected}")
    dirty = subprocess.check_output(["git", "-C", str(repo), "status", "--porcelain"], text=True)
    if dirty:
        raise ValueError("source checkout is dirty")
    paths = subprocess.check_output(["git", "-C", str(repo), "ls-files", "-z"]).decode().split("\0")
    return {"sha": head, "files": {p: digest(repo / p) for p in paths if p}}


def selection(text: str) -> list[tuple[str, str]]:
    value = json.loads(text)
    if not isinstance(value, dict) or not value:
        raise ValueError("test selection must name at least one target")
    result = []
    for target, filters in value.items():
        if target not in ("lib", "civvis_orders") or not isinstance(filters, list) or not filters:
            raise ValueError("test targets are lib and civvis_orders, with nonempty filter lists")
        for pattern in filters:
            if not isinstance(pattern, str) or re.fullmatch(r"[A-Za-z_][A-Za-z0-9_:]*", pattern) is None:
                raise ValueError("each test filter must be a Rust module or test name")
            result.append((target, pattern))
    return result


def verify(repo: Path, artifact: Path, expected: str, tests: str = "", toolchain: str = "") -> dict:
    receipt = json.loads((artifact / "receipt.json").read_text())
    if receipt.get("schema") != 1 or receipt.get("status") != "complete":
        raise ValueError("artifact has no completed receipt")
    if receipt.get("architecture") != "arm64":
        raise ValueError("artifact is not an Apple Silicon build")
    if receipt.get("profile") != FAST_PROFILE:
        raise ValueError("artifact does not use the native fast build profile")
    if toolchain and not receipt.get("rustc", "").startswith(f"rustc {toolchain} "):
        raise ValueError("artifact compiler differs from the requested toolchain")
    if receipt.get("source") != source_manifest(repo, expected):
        raise ValueError("local source inputs differ from the built checkout")
    binaries = receipt.get("binaries", {})
    if set(binaries) != {"civvis", "civvis_orders"}:
        raise ValueError("receipt must cover both native executables")
    for name, sha in binaries.items():
        if digest(artifact / name) != sha:
            raise ValueError(f"binary checksum differs: {name}")
        if not arm64_executable(artifact / name):
            raise ValueError(f"binary is not an arm64 Mach-O executable: {name}")
    if not receipt.get("tests") or any(type(t.get("passed")) is not int or t["passed"] <= 0 for t in receipt["tests"]):
        raise ValueError("artifact has no nonempty passing test selections")
    covered = {(t.get("target"), t.get("filter")) for t in receipt["tests"]}
    if tests and not set(selection(tests)).issubset(covered):
        raise ValueError("artifact does not cover the requested test selections")
    return receipt


def smoke_orders(binary: Path, repo: Path, out: Path, env: dict) -> None:
    smoke = out / "orders-smoke-input"
    smoke.mkdir()
    state = {"kind": "state", "ctx": "agent", "turn": 1, "frame": 0, "gold": 200,
             "cities": [{"id": 7, "name": "Bogota", "x": 4, "y": 6, "pop": 3,
                         "capital": True}],
             "units": [{"id": 17, "kind": "UNIT_WARRIOR", "x": 5, "y": 6,
                        "hp": 100, "moves": 2}],
             "rivals": [{"player": 1, "can_declare": False,
                         "cities": [{"id": 70, "name": "Berlin", "x": 12,
                                     "y": 6, "pop": 3, "capital": True}]}]}
    tiles = {"kind": "tiles", "ctx": "agent", "turn": 1, "width": 16, "height": 12, "chunk": 1,
             "plots": [{"x": x, "y": y, "t": "TERRAIN_GRASS", "vis": True}
                       for x in range(16) for y in range(12)]}
    (smoke / "events.jsonl").write_text(json.dumps(state) + "\n" + json.dumps(tiles) + "\n" +
        json.dumps({"kind": "tiles_done", "ctx": "agent", "turn": 1}) + "\n")
    answer = subprocess.run([str(binary), "--mirror", str(smoke),
                             "--turn", "1", "--players", "2", "--victory", "domination"],
                            cwd=repo, env=env, capture_output=True, text=True, check=True)
    (out / "orders-smoke.json").write_text(answer.stdout)
    (out / "orders-smoke.stderr.log").write_text(answer.stderr)
    payload = json.loads(answer.stdout)
    if (payload.get("decision", {}).get("turn") != 1
            or payload.get("decision", {}).get("frame") != 0
            or not payload.get("decision", {}).get("native_actions")
            or not isinstance(payload.get("orders"), list)):
        raise ValueError("packaged decider did not answer the smoke input")


def build(repo: Path, out: Path, expected: str, tests: str, toolchain: str = "") -> None:
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise ValueError("native candidate builds require an arm64 macOS runner")
    before = source_manifest(repo, expected)
    filters = selection(tests)
    out.mkdir(parents=True, exist_ok=False)
    env = dict(os.environ, CARGO_TARGET_DIR=str(repo / "target"), **FAST_PROFILE)
    receipt = {"schema": 1, "status": "building", "source": before,
               "architecture": "arm64", "platform": platform.platform(),
               "rustc": subprocess.check_output(["rustc", "-vV"], text=True),
               "profile": FAST_PROFILE.copy(),
               "tests": [], "binaries": {}}
    def save():
        (out / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    def run(command, name):
        print(f"native build: {name}", flush=True)
        with (out / f"{name}.log").open("w") as log:
            subprocess.run(command, cwd=repo, env=env, stdout=log,
                           stderr=subprocess.STDOUT, check=True)
    save()
    try:
        if toolchain and not receipt["rustc"].startswith(f"rustc {toolchain} "):
            raise ValueError("runner compiler differs from the requested toolchain")
        run(["cargo", "build", "--release", "--locked", "-j", "1",
             "--bin", "civvis_orders", "--bin", "civvis"], "build")
        for index, (target, pattern) in enumerate(filters):
            command = ["cargo", "test", "--release", "--locked", "-j", "1"]
            command += ["--lib"] if target == "lib" else ["--bin", target]
            command += [pattern, "--", "--test-threads=2"]
            name = f"test-{index}"
            run(command, name)
            matches = re.findall(r"test result: ok\. (\d+) passed; 0 failed;", (out / f"{name}.log").read_text())
            passed = sum(map(int, matches))
            if passed == 0:
                raise ValueError(f"test selection matched no passing tests: {target}:{pattern}")
            receipt["tests"].append({"target": target, "filter": pattern, "passed": passed})
            save()
        if source_manifest(repo, expected) != before:
            raise ValueError("source inputs changed during compilation or tests")
        for name in ("civvis", "civvis_orders"):
            binary = repo / "target" / "release" / name
            arch = subprocess.check_output(["lipo", "-archs", str(binary)], text=True).strip()
            if arch != "arm64":
                raise ValueError(f"unexpected executable architecture: {name}: {arch}")
            shutil.copy2(binary, out / name)
            receipt["binaries"][name] = digest(out / name)
        run([str(out / "civvis"), "simulate", "--players", "4", "--width", "24",
             "--height", "16", "--turns", "20", "--seed", "1"], "smoke")
        smoke_orders(out / "civvis_orders", repo, out, env)
        receipt["status"] = "complete"
        save()
        verify(repo, out, expected, tests, toolchain)
    except Exception as error:
        receipt["status"] = "failed"
        receipt["error"] = str(error)
        save()
        raise


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=("build", "verify"))
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--artifact", type=Path, required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--tests", default='{"lib":["siege_train::muster_tests","find_the_capital::tests"]}')
    parser.add_argument("--toolchain", default="")
    args = parser.parse_args()
    if args.command == "build":
        build(args.repo.resolve(), args.artifact.resolve(), args.sha, args.tests, args.toolchain)
    else:
        receipt = verify(args.repo.resolve(), args.artifact.resolve(), args.sha, args.tests, args.toolchain)
        print(f"verified {receipt['source']['sha']}: {len(receipt['source']['files'])} source files, two binaries")


if __name__ == "__main__":
    main()
