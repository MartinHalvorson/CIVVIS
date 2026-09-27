"""Recovery inputs survive the next game's autosave rotation as complete files."""

import hashlib
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest import mock

import civ6_save_snapshot as saves


class SaveSnapshotTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self.tmp.cleanup)
        self.source = Path(self.tmp.name) / "AutoSave_0159.Civ6Save"
        self.source.write_bytes(b"binary save\x00\xff")
        self.destination = Path(self.tmp.name) / "run" / "native-recovery-save"

    def capture(self, **kwargs):
        return saves.snapshot(self.source, self.destination, root_tag="root",
                              frozen_tag="root-cont2", continuation_tag="root-cont3",
                              last_turn=160, **kwargs)

    def test_complete_binary_and_provenance_survive_source_deletion(self):
        data = bytes(range(256)) * 8193  # Crosses multiple copy chunks.
        self.source.write_bytes(data)
        info = self.source.stat()
        manifest = self.capture()
        self.source.unlink()  # A new game can now clear the rolling saves.
        report = json.loads(manifest.read_text())
        self.assertEqual((self.destination / report["archive"]).read_bytes(), data)
        self.assertEqual(report["bytes"], len(data))
        self.assertEqual(report["sha256"], hashlib.sha256(data).hexdigest())
        self.assertEqual(report["source"], str(self.source.absolute()))
        self.assertEqual(report["source_mtime_ns"], info.st_mtime_ns)
        self.assertEqual(report["root_tag"], "root")
        self.assertEqual(report["frozen_tag"], "root-cont2")
        self.assertEqual(report["continuation_tag"], "root-cont3")
        self.assertEqual(report["last_observed_turn"], 160,
                         "the save counter 159 is not inferred to be the game turn")

    def test_existing_snapshot_cannot_be_replaced_by_another_input(self):
        manifest = self.capture()
        original = manifest.read_bytes()
        original_save = (self.destination / self.source.name).read_bytes()
        self.source.write_bytes(b"replacement game")
        with self.assertRaises(FileExistsError):
            self.capture()
        self.assertEqual(manifest.read_bytes(), original)
        self.assertEqual((self.destination / self.source.name).read_bytes(), original_save)

    def test_incomplete_existing_destination_is_not_claimed_as_success(self):
        self.destination.mkdir(parents=True)
        (self.destination / "earlier-evidence").write_bytes(b"preserve")
        with self.assertRaises(FileExistsError):
            self.capture()
        self.assertEqual((self.destination / "earlier-evidence").read_bytes(), b"preserve")

    def test_empty_and_oversized_inputs_are_rejected_before_reading(self):
        for data in (b"", b"12345"):
            with self.subTest(data=data):
                self.source.write_bytes(data)
                with mock.patch.object(saves.os, "open") as opened:
                    with self.assertRaisesRegex(OSError, "save size"):
                        self.capture(file_limit=4)
                opened.assert_not_called()
                self.assertFalse(self.destination.exists())

    def test_symlinks_directories_and_missing_saves_are_rejected(self):
        self.source.unlink()
        with self.assertRaises(OSError):
            self.capture()
        self.source.mkdir()
        with self.assertRaisesRegex(OSError, "not a regular"):
            self.capture()
        self.source.rmdir()
        other = Path(self.tmp.name) / "other"
        other.write_bytes(b"not the selected save")
        self.source.symlink_to(other)
        with self.assertRaisesRegex(OSError, "not a regular"):
            self.capture()
        self.assertFalse(self.destination.exists())

    def mutate_on_copy(self, mutate):
        original_open = Path.open

        def opening(path, *args, **kwargs):
            stream = original_open(path, *args, **kwargs)
            if path == self.destination / self.source.name and args == ("xb",):
                mutate()
            return stream

        with mock.patch.object(Path, "open", opening):
            with self.assertRaises(OSError):
                self.capture()
        self.assertFalse(self.destination.exists(), "failed copies have no manifest or save")

    def test_growth_and_truncation_never_leave_partial_replay_inputs(self):
        self.mutate_on_copy(lambda: self.source.write_bytes(b"longer than original save"))
        self.source.write_bytes(b"binary save\x00\xff")
        self.mutate_on_copy(lambda: self.source.write_bytes(b"short"))

    def test_same_size_rewrite_with_restored_mtime_is_detected(self):
        before = self.source.stat()

        def rewrite():
            self.source.write_bytes(b"different!!\x00\xff")
            os.utime(self.source, ns=(before.st_atime_ns, before.st_mtime_ns))

        self.mutate_on_copy(rewrite)

    def test_replacement_inode_is_detected_even_when_open_bytes_stay_stable(self):
        replacement = Path(self.tmp.name) / "replacement"
        replacement.write_bytes(self.source.read_bytes())
        self.mutate_on_copy(lambda: os.replace(replacement, self.source))

    def test_failed_save_or_manifest_flush_removes_incomplete_evidence(self):
        for failure in (1, 2):
            with self.subTest(failure=failure):
                calls = 0

                def sync(_):
                    nonlocal calls
                    calls += 1
                    if calls == failure:
                        raise OSError("disk full")

                with mock.patch.object(saves.os, "fsync", sync):
                    with self.assertRaisesRegex(OSError, "disk full"):
                        self.capture()
                self.assertFalse(self.destination.exists())

    def test_checksum_readback_rejects_corrupted_archive(self):
        original_open = Path.open

        def opening(path, *args, **kwargs):
            if path == self.destination / self.source.name and args == ("rb",):
                path.write_bytes(b"corrupted write")
            return original_open(path, *args, **kwargs)

        with mock.patch.object(Path, "open", opening):
            with self.assertRaisesRegex(OSError, "checksum readback"):
                self.capture()
        self.assertFalse(self.destination.exists())


if __name__ == "__main__":
    unittest.main()
