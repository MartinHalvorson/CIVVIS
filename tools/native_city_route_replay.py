#!/usr/bin/env python3
"""Compare two persistent native deciders against an unchanged recorded history.

This measures decisions and emitted routes, not counterfactual survival or wins.
Both arms see identical event prefixes, carry their own AI memory, and rebuild
the observed board on every state frame. Output must be a new directory; the
source run, its orders database, and the live game are never modified.

The original replay is compared to recorded native orders AND the complete
decision, including verification telemetry. The earliest changed complete
reply must match native in the original arm; a later match cannot replace it.
With identical arms, every original frame must match. Failed controls exit 2
after preserving the diagnostics. This gate does not estimate native wins.
Both arms must also identify the requested integer turn and decision frame;
matching native planning fields cannot license a reply to another request.
"""

from __future__ import annotations

import argparse
from contextlib import ExitStack
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import time

from civ6_decision_trace import exact_fact


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


def frame_key(record: dict) -> tuple[int, int] | None:
    decision = record.get("decision")
    frame = decision if isinstance(decision, dict) else record
    turn, number = frame.get("turn"), frame.get("frame")
    if (type(turn) is not int or type(number) is not int
            or turn < 0 or number < 0):
        return None
    return turn, number


def load_native_records(path: Path) -> tuple[dict, int]:
    records = {}
    unkeyed = 0
    with path.open() as source:
        for number, line in enumerate(source, 1):
            if not line.strip():
                continue
            record = json.loads(line)
            if not isinstance(record, dict) or not isinstance(record.get("orders"), list):
                raise ValueError(f"{path}:{number}: expected a native reply with orders")
            key = frame_key(record)
            if key is None:
                # A reply without an explicit frame cannot establish which
                # request saw it. Keep the coverage gap instead of guessing 0.
                unkeyed += 1
                continue
            if key in records:
                raise ValueError(f"{path}:{number}: ambiguous duplicate native frame {key}")
            records[key] = record
    return records, unkeyed


def control_payload(reply: dict) -> dict:
    return {key: reply[key] for key in ("orders", "decision") if key in reply}


def request_identity_errors(reply: dict, turn: int, frame: int) -> list[str]:
    errors = []
    if type(reply.get("turn")) is not int or reply["turn"] != turn:
        errors.append("reply.turn must match the requested integer turn")
    if "decision" not in reply:
        # The CLI can answer before terrain exists. Such a reply has no board
        # identity to compare, and can carry only an empty order list.
        if reply.get("orders") != []:
            errors.append("a reply without a decision must have empty orders")
        return errors
    decision = reply["decision"]
    if not isinstance(decision, dict):
        errors.append("decision must be an object when present")
        return errors
    for key, expected in (("turn", turn), ("frame", frame)):
        if type(decision.get(key)) is not int or decision[key] != expected:
            errors.append(f"decision.{key} must match the requested integer {key}")
    return errors


