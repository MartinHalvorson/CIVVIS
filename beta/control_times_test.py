#!/usr/bin/env python3
"""The computer control benchmark: one game with CIVVIS at the controls of
Civilization VI, timed end to end, and its chart at the top of the CIVVIS
Performance page."""

from __future__ import annotations

import contextlib
import html
import io
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import control_times
import home_rankings
import performance_page

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "performance.html").read_text(encoding="utf-8")
SECTION = PAGE[PAGE.index('<section class="res-section" id="perf-control"'):PAGE.index('<section class="res-section" id="perf-speedup"')]
SCRIPT = PAGE[PAGE.index("// ---- CIVVIS computer control: game speed over time"):PAGE.index('document.getElementById("perf-empty").hidden = true;')]

sys.path.insert(0, str(ROOT / "tools"))
import civ6_ladder  # noqa: E402


def run(tag: str, utc: str, outcome: str = "rival_victory", **fields) -> dict:
    """One line of `results.jsonl`: a run of the harness."""
    row = {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": utc,
        "outcome": outcome,
        "difficulty": "DIFFICULTY_DEITY",
        "players": 6,
        "turns": 180,
    }
    row.update(fields)
    return row


def played(day: str, minutes: list[float], turns: int = 180, **fields) -> list[dict]:
    """A day's games from before the clock, each launched at a whole hour and
    recorded `minutes` later, so its time is read off its two stamps."""
    rows = []
    for hour, took in enumerate(minutes):
        stamp = day.replace("-", "") + f"T{hour:02d}0000Z"
        seconds = round(took * 60)
        rows.append(run(f"civvis-{stamp}", f"{day}T{hour:02d}:{seconds // 60:02d}:{seconds % 60:02d}Z", turns=turns, **fields))
    return rows


