#!/usr/bin/env python3
"""AI Insights: what the Civ games show about AI at large, an insight at a
time. The first: the games against Civilization VI's own AI are over sooner
at each level from Lv 5 King to Lv 8 Deity.

    python3 beta/insights_page_test.py
"""

from __future__ import annotations

import contextlib
import io
import json
import pathlib
import re
import subprocess
import tempfile
import unittest

import home_rankings
import insights_page
import research_page
import verify

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
PAGE = (BETA / "insights.html").read_text(encoding="utf-8")
RESEARCH = (BETA / "research.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")
VIEWER = (ROOT / "web" / "index.html").read_text(encoding="utf-8")

DIFFICULTY = {5: "DIFFICULTY_KING", 6: "DIFFICULTY_EMPEROR", 7: "DIFFICULTY_IMMORTAL", 8: "DIFFICULTY_DEITY"}


def run(tag: str, outcome: str, turns: int, victory: str | None = None, **more) -> dict:
    """One line of the results file, as `civ6_ladder.result_row` writes it."""
    return {
        "tag": tag,
        "game": re.sub(r"-cont\d+$", "", tag),
        "utc": "2026-11-21T10:00:00Z",
        "civ": "CIVILIZATION_ROME",
        "difficulty": "DIFFICULTY_DEITY",
        "players": 6,
        "map": "Continents.lua",
        "outcome": outcome,
        "victory": victory,
        "turns": turns,
        **more,
    }


def ledger(scratch: str, rows: list[dict], short: tuple = ()) -> pathlib.Path:
    """A clone whose ledger holds these result lines, every run played at
    Online speed to 250 turns but the ones tagged in `short`, played to 170."""
    repo = pathlib.Path(scratch)
    repo.mkdir(parents=True, exist_ok=True)
    git = lambda *args: subprocess.run(["git", "-C", scratch, *args], check=True, capture_output=True, text=True)
    git("init", "-q")
    (repo / "results.jsonl").write_text("".join(json.dumps(row) + "\n" for row in rows), encoding="utf-8")
    for row in rows:
        (repo / "runs" / row["tag"]).mkdir(parents=True)
        summary = {"speed": "GAMESPEED_ONLINE", "max_turns": 170 if row["tag"] in short else 250}
        (repo / "runs" / row["tag"] / "summary.json").write_text(json.dumps(summary), encoding="utf-8")
    git("add", "-A")
    git("-c", "user.name=t", "-c", "user.email=t@t", "commit", "-qm", "ledger")
    git("update-ref", "refs/remotes/origin/ledger", "HEAD")
    return repo


#: A small ledger where the stronger the AI, the sooner the games end.
ROWS = [
    # Lv 8 Deity: four games, one of them CIVVIS's.
    run("civvis-20261120T100000Z", "rival_victory", 180, "VICTORY_TECHNOLOGY"),
    run("civvis-20261120T110000Z", "rival_victory", 184, "VICTORY_TECHNOLOGY"),
    run("civvis-20261121T100000Z", "win", 171, "VICTORY_RELIGIOUS"),
    run("civvis-20261122T100000Z", "rival_victory", 190, "VICTORY_CULTURE"),
    # No victory, so no turn: left out, as on Win Analytics.
    run("civvis-20261122T110000Z", "defeat", 120),
    # Played to a 170-turn limit: left out.
    run("civvis-20261122T120000Z", "rival_victory", 170, "VICTORY_SCORE"),
    # Lv 7 Immortal: two games.
    run("civvis-20261119T100000Z", "rival_victory", 196, "VICTORY_TECHNOLOGY", difficulty=DIFFICULTY[7]),
    run("civvis-20261119T110000Z", "rival_victory", 199, "VICTORY_TECHNOLOGY", difficulty=DIFFICULTY[7]),
    # Lv 6 Emperor: nobody has played it.
    # Lv 5 King: three games, two of them CIVVIS's.
    run("civvis-20261101T100000Z", "win", 207, "VICTORY_TECHNOLOGY", difficulty=DIFFICULTY[5]),
    run("civvis-20261102T100000Z", "win", 230, "VICTORY_TECHNOLOGY", difficulty=DIFFICULTY[5]),
    run("civvis-20261103T100000Z", "rival_victory", 250, "VICTORY_SCORE", difficulty=DIFFICULTY[5]),
    # Lv 4 Prince: counted on Win Analytics, and no part of this insight.
    run("civvis-20261104T100000Z", "win", 150, "VICTORY_CULTURE", difficulty="DIFFICULTY_PRINCE"),
]
SHORT = ("civvis-20261122T120000Z",)


