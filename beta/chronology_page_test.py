#!/usr/bin/env python3
"""Project Chronology: the project's milestones along a timeline.

Operator, 2026-11-28: "add a project chronology to charts/ at the end for
now. post the important milestones along a timeline. show the progress we've
made. ai capability, simulator performance, new features, other".

The page (`beta/chronology.html`, `/charts/project-chronology/`) is the last
under Charts/. Its milestones are a file somebody keeps by hand,
`data/chronology.json`, written into the page's slot by
`beta/chronology_page.py` when the site is published. A list kept by hand is
right the day it is written, so this holds it three ways:

- every line to its shape (`TheFile`), with the script's own check;
- every milestone that restates a file of this repository to that file
  (`TheFacts`): the first wins the ladder record keeps, the record videos,
  the self-play batches, the games-per-minute runs, the tournaments. A
  figure read off the ledger branch cannot be read again from this tree, and
  the file says so and names the day it was read;
- and the page to what was asked for (`ThePage`, `TheRecord`).

    python3 beta/chronology_page_test.py
"""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import tempfile
import unittest

import chronology_page
import verify

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "chronology.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")
DATA = json.loads((ROOT / "data" / "chronology.json").read_text(encoding="utf-8"))
MILESTONES = DATA["milestones"]

#: The four categories, in the operator's words and order.
CATEGORIES = [
    ("capability", "AI capability"),
    ("performance", "Simulator performance"),
    ("features", "New features"),
    ("other", "Other"),
]


def load(path: str):
    return json.loads((ROOT / path).read_text(encoding="utf-8"))


def citing(kind: str, ref) -> list[dict]:
    """The milestones that name this source."""
    return [milestone for milestone in MILESTONES
            if any(source["kind"] == kind and source["ref"] == ref for source in milestone["sources"])]


def one(title: str) -> dict:
    found = [milestone for milestone in MILESTONES if milestone["title"] == title]
    assert len(found) == 1, f"{len(found)} milestones are titled {title!r}"
    return found[0]


def script() -> str:
    start = PAGE.index("<script>", PAGE.index('id="chronology-data"'))
    return PAGE[start:PAGE.index("</script>", start)]


class TheFile(unittest.TestCase):
    def test_every_line_is_in_shape(self):
        self.assertEqual(chronology_page.problems(DATA), [])

    def test_the_categories_are_the_four_asked_for_in_that_order(self):
        self.assertEqual([(category["key"], category["name"]) for category in DATA["categories"]], CATEGORIES)
        for key, name in CATEGORIES:
            with self.subTest(category=name):
                self.assertTrue([milestone for milestone in MILESTONES if milestone["category"] == key])

    def test_the_milestones_are_in_date_order_and_none_is_twice(self):
        days = [milestone["date"] for milestone in MILESTONES]
        self.assertEqual(days, sorted(days))
        titles = [milestone["title"] for milestone in MILESTONES]
        self.assertEqual(len(titles), len(set(titles)))

    def test_a_milestone_is_a_title_and_a_sentence_or_two(self):
        for milestone in MILESTONES:
            with self.subTest(milestone=milestone["title"]):
                self.assertLessEqual(len(milestone["title"]), 60)
                self.assertLessEqual(len(milestone["detail"]), 330)
                self.assertTrue(milestone["detail"].endswith("."))

    def test_the_timeline_names_only_a_few_and_names_them_shortly(self):
        major = [milestone for milestone in MILESTONES if milestone.get("major")]
        self.assertTrue(2 <= len(major) <= 8, len(major))
        for milestone in major:
            with self.subTest(milestone=milestone["title"]):
                self.assertLessEqual(len(milestone["label"]), 22)
        # No two named ones share a row within three weeks: their names
        # would meet on a phone.
        for key, _ in CATEGORIES:
            days = sorted(chronology_page.day(milestone["date"]) for milestone in major if milestone["category"] == key)
            for earlier, later in zip(days, days[1:]):
                self.assertGreaterEqual((later - earlier).days, 21, key)

    def test_a_file_it_names_is_in_the_tree(self):
        named = sorted({source["ref"] for milestone in MILESTONES for source in milestone["sources"] if source["kind"] == "data"})
        # Discovered, so an empty list fails rather than passes.
        self.assertGreaterEqual(len(named), 5)
        for path in named:
            with self.subTest(path=path):
                self.assertTrue((ROOT / path).is_file())

    def test_it_says_when_the_ledger_was_read(self):
        self.assertRegex(DATA["checked"], r"results\.jsonl.*2026-\d\d-\d\dT\d\d:\d\d:\d\dZ")
        self.assertTrue(any("checked" in line and "ledger" in line for line in DATA["about"]))

    def test_the_progress_row_restates_milestones(self):
        self.assertEqual(chronology_page.PROGRESS, ("category", "label", "was", "now", "span"))
        self.assertTrue(2 <= len(DATA["progress"]) <= 4)
        text = " ".join(milestone["title"] + " " + milestone["detail"] for milestone in MILESTONES)
        for figure in DATA["progress"]:
            with self.subTest(figure=figure["label"]):
                for reading in (figure["was"], figure["now"]):
                    # "1 in 340" is worded "1 win in 340" in its milestone,
                    # so a reading is looked for number by number.
                    for number in re.findall(r"[\d,.]+", reading):
                        self.assertIn(number, text)


