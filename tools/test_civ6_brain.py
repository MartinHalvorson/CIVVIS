#!/usr/bin/env python3
"""The decider protocol had no test, and that is how a println cost a whole run.

`--serve` is one line in, one line out. `Decider.ask` used to accept ANY JSON
object and turn one without an `orders` key into an empty order list — so a
single stray line on the decider's stdout shifted every turn by one and read as
"CIVVIS chose nothing". The run kept going, reported `orders_source: "fallback"`,
and the hand-written ladder played the game.

That is not hypothetical: the genome report was printed to stdout, and a live run
that had been 236 turns of CIVVIS flipped to fallback the moment a binary
carrying it was swapped in. `why.log` showed the decider founding its capital on
the very turn the brain recorded zero orders.
"""

from __future__ import annotations

import io
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_brain  # noqa: E402


class FakeProc:
    """Just enough of `subprocess.Popen` for the response loop.

    Records what was written so a test can prove the turn was actually asked for,
    and serves canned stdout lines in order.
    """

    def __init__(self, lines: list[str]) -> None:
        self.stdout = io.StringIO("".join(lines))
        self.stdin = io.StringIO()
        self.asked: list[str] = []

    def poll(self):
        return None

    def write(self, text: str) -> None:  # pragma: no cover - stdin shim
        self.asked.append(text)


class _Decider(civ6_brain.Decider):
    """A `Decider` wired to a canned process, never spawning anything."""

    def __init__(self, lines: list[str]) -> None:
        self.proc = FakeProc(lines)
        self.binary = Path("/nonexistent")
        self.run_dir = Path("/nonexistent")
        self.victory = "domination"

    def start(self) -> None:  # pragma: no cover - must never be reached
        raise AssertionError("the canned process must not be replaced")


class DeciderProtocol(unittest.TestCase):
    def test_a_plain_response_is_read(self) -> None:
        decider = _Decider(
            ['{"turn":1,"orders":[{"kind":"unit","subject":7,"verb":"MOVE_TO",'
             '"x":3,"y":4}],"note":"ok"}\n']
        )
        rows, note = decider.ask(1)
        self.assertEqual(rows, [("unit", 7, "MOVE_TO", 3, 4)])
        self.assertEqual(note, "ok")

    def test_a_line_that_is_not_a_response_is_skipped_not_read_as_empty(self) -> None:
        """⚠ The regression. The first line is valid JSON with no `orders` key.

        Read as a response it means "CIVVIS chose nothing", which is
        indistinguishable from a genuine empty turn and hands the game to the
        ladder. It must be stepped over so the real response behind it is used.
        """
        decider = _Decider(
            [
                # exactly the shape that broke it: the genome report
                '{"kind":"genome","strategy":"stock","victory":"domination"}\n',
                '{"turn":1,"orders":[{"kind":"unit","subject":7,"verb":"MOVE_TO",'
                '"x":3,"y":4}],"note":"real"}\n',
            ]
        )
        rows, note = decider.ask(1)
        self.assertEqual(
            rows,
            [("unit", 7, "MOVE_TO", 3, 4)],
            "the response behind the stray line is the answer",
        )
        self.assertEqual(note, "real")

    def test_a_genuinely_empty_turn_is_still_empty(self) -> None:
        """The skip must not paper over a real "nothing to do" answer.

        A response WITH `orders` and an empty list is CIVVIS saying so, and it has
        to stay distinguishable from the stray-line case above.
        """
        decider = _Decider(['{"turn":1,"orders":[],"note":"nothing to do"}\n'])
        rows, note = decider.ask(1)
        self.assertEqual(rows, [])
        self.assertEqual(note, "nothing to do")


if __name__ == "__main__":
    unittest.main()


class SeatCivTest(unittest.TestCase):
    """The civ Civilization VI dealt must reach the decider, or `--strategy auto`
    answers only half the brief and reports `per_civ:false`."""

    def _run(self, *lines: str) -> Path:
        run = Path(tempfile.mkdtemp())
        (run / "events.jsonl").write_text("\n".join(lines))
        return run

    def test_the_dealt_civ_is_read_and_stripped_to_the_league_name(self) -> None:
        run = self._run(
            '{"kind":"tiles","turn":1}',
            '{"kind":"seat","civ":"CIVILIZATION_ROME","leader":"LEADER_JULIUS_CAESAR"}',
        )
        self.assertEqual(civ6_brain.seat_civ(run), "Rome")

    def test_a_run_with_no_seat_event_yet_is_none_not_a_guess(self) -> None:
        """⚠ None, never a default. A wrong civ would narrow the league to a table
        that does not describe this game; no civ correctly falls back to the
        overall pick."""
        self.assertIsNone(civ6_brain.seat_civ(self._run('{"kind":"tiles","turn":1}')))

    def test_a_missing_run_directory_does_not_raise(self) -> None:
        """The decider starts lazily and this runs on the way in; an exception here
        would take the whole turn down over a naming detail."""
        self.assertIsNone(civ6_brain.seat_civ(Path("/nonexistent-run-dir")))

    def test_an_unprefixed_civ_is_passed_through(self) -> None:
        run = self._run('{"kind":"seat","civ":"Rome"}')
        self.assertEqual(civ6_brain.seat_civ(run), "Rome")