class TheRecord(unittest.TestCase):
    def test_the_levels_are_the_four_he_named_in_his_order(self):
        # Operator 2026-11-24: "switching between lv 8, 7, 6, 5".
        self.assertEqual(insights_page.LEVELS, [8, 7, 6, 5])
        for level in insights_page.LEVELS:
            self.assertIn(level, research_page.LEVELS)

    def test_a_level_is_its_every_counted_game_and_the_turn_half_were_over_by(self):
        with tempfile.TemporaryDirectory() as scratch:
            data = insights_page.record(ledger(scratch, ROWS, SHORT))
        self.assertEqual(data["turns"], 250)
        self.assertEqual(data["victories"], research_page.VICTORIES)
        levels = data["speed"]["levels"]
        # Strongest first, and a level nobody has played is not there.
        self.assertEqual([(level["level"], level["rung"]) for level in levels], [(8, "Deity"), (7, "Immortal"), (5, "King")])
        deity, immortal, king = levels
        self.assertEqual((deity["games"], deity["won"], deity["from"], deity["to"]), (4, 1, "2026-11-20", "2026-11-22"))
        # Of 171, 180, 184 and 190, half were over by 180 (the lower middle).
        self.assertEqual(deity["median"], 180)
        # A victory type nobody won at the level is left out; one somebody
        # won holds its count on each of the game's 250 turns.
        self.assertEqual(sorted(deity["victories"]), ["culture", "religious", "science"])
        self.assertEqual(len(deity["victories"]["science"]), 250)
        self.assertEqual({turn + 1: n for turn, n in enumerate(deity["victories"]["science"]) if n}, {180: 1, 184: 1})
        self.assertEqual(sum(map(sum, deity["victories"].values())), deity["games"])
        self.assertEqual((immortal["games"], immortal["won"], immortal["median"]), (2, 0, 196))
        self.assertEqual((king["games"], king["won"], king["median"]), (3, 2, 230))
        self.assertEqual(sorted(king["victories"]), ["science", "score"])

    def test_the_games_are_win_analytics_own(self):
        # The chart is Win Analytics' "CIVVIS vs Lv 8 Deity AI" (operator:
        # "we should show the civvis vs lv 8 deity ai chart"), so a level's
        # games here are that page's every game of the level, turn for turn.
        with tempfile.TemporaryDirectory() as scratch:
            source = ledger(scratch, ROWS, SHORT)
            said = io.StringIO()
            with contextlib.redirect_stderr(said):
                there = research_page.research(source)
            here = insights_page.record(source)
        for level in here["speed"]["levels"]:
            games = there["levels"][str(level["level"])]["windows"]["all"]
            self.assertEqual(level["games"], games["games"])
            self.assertEqual((level["from"], level["to"]), (games["from"], games["to"]))
            for victory, turns in level["victories"].items():
                self.assertEqual(turns, games["victories"][victory]["turns"])
        source = (BETA / "insights_page.py").read_text(encoding="utf-8")
        self.assertIn("research_page.level_groups(rows, lambda tags: research_page.limits(source, tags))", source)
        self.assertIn("research_page.window(played.get(level) or [], None)", source)

    def test_no_ledger_is_no_record_and_never_an_error(self):
        with tempfile.TemporaryDirectory() as scratch:
            self.assertIsNone(insights_page.record(pathlib.Path(scratch)))
        with tempfile.TemporaryDirectory() as scratch:
            # A ledger with games, and none at the four levels.
            rows = [run("civvis-20261104T100000Z", "win", 150, "VICTORY_CULTURE", difficulty="DIFFICULTY_PRINCE")]
            self.assertIsNone(insights_page.record(ledger(scratch, rows)))

    def test_a_publish_says_so_when_the_turn_stops_falling_with_the_level(self):
        # The heading is a claim about the games, and the games go on being
        # played: the day a stronger level's games are not over sooner, the
        # publish log says which.
        falling = [{"level": 8, "rung": "Deity", "median": 183}, {"level": 7, "rung": "Immortal", "median": 194},
                   {"level": 5, "rung": "King", "median": 223}]
        self.assertEqual(insights_page.not_falling(falling), [])
        level = [{**falling[0], "median": 194}, *falling[1:]]
        self.assertEqual(
            insights_page.not_falling(level),
            ["Lv 8 Deity games are half over by turn 194, no sooner than Lv 7 Immortal games (turn 194)"],
        )
        rows = [
            run("civvis-20261120T100000Z", "rival_victory", 210, "VICTORY_TECHNOLOGY"),
            run("civvis-20261119T100000Z", "rival_victory", 196, "VICTORY_TECHNOLOGY", difficulty=DIFFICULTY[7]),
        ]
        with tempfile.TemporaryDirectory() as scratch:
            page = pathlib.Path(scratch) / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                self.assertEqual(insights_page.main(["insights_page.py", str(ledger(scratch + "/l", rows)), str(page)]), 0)
        self.assertEqual(
            out.getvalue(),
            "::warning::insights_page: the page says smarter AI picks up speed, and Lv 8 Deity games are half over "
            "by turn 210, no sooner than Lv 7 Immortal games (turn 196)\n",
        )

    def test_the_record_is_written_into_the_pages_slot(self):
        with tempfile.TemporaryDirectory() as scratch:
            source = ledger(scratch + "/l", ROWS, SHORT)
            page = pathlib.Path(scratch) / "index.html"
            page.write_text(PAGE, encoding="utf-8")
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                self.assertEqual(insights_page.main(["insights_page.py", str(source), str(page)]), 0)
            written = json.loads(insights_page.SLOT.search(page.read_text(encoding="utf-8"))[2])
            self.assertEqual(written, insights_page.record(source))
            # Nothing but the slot moved.
            self.assertEqual(insights_page.SLOT.sub(r"\1null\3", page.read_text(encoding="utf-8")), PAGE)
        self.assertEqual(out.getvalue(), "")
        self.assertIn("insights_page: wrote the games against the AI (Lv 8 4, Lv 7 2, Lv 5 3) into ", err.getvalue())
        # A name or a sentence cannot close the script element.
        self.assertIn("\\u003c", insights_page.fill(PAGE, {"speed": "</script>"}))
        self.assertIsNone(insights_page.fill("<p>no slot</p>", {}))
        self.assertEqual(PAGE.count('<script type="application/json" id="insights-data">null</script>'), 1)


