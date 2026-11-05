"""Exercise read delivery with independent processes and reject weak controls."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import civ6_captured_read_replay as replay


class ReadHashTests(unittest.TestCase):
    def test_repeated_objects_keep_each_read_but_hash_each_run_once(self):
        first, second = bytes(bytearray(b"first")), bytes(bytearray(b"second"))
        reads = [first] * 18 + [second] * 3 + [first] * 2
        expected = [replay.sha(data) for data in reads]
        with mock.patch.object(replay, "sha", wraps=replay.sha) as digest:
            self.assertEqual(replay.read_sha256(reads), expected)
        self.assertEqual(digest.call_count, 3)
        self.assertEqual(replay.read_sha256([]), [])

    def test_equal_distinct_objects_are_not_cached_by_content(self):
        first = bytes(bytearray(b"same"))
        second = bytes(bytearray(b"same"))
        self.assertIsNot(first, second)
        with mock.patch.object(replay, "sha", wraps=replay.sha) as digest:
            hashes = replay.read_sha256([first, second, first])
        self.assertEqual(hashes, [replay.sha(first)] * 3)
        self.assertEqual(digest.call_count, 3)

    def test_mutable_buffers_are_rehashed_even_when_identity_is_repeated(self):
        data = bytearray(b"first")
        expected = [replay.sha(data), replay.sha(b"other")]

        def changing_reads():
            yield data
            data[:] = b"other"
            yield data

        self.assertEqual(replay.read_sha256(changing_reads()), expected)

    def test_cache_does_not_survive_the_call_or_hide_invalid_input(self):
        data = b"snapshot"
        expected = replay.sha(data)
        with mock.patch.object(replay, "sha", wraps=replay.sha) as digest:
            self.assertEqual(replay.read_sha256([data, data]), [expected] * 2)
            self.assertEqual(replay.read_sha256([data]), [expected])
        self.assertEqual(digest.call_count, 2)
        with self.assertRaises(TypeError):
            replay.read_sha256([None])


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




class HybridDeliveryTests(unittest.TestCase):
    def test_regular_snapshot_never_claims_a_consumption_count(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "events.jsonl"
            data = b"identical\x00snapshot"
            feeder = replay.IdenticalReads(path, [data] * 2)
            program = "import pathlib,sys,json; p=pathlib.Path(sys.argv[1]); print(json.dumps([p.read_bytes().hex() for _ in range(3)]))"
            result = subprocess.run([sys.executable, "-c", program, str(path)],
                                    capture_output=True, text=True, check=True, timeout=3)
            self.assertEqual(json.loads(result.stdout), [data.hex()] * 3)
            feeder.finish()
            self.assertIsNone(feeder.served, "extra opens are judged by the actual capture, not inferred here")
            feeder.close()

    def test_regular_snapshot_rejects_empty_or_variable_reads(self):
        with tempfile.TemporaryDirectory() as directory:
            for reads in ([], [b""], [b"one", b"two"]):
                with self.subTest(reads=reads), self.assertRaises(ValueError):
                    replay.IdenticalReads(Path(directory) / "events.jsonl", reads)

    def test_hybrid_variable_reads_keep_the_original_fifo(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "events.jsonl"
            reads = [b"first", b"second"]
            feeder = replay.deliver_reads(path, reads, 3, "hybrid")
            try:
                self.assertIsInstance(feeder, replay.FifoReads)
                program = "import pathlib,sys,json; p=pathlib.Path(sys.argv[1]); print(json.dumps([p.read_bytes().decode() for _ in range(2)]))"
                result = subprocess.run([sys.executable, "-c", program, str(path)],
                                        capture_output=True, text=True, check=True, timeout=3)
                feeder.finish()
                self.assertEqual(json.loads(result.stdout), ["first", "second"])
            finally:
                feeder.close()

    def test_tail_keeps_distinct_prefix_opens_and_never_claims_tail_consumption(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "events.jsonl"
            reads = [b"first", b"second", b"first", b"tail", b"tail"]
            feeder = replay.deliver_reads(path, reads, 3, "hybrid-tail")
            try:
                program = "import pathlib,sys,json; p=pathlib.Path(sys.argv[1]); blobs=[p.read_bytes().decode() for _ in range(6)]; print(json.dumps({'blobs':blobs,'regular':p.is_file()}))"
                result = subprocess.run([sys.executable, "-c", program, str(path)],
                                        capture_output=True, text=True, check=True, timeout=3)
                feeder.finish()
                observed = json.loads(result.stdout)
                self.assertEqual(observed["blobs"], [x.decode() for x in reads] + ["tail"])
                self.assertTrue(observed["regular"])
                self.assertEqual(feeder.prefix_served, 3)
                self.assertIsNone(feeder.served)
            finally:
                feeder.close()

    def probe(self, actual_count=2, frame=0, marker="same", complete=True, delivery="hybrid", expected_blobs=None):
        # An independent child opens the real delivered path and constructs its
        # own capture from the bytes it actually read. It never sees the
        # expected capture count or the expected decision payload.
        program = r"""
