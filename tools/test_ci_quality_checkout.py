"""Exercise the quality gate against a real filtered Git clone."""

from pathlib import Path
import subprocess
import tempfile
import unittest

import rust_quality as quality


class FilteredQualityCheckoutTests(unittest.TestCase):
    def git(self, repo, *args):
        return subprocess.check_output(
            ["git", "-C", str(repo), *args], text=True, stderr=subprocess.PIPE
        ).strip()

    def test_full_ancestry_and_quality_scope_survive_missing_historical_blobs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            source = root / "source"
            source.mkdir()
            self.git(source, "init", "-b", "main")
            self.git(source, "config", "user.name", "Quality fixture")
            self.git(source, "config", "user.email", "fixture@example.invalid")
            self.git(source, "config", "uploadpack.allowFilter", "true")
            (source / "src").mkdir()
            (source / "src/main.rs").write_text("fn main() { println!(\"old\"); }\n")
            (source / "retired-capture.bin").write_bytes(bytes(range(256)) * 256)
            self.git(source, "add", ".")
            self.git(source, "commit", "-m", "Original source and retired capture")
            retired = self.git(source, "rev-parse", "HEAD:retired-capture.bin")
            old_rust = self.git(source, "rev-parse", "HEAD:src/main.rs")
            self.git(source, "rm", "retired-capture.bin")
            self.git(source, "commit", "-m", "Retire the capture")
            base = self.git(source, "rev-parse", "HEAD")
            self.git(source, "checkout", "-b", "feature")
            (source / "src/main.rs").write_text("fn main() { println!(\"new\"); }\n")
            self.git(source, "commit", "-am", "Own Rust change")
            feature = self.git(source, "rev-parse", "HEAD")
            self.git(source, "checkout", "main")
            (source / "src/unrelated.rs").write_text("pub fn unrelated() {}\n")
            self.git(source, "add", "src/unrelated.rs")
            self.git(source, "commit", "-m", "Advance trunk independently")
            trunk = self.git(source, "rev-parse", "HEAD")
            self.git(source, "checkout", "-b", "synthetic")
            self.git(source, "merge", "--no-ff", "feature", "-m", "PR test merge")
            synthetic = self.git(source, "rev-parse", "HEAD")
            self.git(source, "checkout", "feature")
            self.git(source, "merge", "--no-ff", "main", "-m", "Local trunk merge")
            local_merge = self.git(source, "rev-parse", "HEAD")
            self.git(source, "checkout", "main")
            clones = []
            for name, options in [("full", []), ("filtered", ["--filter=blob:none"])]:
                clone = root / name
                subprocess.run(
                    ["git", "clone", "--no-checkout", *options, source.as_uri(), str(clone)],
                    check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                )
                clones.append(clone)
            filtered = clones[1]
            self.assertEqual(self.git(filtered, "rev-parse", "--is-shallow-repository"), "false")
            self.assertEqual(self.git(filtered, "config", "remote.origin.partialclonefilter"), "blob:none")
            missing = self.git(filtered, "rev-list", "--objects", "--all", "--missing=print")
            self.assertIn("?" + retired, missing.splitlines())
            # Stale PR base, ordinary push, and reversed local merge parents.
            for head, expected_base in [(synthetic, trunk), (feature, base), (local_merge, trunk)]:
                with self.subTest(head=head):
                    observed = []
                    for clone in clones:
                        self.git(clone, "checkout", "--detach", head)
                        if clone == filtered and head == synthetic:
                            missing = self.git(clone, "rev-list", "--objects", "--all", "--missing=print")
                            self.assertIn("?" + old_rust, missing.splitlines())
                        resolved = quality.revision(clone, base, "HEAD^")
                        actual_base = quality.merge_base(clone, resolved, head)
                        self.assertEqual(actual_base, expected_base)
                        observed.append((
                            quality.changed_rust_files(clone, actual_base, head),
                            {str(p.relative_to(clone)): ranges for p, ranges in
                             quality.changed_line_ranges(clone, actual_base, head).items()},
                        ))
                    self.assertEqual(observed[0], observed[1])
                    self.assertEqual(observed[1][0], [Path("src/main.rs")])
                    self.assertTrue(observed[1][1])
            # Quality diffs fetched the old Rust source, not an unrelated archive.
            missing = self.git(filtered, "rev-list", "--objects", "--all", "--missing=print")
            self.assertIn("?" + retired, missing.splitlines())
            self.assertNotIn("?" + old_rust, missing.splitlines())
            # An unavailable historical Rust blob must fail, never turn the
            # quality scope into an empty successful diff.
            unavailable = root / "unavailable"
            subprocess.run(
                ["git", "clone", "--no-checkout", "--filter=blob:none",
                 source.as_uri(), str(unavailable)],
                check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            self.git(unavailable, "checkout", "--detach", synthetic)
            self.git(unavailable, "remote", "set-url", "origin", (root / "missing").as_uri())
            with self.assertRaises(RuntimeError):
                quality.changed_line_ranges(unavailable, trunk, synthetic)

    def test_workflow_retains_full_history_and_runs_the_same_quality_gate(self):
        workflow = (Path(__file__).resolve().parents[1] / ".github/workflows/quality.yml").read_text()
        self.assertIn("filter: blob:none", workflow)
        self.assertIn("fetch-depth: 0", workflow)
        self.assertIn("run: python3 tools/rust_quality.py", workflow)


if __name__ == "__main__":
    unittest.main()
