#!/usr/bin/env python3
"""The Performance page: the fastest game on video and how long a self-play game takes."""

from __future__ import annotations

import contextlib
import datetime
import html
import io
import json
import os
import pathlib
import re
import tempfile
import unittest

import performance_page
import speedup_timeline

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "performance.html").read_text(encoding="utf-8")
MARKUP, SCRIPT = PAGE.split('<script type="application/json" id="performance-data">', 1)
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
RESEARCH = (BETA / "research.html").read_text(encoding="utf-8")
TOURNAMENTS = (BETA / "tournaments.html").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

HEADER = {
    "kind": "header",
    "speed": "online",
    "turns": 250,
    "players": 6,
    "map": "continents",
    "all_seats": True,
    "build": {"commit": "39c314e77f0000da86f136cb2221973742f9a197"},
}


def seats(seed: int, secs: float, turn: int = 200, victory: str = "science") -> list[dict]:
    """A game's rows as the gene screen writes them: one per seat, each with
    the game's whole wall time."""
    return [{"kind": "game", "seed": seed, "seat": seat, "secs": secs, "turn": turn, "victory": victory} for seat in range(6)]


def batch(root: pathlib.Path, name: str, games: list[list[dict]], header: dict = HEADER, finished: bool = True, ended: str | None = None, rows: str = "rows-continuous.jsonl") -> pathlib.Path:
    directory = root / name
    directory.mkdir(exist_ok=True)
    lines = [header] + [row for game in games for row in game]
    file = directory / rows
    file.write_text("".join(json.dumps(line) + "\n" for line in lines), encoding="utf-8")
    if finished:
        (directory / "analysis.json").write_text("{}", encoding="utf-8")
    if ended:
        stamp = datetime.datetime.fromisoformat(ended + "T12:00:00+00:00").timestamp()
        os.utime(file, (stamp, stamp))
    return directory


