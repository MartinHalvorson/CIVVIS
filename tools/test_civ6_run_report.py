#!/usr/bin/env python3
"""Tests for the recorded-run report.

Synthetic runs throughout: CI has no `~/civvis-civ6-runs`, and a report that
only works against one operator's disk is not a tool.
"""

from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

sys.path.insert(0, str(Path(__file__).resolve().parent))

import civ6_run_report as rr  # noqa: E402


def write_run(root: Path, states, extra=(), why: str = "") -> Path:
    run = root / "civvis-20260101T000000Z"
    run.mkdir()
    lines = [json.dumps({"kind": "state", **s}) for s in states]
    lines += [json.dumps(e) for e in extra]
    (run / "events.jsonl").write_text("\n".join(lines) + "\n")
    if why:
        (run / "why.log").write_text(why)
    return run


def state(turn, score, rival=None, cities=0, techs=0):
    row = {"turn": turn, "score": score,
           "cities": [{"id": i} for i in range(cities)],
           "techs": [f"TECH_{i}" for i in range(techs)]}
    if rival is not None:
        row["rivals"] = [{"score": rival}]
    return row


class CrossoverTest(unittest.TestCase):
    """The turn the game actually turned, not the first wobble."""

    def test_an_early_dip_is_not_the_crossover(self) -> None:
        """A run that dips at t30 and leads until t112 turned at t112.

        Reporting the dip would send the reader a hundred turns away from the
        moment the game was decided.
        """
        rows = [state(30, 10, rival=20), state(50, 100, rival=40),
                state(112, 400, rival=390), state(120, 410, rival=430),
                state(200, 800, rival=900)]
        cross = rr.crossover(rows)
        self.assertEqual(cross["last_led_turn"], 112)
        self.assertEqual(cross["turn"], 120)
        self.assertEqual(cross["gap_at_end"], -100)

    def test_never_leading_says_so_rather_than_naming_a_turn(self) -> None:
        rows = [state(30, 10, rival=50), state(60, 20, rival=90)]
        self.assertIn("never led", rr.crossover(rows)["note"])

    def test_a_game_still_ahead_has_no_crossover(self) -> None:
        rows = [state(30, 50, rival=10), state(60, 90, rival=40)]
        self.assertIsNone(rr.crossover(rows))

    def test_turns_before_any_rival_is_visible_are_not_a_lead(self) -> None:
        """`rivals` is absent until the seat has met someone.

        Counting those turns as a lead would report every game as leading from
        turn 1 and put the crossover at first contact.
        """
        rows = [{"turn": 10, "score": 30}, state(60, 40, rival=80)]
        self.assertIn("never led", rr.crossover(rows)["note"])


class WinBandTest(unittest.TestCase):
    def test_a_seat_that_trailed_and_recovered_says_FOR_GOOD_not_never_behind(self) -> None:
        """The line has to survive being read next to its own trajectory.

        Live run `civvis-20260819T102134Z` trailed -74 at t100 and led +189 by
        t225. `crossover` correctly returns None — the lead was never lost for
        good — but a bare "never lost the lead" reads as "never trailed" and
        contradicts the table printed directly beneath it. It misled the author
        of this tool into nearly filing a defect against it.
        """
        rows = [state(50, 120, rival=119), state(100, 299, rival=373),
                state(150, 611, rival=589), state(225, 1161, rival=972)]
        self.assertIsNone(rr.crossover(rows), "the lead was regained, so no crossover")
        data = {"run": "civvis-20260819T102134Z", "turns": 225, "cities_at_60": 4,
                "in_win_band": True, "crossover": rr.crossover(rows),
                "ending": {"last_turn": 225, "victory": None},
                "trajectory": rr.trajectory(rows, 50),
                "ballots": {"verdicts": 0, "multi_vote_ballots": 0,
                            "multi_vote_registered": 0},
                "settler": {"holds": 0, "sites": []}}
        line = [l for l in rr.render(data).splitlines()
                if "never lost the lead" in l]
        self.assertTrue(line, "the no-crossover line is still printed")
        self.assertIn("for good", line[0],
                      "and it says FOR GOOD, so it cannot be read as 'never trailed'")

    def test_cities_at_sixty_is_read_from_the_last_turn_at_or_below_sixty(self) -> None:
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(50, 10, rival=1, cities=4),
                                        state(58, 20, rival=2, cities=6),
                                        state(70, 30, rival=3, cities=9)])
            data = rr.report(run, 25)
        self.assertEqual(data["cities_at_60"], 6)
        self.assertTrue(data["in_win_band"])

    def test_a_collapse_is_reported_outside_the_band(self) -> None:
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(60, 10, rival=40, cities=2)])
            data = rr.report(run, 25)
        self.assertEqual(data["cities_at_60"], 2)
        self.assertFalse(data["in_win_band"])
        self.assertIn("OUTSIDE", rr.render(data))

    def test_a_game_short_of_turn_sixty_reports_nothing_rather_than_zero(self) -> None:
        """An unfinished opening is unknown, not a collapse."""
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(20, 10, rival=5, cities=2)])
            data = rr.report(run, 25)
        self.assertIsNone(data["cities_at_60"])
        self.assertIn("turn 60 not reached", rr.render(data))


