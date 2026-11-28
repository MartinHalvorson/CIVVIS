#!/usr/bin/env python3
"""ELO Rankings at the top of the home page: where it stands, what a row
says, and the scale of cheating held to the engine's own ladder.

Operator, 2026-11-27: "put the elo chart at the top of home/ too, above the
benchmarks. expand the format to: Player. Scale of Cheating. ELO (Diplo Off).
ELO (Diplo On)", under it "Prince 0, King 1, Emperor 2, Immortal 3, Deity 4",
then "Mods", "combining this with what you have there".

The last class here opens the page in a browser, filled with a record, and
reads the table as it is drawn. Set `CIVVIS_TEST_CHROME` to run it with a
particular browser binary.

    python3 beta/home_elo_test.py
"""

from __future__ import annotations

import json
import os
import pathlib
import re
import shutil
import subprocess
import tempfile
import time
import unittest
import urllib.request

import elo_page
import home_rankings
from verify import Devtools, find_chrome, free_port, fresh_profile, tether

BETA = pathlib.Path(__file__).resolve().parent
ROOT = BETA.parent
LANDING = (BETA / "landing.html").read_text(encoding="utf-8")
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")


def cut(page: str, start: str, end: str) -> str:
    """The page from `start` through the first `end` after it."""
    at = page.index(start)
    return page[at:page.index(end, at) + len(end)]


#: The section as a visitor is given it, without the comment over it, and
#: the script that fills it.
SECTION = cut(LANDING, '<section class="res-block home-elo" id="elo"', "</section>")
SCRIPT = cut(LANDING, "  // ELO Rankings, over the benchmark (`#elo`)", "    card.hidden = false;\n  })();\n  </script>\n")


def rated(elo: float, margin: float, games: int, wins: int, **more) -> dict:
    return {"elo": elo, "margin": margin, "games": games, "wins": wins, **more}


def elo_record() -> dict:
    """ELO Rankings' record (`beta/elo_page.py`) as the table reads it: two
    agents, two Firaxis levels with a figure and one without, and one
    person, in each reading. Diplo off and Diplo on order them differently."""
    def reading(holy, jack, deity, king, dev):
        return {
            "strategies": {"rows": [
                {"kind": "agent", "id": "holy", "name": "HolyRoller", "tilt": "Religion", **rated(holy, 30, 400, 250)},
                {"kind": "agent", "id": "jack", "name": "JackOfAllTrades", "tilt": "Balanced", **rated(jack, 29, 400, 200)},
                {"kind": "rung", "name": "Deity", "elo": 1955},
            ], "tables": 900, "players": 4, "turns": 250, "runs": 3, "latest_run": "2026-11-27"},
            "firaxis": {"days": 7, "games": 40, "to": "2026-11-27T10:00:00Z", "levels": [
                {"level": 8, "name": "Deity", "now": rated(deity, 90, 30, 9)},
                {"level": 5, "name": "King", "now": rated(king, 150, 10, 7)},
                {"level": 4, "name": "Prince", "now": None},
            ]},
            "people": [{"name": "dev", **rated(dev, 180, 3, 2)}],
        }
    on = reading(holy=1547, jack=1155, deity=1042, king=595, dev=1210)
    on["off"] = reading(holy=1543, jack=1191, deity=1452, king=799, dev=1100)
    return on