class GameTimes(unittest.TestCase):
    def test_the_percentiles_are_the_five_asked_for_in_their_order(self):
        # Operator 2026-11-21: "p1, p10, p50, p90, p99 times for games".
        self.assertEqual(performance_page.PERCENTILES, [1, 10, 50, 90, 99])
        self.assertIn("const PERCENTILES = [1, 10, 50, 90, 99];", PAGE)

    def test_a_percentile_is_the_nearest_rank_on_the_games(self):
        hundred = [float(n) for n in range(1, 101)]
        self.assertEqual([performance_page.percentile(hundred, p) for p in (1, 10, 50, 90, 99)], [1.0, 10.0, 50.0, 90.0, 99.0])
        # Five games: the 1st and the 10th percentile are the fastest game,
        # the 50th the middle one, the 90th and 99th the slowest.
        five = [10.0, 20.0, 30.0, 40.0, 50.0]
        self.assertEqual([performance_page.percentile(five, p) for p in (1, 10, 50, 90, 99)], [10.0, 10.0, 30.0, 50.0, 50.0])
        self.assertEqual(performance_page.percentile([7.5], 99), 7.5)

    def test_a_span_of_time_is_a_twentieth_of_a_tenfold(self):
        self.assertEqual(performance_page.SPANS_PER_TENFOLD, 20)
        self.assertEqual(performance_page.span(10.0), 20)
        self.assertEqual(performance_page.span(99.9), 39)
        self.assertEqual(performance_page.span(100.0), 40)
        self.assertEqual(performance_page.span(8.3), 18)

    def test_the_snapshot_holds_a_game_per_seed_of_the_finished_batches_of_the_one_table(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            old = batch(root, "20261101T000000Z-aaaa", [seats(1, 400.0, 220), seats(2, 600.0, 240)], ended="2026-11-02")
            # The same seed again in the batch's second file is the same game.
            batch(root, old.name, [seats(2, 600.0, 240), seats(3, 500.0, 230, "culture")], ended="2026-11-02", rows="rows-deadline-cutoff.jsonl")
            batch(root, "20261120T000000Z-bbbb", [seats(11, 80.0, 200), seats(12, 20.0, 100, "culture"), seats(13, 120.0, 240)], ended="2026-11-20")
            batch(root, "20261121T000000Z-cccc", [seats(21, 60.0, 200), seats(22, 90.0, 180)], ended="2026-11-21")
            # Still being played: no analysis yet, so it is left out.
            batch(root, "20261122T000000Z-dddd", [seats(31, 1.0)], finished=False)
            # Another table, and a directory that is not a batch.
            batch(root, "20261119T000000Z-eeee", [seats(41, 1.0)], header={**HEADER, "turns": 500})
            batch(root, "notes", [seats(51, 1.0)])
            # A row with no time, or a time that is not one, is not a game.
            batch(root, "20261118T000000Z-ffff", [seats(61, 0.0), seats(62, float("nan")), [{"kind": "game", "seed": 63, "turn": 9}]])
            snap = performance_page.snapshot(root)

        self.assertEqual([b["batch"] for b in snap["batches"]], ["20261101T000000Z-aaaa", "20261120T000000Z-bbbb", "20261121T000000Z-cccc"])
        first, second, third = snap["batches"]
        self.assertEqual((first["games"], first["began"], first["ended"], first["commit"]), (3, "2026-11-01", "2026-11-02", "39c314e77"))
        self.assertEqual(first["p"], [400.0, 400.0, 500.0, 600.0, 600.0])
        self.assertEqual(second["fastest"], {"secs": 20.0, "turn": 100, "victory": "culture"})
        self.assertEqual((second["turns"], second["a_turn"]), (200, 0.4))
        self.assertNotIn("spans", second)

        windows = snap["windows"]
        self.assertEqual(list(windows), ["latest", "7", "all"])
        # The newest finished batch alone.
        self.assertEqual((windows["latest"]["batches"], windows["latest"]["games"]), (["20261121T000000Z-cccc"], 2))
        self.assertEqual(windows["latest"]["p"], [60.0, 60.0, 60.0, 90.0, 90.0])
        # Seven days counted back from its last game: the batch of the 1st is out.
        self.assertEqual(windows["7"]["batches"], ["20261120T000000Z-bbbb", "20261121T000000Z-cccc"])
        self.assertEqual((windows["7"]["games"], windows["7"]["from"], windows["7"]["to"]), (5, "2026-11-20", "2026-11-21"))
        self.assertEqual(windows["7"]["p"], [20.0, 20.0, 80.0, 120.0, 120.0])
        self.assertEqual(windows["7"]["fastest"], {"secs": 20.0, "turn": 100, "victory": "culture", "batch": "20261120T000000Z-bbbb"})
        self.assertEqual((windows["all"]["games"], windows["all"]["slowest"]), (8, 600.0))
        # A window's games are counted in spans of time for its chart.
        self.assertEqual(windows["latest"]["spans"], {"35": 1, "39": 1})
        self.assertEqual(sum(windows["all"]["spans"].values()), 8)

        # The file holds a line per window and per batch, and reads back whole.
        text = performance_page.written(snap)
        self.assertEqual(json.loads(text), snap)
        self.assertEqual(len([line for line in text.splitlines() if line.startswith('{"batch"')]), 3)
        self.assertEqual(len([line for line in text.splitlines() if line.startswith('  "')]), 3)

    def test_the_committed_snapshot_is_whole(self):
        snap = json.loads((ROOT / performance_page.SELF_PLAY).read_text(encoding="utf-8"))
        self.assertEqual(snap["percentiles"], performance_page.PERCENTILES)
        self.assertEqual(snap["spans_per_tenfold"], performance_page.SPANS_PER_TENFOLD)
        self.assertEqual(list(snap["windows"]), performance_page.WINDOWS)
        names = [b["batch"] for b in snap["batches"]]
        self.assertEqual(names, sorted(names))
        self.assertTrue(all(performance_page.BATCH.fullmatch(name) for name in names))
        for held in [*snap["batches"], *snap["windows"].values()]:
            self.assertEqual(len(held["p"]), 5)
            self.assertEqual(held["p"], sorted(held["p"]))
            self.assertLessEqual(held["fastest"]["secs"], held["p"][0])
            self.assertLessEqual(held["p"][4], held["slowest"])
            self.assertGreater(held["games"], 0)
        for window in snap["windows"].values():
            self.assertEqual(sum(window["spans"].values()), window["games"])
            self.assertTrue(set(window["batches"]) <= set(names))
        self.assertEqual(snap["windows"]["latest"]["batches"], [names[-1]])
        self.assertEqual(snap["windows"]["all"]["games"], sum(b["games"] for b in snap["batches"]))


class RecordVideos(unittest.TestCase):
    def test_the_videos_are_youtube_links_the_fastest_first(self):
        listed = [
            {"video": "https://www.youtube.com/watch?v=C4wAsdenSpI", "seconds": 52, "time": "52 seconds", "posted": "2026-11-11", "title": "A full match"},
            {"video": "https://youtu.be/iHlUWPIqr5g?t=2", "seconds": 6, "time": "under 6 seconds", "posted": "2026-11-21", "title": "Under six"},
            {"video": "https://www.youtube.com/watch?v=2K2mHKYZ9Rg&list=PLPI7k77hCAR4", "seconds": 14, "posted": "2026-11-13", "title": "Earlier the same day"},
            {"video": "https://www.youtube.com/watch?v=9c8m87e3rCg", "seconds": 14, "posted": "2026-11-14", "title": "Later"},
            # Not a YouTube link, no time, and not an entry at all: left out.
            {"video": "https://example.com/watch?v=iHlUWPIqr5g", "seconds": 1},
            {"video": "https://www.youtube.com/watch?v=ub-F9qgqLAA"},
            {"video": "https://www.youtube.com/watch?v=ub-F9qgqLAA", "seconds": True},
            "https://www.youtube.com/watch?v=ub-F9qgqLAA",
        ]
        with contextlib.redirect_stderr(io.StringIO()) as said:
            videos = performance_page.videos(listed)
        self.assertEqual([v["id"] for v in videos], ["iHlUWPIqr5g", "9c8m87e3rCg", "2K2mHKYZ9Rg", "C4wAsdenSpI"])
        self.assertEqual(videos[0]["video"], "https://www.youtube.com/watch?v=iHlUWPIqr5g")
        self.assertEqual((videos[0]["time"], videos[1]["time"]), ("under 6 seconds", "14 seconds"))
        self.assertEqual(said.getvalue().count("left out"), 4)
        self.assertEqual(performance_page.videos(None), [])

    def test_the_committed_videos_are_all_carried_and_the_record_is_the_fastest(self):
        listed = json.loads((ROOT / performance_page.VIDEOS).read_text(encoding="utf-8"))["videos"]
        with contextlib.redirect_stderr(io.StringIO()) as said:
            videos = performance_page.videos(listed)
        self.assertEqual(said.getvalue(), "")
        self.assertEqual(len(videos), len(listed))
        self.assertEqual(len({v["id"] for v in videos}), len(videos))
        self.assertEqual(videos[0]["seconds"], min(v["seconds"] for v in listed))
        for video in videos:
            self.assertTrue(video["title"] and video["time"])
            self.assertRegex(video["posted"], r"^\d{4}-\d{2}-\d{2}$")


class TheSpeedup(unittest.TestCase):
    """The page's first part (operator 2026-11-22: "at the top of civvis
    performance we want to calculate a multiplier on the theoretical and
    experimetnal throughput of the simualtor. and the calculations. baeline
    is 5 hrs for manually played game. please add the deity computer control,
    deity self-play, civvis self-play. how much has civvis self-play speed up
    over my manual 5 hr games? add this info, maybe in tables to the top")."""

    SECTION = PAGE[PAGE.index('<section class="res-section" id="perf-speedup"'):PAGE.index('<section class="res-section" id="perf-history"')]

    def test_the_record_carries_the_worked_tables_of_the_home_page_s_ledger(self):
        # The computer control stop reads its benchmark's record from the
        # run ledger where the clone holds it, and a bare copy of the
        # readings holds none: both are read here as a clone without it.
        held = speedup_timeline.control_times.control
        speedup_timeline.control_times.control = lambda source: None
        self.addCleanup(setattr, speedup_timeline.control_times, "control", held)
        with contextlib.redirect_stderr(io.StringIO()):
            record = performance_page.performance(ROOT)
        self.assertEqual(record["speedup"], speedup_timeline.worked(ROOT))
        self.assertEqual(record["speedup"]["answer"]["all"], speedup_timeline.timeline(ROOT)["speedup"])
        # A revision with the readings and nothing else still has its page.
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()) as said:
            root = pathlib.Path(scratch)
            (root / "data/performance").mkdir(parents=True)
            for name in ("speedup_timeline.json", "games_per_minute.json", "processors.json"):
                (root / "data/performance" / name).write_text((ROOT / "data/performance" / name).read_text(encoding="utf-8"), encoding="utf-8")
            alone = performance_page.performance(root)
            # Without the list of processors the two tables stand as they
            # were and the third is not drawn, nor the cloud's cost, which
            # is worked out from the measured processor's score.
            (root / speedup_timeline.PROCESSORS).unlink()
            unlisted = performance_page.performance(root)
        self.assertEqual((alone["speedup"], alone["windows"], alone["videos"]), (record["speedup"], {}, []))
        self.assertNotIn("no speedup readings", said.getvalue())
        self.assertTrue(record["speedup"]["processors"]["rows"])
        self.assertTrue(record["speedup"]["cloud"]["rows"])
        bare = {key: value for key, value in record["speedup"].items() if key not in ("processors", "cloud")}
        bare["goals"] = [{key: value for key, value in goal.items() if key != "cost"} for goal in bare["goals"]]
        self.assertEqual(unlisted["speedup"], bare)
        # And one without them says so and shows the rest.
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()) as said:
            root = pathlib.Path(scratch)
            (root / "data/performance").mkdir(parents=True)
            (root / performance_page.VIDEOS).write_text((ROOT / performance_page.VIDEOS).read_text(encoding="utf-8"), encoding="utf-8")
            without = performance_page.performance(root)
        self.assertIsNone(without["speedup"])
        self.assertTrue(without["videos"])
        self.assertIn("no speedup readings", said.getvalue())

    def test_it_is_the_first_part_of_the_page_and_shows_only_when_filled(self):
        record = PAGE[PAGE.index('<div class="res-record" id="perf-record" hidden>'):]
        # Under the computer control chart, the page's top chart since
        # 2026-11-24 (`control_times_test.py`). After the tables come each way
        # of playing over time and the path through one game.
        self.assertEqual(re.findall(r'<section class="res-section" id="([\w-]+)"', record),
                         ["perf-control", "perf-speedup", "perf-history", "perf-path", "perf-video", "perf-times", "perf-batches"])
        self.assertIn('<section class="res-section" id="perf-speedup" aria-labelledby="perf-speedup-title" hidden>', PAGE)
        self.assertIn('document.getElementById("perf-speedup").hidden = false;', PAGE)
        # The sentence, the tiles, one game at a time, the machine, the
        # same benchmarks processor by processor, then what renting the
        # rest of the way to the goals is estimated to cost.
        order = ['id="perf-speed-lede"', 'id="perf-speed-tiles"', 'id="perf-games-table"', 'id="perf-throughput"', 'id="perf-throughput-table"',
                 'id="perf-processors"', 'id="perf-processors-table"', 'id="perf-cloud"', 'id="perf-cloud-table"', 'id="perf-from"']
        places = [self.SECTION.index(mark) for mark in order]
        self.assertEqual(places, sorted(places))
        self.assertIn("One machine, every core: experimental and theoretical throughput", self.SECTION)
        self.assertIn("Theoretical is a calculation, not a run", self.SECTION)

    def test_every_way_of_playing_in_the_readings_has_its_name(self):
        ways = dict(re.findall(r'^      (\w+): \["([^"]+)", "[^"]+"\],$', PAGE, flags=re.M))
        shown = speedup_timeline.worked(ROOT)
        # Every stop with a row of its own, and the every-core stop, which
        # the charts of each way of playing over time name.
        self.assertEqual(list(ways), [row["key"] for row in shown["games"]] + ["all_cores"])
        # The operator's own words for the stops (2026-11-24: "human player
        # 5 hrs, Firaxis Computer control ..., Firaxis self-play ..., CIVVIS
        # self-play, "Parallel games, M5 Max, all 18 cores""). The game
        # against simulated Deity rivals is not one of them and says so.
        self.assertEqual((ways["hand"], ways["control"], ways["firaxis_self_play"], ways["self_play"], ways["all_cores"]),
                         ("Human player", "Firaxis computer control", "Firaxis self-play", "CIVVIS self-play", "Parallel games"))
        self.assertEqual(ways["deity_sim"], "CIVVIS against Deity rivals")
        self.assertIn('if (row.worked) name.firstChild.after(" ", el("span", "perf-kind is-estimate", "Worked out, not timed"));', PAGE)
        # Every kind of throughput row, and every basis of a theory, is worded.
        self.assertIn('const KINDS = { measured: "Experimental", theory: "Theoretical", goal: "Goal" };', PAGE)
        told = PAGE[PAGE.index("const TOLD = {"):PAGE.index("const through =")]
        for row in shown["throughput"]:
            self.assertIn(f"\n        {row.get('basis') or row['kind']}: (row) =>", told)
        # The multiplier is the first figure beside the name in both tables.
        # The third, processor by processor, has its head written with its
        # rows (a column for each one-core way of playing).
        heads = re.findall(r"<thead>(.*?)</thead>", self.SECTION, flags=re.S)
        self.assertEqual(len(heads), 4)
        for head in heads[:2]:
            self.assertEqual(re.findall(r"<th[^>]*>([^<]+)</th>", head)[1], "Faster than by hand")
            self.assertIn('<th scope="col" class="perf-sum">The calculation</th>', head)
        self.assertEqual(heads[2], "")
        # The fourth is the cloud's cost: what is rented, the speedup it
        # adds, the machines, the sum, and the price for an hour and a year.
        self.assertEqual(re.findall(r"<th[^>]*>([^<]+)</th>", heads[3]),
                         ["Rented", "Speedup it adds", "Machines", "The calculation", "An hour, list price", "An hour, spot price", "A year, list price"])

    def test_the_benchmarks_stand_processor_by_processor_under_the_machine(self):
        # Operator 2026-11-22: "Add in a theoretical 9980, performance and
        # m5 ultra, m6. we should have a table in civvis performance that
        # has our benchmarks (and age) across each processor. we should
        # also list the single thread score and multithread score from the
        # website".
        self.assertIn('<div class="res-kind" id="perf-processors" hidden>', self.SECTION)
        self.assertIn('document.getElementById("perf-processors").hidden = false;', PAGE)
        self.assertIn('<table class="rank-table perf-sum-table perf-proc-table" id="perf-processors-table">', self.SECTION)
        self.assertIn("Theoretical is a calculation from two scores, not a run.", self.SECTION)
        script = PAGE[PAGE.index("// The same benchmarks, processor by processor"):PAGE.index("// How each figure was measured: the reading's own account.")]
        # The multiplier beside the name as in the tables above, the run it
        # comes from, each one-core way of playing, his two scores, the age.
        self.assertIn('["Processor", "Faster than by hand", "Games a minute, every core"].forEach(column);', script)
        self.assertIn('for (const key of played) column(way(key) + ", one core");', script)
        self.assertIn('["Single thread score", "Multithread score", "Benchmark age"].forEach(column);', script)
        # A row is a run or a calculation, said beside its name in the words
        # of the table above, and a figure worked out has its sum under it.
        self.assertIn('el("span", "perf-kind is-" + kind, KINDS[kind])', script)
        self.assertIn('figured(every.a_minute, every.kind === "theory" ? every.sum : "counted in a run")', script)
        self.assertIn('tr.append(figured(game.time, game.kind === "theory" ? game.sum : "measured"));', script)
        # The age is counted on the day the page is read, from the day of
        # the oldest reading the row rests on.
        self.assertIn("Math.floor((Date.now() - then) / DAY)", script)
        self.assertIn('age.append(el("b", "", old(row.taken))', script)
        # A score links the website's page it was read on, and an address
        # from the slot is checked for its shape before it is one.
        self.assertIn("if (!ADDRESS.test(String(address))) return document.createTextNode(words);", script)
        self.assertIn("single.append(linked(text(row.single_thread) || \"—\", row.page));", script)
        self.assertIn("multi.append(linked(text(row.multithread) || \"—\", row.page));", script)
        for row in speedup_timeline.worked(ROOT)["processors"]["rows"]:
            self.assertRegex(row["page"], r"^https://[\w.-]+/[\w/.?=&+%-]*$")
        # The website says for itself how a score is read, with the other
        # accounts of how a figure was measured.
        self.assertIn('const name = el("dt", "", "Single thread and multithread scores");', PAGE)
        self.assertIn('from.append(name, el("dd", "", text(site.from)));', PAGE)

    def test_the_goals_are_tiles_and_rows_and_the_cloud_s_cost_says_whose_estimate_it_is(self):
        # Operator 2026-11-22: "Addd 1 billion x to the scaling scale as
        # well. we should also estimate the cloud cost in $ to fill i nthe
        # remaining amount (note that this is a Fable 5.2 best estimate of
        # the numbers)".
        shown = speedup_timeline.worked(ROOT)
        # The billion went on 2026-11-24: "Change the timeline to just scale
        # to 1,000,000x (drop the 1bX)."
        self.assertEqual([goal["words"] for goal in shown["goals"]], ["1 million x"])
        # A rented machine is taken to reach the share of its cores our own
        # every-core run reached, and the words over the table say so.
        self.assertIn('(cloud.scaling ? ". A machine is taken to reach " + text(cloud.scaling) + " of its cores times that, the share our own every-core run reached: "', PAGE)
        script = PAGE[PAGE.index("// ---- How many times faster than a game played by hand"):PAGE.index("// ---- Each way of playing over time")]
        # A tile for every goal, with what renting the rest of the way costs.
        self.assertIn("for (const goal of goals) {", script)
        self.assertIn('tile("Goal", text(goal.words || goal.times), "a game every " + text(goal.time),', script)
        self.assertIn('(goal.cost ? ". Renting the rest: about " + text(goal.cost) + " an hour." : "")', script)
        self.assertIn(".perf-speed-tiles { grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); }", PAGE)
        # A goal of the whole line is a row of the throughput table.
        self.assertIn('row.whole ? text(row.times) + " in all" : "A game every " + text(row.one_game)', script)
        # The cloud's cost is hidden until the slot has it, opens on whose
        # estimate it is, and no name is written in the page: it is the
        # slot's.
        self.assertIn('<div class="res-kind" id="perf-cloud" hidden>', self.SECTION)
        self.assertIn("The rest of the way, rented: an estimate of the cloud cost", self.SECTION)
        self.assertIn('el("b", "", "A best estimate by " + text(cloud.by) + (cloud.taken ? ", " + day(cloud.taken) : "") + "."),', script)
        self.assertIn('" Not a quote and not a run: nothing has been played in a cloud. "', script)
        self.assertIn('document.getElementById("perf-cloud").hidden = false;', script)
        written = re.sub(r"(?<!:)//.*", "", re.sub(r"<!--.*?-->", "", self.SECTION + script, flags=re.S))
        self.assertIn("A best estimate by", written)
        self.assertNotIn("Fable", written)
        # Each figure of a row has its sum under it, and the estimate gives
        # its own account with the others.
        for cell in ("summed(row.rest, row.rest_sum, \"perf-times\")", "summed(row.an_hour, row.an_hour_sum)", "summed(row.spot_an_hour, row.spot_an_hour_sum)",
                     'el("td", "perf-sum", text(row.machines_sum))', 'el("td", "perf-sum", text(cloud.times_sum))'):
            self.assertIn(cell, script)
        self.assertIn('from.append(name, el("dd", "", text(cloud.from)));', script)
        self.assertEqual(shown["cloud"]["by"], "Claude Fable 5.2")

    def test_no_figure_is_written_into_the_page(self):
        # Every one is the slot's, so the page cannot say a figure the
        # readings do not, and cannot disagree with the home page's line.
        text = html.unescape(re.sub(r"<[^>]+>", " ", re.sub(r"<!--.*?-->", "", self.SECTION, flags=re.S)))
        self.assertIsNone(re.search(r"\d", text), re.findall(r".{0,20}\d.{0,20}", text)[:3])
        script = PAGE[PAGE.index("// ---- How many times faster than a game played by hand"):PAGE.index("// ---- Each way of playing over time")]
        for unsafe in ("innerHTML", "insertAdjacentHTML", "outerHTML"):
            self.assertNotIn(unsafe, script)
        self.assertIsNone(re.search(r"\d[\d,.]*\s*(x|s|min|h)\b", re.sub(r"//.*", "", script)))


