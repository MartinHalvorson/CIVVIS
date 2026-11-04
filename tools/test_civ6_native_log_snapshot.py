"""Native evidence survives a relaunch without unbounded reads or overwrite."""

import hashlib
import io
import json
import os
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

    def test_oversized_log_keeps_tail_after_whole_logs_within_both_limits(self):
        (self.source / "AI_Behavior_Trees.csv").write_bytes(b"old-data-latest")
        (self.source / "Lua.log").write_bytes(b"ok")
        report = self.capture(file_limit=6, total_limit=8)
        self.assertEqual(report["copied_bytes"], 8)
        self.assertEqual((self.destination / "Lua.log").read_bytes(), b"ok")
        tail = (self.destination / "AI_Behavior_Trees.csv").read_bytes()
        self.assertEqual(tail, b"latest")
        entry = next(e for e in report["files"] if e["name"] == "AI_Behavior_Trees.csv")
        self.assertEqual(entry["source_bytes"], 15)
        self.assertEqual(entry["start_offset"], 9)
        self.assertTrue(entry["truncated"])
        self.assertEqual(entry["sha256"], hashlib.sha256(tail).hexdigest())
        self.assertFalse(entry["changed_during_read"])
        (self.source / "AI_Behavior_Trees.csv").write_bytes(b"replacement")
        self.assertEqual(self.capture(), report)
        self.assertEqual((self.destination / "AI_Behavior_Trees.csv").read_bytes(), tail)

    def test_total_budget_can_preserve_a_smaller_tail(self):
        (self.source / "a.log").write_bytes(b"0123456789")
        (self.source / "b.log").write_bytes(b"1234")
        report = self.capture(file_limit=6, total_limit=7)
        self.assertEqual(report["copied_bytes"], 7)
        self.assertEqual((self.destination / "a.log").read_bytes(), b"789")
        self.assertEqual(report["files"][0]["start_offset"], 7)

    def test_tail_reads_stay_bounded_and_identify_source_growth(self):
        path = self.source / "growing.log"
        original_bytes = b"old-data-latest"
        path.write_bytes(original_bytes)
        original_open = Path.open
        reads = []

        class GrowingStream(io.BytesIO):
            def read(self, size):
                reads.append(size)
                path.write_bytes(original_bytes + b"new data")
                return super().read(size)

        def read(candidate, *args, **kwargs):
            if candidate == path and args == ("rb",):
                return GrowingStream(original_bytes)
            return original_open(candidate, *args, **kwargs)

        with mock.patch.object(Path, "open", read):
            report = self.capture(file_limit=6, total_limit=6)
        self.assertEqual(reads, [6])
        self.assertEqual((self.destination / path.name).read_bytes(), b"latest")
        self.assertTrue(report["files"][0]["changed_during_read"])

    def test_deferred_tail_does_not_follow_a_replaced_symlink(self):
        large = self.source / "a.log"
        large.write_bytes(b"oversized")
        small = self.source / "b.log"
        small.write_bytes(b"ok")
        other = Path(self.tmp.name) / "other"
        other.write_bytes(b"not a log")
        original_open = Path.open

        def read(candidate, *args, **kwargs):
            if candidate == small and args == ("rb",):
                large.unlink()
                large.symlink_to(other)
            return original_open(candidate, *args, **kwargs)

        with mock.patch.object(Path, "open", read):
            report = self.capture(file_limit=4, total_limit=8)
        self.assertEqual(report["files"][0]["skipped"], "not_regular_file")
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

        for limit in (32, 4):
            with self.subTest(file_limit=limit):
                self.destination = Path(self.tmp.name) / f"failed-write-{limit}"
                with mock.patch.object(Path, "write_bytes", write):
                    report = self.capture(file_limit=limit)
                self.assertIn("disk full", report["files"][0]["error"])
                self.assertFalse((self.destination / "a.log").exists())
                self.assertEqual(report["copied_bytes"], 0)

    def test_incomplete_existing_snapshot_is_not_reported_as_success(self):
        self.destination.mkdir(parents=True)
        with self.assertRaisesRegex(OSError, "incomplete"):
            self.capture()


if __name__ == "__main__":
    unittest.main()


class CrashReportTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.reports = Path(self.tmp.name) / "DiagnosticReports"
        self.reports.mkdir()
        self.destination = Path(self.tmp.name) / "run" / "native-crash-report"

    def report(self, name, mtime, body=b"{}"):
        path = self.reports / name
        path.write_bytes(body)
        os.utime(path, (mtime, mtime))
        return path

    def test_only_this_segments_civ_reports_are_copied(self):
        self.report("Civ6_Exe_Child-2026-11-03-071230.ips", 100.0)       # earlier game
        self.report("Civ6_Exe_Child-2026-11-04-094633.ips", 500.0, b"stack")
        self.report("replayd-2026-11-04-094700.ips", 600.0)               # not Civ
        self.report("Civ6_Exe_Child-2026-11-04-094700.diag", 600.0)       # not a report
        copied = logs.copy_crash_reports(self.reports, self.destination, since=400.0)
        self.assertEqual(copied, ["Civ6_Exe_Child-2026-11-04-094633.ips"])
        self.assertEqual((self.destination / copied[0]).read_bytes(), b"stack")

    def test_an_existing_copy_is_never_replaced(self):
        self.report("Civ6_Exe_Child-a.ips", 500.0, b"new")
        self.destination.mkdir(parents=True)
        (self.destination / "Civ6_Exe_Child-a.ips").write_bytes(b"kept")
        self.assertEqual(logs.copy_crash_reports(self.reports, self.destination, since=0.0),
                         ["Civ6_Exe_Child-a.ips"])
        self.assertEqual((self.destination / "Civ6_Exe_Child-a.ips").read_bytes(), b"kept")

    def test_waits_a_bounded_time_for_a_late_report(self):
        reports = self.reports
        destination = self.destination

        class Clock:
            now = 0.0
            sleeps = 0

            def time(self):
                return self.now

            def sleep(self, seconds):
                self.sleeps += 1
                self.now += seconds
                if self.sleeps == 2:
                    path = reports / "Civ6_Exe_Child-late.ips"
                    path.write_bytes(b"late")
                    os.utime(path, (1000.0, 1000.0))

        clock = Clock()
        copied = logs.copy_crash_reports(reports, destination, since=10.0,
                                         wait_s=5.0, poll_s=0.5, clock=clock)
        self.assertEqual(copied, ["Civ6_Exe_Child-late.ips"])
        self.assertEqual(clock.sleeps, 2)

        empty = Path(self.tmp.name) / "empty"
        empty.mkdir()
        clock = Clock()
        self.assertEqual(logs.copy_crash_reports(empty, destination, since=10.0,
                                                 wait_s=2.0, poll_s=0.5, clock=clock), [])
        self.assertEqual(clock.sleeps, 4)
        self.assertFalse((Path(self.tmp.name) / "nowhere").exists())
        self.assertEqual(logs.copy_crash_reports(Path(self.tmp.name) / "nowhere",
                                                 destination, since=0.0), [])

    def test_a_failed_copy_leaves_no_partial_file(self):
        self.report("Civ6_Exe_Child-a.ips", 500.0, b"stack")
        with mock.patch.object(logs.shutil, "copyfile", side_effect=OSError("disk full")):
            self.assertEqual(logs.copy_crash_reports(self.reports, self.destination,
                                                     since=0.0), [])
        self.assertFalse((self.destination / "Civ6_Exe_Child-a.ips").exists())
