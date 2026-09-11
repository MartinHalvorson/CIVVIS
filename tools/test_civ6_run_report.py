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
                            "multi_vote_count_matches": 0},
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
        self.assertEqual(data["ballots"]["multi_vote_count_matches"], 0)
        self.assertEqual(data["ballots"]["first_count_mismatch"]["turn"], 162)

    def test_legacy_counts_and_complete_selections_are_not_conflated(self) -> None:
        def ballot(version, asked, option, actual_option, target, actual_target):
            return {"kind": "wc_ballot_verdict", "turn": 160, "asked": asked,
                    "recorded": asked, "registered": True,
                    "verification_version": version,
                    "option_asked": option, "option_recorded": actual_option,
                    "target_asked": target, "target_recorded": actual_target}
        extra = [
            ballot(1, 1, 2, 1, None, None),
            ballot(1, 3, 2, 2, None, None),
            ballot(2, 1, 2, 1, 4, 4),
            ballot(2, 3, 2, 2, 4, "4"),
            ballot(2, 3, 2, 2, 4, 0),
        ]
        with TemporaryDirectory() as raw:
            root = Path(raw)
            run = write_run(root, [state(60, 10, rival=20, cities=5)], extra)
            data = rr.report(run, 25)
            cohort = rr.aggregate(root, 25)
        ball = data["ballots"]
        self.assertEqual(ball["multi_vote_count_matches"], 3)
        self.assertEqual(ball["selection_verdicts"], 3)
        self.assertEqual(ball["selection_matches"], 1)
        self.assertEqual(ball["legacy_verdicts"], 2)
        self.assertEqual(ball["first_selection_mismatch"]["option_recorded"], 1)
        self.assertEqual(cohort["selection_matches"], 1)
        for rendered in (rr.render(data), rr.render_aggregate(cohort)):
            self.assertIn("purchased-vote counts matched", rendered)
            self.assertIn("complete selections verified: 1/3", rendered)
            self.assertIn("2 legacy verdicts lack complete verification", rendered)
            self.assertNotIn("ballots registered", rendered)

    def test_missing_target_and_extra_votes_do_not_verify_a_selection(self) -> None:
        extra = [
            {"kind": "wc_ballot_verdict", "verification_version": 2,
             "asked": 1, "recorded": 1, "option_asked": 1,
             "option_recorded": 1, "registered": True},
            {"kind": "wc_ballot_verdict", "verification_version": 2,
             "asked": 3, "recorded": 4, "option_asked": 1,
             "option_recorded": 1, "target_asked": 4,
             "target_recorded": 4, "registered": True},
        ]
        with TemporaryDirectory() as raw:
            run = write_run(Path(raw), [state(60, 10)], extra)
            ball = rr.ballots(run / "events.jsonl")
        self.assertEqual(ball["selection_matches"], 0)
        self.assertEqual(ball["selection_verdicts"], 2)
        self.assertEqual(ball["multi_vote_count_matches"], 0)


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