class EachWayOverTime(unittest.TestCase):
    """Operator 2026-11-24: "in civvis performance, we should have charts
    studying the improvements rates of each of these over time"."""

    SECTION = PAGE[PAGE.index('<section class="res-section" id="perf-history"'):PAGE.index('<section class="res-section" id="perf-path"')]
    SCRIPT = PAGE[PAGE.index("// ---- Each way of playing over time"):PAGE.index("// ---- The path through one game")]

    def test_the_record_carries_every_stop_s_readings_and_pace(self):
        shown = performance_page.performance(ROOT)
        self.assertEqual(shown["history"], speedup_timeline.history(ROOT))
        self.assertEqual([row["key"] for row in shown["history"]["rows"]], ["control", "firaxis_self_play", "self_play", "all_cores"])
        with tempfile.TemporaryDirectory() as scratch:
            # A tree with nothing to read has no charts and no page.
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertIsNone(performance_page.performance(pathlib.Path(scratch)))

    def test_it_is_a_small_chart_for_each_stop_shown_only_when_filled(self):
        self.assertIn('<section class="res-section" id="perf-history" aria-labelledby="perf-history-title" hidden>', PAGE)
        self.assertIn('<div class="perf-trends" id="perf-trends"></div>', self.SECTION)
        self.assertIn('document.getElementById("perf-history").hidden = false;', self.SCRIPT)
        # One chart a stop, each over its own run of days and on its own
        # scale of seconds; never two scales on one chart.
        self.assertIn("for (const row of trends) {", self.SCRIPT)
        self.assertIn("const from = earliest - (earliest === latest ? DAY_MS : 0), to = latest + (earliest === latest ? DAY_MS : 0);", self.SCRIPT)
        self.assertIn("Math.log10(s)", self.SCRIPT)
        # Two lines on a chart are told apart by a key as well as colour,
        # and the second colour is the page's own second, not a new one.
        self.assertIn('const COLOURS = { readings: "var(--series)", days: "var(--series)", tournament: "var(--series-2)", hours: "var(--series-3)", production: "var(--series-3)" };', self.SCRIPT)
        self.assertIn("if (row.lines.length > 1) {", self.SCRIPT)
        self.assertIn("--series-2: #d95926;", PAGE)
        self.assertIn("--series-3: #199e70;", PAGE)
        # Every kind of line the record can carry has its name.
        for kind in ("readings", "days", "tournament", "hours", "production"):
            self.assertRegex(self.SCRIPT, rf'\b{kind}: "[A-Z][^"]+"')
        # The pace is said in words, the pointer reads any reading out, and
        # every reading is in a table for whoever cannot use the chart.
        for words in ('" faster"', '" slower"', '"Steady over "', '"One reading so far."', '", halving every "'):
            self.assertIn(words, self.SCRIPT)
        self.assertIn("showTip(event, day(near.taken),", self.SCRIPT)
        self.assertIn('<table class="rank-table" id="perf-history-table">', self.SECTION)
        # A reading worked out and not timed is a ring and says so.
        self.assertIn('if (newest.worked) now.append(el("small", "", "worked out, not timed"));', self.SCRIPT)
        # The figure over a chart is the stop's own, as the home page has it.
        self.assertIn("newest = row.now && row.now.time ? row.now : own.points[own.points.length - 1];", self.SCRIPT)
        self.assertIn('dot.style.fill = point.worked ? "var(--panel-raised)" : colour;', self.SCRIPT)

    def test_no_figure_is_written_into_the_page(self):
        text = html.unescape(re.sub(r"<[^>]+>", " ", re.sub(r"<!--.*?-->", "", self.SECTION, flags=re.S)))
        self.assertIsNone(re.search(r"\d", text), re.findall(r".{0,20}\d.{0,20}", text)[:3])
        for unsafe in ("innerHTML", "insertAdjacentHTML", "outerHTML"):
            self.assertNotIn(unsafe, self.SCRIPT)


