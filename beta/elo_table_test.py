#!/usr/bin/env python3
"""ELO Rankings, one table on Home, Tournaments and ELO Rankings.

Operator, 2026-11-28: "merge the home elo ranking and the elo chart together
into the same elo ranking table and replace both copies with this new merged
results. fill out as completely as possible".

The table is a block of three parts, its rules, its markup and its script,
that stands in each of the three pages byte for byte. This holds the copies to
one another and to where each page keeps it, the scale of cheating to the
engine's own ladder, and, in a browser, the table as it is drawn from a
record: every player in one ranking across two tables, a column a setup, the
levels with their benchmark figure on the left and their simulator stand-in
on the right, the mods' row last, any column sorting both tables, the rows
lined up and lit together, and the two tables scrolled together.

Set `CIVVIS_TEST_CHROME` to run the browser part with a particular binary.

    python3 beta/elo_table_test.py
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
PAGES = {name: (BETA / file).read_text(encoding="utf-8")
         for name, file in (("home", "landing.html"), ("tournaments", "tournaments.html"), ("elo", "elo.html"))}
PUBLISH = (BETA / "publish.sh").read_text(encoding="utf-8")

#: The block's three parts, each from its first line through its last.
PARTS = {
    "rules": ("  /* elo-table: begin", "  /* elo-table: end */\n"),
    "markup": ("        <!-- elo-table: begin", "        <!-- elo-table: end -->\n"),
    "script": ("  <script>\n  // elo-table: begin", "  // elo-table: end\n  </script>\n"),
}


def part(page: str, name: str) -> str:
    begin, end = PARTS[name]
    at = page.index(begin)
    return page[at:page.index(end, at) + len(end)]


def cut(page: str, start: str, end: str) -> str:
    at = page.index(start)
    return page[at:page.index(end, at) + len(end)]


RULES = part(PAGES["home"], "rules")
MARKUP = part(PAGES["home"], "markup")
SCRIPT = part(PAGES["home"], "script")


def rated(elo: float, margin: float, games: int, wins: int, **more) -> dict:
    return {"elo": elo, "margin": margin, "games": games, "wins": wins, "bound": "", **more}


def elo_record(ledger: bool = True) -> dict:
    """ELO Rankings' record (`beta/elo_page.py`) as the table reads it.

    The four-player fits the ratings began with, the record itself (every
    victory on) and `off`, each with the simulator's stand-ins at Deity and
    Prince; the two six-player fits, each with its Prince anchor; and two
    more setups, the agents alone. With the run ledger, each six-player fit
    has the Firaxis levels from the benchmark (Prince with no figure) and a
    person from their hot seat games; without it, neither."""
    def agents(holy, jack):
        rows = [{"kind": "agent", "id": "holy", "name": "HolyRoller", "tilt": "Religion", **rated(holy, 30, 400, 250)}]
        if jack is not None:
            rows.append({"kind": "agent", "id": "jack", "name": "JackOfAllTrades", "tilt": "Balanced", **rated(jack, 29, 400, 200)})
        return rows

    def fit(rows, tables, players, simulator, firaxis=None, people=()):
        return {"strategies": {"rows": rows, "tables": tables, "players": players, "turns": 250, "runs": 1, "latest_run": "2026-11-27"},
                "simulator": simulator, "firaxis": firaxis if ledger else None, "people": list(people) if ledger else []}

    def four(holy, jack, deity):
        return fit(agents(holy, jack) + [{"kind": "rung", "name": "Deity", "levels": [8], "elo": deity, "margin": 46}], 9000, 4,
                   [{"level": 8, "name": "Deity", "elo": deity, "margin": 46}, {"level": 4, "name": "Prince", "elo": 1000.0, "margin": 0}])

    def six(holy, jack, deity, king, dev):
        firaxis = {"days": 7, "games": 40, "to": "2026-11-27T10:00:00Z", "levels": [
            {"level": 8, "name": "Deity", "now": rated(deity, 90, 30, 9)},
            {"level": 5, "name": "King", "now": rated(king, 150, 10, 7)},
            {"level": 4, "name": "Prince", "now": None},
        ]}
        return fit(agents(holy, jack), 480, 6, [{"level": 4, "name": "Prince", "elo": 1000.0, "margin": 0}],
                   firaxis, [{"name": "dev", **rated(dev, 180, 3, 2)}])

    record = four(holy=1547, jack=1155, deity=1955)
    record["off"] = four(holy=1543, jack=1191, deity=1940)
    record["setups"] = {
        "continents6_no_diplo": six(holy=1837, jack=1351, deity=1830, king=1064, dev=1100),
        "continents6": six(holy=1920, jack=1412, deity=1402, king=978, dev=1210),
        "continents6_science": fit(agents(1300, 1400), 120, 6, []),
        "pangaea2": fit(agents(1200, None), 120, 2, []),
    }
    return record


class Copies(unittest.TestCase):
    def test_the_three_pages_carry_the_one_table_byte_for_byte(self):
        for name, page in PAGES.items():
            for piece, (begin, end) in PARTS.items():
                self.assertEqual(page.count(begin), 1, (name, piece))
                self.assertEqual(part(page, piece), part(PAGES["home"], piece), (name, piece))
            # Each has the one slot the script reads, and publishing fills it.
            self.assertEqual(page.count('<script type="application/json" id="elo-data">null</script>'), 1, name)
        for at in ("home_at", "tournaments_at", "elo_at"):
            self.assertIn(f'fill elo_page.py "$out/${at}/index.html"', PUBLISH)
        # What the three tables were before it is gone from every page.
        for name, page in PAGES.items():
            for gone in ('class="elo-rank"', 'class="rank-list"', "data-rank-note></span>\n            <a", "#tour-elo .elo-pair",
                         "// ---- Player strategies", "show(VIEWS.off", 'const item = el("li", "rank-row'):
                self.assertNotIn(gone, page, (name, gone))

    def test_each_page_keeps_it_where_it_stood(self):
        # Home: the page's first part, under the line it opens on, over the
        # benchmark (2026-11-27: "put the elo chart at the top of home/ too,
        # above the benchmarks"), with its head and the way to the ratings
        # over time.
        home = PAGES["home"]
        main = cut(home, '<main class="shell">', "</main>")
        self.assertEqual(re.findall(r"<section [^>]*\bid=\"([\w-]+)\"", main), ["elo", "bench", "watch"])
        section = cut(home, '<section class="res-block home-elo" id="elo" aria-labelledby="home-elo-title" hidden>', "</section>")
        self.assertIn('<h2 class="res-side-title" id="home-elo-title">ELO Rankings</h2>', section)
        self.assertIn('<p class="home-bench-more"><a href="../charts/elo-rankings/">Elo over time</a></p>', section)
        self.assertIn(MARKUP, section)
        # Tournaments: the first thing in the left column, its title the way
        # to ELO Rankings.
        tournaments = PAGES["tournaments"]
        left = cut(tournaments, '<div class="tour-main">', '<div class="tour-side">')
        self.assertEqual(re.findall(r'<section [^>]*id="([a-z-]+)"', left)[0], "tour-elo")
        section = cut(tournaments, '<section class="tour-section" id="tour-elo" hidden', "</section>")
        self.assertIn('<a href="../charts/elo-rankings/" title="ELO Rankings: the ratings now and over time">ELO Rankings</a></h2>', section)
        self.assertIn(MARKUP, section)
        # ELO Rankings: the player strategies at the top, then the graph.
        elo = PAGES["elo"]
        record = elo[elo.index('<div class="elo-record" id="elo-record" hidden>'):]
        self.assertEqual(re.findall(r'<section [^>]*id="([a-z-]+)"', record), ["rank-strategies", "elo-over-time"])
        section = cut(elo, '<section class="res-section" id="rank-strategies" data-elo-card hidden', "</section>")
        self.assertIn(MARKUP, section)
        # The script stands after the slot it reads, on every page.
        for name, page in PAGES.items():
            self.assertLess(page.index('id="elo-data"'), page.index(SCRIPT), name)
        # On Home, after the benchmark's: `home_rankings_test.py` reads the
        # page's first `el` and `day` as Win Analytics'.
        self.assertLess(home.index('document.getElementById("home-benchmark")'), home.index(SCRIPT))

    def test_its_rules_are_its_own(self):
        # Every rule of the block is under `.elo-table`, so it touches
        # nothing else on any page, and no page's own rules reach into it.
        style = re.sub(r"/\*.*?\*/", "", RULES, flags=re.S)
        style = re.sub(r"@(media|container)[^{]*\{", "", style)
        selectors = [chunk.rsplit("{", 1)[0].strip() for chunk in style.split("}") if "{" in chunk]
        self.assertGreater(len(selectors), 30)
        for selector in selectors:
            for one in selector.split(","):
                self.assertTrue(one.strip().startswith(".elo-table"), one)
        # Each kind faintly its colour ("we can highlight dev blue, civvis
        # gold, and firaxis red in the elo table. faint"), with a key.
        for kind, ink in (("agent", "var(--gold) 8%"), ("level", "#e5534b 10%"), ("person", "var(--accent) 11%")):
            self.assertIn(f".elo-table tbody tr.is-{kind} {{ background: color-mix(in srgb, {ink}, transparent); }}", RULES)
        self.assertIn('<p class="elo-kinds"><span class="elo-kind is-agent">CIVVIS AI</span>'
                      '<span class="elo-kind is-level">Classic Firaxis AI</span><span class="elo-kind is-person">Dev</span></p>', MARKUP)
        # Every row and head row one height, so the two tables line up.
        self.assertIn(".elo-table thead tr { height: 24px; }", RULES)
        self.assertIn(".elo-table tbody tr { height: 28px; }", RULES)
        # Hidden until its script has a record to draw; nothing of the record
        # is trusted as markup.
        self.assertIn('<div class="elo-table" hidden>', MARKUP)
        self.assertIn("table.hidden = false;", SCRIPT)
        self.assertNotIn("innerHTML", SCRIPT)

    def test_the_columns_are_every_setup_rated_in_the_operator_s_order(self):
        # "6p Continents Online (Diplo off, Diplo on, science, culture,
        # religion, diplomacy, domination, score) , 2P Pangaea (religion off,
        # religion on)", with Home's two, the four-player fits, between them;
        # on the left, what Civilization VI itself measured.
        columns = re.findall(r'\["(\w+)", "([\w ]+)", (SIX|FOUR|TWO)\]', cut(SCRIPT, "const SELF = [", "];"))
        self.assertEqual(columns, [
            ("continents6_no_diplo", "Diplo off", "SIX"), ("continents6", "Diplo on", "SIX"),
            ("continents6_science", "Science", "SIX"), ("continents6_culture", "Culture", "SIX"),
            ("continents6_religion", "Religion", "SIX"), ("continents6_diplomacy", "Diplomacy", "SIX"),
            ("continents6_domination", "Domination", "SIX"), ("continents6_score", "Score", "SIX"),
            ("pangaea4_no_diplo", "Diplo off", "FOUR"), ("pangaea4", "Diplo on", "FOUR"),
            ("pangaea2_no_religion", "Religion off", "TWO"), ("pangaea2", "Religion on", "TWO")])
        self.assertIn('const SIX = "6P Continents Online", FOUR = "4P Pangaea", TWO = "2P Pangaea";', SCRIPT)
        self.assertIn('const CONTROL = [["continents6_no_diplo", "Diplo off", SIX], ["continents6", "Diplo on", SIX]];', SCRIPT)
        # Each six-player column is a fit the record names, and the
        # four-player two are the record itself and its `off`.
        self.assertEqual({key for key, _, game in columns if game == "SIX"} | {key for key, _, game in columns if game == "TWO"},
                         set(elo_page.SETUPS))
        self.assertIn("const FOUR_PLAYER = { pangaea4: data, pangaea4_no_diplo: data.off || null };", SCRIPT)


class Cheating(unittest.TestCase):
    """The scale of cheating beside a level: "Prince 0, King 1, Emperor 2,
    Immortal 3, Deity 4"."""

    LEVELS = [(int(level), name) for level, name in re.findall(r'\[(\d+), "(\w+)"\]', cut(SCRIPT, "const LEVELS = [", "];"))]
    FAIR = int(re.search(r"const FAIR = (\d+);", SCRIPT)[1])

    def test_the_levels_are_the_ones_elo_rankings_rates(self):
        self.assertEqual(self.LEVELS, [(level, home_rankings.CIV6_RUNGS[level - 1].title()) for level in elo_page.LEVELS[::-1]])
        self.assertEqual([(name, level - self.FAIR) for level, name in self.LEVELS],
                         [("Deity", 4), ("Immortal", 3), ("Emperor", 2), ("King", 1), ("Prince", 0)])
        self.assertIn("cheating: n - FAIR });", SCRIPT)
        # An agent and a person are handed nothing.
        self.assertEqual(SCRIPT.count(", cheating: 0 })"), 2)

    def test_the_scale_counts_the_steps_of_bonus_the_game_hands_its_own_ai(self):
        # The engine's ladder is the game's own (`data/difficulties.json`):
        # Prince hands the AI nothing, and each level over it one more step
        # of its bonuses. The scale is that count.
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

    READ = """(() => { const table = document.querySelector('.elo-table');
      const side = name => table.querySelector('[data-elo-side="' + name + '"]');
      const rows = name => [...side(name).querySelectorAll('tbody tr')];
      const text = row => [...row.children].map(cell => cell.firstChild ? cell.firstChild.textContent : '');
      const tops = name => rows(name).map(row => { const r = row.getBoundingClientRect(); return [Math.round(r.top), Math.round(r.height)]; });
      return {hidden: table.hidden || table.closest('section').hidden, left: rows('control').map(text), right: rows('self').map(text),
        kinds: rows('control').map(row => row.className.replace(' is-hover', '')),
        inks: Object.fromEntries(rows('control').map(row => [row.className.replace(' is-hover', ''), getComputedStyle(row).backgroundColor])),
        titles: rows('control').map(row => row.title), cells: rows('self').map(row => [...row.children].map(cell => cell.title)),
        heads: ['control', 'self'].map(name => [...side(name).querySelectorAll('thead tr')].map(row => [...row.children].map(cell => cell.textContent.trim()))),
        headHeights: ['control', 'self'].map(name => Math.round(side(name).querySelector('thead').getBoundingClientRect().height)),
        lefts: tops('control'), rights: tops('self'),
        pressed: [...table.querySelectorAll('.rank-sort')].filter(b => b.getAttribute('aria-pressed') === 'true').map(b => b.dataset.sort),
        note: table.querySelector('[data-elo-note]').textContent,
        wide: document.documentElement.scrollWidth > innerWidth}; })()"""

    DASH = "–"

    @classmethod
    def setUpClass(cls):
        cls.stage = pathlib.Path(tempfile.mkdtemp(prefix="civvis-elo-table-"))
        cls.profile = fresh_profile("civvis-elo-table-profile-")
        cls.chrome = None
        cls.pages = {}
        for name, page in PAGES.items():
            for kind, record in (("full", elo_record()), ("bare", elo_record(ledger=False))):
                path = cls.stage / f"{name}-{kind}.html"
                path.write_text(elo_page.fill(page, record), encoding="utf-8")
                cls.pages[name, kind] = path.as_uri()
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

    def drawn(self, page: str, kind: str = "full", width: int = 1280) -> dict:
        if not self.chrome:
            self.skipTest("no Chrome on this machine to open the page in")
        address = self.pages[page, kind]
        self.dev.call("Emulation.setDeviceMetricsOverride", {"width": width, "height": 1200, "deviceScaleFactor": 1, "mobile": False})
        ready = "document.readyState === 'complete' && location.href === %s && !!document.querySelector('.elo-table tbody tr')" % json.dumps(address)
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
            print(f"attempt {attempt}: {page} had not drawn its ELO Rankings after 15s; opening it again.")
        self.fail(f"{page} never drew its ELO Rankings ({self.dev.console[-5:]})")

    def test_every_player_ranked_together_across_both_tables_every_column_filled_that_can_be(self):
        drawn = self.drawn("home")
        self.assertFalse(drawn["hidden"])
        dash = self.DASH
        # Its heads: on the left the player, the scale of cheating and what
        # Civilization VI measured; on the right CIVVIS self-play, a column a
        # setup under its game.
        self.assertEqual(drawn["heads"][0], [
            ["Civilization VI · computer control"], ["", "Player", "Scale of Cheating", "6P Continents Online"], ["Diplo off", "Diplo on"]])
        self.assertEqual(drawn["heads"][1], [
            ["CIVVIS self-play"], ["6P Continents Online", "4P Pangaea", "2P Pangaea"],
            ["Diplo off", "Diplo on", "Science", "Culture", "Religion", "Diplomacy", "Domination", "Score", "Diplo off", "Diplo on",
             "Religion off", "Religion on"]])
        # It opens on Diplo off, both tables' heads pressed: every player by
        # the figure that measures it, an agent's in self-play, a level's and
        # a person's in Civilization VI (Prince, with no benchmark figure, by
        # its stand-in); a player with no figure there follows, and the levels
        # with none anywhere, the highest first; the mods' row last.
        self.assertEqual(drawn["pressed"], ["control:continents6_no_diplo", "self:continents6_no_diplo"])
        self.assertEqual(drawn["left"], [
            ["1", "HolyRoller", "0", dash, dash],
            ["2", "Lv 8 Deity", "4", "1830", "1402"],
            ["3", "JackOfAllTrades", "0", dash, dash],
            ["4", "dev", "0", "1100", "1210"],
            ["5", "Lv 5 King", "1", "1064", "978"],
            ["6", "Lv 4 Prince", "0", dash, dash],
            ["", "Lv 7 Immortal", "3", dash, dash],
            ["", "Lv 6 Emperor", "2", dash, dash],
            ["", "Mods", dash, dash, dash],
        ])
        # Self-play: the agents in every fit that rated them; a level's
        # stand-in where the fit rated one (the six-player fits seat only the
        # Prince anchor); nothing for a person or the mods; a dash for a setup
        # not yet rated.
        self.assertEqual(drawn["right"], [
            ["1837", "1920", "1300", dash, dash, dash, dash, dash, "1543", "1547", dash, "1200"],
            [dash] * 8 + ["1940", "1955", dash, dash],
            ["1351", "1412", "1400", dash, dash, dash, dash, dash, "1191", "1155", dash, dash],
            [dash] * 12,
            [dash] * 12,
            ["1000", "1000"] + [dash] * 6 + ["1000", "1000", dash, dash],
            [dash] * 12,
            [dash] * 12,
            [dash] * 12,
        ])
        # A row runs across: each left row level with its right row, and the
        # two heads one height.
        self.assertEqual(drawn["lefts"], drawn["rights"])
        self.assertEqual(drawn["headHeights"][0], drawn["headHeights"][1])
        # Each kind faintly its own colour, the three apart; the mods' none.
        inks = drawn["inks"]
        self.assertEqual(len({inks["is-agent"], inks["is-level"], inks["is-person"]}), 3, inks)
        for kind in ("is-agent", "is-level", "is-person"):
            self.assertNotIn(inks[kind], ("rgba(0, 0, 0, 0)", "transparent"), kind)
        self.assertEqual(inks["is-mods"], "rgba(0, 0, 0, 0)")
        # A row and a figure say what they are when pointed at.
        said = drawn["titles"]
        self.assertIn("Scale of cheating 0: plays with no bonus", said[0])
        self.assertIn("Scale of cheating 4: 4 steps of the bonuses Civilization VI hands its own AI", said[1])
        self.assertIn("Civilization VI, 6P Continents Online, Diplo off: our seat won 9 of 30 benchmark games against five of it; "
                      "95% interval 1740–1920", said[1])
        self.assertIn("Civilization VI, 6P Continents Online, Diplo on: won 2 of 3 games against the Civ AI Agents in Civilization VI", said[3])
        self.assertIn("Scale of cheating 0: Civilization VI hands its own AI no bonus at this level", said[5])
        self.assertEqual(said[8], "Mods of Civilization VI’s AI: none is rated yet")
        self.assertEqual(drawn["cells"][1][8], "CIVVIS self-play, 4P Pangaea, Diplo off: the simulator’s stand-in for it, our agent with "
                                               "the level’s handicap (Deity), rated at these tables; 95% interval 1894–1986")
        self.assertEqual(drawn["cells"][0][2], "CIVVIS self-play, 6P Continents Online, Science: won 250 of 400 rated tables; 95% interval 1270–1330")
        # How it is counted: each fit's tables, and what is not yet rated.
        self.assertIn("6P Continents Online Diplo off 480, 6P Continents Online Diplo on 480, 6P Continents Online Science 120, "
                      "4P Pangaea Diplo off 9,000, 4P Pangaea Diplo on 9,000, 2P Pangaea Religion on 120.", drawn["note"])
        self.assertIn("Not yet rated: 6P Continents Online Culture, 6P Continents Online Religion, 6P Continents Online Diplomacy, "
                      "6P Continents Online Domination, 6P Continents Online Score, 2P Pangaea Religion off.", drawn["note"])
        self.assertIn("40 Diplo off and 40 Diplo on.", drawn["note"])
        self.assertFalse(drawn["wide"])
        # Any head of either table sorts both: Science (right) puts those
        # rated there first, and the rest in the Diplo off ranking.
        self.dev.evaluate("document.querySelector('.elo-table [data-sort=\"self:continents6_science\"]').click(), true")
        again = self.dev.evaluate(self.READ)
        self.assertEqual(again["pressed"], ["self:continents6_science"])
        self.assertEqual([row[1] for row in again["left"]],
                         ["JackOfAllTrades", "HolyRoller", "Lv 8 Deity", "dev", "Lv 5 King", "Lv 4 Prince", "Lv 7 Immortal", "Lv 6 Emperor", "Mods"])
        self.assertEqual(again["right"][0][2], "1400")
        # Civilization VI's Diplo on (left) sorts both by it.
        self.dev.evaluate("document.querySelector('.elo-table [data-sort=\"control:continents6\"]').click(), true")
        again = self.dev.evaluate(self.READ)
        self.assertEqual([row[1] for row in again["left"]],
                         ["Lv 8 Deity", "dev", "Lv 5 King", "HolyRoller", "JackOfAllTrades", "Lv 4 Prince", "Lv 7 Immortal", "Lv 6 Emperor", "Mods"])
        self.assertEqual(again["right"][3][1], "1920")
        self.assertEqual(again["lefts"], again["rights"])
        # Pointing at a row in one table lights it in the other.
        lit = self.dev.evaluate("""(() => { const table = document.querySelector('.elo-table');
          table.querySelectorAll('[data-elo-side="control"] tbody tr')[1].children[1].dispatchEvent(new MouseEvent('mouseover', {bubbles: true}));
          return [...table.querySelectorAll('[data-elo-side="self"] tbody tr')].map(row => row.classList.contains('is-hover')); })()""")
        self.assertEqual(lit, [False, True] + [False] * 7)
        self.assertEqual(self.dev.console, [])

    def test_the_other_two_pages_draw_the_same_table(self):
        home = self.drawn("home")
        for page in ("tournaments", "elo"):
            drawn = self.drawn(page)
            self.assertEqual((drawn["left"], drawn["right"], drawn["heads"], drawn["pressed"]),
                             (home["left"], home["right"], home["heads"], home["pressed"]), page)
            self.assertEqual(self.dev.console, [], page)

    def test_with_no_run_ledger_the_levels_keep_their_rows_and_their_stand_ins(self):
        # A clone that never fetched the ledger rates no level from the
        # benchmark and no person: the agents are ranked, a level with a
        # stand-in is ranked by it where it has one, and every level keeps its
        # row and its place on the scale.
        drawn = self.drawn("home", "bare")
        self.assertFalse(drawn["hidden"])
        dash = self.DASH
        self.assertEqual([row[:3] for row in drawn["left"]], [
            ["1", "HolyRoller", "0"], ["2", "JackOfAllTrades", "0"], ["3", "Lv 4 Prince", "0"], ["4", "Lv 8 Deity", "4"],
            ["", "Lv 7 Immortal", "3"], ["", "Lv 6 Emperor", "2"], ["", "Lv 5 King", "1"], ["", "Mods", dash]])
        self.assertTrue(all(row[3:] == [dash, dash] for row in drawn["left"]))
        self.assertEqual(drawn["right"][3], [dash] * 8 + ["1940", "1955", dash, dash])
        self.assertEqual(self.dev.console, [])

    def test_on_a_phone_the_two_tables_scroll_sideways_together(self):
        self.drawn("home", width=390)
        moved = self.dev.evaluate("""new Promise(done => {
          const [left, right] = ['control', 'self'].map(name => document.querySelector('.elo-table [data-elo-side="' + name + '"]'));
          const span = pane => pane.scrollWidth - pane.clientWidth;
          right.scrollLeft = span(right);
          setTimeout(() => {
            const after = [span(left), span(right), left.scrollLeft, right.scrollLeft];
            left.scrollLeft = 0;
            setTimeout(() => done([...after, left.scrollLeft, right.scrollLeft, document.documentElement.scrollWidth > innerWidth]), 300);
          }, 300);
        })""")
        span_left, span_right, left_at, right_at, left_back, right_back, wide = moved
        self.assertGreater(span_right, 0, moved)
        self.assertEqual(right_at, span_right, moved)
        self.assertLessEqual(abs(left_at - span_left), 1, moved)
        self.assertEqual((left_back, right_back), (0, 0), moved)
        self.assertFalse(wide, moved)
        self.assertEqual(self.dev.console, [])


if __name__ == "__main__":
    unittest.main()