class SpaceRaceTest(unittest.TestCase):
    """Which of the four steps stopped the science lane, named."""

    @staticmethod
    def _rows(*, pad=None, pad_complete=True, projects=(), rival_projects=()):
        """A 250-turn run whose pad appears at `pad` and whose launches land
        at the turns given as {turn: [project, ...]}."""
        rows = []
        done: list[str] = []
        for turn in range(50, 251, 25):
            for name in projects.get(turn, ()) if isinstance(projects, dict) else ():
                done.append(name)
            districts = []
            if pad is not None and turn >= pad:
                districts = [{"type": "DISTRICT_SPACEPORT",
                              "complete": pad_complete}]
            rows.append({
                "turn": turn, "score": 100 + turn,
                "cities": [{"id": 0, "districts": districts}],
                "techs": [f"TECH_{i}" for i in range(40)],
                "science": 300.0,
                "science_projects": list(done),
                "rivals": [{"score": 90, "science": 250.0, "techs": 38,
                            "science_projects": list(rival_projects)}],
            })
        return rows

    def test_a_pad_that_never_completed_is_not_reported_as_a_pad(self) -> None:
        """★ The live defect this section was written for: run
        civvis-20260819T081800Z ordered a Spaceport at t206 and it only stood
        at t238. A reader that counts the district as soon as it appears says
        the empire had a launch site for thirty-two turns it did not have."""
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), self._rows(pad=200, pad_complete=False))
            race = rr.report(run, 25)["space_race"]
            self.assertEqual(race["pad_ordered_turn"], 200)
            self.assertIsNone(race["pad_standing_turn"])
            self.assertIn("NEVER COMPLETED", rr.render(rr.report(run, 25)))

    def test_a_district_written_as_a_bare_string_still_reads(self) -> None:
        """The export has carried districts both as objects and as bare type
        strings; most of the recorded corpus is the older shape."""
        with TemporaryDirectory() as tmp:
            rows = self._rows()
            for row in rows:
                row["cities"] = [{"id": 0, "districts": ["DISTRICT_SPACEPORT"]}]
            run = write_run(Path(tmp), rows)
            race = rr.report(run, 25)["space_race"]
            self.assertEqual(race["pad_standing_turn"], 50)

    def test_each_launch_is_dated_and_counted_against_the_best_rival(self) -> None:
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), self._rows(
                pad=100,
                projects={150: ["PROJECT_LAUNCH_EARTH_SATELLITE"],
                          200: ["PROJECT_LAUNCH_MOON_LANDING"]},
                rival_projects=["PROJECT_LAUNCH_EARTH_SATELLITE",
                                "PROJECT_LAUNCH_MOON_LANDING",
                                "PROJECT_LAUNCH_MARS_BASE"]))
            race = rr.report(run, 25)["space_race"]
            self.assertEqual(race["projects_done"], 2)
            self.assertEqual(race["best_rival_projects"], 3)
            dated = {p["label"]: p["turn"] for p in race["projects"]}
            self.assertEqual(dated["earth satellite"], 150)
            self.assertEqual(dated["moon landing"], 200)
            self.assertIsNone(dated["mars colony"])

    def test_a_rivals_tech_count_is_an_integer_not_a_list(self) -> None:
        """The seat exports its own techs as the list it knows and a rival's
        as a count; reading them alike reports every rival on one tech."""
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), self._rows())
            self.assertEqual(rr.report(run, 25)["space_race"]["best_rival_techs"], 38)

    def test_the_refusing_horizon_is_named_and_counted(self) -> None:
        """★ What the four recorded science runs all had in common: the race
        refused from ~t120 on, by the stock horizon, ~100 turns a game."""
        why = ("[why] t120 Cities/Detail The space race cannot finish before "
               "the turn limit | 130 turns left\n"
               "[why] t125 Cities/Detail The space race cannot finish before "
               "the turn limit | 125 turns left\n")
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), self._rows(), why=why)
            rendered = rr.render(rr.report(run, 25))
            self.assertIn("refused on 2 turns by the stock horizon, from t120",
                          rendered)
            self.assertIn("never engaged", rendered)

    def test_the_drive_reports_the_turn_it_engaged(self) -> None:
        why = ("[why] t88 Strategy/Decision Driving for a science victory | "
               "leading the field\n")
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), self._rows(), why=why)
            self.assertIn("science-victory-drive: engaged t88",
                          rr.render(rr.report(run, 25)))

    def test_the_v2_drive_uses_the_genome_header_instead_of_the_shared_phrase(self) -> None:
        header = json.dumps({
            "kind": "genome",
            "treatments": ["science-victory-drive-2"],
        })
        why = (header + "\n"
               "[why] t88 Strategy/Decision Driving for a science victory | "
               "leading the field\n")
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), self._rows(), why=why)
            race = rr.report(run, 25)["space_race"]
            self.assertEqual(race["drive_version"], "science-victory-drive-2")
            self.assertIn("science-victory-drive-2: engaged t88",
                          rr.render(rr.report(run, 25)))

    def test_a_run_without_a_why_log_says_so_rather_than_claiming_silence(self) -> None:
        with TemporaryDirectory() as tmp:
            run = write_run(Path(tmp), self._rows())
            self.assertIn("no why.log beside this run",
                          rr.render(rr.report(run, 25)))


