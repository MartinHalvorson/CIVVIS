#!/usr/bin/env python3
"""Compare two persistent native deciders against an unchanged recorded history.

This measures decisions and emitted routes, not counterfactual survival or wins.
Both arms see identical event prefixes, carry their own AI memory, and rebuild
the observed board on every state frame. Output must be a new directory; the
source run, its orders database, and the live game are never modified.
"""

from __future__ import annotations

import argparse
from contextlib import ExitStack
import hashlib
import json
from pathlib import Path
import subprocess
import time


def digest(path: Path) -> str:
    sha = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(1024 * 1024), b""):
            sha.update(block)
    return sha.hexdigest()


def actionable_orders(reply: dict) -> list[dict]:
    # CivvisVerify.isVerdict routes all three as telemetry, not game actions.
    return [order for order in reply["orders"]
            if order["kind"] not in ("order_verified", "order_failed", "turn_verified")]


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--force-file", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--through-turn", type=int)
    args = parser.parse_args()
    source = args.run.resolve() / "events.jsonl"
    policies = [
        tag.strip()
        for tag in args.force_file.read_text().strip().replace("\n", ",").split(",")
        if tag.strip()
    ]
    binaries = {name: path.resolve() for name, path in (
        ("baseline", args.baseline), ("candidate", args.candidate)
    )}
    metadata = {
        "scope": "Recorded-history route decisions; no survival or win estimate",
        "source": str(source),
        "source_sha256": digest(source),
        "binary_sha256": {name: digest(path) for name, path in binaries.items()},
        "force_file_sha256": digest(args.force_file),
        "forced_policies": policies,
        "through_turn": args.through_turn,
    }
    args.out.mkdir(parents=True, exist_ok=False)
    started = time.monotonic()
    processes = {}
    frames = changed_orders = changed_actions = no_board_frames = 0
    changed_routes = []
    prefix_sha = hashlib.sha256()
    try:
        with ExitStack() as stack:
            arms = {}
            for name, binary in binaries.items():
                directory = args.out / name
                directory.mkdir()
                events = stack.enter_context((directory / "events.jsonl").open("wb"))
                replies = stack.enter_context((directory / "decisions.jsonl").open("w"))
                why = stack.enter_context((directory / "why.log").open("w"))
                command = [str(binary), "--mirror", str(directory.resolve()),
                           "--serve", "--fresh-board", "--victory", "domination",
                           "--civ", "CIVILIZATION_GRAN_COLOMBIA"]
                for policy in policies:
                    command.extend(["--with", policy])
                processes[name] = subprocess.Popen(
                    command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                    stderr=why, text=True, bufsize=1,
                )
                arms[name] = events, replies
                metadata.setdefault("commands", {})[name] = command
            with source.open("rb") as history:
                for line in history:
                    try:
                        event = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    turn = event.get("turn")
                    if (args.through_turn is not None and isinstance(turn, int)
                            and turn > args.through_turn):
                        break
                    prefix_sha.update(line)
                    for events, _ in arms.values():
                        events.write(line)
                    if event.get("kind") != "state":
                        continue
                    answers = {}
                    for name, (events, replies) in arms.items():
                        events.flush()
                        process = processes[name]
                        process.stdin.write(f"{turn}\n")
                        process.stdin.flush()
                        answer = process.stdout.readline()
                        if not answer:
                            raise RuntimeError(f"{name} closed output at turn {turn}")
                        answers[name] = json.loads(answer)
                        replies.write(answer)
                        replies.flush()
                    before, after = answers["baseline"], answers["candidate"]
                    if actionable_orders(before) != actionable_orders(after):
                        changed_orders += 1
                        changed_routes.append({
                            "turn": turn, "frame": event.get("frame", 0),
                            "baseline": actionable_orders(before),
                            "candidate": actionable_orders(after),
                        })
                    before_decision, after_decision = before.get("decision"), after.get("decision")
                    if before_decision is None and after_decision is None:
                        # The initial state can precede the first terrain export.
                        # Keep both protocol replies, including their empty orders.
                        no_board_frames += 1
                    elif (before_decision is None or after_decision is None or
                          before_decision["native_actions"] != after_decision["native_actions"]):
                        changed_actions += 1
                    frames += 1
                    if frames % 25 == 0:
                        print(json.dumps({"frames": frames, "turn": turn,
                                          "changed_order_frames": changed_orders,
                                          "elapsed": round(time.monotonic() - started, 1)}),
                              flush=True)
            for name, process in processes.items():
                process.stdin.close()
                code = process.wait(timeout=30)
                metadata.setdefault("returncodes", {})[name] = code
                if code:
                    raise RuntimeError(f"{name} exited {code}")
            if not frames:
                raise RuntimeError("No state frames were replayed")
            metadata.update(frames=frames, changed_order_frames=changed_orders,
                            changed_native_action_frames=changed_actions,
                            no_board_frames=no_board_frames,
                            replay_prefix_sha256=prefix_sha.hexdigest())
            (args.out / "route-changes.json").write_text(
                json.dumps(changed_routes, indent=2) + "\n")
    except Exception as error:
        metadata["error"] = str(error)
        raise
    finally:
        for process in processes.values():
            if process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()
        metadata["returncodes"] = {name: process.returncode
                                   for name, process in processes.items()}
        metadata["elapsed_seconds"] = round(time.monotonic() - started, 3)
        (args.out / "provenance.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(metadata), flush=True)


if __name__ == "__main__":
    main()