class Page(unittest.TestCase):
    def test_elo_rankings_is_the_page_s_first_part_over_the_benchmark(self):
        main = cut(LANDING, '<main class="shell">', "</main>")
        bare = re.sub(r"<!--.*?-->", "", main, flags=re.S)
        self.assertRegex(bare, r'^<main class="shell">\s*<p class="intro">[^<]*(<cite>[^<]*</cite>[^<]*)*</p>\s*<section class="res-block home-elo" id="elo" ')
        self.assertEqual(re.findall(r"<section [^>]*\bid=\"([\w-]+)\"", main), ["elo", "bench", "watch"])
        # Hidden until its script has the record to draw.
        self.assertIn('<section class="res-block home-elo" id="elo" aria-labelledby="home-elo-title" hidden>', SECTION)
        self.assertIn("card.hidden = false;", SCRIPT)
        # Its head is the benchmark's: the title, a line, and at the right
        # the page that has the rest (how it is counted, the ratings over
        # time). No disclosure of its own.
        self.assertIn('<h2 class="res-side-title" id="home-elo-title">ELO Rankings</h2>', SECTION)
        self.assertIn('<p class="home-bench-more"><a href="../charts/elo-rankings/">Elo over time</a></p>', SECTION)
        self.assertEqual(SECTION.count('<header class="res-side-head">'), 1)
        self.assertNotIn("<details", SECTION)

    def test_the_format_is_player_scale_of_cheating_and_the_two_elos(self):
        head = cut(SECTION, '<div class="rank-row rank-head">', "</div>")
        columns = [re.sub(r"<[^>]+>", "", cell).strip()
                   for cell in re.findall(r"<(?:span|button)\b[^>]*class=\"rank-(?:col|sort)[^>]*>(.*?)</(?:span|button)>\n", head)]
        self.assertEqual(columns, ["Player", "Scale of Cheating", "ELO (Diplo Off)", "ELO (Diplo On)"])
        # Diplo off first, and pressed: the order the list opens in.
        self.assertEqual(re.findall(r'class="rank-sort" data-sort="(\w+)" aria-pressed="(\w+)"', head), [("off", "true"), ("on", "false")])
        self.assertIn('show(VIEWS.off ? "off" : "on");', SCRIPT)
        # A row's cells are in the heads' order.
        self.assertIn('item.append(label, cheat(r), value(r, "off"), value(r, "on"));', SCRIPT)
        # In a narrow table a head keeps its last word or two.
        self.assertIn('<span class="rank-wide">Scale of </span>Cheating</span>', head)
        self.assertIn('<span class="rank-wide">ELO (</span>Diplo Off<span class="rank-wide">)</span></button>', head)
        self.assertIn(".elo-rank .rank-wide { display: none; }", LANDING)
        # The line under the title says what the scale is, in the format's
        # own numbers.
        self.assertRegex(SECTION, r'<p class="res-counted">[^<]*Scale of Cheating: [^<]*0 at Prince to 4 at Deity\.</p>')
        # Under the players a row is kept for the mods ("Mods", the last
        # line of the format): none is rated, and the row says so.
        self.assertIn('const MODS = { kind: "mods", name: "Mods", tilt: "None rated yet", cheating: null, on: null, off: null };', SCRIPT)
        self.assertIn("for (const r of [...order(side), MODS]) {", SCRIPT)

    def test_a_row_is_faintly_its_kind_s_colour(self):
        # Operator 2026-11-27, of Tournaments' ELO Rankings: "we can
        # highlight dev blue, civvis gold, and firaxis red in the elo table.
        # faint. help show the categories all being compared together". The
        # same three inks here, and a key that names them.
        for kind, ink in (("agent", "var(--gold) 8%"), ("level", "#e5534b 10%"), ("person", "var(--accent) 11%")):
            self.assertIn(f".elo-rank .rank-row.is-{kind} {{ background: color-mix(in srgb, {ink}, transparent); }}", LANDING)
        self.assertIn('<p class="elo-kinds"><span class="elo-kind is-agent">CIVVIS AI</span>'
                      '<span class="elo-kind is-level">Classic Firaxis AI</span><span class="elo-kind is-person">Dev</span></p>', SECTION)
        self.assertIn('const item = el("li", "rank-row is-" + r.kind);', SCRIPT)

    def test_its_figures_are_elo_rankings_own_record(self):
        self.assertEqual(LANDING.count('<script type="application/json" id="elo-data">null</script>'), 1)
        self.assertLess(PUBLISH.index('fill home_rankings.py "$out/$home_at/index.html"'),
                        PUBLISH.index('fill elo_page.py "$out/$home_at/index.html"'))
        filled = elo_page.fill(LANDING, elo_record())
        self.assertEqual(json.loads(elo_page.SLOT.search(filled)[2]), elo_record())
        self.assertEqual(filled.count("</script>"), LANDING.count("</script>"))
        # The other two slots are as they were.
        self.assertEqual(home_rankings.BENCHMARK_SLOT.search(filled)[2], "null")
        # Nothing of the record is trusted as markup.
        self.assertNotIn("innerHTML", SCRIPT)
        # The benchmark's helpers are still that chart's own first: this
        # table's script stands after it (`home_rankings_test.py` reads the
        # page's first `el` and `day` as Win Analytics').
        self.assertLess(LANDING.index('document.getElementById("home-benchmark")'), LANDING.index('document.getElementById("elo-data")'))
        self.assertLess(LANDING.index('document.getElementById("elo-data")'), LANDING.index("// The video tiles."))