class AggregateTest(unittest.TestCase):
    """The counterweight to reading three games and believing a story.

    Reading three runs by hand produced "we win the opening and get
    out-developed from turn 100"; the distribution over sixty-one completed
    losses put the median crossover at turn 77 with the mode at t25-49. These
    pin the arithmetic that corrected it.
    """

    def _ladder(self, root: Path):
        # two wins in band, one loss in band, one loss below band, one unfinished
        specs = [
            ("a", 5, True, 6, None),
            ("b", 6, True, 0, None),
            ("c", 4, False, 6, 120),      # led to t120 then lost it
            ("d", 2, False, 0, None),     # never led
            ("e", 4, None, None, None),   # no terminal event
        ]
        for name, cities, won, victory, led_to in specs:
            run = root / f"civvis-2026010{name}T000000Z"
            run.mkdir()
            rows = [state(60, 100, rival=(50 if led_to or won else 400),
                          cities=cities)]
            if led_to:
                rows.append(state(led_to, 300, rival=200, cities=cities))
                rows.append(state(led_to + 20, 310, rival=500, cities=cities))
            lines = [json.dumps({"kind": "state", **r}) for r in rows]
            if won is not None:
                lines.append(json.dumps({"kind": "victory", "victory": victory,
                                         "won": won}))
            (run / "events.jsonl").write_text("\n".join(lines) + "\n")

    def test_denominators_are_stated_rather_than_silently_dropped(self) -> None:
        """A rate whose denominator is unstated is the other way to be wrong."""
        with TemporaryDirectory() as raw:
            root = Path(raw)
            self._ladder(root)
            data = rr.aggregate(root, 25)
        self.assertEqual(data["runs_seen"], 5)
        self.assertEqual(data["completed"], 4)
        self.assertEqual(data["skipped_unfinished"], 1)
        self.assertIn("without a terminal event", rr.render_aggregate(data))

    def test_wins_are_grouped_by_the_opening_they_came_from(self) -> None:
        with TemporaryDirectory() as raw:
            root = Path(raw)
            self._ladder(root)
            data = rr.aggregate(root, 25)
        table = data["by_cities_at_60"]
        self.assertEqual(table[5], {"games": 1, "wins": 1})
        self.assertEqual(table[6], {"games": 1, "wins": 1})
        self.assertEqual(table[4], {"games": 1, "wins": 0})
        self.assertEqual(table[2], {"games": 1, "wins": 0})

    def test_a_loss_that_never_led_is_not_given_a_crossover_turn(self) -> None:
        """Otherwise a third of losses would invent a crossover at first contact."""
        with TemporaryDirectory() as raw:
            root = Path(raw)
            self._ladder(root)
            data = rr.aggregate(root, 25)
        self.assertEqual(data["never_led"], 1)
        self.assertEqual(data["crossovers"], [120])
        self.assertEqual(data["crossover_median"], 120)

    def test_a_win_contributes_no_crossover(self) -> None:
        """A won game did not lose its lead, and counting it would drag the median."""
        with TemporaryDirectory() as raw:
            root = Path(raw)
            self._ladder(root)
            data = rr.aggregate(root, 25)
        self.assertEqual(len(data["crossovers"]) + data["never_led"],
                         data["completed"] - data["wins"])

    def test_an_empty_directory_is_named_not_a_table_of_zeroes(self) -> None:
        with TemporaryDirectory() as raw:
            with self.assertRaises(rr.ReportError):
                rr.aggregate(Path(raw), 25)


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


def write_sized_run(root: Path, name: str, size, cities_at_60: int,
                    won: bool = False, last_turn: int = 120):
    """A run that reached a terminal event, with a recorded map size."""
    run = root / name
    run.mkdir()
    rows = [state(t, 100 + t, rival=90, cities=(cities_at_60 if t >= 60 else 1))
            for t in (20, 40, 60, last_turn)]
    lines = [json.dumps({"kind": "state", **s}) for s in rows]
    lines.append(json.dumps({"kind": "victory", "turn": last_turn,
                             "won": won, "victory": 5, "local_player": 0,
                             "team": 0 if won else 3, "local_team": 0}))
    (run / "events.jsonl").write_text("\n".join(lines) + "\n")
    doc = {"tag": name, "last_turn": last_turn}
    if size is not None:
        doc["map_size"] = size
    (run / "summary.json").write_text(json.dumps(doc))
    return run