class TheFacts(unittest.TestCase):
    """A milestone that restates a file of this repository agrees with it."""

    def test_the_first_wins_are_the_ladder_records(self):
        wins = load("docs/civ6_ladder.json")["wins"]
        self.assertEqual(sorted(wins), sorted(f"DIFFICULTY_{name}" for name in (
            "SETTLER", "CHIEFTAIN", "WARLORD", "PRINCE", "KING", "EMPEROR")))
        for difficulty, win in wins.items():
            with self.subTest(difficulty=difficulty):
                found = citing("ledger", win["tag"])
                self.assertEqual(len(found), 1, win["tag"])
                milestone = found[0]
                self.assertEqual(milestone["category"], "capability")
                day = win["utc"][:10]
                # Its own day, or, where one milestone tells several wins,
                # the day said in its sentence.
                self.assertTrue(milestone["date"] == day or f"({day[5:]})" in milestone["detail"], day)
                self.assertGreaterEqual(milestone["date"], day)
                level = chronology_page_level(difficulty)
                self.assertIn(level, milestone["title"] + " " + milestone["detail"])
                if win["turns"] < 250:
                    self.assertIn(f"turn {win['turns']}", milestone["detail"])
                else:
                    self.assertIn("250 turns", milestone["detail"])

    def test_the_record_videos_are_the_performance_pages(self):
        videos = {video["video"]: video for video in load("data/performance/fastest_game_videos.json")["videos"]}
        cited = [(milestone, source) for milestone in MILESTONES for source in milestone["sources"] if source["kind"] == "video"]
        self.assertTrue(cited)
        for milestone, source in cited:
            with self.subTest(milestone=milestone["title"]):
                video = videos[source["ref"]]
                self.assertEqual(milestone["date"], video["posted"])
                self.assertIn(str(video["seconds"]), milestone["title"])
                self.assertEqual(milestone["category"], "performance")
        seconds = [video["seconds"] for video in videos.values()]
        figure = next(figure for figure in DATA["progress"] if figure["label"] == "Fastest whole game on video")
        self.assertEqual(figure["was"], f"{max(seconds)} s")
        self.assertEqual(figure["now"], f"under {min(seconds)} s")
        # The records its sentences name on the way down.
        by = {video["seconds"]: video["posted"] for video in videos.values()}
        self.assertIn("14 seconds two days later", one("A whole game in 52 seconds, on video")["detail"])
        self.assertEqual((chronology_page.day(by[14]) - chronology_page.day(by[52])).days, 2)
        self.assertIn(f"9 seconds on {by[9][5:]}", one("A whole game in under 6 seconds")["detail"])

    def test_the_self_play_times_are_the_batches(self):
        batches = load("data/performance/self_play_times.json")["batches"]
        first, newest = batches[0], batches[-1]
        base = one("The baseline: ten minutes a self-play game")
        self.assertEqual(base["date"], first["ended"])
        for said in (f"{first['games']:,} ", f"{first['began'][5:]} to {first['ended'][5:]}", f"{round(first['p'][2])} s", f"{first['a_turn']:.1f} s a turn"):
            self.assertIn(said, base["detail"])
        faster = one("A self-play game in under two and a half minutes")
        batch = next(batch for batch in batches if batch["ended"] == faster["date"] and f"{batch['games']:,} " in faster["detail"])
        self.assertLess(batch["p"][2], 150)
        for said in (f"{batch['began'][5:]} to {batch['ended'][5:]}", f"{round(batch['p'][2])} s", f"{batch['a_turn']:.2f} s a turn",
                     f"{first['p'][2] / batch['p'][2]:.1f} times faster"):
            self.assertIn(said, faster["detail"])
        latest = one("Games per minute: 18 on one machine")
        self.assertEqual(latest["date"], newest["ended"])
        self.assertIn(f"from {newest['began'][5:]} to {newest['ended'][5:]}, the median game takes {round(newest['p'][2])} s", latest["detail"])
        figure = next(figure for figure in DATA["progress"] if figure["label"] == "A self-play game, the median")
        self.assertEqual((figure["was"], figure["now"]), (f"{round(first['p'][2])} s", f"{round(newest['p'][2])} s"))
        self.assertIn(f"{first['began']} to {first['ended'][5:]}", figure["span"])
        self.assertIn(f"{newest['began'][5:]} to {newest['ended'][5:]}", figure["span"])

    def test_the_games_per_minute_are_the_runs(self):
        runs = load("data/performance/games_per_minute.json")["runs"]
        first, best = runs[0], max(runs, key=lambda run: run["games_per_minute"])
        began = one("Games per minute: 18 on one machine")
        self.assertEqual(began["date"], first["taken"][:10])
        self.assertIn(f"{first['games_per_minute']:.1f} on an {first['machine']['cores']}-core M5 Max", began["detail"])
        self.assertIn("M5 Max", first["machine"]["chip"])
        top = one("24 games a minute: 35,000 a day")
        self.assertEqual(top["date"], best["taken"][:10])
        self.assertIn(f"{best['games_per_minute']:.1f} games a minute", top["detail"])
        self.assertEqual(round(best["games_per_minute"] * 60 * 24, -3), 35000)
        # Against a game by hand, the speed timeline's own reading.
        hand = next(stop for stop in load("data/performance/speedup_timeline.json")["stops"] if stop["key"] == "hand")
        by_hand = hand["readings"][-1]["seconds"]
        self.assertEqual(by_hand, 5 * 3600)
        self.assertIn(f"{round(by_hand / (60 / best['games_per_minute'])):,} times", top["detail"])
        self.assertIn("five-hour game", top["detail"])

    def test_the_tournaments_are_the_tournaments_pages(self):
        tournaments = load("data/tournaments.json")["tournaments"]
        first = tournaments[0]
        began = one("Tournaments begin")
        stamp = re.search(r"(\d{4})(\d{2})(\d{2})T", first["start"])
        self.assertEqual(began["date"], "-".join(stamp.groups()))
        self.assertEqual(first["series_size"], 10)
        self.assertIn("ten games", began["detail"])
        self.assertEqual(first["match"]["difficulty"], "DIFFICULTY_DEITY")
        benchmark = [tournament for tournament in tournaments if tournament.get("benchmark")]
        self.assertEqual(len(benchmark), 1)
        started = one("The official benchmark starts")
        tag = next(source["ref"] for source in started["sources"] if source["kind"] == "ledger")
        self.assertTrue(tag.startswith(benchmark[0]["start"]))
        self.assertEqual(started["date"], "-".join(re.search(r"(\d{4})(\d{2})(\d{2})T", tag).groups()))
        eighth = next(tournament for tournament in tournaments if tournament["start"] == "civvis-t8-")
        self.assertEqual(eighth["series_size"], 100)
        self.assertIn("no Diplomatic victory", eighth["conditions"])
        even = one("Deity at even odds")
        self.assertIn("a hundred Deity games with the Diplomatic victory off, ends 21 to 79", even["detail"])

    def test_the_spans_of_days_it_states_add_up(self):
        day = chronology_page.day
        first_win = one("First win against Civilization VI's own AI")
        deity = one("First win at Lv 8 Deity")
        first_run = one("CIVVIS takes a seat in the real Civilization VI")
        self.assertIn(f"{(day(deity['date']) - day(first_win['date'])).days} days after the first win at Lv 1", deity["detail"])
        self.assertIn(f"{(day(first_win['date']) - day(first_run['date'])).days} days after the first run", first_win["detail"])
        figure = next(figure for figure in DATA["progress"] if figure["label"] == "Highest level beaten in Civilization VI")
        self.assertEqual(figure["span"], f"{(day(deity['date']) - day(first_win['date'])).days} days: {first_win['date']} to {deity['date'][5:]}")
        start, last = MILESTONES[0], one("Pull request 6,000")
        self.assertEqual(start["title"], "The project starts")
        self.assertIn(f"{(day(last['date']) - day(start['date'])).days} days in", last["detail"])