class Cheating(unittest.TestCase):
    """The scale of cheating the page writes beside a level: "Prince 0, King
    1, Emperor 2, Immortal 3, Deity 4"."""

    LEVELS = [(int(level), name) for level, name in
              re.findall(r'\[(\d+), "(\w+)"\]', cut(SCRIPT, "const LEVELS = [", "];"))]
    FAIR = int(re.search(r"const FAIR = (\d+);", SCRIPT)[1])

    def test_the_levels_are_the_ones_elo_rankings_rates(self):
        self.assertEqual(self.LEVELS, [(level, home_rankings.CIV6_RUNGS[level - 1].title()) for level in elo_page.LEVELS[::-1]])
        self.assertEqual([(name, level - self.FAIR) for level, name in self.LEVELS],
                         [("Deity", 4), ("Immortal", 3), ("Emperor", 2), ("King", 1), ("Prince", 0)])
        self.assertIn("cheating: n - FAIR });", SCRIPT)
        # An agent and a person are handed nothing.
        self.assertEqual(SCRIPT.count("cheating: 0 })[side] = reading("), 2)

    def test_the_scale_counts_the_steps_of_bonus_the_game_hands_its_own_ai(self):
        # The engine's ladder is the game's own (`data/difficulties.json`,
        # `rules::DifficultySpec`): Prince hands the AI nothing, and each
        # level over it one more step of its bonuses. The scale is that
        # count, so it cannot drift from what a level is.
        ladder = json.loads((ROOT / "data" / "difficulties.json").read_text(encoding="utf-8"))
        fair = ladder[home_rankings.CIV6_RUNGS[self.FAIR - 1]]
        self.assertEqual(home_rankings.CIV6_RUNGS[self.FAIR - 1], "prince")
        self.assertEqual([key for key in fair if key.startswith("ai_")], [])
        step = ladder[home_rankings.CIV6_RUNGS[self.FAIR]]
        self.assertGreater(step["ai_xp_pct"], 0)
        for level, name in self.LEVELS:
            rung, scale = ladder[name.lower()], level - self.FAIR
            self.assertEqual(rung["order"] - fair["order"], scale, name)
            self.assertEqual(rung.get("ai_era_boosts", 0), scale, name)
            self.assertEqual(rung.get("ai_xp_pct", 0), scale * step["ai_xp_pct"], name)
            self.assertEqual(rung.get("ai_yield_pct", {}).get("production", 0), scale * step["ai_yield_pct"]["production"], name)