class TheBandBelongsToAMapSize(unittest.TestCase):
    """`WIN_BAND` is a fact about 218 MAPSIZE_SMALL runs, not about Civ VI.

    The lobby ignored the configured size until 2026-09-10, so every run behind
    that band was Small. The first 17 runs after the size began to be applied
    were Tiny and none sat inside it. Pooling the two hides that.
    """

    def test_map_size_is_read_from_the_runs_own_summary(self) -> None:
        with TemporaryDirectory() as d:
            run = write_sized_run(Path(d), "civvis-20260101T000000Z",
                                  "MAPSIZE_TINY", 3)
            self.assertEqual(rr.map_size(run), "MAPSIZE_TINY")

    def test_a_run_with_no_recorded_size_says_so_rather_than_guessing(self) -> None:
        with TemporaryDirectory() as d:
            run = write_sized_run(Path(d), "civvis-20260101T000000Z", None, 3)
            self.assertEqual(rr.map_size(run), rr.UNKNOWN_MAP_SIZE)

    def test_an_unreadable_summary_does_not_raise(self) -> None:
        with TemporaryDirectory() as d:
            run = write_sized_run(Path(d), "civvis-20260101T000000Z",
                                  "MAPSIZE_TINY", 3)
            (run / "summary.json").write_text("{ not json")
            self.assertEqual(rr.map_size(run), rr.UNKNOWN_MAP_SIZE)

    def test_the_aggregate_splits_the_band_by_size(self) -> None:
        with TemporaryDirectory() as d:
            root = Path(d)
            write_sized_run(root, "civvis-20260101T000001Z", "MAPSIZE_SMALL", 5)
            write_sized_run(root, "civvis-20260101T000002Z", "MAPSIZE_SMALL", 4)
            write_sized_run(root, "civvis-20260101T000003Z", "MAPSIZE_TINY", 2)
            write_sized_run(root, "civvis-20260101T000004Z", "MAPSIZE_TINY", 3)
            data = rr.aggregate(root, every=20)
            sizes = data["by_map_size"]
            self.assertEqual(sizes["MAPSIZE_SMALL"]["in_band"], 2)
            self.assertEqual(sizes["MAPSIZE_TINY"]["in_band"], 0)
            self.assertEqual(sizes["MAPSIZE_TINY"]["mean_cities_at_60"], 2.5)
            self.assertTrue(sizes["MAPSIZE_SMALL"]["band_applies"])
            self.assertFalse(sizes["MAPSIZE_TINY"]["band_applies"])

    def test_the_render_warns_when_two_sizes_are_pooled(self) -> None:
        with TemporaryDirectory() as d:
            root = Path(d)
            write_sized_run(root, "civvis-20260101T000001Z", "MAPSIZE_SMALL", 5)
            write_sized_run(root, "civvis-20260101T000002Z", "MAPSIZE_TINY", 2)
            text = rr.render_aggregate(rr.aggregate(root, every=20))
            self.assertIn("MORE THAN ONE SIZE IS POOLED", text)
            self.assertIn("band not measured here", text)

    def test_one_size_that_matches_raises_no_warning(self) -> None:
        with TemporaryDirectory() as d:
            root = Path(d)
            write_sized_run(root, "civvis-20260101T000001Z", "MAPSIZE_SMALL", 5)
            write_sized_run(root, "civvis-20260101T000002Z", "MAPSIZE_SMALL", 4)
            text = rr.render_aggregate(rr.aggregate(root, every=20))
            self.assertNotIn("MORE THAN ONE SIZE IS POOLED", text)
            self.assertNotIn("band not measured here", text)

    def test_a_single_run_says_whether_the_band_applies_to_it(self) -> None:
        with TemporaryDirectory() as d:
            root = Path(d)
            run = write_sized_run(root, "civvis-20260101T000001Z",
                                  "MAPSIZE_TINY", 5)
            data = rr.report(run, every=20)
            self.assertTrue(data["in_win_band"], "5 cities is inside 4-6")
            self.assertFalse(data["band_applies"],
                             "but the band was never measured on Tiny")


def founding_run(root: Path, name: str, founded: list[int],
                 captured: tuple[int, int] | None = None, last_turn: int = 200):
    """A run whose cities appear at given turns; `captured` is (turn, pop>1)."""
    run = root / name
    run.mkdir()
    turns = sorted({1, *founded, *( [captured[0]] if captured else []), last_turn})
    lines = []
    for t in turns:
        cities = [{"id": f"f{i}", "pop": 1}
                  for i, ft in enumerate(founded) if ft <= t]
        if captured and captured[0] <= t:
            cities.append({"id": "taken", "pop": captured[1]})
        lines.append(json.dumps({"kind": "state", "turn": t, "score": 100 + t,
                                 "rivals": [{"score": 90}], "cities": cities,
                                 "techs": []}))
    lines.append(json.dumps({"kind": "victory", "turn": last_turn, "won": False,
                             "victory": 5, "local_player": 0, "team": 3,
                             "local_team": 0}))
    (run / "events.jsonl").write_text("\n".join(lines) + "\n")
    (run / "summary.json").write_text(json.dumps(
        {"tag": name, "map_size": "MAPSIZE_SMALL", "last_turn": last_turn}))
    return run