def chronology_page_level(difficulty: str) -> str:
    """`DIFFICULTY_KING` as the milestones word it: `Lv 5 King`."""
    names = ["Settler", "Chieftain", "Warlord", "Prince", "King", "Emperor", "Immortal", "Deity"]
    name = difficulty.removeprefix("DIFFICULTY_").title()
    return f"Lv {names.index(name) + 1} {name}"


class TheRecord(unittest.TestCase):
    def setUp(self):
        self.record = chronology_page.record(ROOT)

    def test_it_is_the_file_counted_and_spanned(self):
        record = self.record
        self.assertEqual(len(record["milestones"]), len(MILESTONES))
        self.assertEqual((record["from"], record["to"]), (MILESTONES[0]["date"], MILESTONES[-1]["date"]))
        self.assertEqual(record["days"], (chronology_page.day(record["to"]) - chronology_page.day(record["from"])).days)
        self.assertEqual([(category["key"], category["name"]) for category in record["categories"]], CATEGORIES)
        self.assertEqual(sum(category["count"] for category in record["categories"]), len(MILESTONES))
        self.assertEqual(record["progress"], [{field: figure[field] for field in chronology_page.PROGRESS} for figure in DATA["progress"]])
        self.assertEqual(record["checked"], DATA["checked"])
        self.assertNotIn("about", record)

    def test_only_a_major_milestone_carries_a_label(self):
        for row in self.record["milestones"]:
            with self.subTest(milestone=row["title"]):
                self.assertEqual("major" in row, "label" in row)
                self.assertEqual(set(row) - {"major", "label"}, {"date", "category", "title", "detail", "sources"})

    def test_a_source_is_worded_and_opens_where_it_can(self):
        source = chronology_page.source
        repo = "https://github.com/MartinHalvorson/CIVVIS"
        self.assertEqual(source({"kind": "pr", "ref": 541}), {"kind": "pr", "label": "#541", "href": f"{repo}/pull/541"})
        self.assertEqual(source({"kind": "commit", "ref": "67c2dd9"}), {"kind": "commit", "label": "67c2dd9", "href": f"{repo}/commit/67c2dd9"})
        self.assertEqual(source({"kind": "release", "ref": "v0.6.0"}), {"kind": "release", "label": "v0.6.0", "href": f"{repo}/releases/tag/v0.6.0"})
        self.assertEqual(source({"kind": "data", "ref": "data/tournaments.json"}),
                         {"kind": "data", "label": "tournaments.json", "href": f"{repo}/blob/main/data/tournaments.json"})
        self.assertEqual(source({"kind": "video", "ref": "https://www.youtube.com/watch?v=iHlUWPIqr5g", "says": "the game"}),
                         {"kind": "video", "label": "the game", "href": "https://www.youtube.com/watch?v=iHlUWPIqr5g"})
        # A game has no address of its own: it is printed as its tag.
        self.assertEqual(source({"kind": "ledger", "ref": "lad-7"}), {"kind": "ledger", "label": "game lad-7"})
        self.assertEqual(source({"kind": "ledger", "ref": "results.jsonl", "says": "the ledger's results"}),
                         {"kind": "ledger", "label": "the ledger's results"})
        for row in self.record["milestones"]:
            for found in row["sources"]:
                self.assertTrue(found["label"])
                self.assertTrue(found.get("href", "https://").startswith("https://"))

    def test_a_line_out_of_shape_is_said(self):
        def wrong(change) -> list[str]:
            data = json.loads(json.dumps(DATA))
            change(data)
            return chronology_page.problems(data)

        self.assertTrue(wrong(lambda data: data["milestones"][3].update(date="2026-7-22")))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(date="2026-13-01")))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(category="speed")))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(detail=" ")))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(sources=[])))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(major=True)))  # no label
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(sources=[{"kind": "pr", "ref": "541"}])))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(sources=[{"kind": "commit", "ref": "main"}])))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(sources=[{"kind": "video", "ref": "https://example.com/v", "says": "it"}])))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(sources=[{"kind": "data", "ref": "docs/<b>.md"}])))
        self.assertTrue(wrong(lambda data: data["milestones"][3].update(sources=[{"kind": "wiki", "ref": "x"}])))
        self.assertTrue(wrong(lambda data: data["milestones"].reverse()))
        self.assertTrue(wrong(lambda data: data["categories"].append(dict(data["categories"][0]))))
        self.assertTrue(wrong(lambda data: data["progress"][0].pop("span")))
        self.assertTrue(wrong(lambda data: data.pop("milestones")))
        self.assertEqual(wrong(lambda data: None), [])

    def test_a_file_it_cannot_use_costs_the_page_its_record_and_not_the_publish(self):
        with tempfile.TemporaryDirectory() as scratch:
            tree = pathlib.Path(scratch)
            said = io.StringIO()
            with contextlib.redirect_stderr(said):
                self.assertIsNone(chronology_page.record(tree))
            self.assertIn("cannot be read", said.getvalue())
            (tree / "data").mkdir()
            broken = json.loads(json.dumps(DATA))
            broken["milestones"][0]["date"] = "yesterday"
            (tree / "data" / "chronology.json").write_text(json.dumps(broken), encoding="utf-8")
            said = io.StringIO()
            with contextlib.redirect_stderr(said):
                self.assertIsNone(chronology_page.record(tree))
            self.assertIn("The project starts", said.getvalue())
            page = tree / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            with contextlib.redirect_stderr(io.StringIO()):
                self.assertEqual(chronology_page.main(["chronology_page.py", str(tree), str(page)]), 0)
            self.assertIn('<script type="application/json" id="chronology-data">null</script>', page.read_text(encoding="utf-8"))

    def test_it_is_written_into_the_slot_and_cannot_close_it(self):
        filled = chronology_page.fill(PAGE, self.record)
        slot = chronology_page.SLOT.search(filled)
        self.assertEqual(json.loads(slot[2]), self.record)
        unsafe = json.loads(json.dumps(self.record))
        unsafe["milestones"][0]["title"] = "</script><script>alert(1)</script>"
        text = chronology_page.SLOT.search(chronology_page.fill(PAGE, unsafe))[2]
        self.assertNotIn("<", text)
        self.assertEqual(json.loads(text), unsafe)
        self.assertIsNone(chronology_page.fill("<p>no slot</p>", self.record))