class NativeControl:
    def __init__(self, records: dict, unkeyed: int):
        self.records = records
        self.unkeyed = unkeyed
        self.frames = []
        self.first_mismatch = None
        self.first_change = None
        self.changed_complete_replies = 0
        self.request_errors = []

    def observe(self, turn: int, frame: int, original: dict, candidate: dict) -> None:
        identity_errors = {name: request_identity_errors(reply, turn, frame)
                           for name, reply in (("baseline", original), ("candidate", candidate))}
        for name, reply in (("baseline", original), ("candidate", candidate)):
            if identity_errors[name]:
                self.request_errors.append({"index": len(self.frames), "turn": turn,
                                            "frame": frame, "arm": name,
                                            "errors": identity_errors[name], "reply": reply})
        recorded = self.records.get((turn, frame))
        orders_match = recorded is not None and exact_fact(original["orders"], recorded["orders"])
        decision_match = recorded is not None and (
            "decision" in original) == ("decision" in recorded) and (
            exact_fact(original.get("decision"), recorded.get("decision")))
        row = {"index": len(self.frames), "turn": turn, "frame": frame,
               "native_record_present": recorded is not None,
               "orders_match": orders_match, "complete_decision_match": decision_match,
               "original_request_matches": not identity_errors["baseline"],
               "candidate_request_matches": not identity_errors["candidate"],
               "whole_match": orders_match and decision_match}
        self.frames.append(row)
        if not row["whole_match"] and self.first_mismatch is None:
            self.first_mismatch = {**row, "recorded": control_payload(recorded or {}),
                                   "original": control_payload(original)}
        if not exact_fact(original, candidate):
            self.changed_complete_replies += 1
            if self.first_change is None:
                self.first_change = {**row, "recorded": control_payload(recorded or {}),
                                     "original": original, "candidate": candidate}

    def result(self) -> dict:
        mismatches = [row for row in self.frames if not row["whole_match"]]
        history_gate = (self.first_change["whole_match"] if self.first_change is not None
                        else bool(self.frames) and not mismatches)
        identity_gate = bool(self.frames) and not self.request_errors
        return {"scope": "Complete native orders and decision; no survival or win estimate",
                "gate_passed": history_gate and identity_gate,
                "native_history_gate_passed": history_gate,
                "request_identity_gate_passed": identity_gate,
                "invalid_request_reply_frames": len({row["index"] for row in self.request_errors}),
                "request_identity_errors": self.request_errors,
                "gate_kind": ("earliest_changed_complete_reply" if self.first_change is not None
                              else "unchanged_original_history"),
                "frames": len(self.frames),
                "matched_frames": len(self.frames) - len(mismatches),
                "missing_record_frames": sum(not row["native_record_present"] for row in self.frames),
                "unkeyed_native_records": self.unkeyed,
                "changed_complete_reply_frames": self.changed_complete_replies,
                "first_mismatch": self.first_mismatch,
                "first_changed_complete_reply": self.first_change,
                "all_native_mismatches": mismatches}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--force-file", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--through-turn", type=int)
    parser.add_argument("--recorded-decisions", type=Path,
                        help="native replies (default: <run>/decisions.jsonl)")
    parser.add_argument("--explain", action="store_true", help="record each decider's journal")
    args = parser.parse_args()
    source = args.run.resolve() / "events.jsonl"
    recorded = (args.recorded_decisions or args.run / "decisions.jsonl").resolve()
    control = NativeControl(*load_native_records(recorded))
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
        "recorded_decisions": str(recorded),
        "recorded_decisions_sha256": digest(recorded),
    }
    inputs = {"events": source, "recorded_decisions": recorded,
              "force_file": args.force_file.resolve(), **binaries}
    input_hashes = {name: digest(path) for name, path in inputs.items()}
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
                if args.explain:
                    command.append("--explain")
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
                    frame = event.get("frame")
                    if type(turn) is not int or type(frame) is not int:
                        raise ValueError("state frame needs explicit integer turn and frame")
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
                    control.observe(turn, frame, before, after)
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
        metadata["inputs_unchanged"] = {
            name: digest(path) == input_hashes[name] if path.is_file() else False
            for name, path in inputs.items()}
        native_control = control.result()
        (args.out / "native-control.json").write_text(json.dumps(native_control, indent=2) + "\n")
        metadata["native_control"] = {key: native_control[key] for key in (
            "gate_passed", "native_history_gate_passed", "request_identity_gate_passed",
            "invalid_request_reply_frames", "gate_kind", "frames", "matched_frames",
            "missing_record_frames", "changed_complete_reply_frames")}
        metadata["validation_passed"] = (
            "error" not in metadata and native_control["gate_passed"]
            and all(metadata["inputs_unchanged"].values())
            and all(code == 0 for code in metadata["returncodes"].values()))
        (args.out / "provenance.json").write_text(json.dumps(metadata, indent=2) + "\n")
    print(json.dumps(metadata), flush=True)
    return 0 if metadata["validation_passed"] else 2


if __name__ == "__main__":
    sys.exit(main())
