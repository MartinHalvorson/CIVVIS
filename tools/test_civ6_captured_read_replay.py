"""Exercise read delivery with independent processes and reject weak controls."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import civ6_captured_read_replay as replay


@unittest.skipUnless(hasattr(os, "mkfifo"), "ordered input replay requires POSIX FIFOs")
class FifoTests(unittest.TestCase):
    def test_writer_stays_connected_until_the_reader_consumes_its_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "events.jsonl"
            feeder = replay.FifoReads(path, [b"x"], 3)
            program = "import os,sys,json; f=os.open(sys.argv[1],os.O_RDONLY|os.O_NONBLOCK); os.set_blocking(f,True); print(json.dumps({'ready':True}),flush=True); sys.stdin.readline(); data=os.read(f,1); print(json.dumps({'data':data.decode()}),flush=True); os.close(f)"
            process = subprocess.Popen([sys.executable, "-c", program, str(path)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
            try:
                self.assertEqual(replay.request_reply(process, 2), {"ready": True})
                self.assertFalse(feeder.done.wait(.1), "the writer disconnected before its queued byte was consumed")
            finally:
                process.stdin.write("read\n")
                process.stdin.flush()
                process.stdin.close()
                process.wait(timeout=2)
                process.stdout.close()
                feeder.close()

    def test_each_open_gets_only_its_own_bytes(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "events.jsonl"
            reads = [b"first\n", b"first\nsecond\n", b"replacement", b"replacement", b"\x00\xff" * 150000]
            feeder = replay.FifoReads(path, reads, 5)
            try:
                program = "import pathlib,sys,json,hashlib; p=pathlib.Path(sys.argv[1]); print(json.dumps([hashlib.sha256(p.read_bytes()).hexdigest() for _ in range(int(sys.argv[2]))]))"
                result = subprocess.run([sys.executable, "-c", program, str(path), str(len(reads))],
                                        capture_output=True, text=True, timeout=5, check=True)
                feeder.finish()
                self.assertEqual(json.loads(result.stdout), [replay.sha(x) for x in reads])
                self.assertEqual(feeder.served, len(reads))
            finally:
                feeder.close()

    def test_fewer_opens_fail_without_leaving_a_writer(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "events.jsonl"
            feeder = replay.FifoReads(path, [b"one", b"two"], 3)
            try:
                subprocess.run([sys.executable, "-c", "import pathlib,sys; pathlib.Path(sys.argv[1]).read_bytes()", str(path)], timeout=3, check=True)
                with self.assertRaisesRegex(RuntimeError, "1/2"):
                    feeder.finish()
                self.assertFalse(feeder.thread.is_alive())
            finally:
                feeder.close()

    def test_no_reader_times_out(self):
        with tempfile.TemporaryDirectory() as directory:
            feeder = replay.FifoReads(Path(directory) / "events.jsonl", [b"one"], 0.1)
            self.assertTrue(feeder.done.wait(2))
            with self.assertRaisesRegex(RuntimeError, "deadline"):
                feeder.finish()

    def test_extra_open_cannot_reuse_the_last_input(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "events.jsonl"
            feeder = replay.FifoReads(path, [b"one"], 3)
            process = subprocess.Popen([sys.executable, "-c", "import pathlib,sys; p=pathlib.Path(sys.argv[1]); p.read_bytes(); p.read_bytes(); print('unexpected',flush=True)", str(path)], stdout=subprocess.PIPE)
            try:
                with self.assertRaises(TimeoutError):
                    replay.request_reply(process, 0.3)
                feeder.finish()
                self.assertIsNone(process.poll())
            finally:
                process.terminate()
                process.wait(timeout=2)
                process.stdout.close()
                feeder.close()

    def test_reader_exit_mid_payload_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "events.jsonl"
            feeder = replay.FifoReads(path, [b"x" * 2000000], 3)
            try:
                subprocess.run([sys.executable, "-c", "import os,sys; f=os.open(sys.argv[1],os.O_RDONLY); os.read(f,1); os.close(f)", str(path)], timeout=3, check=True)
                with self.assertRaises(RuntimeError):
                    feeder.finish()
                self.assertEqual(feeder.served, 0)
            finally:
                feeder.close()

    def test_empty_reads_have_no_consumption_acknowledgement(self):
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "nonempty"):
                replay.FifoReads(Path(directory) / "events.jsonl", [b""], 3)


class ReplyTests(unittest.TestCase):
    def consume(self, program, exception):
        process = subprocess.Popen([sys.executable, "-c", program], stdout=subprocess.PIPE)
        try:
            with self.assertRaises(exception):
                replay.request_reply(process, 0.2)
        finally:
            if process.poll() is None:
                process.terminate()
            process.wait(timeout=2)
            process.stdout.close()

    def test_partial_reply_cannot_disable_the_deadline(self):
        self.consume("import sys,time; sys.stdout.write('{'); sys.stdout.flush(); time.sleep(5)", TimeoutError)

    def test_exit_without_reply_fails(self):
        self.consume("pass", RuntimeError)

    def test_extra_reply_is_rejected(self):
        self.consume("import os; os.write(1,b'{}\\n{}\\n')", ValueError)


class EvidenceTests(unittest.TestCase):
    def capture(self, directory):
        directory.mkdir()
        (directory / "bytes.bin").write_bytes(b"one\ntwo\n")
        rows = [{"id": 0, "parent": None, "offset": 0, "append_bytes": 4, "total_bytes": 4},
                {"id": 1, "parent": 0, "offset": 4, "append_bytes": 4, "total_bytes": 8}]
        (directory / "snapshots.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
        return {"schema": 1, "complete": True, "error": None, "directory": str(directory),
                "source": str(directory.parent / "events.jsonl"), "archive": "bytes.bin",
                "index": "snapshots.jsonl", "reads": [{"snapshot_id": 0, "count": 2}, {"snapshot_id": 1, "count": 1}]}

    def test_order_and_repetition_are_preserved(self):
        with tempfile.TemporaryDirectory() as directory:
            capture = self.capture(Path(directory) / "capture")
            self.assertEqual(replay.expanded_reads(capture), [b"one\n", b"one\n", b"one\ntwo\n"])

    def test_truncated_bytes_and_recorded_read_errors_fail(self):
        with tempfile.TemporaryDirectory() as directory:
            capture = self.capture(Path(directory) / "capture")
            (Path(capture["directory"]) / "bytes.bin").write_bytes(b"one\n")
            with self.assertRaisesRegex(ValueError, "truncated"):
                replay.expanded_reads(capture)
            capture["reads"] = [{"error": "permission denied", "count": 1}]
            with self.assertRaisesRegex(ValueError, "unsupported"):
                replay.expanded_reads(capture)

    def test_sealed_prefix_does_not_follow_later_source_growth(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            capture = self.capture(root / "capture")
            out = root / "out"
            out.mkdir()
            mapping, receipts = replay.seal_captures([{"input_capture": capture}], out)
            (root / "capture/bytes.bin").write_bytes(b"changed source")
            sealed = dict(capture, directory=str(mapping[capture["directory"]]))
            self.assertEqual(replay.expanded_reads(sealed), [b"one\n", b"one\n", b"one\ntwo\n"])
            self.assertEqual(receipts[0]["archive_sha256"], replay.sha(b"one\ntwo\n"))

    def context(self):
        digest = replay.sha(Path(sys.executable).read_bytes())
        return {"processes": [{"argv": [sys.executable, "--mirror", "/original", "--serve", "--with", "original-policy", "--capture-inputs", "/original-capture"], "cwd": "/tmp"}],
                "binary_sha256": digest, "provenance": {"sha256": digest}}

    def test_actual_arguments_are_preserved_except_destinations(self):
        context = self.context()
        command, cwd = replay.replay_command(context, Path("/tmp/control"))
        root = str(Path("/tmp").resolve())
        self.assertEqual(command, [sys.executable, "--mirror", root + "/control", "--serve", "--with", "original-policy", "--capture-inputs", root + "/control/recaptured"])
        self.assertEqual(cwd, "/tmp")
        self.assertEqual(context["processes"][0]["argv"][2], "/original")

    def test_missing_context_or_changed_binary_cannot_qualify(self):
        for case in ["cwd", "provenance", "duplicate", "binary"]:
            context = self.context()
            if case == "cwd":
                context["processes"][0].pop("cwd")
            elif case == "provenance":
                context["provenance"]["sha256"] = "wrong"
            elif case == "duplicate":
                context["processes"][0]["argv"].extend(["--mirror", "/other"])
            else:
                context["binary_sha256"] = "wrong"
            with self.subTest(case=case), self.assertRaises(ValueError):
                replay.replay_command(context, Path("/tmp/control"))


if __name__ == "__main__":
    unittest.main()
