#!/usr/bin/env python3
"""Qualify original-binary planning against ordered, captured native reads.

POSIX only. Each filesystem open receives its own FIFO and recorded text.
This never publishes orders, runs a treatment, or estimates native strength.
"""
from __future__ import annotations

import argparse
import sys
sys.path.insert(0, str(__import__("pathlib").Path(__file__).resolve().parents[3] / "tools"))
import array
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import selectors
import select
import subprocess
import threading
import termios
import time

from civ6_decision_trace import captured_input, exact_fact, validate_input_capture
from native_city_route_replay import control_payload, frame_key, request_identity_errors


def sha(data):
    return hashlib.sha256(data).hexdigest()


class FifoReads:
    """Serve one immutable byte string per open, with a distinct next inode."""

    def __init__(self, path, reads, timeout):
        if not reads or any(not data for data in reads):
            raise ValueError("FIFO delivery requires nonempty recorded reads")
        self.path = Path(path)
        self.reads = reads
        self.deadline = time.monotonic() + timeout
        self.cancelled = threading.Event()
        self.done = threading.Event()
        self.error = None
        self.served = 0
        self.phase = "starting"
        self.read_number = None
        self.bytes_sent = 0
        os.mkfifo(self.path)
        self.thread = threading.Thread(target=self._run, daemon=True)
        self.thread.start()

    def _check(self):
        if self.cancelled.is_set():
            raise RuntimeError("input delivery cancelled")
        if time.monotonic() >= self.deadline:
            raise TimeoutError("recorded input open/write deadline expired")

    def _run(self):
        try:
            for number, data in enumerate(self.reads):
                self.read_number = number
                self.phase = "opening"
                self.bytes_sent = 0
                descriptor, observer = None, None
                try:
                    while descriptor is None:
                        self._check()
                        try:
                            descriptor = os.open(self.path, os.O_WRONLY | os.O_NONBLOCK)
                        except OSError as error:
                            if error.errno != errno.ENXIO:
                                raise
                            self.cancelled.wait(0.002)
                    observer = os.open(self.path, os.O_RDONLY | os.O_NONBLOCK)
                    following = self.path.with_name(f".next-read-{number}")
                    os.mkfifo(following)
                    os.replace(following, self.path)
                    self.phase = "writing"
                    remaining = memoryview(data)
                    while remaining:
                        self._check()
                        _, ready, _ = select.select([], [descriptor], [], 0.05)
                        if ready:
                            try:
                                sent = os.write(descriptor, remaining)
                                self.bytes_sent += sent
                                remaining = remaining[sent:]
                            except BlockingIOError:
                                continue
                    self.phase = "draining"
                    queued = array.array("i", [0])
                    while True:
                        self._check()
                        fcntl.ioctl(observer, termios.FIONREAD, queued, True)
                        if queued[0] == 0:
                            break
                        self.cancelled.wait(0.001)
                    self.served += 1
                    self.phase = "closing"
                finally:
                    if observer is not None:
                        os.close(observer)
                    if descriptor is not None:
                        os.close(descriptor)
        except Exception as error:
            self.error = f"{type(error).__name__}: {error}"
        finally:
            self.done.set()

    def finish(self):
        self.done.wait(0.2)
        complete = self.done.is_set() and self.error is None and self.served == len(self.reads)
        error = self.error or f"consumer opened {self.served}/{len(self.reads)} recorded reads"
        self.cancelled.set()
        self.thread.join(timeout=1)
        if self.thread.is_alive():
            raise RuntimeError("input delivery thread did not stop")
        if not complete:
            raise RuntimeError(error)

    def close(self):
        self.cancelled.set()
        self.thread.join(timeout=1)


def expanded_reads(capture):
    validate_input_capture(capture)
    if not capture["complete"] or not capture["reads"]:
        raise ValueError("request has no complete captured read sequence")
    result = []
    for read in capture["reads"]:
        if "snapshot_id" not in read:
            raise ValueError("recorded read errors are unsupported")
        data = captured_input(capture, read["snapshot_id"])
        result.extend([data] * read["count"])
    return result