class TheGames(unittest.TestCase):
    def test_it_reads_the_one_ledger_by_the_one_ladder(self):
        # The page's other parts read the same file under the same names.
        self.assertEqual(control_times.RESULTS_REF, home_rankings.RESULTS_REF)
        self.assertEqual(control_times.OUTCOMES, home_rankings.OUTCOMES)
        self.assertEqual(control_times.CIV6_RUNGS, home_rankings.CIV6_RUNGS)
        self.assertLessEqual(control_times.COMPLETE, set(control_times.OUTCOMES))
        self.assertEqual(control_times.level("DIFFICULTY_DEITY"), 8)
        self.assertEqual(control_times.level("DIFFICULTY_SETTLER"), 1)
        self.assertIsNone(control_times.level(None))
        for games in ([3.0], [1.0, 2.0, 3.0, 4.0], [float(n) for n in range(1, 62)]):
            for p in control_times.PERCENTILES:
                self.assertEqual(control_times.percentile(games, p), performance_page.percentile(games, p))

    def test_a_game_before_the_clock_runs_from_its_tag_s_stamp_to_its_result(self):
        games, untimed = control_times.game_times([run("civvis-t7-20261124T150035Z", "2026-11-24T15:13:11Z", turns=203, difficulty="DIFFICULTY_KING")])
        self.assertEqual(untimed, 0)
        self.assertEqual(games, [{"day": "2026-11-24", "seconds": 756.0, "turns": 203, "level": 5, "players": 6, "clocked": False, "split": None}])

    def test_a_clocked_game_is_the_harness_s_own_seconds(self):
        # The stamps say 756 s; the clock is what counts, and it says its parts.
        row = run("civvis-t7-20261124T150035Z", "2026-11-24T15:13:11Z", outcome="win", seconds=741.5, setup_seconds=48.0, play_seconds=665.5)
        games, _ = control_times.game_times([row])
        self.assertEqual((games[0]["seconds"], games[0]["clocked"], games[0]["split"]), (741.5, True, (48.0, 665.5, 28.0)))
        # A clocked run without its parts is still a time.
        bare, _ = control_times.game_times([run("civvis-20261124T150035Z", "2026-11-24T15:13:11Z", seconds=741.5)])
        self.assertEqual((bare[0]["seconds"], bare[0]["clocked"], bare[0]["split"]), (741.5, True, None))

    def test_a_reloaded_game_is_the_sum_of_its_runs_and_has_no_time_before_the_clock(self):
        # A continuation's tag carries the first run's stamp, so the stamps
        # cannot time it. The clock can: each run says its own seconds.
        first = run("civvis-20261124T100000Z", "2026-11-24T10:05:00Z", outcome="unfinished", seconds=300.0, setup_seconds=50.0, play_seconds=240.0)
        again = run("civvis-20261124T100000Z-cont1", "2026-11-24T12:10:00Z", outcome="win", seconds=500.0, setup_seconds=60.0, play_seconds=420.0)
        games, untimed = control_times.game_times([again, first])
        self.assertEqual((len(games), untimed), (1, 0))
        self.assertEqual((games[0]["seconds"], games[0]["split"], games[0]["day"]), (800.0, (110.0, 660.0, 30.0), "2026-11-24"))
        # The same game from before the clock, and one whose first run had none.
        for rows in (
            [{k: v for k, v in row.items() if "seconds" not in k} for row in (first, again)],
            [{k: v for k, v in first.items() if "seconds" not in k}, again],
        ):
            self.assertEqual(control_times.game_times(rows), ([], 1))

    def test_only_a_game_played_to_a_victory_counts(self):
        rows = []
        for hour, outcome in enumerate(control_times.OUTCOMES):
            rows.append(run(f"civvis-20261124T{hour:02d}0000Z", f"2026-11-24T{hour:02d}:10:00Z", outcome=outcome))
        games, untimed = control_times.game_times(rows)
        self.assertEqual((len(games), untimed), (2, 0))
        # A reloaded game ended the best way any of its runs did.
        stopped = run("civvis-20261123T100000Z", "2026-11-23T10:05:00Z", outcome="unfinished", seconds=300.0)
        won = run("civvis-20261123T100000Z-cont1", "2026-11-23T10:20:00Z", outcome="win", seconds=600.0)
        self.assertEqual([game["seconds"] for game in control_times.game_times([stopped, won])[0]], [900.0])

    def test_a_line_that_cannot_be_timed_costs_nothing_but_itself(self):
        good = run("civvis-20261124T100000Z", "2026-11-24T10:12:00Z")
        odd = [
            run("civvis-notag", "2026-11-24T11:00:00Z"),  # no stamp in the tag
            run("civvis-20261124T120000Z", "2026-11-24T11:00:00Z"),  # recorded before it was launched
            run("civvis-20261124T130000Z", "not a moment"),
            run("civvis-20269999T130000Z", "2026-11-24T13:10:00Z"),  # not a day
            {"tag": "civvis-20261124T140000Z", "outcome": "win"},  # no result stamp
            {"outcome": "win", "utc": "2026-11-24T15:00:00Z"},  # no tag
        ]
        for seconds in (True, "741", -5, 0, float("nan")):
            odd.append(run(f"civvis-odd-{seconds}", "2026-11-24T16:00:00Z", seconds=seconds))
        games, untimed = control_times.game_times([good, *odd])
        self.assertEqual([game["seconds"] for game in games], [720.0])
        self.assertEqual(untimed, 8)
        # The turn a game ended on is a whole turn or it is unknown.
        for turns in (0, None, True, "180", 180.5):
            game = control_times.game_times([run("civvis-20261124T100000Z", "2026-11-24T10:12:00Z", turns=turns)])[0][0]
            self.assertIsNone(game["turns"], turns)

    def test_the_line_the_harness_writes_is_the_line_the_page_reads(self):
        # The names on the run's line are the ladder's (`tools/civ6_ladder.py`
        # `result_row`), from the summary the harness writes: read here from
        # that code, so neither side can rename a reading alone.
        summary = {
            "tag": "civvis-t7-20261124T150035Z",
            "finished_utc": "2026-11-24T15:13:11Z",
            "started_utc": "2026-11-24T15:00:36Z",
            "timing": {"seconds": 755.2, "setup_seconds": 47.9, "play_seconds": 679.0, "end_seconds": 28.3},
            "difficulty": "DIFFICULTY_DEITY",
            "last_turn": 191,
            "outcome": {"kind": "victory", "won": True, "victory": 3},
            "seat": {"players": 6},
            "reason": "stopped",
        }
        row = civ6_ladder.result_row(civ6_ladder.result_entry(summary))
        games, untimed = control_times.game_times([json.loads(json.dumps(row))])
        self.assertEqual(untimed, 0)
        self.assertEqual(games[0]["seconds"], 755.2)
        self.assertTrue(games[0]["clocked"])
        self.assertEqual([round(part, 1) for part in games[0]["split"]], [47.9, 679.0, 28.3])
        self.assertEqual((games[0]["turns"], games[0]["level"], games[0]["players"]), (191, 8, 6))


