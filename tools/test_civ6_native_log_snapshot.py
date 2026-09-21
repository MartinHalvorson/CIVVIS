"""Native evidence survives a relaunch without unbounded reads or overwrite."""

import hashlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import civ6_native_log_snapshot as logs


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.source = Path(self.tmp.name) / "logs"
        self.source.mkdir()
        self.destination = Path(self.tmp.name) / "run" / "native-freeze-logs"

    def capture(self, **kwargs):
        return json.loads(logs.snapshot(self.source, self.destination, **kwargs).read_text())

    def test_bytes_hashes_and_existing_evidence_survive_source_replacement(self):
        original = b"136, Babylon, waiting\x00\xff"
        (self.source / "AI.csv").write_bytes(original)
        report = self.capture()
        self.assertEqual(report["files"][0]["sha256"], hashlib.sha256(original).hexdigest())
        self.assertFalse(report["files"][0]["changed_during_read"])
        (self.source / "AI.csv").write_bytes(b"new game")
        self.assertEqual(self.capture(), report)
        self.assertEqual((self.destination / "AI.csv").read_bytes(), original)

    def test_limits_skip_large_files_and_keep_later_small_ones(self):
        for name, data in (("a.log", b"12345"), ("b.csv", b"123"),
                           ("c.txt", b"123"), ("d.log", b"1")):
            (self.source / name).write_bytes(data)
        report = self.capture(file_limit=4, total_limit=4)
        self.assertEqual(report["copied_bytes"], 4)
        self.assertEqual([e["name"] for e in report["files"] if "sha256" in e],
                         ["b.csv", "d.log"])
        self.assertFalse((self.destination / "a.log").exists())

    def test_symlinks_directories_and_reserved_manifest_are_not_copied(self):
        (self.source / "nested").mkdir()
        (self.source / "link").symlink_to(self.source / "nested", target_is_directory=True)
        (self.source / "manifest.json").write_text("native file")
        report = self.capture()
        self.assertTrue(all("skipped" in e for e in report["files"]))
        self.assertEqual(report["copied_bytes"], 0)

    def test_file_read_failure_does_not_discard_other_logs(self):
        (self.source / "a.log").write_text("unreadable")
        (self.source / "b.log").write_text("evidence")
        original = Path.open

        def read(path, *args, **kwargs):
            if path == self.source / "a.log":
                raise OSError("read failed")
            return original(path, *args, **kwargs)

        with mock.patch.object(Path, "open", read):
            report = self.capture()
        self.assertIn("error", report["files"][0])
        self.assertEqual((self.destination / "b.log").read_text(), "evidence")

    def test_missing_source_is_an_explicit_failure(self):
        self.source.rmdir()
        with self.assertRaises(OSError):
            self.capture()
        self.assertFalse(self.destination.exists())

    def test_growth_during_read_is_bounded_and_reported(self):
        path = self.source / "growing.log"
        path.write_bytes(b"ok")
        original = Path.open

        def read(candidate, *args, **kwargs):
            if candidate == path and args == ("rb",):
                return io.BytesIO(b"grew beyond the limit")
            return original(candidate, *args, **kwargs)

        with mock.patch.object(Path, "open", read):
            report = self.capture(file_limit=4, total_limit=4)
        self.assertEqual(report["files"][0]["skipped"], "grew_past_byte_limit")
        self.assertFalse((self.destination / path.name).exists())

    def test_failed_write_leaves_no_partial_evidence(self):
        (self.source / "a.log").write_bytes(b"evidence")
        original = Path.write_bytes

        def write(path, data):
            if path == self.destination / "a.log":
                original(path, data[:2])
                raise OSError("disk full")
            return original(path, data)

        with mock.patch.object(Path, "write_bytes", write):
            report = self.capture()
        self.assertIn("disk full", report["files"][0]["error"])
        self.assertFalse((self.destination / "a.log").exists())
        self.assertEqual(report["copied_bytes"], 0)

    def test_incomplete_existing_snapshot_is_not_reported_as_success(self):
        self.destination.mkdir(parents=True)
        with self.assertRaisesRegex(OSError, "incomplete"):
            self.capture()


if __name__ == "__main__":
    unittest.main()