class UnmetRivalTest(unittest.TestCase):
    """A rival nobody has met is not a rival on nothing."""

    def test_an_unmet_rival_renders_as_a_dash_not_a_commanding_lead(self) -> None:
        """`best_rival` is 0 in both cases and the difference is the whole point.

        Observed against a live run: turn 50 showed `117  0  +117`, a crushing
        lead over an empty board, because no civilization had been contacted
        yet. The report exists to remove exactly that kind of false signal.
        """
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [
                {"turn": 50, "score": 117, "cities": [{}], "techs": []},
                state(75, 200, rival=260, cities=6),
            ])
            data = rr.report(run, 25)
        rendered = rr.render(data)
        self.assertNotIn("+117", rendered)
        self.assertIn("—", rendered)
        by_turn = {r["turn"]: r for r in data["trajectory"]}
        self.assertFalse(by_turn[50]["rival_seen"])
        self.assertTrue(by_turn[75]["rival_seen"])


class BallotTest(unittest.TestCase):
    """The row that exists because the seat's own report was wrong."""

    def test_only_purchased_vote_ballots_count_toward_the_ratio(self) -> None:
        """A free vote registering proves nothing about buying one."""
        extra = [
            {"kind": "wc_ballot_verdict", "turn": 62, "asked": 1,
             "recorded": 1, "registered": True},
            {"kind": "wc_ballot_verdict", "turn": 162, "asked": 13,
             "votes_sent": 13, "recorded": 1, "registered": False,
             "favor_at_ballot": 359},
        ]
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(60, 10, rival=5, cities=5)], extra)
            data = rr.report(run, 25)
        self.assertEqual(data["ballots"]["verdicts"], 2)
        self.assertEqual(data["ballots"]["multi_vote_ballots"], 1)
        self.assertEqual(data["ballots"]["multi_vote_registered"], 0)
        self.assertEqual(data["ballots"]["first_unregistered"]["turn"], 162)


class EndingTest(unittest.TestCase):
    def test_a_rivals_victory_is_not_reported_as_ours(self) -> None:
        """The worst possible error here, and the ladder's own standing rule."""
        extra = [{"kind": "victory", "victory": 6, "won": False}]
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(60, 10, rival=99, cities=4)], extra)
            data = rr.report(run, 25)
        rendered = rr.render(data)
        self.assertIn("a rival's DIPLOMATIC", rendered)
        self.assertNotIn("OURS", rendered)

    def test_our_own_victory_says_ours(self) -> None:
        extra = [{"kind": "victory", "victory": 0, "won": True}]
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(60, 99, rival=10, cities=5)], extra)
            data = rr.report(run, 25)
        self.assertIn("OURS SCORE", rr.render(data))


class SettlerHoldTest(unittest.TestCase):
    def test_sites_are_rendered_so_a_pair_is_not_read_as_two_numbers(self) -> None:
        """"14, 28" joined bare with a comma renders three sites as six numbers."""
        why = ("[why] t62 Settler HELD short of (14, 28) | 3 tiles away\n"
               "[why] t63 Settler HELD short of (14, 28) | 3 tiles away\n"
               "[why] t70 Settler HELD short of (9, 40) | 2 tiles away\n")
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(60, 10, rival=5, cities=4)], why=why)
            data = rr.report(run, 25)
        self.assertEqual(data["settler"]["holds"], 3)
        self.assertIn("(14, 28)×2", rr.render(data))

    def test_a_run_without_a_why_log_reports_no_holds_rather_than_failing(self) -> None:
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(60, 10, rival=5, cities=4)])
            self.assertEqual(rr.report(run, 25)["settler"]["holds"], 0)


class RefusalTest(unittest.TestCase):
    def test_a_missing_run_is_named_not_an_empty_table(self) -> None:
        with TemporaryDirectory() as raw:
            with self.assertRaises(rr.ReportError):
                rr.report(Path(raw) / "nope", 25)

    def test_a_run_with_no_state_records_is_named(self) -> None:
        with TemporaryDirectory() as raw:
            run = Path(raw) / "civvis-x"
            run.mkdir()
            (run / "events.jsonl").write_text('{"kind":"orders"}\n')
            with self.assertRaises(rr.ReportError):
                rr.report(run, 25)


if __name__ == "__main__":
    unittest.main()
