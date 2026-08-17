import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


MODULE_PATH = Path(__file__).with_name("rust_quality.py")
SPEC = importlib.util.spec_from_file_location("rust_quality", MODULE_PATH)
assert SPEC and SPEC.loader
quality = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(quality)


class QualityHelpersTests(unittest.TestCase):
    def test_changed_rust_files_ignores_deleted_and_non_rust_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            (repo / "src").mkdir()
            (repo / "src/main.rs").write_text("fn main() {}\n")
            (repo / "README.md").write_text("docs\n")
            completed = quality.subprocess.CompletedProcess(
                [], 0, "src/main.rs\nREADME.md\nold.rs\n", ""
            )
            with patch.object(quality, "run", return_value=completed):
                self.assertEqual(
                    quality.changed_rust_files(repo, "base", "head"),
                    [Path("src/main.rs")],
                )

    def test_span_paths_normalizes_relative_and_absolute_diagnostics(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            message = {
                "spans": [
                    {"file_name": "src/lib.rs"},
                    {"file_name": str(repo / "src/main.rs")},
                ]
            }
            self.assertEqual(
                quality._span_paths(message, repo),
                {(repo / "src/lib.rs").resolve(), (repo / "src/main.rs").resolve()},
            )

    def test_clippy_only_reports_warnings_in_changed_files(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory)
            warning = {
                "reason": "compiler-message",
                "message": {
                    "level": "warning",
                    "code": {"code": "clippy::needless_return"},
                    "message": "remove the return",
                    "rendered": "warning: remove the return",
                    "spans": [{"file_name": "src/lib.rs"}],
                },
            }
            unrelated = {
                **warning,
                "message": {
                    **warning["message"],
                    "spans": [{"file_name": "src/other.rs"}],
                },
            }
            completed = quality.subprocess.CompletedProcess(
                [],
                0,
                "\n".join([json.dumps(warning), json.dumps(unrelated)]),
                "",
            )
            with patch.object(quality, "run", return_value=completed):
                diagnostics, raw = quality.clippy(repo, [Path("src/lib.rs")])
            self.assertEqual(len(diagnostics), 1)
            self.assertIn("needless_return", diagnostics[0])
            self.assertEqual(raw, "")


if __name__ == "__main__":
    unittest.main()