class Drawn(unittest.TestCase):
    """The table, drawn in a browser from a filled page."""

    #: The table as it is drawn: each row's cells, its kind and its tint,
    #: the heads, and which one is pressed.
    READ = """(() => { const card = document.getElementById('elo'), list = card.querySelector('.elo-rank');
      const rows = [...list.querySelectorAll('.rank-list .rank-row')];
      return {hidden: card.hidden, rows: rows.map(row => [...row.children].map(cell => cell.firstChild ? cell.firstChild.textContent : '')),
        kinds: rows.map(row => row.className.replace('rank-row', '').trim()),
        inks: Object.fromEntries(rows.map(row => [row.className.replace('rank-row', '').trim(), getComputedStyle(row).backgroundColor])),
        titles: rows.map(row => row.title),
        heads: [...list.querySelectorAll('.rank-head > *')].map(head => head.innerText.trim()),
        pressed: [...list.querySelectorAll('.rank-sort')].map(b => [b.dataset.sort, b.getAttribute('aria-pressed')]),
        columns: rows.every(row => getComputedStyle(row).gridTemplateColumns.split(' ').length === 5),
        cut: [...list.querySelectorAll('.rank-name')].filter(n => n.scrollWidth > n.clientWidth).map(n => n.firstChild.textContent),
        wide: document.documentElement.scrollWidth > innerWidth}; })()"""

    #: The fixture's players as the table lists them, by Diplo off: every
    #: rated player numbered in one ranking, then the levels with no figure,
    #: the highest first, then the row kept for the mods. A row is its place,
    #: the player, its place on the scale of cheating, and its Elo with the
    #: Diplomatic victory off and on.
    BY_OFF = [
        ["1", "HolyRoller", "0", "1543", "1547"],
        ["2", "Lv 8 Deity", "4", "1452", "1042"],
        ["3", "JackOfAllTrades", "0", "1191", "1155"],
        ["4", "dev", "0", "1100", "1210"],
        ["5", "Lv 5 King", "1", "799", "595"],
        ["", "Lv 7 Immortal", "3", "–", "–"],
        ["", "Lv 6 Emperor", "2", "–", "–"],
        ["", "Lv 4 Prince", "0", "–", "–"],
        ["", "Mods", "–", "–", "–"],
    ]

    @classmethod
    def setUpClass(cls):
        cls.stage = pathlib.Path(tempfile.mkdtemp(prefix="civvis-home-elo-"))
        cls.profile = fresh_profile("civvis-home-elo-profile-")
        cls.chrome = None
        # The page with both its records: the benchmark stands under the
        # table only when it is shown.
        bench = {"to": "2026-11-27 10:00Z", "days": [1, 3, 7, 30], "games": 12, "rungs": {"8": "Deity"}, "tables": {
            name: {"games": 6, "levels": [{"level": 8, "windows": [{"days": days, "games": 6, "wins": 2} for days in (1, 3, 7, 30)]}]}
            for name in ("on", "off", "religion_off")}}
        page = cls.stage / "index.html"
        page.write_text(elo_page.fill(home_rankings.fill(LANDING, bench, home_rankings.BENCHMARK_SLOT), elo_record()), encoding="utf-8")
        cls.address = page.as_uri()
        # And as a clone with no run ledger publishes it: the agents alone.
        bare = elo_record()
        for view in (bare, bare["off"]):
            view.update(firaxis=None, people=[])
        page = cls.stage / "bare.html"
        page.write_text(elo_page.fill(LANDING, bare), encoding="utf-8")
        cls.bare = page.as_uri()
        chrome = os.environ.get("CIVVIS_TEST_CHROME") or find_chrome()
        if not pathlib.Path(chrome).exists() and not shutil.which(chrome):
            return
        port = free_port()
        cls.chrome = subprocess.Popen(
            [chrome, "--headless=new", f"--remote-debugging-port={port}", f"--user-data-dir={cls.profile}",
             "--no-first-run", "--disable-gpu", "--window-size=1280,1200", "about:blank"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        tether(cls.chrome)
        target, deadline = None, time.time() + 60
        while time.time() < deadline and target is None:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{port}/json", timeout=2) as answer:
                    pages = [page for page in json.load(answer) if page.get("type") == "page"]
                target = next((page for page in pages if page.get("webSocketDebuggerUrl")), None)
            except OSError:
                time.sleep(0.2)
        if target is None:
            cls.chrome.terminate()
            cls.chrome = None
            raise AssertionError("Chrome never offered a debuggable page within 60s")
        cls.dev = Devtools(target["webSocketDebuggerUrl"])
        cls.dev.call("Page.enable")
        cls.dev.call("Runtime.enable")

    @classmethod
    def tearDownClass(cls):
        if cls.chrome:
            cls.chrome.terminate()
            try:
                cls.chrome.wait(timeout=5)
            except subprocess.TimeoutExpired:
                cls.chrome.kill()
        shutil.rmtree(cls.stage, ignore_errors=True)
        shutil.rmtree(cls.profile, ignore_errors=True)

    def drawn(self, address: str, width: int) -> dict:
        """The page opened in a window `width` wide: its table, read."""
        if not self.chrome:
            self.skipTest("no Chrome on this machine to open the page in")
        self.dev.call("Emulation.setDeviceMetricsOverride",
                      {"width": width, "height": 1200, "deviceScaleFactor": 1, "mobile": False})
        ready = "document.readyState === 'complete' && location.href === %s && !!document.querySelector('#elo .rank-list .rank-row')" % json.dumps(address)
        for attempt in range(1, 5):
            self.dev.call("Page.navigate", {"url": address})
            deadline = time.time() + 15
            while time.time() < deadline:
                try:
                    if self.dev.evaluate(ready):
                        return self.dev.evaluate(self.READ)
                except Exception:  # noqa: BLE001 - the page may be between two documents
                    pass
                time.sleep(0.1)
            print(f"attempt {attempt}: the page had not drawn its ELO Rankings after 15s; opening it again.")
        self.fail(f"the page never drew its ELO Rankings ({self.dev.console[-5:]})")

    def test_everyone_is_ranked_together_with_the_scale_beside_them(self):
        drawn = self.drawn(self.address, 1280)
        self.assertFalse(drawn["hidden"])
        self.assertEqual(drawn["heads"], ["", "Player", "Scale of Cheating", "ELO (Diplo Off)", "ELO (Diplo On)"])
        self.assertEqual(drawn["pressed"], [["off", "true"], ["on", "false"]])
        self.assertTrue(drawn["columns"])
        self.assertEqual(drawn["rows"], self.BY_OFF)
        self.assertEqual(drawn["kinds"], ["is-agent", "is-level", "is-agent", "is-person", "is-level", "is-level", "is-level", "is-level", "is-mods"])
        # Each kind faintly its own colour, the three apart; the mods' row
        # has none.
        inks = drawn["inks"]
        self.assertEqual(len({inks["is-agent"], inks["is-level"], inks["is-person"]}), 3, inks)
        for kind in ("is-agent", "is-level", "is-person"):
            self.assertNotIn(inks[kind], ("rgba(0, 0, 0, 0)", "transparent"), kind)
        self.assertEqual(inks["is-mods"], "rgba(0, 0, 0, 0)")
        # A row says what its figures are when it is pointed at.
        said = drawn["titles"]
        self.assertIn("Scale of cheating 0: plays with no bonus", said[0])
        self.assertIn("Scale of cheating 4: 4 steps of the bonuses Civilization VI hands its own AI", said[1])
        self.assertIn("Diplo off: our seat won 9 of 30 benchmark games against five of it; 95% interval 1362–1542", said[1])
        self.assertIn("Scale of cheating 1: 1 step of the bonuses", said[4])
        self.assertIn("Scale of cheating 0: Civilization VI hands its own AI no bonus at this level", said[7])
        self.assertEqual(said[8], "Mods of Civilization VI’s AI: none is rated yet")
        # It is the page's first part: under the line the page opens on,
        # over the benchmark, which is over the videos.
        places = self.dev.evaluate("""(() => { const box = q => document.querySelector(q).getBoundingClientRect();
          return [box('.intro').bottom <= box('#elo').top, box('#elo').bottom <= box('#bench').top, box('#bench').bottom <= box('#watch').top,
            document.getElementById('bench').hidden]; })()""")
        self.assertEqual(places, [True, True, True, False])
        self.assertFalse(drawn["wide"])
        self.assertEqual(drawn["cut"], [])
        # Diplo on sorts it the other way; the rows with no figure stay last.
        self.dev.evaluate("document.querySelector('#elo [data-sort=\"on\"]').click(), true")
        again = self.dev.evaluate(self.READ)
        self.assertEqual(again["pressed"], [["off", "false"], ["on", "true"]])
        self.assertEqual([row[1] for row in again["rows"]],
                         ["HolyRoller", "dev", "JackOfAllTrades", "Lv 8 Deity", "Lv 5 King", "Lv 7 Immortal", "Lv 6 Emperor", "Lv 4 Prince", "Mods"])
        self.assertEqual([row[0] for row in again["rows"]], ["1", "2", "3", "4", "5", "", "", "", ""])
        self.assertEqual(self.dev.console, [])

    def test_on_a_phone_it_keeps_its_columns_and_every_name(self):
        drawn = self.drawn(self.address, 390)
        self.assertEqual(drawn["heads"], ["", "Player", "Cheating", "Diplo Off", "Diplo On"])
        self.assertTrue(drawn["columns"])
        self.assertEqual(drawn["rows"], self.BY_OFF)
        self.assertEqual(drawn["cut"], [])
        self.assertFalse(drawn["wide"])
        self.assertEqual(self.dev.console, [])

    def test_with_no_run_ledger_the_levels_keep_their_rows(self):
        # A clone that never fetched the ledger rates no Firaxis level and
        # no person: the agents are ranked, and each level still has its row
        # and its place on the scale.
        drawn = self.drawn(self.bare, 1280)
        self.assertFalse(drawn["hidden"])
        self.assertEqual(drawn["rows"], [
            ["1", "HolyRoller", "0", "1543", "1547"],
            ["2", "JackOfAllTrades", "0", "1191", "1155"],
            ["", "Lv 8 Deity", "4", "–", "–"],
            ["", "Lv 7 Immortal", "3", "–", "–"],
            ["", "Lv 6 Emperor", "2", "–", "–"],
            ["", "Lv 5 King", "1", "–", "–"],
            ["", "Lv 4 Prince", "0", "–", "–"],
            ["", "Mods", "–", "–", "–"],
        ])
        # With no benchmark record either, the videos follow it.
        self.assertTrue(self.dev.evaluate("document.getElementById('bench').hidden"))
        self.assertEqual(self.dev.console, [])


if __name__ == "__main__":
    unittest.main()