class ThePage(unittest.TestCase):
    def test_it_is_the_last_page_under_charts_and_ringed(self):
        self.assertIn("<title>Project Chronology · CIVVIS.ai</title>", PAGE)
        self.assertIn('<link rel="canonical" href="https://civvis.ai/charts/project-chronology/">', PAGE)
        nav = PAGE[PAGE.index('<nav class="site-nav"'):PAGE.index("</nav>")]
        menu = nav[nav.index('<div class="site-menu">'):nav.index("</div>", nav.index('<div class="site-menu">'))]
        names = re.findall(r'<a class="site-link sub-link" href="([^"]+)"[^>]*>([^<]+)</a>', menu)
        self.assertEqual(names[-1], ("./", "Project Chronology"))
        self.assertEqual(
            [name for _, name in names],
            ["AI Insights", "Civ AI Interpretability", "Civ Analytics", "Win Analytics", "ELO Rankings",
             "CIVVIS Performance", "Unreviewed Gen AI Charts", "Project Chronology"],
        )
        self.assertIn('<a class="site-link sub-link" href="./" aria-current="page">Project Chronology</a>', nav)
        # Charts/ itself opens the folder's first page, as on every page.
        self.assertIn('<a class="site-link" href="../ai-insights/" ', nav)

    def test_every_other_page_lists_it_last_with_the_same_words(self):
        said = 'title="The project&#8217;s milestones along a timeline: AI capability, simulator performance, new features.">Project Chronology</a>\n        </div>'
        sources = sorted(set(re.findall(r'_page="\$source_tree/beta/([a-z_]+\.html)"', PUBLISH)))
        listing = [source for source in sources if "Project Chronology</a>" in (BETA / source).read_text(encoding="utf-8")]
        # Discovered: Home, Tournaments and the other seven pages of Charts/.
        self.assertGreaterEqual(len(listing), 10)
        for source in listing:
            if source == "chronology.html":
                continue
            with self.subTest(page=source):
                self.assertIn(said, (BETA / source).read_text(encoding="utf-8"))

    def test_it_opens_on_one_line(self):
        main = PAGE[PAGE.index('<main class="shell">'):PAGE.index("</main>")]
        self.assertEqual(main.count('<p class="intro">'), 1)
        intro = main[main.index('<p class="intro">'):main.index("</p>")]
        self.assertLess(len(re.sub(r"\s+", " ", re.sub(r"<[^>]+>", "", intro))), 220)

    def test_a_copy_with_no_record_says_so_in_one_sentence(self):
        self.assertIn('<script type="application/json" id="chronology-data">null</script>', PAGE)
        self.assertIn('<p class="chr-empty" id="chr-empty">', PAGE)
        self.assertIn('<div class="chr-record" id="chr-record" hidden>', PAGE)
        code = script()
        self.assertIn('document.getElementById("chr-empty").hidden = true;', code)
        self.assertIn('document.getElementById("chr-record").hidden = false;', code)

    def test_it_has_the_figures_the_switches_the_timeline_and_the_list_in_that_order(self):
        main = PAGE[PAGE.index('<main class="shell">'):PAGE.index("</main>")]
        order = [main.index(mark) for mark in (
            'id="chr-tiles"', 'id="chr-cats"', 'id="chr-order"', 'id="chr-svg"', 'id="chr-months"', 'id="chr-note"')]
        self.assertEqual(order, sorted(order))
        # One row of switches over everything they scope, none in a card.
        filters = main[main.index('<div class="chr-filters">'):main.index('<section class="chr-card"')]
        self.assertEqual(filters.count('class="chr-seg"'), 2)
        self.assertIn('<button type="button" data-order="oldest" aria-pressed="true">Oldest first</button>', filters)
        self.assertIn('<button type="button" data-order="newest" aria-pressed="false">Newest first</button>', filters)

    def test_the_slot_is_data_and_never_markup(self):
        code = script()
        for unsafe in ("innerHTML", "outerHTML", "insertAdjacentHTML", "document.write", "eval("):
            self.assertNotIn(unsafe, code)
        self.assertIn("textContent", code)
        # An address becomes a link only when it is https.
        self.assertIn('source.href.startsWith("https://")', code)
        self.assertIn('link.rel = "noreferrer";', code)

    def test_the_categories_wear_three_hues_and_the_grey(self):
        root = PAGE[PAGE.index("  :root {"):PAGE.index("  }", PAGE.index("  :root {"))]
        for token in ("--c-capability: #3987e5;", "--c-performance: #d95926;", "--c-features: #199e70;", "--c-other: #93a3b5;"):
            self.assertIn(token, root)
        for key, _ in CATEGORIES:
            self.assertIn(f"  .chr-{key} {{ --c: var(--c-{key}); }}\n", PAGE)
        # Text never wears a category's colour: only a swatch and a dot do.
        style = PAGE[PAGE.index("<style>"):PAGE.index("</style>")]
        wearing = re.findall(r"^\s*([^{}\n]+)\{[^}]*var\(--c[,)][^}]*\}", style, re.M)
        self.assertEqual(sorted(name.strip() for name in wearing), [".chr-dot", ".chr-swatch"])

    def test_a_dot_is_a_link_to_its_line_with_room_to_hit(self):
        code = script()
        self.assertIn('href: `#chr-m-${milestone.at}`', code)
        self.assertIn("item.id = `chr-m-${milestone.at}`;", code)
        self.assertIn('svg("circle", { class: "chr-hit", cx: dot.cx, cy: dot.cy, r: 12 })', code)
        self.assertIn('mark.setAttribute("aria-label", `${milestone.date}, ${lane.category.name}: ${milestone.title}`);', code)
        for event in ('"pointerenter"', '"focus"', '"pointerleave"', '"blur"'):
            self.assertIn(f"mark.addEventListener({event}", code)

    def test_publish_places_it_fills_it_and_the_check_knows_it(self):
        self.assertIn('chronology_page="$source_tree/beta/chronology.html"', PUBLISH)
        self.assertIn('chronology_at="$(address "$chronology_page" charts/project-chronology)"', PUBLISH)
        self.assertIn('  fill chronology_page.py "$out/$chronology_at/index.html"\n  pages+=("$chronology_at")', PUBLISH)
        self.assertIn("  without+=(project-chronology)", PUBLISH)
        # Last of Charts/ in the lane's list, after Unreviewed Gen AI Charts.
        self.assertLess(PUBLISH.index('pages+=("$random_at")'), PUBLISH.index('pages+=("$chronology_at")'))
        self.assertEqual(verify.PAGES["charts/project-chronology"], b'id="chronology-data"')
        self.assertIn(verify.PAGES["charts/project-chronology"], PAGE.encode())


if __name__ == "__main__":
    unittest.main()