class ThePath(unittest.TestCase):
    """Operator 2026-11-24: "we should try to reduce the path of civvis
    through civ 6 down to the computatoinal irreducability and should have a
    chart tracking the size/num of steps"."""

    SECTION = PAGE[PAGE.index('<section class="res-section" id="perf-path"'):PAGE.index('<section class="res-section" id="perf-video"')]
    SCRIPT = PAGE[PAGE.index("// ---- The path through one game"):PAGE.index("// ---- The fastest game on video.")]

    READING = {"taken": "2026-11-24", "build": "f55bc3543685cfb88b8728435e03957d8f708792", "games": 3, "turns": 250.0, "steps": 1000, "bytes": 40000,
               "hops": 700, "walks": 400, "merged": 700, "instructions": 2000, "replay_instructions": 500,
               "kinds": {"move": 700, "end_turn": 100, "fortify": 60, "produce": 50, "ranged": 30, "attack": 20, "research": 15, "civic": 15, "buy": 10}}

    def book(self, scratch: str, *readings: dict) -> pathlib.Path:
        source = pathlib.Path(scratch)
        (source / performance_page.PATH).parent.mkdir(parents=True, exist_ok=True)
        (source / performance_page.PATH).write_text(json.dumps({"readings": list(readings)}), encoding="utf-8")
        return source

    def test_a_reading_is_its_steps_and_what_a_walk_as_one_step_would_save(self):
        older = dict(self.READING, taken="2026-11-20", steps=1200, hops=800, walks=500)
        with tempfile.TemporaryDirectory() as scratch:
            shown = performance_page.path(self.book(scratch, self.READING, older))
        # Oldest first, whatever order the ledger has them in.
        self.assertEqual([row["taken"] for row in shown["readings"]], ["2026-11-20", "2026-11-24"])
        newest = shown["readings"][-1]
        self.assertEqual((newest["steps"], newest["kept"], newest["saved"], newest["bytes"], newest["build"]), (1000, 700, 300, 40000, "f55bc35436"))
        self.assertEqual(newest["kept"] + newest["saved"], newest["steps"])
        self.assertEqual((newest["instructions"], newest["replay_instructions"], newest["replay_share"]), (2000, 500, 0.25))

    def test_the_newest_reading_s_steps_are_listed_by_kind_and_the_rest_together(self):
        with tempfile.TemporaryDirectory() as scratch:
            shown = performance_page.path(self.book(scratch, self.READING))
        kinds = shown["kinds"]
        self.assertEqual(len(kinds), performance_page.PATH_KINDS + 1)
        # The moves' bar is in its two parts: the walks, and the further
        # hexes of a walk already begun.
        self.assertEqual(kinds[0], {"kind": "move", "steps": 700, "kept": 400, "saved": 300})
        self.assertEqual([row["steps"] for row in kinds[:-1]], sorted((row["steps"] for row in kinds[:-1]), reverse=True))
        self.assertEqual(kinds[-1], {"rest": True, "kinds": 2, "steps": 25})
        self.assertEqual(sum(row["steps"] for row in kinds), 1000)

    def test_a_ledger_that_cannot_be_read_or_does_not_add_up_shows_nothing(self):
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(performance_page.path(pathlib.Path(scratch)))
            self.assertIsNone(performance_page.path(self.book(scratch)))
            self.assertIsNone(performance_page.path(self.book(scratch, dict(self.READING, walks=900))))
            without = dict(self.READING)
            del without["instructions"]
            shown = performance_page.path(self.book(scratch, without))
            self.assertNotIn("replay_share", shown["readings"][0])

    def test_the_committed_readings_reach_the_page(self):
        shown = performance_page.performance(ROOT)["path"]
        book = json.loads((ROOT / performance_page.PATH).read_text(encoding="utf-8"))
        self.assertEqual(len(shown["readings"]), len(book["readings"]))
        newest = shown["readings"][-1]
        self.assertGreater(newest["steps"], newest["kept"])
        self.assertLess(newest["replay_instructions"], newest["instructions"])
        self.assertIn("docs/PATH_STEPS.md", book["about"])
        self.assertTrue((ROOT / "docs/PATH_STEPS.md").is_file())
        self.assertTrue((ROOT / "tools/path_steps.py").is_file())

    def test_it_is_tiles_a_column_for_every_reading_and_the_steps_by_kind(self):
        self.assertIn('<section class="res-section" id="perf-path" aria-labelledby="perf-path-title" hidden>', PAGE)
        order = ['id="perf-path-tiles"', 'id="perf-path-keys"', 'id="perf-path-plot"', 'id="perf-path-steps"', 'id="perf-path-table"', 'id="perf-path-note"']
        places = [self.SECTION.index(mark) for mark in order]
        self.assertEqual(places, sorted(places))
        self.assertIn('document.getElementById("perf-path").hidden = false;', self.SCRIPT)
        # Blue is the steps that stay and orange the steps a walk as one
        # step would save, in the columns and in the bars alike.
        self.assertIn('const KEYS = [["Steps that stay", ""], ["Steps a walk as one step could save", "is-second"]];', self.SCRIPT)
        # The saving is an upper bound (a walk that doubles back cannot be
        # one go-to), and the tile says so.
        self.assertIn('"up to " + part(newest.saved / newest.steps) + " fewer"', self.SCRIPT)
        self.assertIn('upper.style.fill = "var(--series-2)";', self.SCRIPT)
        self.assertIn('const parts = k.saved ? [[k.kept, ""], [k.saved, "is-second"]] : [[k.steps, ""]];', self.SCRIPT)
        # A column stands on nought, and the pointer reads a reading out.
        self.assertIn("y = (v) => m.t + (1 - v / ceiling) * (height - m.t - m.b);", self.SCRIPT)
        self.assertIn('showTip(event, day(r.taken) + (r.build ? " · build " + r.build : ""), [', self.SCRIPT)
        for link in ("data/performance/path_steps.json", "docs/PATH_STEPS.md", "docs/CORE_STATE.md"):
            self.assertIn(f'href="https://github.com/MartinHalvorson/CIVVIS/blob/main/{link}"', self.SECTION)

    def test_no_figure_is_written_into_the_page(self):
        text = html.unescape(re.sub(r"<[^>]+>", " ", re.sub(r"<!--.*?-->", "", self.SECTION, flags=re.S)))
        self.assertIsNone(re.search(r"\d", text), re.findall(r".{0,20}\d.{0,20}", text)[:3])
        for unsafe in ("innerHTML", "insertAdjacentHTML", "outerHTML"):
            self.assertNotIn(unsafe, self.SCRIPT)