class TheRecord(unittest.TestCase):
    def test_a_day_is_its_games_at_three_percentiles(self):
        minutes = [10, 11, 12, 13, 14, 15, 16, 17, 18, 30]
        day = control_times.summary(control_times.game_times(played("2026-11-20", minutes, turns=120))[0])
        # Nearest rank on the ten games: the 1st, the 5th and the 9th.
        self.assertEqual(day["p"], [600, 840, 1080])
        self.assertEqual(day["a_turn"], [5.0, 7.0, 9.0])
        self.assertEqual((day["games"], day["turns"], day["levels"], day["players"], day["clocked"]), (10, 120, [8, 8], 6, 0))
        self.assertNotIn("split", day)

    def test_the_parts_of_a_game_are_shown_from_three_clocked_games(self):
        def clocked(n: int) -> list[dict]:
            return [
                run(f"civvis-20261124T{hour:02d}0000Z", f"2026-11-24T{hour:02d}:13:00Z", seconds=700.0 + 10 * hour, setup_seconds=40.0 + hour, play_seconds=630.0 + 8 * hour)
                for hour in range(n)
            ]

        two = control_times.summary(control_times.game_times(clocked(2))[0])
        three = control_times.summary(control_times.game_times(clocked(3))[0])
        self.assertNotIn("split", two)
        self.assertNotIn("split_games", two)
        self.assertEqual((three["clocked"], three["split"]), (3, [41, 638, 31]))
        self.assertEqual(control_times.MIN_GAMES, 3)
        # How many games the parts are the middle of goes with them. On the
        # clock's first day, 2026-11-24, three clocked games' parts (49 s,
        # 12.5 min, 44 s: 14.1 min) stood under a tile reading 12.7 min, the
        # middle of the week's 452, as "the middle game's".
        self.assertEqual(three["split_games"], 3)
        unclocked = [run(f"civvis-20261123T{hour:02d}0000Z", f"2026-11-23T{hour:02d}:11:00Z") for hour in range(5)]
        mixed = control_times.summary(control_times.game_times(clocked(3) + unclocked)[0])
        self.assertEqual((mixed["games"], mixed["clocked"], mixed["split_games"]), (8, 3, 3))

    def test_the_page_says_how_many_games_the_parts_come_from_while_they_are_few(self):
        block = SCRIPT[SCRIPT.index("// Where a game's time goes"):SCRIPT.index("const all = control.all")]
        self.assertIn("drawn = Number(week.split_games) || 0", block)
        self.assertIn("const lead = drawn && drawn < Number(week.games)\n", block)
        self.assertIn('? "Where a game’s time goes, in the " + count(drawn) + " timed part by part so far: "', block)
        self.assertIn(': "Where the middle game’s time goes: ";', block)
        # The reader rounds the parts to whole seconds, so no tenth is shown.
        self.assertIn('const part = (x) => (x < 99.5 ? Math.round(x) + " s" : span(x));', block)
        self.assertEqual(block.count("part(parts["), 3)
        self.assertNotIn("span(parts[", block)

    def test_the_record_is_the_days_the_last_week_and_the_first(self):
        rows = (
            played("2026-08-01", [40, 41, 42], turns=250, difficulty="DIFFICULTY_SETTLER", players=None)
            + played("2026-08-03", [39, 44], turns=250, difficulty="DIFFICULTY_SETTLER", players=None)  # two games: not a point
            + played("2026-08-09", [50, 51, 52], difficulty="DIFFICULTY_KING")  # past the first seven days
            + played("2026-11-17", [20, 21, 22])  # before the last seven
            + played("2026-11-18", [13, 14, 15])
            + played("2026-11-24", [11, 12, 12.5, 16])
        )
        # A game we were eliminated from and one somebody stopped are no part of it.
        rows += [run("civvis-20261124T200000Z", "2026-11-24T20:03:00Z", outcome="defeat"), run("civvis-20261124T210000Z", "2026-11-24T21:01:00Z", outcome="unfinished")]
        record = control_times.record(rows, 18000.0)
        self.assertEqual([day["day"] for day in record["days"]], ["2026-08-01", "2026-08-09", "2026-11-17", "2026-11-18", "2026-11-24"])
        self.assertEqual(record["days"][-1]["p"], [660, 720, 960])
        week, first = record["week"], record["first"]
        self.assertEqual((week["from"], week["to"], week["games"], week["p"][1]), ("2026-11-18", "2026-11-24", 7, 780))
        # The first seven days hold the day that was too thin to draw.
        self.assertEqual((first["from"], first["to"], first["games"], first["p"][1], first["levels"]), ("2026-08-01", "2026-08-03", 5, 2460, [1, 1]))
        self.assertEqual(record["all"], {"games": 18, "from": "2026-08-01", "to": "2026-11-24", "untimed": 0, "clocked": 0})
        self.assertEqual((record["by_hand"], record["percentiles"], record["min_games"], record["window_days"]), (18000.0, [10, 50, 90], 3, 7))
        # It goes into the page's slot as JSON and comes back the same.
        self.assertEqual(json.loads(json.dumps(record)), record)

    def test_without_a_timed_game_there_is_no_record(self):
        self.assertIsNone(control_times.record([]))
        self.assertIsNone(control_times.record([run("civvis-20261124T200000Z", "2026-11-24T20:03:00Z", outcome="abandoned")]))

    def test_the_baseline_is_the_speed_timeline_s_game_by_hand(self):
        self.assertEqual(control_times.by_hand(ROOT), 18000.0)
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(control_times.by_hand(pathlib.Path(scratch)))

    def test_it_is_read_from_the_ledger_branch_of_the_clone(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            # A tree that is no clone, and a clone with no ledger, have none.
            self.assertIsNone(control_times.control(root))
            git = lambda *args, **more: subprocess.run(["git", "-C", str(root), *args], check=True, capture_output=True, **more)
            git("init", "-q")
            self.assertIsNone(control_times.control(root))
            lines = "".join(json.dumps(row) + "\n" for row in played("2026-11-24", [11, 12, 13])) + "half a li"
            blob = git("hash-object", "-w", "--stdin", input=lines.encode()).stdout.decode().strip()
            tree = git("mktree", input=f"100644 blob {blob}\tresults.jsonl\n".encode()).stdout.decode().strip()
            identity = ["-c", "user.name=test", "-c", "user.email=test@example.invalid"]
            commit = subprocess.run(["git", "-C", str(root), *identity, "commit-tree", tree, "-m", "ledger"], check=True, capture_output=True).stdout.decode().strip()
            git("update-ref", "refs/remotes/origin/ledger", commit)
            record = control_times.control(root)
            self.assertEqual((record["all"]["games"], record["days"][0]["p"], record["by_hand"]), (3, [660, 720, 780], None))
            with contextlib.redirect_stdout(io.StringIO()) as printed:
                self.assertEqual(control_times.main(["control_times.py", str(root)]), 0)
            self.assertEqual(json.loads(printed.getvalue()), record)


ROWS = played("2026-11-23", [12, 13, 14, 15]) + played("2026-11-24", [11, 12, 13])


class ThePage(unittest.TestCase):
    def test_the_page_s_record_carries_it_as_the_ledger_stands(self):
        with mock.patch.object(control_times, "results", return_value=ROWS), contextlib.redirect_stderr(io.StringIO()) as said:
            record = performance_page.performance(ROOT)
        self.assertEqual(record["control"], control_times.record(ROWS, 18000.0))
        self.assertNotIn("no computer control game times", said.getvalue())
        held = json.loads(performance_page.SLOT.search(performance_page.fill(PAGE, record))[2])
        self.assertEqual(held["control"], record["control"])

    def test_a_clone_without_the_ledger_has_its_page_without_this_part(self):
        with mock.patch.object(control_times, "results", return_value=None), contextlib.redirect_stderr(io.StringIO()) as said:
            record = performance_page.performance(ROOT)
        self.assertIsNone(record["control"])
        self.assertTrue(record["speedup"] and record["videos"])
        self.assertIn("no computer control game times", said.getvalue())

    def test_a_ledger_that_cannot_be_read_costs_the_page_this_part_alone(self):
        # Every finished game writes the ledger, and the site must still publish.
        with mock.patch.object(control_times, "control", side_effect=KeyError("utc")), contextlib.redirect_stderr(io.StringIO()) as said:
            record = performance_page.performance(ROOT)
        self.assertIsNone(record["control"])
        self.assertTrue(record["speedup"] and record["videos"])
        self.assertIn("could not be timed", said.getvalue())

    def test_the_games_alone_are_a_page(self):
        with tempfile.TemporaryDirectory() as scratch, mock.patch.object(control_times, "results", return_value=ROWS), contextlib.redirect_stderr(io.StringIO()):
            record = performance_page.performance(pathlib.Path(scratch))
        self.assertEqual(record["control"]["all"]["games"], 7)
        self.assertEqual((record["windows"], record["videos"], record["speedup"]), ({}, [], None))

    def test_it_is_a_top_part_of_the_page_and_shows_only_when_filled(self):
        # Operator 2026-11-24: "be a top chart for civvis performance". It
        # was the first part until 2026-11-27, when the speed timeline went
        # over it ("move civ game speedup - synthetic data pipeline up to
        # the top of civvis.ai performance"); it is the second.
        record = PAGE[PAGE.index('<div class="res-record" id="perf-record" hidden>'):]
        self.assertEqual(re.findall(r'<section class="res-section" id="([\w-]+)"', record)[:2], ["perf-line", "perf-control"])
        self.assertIn('<section class="res-section" id="perf-control" aria-labelledby="perf-control-title" hidden>', PAGE)
        self.assertIn('<h2 class="rank-title" id="perf-control-title">CIVVIS computer control: game speed over time</h2>', SECTION)
        self.assertIn('document.getElementById("perf-control").hidden = false;', SCRIPT)
        # The control, the tiles, the chart, its table, then what it counts.
        order = ['id="perf-control-measure"', 'id="perf-control-tiles"', 'id="perf-control-split"', 'id="perf-control-plot"', 'id="perf-control-table"', 'id="perf-control-note"']
        places = [SECTION.index(mark) for mark in order]
        self.assertEqual(places, sorted(places))
        buttons = re.findall(r'<button type="button" data-measure="(\w+)" aria-pressed="(\w+)">([^<]+)</button>', SECTION)
        self.assertEqual(buttons, [("game", "true", "A game"), ("turn", "false", "A turn")])
        self.assertIn('const cstate = { measure: "game" };', SCRIPT)
        for measure, _, _ in buttons:
            self.assertRegex(SCRIPT, rf"\n        {measure}: \{{ p: ")

    def test_it_is_drawn_once_the_record_shows_and_again_when_the_window_changes(self):
        # A chart is drawn to the width it has, and a hidden page has none.
        tail = PAGE[PAGE.index('document.getElementById("perf-empty").hidden = true;'):]
        shown = tail.index('document.getElementById("perf-record").hidden = false;')
        self.assertLess(shown, tail.index("if (drawControl) drawControl();"))
        self.assertIn("if (drawControl) charts.push(drawControl);", tail)
        self.assertIn("for (const chart of charts) chart();", tail)
        self.assertIn("drawControl = drawControlPlot;", SCRIPT)

    def test_no_figure_is_written_into_the_page_and_none_of_it_is_markup(self):
        text = html.unescape(re.sub(r"<[^>]+>", " ", re.sub(r"<!--.*?-->", "", SECTION, flags=re.S)))
        self.assertIsNone(re.search(r"\d", text), re.findall(r".{0,20}\d.{0,20}", text)[:3])
        for unsafe in ("innerHTML", "insertAdjacentHTML", "outerHTML", "document.write"):
            self.assertNotIn(unsafe, SCRIPT)
        # A day becomes part of a date, so it is checked for its shape first.
        self.assertIn("const DAY_ISO = /^\\d{4}-\\d{2}-\\d{2}$/;", SCRIPT)
        self.assertIn("DAY_ISO.test(String(d.day)) && three(d.p)", SCRIPT)

    def test_the_words_are_few_and_what_it_counts_is_closed(self):
        # Operator 2026-11-24: "there is too much text on pages ... should be
        # a more visual experience. some text ok. just not too much."
        lines = re.findall(r'<p class="rank-sub">(.*?)</p>', SECTION, flags=re.S)
        self.assertEqual(len(lines), 1)
        self.assertLessEqual(len(html.unescape(lines[0]).split()), 35)
        self.assertIn('<details class="fine">\n          <summary>How it is counted</summary>', SECTION)
        self.assertNotRegex(SECTION, r"<details[^>]*\sopen")
        counted = SECTION[SECTION.index('<details class="fine">'):]
        self.assertLess(counted.index('id="perf-control-note"'), counted.index("</details>"))
        # The write-up and the games themselves are a press away.
        self.assertIn("https://github.com/MartinHalvorson/CIVVIS/blob/main/docs/CONTROL_GAME_TIME.md", SECTION)
        self.assertTrue((ROOT / "docs/CONTROL_GAME_TIME.md").is_file())
        self.assertIn("https://github.com/MartinHalvorson/CIVVIS/blob/ledger/results.jsonl", SECTION)


if __name__ == "__main__":
    unittest.main()
