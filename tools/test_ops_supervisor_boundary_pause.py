"""The supervisor's pause between cycles (`# >>> boundary pause` block).

After a cycle that played turns, the loop waits only for Civ VI to leave the
process table (bounded at 10 s) plus 2 s for Steam; a cycle that played
nothing keeps the flat 10 s. The block is run under zsh with `pgrep` and
`sleep` stubbed, so the test measures what the loop would wait.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

SUPERVISOR = Path(__file__).resolve().parent / "ops" / "civvis-game-supervisor.sh"


def _block() -> str:
    source = SUPERVISOR.read_text(encoding="utf-8")
    start = source.index("  # >>> boundary pause\n")
    end = source.index("  # <<< boundary pause\n")
    return source[start:end]


@unittest.skipUnless(shutil.which("zsh"), "requires the production zsh shell")
class BoundaryPauseTest(unittest.TestCase):
    def _count_played(self, current: list[str], old: list[str] | None = None) -> int:
        """Run the production counter against real files and cycle mtimes."""
        source = SUPERVISOR.read_text(encoding="utf-8")
        start = source.index("  played_games=0\n")
        end = source.index("  if (( played_games > 0 )); then", start)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "cycle"
            marker.touch()
            os.utime(marker, (100, 100))
            for age, logs in [(99, old or []), (101, current)]:
                for number, text in enumerate(logs):
                    path = root / f"civvis-{age}-{number}-play.log"
                    path.write_text(text, encoding="utf-8")
                    os.utime(path, (age, age))
            script = ('LOGS="$1"\nCYCLE_MARK="$1/cycle"\n'
                      + source[start:end] + 'print "$played_games"\n')
            done = subprocess.run(["zsh", "-c", script, "counter", directory],
                                  capture_output=True, text=True, check=True)
            return int(done.stdout.strip())

    def test_actual_padded_progress_and_unpadded_turn_messages_count(self) -> None:
        logs = ["[turn   10] score=12 cities=1 units=1\n",
                "[turn  100] score=200 cities=4 units=12\n",
                "[turn 1000] score=900 cities=9 units=30\n",
                "[turn 3] blocked on ENDTURN_BLOCKING_UNITS\n"]
        for line in logs:
            with self.subTest(line=line):
                self.assertEqual(self._count_played([line]), 1)

    def test_only_current_cycle_logs_count_and_setup_is_not_a_turn(self) -> None:
        self.assertEqual(self._count_played(
            ["[agent] turn limit asked=500 config=500 game=500\n"],
            old=["[turn   10] score=12 cities=1 units=1\n"]), 0)

    def test_every_played_game_in_the_batch_counts_once(self) -> None:
        self.assertEqual(self._count_played([
            "[turn   10] score=12\n[turn   20] score=18\n",
            "[turn    1] score=0\n",
            "[setup] no game started\n"]), 2)

    def _run(self, played: int, civ_polls: int) -> list[str]:
        """Sleeps the block asks for, with Civ still listed for `civ_polls` reads."""
        script = (
            f"played_games={played}\n"
            f"left={civ_polls}\n"
            "pgreps=()\n"
            "pgrep() { pgreps+=(\"$*\"); (( left > 0 )) && { left=$((left - 1)); return 0; }; return 1 }\n"
            "sleep() { print \"sleep $1\" }\n"
            + _block()
            + "print \"pattern ${pgreps[1]:-none}\"\n"
        )
        done = subprocess.run(["zsh", "-c", script], capture_output=True, text=True,
                              check=True)
        return done.stdout.split("\n")[:-1]

    def test_a_played_cycle_with_civ_gone_waits_two_seconds(self) -> None:
        out = self._run(played=1, civ_polls=0)
        self.assertEqual([line for line in out if line.startswith("sleep")], ["sleep 2"])
        self.assertIn('pattern -x Civ6_Exe(_Child)?', out)

    def test_a_played_cycle_waits_for_civ_to_exit(self) -> None:
        out = self._run(played=1, civ_polls=3)
        self.assertEqual([line for line in out if line.startswith("sleep")],
                         ["sleep 1"] * 3 + ["sleep 2"])

    def test_a_civ_that_stays_up_is_waited_on_at_most_ten_seconds(self) -> None:
        # An operator-opened game is not waited out here; the next cycle's
        # outside-Civ handling owns it.
        out = self._run(played=2, civ_polls=99)
        self.assertEqual([line for line in out if line.startswith("sleep")],
                         ["sleep 1"] * 10 + ["sleep 2"])

    def test_a_cycle_that_played_nothing_keeps_the_flat_ten_seconds(self) -> None:
        out = self._run(played=0, civ_polls=0)
        self.assertEqual([line for line in out if line.startswith("sleep")], ["sleep 10"])
        self.assertIn("pattern none", out)

    def test_the_block_ends_the_loop_body(self) -> None:
        source = SUPERVISOR.read_text(encoding="utf-8")
        self.assertIn("  # <<< boundary pause\ndone\n", source)
        self.assertEqual(source.count("  # >>> boundary pause\n"), 1)


if __name__ == "__main__":
    unittest.main()
