#!/usr/bin/env python3
"""The Performance page: the fastest game on video and how long a self-play game takes."""

from __future__ import annotations

import contextlib
import datetime
import io
import json
import os
import pathlib
import re
import tempfile
import unittest

import performance_page

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
        for part in ("perf-video", "perf-times", "perf-batches"):
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
        self.assertIn('"https://www.youtube-nocookie.com/embed/" + record.id + "?autoplay=1&rel=0&playsinline=1"', press)
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
        self.assertIn("These are a tournament&#8217;s times, not a benchmark&#8217;s", PAGE)


class TheSiteList(unittest.TestCase):
    def test_performance_stands_under_charts_on_this_page_in_gold(self):
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
                ("site-link", "../home/", "Home"),
                ("site-link", "../research/", "Charts/"),
                ("site-link sub-link", "../research/", "Win Analytics"),
                ("site-link sub-link", "./", "CIVVIS Performance"),
                ("site-link sub-link", "../interpretability/", "AI Interpretability"),
                ("site-link", "../tournaments/", "Auto-Tourneys"),
                ("site-link", "../", "CIVVIS"),
                ("site-link", "../?mode=play", "AI Debugger"),
                ("site-link", "https://store.steampowered.com/app/289070/Sid_Meiers_Civilization_VI/", "Civ 6"),
            ],
        )
        self.assertEqual(nav.count('aria-current="page"'), 1)
        self.assertIn('<a class="site-link sub-link" href="./" aria-current="page">CIVVIS Performance</a>', nav)
        self.assertIn(".site-link.sub-link { margin-left: 10px; padding: 2px 6px 3px; font-size: 10px; letter-spacing: 0.03em; }", PAGE)
        self.assertIn("<title>CIVVIS Performance · CIVVIS.ai</title>", PAGE)
        # Every link of the page stays in its lane.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])

    def test_only_charts_lists_the_branch(self):
        # A branch is folded unless one of its pages is on screen: Win
        # Analytics, the page Charts/ opens, lists every page of Charts/
        # under it as this page does, and Home, Auto-Tourneys and the viewer
        # (CIVVIS and AI Debugger, the two tools) list none of them.
        self.assertIn(
            '<a class="site-link sub-link" href="./" aria-current="page">Win Analytics</a>\n'
            '        <a class="site-link sub-link" href="../performance/" ',
            RESEARCH,
        )
        for page in (LANDING, TOURNAMENTS, VIEWER):
            self.assertNotIn("Performance</a>", page.replace(">CIVVIS Performance</a></figcaption>", ""))
            self.assertNotIn(">Win Analytics</a>", page)
        for gone in ("perflink", "under-watch", "opens-watching", "sub-link"):
            self.assertNotIn(gone, VIEWER)

    def test_publishing_builds_the_page_into_every_lane(self):
        for line in (
            'performance_page="$source_tree/beta/performance.html"',
            '[ -f "$performance_page" ] || performance_page="$repo_root/beta/performance.html"',
            'mkdir -p "$out/performance"',
            'cp "$performance_page" "$out/performance/index.html"',
            'python3 "$repo_root/beta/performance_page.py" "$repo_root" "$out/performance/index.html"',
            "('href=\"/performance\"', 'href=\"performance/\"'),",
            'out / "performance" / "index.html": [],',
        ):
            self.assertIn(line, PUBLISH)
        verify = (BETA / "verify.py").read_text(encoding="utf-8")
        self.assertIn('f"{lane}performance/index.html",', verify)
        self.assertIn("""b'id="performance-data"' not in get("/performance/").read()""", verify)
        self.assertIn("""b'id="performance-data"' not in get("/test/performance/").read()""", verify)


if __name__ == "__main__":
    unittest.main()