class ThePage(unittest.TestCase):
    def test_the_page_carries_one_empty_slot_and_publishing_fills_it(self):
        self.assertEqual(len(performance_page.SLOT.findall(PAGE)), 1)
        self.assertIn('<script type="application/json" id="performance-data">null</script>', PAGE)
        with contextlib.redirect_stderr(io.StringIO()):
            record = performance_page.performance(ROOT)
        self.assertEqual(list(record["windows"]), performance_page.WINDOWS)
        self.assertTrue(record["batches"] and record["videos"])
        filled = performance_page.fill(PAGE, record)
        held = json.loads(performance_page.SLOT.search(filled)[2])
        self.assertEqual(held, record)
        # No name or sentence can close the script element.
        risky = performance_page.fill(PAGE, {"videos": [{"title": "</script><b>"}]})
        self.assertNotIn("</script><b>", risky)
        self.assertIsNone(performance_page.fill("<html></html>", record))

    def test_without_figures_the_page_says_so(self):
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()) as said:
            root = pathlib.Path(scratch)
            self.assertIsNone(performance_page.performance(root))
            page = root / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            self.assertEqual(performance_page.main(["performance_page.py", str(root), str(page)]), 0)
            self.assertIn('id="performance-data">null</script>', page.read_text(encoding="utf-8"))
        self.assertIn("no self-play times", said.getvalue())
        self.assertIn("no record videos", said.getvalue())
        self.assertIn('<p class="res-empty" id="perf-empty">This copy of the page has no figures.', PAGE)
        self.assertIn('<div class="res-record" id="perf-record" hidden>', PAGE)
        # Each part shows only when its own figures arrived.
        for part in ("perf-control", "perf-video", "perf-times", "perf-batches"):
            self.assertRegex(PAGE, rf'<section class="res-section" id="{part}" aria-labelledby="{part}-title" hidden>')

    def test_a_snapshot_taken_with_other_percentiles_is_not_drawn(self):
        with tempfile.TemporaryDirectory() as scratch, contextlib.redirect_stderr(io.StringIO()) as said:
            root = pathlib.Path(scratch)
            snap = json.loads((ROOT / performance_page.SELF_PLAY).read_text(encoding="utf-8"))
            snap["percentiles"] = [5, 25, 50, 75, 95]
            (root / performance_page.SELF_PLAY).parent.mkdir(parents=True)
            (root / performance_page.SELF_PLAY).write_text(json.dumps(snap), encoding="utf-8")
            (root / performance_page.VIDEOS).write_text((ROOT / performance_page.VIDEOS).read_text(encoding="utf-8"), encoding="utf-8")
            record = performance_page.performance(root)
        self.assertEqual((record["windows"], record["batches"]), ({}, []))
        self.assertTrue(record["videos"])
        self.assertIn("rebuild it", said.getvalue())

    def test_the_page_draws_from_data_and_trusts_none_of_it_as_markup(self):
        for unsafe in ("innerHTML", "insertAdjacentHTML", "document.write", "outerHTML"):
            self.assertNotIn(unsafe, PAGE)
        # A video's id and a build's commit become part of an address, so
        # each is checked for its shape first.
        self.assertIn("const VIDEO_ID = /^[\\w-]{11}$/;", PAGE)
        self.assertIn(".filter((v) => v && VIDEO_ID.test(v.id));", PAGE)
        self.assertIn("const COMMIT = /^[0-9a-f]{7,40}$/;", PAGE)
        self.assertIn('if (!COMMIT.test(String(commit))) return document.createTextNode("—");', PAGE)

    def test_the_video_stays_on_youtube_until_its_picture_is_pressed(self):
        # Operator 2026-11-21: "website is a fast and light surface layer ...
        # store the big stuff elsewhere". The page as it loads asks YouTube
        # for one picture; the player is made by the press.
        self.assertNotIn("<iframe", PAGE)
        self.assertNotIn("<video", PAGE)
        self.assertNotIn("youtube-nocookie", MARKUP)
        press = PAGE[PAGE.index('poster.addEventListener("click"'):]
        self.assertLess(press.index('el("iframe")'), press.index("poster.replaceWith(frame);"))
        # It loops for as long as the page is open (operator 2026-11-23:
        # "please have the vido on the website all loop indefinitely").
        self.assertIn('"https://www.youtube-nocookie.com/embed/" + record.id + "?autoplay=1&rel=0&playsinline=1&loop=1&playlist=" + record.id;', press)
        # A press that asks for a new tab is the link it is.
        self.assertIn("if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey || event.button) return;", press)
        self.assertIn('<a class="perf-poster" id="perf-poster" target="_blank" rel="noreferrer">', PAGE)
        self.assertIn("poster.href = watch(record);", PAGE)

    def test_the_three_parts_stand_in_order_with_one_control_over_the_times(self):
        order = ['id="perf-video"', 'id="perf-times"', 'id="perf-window"', 'id="perf-tiles"', 'id="perf-spans-plot"', 'id="perf-batches"', 'id="perf-batches-plot"', 'id="perf-batches-table"']
        places = [PAGE.index(mark) for mark in order]
        self.assertEqual(places, sorted(places))
        # The page opens on the newest finished batch.
        buttons = re.findall(r'<button type="button" data-window="(\w+)" aria-pressed="(\w+)">([^<]+)</button>', PAGE)
        self.assertEqual(buttons, [("latest", "true", "Latest batch"), ("7", "false", "Last 7 days"), ("all", "false", "Every game")])
        self.assertEqual([key for key, _, _ in buttons], performance_page.WINDOWS)
        # One blue for every mark, and what a time is and is not is said.
        self.assertIn("--series: #3987e5;", PAGE)
        self.assertIn("A tournament&#8217;s times, not a benchmark&#8217;s.", PAGE)