class ThePage(unittest.TestCase):
    def test_it_is_ai_insights_at_the_address_its_name_makes(self):
        self.assertIn("<title>AI Insights · CIVVIS.ai</title>", PAGE)
        self.assertIn('<link rel="canonical" href="https://civvis.ai/charts/ai-insights/">', PAGE)
        # Every link of the page stays in its lane.
        self.assertEqual(re.findall(r'(?:href|src)="(/[^"]*)"', PAGE), [])
        # It opens on one line that names no other page: the list over it does.
        intro = PAGE[PAGE.index('<p class="intro">'):]
        intro = intro[:intro.index("</p>")]
        self.assertNotIn("<a ", intro)
        self.assertLess(len(re.sub(r"<[^>]+>", "", intro).split()), 20)

    def test_the_first_insight_says_what_he_said(self):
        # Operator 2026-11-24: "to highlight that smarter ai picks up speed.
        # this is the process of optimization decisions chosen to move fast.
        # as ai gets smarter, expect the pace of progress to speed up".
        first = PAGE[PAGE.index('<section class="ins-insight" id="ins-speed"'):]
        first = first[:first.index("</section>")]
        self.assertIn('<h2 class="ins-title" id="ins-speed-title">Smarter AI picks up speed</h2>', first)
        self.assertIn(
            '<p class="rank-sub ins-sub">Optimization is choosing the decisions that move fastest. '
            "As AI gets smarter, expect the pace of progress to speed up.</p>",
            first,
        )
        # It is the page's only insight so far, and each is a section of its own.
        self.assertEqual(PAGE.count('<section class="ins-insight"'), 1)

    def test_the_switch_lists_lv_8_7_6_5_and_opens_on_deity(self):
        # "we should show the civvis vs lv 8 deity ai chart, switching
        # between lv 8, 7, 6, 5": four rows in that order, each a button,
        # with Deity's pressed as the page opens.
        switch = PAGE[PAGE.index('<div class="ins-levels" id="ins-speed-levels"'):]
        switch = switch[:switch.index("</div>")]
        rows = re.findall(r'<button type="button" data-level="(\d)" data-rung="([A-Za-z]+)" aria-pressed="(true|false)">'
                          r'<span class="ins-level-name"><b>Lv (\d)</b>([A-Za-z]+)</span>', switch)
        self.assertEqual([int(level) for level, *_ in rows], insights_page.LEVELS)
        self.assertEqual(len(rows), switch.count("<button"))
        for level, rung, pressed, shown, name in rows:
            # A row names its level as the site does everywhere.
            self.assertEqual(rung, home_rankings.CIV6_RUNGS[int(level) - 1].title())
            self.assertEqual((shown, name), (level, rung))
            self.assertEqual(pressed, "true" if level == "8" else "false")
            self.assertEqual(research_page.level_name(int(level)), f"Lv {level} {rung}")
        # Every row has a bar, the turn it stands for and the games counted.
        for part in ("data-bar", "data-turn", "data-games"):
            self.assertEqual(switch.count(part), len(rows))
        # The chart is named as Win Analytics names it, for the level chosen.
        self.assertIn('<h3 class="ins-chart-title" id="ins-speed-chart-title" data-say="CIVVIS vs {level} AI">CIVVIS vs Lv 8 Deity AI</h3>', PAGE)
        self.assertIn('data-say="CIVVIS vs {level} AI">CIVVIS vs Lv 8 Deity AI</h2>', RESEARCH)
        # And by the name he gave that chart there the same day: "lets call
        # it: Turn Length and Victory Type Histogram of All Games".
        self.assertIn('<p class="ins-chart-kind">Turn Length and Victory Type Histogram of All Games</p>', PAGE)
        self.assertIn("<span data-title>Turn Length and Victory Type Histogram of All Games</span>", RESEARCH)
        self.assertIn("const state = { level: played[0], playing: 0 };", PAGE)

    def test_a_bar_is_the_turn_from_zero_and_the_chart_is_the_whole_game(self):
        # A row's bar is as long as its turn out of the game's 250, from
        # zero: the stronger the AI, the shorter the bar, by no trick of the
        # scale. The chart under it runs the whole game too, as Win
        # Analytics' does, and its four levels share one scale of shares.
        self.assertIn('row.querySelector("[data-bar]").style.width = "calc((100% - 76px) * " + (level.median / TURNS).toFixed(4) + ")";', PAGE)
        self.assertIn("const x = (turn) => m.l + (turn / TURNS) * (width - m.l - m.r);", PAGE)
        self.assertIn("const YMAX = Math.max(...played.flatMap((level) => Array.from({ length: SPANS }, (_, i) => total(level, i) / LEVELS[level].games)));", PAGE)
        # The turn marked on the chart is the record's, the one on the row.
        self.assertIn("median: LEVELS[level].median,", PAGE)
        self.assertIn('label.textContent = "Half over by turn " + Math.round(shown.median);', PAGE)
        self.assertIn('row.querySelector("[data-turn]").textContent = "Turn " + level.median;', PAGE)

    def test_the_plot_never_stays_on_one_level_under_anothers_name(self):
        # The plot moves between levels over half a second of frames. A
        # browser that draws none (a tab in the background) is not waited
        # for, and a reader who asked for less motion is given none.
        self.assertIn("settle = setTimeout(() => { cancelAnimationFrame(frame); shown = to; paint(); }, LASTS + 80);", PAGE)
        self.assertIn("if (still.matches) { shown = to; paint(); return; }", PAGE)
        self.assertIn('const still = matchMedia("(prefers-reduced-motion: reduce)");', PAGE)
        self.assertIn("  @media (prefers-reduced-motion: reduce) { .ins-level-bar { transition: none; } }\n", PAGE)

    def test_play_runs_the_levels_weakest_first_and_stops(self):
        # The button plays Lv 5 to Lv 8, so the endings are seen to come
        # sooner as the AI gets stronger; it is not shown with one level.
        self.assertIn('<button type="button" class="ins-play" id="ins-speed-play" aria-pressed="false" hidden>Play Lv 5 to Lv 8</button>', PAGE)
        self.assertIn("const climb = played.slice().sort((a, b) => Number(a) - Number(b));", PAGE)
        self.assertIn("if (climb.length > 1) {", PAGE)
        self.assertIn("if (state.playing) stop();", PAGE)

    def test_what_the_chart_counts_is_closed_and_says_what_a_level_is(self):
        note = PAGE[PAGE.index('<details class="fine">'):]
        note = note[:note.index("</details>")]
        self.assertIn("<summary>How it is counted</summary>", note)
        # A level of Civilization VI's AI is a stronger player, by bonuses:
        # the note says what "smarter" stands on here.
        self.assertIn("A higher level is a stronger AI: Civilization VI gives its AI bigger bonuses at each level, not a different way of deciding.", note)
        self.assertIn("the turn marked is the median: half the level&#8217;s games were over by it.", note)
        self.assertIn('<a href="../win-analytics/">The same games on Win Analytics</a>', note)
        self.assertNotIn("<details class=\"fine\" open", PAGE)

    def test_a_page_with_no_record_says_so_once(self):
        self.assertIn('<p class="ins-empty" id="ins-empty">This copy of the page has no figures. They are written in when the site is published.</p>', PAGE)
        self.assertIn('<div class="ins-record" id="ins-record" hidden>', PAGE)
        self.assertIn("if (!speed || !Array.isArray(speed.levels) || !speed.levels.length) return;", PAGE)
        # Names and figures reach the page as text, never as markup.
        script = PAGE[PAGE.index('<script type="application/json" id="insights-data">'):]
        self.assertNotIn("innerHTML", script)


