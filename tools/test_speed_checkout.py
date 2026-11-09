"""Exercise the paired-cost workflow's actual fetch commands in local Git repos."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


WORKFLOW = Path(__file__).resolve().parents[1] / ".github/workflows/speed.yml"


def baseline_step():
    text = WORKFLOW.read_text()
    step = text.split("      - name: Fetch pinned paired-cost baseline\n", 1)[1]
    step = step.split("\n      - ", 1)[0]
    script = step.split("        run: |\n", 1)[1]
    return "\n".join(line[10:] for line in script.splitlines() if line.startswith("          "))


class SpeedCheckoutTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.full = self.root / "full"
        self.full.mkdir()
        self.git(self.full, "init", "-b", "main")
        self.git(self.full, "config", "user.name", "Fixture")
        self.git(self.full, "config", "user.email", "fixture@example.invalid")
        (self.full / "historical.bin").write_bytes(b"retired history\n" * 4096)
        self.commit("historical content")
        self.historical = self.git(self.full, "rev-parse", "HEAD")
        (self.full / "historical.bin").unlink()
        (self.full / "src").mkdir()
        (self.full / "src/main.rs").write_text("baseline source\n")
        self.commit("baseline")
        self.base = self.git(self.full, "rev-parse", "HEAD")
        # The event's pinned base can precede the current merge parent.
        (self.full / "new-base-file").write_text("main advanced\n")
        self.commit("newer main")
        self.git(self.full, "checkout", "-b", "feature", self.base)
        (self.full / "src/main.rs").write_text("candidate source\n")
        self.commit("candidate")
        self.git(self.full, "checkout", "main")
        self.git(self.full, "merge", "--no-ff", "feature", "-m", "candidate merge")
        self.candidate = self.git(self.full, "rev-parse", "HEAD")
        self.remote = self.root / "remote.git"
        self.git(self.root, "clone", "--bare", str(self.full), str(self.remote))
        self.limited = self.root / "limited"
        # file:// is required: a plain local clone ignores --depth.
        self.git(self.root, "clone", "--depth=1", "--no-tags", "--single-branch",
                 self.remote.as_uri(), str(self.limited))
        self.git(self.limited, "checkout", "--detach", self.candidate)

    def git(self, cwd, *args, check=True):
        return subprocess.run(["git", *args], cwd=cwd, check=check,
                              text=True, capture_output=True).stdout.strip()

    def commit(self, message):
        self.git(self.full, "add", "--all")
        self.git(self.full, "commit", "-m", message)

    def fetch(self, base):
        environment = dict(os.environ, CIVVIS_BASE_SHA=base)
        return subprocess.run(["bash", "-euo", "pipefail", "-c", baseline_step()],
                              cwd=self.limited, env=environment,
                              text=True, capture_output=True)

    def test_workflow_pins_both_trees_and_scopes_the_same_build_inputs(self):
        text = WORKFLOW.read_text()
        checkout = text.split("      - uses: actions/checkout@v4", 1)[1].split("\n      - ", 1)[0]
        self.assertIn("ref: ${{ github.sha }}", checkout)
        self.assertIn("fetch-depth: 1", checkout)
        self.assertNotIn("fetch-depth: 0", checkout)
        self.assertIn('git diff --name-only "$CIVVIS_BASE_SHA" HEAD', text)
        self.assertIn('git checkout --detach "$CIVVIS_BASE_SHA"', text)

    def test_shallow_fetch_preserves_candidate_baseline_and_return_checkout(self):
        self.assertEqual(self.git(self.limited, "rev-parse", "--is-shallow-repository"), "true")
        missing = subprocess.run(["git", "cat-file", "-e", f"{self.base}^{{commit}}"],
                                 cwd=self.limited, capture_output=True)
        self.assertNotEqual(missing.returncode, 0, "baseline must begin outside the shallow clone")
        result = self.fetch(self.base)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git(self.limited, "rev-parse", "HEAD"), self.candidate)
        for revision in (self.base, self.candidate):
            self.assertEqual(self.git(self.limited, "rev-parse", f"{revision}^{{tree}}"),
                             self.git(self.full, "rev-parse", f"{revision}^{{tree}}"))
        # The direct tree diff retains changes from the intervening main
        # commit, matching the old full-history merge-base query exactly.
        self.assertEqual(self.git(self.limited, "diff", "--name-only", self.base, "HEAD"),
                         self.git(self.full, "diff", "--name-only", f"{self.base}...{self.candidate}"))
        self.git(self.limited, "checkout", "--detach", self.base)
        self.assertEqual((self.limited / "src/main.rs").read_text(), "baseline source\n")
        self.git(self.limited, "checkout", "--force", "-")
        self.assertEqual(self.git(self.limited, "rev-parse", "HEAD"), self.candidate)
        self.assertEqual((self.limited / "src/main.rs").read_text(), "candidate source\n")
        historical = subprocess.run(["git", "cat-file", "-e", f"{self.historical}^{{commit}}"],
                                    cwd=self.limited, capture_output=True)
        self.assertNotEqual(historical.returncode, 0, "the retired history must remain unfetched")

    def test_unavailable_baseline_fails_without_changing_the_candidate(self):
        result = self.fetch("f" * 40)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(self.git(self.limited, "rev-parse", "HEAD"), self.candidate)


if __name__ == "__main__":
    unittest.main()