class TheSiteList(unittest.TestCase):
    def test_performance_stands_under_charts_on_this_page_ringed(self):
        # Operator 2026-11-21: "a submenu appears under civvis 'Performance'
        # only when CIVVIS is selected", then "performance should be a page
        # similar to the home/ and other similar pages"; 2026-11-22: "Move
        # performance from under civvis to charts. Charts is where almost
        # all our charts can go. This keeps AI Debugger as a clean tool.
        # CIVVIS as a clean tool."
        nav = PAGE[PAGE.index('<nav class="site-nav"'):PAGE.index("</nav>")]
        self.assertEqual(
            re.findall(r'<a class="([^"]+)" href="([^"]+)"[^>]*>([^<]+)</a>', nav),
            [
                ("site-link", "../../home/", "Home"),
                ("site-link", "../civ-ai-interpretability/", "Charts/"),
                ("site-link sub-link", "../civ-ai-interpretability/", "Civ AI Interpretability"),
                ("site-link sub-link", "../civ-analytics/", "Civ Analytics"),
                ("site-link sub-link", "../win-analytics/", "Win Analytics"),
                ("site-link sub-link", "../elo-rankings/", "ELO Rankings"),
                ("site-link sub-link", "./", "CIVVIS Performance"),
                ("site-link", "../../tournaments/", "Tournaments"),
                ("site-link", "../../", "CIVVIS"),
                ("site-link", "../../?mode=strange", "Doctor Strange's Civ 6"),
                ("site-link", "../../?mode=play", "AI Debugger"),
                ("site-link", "https://store.steampowered.com/app/289070/Sid_Meiers_Civilization_VI/", "Civ 6"),
            ],
        )
        self.assertEqual(nav.count('aria-current="page"'), 1)
        self.assertIn('<a class="site-link sub-link" href="./" aria-current="page">CIVVIS Performance</a>', nav)
        self.assertIn("  .site-folder:hover > .site-menu,\n  .site-folder:focus-within > .site-menu,\n  .site-folder.is-open > .site-menu { display: grid; }\n", PAGE)
        self.assertIn("<title>CIVVIS Performance · CIVVIS.ai</title>", PAGE)
        # Every link of the page stays in its lane.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])

    def test_charts_drops_its_pages_down_on_every_page(self):
        # Charts/ is a folder that drops down to its pages (operator
        # 2026-11-24: "charts/ can drop down folder style and the user can
        # pick the right chart"), on Home and Tournaments as on its own
        # pages. Until then a branch was folded unless one of its pages was
        # on screen. Its order is the operator's of the same day: "move ai
        # interpretablity to the top of the charts/ then civ analytics, win
        # analytics. elo rankings.. civvis performanc". The viewer (CIVVIS and
        # AI Debugger, the two tools) still lists none of them.
        self.assertIn(
            '<a class="site-link sub-link" href="./" aria-current="page">Win Analytics</a>\n'
            '          <a class="site-link sub-link" href="../elo-rankings/" ',
            RESEARCH,
        )
        self.assertIn('          <a class="site-link sub-link" href="../civvis-performance/" ', RESEARCH)
        for page in (LANDING, TOURNAMENTS):
            menu = page[page.index('<div class="site-menu">'):]
            menu = menu[:menu.index("</div>")]
            self.assertEqual(
                re.findall(r'<a class="site-link sub-link" href="\.\./charts/([^"]+)/"[^>]*>([^<]+)</a>', menu),
                [("civ-ai-interpretability", "Civ AI Interpretability"), ("civ-analytics", "Civ Analytics"), ("win-analytics", "Win Analytics"),
                 ("elo-rankings", "ELO Rankings"), ("civvis-performance", "CIVVIS Performance")],
            )
        self.assertNotIn("Performance</a>", VIEWER)
        for name in (">Win Analytics</a>", ">ELO Rankings</a>", ">Civ Analytics</a>"):
            self.assertNotIn(name, VIEWER)
        for gone in ("perflink", "under-watch", "opens-watching", "sub-link"):
            self.assertNotIn(gone, VIEWER)

    def test_publishing_builds_the_page_into_a_lane_whose_revision_has_it(self):
        # The lane's own page, filled by the lane's own script from the
        # lane's own record videos and snapshot (it was head's script and
        # head's record in both lanes until 2026-11-22).
        self.assertNotIn('"$repo_root/beta/performance', PUBLISH)
        for line in (
            'performance_page="$source_tree/beta/performance.html"',
            'if [ -f "$performance_page" ]; then',
            # At the address the page states itself (operator 2026-11-23:
            # "clean up the URL please. standardized."), which is its name
            # in the site list, under Charts/; `performance` is where a
            # revision from before pages said so had it.
            'performance_at="$(address "$performance_page" performance)"',
            'mkdir -p "$out/$performance_at"',
            'cp "$performance_page" "$out/$performance_at/index.html"',
            'fill performance_page.py "$out/$performance_at/index.html"',
            'pages+=("$performance_at")',
        ):
            self.assertIn(line, PUBLISH)
        self.assertIn('<link rel="canonical" href="https://civvis.ai/charts/civvis-performance/">', PAGE)
        verify = (BETA / "verify.py").read_text(encoding="utf-8")
        self.assertIn("""    "charts/civvis-performance": b'id="performance-data"',\n""", verify)
        self.assertIn("""    "performance": "charts/civvis-performance",\n""", verify)


if __name__ == "__main__":
    unittest.main()