class TheOpeningCadenceIsMeasured(unittest.TestCase):
    """`opening_settler_waits` states in its doc that the book founds city 2 at
    t19-24. Nothing checked it, and the recorded runs do not agree."""

    def test_a_captured_city_is_not_a_founding(self) -> None:
        """It arrives at the population it had. Counting it would credit the
        settler pipeline with a conquest and hide the real cadence."""
        with TemporaryDirectory() as d:
            run = founding_run(Path(d), "civvis-20260101T000001Z",
                               founded=[1, 30, 55], captured=(129, 2))
            rows = [json.loads(l) for l in
                    (run / "events.jsonl").read_text().splitlines()]
            rows = [r for r in rows if r.get("kind") == "state"]
            self.assertEqual(rr.founding_turns(rows), [1, 30, 55])

    def test_a_city_without_an_id_is_tracked_by_name(self) -> None:
        rows = [{"turn": 1, "cities": [{"name": "Bogota", "pop": 1}]},
                {"turn": 9, "cities": [{"name": "Bogota", "pop": 2},
                                       {"name": "Quito", "pop": 1}]}]
        self.assertEqual(rr.founding_turns(rows), [1, 9])

    def test_a_city_growing_past_pop_one_is_not_counted_twice(self) -> None:
        rows = [{"turn": 1, "cities": [{"id": 1, "pop": 1}]},
                {"turn": 20, "cities": [{"id": 1, "pop": 4}]}]
        self.assertEqual(rr.founding_turns(rows), [1])

    def test_the_aggregate_reports_a_median_turn_per_city(self) -> None:
        with TemporaryDirectory() as d:
            root = Path(d)
            founding_run(root, "civvis-20260101T000001Z", [1, 20, 50, 70])
            founding_run(root, "civvis-20260101T000002Z", [1, 30, 60, 80])
            founding_run(root, "civvis-20260101T000003Z", [1, 40, 70, 90])
            data = rr.aggregate(root, every=20)
            cadence = data["founding_cadence"]
            self.assertEqual(cadence[2]["median_turn"], 30)
            self.assertEqual(cadence[4]["median_turn"], 80)
            self.assertEqual(cadence[4]["runs"], 3)

    def test_a_fourth_city_after_turn_sixty_is_counted_as_late(self) -> None:
        with TemporaryDirectory() as d:
            root = Path(d)
            founding_run(root, "civvis-20260101T000001Z", [1, 20, 40, 55])
            founding_run(root, "civvis-20260101T000002Z", [1, 30, 60, 80])
            data = rr.aggregate(root, every=20)
            self.assertEqual(data["fourth_city_by_turn_60"],
                             {"runs": 2, "in_time": 1})

    def test_the_render_flags_a_city_two_outside_the_documented_window(self) -> None:
        with TemporaryDirectory() as d:
            root = Path(d)
            founding_run(root, "civvis-20260101T000001Z", [1, 32, 60, 77])
            text = rr.render_aggregate(rr.aggregate(root, every=20))
            self.assertIn("the opening book's own doc says", text)

    def test_a_city_two_inside_the_documented_window_is_not_flagged(self) -> None:
        with TemporaryDirectory() as d:
            root = Path(d)
            founding_run(root, "civvis-20260101T000001Z", [1, 21, 40, 55])
            text = rr.render_aggregate(rr.aggregate(root, every=20))
            self.assertNotIn("the opening book's own doc says", text)

    def test_a_single_run_carries_its_own_cadence(self) -> None:
        with TemporaryDirectory() as d:
            run = founding_run(Path(d), "civvis-20260101T000001Z",
                               [1, 29, 48, 88], captured=(129, 2))
            data = rr.report(run, every=20)
            self.assertEqual(data["founding_turns"], [1, 29, 48, 88])
            self.assertEqual(data["city_two_turn"], 29)
            self.assertEqual(data["fourth_city_turn"], 88)
