"""The supervisor's pause between cycles (`# >>> boundary pause` block).

After a cycle that played turns, the loop waits only for Civ VI to leave the
process table (bounded at 10 s) plus 2 s for Steam; a cycle that played
nothing keeps the flat 10 s. The block is run under zsh with `pgrep` and
`sleep` stubbed, so the test measures what the loop would wait.
"""

from __future__ import annotations

import shutil
import subprocess
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