def replay_command(context, directory):
    processes = context.get("processes", [])
    if len(processes) != 1 or not processes[0].get("cwd"):
        raise ValueError("exactly one observed process and actual cwd are required")
    words = list(processes[0]["argv"])
    for flag, replacement in (("--mirror", directory), ("--capture-inputs", directory / "recaptured")):
        if words.count(flag) != 1:
            raise ValueError(f"observed command requires exactly one {flag}")
        index = words.index(flag) + 1
        if index >= len(words):
            raise ValueError(f"missing value for {flag}")
        words[index] = str(replacement.resolve())
    if "--serve" not in words:
        raise ValueError("observed process must be persistent")
    if sha(Path(words[0]).read_bytes()) != context["binary_sha256"]:
        raise ValueError("original executable no longer matches observed bytes")
    if context.get("provenance", {}).get("sha256") != context["binary_sha256"]:
        raise ValueError("frozen provenance does not match the original executable")
    return words, processes[0]["cwd"]


def select_records(path, through, context):
    result, seen, closed = [], set(), False
    original = context["processes"][0]["argv"][0]
    with path.open() as stream:
        for line in stream:
            record = json.loads(line)
            key = frame_key(record)
            if key is None:
                raise ValueError("native request lacks an explicit identity")
            if key[0] > through:
                closed = True
                break
            if key in seen or (result and key <= frame_key(result[-1])):
                raise ValueError("native requests are duplicated or out of order")
            if record.get("binary") != original or record.get("binary_on_disk_sha256") != context["binary_sha256"]:
                raise ValueError("native request uses a different executable")
            expected_sha = record.get("sha256")
            unsigned = {k: v for k, v in record.items() if k != "sha256"}
            if sha(json.dumps(unsigned, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()) != expected_sha:
                raise ValueError("native decision receipt digest differs")
            validate_input_capture(record.get("input_capture"))
            seen.add(key)
            result.append(record)
    if not result or frame_key(result[0])[0] > 5 or not closed or frame_key(result[-1])[0] != through:
        raise ValueError("complete chronological history through the fixed turn is unavailable")
    return result


def seal_captures(records, out):
    captures = {}
    for record in records:
        capture = record["input_capture"]
        expanded_reads(capture)
        source = capture["directory"]
        maximum = max(r["snapshot_id"] for r in capture["reads"])
        captures[source] = max(captures.get(source, -1), maximum)
    mapping, receipts = {}, []
    for number, (source, maximum) in enumerate(captures.items()):
        original = Path(source)
        destination = out / f"source-capture-{number}"
        destination.mkdir()
        rows, end = [], None
        with (original / "snapshots.jsonl").open("rb") as index:
            for line in index:
                row = json.loads(line)
                rows.append(line)
                if row["id"] == maximum:
                    end = row["offset"] + row["append_bytes"]
                    break
        if end is None:
            raise ValueError("required source capture index is missing")
        index_bytes = b"".join(rows)
        with (original / "bytes.bin").open("rb") as archive:
            archive_bytes = archive.read(end)
        if len(archive_bytes) != end:
            raise ValueError("required source capture bytes are truncated")
        (destination / "snapshots.jsonl").write_bytes(index_bytes)
        (destination / "bytes.bin").write_bytes(archive_bytes)
        mapping[source] = destination
        receipts.append({"source": source, "sealed_directory": str(destination),
                         "maximum_snapshot": maximum, "archive_bytes": end,
                         "archive_sha256": sha(archive_bytes), "index_sha256": sha(index_bytes)})
    return mapping, receipts


def request_reply(process, timeout):
    deadline = time.monotonic() + timeout
    data = bytearray()
    with selectors.DefaultSelector() as selector:
        selector.register(process.stdout, selectors.EVENT_READ)
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0 or not selector.select(remaining):
                raise TimeoutError("decider reply deadline expired")
            chunk = os.read(process.stdout.fileno(), 65536)
            if not chunk:
                raise RuntimeError(f"decider exited before a complete reply ({process.poll()})")
            data.extend(chunk)
            if b"\n" in data:
                line, _, trailing = data.partition(b"\n")
                if trailing.strip():
                    raise ValueError("decider emitted more than one reply")
                return json.loads(line)


def run_control(context, records, mapping, directory, timeout):
    directory.mkdir()
    command, cwd = replay_command(context, directory)
    process, feeder, key = None, None, None
    results = []
    with (directory / "stderr.log").open("w") as stderr, (directory / "replies.jsonl").open("w") as replies:
        try:
            process = subprocess.Popen(command, cwd=cwd, stdin=subprocess.PIPE,
                                       stdout=subprocess.PIPE, stderr=stderr, text=True, bufsize=1)
            for record in records:
                key = frame_key(record)
                capture = dict(record["input_capture"], directory=str(mapping[record["input_capture"]["directory"]]))
                expected = expanded_reads(capture)
                path = directory / "events.jsonl"
                if path.exists():
                    path.unlink()
                # A constant byte sequence needs no open-to-open handoff.
                # Recapture below still verifies every read and its count.
                if all(data == expected[0] for data in expected):
                    path.write_bytes(expected[0])
                else:
                    feeder = FifoReads(path, expected, timeout)
                process.stdin.write(f"{key[0]}\n")
                process.stdin.flush()
                reply = request_reply(process, timeout)
                replies.write(json.dumps(reply, sort_keys=True) + "\n")
                replies.flush()
                if feeder is not None:
                    feeder.finish()
                    feeder = None
                actual = expanded_reads(reply["input_capture"])
                result = {"request": key, "identity_errors": request_identity_errors(reply, *key),
                          "native_payload_matches": exact_fact(control_payload(record), control_payload(reply)),
                          "read_sequence_matches": expected == actual, "recorded_reads": len(expected),
                          "replayed_reads": len(actual), "expected_read_sha256": [sha(x) for x in expected],
                          "actual_read_sha256": [sha(x) for x in actual]}
                results.append(result)
                if result["identity_errors"] or not result["native_payload_matches"] or not result["read_sequence_matches"]:
                    raise ValueError(f"original-binary control differs at {key}")
            process.stdin.close()
            if process.wait(timeout=5) != 0:
                raise RuntimeError("original decider exited unsuccessfully")
            return {"passed": True, "command": command, "cwd": cwd, "requests": results}
        except Exception as error:
            return {"passed": False, "command": command, "cwd": cwd, "requests": results,
                    "failed_request": key,
                    "input_delivery": ({"served": feeder.served, "expected": len(feeder.reads),
                                        "done": feeder.done.is_set(), "error": feeder.error,
                                        "phase": feeder.phase, "read_number": feeder.read_number,
                                        "bytes_sent": feeder.bytes_sent}
                                       if feeder is not None else None),
                    "error": f"{type(error).__name__}: {error}"}
        finally:
            if feeder is not None:
                feeder.close()
            if process is not None and process.poll() is None:
                process.terminate()
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--context", type=Path, required=True)
    parser.add_argument("--decisions", type=Path, required=True)
    parser.add_argument("--through-turn", type=int, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--request-timeout", type=float, default=30)
    args = parser.parse_args()
    if args.through_turn < 1 or args.request_timeout <= 0:
        parser.error("turn and timeout must be positive")
    args.out = args.out.resolve()
    args.out.mkdir(parents=True, exist_ok=False)
    report = {"scope": "original-binary ordered-input reproduction only; no treatment or native strength estimate",
              "passed": False, "through_turn": args.through_turn, "controls": []}
    try:
        context = json.loads(args.context.read_text())
        replay_command(context, args.out / "preflight")
        records = select_records(args.decisions, args.through_turn, context)
        (args.out / "observed-context.json").write_text(json.dumps(context, indent=2) + "\n")
        native = "".join(json.dumps(r, sort_keys=True) + "\n" for r in records)
        (args.out / "native-decisions.jsonl").write_text(native)
        report["native_decisions_sha256"] = sha(native.encode())
        mapping, report["sealed_captures"] = seal_captures(records, args.out)
        for number in range(2):
            result = run_control(context, records, mapping, args.out / f"control-{number}", args.request_timeout)
            report["controls"].append(result)
            if not result["passed"]:
                break
        report["passed"] = len(report["controls"]) == 2 and all(r["passed"] for r in report["controls"])
    except Exception as error:
        report["error"] = f"{type(error).__name__}: {error}"
    (args.out / "qualification.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps({"passed": report["passed"], "controls": len(report["controls"]),
                      "requests": [len(r["requests"]) for r in report["controls"]],
                      "error": report.get("error"), "out": str(args.out)}))
    return 0 if report["passed"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