import json,pathlib,sys
root=pathlib.Path(sys.argv[1]); count=int(sys.argv[2]); frame=int(sys.argv[3]); marker=sys.argv[4]; complete=sys.argv[5]=='true'
for line in sys.stdin:
    blobs=[(root/'events.jsonl').read_bytes() for _ in range(count)]
    capture=root/'actual'; capture.mkdir()
    (capture/'bytes.bin').write_bytes(b''.join(blobs))
    offset=0; index=[]
    for number,blob in enumerate(blobs):
        index.append({'id':number,'parent':None,'offset':offset,'append_bytes':len(blob),'total_bytes':len(blob)})
        offset+=len(blob)
    (capture/'snapshots.jsonl').write_text(''.join(json.dumps(row)+'\n' for row in index))
    trace={'schema':1,'complete':complete,'error':None,'directory':str(capture),'source':str(root/'events.jsonl'),'archive':'bytes.bin','index':'snapshots.jsonl','reads':[{'snapshot_id':number,'count':1} for number in range(count)]}
    print(json.dumps({'turn':int(line),'decision':{'turn':int(line),'frame':frame,'marker':marker},'orders':[],'input_capture':trace}),flush=True)
"""
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source"
            source.mkdir()
            blobs = [b"snapshot\n"] * 2 if expected_blobs is None else expected_blobs
            (source / "bytes.bin").write_bytes(b"".join(blobs))
            offset, index = 0, []
            for number, blob in enumerate(blobs):
                index.append({"id": number, "parent": None, "offset": offset,
                              "append_bytes": len(blob), "total_bytes": len(blob)})
                offset += len(blob)
            (source / "snapshots.jsonl").write_text("".join(json.dumps(row) + "\n" for row in index))
            capture = {"schema": 1, "complete": True, "error": None, "directory": str(source),
                       "source": str(root / "original-events"), "archive": "bytes.bin",
                       "index": "snapshots.jsonl", "reads": [{"snapshot_id": n, "count": 1} for n in range(len(blobs))]}
            record = {"turn": 1, "decision": {"turn": 1, "frame": 0, "marker": "same"},
                      "orders": [], "input_capture": capture}
            def command(context, output):
                return ([sys.executable, "-c", program, str(output), str(actual_count),
                         str(frame), marker, str(complete).lower()], str(root))
            kwargs = {} if delivery is None else {"delivery": delivery}
            processes = []
            original_popen = subprocess.Popen
            def launch(*args, **options):
                process = original_popen(*args, **options)
                processes.append(process)
                return process
            with mock.patch.object(replay, "replay_command", side_effect=command), \
                    mock.patch.object(replay.subprocess, "Popen", side_effect=launch):
                result = replay.run_control({}, [record], {str(source): source}, root / "run", 3, **kwargs)
            self.assertEqual(len(processes), 1)
            self.assertTrue(processes[0].stdin.closed)
            self.assertTrue(processes[0].stdout.closed)
            return result

    def test_hybrid_identical_actual_reads_can_qualify(self):
        result = self.probe()
        self.assertTrue(result["passed"], result)
        self.assertEqual(result["requests"][0]["input_delivery"], "regular")
        self.assertEqual(result["requests"][0]["replayed_reads"], 2)

    def test_variable_tail_actual_capture_can_qualify(self):
        result = self.probe(actual_count=3, delivery="hybrid-tail", expected_blobs=[b"first", b"tail", b"tail"])
        self.assertTrue(result["passed"], result)
        self.assertEqual(result["requests"][0]["input_delivery"], "fifo_prefix_regular_tail")

    def test_variable_tail_missing_and_extra_captured_reads_fail(self):
        for count in (2, 4):
            with self.subTest(count=count):
                result = self.probe(actual_count=count, delivery="hybrid-tail", expected_blobs=[b"first", b"tail", b"tail"])
                self.assertFalse(result["passed"])
                request = result["requests"][0]
                self.assertTrue(request["native_payload_matches"])
                self.assertEqual(request["identity_errors"], [])
                self.assertFalse(request["read_sequence_matches"])
                self.assertEqual(request["replayed_reads"], count)

    def test_variable_tail_still_checks_identity_and_full_payload(self):
        for change in ({"frame": 1}, {"marker": "changed"}):
            with self.subTest(change=change):
                result = self.probe(delivery="hybrid-tail", expected_blobs=[b"first", b"tail"], **change)
                self.assertFalse(result["passed"])
                self.assertTrue(result["requests"][0]["read_sequence_matches"])

    def test_fifo_remains_the_run_control_default(self):
        result = self.probe(delivery=None)
        self.assertTrue(result["passed"], result)
        self.assertEqual(result["requests"][0]["input_delivery"], "fifo")

    def test_fewer_and_extra_actual_opens_cannot_qualify(self):
        for count in (1, 3):
            with self.subTest(count=count):
                result = self.probe(actual_count=count)
                self.assertFalse(result["passed"])
                request = result["requests"][0]
                self.assertTrue(request["native_payload_matches"])
                self.assertEqual(request["identity_errors"], [])
                self.assertFalse(request["read_sequence_matches"])
                self.assertEqual(request["replayed_reads"], count)

    def test_frame_and_complete_payload_checks_are_still_required(self):
        wrong_frame = self.probe(frame=1)
        self.assertFalse(wrong_frame["passed"])
        self.assertTrue(wrong_frame["requests"][0]["identity_errors"])
        self.assertTrue(wrong_frame["requests"][0]["read_sequence_matches"])
        wrong_metadata = self.probe(marker="changed")
        self.assertFalse(wrong_metadata["passed"])
        self.assertEqual(wrong_metadata["requests"][0]["identity_errors"], [])
        self.assertTrue(wrong_metadata["requests"][0]["read_sequence_matches"])
        self.assertFalse(wrong_metadata["requests"][0]["native_payload_matches"])

    def test_an_incomplete_actual_capture_cannot_qualify(self):
        result = self.probe(complete=False)
        self.assertFalse(result["passed"])
        self.assertIn("complete", result["error"])

if __name__ == "__main__":
    unittest.main()