class TheSiteList(unittest.TestCase):
    def test_the_page_is_the_first_under_charts_ringed(self):
        # Operator 2026-11-24: "the old ai interpretability should be in the
        # process of being renamed civ ai interpretability. we want a new ai
        # insights". It is listed first under Charts/, so Charts/ opens it.
        nav = PAGE[PAGE.index('<nav class="site-nav"'):PAGE.index("</nav>")]
        links = re.findall(r'<a class="([^"]+)" href="([^"]+)"[^>]*>([^<]+)</a>', nav)
        self.assertEqual(links[:3], [
            ("site-link", "../../home/", "Home"),
            ("site-link", "./", "Charts/"),
            ("site-link sub-link", "./", "AI Insights"),
        ])
        self.assertEqual([name for cls, _, name in links if cls == "site-link sub-link"][1:],
                         [name for cls, _, name in re.findall(
                             r'<a class="([^"]+)" href="([^"]+)"[^>]*>([^<]+)</a>',
                             RESEARCH[RESEARCH.index('<nav class="site-nav"'):RESEARCH.index("</nav>")])
                          if cls == "site-link sub-link"][1:])
        self.assertEqual(nav.count('aria-current="page"'), 1)
        self.assertIn('<a class="site-link sub-link" href="./" aria-current="page">AI Insights</a>', nav)
        # The viewer opens Charts/ at its first page too, and lists none of
        # the folder's pages.
        self.assertIn('id="chartslink" href="/charts/ai-insights" target="_top">Charts/</a>', VIEWER)
        self.assertNotIn(">AI Insights</a>", VIEWER)

    def test_the_header_is_the_one_every_page_carries(self):
        header = lambda page: re.sub(r"<!--.*?-->", "", page[page.index('<nav class="utility-nav"'):page.index("</header>")], flags=re.S)
        self.assertEqual(header(PAGE), header(RESEARCH))

    def test_publishing_builds_the_page_first_under_charts(self):
        for line in (
            'insights_page="$source_tree/beta/insights.html"',
            'if [ -f "$insights_page" ]; then',
            'insights_at="$(address "$insights_page" charts/ai-insights)"',
            'mkdir -p "$out/$insights_at"',
            'cp "$insights_page" "$out/$insights_at/index.html"',
            'fill insights_page.py "$out/$insights_at/index.html"',
            'pages+=("$insights_at")',
            "without+=(ai-insights)",
        ):
            self.assertIn(line, PUBLISH)
        self.assertNotIn('"$repo_root/beta/insights', PUBLISH)
        # `/charts` opens the folder's first page in the lane's list, which
        # is the order the pages are added in: this one before every other
        # page of Charts/.
        added = re.findall(r'pages\+=\("\$([a-z]+)_at"\)', PUBLISH)
        self.assertEqual(added[:3], ["home", "tournaments", "insights"])
        # And the publish check knows the page by the slot that is filled.
        self.assertEqual(verify.PAGES["charts/ai-insights"], b'id="insights-data"')


if __name__ == "__main__":
    unittest.main()
