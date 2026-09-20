"""Saved victory overrides must be visible before starting verification games."""

import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SOURCE = Path(__file__).parent / "ops" / "civvis-games.sh"


@unittest.skipUnless(shutil.which("zsh"), "requires zsh")
class VictoryStatusTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="victory status ")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        (self.root / "tools").mkdir()
        (self.root / "tools/civ6_play.py").write_text(
            'DEFAULT_CIVVIS_VICTORY = "science"\n'
            'VICTORY_LANES = ["science", "culture", "domination", "futurelane"]\n'
            'raise RuntimeError("status must never import the game controller")\n'
        )
        self.policy = self.root / "policy"
        self.lane = self.root / "lane"
        self.script = self.root / "report.zsh"
        self.script.write_text(
            SOURCE.read_text().split("case ${1:-status} in", 1)[0]
            + '\nreport_tree=$1\nplay_tree() { print -r -- "$report_tree"; }\n'
            + 'victory_report\n'
        )

    def report(self):
        result = subprocess.run(
            [shutil.which("zsh"), str(self.script), str(self.root)],
            env=dict(os.environ, CIVVIS_VERIFICATION_POLICY=str(self.policy),
                     CIVVIS_VICTORY_LANE_FILE=str(self.lane)),
            capture_output=True, text=True, timeout=10,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        return result.stdout

    def test_conflicting_standing_file_is_reported(self):
        self.policy.write_text("CIVVIS_VICTORY=domination\n")
        self.lane.write_text("culture\n")
        result = self.report()
        self.assertIn("requested  domination", result)
        self.assertIn("OVERRIDE   culture replaces domination", result)
        self.assertIn(str(self.lane), result)

    def test_missing_files_show_source_default(self):
        result = self.report()
        self.assertIn("requested  science (tree default)", result)
        self.assertIn("override   none", result)

    def test_policy_comments_whitespace_and_last_value_match_launcher(self):
        self.policy.write_text(
            "CIVVIS_VICTORY=culture\n  CIVVIS_VICTORY = domination # desired\n"
        )
        self.lane.write_text("domination\n")
        result = self.report()
        self.assertIn("requested  domination", result)
        self.assertIn("same as requested", result)

    def test_invalid_override_is_not_claimed_as_effective(self):
        self.lane.write_text("badlane\n")
        self.assertIn("'badlane' is invalid; climber ignores it", self.report())

    def test_supported_lanes_come_from_controller_source(self):
        self.lane.write_text("futurelane\n")
        self.assertIn("OVERRIDE   futurelane replaces science", self.report())

    def test_blank_override_is_absent(self):
        self.lane.write_text(" \n\t")
        self.assertIn("override   none", self.report())

    def test_unreadable_override_is_reported(self):
        self.lane.mkdir()
        self.assertIn("override   unreadable; climber ignores it", self.report())

    def test_policy_text_is_never_executed(self):
        marker = self.root / "must-not-exist"
        self.policy.write_text(f"CIVVIS_VICTORY=$(touch {marker})\n")
        self.report()
        self.assertFalse(marker.exists())

    def test_on_and_status_both_report_saved_victory(self):
        source = SOURCE.read_text()
        branches = source.split("case ${1:-status} in", 1)[1]
        on = branches.split("on)", 1)[1].split(";;", 1)[0]
        status = branches.split("status)", 1)[1].split(";;", 1)[0]
        self.assertIn("victory_report", status)
        self.assertLess(on.index("victory_report"), on.index("relaunch_ladder"))


if __name__ == "__main__":
    unittest.main()
