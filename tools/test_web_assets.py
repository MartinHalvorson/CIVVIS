#!/usr/bin/env python3
"""The browser client's first automated check.

civvis.ai is one of two shipped products and its 30,738-line renderer had no
verification of any kind — no lint, no syntax check, no test. That absence is
why nobody splits `app.js`: the roadmap has listed it as a conflict hotspot for
weeks while the two Rust hotspots were dealt with, because a careless carve
breaks a live site and nothing would catch it.

The failure is not hypothetical and the supervisor's own header records it: "one
bad top-level lookup blanks the whole map — the sidebar, buttons and title still
paint, so it reads as 'CIVVIS is up but the game is not showing'. Cost most of
an afternoon on 2026-08-10."

## The invariant a split actually trips

A script the page loads has to be named in FOUR places, and missing any one of
them serves a page that 404s a script and paints half a client:

1. `<script src="/assets/NAME.js">` in `web/index.html`
2. `include_str!("../web/assets/NAME.js")` in `src/server.rs`, so the binary
   carries it
3. a `("GET", "/assets/NAME.js")` route arm, so the binary hands it over
4. the file itself on disk

These check all four agree, in both directions, so adding a script and
forgetting to serve it fails here instead of in a browser.

## And the hazard the split itself creates

Every one of these scripts is a classic script sharing one global scope. Two
`const` declarations of the same name across two files is a `SyntaxError` that
kills the whole page — not a shadowed variable, not a warning. Splitting a file
by moving declarations is exactly the operation that can produce one.
"""

from __future__ import annotations

import json
import re
import shutil
import subprocess
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
INDEX = REPO / "web" / "index.html"
ASSETS = REPO / "web" / "assets"
SERVER = REPO / "src" / "server.rs"

# A top-level declaration: `const X`, `let X`, `var X`, `function X`, `class X`
# written at column zero. Anything indented is inside a scope and cannot collide.
TOP_LEVEL = re.compile(r"^(?:const|let|var|function|class)\s+([A-Za-z_$][\w$]*)")
# `const` and `let` collide across scripts; `var` and `function` redeclare
# harmlessly, which is why the old inline controls could ever have worked.
FATAL_KINDS = re.compile(r"^(?:const|let|class)\s+([A-Za-z_$][\w$]*)")


def page_scripts() -> list[str]:
    """`/assets/*.js` sources the page loads, in load order."""
    html = INDEX.read_text(encoding="utf-8")
    return re.findall(r'<script src="/assets/([A-Za-z0-9_.-]+\.js)"', html)


def embedded_scripts() -> set[str]:
    text = SERVER.read_text(encoding="utf-8")
    return set(re.findall(r'include_str!\("\.\./web/assets/([A-Za-z0-9_.-]+\.js)"\)', text))


def served_scripts() -> set[str]:
    text = SERVER.read_text(encoding="utf-8")
    return set(re.findall(r'\("GET", "/assets/([A-Za-z0-9_.-]+\.js)"\)', text))


def declarations(path: Path, pattern: re.Pattern) -> dict[str, int]:
    names: dict[str, int] = {}
    for number, line in enumerate(path.read_text(encoding="utf-8").split("\n"), 1):
        match = pattern.match(line)
        if match:
            names.setdefault(match.group(1), number)
    return names


class TheScriptChainAgrees(unittest.TestCase):
    def test_the_page_loads_at_least_the_renderer(self):
        scripts = page_scripts()
        self.assertIn("app.js", scripts, "the page stopped loading the renderer")

    def test_every_script_the_page_loads_exists_on_disk(self):
        for name in page_scripts():
            self.assertTrue(
                (ASSETS / name).is_file(),
                f"index.html loads /assets/{name}, which is not in web/assets/",
            )

    def test_every_script_the_page_loads_is_embedded_in_the_binary(self):
        missing = [n for n in page_scripts() if n not in embedded_scripts()]
        self.assertEqual(
            missing,
            [],
            f"index.html loads {missing} but server.rs does not include_str! them, "
            f"so a published build ships a page that cannot find its own script.",
        )

    def test_every_script_the_page_loads_has_a_route(self):
        missing = [n for n in page_scripts() if n not in served_scripts()]
        self.assertEqual(
            missing,
            [],
            f"index.html loads {missing} but server.rs has no "
            f'("GET", "/assets/NAME") arm, so the browser gets a 404 and the page '
            f"paints without them.",
        )

    def test_nothing_is_embedded_that_the_page_never_loads(self):
        # Dead weight in the binary, and a sign a script was removed from the
        # page without being removed from the server.
        orphans = sorted(embedded_scripts() - set(page_scripts()))
        self.assertEqual(
            orphans,
            [],
            f"server.rs embeds {orphans}, which index.html never loads",
        )


class OneGlobalScope(unittest.TestCase):
    """Every served script shares one scope, so a repeated name is fatal."""

    def test_no_const_or_class_name_is_declared_in_two_scripts(self):
        seen: dict[str, tuple[str, int]] = {}
        clashes = []
        for name in page_scripts():
            for symbol, line in declarations(ASSETS / name, FATAL_KINDS).items():
                if symbol in seen:
                    first_file, first_line = seen[symbol]
                    clashes.append(
                        f"{symbol}: {first_file}:{first_line} and {name}:{line}"
                    )
                else:
                    seen[symbol] = (name, line)
        self.assertEqual(
            clashes,
            [],
            "these names are declared at the top level of two scripts that share "
            "one global scope. `const`/`let`/`class` redeclaration is a "
            "SyntaxError that kills the whole page, not a shadowed variable:\n  "
            + "\n  ".join(clashes),
        )

    def test_no_script_declares_the_same_name_twice_itself(self):
        """Redeclaration inside one file is the same SyntaxError.

        Missed by the cross-file check on the first attempt, and found by
        reintroducing the bug: a name moved OUT of app.js and then re-added to
        the file it moved to is a duplicate within one script, which the
        cross-file comparison cannot see.
        """
        for name in page_scripts():
            counts: dict[str, list[int]] = {}
            path = ASSETS / name
            for number, line in enumerate(path.read_text().split("\n"), 1):
                match = FATAL_KINDS.match(line)
                if match:
                    counts.setdefault(match.group(1), []).append(number)
            repeats = {k: v for k, v in counts.items() if len(v) > 1}
            self.assertEqual(
                repeats,
                {},
                f"{name} declares these names twice at the top level, which is a "
                f"SyntaxError that kills the page: {repeats}",
            )

    def test_the_split_is_measured_so_it_can_be_continued(self):
        # Not a threshold to satisfy — a number in the record, so the next carve
        # can see whether it moved.
        sizes = {n: len((ASSETS / n).read_text().split("\n")) for n in page_scripts()}
        biggest = max(sizes.values())
        self.assertGreater(
            len(sizes), 1, "the client is a single script again; the carve was reverted"
        )
        self.assertLess(
            biggest,
            40_000,
            f"a client script reached {biggest} lines: {sizes}",
        )


class ThePublishPipelineVisitsEveryScript(unittest.TestCase):
    """A carve must not strand a root-absolute asset reference in a lane.

    `web/index.html` and the scripts ask for assets from the site root, which
    is where a desktop build serves them. Published, they sit beside the page
    instead, so `beta/publish.sh` rewrites `"/assets/` to `"assets/`.

    That rewrite named ONE script. The day the renderer was carved further,
    `app_palette.js` carried `"/assets/feature-atlas.png"` and two more like
    it, and a script the rewrite never visited would have kept them
    root-absolute: published at /test they resolve against the site root, 404,
    and the terrain atlases silently do not load. `beta/verify.py` would not
    have caught it either — it checks `index.html` for surviving `"/assets/`
    and nothing else.

    Found before it shipped, and gated here so the next carve cannot repeat it.
    """

    PUBLISH = REPO / "beta" / "publish.sh"
    CACHE_BUST = REPO / "beta" / "cache_bust.py"

    def test_the_rewrite_reads_every_script_not_a_named_one(self):
        source = self.PUBLISH.read_text(encoding="utf-8")
        self.assertIn(
            'assets.glob("*.js")',
            source,
            "beta/publish.sh rewrites a hardcoded script instead of every one "
            "the lane ships; a carved-out script keeps its root-absolute asset "
            "references and 404s in a published lane",
        )
        self.assertNotIn(
            'app_js = assets / "app.js"',
            source,
            "beta/publish.sh is back to naming one script",
        )

    def test_cache_busting_versions_every_script(self):
        source = self.CACHE_BUST.read_text(encoding="utf-8")
        self.assertIn(
            'glob("*.js")',
            source,
            "beta/cache_bust.py versions the atlas references of one named "
            "script, so a carved-out script's atlases keep a stale cache",
        )

    def test_a_script_with_root_absolute_assets_is_actually_rewritten(self):
        # The reason the two checks above matter: these files really do carry
        # root-absolute references, so a pipeline that skips one ships a 404.
        carrying = [
            name
            for name in page_scripts()
            if '"/assets/' in (ASSETS / name).read_text(encoding="utf-8")
        ]
        self.assertTrue(
            carrying,
            "no served script references /assets/ any more; if that is real, "
            "the two checks above are guarding something that no longer exists",
        )


NODE = shutil.which("node")


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class EveryScriptParses(unittest.TestCase):
    """`node --check` is the cheapest real check this client has ever had."""

    def test_each_script_the_page_loads_parses(self):
        for name in page_scripts():
            done = subprocess.run(
                [NODE, "--check", str(ASSETS / name)],
                capture_output=True,
                text=True,
                timeout=120,
            )
            self.assertEqual(
                done.returncode,
                0,
                f"web/assets/{name} does not parse:\n{done.stderr.strip()}",
            )


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class StandingsRowsWearTheirCivilization(unittest.TestCase):
    """A standings row is its civilization's colour from edge to edge.

    The row carries a dozen figures that each keep the colour of what they
    count, so the colour under them is the jersey's deep wash, and the jersey
    itself stands at full strength only under the names. Three sentences in the
    page rest on arithmetic nobody can see by reading it: every figure reads on
    every wash, every name reads on its jersey, and no figure ever stands on the
    jersey. These run the page's own functions over every colour a civilization
    can wear and every colour the stylesheet gives a figure, so a brighter
    jersey, a darker figure or a nudged constant fails here rather than on a
    viewer's screen.
    """

    FLOOR = 4.5

    def setUp(self):
        # The colour arithmetic lives in the palette script and the row that
        # uses it in app.js; the page runs them as one program, and so do these.
        self.source = "".join((ASSETS / name).read_text(encoding="utf-8")
                              for name in ("app_palette.js", "app.js"))
        self.page = INDEX.read_text(encoding="utf-8")

    def cut(self, start, end):
        begin = self.source.index(start)
        return self.source[begin:self.source.index(end, begin)]

    def fn(self, name):
        return self.cut(f"function {name}(", "\n}\n") + "\n}\n"

    def run_node(self, code):
        done = subprocess.run([NODE, "-e", code], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr[-3000:])
        return done.stdout

    def colour_code(self):
        return (self.fn("rgbOf")
                + self.cut("const HUD_ROW_WASH_FLOOR =", "function contrastRatio(")
                + self.fn("contrastRatio")
                + self.cut("const HUD_JERSEY_INK_LIGHT =", "\n") + "\n"
                + self.fn("hudJerseyInk")
                + "function expect(condition, message) { if (!condition) throw Error(message); }\n")

    def jerseys(self):
        """Every (primary, partner) a major civilization can be dealt."""
        region = self.cut("// >>> generated by tools/civ6_player_colors.py",
                          "// <<< generated by tools/civ6_player_colors.py")
        pairs = set(re.findall(r'\["(#[0-9a-f]{6})","(#[0-9a-f]{6})"\]', region))
        generic = [re.findall(r"#[0-9a-f]{6}", self.cut(f"const {name} = [", "];"))
                   for name in ("PCOLORS", "PCOLORS2")]
        self.assertEqual(len(generic[0]), len(generic[1]))
        pairs.update(zip(*generic))
        self.assertGreater(len(pairs), 100, "the jersey table was not found")
        return sorted(pairs)

    def custom_properties(self):
        """Every value the page gives a custom property, as it restyles a few."""
        found = {}
        for name, value in re.findall(r"(--[\w-]+):\s*(#[0-9a-fA-F]{6})\s*;", self.page):
            found.setdefault(name, set()).add(value.lower())
        return found

    def figure_colours(self):
        """Every colour the stylesheet sets a row's figures and labels in.

        Read out of the rules rather than listed, with a `var()` expanded to
        every value the page ever gives it: a figure added in a new colour, or
        an old one dimmed, is in the set the next time this runs.
        """
        properties = self.custom_properties()
        found = {}

        def take(label, value):
            value = value.strip()
            named = re.fullmatch(r"var\((--[\w-]+)\)", value)
            for colour in (sorted(properties.get(named.group(1), ())) if named else [value]):
                if re.fullmatch(r"#[0-9a-fA-F]{6}", colour):
                    found[f"{label} {colour}"] = colour.lower()

        for kind, value in re.findall(r"\.ribbon-stat\.(\w+) \{ --stat-color: ([^;]+); \}", self.page):
            take(f"figure {kind}", value)
        self.assertGreaterEqual(len(found), 14, "the figures' colours were not found")
        figures = len(found)
        for rule, value in re.findall(
                r"^\s*((?:#playerhud )?\.(?:diplomacy-(?:elo|elo-delta|expected|player)"
                r"|diplomacy-age \.civ-age(?:\.\w+)?|diplomacy-expected \.odds-move\.\w+"
                r"|ai-plan\.\w+)) \{[^}]*?\bcolor: ([^;]+);", self.page, re.M):
            take(rule, value)
        self.assertGreaterEqual(len(found) - figures, 14, "the labels' colours were not found")
        return found

    def test_every_figure_reads_on_every_civilizations_wash(self):
        primaries = sorted({primary for primary, _ in self.jerseys()})
        scenarios = (
            f"const primaries = {json.dumps(primaries)}, figures = {json.dumps(self.figure_colours())};\n"
            f"const FLOOR = {self.FLOOR};\n" + r'''
for (const primary of primaries) {
  const wash = hudRowWash(primary);
  expect(/^#[0-9a-f]{6}$/.test(wash), `${primary} washed to ${wash}`);
  expect(hudRowWash(primary) === wash, "the wash of one colour changed between two askings");
  for (const [name, ink] of Object.entries(figures)) {
    const ratio = contrastRatio(wash, ink);
    expect(ratio >= FLOOR, `${name} on the wash of ${primary} (${wash}) is ${ratio.toFixed(2)}:1`);
  }
}
// The wash is the jersey's own hue, not a grey with a name.
const [red, green, blue] = ["#ca1415", "#156c30", "#004fce"].map(colour => rgbOf(hudRowWash(colour)));
expect(red[0] > 2 * red[1] && red[0] > 2 * red[2], `red washed to ${red}`);
expect(green[1] > 2 * green[0] && green[1] > 2 * green[2], `green washed to ${green}`);
expect(blue[2] > 2 * blue[0] && blue[2] > 2 * blue[1], `blue washed to ${blue}`);
const white = rgbOf(hudRowWash("#f9f9f9"));
expect(Math.max(...white) - Math.min(...white) <= 1, `white washed to a colour: ${white}`);
// One hue in three weights is still three rows: Majapahit, England and Poland
// can share a table.
for (const family of [["#e57574", "#ca1415", "#780001"], ["#74a3f3", "#004fce", "#012a6c"],
                      ["#79e077", "#61bf22", "#156c30"], ["#f9f9f9", "#aeaeae", "#181818"]])
  expect(new Set(family.map(hudRowWash)).size === 3, `${family} washed to ${family.map(hudRowWash)}`);
''')
        self.run_node(self.colour_code() + scenarios)

    def test_every_name_reads_on_its_own_jersey(self):
        scenarios = (
            f"const jerseys = {json.dumps(self.jerseys())}, FLOOR = {self.FLOOR};\n" + r'''
let worn = null;
function pcol() { return worn[0]; }
function pcol2() { return worn[1]; }
for (const jersey of jerseys) {
  worn = jersey;
  const ink = hudJerseyInk(0), ratio = contrastRatio(jersey[0], ink);
  expect(ratio >= FLOOR, `${ink} on ${jersey[0]} (partner ${jersey[1]}) is ${ratio.toFixed(2)}:1`);
}
// The partner is the ink wherever it reads as type: Rome is yellow on purple.
worn = ["#6d00cd", "#f7d801"];
expect(hudJerseyInk(0) === "#f7d801", "Rome lost its yellow");
// Korea's blue on red parts in lightness, which is all a unit token asks, and
// is 2.2:1 as type.
worn = ["#ca1415", "#74a3f3"];
expect(contrastRatio(...worn) < 3 && hudJerseyInk(0) !== "#74a3f3", "Korea's name is set in its partner");
''')
        self.run_node(self.colour_code() + scenarios)

    def test_no_figure_ever_stands_on_the_jersey(self):
        block = self.cut("const PLAYER_HUD_JERSEY_COLUMNS =", "\n") + "\n" + self.fn("playerHudJerseySpan")
        scenarios = r'''
function expect(condition, message) { if (!condition) throw Error(message); }
const span = keys => playerHudJerseySpan(keys.map(key => ({key})));
const same = (keys, wanted) => expect(JSON.stringify(span(keys)) === JSON.stringify(wanted),
  `${keys} → ${JSON.stringify(span(keys))}, wanted ${JSON.stringify(wanted)}`);
// As shipped: the plate opens the row and runs through the player.
same(["lock", "rank", "civ", "watch", "player", "elo", "science", "score"], {first:0, last:4, flush:true});
same(["lock", "rank", "civ", "leader", "watch", "player", "elo"], {first:0, last:5, flush:true});
// A narrow table sheds the rank and the plate still opens the row.
same(["lock", "civ", "watch", "player", "elo"], {first:0, last:3, flush:true});
// A figure dragged in front of the names stays on the wash, and so does the lock beside it.
same(["lock", "score", "rank", "civ", "watch", "player", "elo"], {first:2, last:5, flush:false});
// A figure dragged in among the names ends the plate; the player beyond it stands on the wash.
same(["lock", "rank", "civ", "watch", "science", "player", "elo"], {first:0, last:3, flush:true});
same(["lock", "rank", "elo", "civ", "science", "player"], {first:3, last:3, flush:false});
// With the name hidden the plate forms round the rank, then round the player.
same(["lock", "rank", "watch", "elo", "player"], {first:0, last:2, flush:true});
same(["lock", "elo", "watch", "player", "score"], {first:2, last:3, flush:false});
expect(span(["lock", "elo", "score"]) === null, "a row with no name on it grew a plate");
// Whatever the order, nothing but a name is under the plate.
const names = new Set(["lock", "rank", "civ", "leader", "watch", "player"]);
const keys = ["lock", "rank", "civ", "leader", "watch", "player", "elo", "win", "age", "science", "score"];
const noise = seed => { const value = Math.sin(seed) * 43758.5453; return value - Math.floor(value); };
for (let turn = 0; turn < 400; turn++) {
  const order = keys.map((key, index) => [noise(turn * 12.9898 + index * 78.233), key])
    .sort((a, b) => a[0] - b[0]).map(pair => pair[1]).slice(0, 4 + turn % 8);
  const plate = span(order);
  if (!plate) {
    expect(!order.some(key => ["civ", "rank", "player"].includes(key)), `${order} has no plate`);
    continue;
  }
  for (let index = plate.first; index <= plate.last; index++)
    expect(names.has(order[index]), `${order[index]} is under the plate in ${order}`);
  expect(plate.flush === (plate.first === 0), `${order} is flush without reaching the edge`);
}
'''
        self.run_node(block + scenarios)

    def test_a_column_of_names_is_set_in_one_size_and_none_of_them_is_cut(self):
        block = self.fn("responsiveFitWithColumns") + self.fn("responsiveFit")
        scenarios = r'''
function expect(condition, message) { if (!condition) throw Error(message); }
const responsiveTypeFits = new Map();
let measured = null;
const responsiveTextRange = {
  selectNodeContents(element) { measured = element; },
  getBoundingClientRect() { return {width:measured.run(parseFloat(measured.style.fontSize))}; },
};
function getComputedStyle() { return {paddingLeft:"0", paddingRight:"0"}; }
// A one-line cell `width` wide, whose words stand `advance(size)` wide a letter.
function cell(text, width, advance) {
  return {textContent:text, className:"", style:{}, dataset:{}, isConnected:true,
          clientWidth:width, clientHeight:20, scrollHeight:14, scrollWidth:width,
          parentElement:{clientWidth:width, clientHeight:20},
          run:size => text.length * advance(size),
          getBoundingClientRect() { return {width}; }};
}
const size = element => parseFloat(element.style.fontSize);
const fit = (cells, rule) => {
  const targets = new Map(cells.map(element => [element, rule]));
  responsiveFit(cells, targets);
  return targets;
};
const scaled = size => size * .56;
const handles = () => ["JackOfAllTrades", "TouristTrap", "SwingVote"].map(name => cell(name, 86, scaled));

// Fitted one by one, a column of handles comes out in as many sizes as it has lengths.
let column = handles();
fit(column, {min:9, max:13, exact:true});
expect(size(column[0]) < 11 && size(column[2]) === 13, `apart: ${column.map(size)}`);

// Together, every handle takes the size the longest one needs.
column = handles();
const together = {min:9, max:13, exact:true, together:true};
const targets = fit(column, together);
expect(new Set(column.map(size)).size === 1, `together: ${column.map(size)}`);
expect(size(column[0]) >= 10 && column[0].run(size(column[0])) <= 86, `the longest is cut: ${column.map(size)}`);
expect(!column.some(element => element.dataset.fitClipped), "a handle that fits is marked clipped");

// One handle going stale puts its whole column back on the bench, and nobody else's.
const other = cell("Rome", 60, scaled);
targets.set(other, {min:9, max:13, exact:true});
expect(responsiveFitWithColumns([column[1]], targets).length === 3, "a column was fitted in part");
expect(responsiveFitWithColumns([other], targets).length === 1, "a lone cell took a column with it");

// The interface face tracks looser as it gets smaller: a run measured at the
// ceiling stands wider at the fitted size than the ratio promised. The fit is
// measured again where it landed, so the name still ends inside its cell.
const loosening = size => size * (.56 + (13 - size) * .008);
column = ["JackOfAllTrades", "TouristTrap"].map(name => cell(name, 86, loosening));
fit(column, together);
expect(column[0].run(size(column[0])) <= 86,
  `a ${column[0].run(size(column[0])).toFixed(1)}px name in an 86px cell at ${size(column[0])}px`);
expect(size(column[0]) === size(column[1]) && size(column[0]) >= 9.5, `after the second look: ${column.map(size)}`);

// A name too long for the floor stays at the floor, says so, and holds the column there.
column = [cell("A name nobody could fit in a cell this narrow", 86, scaled), cell("Rome", 86, scaled)];
fit(column, together);
expect(size(column[0]) === 9 && size(column[1]) === 9, `at the floor: ${column.map(size)}`);
expect(column[0].dataset.fitClipped === "true" && !column[1].dataset.fitClipped, "the wrong cell is marked clipped");
'''
        self.run_node(block + scenarios)

    def test_the_row_is_drawn_with_both_grounds(self):
        # The row hands the stylesheet its two grounds and its ink, computed
        # from the one jersey colour, and the stylesheet paints them.
        self.assertIn("--civ-ink:${hudJerseyInk(p.id)};--civ-wash:${hudRowWash(color)}", self.source)
        self.assertIn('class="diplomacy-card jersey-row ', self.source)
        row = self.page[self.page.index("#playerhud .diplomacy-card.jersey-row {"):]
        row = row[:row.index("}")]
        self.assertIn("var(--civ-wash)", row)
        self.assertIn("var(--civ)", row, "the row lost the cap of its jersey at the score end")
        plate = self.page[self.page.index("#playerhud .diplomacy-card.jersey-plate::before {"):]
        plate = plate[:plate.index("}")]
        self.assertIn("grid-column: var(--hud-jersey-start) / var(--hud-jersey-end);", plate)
        self.assertIn("background: var(--civ);", plate)
        # The three name columns are the ones fitted as columns.
        for column in ("#playerhud .diplomacy-civ-link .diplomacy-civ",
                       "#playerhud .diplomacy-leader-name", "#playerhud .diplomacy-player"):
            self.assertRegex(self.source,
                             r'\{selector:"' + re.escape(column) + r'",[^}]*\btogether:true\}')


if __name__ == "__main__":
    unittest.main()


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class EachColumnFromEloRightMarksItsLeader(unittest.TestCase):
    """The highest value of each standings column stands out.

    Operator 2026-11-26: "for each from elo right or so in player hud, lets
    highlight the highest value in the col. to give a quick sense of the rough
    eladers in each col nonzero ties can all highlight. zero ties can have no
    highlight." The figures from Elo to Win odds now join the stat columns,
    which always carried `.category-leader` but never showed it: the HUD's
    bare-cell rule wiped its wash.
    """

    def setUp(self):
        self.palette = (ASSETS / "app_palette.js").read_text(encoding="utf-8")
        self.app = (ASSETS / "app.js").read_text(encoding="utf-8")
        self.page = INDEX.read_text(encoding="utf-8")

    def test_a_column_s_highest_ties_included_and_none_at_zero(self):
        start = self.palette.index("const PLAYER_HUD_LEADER_FACTS =")
        code = self.palette[start:self.palette.index("\n}\n", start) + 3]
        rows = [
            {"elo": 1200, "elo_delta": 0, "win_delta": -3, "win": None, "nukes": 0, "score": 300},
            {"elo": 1374, "elo_delta": 0, "win_delta": -8, "win": None, "nukes": 2, "score": 500},
            {"elo": 1374, "elo_delta": 0, "win_delta": None, "win": None, "nukes": 0, "score": 420},
        ]
        done = subprocess.run(
            [NODE, "-e", code + f"const rows = {json.dumps(rows)};\n"
             "console.log(JSON.stringify(Object.fromEntries(playerHudColumnHighs(rows, row => Object.entries(row)))));"],
            capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr[-2000:])
        # Two seats tied on top both lead; a column all at nought, or with no
        # value in it, leads with nobody; the highest of a column below zero
        # is still its highest.
        self.assertEqual(json.loads(done.stdout), {"elo": 1374, "win_delta": -3, "nukes": 2, "score": 500})

    def test_the_figures_from_elo_to_win_odds_take_part(self):
        keys = re.findall(r'\{key:"(\w+)"', self.app[self.app.index("const PLAYER_HUD_COLUMNS = ["):
                                                     self.app.index("const PLAYER_HUD_COLUMN_BY_KEY")])
        facts = re.search(r"const PLAYER_HUD_LEADER_FACTS = \[([^\]]*)\];", self.palette).group(1)
        self.assertEqual(re.findall(r'"(\w+)"', facts), keys[keys.index("elo"):keys.index("win") + 1])
        # Each column's highest is worked out once a paint, over the stat
        # columns and those figures together, and a cell is marked when its
        # value is its column's highest.
        self.assertIn("const highestByStat = playerHudColumnHighs(scoreRankedMajors, p => [", self.app)
        self.assertIn("...PLAYER_HUD_LEADER_FACTS.map(key => [key, playerHudSortValue(p, key, statsByPlayer.get(p.id), rankById)])]);", self.app)
        self.assertIn('const leads = key => playerHudSortValue(p, key, stats, rankById) === highestByStat.get(key) ? " category-leader" : "";', self.app)
        for key in re.findall(r'"(\w+)"', facts):
            self.assertIn(f'`<button class="diplomacy-identity${{leads("{key}")}}" data-hud-col="{key}" ${{dossierAttrs}}>`', self.app)
        self.assertIn('class="ribbon-stat ${kind}${isLeader ? " category-leader" : ""}"', self.app)

    def test_a_leader_is_a_dark_tile_the_bare_cell_rule_does_not_wipe(self):
        rule = ("  #playerhud .ribbon-stat.category-leader,\n"
                "  #playerhud .diplomacy-identity.category-leader .diplomacy-identity-field {\n"
                "    border-radius: 4px; background: #000000a6;\n"
                "    box-shadow: inset 0 0 0 1.5px var(--stat-color, currentColor);\n"
                "  }\n")
        self.assertIn(rule, self.page)
        # It comes after the rule that strips the HUD's cells bare, and after
        # their hover, at the same weight, so it is the one that holds.
        self.assertLess(self.page.index("  #playerhud .ribbon-stat {\n"), self.page.index(rule))
        self.assertLess(self.page.index("  #playerhud .ribbon-stat:hover,"), self.page.index(rule))


class QuickDealsEndsTheLaunchBar(unittest.TestCase):
    """Quick Deals is the last button of the launch bar, not one of the hooks.

    Operator 2026-11-26: "quickdeals should technically be on the right end of
    the left menu". The mod that adds Quick Deals to Civilization VI puts its
    button at the right end of the launch bar (upper left); it stood among the
    partial-screen hooks (upper right) here.
    """

    def setUp(self):
        self.page = INDEX.read_text(encoding="utf-8")

    def test_the_button_follows_the_launch_tabs_and_left_the_hooks(self):
        bar = self.page[self.page.index('<div id="launchbar"'):self.page.index('<div id="livepause"')]
        hooks = self.page[self.page.index('<div id="hooksbar"'):self.page.index('<div id="leaderribbon"')]
        self.assertLess(bar.index('<div id="launchtabs"></div>'), bar.index('id="tradebtn"'))
        self.assertEqual(self.page.count('id="tradebtn"'), 1)
        self.assertNotIn("tradebtn", hooks)
        self.assertIn('id="diplomacybtn"', hooks)
        # Only the Civilization VI screen shows it, as only that screen shows
        # the hooks: a watched world's launch bar keeps its two trees alone.
        self.assertIn("  #launchbar > #tradebtn { display: none; }\n"
                      "  body.civ6-frame #launchbar > #tradebtn { display: grid; }\n", self.page)


class TheDedicatedDisplayShowsTheHud(unittest.TestCase):
    """The window the keeper opens beside a live game exists to show the player
    HUD. Its Chrome profile keeps this page's localStorage across launches, so
    one stray click that hid the players overlay hid it for every game after —
    on 2026-09-10 the display beside a live Emperor game showed a bare map."""

    def test_victory_tracker_keeps_its_scroll_box_and_ranks_by_estimate(self):
        # Operator, 2026-11-09: "trying to scroll doesn't really seem to work",
        # and the paths should stand "closest to victory based on our
        # estimation".
        source = (REPO / "web" / "assets" / "app.js").read_text(encoding="utf-8")
        draw = source[source.index("function drawPlayerHud() {"):]
        draw = draw[:draw.index("\n}\n")]
        self.assertIn("patchHudMarkup(victoryHud, overview);", draw)
        self.assertNotIn("victoryHud.innerHTML", draw)
        # A keyed path keeps its element when the order changes, and what the
        # later passes write on a row survives the repaint.
        overview = source[source.index("function playerHudOverview()"):]
        overview = overview[:overview.index("\n}\n")]
        self.assertIn('class="victory-track" data-key="${track.id}"', overview)
        self.assertIn('data-key="${player.id}" data-victory-focus=', overview)
        self.assertIn('new Set(["hidden", "style", "data-fit-clipped"])', source)
        # The planner's clock orders the paths everywhere but the dedicated
        # display, which keeps its own meter ranking.
        self.assertIn("rankVictoryTracksBySoonest(enabledVictoryTracks, contenders);", overview)
        road = source[source.index("function victoryRoadTurns("):]
        road = road[:road.index("\n}\n")]
        self.assertIn("player.victory_eta?.find(", road)
        self.assertIn('road === "score"', road)
        page = (REPO / "web" / "index.html").read_text(encoding="utf-8")
        self.assertIn("#victoryhud .hud-rail { overflow-x: hidden; overflow-y: auto; overflow-anchor: none; }", page)

    def test_dedicated_mode_shows_the_player_hud_and_victory_tracker(self):
        source = (REPO / "web" / "assets" / "app.js").read_text(encoding="utf-8")
        gate = source.index('get("display") === "dedicated"')
        # After the saved overlays are read, so the forced values win.
        self.assertGreater(gate, source.index("OVERLAY_STORAGE_KEY) ||"))
        block = source[gate:gate + 400]
        self.assertIn("OVERLAY_VISIBILITY.players = true", block)
        self.assertIn("OVERLAY_VISIBILITY.victory = true", block)
        for name in ("minimap", "controls", "lenses"):
            self.assertIn(f"OVERLAY_VISIBILITY.{name} = false", block)

    def test_the_tracker_is_the_right_hand_panel_to_the_bottom(self):
        # Operator, 2026-10-30: the standings span the top; the tracker stands
        # under them on the right, all the way to the bottom, at every width.
        source = (REPO / "web" / "assets" / "app.js").read_text(encoding="utf-8")
        gate = source.index('get("display") === "dedicated"')
        self.assertIn('document.body.classList.add("display-dedicated")',
                      source[gate:source.index("\n}\n", gate)])
        html = INDEX.read_text(encoding="utf-8")
        start = html.index("body.display-dedicated #maparea #victoryhud {")
        rule = html[start:html.index("}", start)]
        self.assertIn("right: var(--panel-edge)", rule)
        self.assertIn("bottom: var(--panel-edge)", rule)
        self.assertIn("left: auto", rule)
        # Two ids and a class plus `body`: outranks the compact band's
        # `#maparea.player-hud-compact #victoryhud` and its ribbon columns.
        self.assertIn("body.display-dedicated #maparea #victoryhud .victory-ribbon {", html)

    def test_a_saved_tracker_position_cannot_move_the_panel(self):
        source = (REPO / "web" / "assets" / "app.js").read_text(encoding="utf-8")
        loaded = source.index("localStorage.getItem(HUD_LAYOUT_STORAGE_KEY)")
        start = source.index("if (DEDICATED_DISPLAY && HUD_LAYOUT.victory) {")
        block = source[start:source.index("\n}\n", start)]
        self.assertGreater(start, loaded)
        self.assertLess(start, source.index("HUD_LAYOUT[name] = serializeHudWidget"))
        self.assertIn("delete HUD_LAYOUT.victory;", block)
        # Only a rail's width survives the reload, through the stylesheet.
        self.assertIn('"--dedicated-victory-width"', block)
        self.assertIn("window.innerWidth * .45", block)
        html = INDEX.read_text(encoding="utf-8")
        self.assertIn("width: var(--dedicated-victory-width, max(var(--victory-hud-width), 224px));", html)

    def test_the_paths_nearest_a_win_stand_first_with_two_leaders(self):
        # Operator, 2026-10-30: "rank the victory tracker victory types in
        # order of closest to victory ... top 2 only".
        source = (REPO / "web" / "assets" / "app.js").read_text(encoding="utf-8")
        self.assertIn("const DEDICATED_VICTORY_LEADERS = 2;", source)
        overview = source[source.index("function playerHudOverview()"):]
        overview = overview[:overview.index("\n}\n")]
        ranking = overview[overview.index("if (DEDICATED_DISPLAY) {"):]
        ranking = ranking[:ranking.index("\n  }\n")]
        self.assertIn("+victoryMetric(player, track.id).progress", ranking)
        self.assertIn("enabledVictoryTracks.sort((a, b) => leading.get(b.id) - leading.get(a.id));", ranking)
        # The ranking happens before the tracks are drawn from that list.
        self.assertLess(overview.index("enabledVictoryTracks.sort("),
                        overview.index("const victoryTracks = enabledVictoryTracks.map("))
        sync = source[source.index("function syncVictoryLeaderCapacity("):]
        sync = sync[:sync.index("\n}\n")]
        self.assertIn("fitVictoryTracker(victoryHud, tracks, DEDICATED_DISPLAY ? DEDICATED_VICTORY_LEADERS : null)", sync)
        # No third row for the viewed civilization on the dedicated display.
        self.assertIn('(DEDICATED_DISPLAY || entry.dataset.victoryFocus !== "true")', sync)

    def test_the_keeper_opens_the_page_in_dedicated_mode(self):
        keeper = (Path(__file__).resolve().parent / "ops" / "civvis-display-keeper.mjs").read_text(encoding="utf-8")
        self.assertIn("?display=dedicated", keeper)

    def test_the_tree_only_opens_on_request(self):
        html = INDEX.read_text(encoding="utf-8")
        self.assertRegex(html, r'<div id="tree">', "the tech/civics tree is a modal")
        self.assertNotRegex(html, r'<div id="tree"[^>]*class="[^"]*\bopen\b')


ACTIVE_AREA_SCENARIOS = r"""
const key = pos => pos[0] + "," + pos[1];
const militaryUnit = unit => !["builder", "trader", "settler"].includes(unit.type);
const whexDist = (a, b) => Math.max(Math.abs(a[0] - b[0]), Math.abs(a[1] - b[1]));
const statePlayerAnchor = (st, pid) =>
  st.cities.find(city => city.owner === pid && city.is_capital) || null;
__BLOCK__
const tiles = [];
for (let y = 0; y < 40; y++) for (let x = 0; x < 60; x++) tiles.push({pos:[x, y]});
const players = [{id:0}, {id:1, at_war_with_me:true}, {id:2, at_war_with_me:false}];
const capital = {owner:0, is_capital:true, pos:[10, 10], hp:200, wall_hp:0, wall_max:0};
const outpost = {owner:0, pos:[50, 5], hp:60, wall_hp:0, wall_max:0};
const world = (seed, turn, units, cities = []) =>
  ({seed, turn, units, cities:[capital, ...(seed === 1 ? [outpost] : []), ...cities], players, map:{tiles}});
const unit = (owner, pos, extra = {}) => ({owner, pos, type:"musketman", hp:100, ...extra});
const near = (pos, x, y, r = 0) => !!pos && whexDist(pos, [x, y]) <= r;
const check = (ok, why) => { if (!ok) throw new Error(why); };
let t = 0;
const at = (st, wait = 5000) => activeAreaCenter(st, 0, (t += wait));

check(near(at(world(1, 10, [unit(0, [11, 10])])), 10, 10), "no activity yet: frame the capital");

const army = [unit(0, [30, 20]), unit(0, [31, 20]), unit(0, [30, 21])];
const addis = {owner:1, pos:[32, 20], hp:200, wall_hp:100, wall_max:400};
const guard = unit(1, [33, 20]);
check(near(at(world(1, 10, [...army, guard], [addis])), 32, 20),
      "a siege of an enemy city outweighs a quiet capital, centred on the city");
check(activeAreaSubjects(world(1, 10, [...army, guard], [addis]), 0, (t += 5000)).length === (2 * ACTIVE_AREA.frame + 1) ** 2,
      "the frame is the hot tile's neighbourhood, not the empire");

check(near(at(world(1, 10, [...army, unit(0, [31, 21], {hp:40}), guard], [addis])), 32, 20),
      "more fighting in the same place keeps the frame");
check(near(at(world(1, 10, [...army, guard, unit(0, [5, 35], {type:"builder"})], [addis])), 32, 20),
      "a builder elsewhere does not steal the frame from a siege");
const rivalsTown = {owner:2, pos:[13, 10], hp:0, wall_hp:0, wall_max:200};
check(near(at(world(1, 10, [...army, guard, unit(0, [11, 10]), unit(2, [12, 10])], [addis, rivalsTown])), 32, 20),
      "a city at peace with us is not our front");

const raid = [unit(1, [50, 6]), unit(1, [51, 6]), unit(1, [49, 5])];
const defenders = [unit(0, [50, 4], {hp:20}), unit(0, [51, 5], {hp:30}), unit(0, [49, 4], {hp:10})];
const twoFronts = world(1, 10, [...army, guard, ...raid, ...defenders], [addis]);
check(near(at(twoFronts), 50, 5, 1), "a clearly hotter front takes the frame");
const breached = {...addis, wall_hp:0, hp:50};
const raidOver = world(1, 10, [...army, guard, ...defenders], [breached]);
check(near(at(raidOver, 1000), 50, 5, 1), "the next hotter front waits out the hold");
check(near(at(world(1, 10, [...army, guard, ...defenders], [breached]), 4000), 32, 20),
      "and takes the frame once the hold has passed");

const sieged = [...army, guard, ...defenders];
const west = [[10, 30], [11, 30], [10, 31], [11, 31]].map(pos => unit(0, pos, {hp:30}));
const westRaider = unit(1, [12, 32]);
check(near(at(world(1, 10, [...sieged, ...west, westRaider], [breached])), 32, 20),
      "a front only slightly hotter does not take the frame");
const westWorse = [...west, unit(0, [10, 32], {hp:0}), unit(0, [11, 32], {hp:0})];
check(near(at(world(1, 10, [...sieged, ...westWorse, westRaider], [breached])), 11, 31, 1),
      "a front clearly hotter does");
check(near(activeAreaTrack.center, 10, 32), "centred on the west front's hottest tile");
const beside = [[13, 32], [14, 32], [15, 32], [16, 32]].map(pos => unit(0, pos, {hp:0}));
check(near(at(world(1, 10, [...sieged, ...westWorse, westRaider, unit(1, [15, 33]), ...beside], [breached])), 10, 32),
      "a hotter tile three tiles over is still the same view");

check(near(at(world(2, 3, [unit(0, [11, 10])])), 10, 10), "a new world starts over at its capital");
let marching = [unit(0, [11, 10])];
at(world(2, 3, marching));
for (let step = 0; step < 4; step++) {
  marching = [unit(0, [11, 10]), ...[0, 1, 2, 3].map(i => unit(0, [40 + i, 30 + step]))];
  at(world(2, 4 + step, marching));
}
check(near(activeAreaTrack.center, 41, 32, 3), "where our units keep arriving is where we are acting");
check(near(at(world(2, 30, marching)), 41, 32, 3),
      "a quiet stretch keeps the last front rather than snapping home");
check(activeAreaTrack.memory.size <= 1, "arrivals older than the memory are forgotten");
const founded = {owner:0, pos:[5, 5], hp:200, wall_hp:0, wall_max:0};
check(near(at(world(2, 30, marching, [founded])), 5, 5), "a city that becomes ours is where we are acting");
check(near(at(world(2, 5, [unit(0, [11, 10])])), 10, 10), "a reload to an earlier turn starts over");
console.log("active area checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class TheDedicatedDisplayFollowsTheActiveArea(unittest.TestCase):
    """The page beside a live game is filmed with the Civ VI window, and the
    operator asked for both to stay close on where our civilization is acting
    (2026-10-04). Framing the whole empire showed the whole map."""

    def test_only_the_dedicated_display_frames_the_active_area(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        body = source[source.index("function watchedEmpireSubjects(player)"):]
        body = body[:body.index("\n}\n")]
        self.assertIn("if (DEDICATED_DISPLAY) {", body)
        self.assertIn("activeAreaSubjects(state, player)", body)

    def test_the_frame_follows_the_hottest_front(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        start = source.index("const ACTIVE_AREA = {")
        block = source[start:source.index("function watchedEmpireSubjects(player)")]
        done = subprocess.run(
            [NODE, "-e", ACTIVE_AREA_SCENARIOS.replace("__BLOCK__", block)],
            capture_output=True, text=True, timeout=60,
        )
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("active area checks passed", done.stdout)


VICTORY_FIT_SCENARIOS = r"""
const clampPanelDimension = (value, low, high) => Math.min(high, Math.max(low, value));
__BLOCK__
const eq = (actual, wanted, note) => {
  if (JSON.stringify(actual) !== JSON.stringify(wanted))
    throw new Error(`${note}: got ${JSON.stringify(actual)}, wanted ${JSON.stringify(wanted)}`);
};
const layout = (width, height, total = 4, count = 6, focusRank = -1, fixedRows = null) =>
  chooseVictoryLayout({width, height, count, total, focusRank, fixedRows});
const brief = l => [l.columns, l.rows, l.type];

eq(brief(layout(191, 649)), [1, 3, 15], "a laptop rail holds all six paths with three leaders");
eq(brief(layout(1010, 92)), [6, 3, 13], "a short band is one row of columns with whole rows");
eq(brief(layout(155, 458)), [1, 2, 14], "a narrow rail gives up the third leader, not the type");
eq(layout(374, 593).columns, 2, "a widened rail divides into columns");
eq(layout(374, 593).rows, 4, "and the columns buy back the leaders");
eq(brief(layout(191, 449, 4, 6, 3)), [1, 1, 13.5], "a watched civilization takes the runner-up's place");
eq(layout(191, 854, 4, 6, -1, 2).rows, 2, "the dedicated display always lists two");

for (const total of [1, 2, 4, 8]) {
  for (const width of [141, 155, 191, 312, 374, 572, 797, 1010]) {
    for (let height = 30; height < 1400; height += 7) {
      const fit = layout(width, height, total);
      if (fit.rows >= DEFAULT_VICTORY_LEADERS && fit.type < VICTORY_TYPE_THIRD_ROW_FLOOR)
        throw new Error(`a third leader cost too much type at ${width}x${height}: ${fit.type}`);
      if (fit.type < VICTORY_TYPE_FLOOR)
        throw new Error(`below the type floor at ${width}x${height}: ${fit.type}`);
      if (victoryBandHeight(fit.rows, fit.type) > fit.room + 1e-6 && fit.type !== VICTORY_TYPE_FLOOR)
        throw new Error(`a row was cut at ${width}x${height}: ${brief(fit)}`);
    }
  }
}
console.log("victory fit checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class TheVictoryTrackerFitsItsRoom(unittest.TestCase):
    """The arrangement, the leaders shown and the one type size are one decision
    (operator, 2026-11-09: the tracker was "hard to read"). The numbers here are
    real boxes measured in Chrome at common window sizes."""

    def test_the_fit(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        start = source.index("const DEFAULT_VICTORY_LEADERS = 3;")
        block = source[start:source.index("// Returns the arrangement and the rows")]
        done = subprocess.run(
            [NODE, "-e", VICTORY_FIT_SCENARIOS.replace("__BLOCK__", block)],
            capture_output=True, text=True, timeout=60,
        )
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("victory fit checks passed", done.stdout)


CULTURE_RANK_SCENARIOS = r"""
__FIGURES__
__COMPACT__
const victoryMetric = (player, road) => player.victories[road];
const reportedTurn = () => 2;
const track = {id:"culture"}, tieField = "civics";
const seat = (id, tourists, target, civics, progress) =>
  ({id, score:0, victories:{culture:{tourists, target, civics, civic_total:50, progress}}});
const order = contenders => {
__RANKED__
  return ranked.map(player => player.id);
};
const eq = (actual, wanted, note) => {
  if (JSON.stringify(actual) !== JSON.stringify(wanted))
    throw new Error(`${note}: got ${JSON.stringify(actual)}, wanted ${JSON.stringify(wanted)}`);
};
// The row prints what the game prints, and the heading names what is counted.
eq(victoryTrackerStats("culture", {tourists:34, target:87}), "34/87", "tourists over the tourists needed");
eq(victoryTrackerStats("culture", {tourists:1234, target:5678}), "1.2k/5.7k", "long counts are compacted");
eq(victoryTrackerStats("culture", {}), "0/1", "a seat with no figures yet");
eq(victoryTrackerSubheading("culture"), "<span>Civics</span><span>Tourists</span>", "the heading");
// Nearest its own bar leads, not the larger crowd.
eq(order([seat(0, 60, 120, 9, 50), seat(1, 40, 50, 3, 80)]), [1, 0], "the ratio outranks the count");
// One tourist against a far bar rounds to 0.0% on the meter. It still leads a
// seat with none, whatever that seat's civics.
eq(order([seat(0, 0, 10, 30, 0), seat(1, 1, 3000, 5, 0)]), [1, 0], "counts decide under the rounding");
// The meter stops at a hundred; the ratio does not.
eq(order([seat(0, 101, 100, 40, 100), seat(1, 150, 100, 2, 100)]), [1, 0], "past the bar");
// With no tourists anywhere the civics the left meter draws keep the order.
eq(order([seat(0, 0, 1, 4, 0), seat(1, 0, 1, 11, 0), seat(2, 0, 1, 7, 0)]), [1, 2, 0], "civics break a tie");
// Every other path still ranks on its meter and prints what it printed.
eq(victoryLead("science", {progress:45, tourists:9, target:1}), 45, "other paths rank on progress");
eq(victoryTrackerStats("science", {projects:1, project_target:4, techs:30, tech_total:77}), "1/4", "launches");
eq(victoryTrackerStats("religious", {converted:2, target:5}), "2/5", "religious");
eq(victoryTrackerStats("diplomatic", {points:7}), "7/20", "diplomatic");
eq(victoryTrackerStats("domination", {capitals:1, target:6}), "1/6", "domination");
eq(victoryTrackerStats("score", {points:412}), "412", "score");
console.log("culture rank checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class CultureIsLedOnTouristsOverTheTouristsNeeded(unittest.TestCase):
    """Operator, 2026-11-15: the culture path should read "tourists / required
    tourists to win" like the game, sorted "by whoever is leading this ratio".
    It printed a bare tourist count under "Tourism" and ranked on the meter's
    percent, which is rounded to a tenth and stops at a hundred."""

    def test_the_rows_and_their_order(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        figures = source[source.index("function victoryValue("):
                         source.index("function victoryDescription(")]
        compact = source[source.index("function compactStat("):]
        compact = compact[:compact.index("\n}\n") + 3]
        overview = source[source.index("function playerHudOverview()"):]
        overview = overview[:overview.index("\n}\n")]
        ranked = overview[overview.index("const ranked = [...contenders]"):]
        ranked = ranked[:ranked.index("      });\n") + len("      });")]
        # The tracker's own comparator is what runs below, not a copy of it.
        self.assertIn("victoryLead(track.id, bm) - victoryLead(track.id, am)", ranked)
        script = (CULTURE_RANK_SCENARIOS.replace("__FIGURES__", figures)
                  .replace("__COMPACT__", compact).replace("__RANKED__", ranked))
        done = subprocess.run([NODE, "-e", script], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("culture rank checks passed", done.stdout)


DOMINATION_ROW_SCENARIOS = r"""
__FIGURES__
__COMPACT__
const victoryMetric = (player, road) => player.victories[road];
let turn = 2;
const reportedTurn = () => turn;
const seat = (id, capitals, cities, score = 0) => ({id, score, victories:{domination:
  {capitals, target:6, cities, city_total:40, progress:100 * capitals / 6}}});
const order = contenders => {
  const track = {id:"domination"};
__TIE__
__RANKED__
  return ranked.map(player => player.id);
};
const eq = (actual, wanted, note) => {
  if (JSON.stringify(actual) !== JSON.stringify(wanted))
    throw new Error(`${note}: got ${JSON.stringify(actual)}, wanted ${JSON.stringify(wanted)}`);
};
const metric = {capitals:2, target:6, cities:9, city_total:36, progress:33.3};
// The meter is cities out of every city on the map; the figure is capitals.
eq(victoryMeterFill("domination", metric), 25, "the meter is the share of the map's cities");
eq(victoryMeter("domination", metric, victoryMeterFill("domination", metric)),
   '<div class="victory-meter"><b style="width:25%"></b></div>', "one whole meter");
// The row prints that share as a column of its own, left of the capitals.
eq(victoryTrackerStats("domination", metric), '<i class="victory-share">25%</i>2/6',
   "the share of the map's cities, then capitals held over capitals in the game");
eq(victoryValue("domination", metric), "2/6", "the dossier's figure is still the capitals alone");
// It is blank on turn 1 and starts on turn 2.
turn = 1;
eq(victoryTrackerStats("domination", metric), "2/6", "turn 1 prints the capitals alone");
eq(victoryCityShare(metric), "", "turn 1 is blank");
eq(victoryCityShare(metric, 2), "25%", "turn 2 starts the column");
turn = 2;
eq(victoryCityShare({cities:1, city_total:15}), "7%", "a whole percent");
eq(victoryCityShare({city_total:15}), "0%", "a civilization that holds no city");
eq(victoryCityShare({cities:15, city_total:15}), "100%", "every city on the map");
eq(victoryCityShare({capitals:3, target:6, progress:50}), "", "an older frame has no share to print");
eq(victoryTrackerStats("domination", {capitals:3, target:6, progress:50}), "3/6", "nor a column for it");
// No other path gains a column.
eq(victoryTrackerStats("religious", {converted:1, target:4, cities:9, city_total:36}), "1/4", "religious");
eq(victoryTrackerSubheading("domination"), "<span>Cities</span><span>Capitals</span>", "the heading");
eq(victoryDescription("domination", metric),
   "9 of 36 cities on the map · 2 of 6 original capitals controlled, their own counted", "the hover");
// A civilization with no city yet, and a map with none.
eq(victoryMeterFill("domination", {capitals:1, target:6, city_total:12, progress:16.7}), 0, "no city");
eq(victoryMeterFill("domination", {capitals:1, target:6, cities:0, city_total:0, progress:16.7}), 16.7,
   "an empty map has no share to draw");
// A frame recorded before the server counted cities keeps its capitals meter.
eq(victoryMeterFill("domination", {capitals:3, target:6, progress:50}), 50, "an older frame");
eq(victoryDescription("domination", {capitals:3, target:6}),
   "3 of 6 original capitals controlled, their own counted", "an older frame's hover");
// Every other path's meter is its progress, as before.
eq(victoryMeterFill("religious", {converted:1, target:4, progress:25, cities:9, city_total:36}), 25, "religious");
eq(victoryMeterFill("score", {points:400, progress:61.5}), 61.5, "score");
// Capitals lead. Level on capitals, the larger empire stands first.
eq(order([seat(0, 1, 12), seat(1, 2, 3), seat(2, 1, 5)]), [1, 0, 2], "capitals, then cities");
eq(order([seat(0, 1, 4, 90), seat(1, 1, 4, 300), seat(2, 1, 7)]), [2, 1, 0], "then score");
console.log("domination row checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class DominationShowsCitiesOnItsMeterAndCapitalsBesideIt(unittest.TestCase):
    """Operator, 2026-11-19: "domination in victory tracker should track cities
    and capitals. bar can show cities by percent of cities on the map. on the
    right should be capitals count N/total". The meter had drawn the capitals
    the figure already printed, so it stood still for most of a game.

    Operator, 2026-11-19, later: "lets add a short city col to the left of
    capitalis. this col can list the percent of all cities on the map
    (including city states) that belong to the civ. should be left blank on
    turn 1 and start on turn 2."""

    def test_the_meter_the_figure_and_the_order(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        figures = source[source.index("function victoryMeterFill("):
                         source.index("const DEFAULT_VICTORY_LEADERS = 3;")]
        compact = source[source.index("function compactStat("):]
        compact = compact[:compact.index("\n}\n") + 3]
        overview = source[source.index("function playerHudOverview()"):]
        overview = overview[:overview.index("\n}\n")]
        tie = overview[overview.index("    const tieField = "):]
        tie = tie[:tie.index("\n") + 1]
        ranked = overview[overview.index("const ranked = [...contenders]"):]
        ranked = ranked[:ranked.index("      });\n") + len("      });")]
        # Both places a path's meter is drawn take their fill from one rule.
        self.assertEqual(source.count("victoryMeterFill(track.id, metric)"), 2)
        script = (DOMINATION_ROW_SCENARIOS.replace("__FIGURES__", figures)
                  .replace("__COMPACT__", compact).replace("__TIE__", tie)
                  .replace("__RANKED__", ranked))
        done = subprocess.run([NODE, "-e", script], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("domination row checks passed", done.stdout)


HUD_WINDOW_SCENARIOS = r"""
const SPEC = false;
__CONSTS__
const stubHud = {style:{getPropertyValue:() => ""}};
const document = {getElementById:id => id === "maparea" ? {clientHeight:900} : ribbonHost};
const getComputedStyle = () => ({getPropertyValue:() => ""});
const playerHudRowPitch = () => 32;
let ribbonHost = null;
__BLOCK__
const eq = (actual, wanted, note) => {
  if (JSON.stringify(actual) !== JSON.stringify(wanted))
    throw new Error(`${note}: got ${JSON.stringify(actual)}, wanted ${JSON.stringify(wanted)}`);
};
const seats = n => Array.from({length:n}, (_, id) => ({id}));
const keys = majors => new Map(majors.map(p => [p.id, "s" + p.id]));
const plan = (n, scrollTop, clientHeight, locked = [], open = -1, cityRows = 0) => {
  const majors = seats(n);
  ribbonHost = {querySelector:() => clientHeight === null ? null : {scrollTop, clientHeight}};
  return {majors, w:planPlayerHudWindow(majors, keys(majors),
    new Set(locked.map(id => "s" + id)), open >= 0 ? majors[open] : null, cityRows)};
};
const built = ({majors, w}) => majors.map((p, i) => w.built(i) ? i : -1).filter(i => i >= 0);
// A stock-sized table is never windowed: every row stays real.
eq(plan(30, 0, 300).w, null, "thirty seats keep every row");
// A hundred seats at the top: what shows, plus eight rows of overscan.
const top = plan(100, 0, 320);
eq(built(top), Array.from({length:19}, (_, i) => i), "first rows");
// One spacer per unbuilt run, spanning exactly the rows it replaces.
eq(top.majors.map((p, i) => top.w.built(i) ? "" : top.w.gap(i)).join("").match(/span (\d+)/g),
   ["span 81"], "spacer spans the rest of the table");
const middle = plan(100, 1600, 320);
const seatsOf = built(middle);
eq(seatsOf[0], 42, "eight above row 50");
eq(seatsOf[seatsOf.length - 1], 68, "eight below");
const gaps = middle.majors.map((p, i) => middle.w.built(i) ? "" : middle.w.gap(i)).join("").match(/span (\d+)/g);
eq(gaps, ["span 42", "span 31"], "a spacer either side");
// A locked row exists wherever the table is scrolled, so it can hold at an edge.
eq(built(plan(100, 0, 320, [54])).includes(54), true, "locked row is built");
// An open city list hangs under its seat: the rows below read back through it.
const listed = plan(100, 0, 320, [], 2, 40);
eq(built(listed).includes(2), true, "open seat is built");
eq(built(listed).slice(-1)[0], 10, "the list fills the view, so overscan stops short of the rows under it");
const deep = plan(100, 1600, 320, [], 2, 20);
eq(built(deep).filter(i => i > 2)[0], 22, "the list's rows shift every later seat");
// Scrolling near the edge of what exists asks for more; inside it does not.
const live = plan(100, 0, 320);
eq(live.w.stale({scrollTop:0, clientHeight:320}), false, "top of a fresh window");
eq(live.w.stale({scrollTop:15 * 32, clientHeight:320}), true, "two rows from the unbuilt part");
// The masthead takes its share of a 900px map in whole rows and never more.
eq(playerHudSeatHeight(100, 900), 370, "ten rows and the chrome");
eq(playerHudSeatHeight(8, 900), 306, "a table that fits stays whole");
console.log("hud window checks passed");
"""


TURN_GAUGE_SCENARIOS = r"""
let state = {};
__CLOCK__
const eq = (actual, wanted, note) => {
  if (JSON.stringify(actual) !== JSON.stringify(wanted))
    throw new Error(`${note}: got ${JSON.stringify(actual)}, wanted ${JSON.stringify(wanted)}`);
};
const rise = () => {
  const fill = /style="--rise:([\d.]+)%"/.exec(turnGauge());
  return fill ? +fill[1] : null;
};
// The operator's two ends: the bottom on turn 1 of 250, the top on turn 250.
state = {turn:1, turn_limit:250}; eq(rise(), 0, "turn 1 of 250");
state = {turn:250, turn_limit:250}; eq(rise(), 100, "the last turn");
// A server that sends only max_turns is read the way the turn plate reads it.
state = {turn:126, max_turns:251}; eq(rise(), 50, "halfway");
// It rises on every turn of the game and never falls.
let last = -1;
for (let turn = 1; turn <= 250; turn++) {
  state = {turn, turn_limit:250};
  const now = rise();
  if (!(now > last)) throw new Error(`turn ${turn} stands at ${now}, no higher than ${last}`);
  last = now;
}
// A score game is settled on the wrap out of its final turn: the bar reads the
// turn it was won on, and nothing takes it past the top or under the bottom.
state = {turn:251, victory_turn:250, turn_limit:250}; eq(rise(), 100, "a finished game");
state = {turn:300, turn_limit:250}; eq(rise(), 100, "past the cap");
state = {turn:0, turn_limit:250}; eq(rise(), 0, "before the first turn");
// A game won early stops where it was won.
state = {turn:181, victory_turn:180, turn_limit:250}; eq(rise(), 71.89, "won on turn 180");
// No cap, nothing to rise toward: no bar at all.
state = {turn:40, turn_limit:null, max_turns:250}; eq(turnGauge(), "", "an uncapped world");
state = {turn:40, turn_limit:100000}; eq(turnGauge(), "", "a cap nobody reaches");
state = {turn:1, turn_limit:1}; eq(turnGauge(), "", "a one-turn world");
// It says which turn it stands on, to a pointer and to a screen reader.
state = {turn:37, turn_limit:250};
for (const part of ['title="Turn 37 of 250"', 'role="progressbar"', 'aria-valuemin="1"',
                    'aria-valuemax="250"', 'aria-valuenow="37"', 'data-key="turn-gauge"'])
  if (!turnGauge().includes(part)) throw new Error(`the bar does not carry ${part}: ${turnGauge()}`);

// The scale. Both copies, the dim one for the track and the dark one for the
// gold, carry the same figures at the same heights.
const scales = () => {
  const found = [...turnGauge().matchAll(/<span class="turn-gauge-scale( lit)?" aria-hidden="true">(.*?)<\/span>/g)];
  eq(found.map(copy => copy[1] || ""), ["", " lit"], "one dim copy, then one lit");
  eq(found[0][2], found[1][2], "the two copies agree");
  return [...found[0][2].matchAll(/<b class="([a-z ]+)"(?: style="bottom:([\d.]+)%")?>([^<]+)<\/b>/g)]
    .map(([, kind, at, text]) => ({kind, at:at === undefined ? null : +at, text}));
};
const figures = cap => { state = {turn:1, turn_limit:cap}; return scales().map(mark => mark.text); };
// The cap comes first, under the top of the bar and at no height of its own.
state = {turn:1, turn_limit:250};
eq(scales()[0], {kind:"cap", at:null, text:"250"}, "the cap");
eq(figures(250), ["250", "50", "100", "150", "200"], "a 250-turn game");
eq(scales().slice(1).map(mark => mark.kind), ["minor", "major", "minor", "major"], "every other figure is a major one");
// A figure stands exactly where the bar stands on that turn.
for (const mark of scales().slice(1)) {
  state = {turn:+mark.text, turn_limit:250};
  eq(rise(), mark.at, `the bar on turn ${mark.text} against its line`);
}
eq(figures(500), ["500", "100", "200", "300", "400"], "Standard speed");
eq(figures(180), ["180", "50", "100", "150"], "a 180-turn score game");
// 300 of 330 would sit under the cap's own figure.
eq(figures(330), ["330", "100", "200"], "Quick speed");
eq(figures(60), ["60", "10", "20", "30", "40", "50"], "a short Tactics battle");
eq(figures(2), ["2"], "two turns have no round turn between them");
// Four figures do not fit the bar at full size: thousands are written short,
// and a long figure is marked to be set smaller.
eq(figures(1500), ["1.5k", "250", "500", "750", "1k", "1.25k"], "Marathon");
state = {turn:1, turn_limit:1500};
eq(scales().map(mark => mark.kind.includes("long")), [true, false, false, false, false, true], "long figures");
// Never more figures than the bar has room for, at any cap.
for (let cap = 2; cap <= 3000; cap++) {
  const marks = turnGaugeMarks(cap);
  if (marks.length > 6) throw new Error(`cap ${cap} draws ${marks.length} figures`);
  for (const mark of marks)
    if (!(mark.at >= .08 && mark.at <= .9)) throw new Error(`cap ${cap} puts ${mark.turn} at ${mark.at}`);
}
console.log("turn gauge checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class AGoldBarRisesUpTheVictoryTrackerWithTheTurn(unittest.TestCase):
    """Operator, 2026-11-23: "on the left side of the victory tracker, add a
    rising gold bar. starts at the bottom when turn is 1 of 250 and rises
    throughout the game. tasteful. not too big." Then, of the three-pixel bar
    that made: "wider please. like 6mm wide-ish, with a bit for labels and
    such". """

    def test_the_bar_stands_where_the_turn_is(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        clock = source[source.index("function reportedTurn("):
                       source.index("function victoryMeterFill(")]
        self.assertIn("function turnGauge()", clock)
        self.assertIn("function turnGaugeMarks(cap)", clock)
        done = subprocess.run([NODE, "-e", TURN_GAUGE_SCENARIOS.replace("__CLOCK__", clock)],
                              capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("turn gauge checks passed", done.stdout)

    def test_it_is_the_trackers_own_left_column(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        overview = source[source.index("function playerHudOverview()"):]
        overview = overview[:overview.index("\n}\n")]
        # Drawn once, inside the tracker's box and outside the rail that
        # scrolls, so the paths cannot carry it away.
        self.assertEqual(source.count("turnGauge()"), 2)
        region = overview.index('<section class="hud-region victory-region"')
        gauge = overview.index("turnGauge() +")
        rail = overview.index('<div class="hud-rail"')
        self.assertLess(region, gauge)
        self.assertLess(gauge, rail)
        page = INDEX.read_text(encoding="utf-8")
        style = page[page.index("  #victoryhud > .victory-region {\n    display: grid;"):]
        style = style[:style.index("  @media (prefers-reduced-motion: reduce) {\n    .turn-gauge-fill")]
        # The bar's column is as wide as the bar and no wider without one; the
        # heading and the "Off" line span both columns, the bar and the rail
        # share the row under them.
        self.assertIn("grid-template-columns: auto minmax(0, 1fr);", style)
        self.assertIn("#victoryhud > .victory-region > .hud-region-bar { grid-area: 1 / 1 / 2 / -1; }", style)
        self.assertIn("#victoryhud > .victory-region > .victory-off-note { grid-area: 2 / 1 / 3 / -1; }", style)
        self.assertIn("#victoryhud > .victory-region > .hud-rail { grid-area: 3 / 2; }", style)
        gauge_rule = style[style.index("  .turn-gauge {"):]
        gauge_rule = gauge_rule[:gauge_rule.index("}")]
        self.assertIn("grid-area: 3 / 1;", gauge_rule)
        # A folded tracker folds the bar away with its paths.
        self.assertIn("#victoryhud.victory-collapsed .turn-gauge { display: none; }", style)

    def test_six_millimetres_of_bar_come_off_the_map(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        page = INDEX.read_text(encoding="utf-8")
        gauge_rule = page[page.index("  .turn-gauge {"):]
        gauge_rule = gauge_rule[:gauge_rule.index("}")]
        # 30px is six millimetres on the operator's 16-inch screen, which
        # draws 1728 points across 345 mm.
        width = re.search(r"\bwidth: (\d+)px;", gauge_rule)
        gap = re.search(r"margin: 2px (\d+)px 2px 0;", gauge_rule)
        self.assertEqual(width.group(1), "30")
        self.assertAlmostEqual(int(width.group(1)) / (1728 / 345), 6, delta=.25)
        # The panel is wider by exactly the bar and its gap, so the paths keep
        # the width they had before there was a bar.
        column = re.search(r"\nconst TURN_GAUGE_COLUMN = (\d+);", source)
        self.assertEqual(int(column.group(1)), int(width.group(1)) + int(gap.group(1)))
        self.assertIn("const victoryWidth = Math.round(clampPanelDimension(width * .15, 156, 340)) + "
                      "TURN_GAUGE_COLUMN;", source)
        self.assertLess(source.index("const TURN_GAUGE_COLUMN"),
                        source.index("function syncResponsivePanelGeometry() {"))

    def test_the_gold_is_cut_at_the_turn_and_the_scale_reads_on_both_sides(self):
        page = INDEX.read_text(encoding="utf-8")

        def rule(selector):
            body = page[page.index(f"  {selector} {{"):]
            return body[:body.index("}")]
        # Gold, filled from the bottom up to the turn's height.
        fill = rule(".turn-gauge-fill")
        self.assertIn("var(--gold-bright)", fill)
        self.assertIn("linear-gradient(to top,", fill)
        self.assertIn("clip-path: inset(calc(100% - var(--rise, 0%)) 0 0 0);", fill)
        # The head is what there is of the bar on turn 1, and stays inside it
        # on the last turn.
        head = rule(".turn-gauge > i")
        self.assertIn("height: 2px;", head)
        self.assertIn("bottom: clamp(0px, calc(var(--rise, 0%) - 2px), calc(100% - 2px));", head)
        # The dim copy of the scale is cut off where the gold begins and the
        # dark copy where it ends, so no figure is drawn twice in one place.
        self.assertIn("clip-path: inset(0 0 var(--rise, 0%) 0);", rule(".turn-gauge-scale"))
        self.assertIn("clip-path: inset(calc(100% - var(--rise, 0%)) 0 0 0);", rule(".turn-gauge-scale.lit"))
        # Both copies come after the head in the markup, so they paint over it.
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        gauge = source[source.index("function turnGauge() {"):]
        gauge = gauge[:gauge.index("\n}\n")]
        self.assertLess(gauge.index('<span class="turn-gauge-fill"></span><i></i>'),
                        gauge.index('<span class="turn-gauge-scale" aria-hidden="true">'))
        self.assertLess(gauge.index('<span class="turn-gauge-scale" aria-hidden="true">'),
                        gauge.index('<span class="turn-gauge-scale lit" aria-hidden="true">'))
        # A figure hangs under its line, which is the element's top edge
        # brought down to the height it was placed at.
        figure = rule(".turn-gauge-scale > b")
        self.assertIn("transform: translateY(100%);", figure)
        self.assertIn("border-top: 1px solid", figure)
        # A short bar keeps every other figure, a narrow tracker keeps a slim
        # bar with no scale.
        short = page[page.index("@container turn-gauge (max-height: 150px) {"):]
        self.assertIn(".turn-gauge-scale > b.minor { font-size: 0; }", short[:short.index("\n  }\n")])
        narrow = page[page.index("@container victory-region (max-width: 170px) {"):]
        narrow = narrow[:narrow.index("\n  }\n")]
        self.assertIn(".turn-gauge { width: 6px;", narrow)
        self.assertIn(".turn-gauge-scale { display: none; }", narrow)
        self.assertIn("container: victory-region / inline-size;", page)
        self.assertIn("container: turn-gauge / size;", rule(".turn-gauge"))


TURN_GAUGE_NEW_GAME_SCENARIOS = r"""
let state = {};
__CLOCK__
let bar = null;
const did = [];
const document = {querySelector: selector => { did.push(["find", selector]); return bar; }};
const standing = () => ({
  classList: {add: name => did.push(["hold", name]), remove: name => did.push(["release", name])},
  style: {setProperty: (name, value) => did.push(["put", name, value])},
  get offsetHeight() { did.push(["read"]); return 0; },
});
__SETTLE__
const eq = (actual, wanted, note) => {
  if (JSON.stringify(actual) !== JSON.stringify(wanted))
    throw new Error(`${note}: got ${JSON.stringify(actual)}, wanted ${JSON.stringify(wanted)}`);
};
const settled = world => { state = world; did.length = 0; settleTurnGauge(); return did.slice(); };
const found = ["find", "#victoryhud .turn-gauge"];
const at = rise => [found, ["hold", "turn-gauge-settling"], ["put", "--rise", rise], ["read"],
                    ["release", "turn-gauge-settling"]];
// The bar of the game that just ended is still in the document. It is put at
// the new game's turn while its motion is held off: the hold goes on before
// the turn is written, the layout is read while it is on, and only then does
// it come off.
bar = standing();
eq(settled({turn:1, turn_limit:250, seed:2}), at("0%"), "turn 1 of 250");
// A new game first seen a few turns in stands on that turn, not at the foot.
eq(settled({turn:3, turn_limit:250, seed:3}), at("0.8%"), "first seen on turn 3");
eq(settled({turn:126, max_turns:251, seed:4}), at("50%"), "a game wound back to its middle");
// The height is the one the redraw is about to write, to the figure.
state = {turn:181, victory_turn:180, turn_limit:250};
eq(settled(state)[2][2], /--rise:([\d.]+%)/.exec(turnGauge())[1], "the redraw's own height");
// A new game with no cap draws no bar: nothing is put anywhere, and the
// redraw takes the old bar away.
eq(settled({turn:1, turn_limit:null, seed:5}), [found], "an uncapped new game");
// A page's first world has no bar in the document yet.
bar = null;
eq(settled({turn:1, turn_limit:250, seed:6}), [found], "the first world");
console.log("turn gauge new game checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class ANewGamesGoldBarStartsAtItsFirstTurn(unittest.TestCase):
    """Operator, 2026-11-24: "on civvis have gold bar immediately start at
    1/250 rather than a motion down", and then "between games that is". The
    bar glides to each new turn, so a new game began with it gliding down
    from where the last game ended."""

    def setUp(self):
        self.source = (ASSETS / "app.js").read_text(encoding="utf-8")
        self.clock = (ASSETS / "app_clock.js").read_text(encoding="utf-8")
        self.page = INDEX.read_text(encoding="utf-8")
        settle = self.clock[self.clock.index("function settleTurnGauge() {"):]
        self.settle = settle[:settle.index("\n}\n") + 3]

    def test_the_bar_is_put_at_the_new_games_turn_with_its_motion_held_off(self):
        gauge = self.source[self.source.index("function reportedTurn("):
                            self.source.index("function victoryMeterFill(")]
        script = (TURN_GAUGE_NEW_GAME_SCENARIOS.replace("__CLOCK__", gauge)
                  .replace("__SETTLE__", self.settle))
        done = subprocess.run([NODE, "-e", script], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("turn gauge new game checks passed", done.stdout)

    def test_it_happens_when_the_world_on_screen_is_a_new_one_and_before_the_redraw(self):
        # A new world is the page's own reading of one, another game or the
        # same game wound back, and the stopwatch is told of it once the
        # world is on screen and before anything is redrawn for it. The bar
        # is settled there, in the script that is loaded ahead of the
        # renderer: app.js is at its ceiling and gains no line by this.
        self.assertIn("  const newWorld = !state || st.seed !== state.seed || st.turn < state.turn;", self.source)
        arrival = self.source[self.source.index("  const newWorld = !state ||"):]
        self.assertLess(arrival.index("\n  state = st;\n"), arrival.index("  syncLiveClock(st, newWorld);"))
        self.assertNotIn("drawPlayerHud(", arrival[:arrival.index("  syncLiveClock(st, newWorld);")])
        self.assertNotIn("settleTurnGauge", self.source)
        sync = self.clock[self.clock.index("function syncLiveClock(st, newWorld) {"):]
        sync = sync[:sync.index("\n}\n")]
        new_world = sync[sync.index("  if (newWorld) {"):sync.index("\n  }\n")]
        self.assertIn("    settleTurnGauge();", new_world)
        self.assertEqual(self.clock.count("settleTurnGauge();"), 1)
        self.assertIn("`turnGauge`", self.clock[:self.clock.index("let liveClock = {")])
        # It reads the height off the bar the redraw is about to draw, so
        # the two cannot stand apart.
        self.assertIn("/--rise:([\\d.]+%)/.exec(turnGauge())?.[1]", self.settle)
        self.assertIn('document.querySelector("#victoryhud .turn-gauge")', self.settle)

    def test_the_hold_stops_every_part_of_the_bar_that_glides(self):
        page = self.page
        gauge = page[page.index("  .turn-gauge {"):page.index("  .overlay-options {")]
        # Every part of the bar that glides to a new turn...
        gliding = sorted(selector.strip() for selector, body in re.findall(r"\n  ([^{}\n]+) \{([^}]*)\}", gauge)
                         if "transition:" in body and "none" not in body)
        self.assertEqual(gliding, [".turn-gauge > i", ".turn-gauge-fill", ".turn-gauge-scale"])
        # ...is held by the one rule, which stands after them all.
        hold = ("  .turn-gauge.turn-gauge-settling > .turn-gauge-fill,\n"
                "  .turn-gauge.turn-gauge-settling > i,\n"
                "  .turn-gauge.turn-gauge-settling > .turn-gauge-scale { transition: none; }\n")
        self.assertIn(hold, gauge)
        for selector in gliding:
            self.assertLess(gauge.index(f"  {selector} {{"), gauge.index(hold))
        # The parts are the bar's own children, as the hold's selectors say.
        drawn = self.source[self.source.index("function turnGauge() {"):]
        drawn = drawn[:drawn.index("\n}\n")]
        self.assertIn('<span class="turn-gauge-fill"></span><i></i>', drawn)
        self.assertIn("turn-gauge-settling", self.settle)
        # The markup never names the hold, so a redraw cannot leave it on.
        self.assertNotIn("turn-gauge-settling", self.source)


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class AHundredSeatMastheadStaysOnItsShare(unittest.TestCase):
    """Operator, 2026-11-09: the player HUD should cap "such that it isn't taking
    over the whole screen". A hundred rows also rebuilt 790 KB of markup a turn,
    so only the rows near what is showing are built."""

    def test_the_window(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        consts = source[source.index("const PLAYER_HUD_ROW_GAP = 2;"):
                        source.index("function playerHudRowPitch()")]
        heights = source[source.index("function playerHudSeatHeight"):
                         source.index("const HUD_WIDGETS = {")]
        window = source[source.index("function planPlayerHudWindow"):
                        source.index("function refillPlayerHudWindow")]
        chrome = source[source.index("const PLAYER_HUD_CHROME_HEIGHT ="):
                        source.index("const PLAYER_HUD_ROW_GAP = 2;")]
        script = HUD_WINDOW_SCENARIOS.replace("__CONSTS__", chrome + consts).replace(
            "__BLOCK__", heights + window)
        done = subprocess.run(
            [NODE, "-e", script], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("hud window checks passed", done.stdout)


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class PublishedViewingDefaults(unittest.TestCase):
    def test_fresh_visitors_spin_with_two_turn_trails_and_saved_choices_win(self):
        source = (REPO / "beta/shim.js").read_text(encoding="utf-8")
        start = source.index("  function applyPublishedViewDefaults(storage)")
        block = source[start:source.index("\n  if (!LOCAL_DESKTOP_HOST)", start)]
        scenarios = r'''
const values = new Map();
const storage = {getItem:k => values.has(k) ? values.get(k) : null,
                 setItem:(k,v) => values.set(k,v)};
applyPublishedViewDefaults(storage);
if (values.get("civvis-world-spin-2") !== "1" || values.get("civvis-unit-tail-turns") !== "2")
  throw Error("fresh public visitor does not get spin and two-turn trails");
values.set("civvis-world-spin-2", "0"); values.set("civvis-unit-tail-turns", "0");
applyPublishedViewDefaults(storage);
if (values.get("civvis-world-spin-2") !== "0" || values.get("civvis-unit-tail-turns") !== "0")
  throw Error("saved off preferences were overwritten");
// The first spin key was written by every drag of the map, so an "off" left
// there is not a choice and must not hold a returning visitor's world still.
values.clear(); values.set("civvis-world-spin", "0");
applyPublishedViewDefaults(storage);
if (values.get("civvis-world-spin-2") !== "1")
  throw Error("an old drag still holds a returning visitor's world still");
values.set("civvis-unit-tail-turns", "4"); applyPublishedViewDefaults(storage);
if (values.get("civvis-unit-tail-turns") !== "4") throw Error("saved trail length changed");
'''
        done = subprocess.run([NODE, "-e", block + scenarios], capture_output=True, text=True, timeout=15)
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_turning_can_use_each_refresh_and_a_higher_canvas_budget(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        start = source.index("const MAX_ANIMATION_PAINT_SHARE =")
        block = source[start:source.index("function animTick(now)", start)]
        scenarios = r'''
let drawCost = 2;
function animationRefreshInterval() { return 1000 / 60; }
function paints(spinning) {
  let previous = 0, count = 0;
  for (let frame = 1; frame <= 120; frame++) {
    const now = frame * (1000 / 60) + (frame % 2 ? -.02 : .02);
    if (now - previous > animationPaintInterval(now, spinning)) { previous = now; count++; }
  }
  return count;
}
if (paints(true) !== 120 || paints(false) >= paints(true)) throw Error("turning skips browser refreshes");
drawCost = 20;
if (paints(true) <= paints(false)) throw Error("busy Canvas rotation did not get smoother");
if (animationPaintInterval(0, false) !== 40) throw Error("normal paint budget changed");
if (animationPaintInterval(0, true, true) !== drawCost / .6) throw Error("GPU overlay stole terrain frame budget");
'''
        done = subprocess.run([NODE, "-e", block + scenarios], capture_output=True, text=True, timeout=15)
        self.assertEqual(done.returncode, 0, done.stderr)


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class SimulationWorldRotation(unittest.TestCase):
    def test_player_games_stay_still_and_simulations_keep_the_saved_preference(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        start = source.index("function spinAvailable()")
        block = source[start:source.index("function advanceWorldSpin(dt)", start)]
        scenarios = r'''
const WORLD_SPIN_STORAGE_KEY = "spin", saved = new Map([["spin", "1"]]);
const localStorage = {getItem:k => saved.has(k) ? saved.get(k) : null, setItem:(k,v) => saved.set(k, v)};
let state = {}, SPEC = false, WORLD_SPIN = true;
let dragState = null, touchGesture = null, cameraFlight = null, cameraZoom = null;
let cameraFollowManual = false, wraps = true, worldSpinPeriodMs = 36000;
const REDUCED_MOTION_QUERY = {matches:false};
const spinSpeedBar = null;
function mapWrapsX() { return wraps; }
const attributes = {};
const spinButton = {title:"", setAttribute:(k,v) => attributes[k] = v, style:{setProperty() {}}};
function expect(condition, message) { if (!condition) throw Error(message); }
updateSpinButton();
expect(!spinAvailable() && !spinRunning(), "player game rotates with saved spin enabled");
expect(spinButton.disabled && attributes["aria-pressed"] === "false", "player control advertises rotation");
expect(spinButton.title.includes("AI simulations"), "player control does not explain availability");
SPEC = true; updateSpinButton();
expect(spinRunning() && !spinButton.disabled, "simulation lost its saved rotation preference");
expect(attributes["aria-pressed"] === "true" && WORLD_SPIN, "mode switch changed spin preference");
SPEC = false; expect(!spinRunning(), "switching into player game leaves rotation running");
SPEC = true; REDUCED_MOTION_QUERY.matches = true;
expect(!spinRunning(), "simulation ignores reduced motion");
REDUCED_MOTION_QUERY.matches = false; wraps = false;
expect(!spinRunning(), "unwrapped world rotates");
wraps = true; WORLD_SPIN = false;
expect(!spinRunning(), "simulation ignores saved spin off");
WORLD_SPIN = true; state = {view_player:1}; updateSpinButton();
expect(!spinAvailable() && !spinRunning(), "the world keeps turning under a watched empire");
expect(spinButton.disabled && spinButton.title.includes("watch one civilization"), "Watch as does not explain the hold");
stopWorldSpin();
expect(WORLD_SPIN, "looking around a watched empire cleared the saved rotation");
state = {view_player:null}; updateSpinButton();
expect(spinRunning() && !spinButton.disabled, "going back to All did not pick the turn up again");
stopWorldSpin();
expect(!WORLD_SPIN, "a hand on the turning world no longer stops it");
expect(!spinRunning() && attributes["aria-pressed"] === "false", "the button still reads as turning under a hand");
expect(saved.get("spin") === "1", "a hand on the world was saved as the viewer's choice");
restoreWorldSpin();
expect(WORLD_SPIN && spinRunning() && attributes["aria-pressed"] === "true", "the next world did not open turning again");
stopWorldSpin(); setWorldSpin(true);
expect(WORLD_SPIN && spinRunning(), "the button does not bring the turn back after a hand");
setWorldSpin(false); restoreWorldSpin();
expect(!WORLD_SPIN && saved.get("spin") === "0", "the next world overrode the button's off");
'''
        done = subprocess.run([NODE, "-e", block + scenarios], capture_output=True, text=True, timeout=15)
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_the_speed_bar_runs_from_a_halt_to_one_turn_a_second(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        start = source.index("const WORLD_SPIN_SPEED_STORAGE_KEY")
        speeds = source[start:source.index("\n", source.index("let worldSpinPeriodMs", start))]
        start = source.index("function spinAvailable()")
        block = source[start:source.index("function advanceWorldSpin(dt)", start)]
        start = source.index("if (spinSpeedBar) {\n  spinSpeedBar.addEventListener")
        wiring = source[start:source.index("\n}\n", start) + 3]
        stubs = r'''
const WORLD_SPIN_PERIOD_MS = 36000, WORLD_SPIN_STORAGE_KEY = "spin";
const saved = new Map();
const localStorage = {getItem:k => saved.has(k) ? saved.get(k) : null, setItem:(k,v) => saved.set(k, v)};
let state = null, SPEC = true, WORLD_SPIN = true, wraps = true;
let dragState = null, touchGesture = null, cameraFlight = null, cameraZoom = null, cameraFollowManual = false;
const REDUCED_MOTION_QUERY = {matches:false};
function mapWrapsX() { return wraps; }
const window = {addEventListener() {}};
const heard = {};
const spinSpeedBar = {value:"0", attributes:{}, setAttribute(k, v) { this.attributes[k] = v; },
  addEventListener(name, listener) { heard[name] = listener; }};
const spinSpeedPanel = {hidden:false, classList:{add() {}, remove() {}}};
const spinButton = {title:"", setAttribute() {}, style:{setProperty() {}}};
'''
        scenarios = r'''
function expect(condition, message) { if (!condition) throw Error(message); }
function drag(...notches) { for (const n of notches) { spinSpeedBar.value = String(n); heard.input(); } heard.change(); }
updateSpinButton();
expect(worldSpinPeriodAt(WORLD_SPIN_DEFAULT_SPEED) === 36000, "the default notch is not today's pace");
expect(Math.abs(worldSpinPeriodAt(WORLD_SPIN_TOP_SPEED) - 1000) < 1e-6, "the rabbit is not one turn a second");
expect(worldSpinPeriodAt(1) > 36000, "the turtle is not slower than the default");
expect(spinSpeedBar.value === "8", "the bar does not open at the default pace");
drag(12, 20);
expect(WORLD_SPIN && Math.abs(worldSpinPeriodMs - 1000) < 1e-6, "the top of the bar is not the top speed");
expect(saved.get(WORLD_SPIN_SPEED_STORAGE_KEY) === "20", "the chosen speed is not remembered");
expect(spinSpeedBar.attributes["aria-valuetext"] === "One full turn a second", "the bar does not read out its speed");
drag(10, 4, 1, 0);
expect(!WORLD_SPIN && spinSpeedBar.value === "0", "the very bottom does not halt the world");
expect(worldSpinSpeed === 20 && saved.get(WORLD_SPIN_SPEED_STORAGE_KEY) === "20", "the way down to halt overwrote the speed");
expect(spinSpeedBar.attributes["aria-valuetext"] === "Halted", "the bar does not read out a halt");
setWorldSpin(true);
expect(spinSpeedBar.value === "20" && worldSpinPeriodMs === worldSpinPeriodAt(20), "the button does not bring the speed back");
stopWorldSpin();
expect(spinSpeedBar.value === "0", "a hand on the world leaves the bar reading a speed");
drag(3);
expect(WORLD_SPIN && worldSpinSpeed === 3, "raising the bar from a halt does not turn the world");
for (const value of ["0", "21", "1.5", "fast"]) {
  saved.set(WORLD_SPIN_SPEED_STORAGE_KEY, value);
  expect(savedWorldSpinSpeed() === WORLD_SPIN_DEFAULT_SPEED, "a bad saved speed was believed: " + value);
}
state = {view_player:1}; updateSpinButton();
expect(spinSpeedPanel.hidden, "the bar offers a speed while the world is held for Watch as");
'''
        done = subprocess.run([NODE, "-e", stubs + speeds + block + wiring + scenarios],
                              capture_output=True, text=True, timeout=15)
        self.assertEqual(done.returncode, 0, done.stderr)


class PlanetEmpireNames(unittest.TestCase):
    """A civ's name is a spot on the ground that the camera only projects."""

    def source(self):
        return (ASSETS / "app.js").read_text(encoding="utf-8")

    def test_names_keep_the_grounds_time_and_are_not_lettered_per_frame(self):
        source = self.source()
        start = source.index("function drawPlanetGpuAnimationFrame()")
        gpu_frame = source[start:source.index("\n}\n", start)]
        self.assertIn("paintPlanetEmpireLabelLayer(camera, radius, centerX, centerY);", gpu_frame)
        start = source.index("function drawPlanetEmpireLabels(")
        painter = source[start:source.index("\n}\n", start)]
        # fillText snaps its baseline to whole device pixels.
        self.assertNotIn("fillText", painter)
        self.assertIn("planetProject(label.at, camera, scale, centerX, centerY)", painter)
        start = source.index("function animTick(now)")
        self.assertIn("planetEmpireLabelsSettling()", source[start:])

    def test_names_are_the_top_of_the_map_under_only_what_flies(self):
        source = self.source()
        start = source.index("function drawPlanetMap()")
        frame = source[start:source.index("\n}\n", start)]
        for names in (frame.index("drawPlanetEmpireLabels(cx,"),
                      frame.index("paintPlanetEmpireLabelLayer(")):
            for below in ("drawCityIcon(cx", "drawCityNameplate(", "drawStrategicUnitCounter(",
                          "drawPlanetBlasts("):
                self.assertLess(frame.index(below), names, below)
            self.assertLess(names, frame.index("drawPlanetSky("))
        # The GPU path's own layer has to clear the map canvas, take no input
        # away from it, and go when the names move back onto the canvas.
        start = source.index("function paintPlanetEmpireLabelLayer(")
        layer = source[start:source.index("\n}\n", start)]
        self.assertIn("cv.after(canvas)", layer)
        self.assertIn('canvas.style.zIndex = "1"', layer)
        self.assertIn("gpu.canvas.style.cssText", layer)
        self.assertIn('"pointer-events:none"', source)
        self.assertIn('PLANET_GPU.labels.canvas.style.display = "none"', frame)
        # That layer is above the rockets' canvas, so it is only safe while
        # nothing is flying.
        start = source.index("function planetGpuEligible(")
        eligible = source[start:source.index("\n}\n", start)]
        self.assertIn("activeSkyLaunches().length", eligible)

    @unittest.skipUnless(NODE, "no node on this host; CI runners have one")
    def test_a_launch_leaving_the_frame_leaves_its_name_waiting_at_the_edge(self):
        source = self.source()
        def between(first, last):
            start = source.index(first)
            return source[start:source.index(last, start)]
        code = (between("function activeSkyLaunches(", "// What a launch is called on screen")
                + between("// What a launch is called on screen", "// When a body stops being a marble")
                + between("const SKY_NOSE_RATE", "// A body nobody could name is just a light."))
        scenarios = r"""
let T = 0, painted = [], panels = [];
const performance = {now:() => T};
const anim = {skyLaunches:[]};
const nothing = () => {};
const cx = {globalAlpha:1, save:nothing, restore:nothing, strokeText:nothing, beginPath:nothing,
  moveTo:nothing, lineTo:nothing, stroke:nothing, arc:nothing, rect:nothing, clip:nothing,
  fill:nothing, translate:nothing, rotate:nothing, scale:nothing,
  measureText:text => ({width:text.length * 6}),
  fillText(text, x, y) { painted.push({text, x, y, alpha:this.globalAlpha}); }};
function civName() { return "Gran Colombia"; }
function skyWidgetBoxes() { return panels; }
function pcol() { return "#fff"; }
function skySmooth(value) { const t = Math.max(0, Math.min(1, value)); return t * t * (3 - 2 * t); }
function skyRocketGeometry() { return {length:12, halfWidth:2}; }
function drawMissionRocket() {}
function drawLaunchSmoke() {}
function expect(condition, message) { if (!condition) throw Error(message); }
const near = (a, b) => Math.abs(a - b) < .01;
const STAGE = {left:0, top:0, right:800, bottom:500};
function film(end, leaving = true) {
  const start = {x:400, y:300};
  return {start, end, entering:false, leaving, law:clock => clock,
          pointAt:p => ({x:start.x + (end.x - start.x) * p, y:start.y + (end.y - start.y) * p})};
}
function flight(mission = "moon") {
  const launch = {playerId:1, mission, t0:0, duration:1000, endsAt:1000};
  anim.skyLaunches.push(launch);
  return launch;
}
function paint(launch, route, now, stage = STAGE, pixel = 1) {
  T = now; painted = [];
  if (activeSkyLaunches(now).includes(launch))
    drawLaunchFlight(launch, {id:1}, {rocket:"saturn_v"}, route, 4, now, pixel, stage);
  return painted[painted.length - 1];
}

expect(skyLaunchLabel({playerId:1, mission:"moon"}) === "GRAN COLOMBIA – MOON", "no dash in the label");
expect(skyLaunchLabel({playerId:1, mission:"expedition"}) === "GRAN COLOMBIA – EXOPLANET", "expedition label");

// Out through the bottom: the name stops at the edge, inside the frame, readable.
const down = flight(), exit = film({x:300, y:560});
const flying = paint(down, exit, 950);
expect(near(flying.alpha, .88), "the name faded with the rocket on the way out: " + flying.alpha);
const first = paint(down, exit, 1050);
expect(first && first.y <= 497 && near(first.alpha, .88), "name is not readable at the edge");
expect(!skyLaunchInFlight(1, "moon", 1050), "a waiting name holds up the craft it carried");
expect(activeSkyLaunches(1050).includes(down), "launch ended with its name");
// A second or so, and it does not move, whatever the camera does to the route.
const later = paint(down, film({x:600, y:560}), 2000);
expect(later.x === first.x && later.y === first.y, "a waiting name slid along the edge");
expect(near(later.alpha, .88), "name faded before its second was up: " + later.alpha);
const going = paint(down, exit, 2150);
expect(going.alpha < .88 && going.alpha > 0, "name did not fade at the end of its wait");
expect(!paint(down, exit, 2300) && !anim.skyLaunches.length, "launch outstayed its pause");

// Out through the top stops below the masthead's shadow.
const up = flight();
paint(up, film({x:300, y:-60}), 950);
const top = paint(up, film({x:300, y:-60}), 1100);
expect(top.y >= SKY_LABEL_TOP, "name sits in the masthead's shadow");
anim.skyLaunches.length = 0;

// Flights that stay in the picture are over when their film is.
const landing = flight(), landed = film({x:100, y:100}, false);
paint(landing, landed, 900);
expect(!paint(landing, landed, 1000) && !anim.skyLaunches.length, "a landing got a pause");
const hop = flight(), hopped = film({x:400, y:256});
paint(hop, hopped, 900);
expect(!paint(hop, hopped, 1000) && !anim.skyLaunches.length, "a hop inside the frame got a pause");

// A panel over the spot, and a second name arriving there, both move it clear.
panels = [{left:240, right:360, top:440, bottom:540}];
const under = flight(), route = film({x:300, y:560});
paint(under, route, 950);
const clear = paint(under, route, 1100), half = cx.measureText(clear.text).width / 2 + 4;
expect(clear.x + half <= 240.5 || clear.x - half >= 359.5 || clear.y <= 440.5, "name is under a panel");
const twin = flight();
paint(twin, route, 950);
const other = paint(twin, route, 1100);
expect(Math.abs(other.y - clear.y) >= 14 || Math.abs(other.x - clear.x) >= half * 2, "two names overprint");
panels = []; anim.skyLaunches.length = 0;

// A flat board's frame is centred and scaled, and the name stays on it.
const scale = 2, board = {left:-800, right:800, top:-500, bottom:500};
const flat = flight(), rise = {...film({x:0, y:-620}), start:{x:0, y:0}, pointAt:p => ({x:0, y:-620 * p})};
paint(flat, rise, 950, board, scale);
const onBoard = paint(flat, rise, 1100, board, scale);
expect(near(onBoard.y, board.top + SKY_LABEL_TOP * scale), "board name is off its edge: " + onBoard.y);
const zoomed = {left:-400, right:400, top:-250, bottom:250};
const held = paint(flat, rise, 1300, zoomed, 1);
expect(near(held.y, zoomed.top + SKY_LABEL_TOP), "board name did not keep its place on screen");
"""
        done = subprocess.run([NODE, "-e", code + scenarios], capture_output=True, text=True, timeout=15)
        self.assertEqual(done.returncode, 0, done.stderr)

    @unittest.skipUnless(NODE, "no node on this host; CI runners have one")
    def test_a_name_is_fixed_to_the_ground_while_the_world_turns(self):
        source = self.source()
        start = source.index("const PLANET_PROJECT_TRIG")
        project = source[start:source.index("// ------------------------------------------------------- turning it by hand", start)]
        start = source.index("const PLANET_MAJOR_CITY_LABEL_SCALE")
        names = source[start:source.index("// While the GPU turns the ground on every display refresh", start)]
        scenarios = r'''
let T = 1000;
const performance = {now:() => T};
const DPR = 2, SKY_MARKERS = 100;
function key(p) { return p[0] + "," + p[1]; }
function jerseyLanes() { return ["#000", "#fff"]; }
function planetMap() { return true; }
function expect(condition, message) { if (!condition) throw Error(message); }
let drawn = [];
function fakeContext() {
  return {globalAlpha:1, save() {}, restore() {}, setTransform() {}, strokeText() {}, fillText() {},
    measureText:text => ({width:text.length * 8}),
    drawImage(image, x, y, width, height) {
      drawn.push({x:x + width / 2, y:y + height / 2, alpha:this.globalAlpha});
    }};
}
const document = {createElement:() => ({width:0, height:0, getContext:fakeContext})};
const COLUMNS = 40, ROWS = 20, cells = new Map(), blank = [];
for (let i = 0; i < COLUMNS; i++) for (let j = 0; j < ROWS; j++) {
  const longitude = 2 * Math.PI * i / COLUMNS - Math.PI;
  const latitude = Math.PI * ((j + .5) / ROWS - .5);
  cells.set(i + "," + j, {
    center:[Math.cos(latitude) * Math.cos(longitude), Math.cos(latitude) * Math.sin(longitude), Math.sin(latitude)],
    nbrs:[((i + 1) % COLUMNS) + "," + j, ((i + COLUMNS - 1) % COLUMNS) + "," + j, i + "," + (j + 1), i + "," + (j - 1)]});
  blank.push([i, j]);
}
const PLANET = {cells};
const store = PLANET_EMPIRE_LABELS;
let players = [{civ:"Rome"}];
let state;
function world(rule) {
  state = {players, map:{tiles:blank.map(([i, j]) => ({pos:[i, j], owner:rule(i, j)}))}};
}
const cameraAt = longitude => ({longitude, latitude:0, roll:0, radius:400});
const RADIUS = 100, CENTER = 300;
function paint(longitude, visible = new Set(), spectator = true) {
  drawn = [];
  drawPlanetEmpireLabels(fakeContext(), cameraAt(longitude), .25, RADIUS, CENTER, CENTER, visible, spectator);
}
const patch = (i, j) => i >= 18 && i <= 23 && j >= 7 && j <= 12 ? 0 : null;
world(patch);
paint(.2);
expect(store.list.length === 1, "one territory must carry one name");
const label = store.list[0], spot = label.target;
for (let step = 0; step < 50; step++) {
  T += 16;
  const longitude = .2 + step * .02;
  paint(longitude);
  const truth = planetProject(spot, cameraAt(longitude), .25, CENTER, CENTER);
  expect(drawn.length === 1, "a name in full view was not drawn");
  expect(Math.abs(drawn[0].x - truth.x) < 1e-9 && Math.abs(drawn[0].y - truth.y) < 1e-9,
         "a name left its spot on the ground as the world turned");
  expect(label.target === spot, "turning the world moved a name's spot");
}
T += 16; paint(Math.PI);
expect(drawn.length === 0, "a name was drawn through the far side of the world");

world((i, j) => patch(i, j) ?? (i === 24 && j === 9 ? 0 : null));
T += 16; paint(.2);
expect(label.target === spot, "one tile of border growth moved a name");

world((i, j) => i >= 20 && i <= 25 && j >= 7 && j <= 12 ? 0 : null);
T += 16; paint(.2);
expect(label.target !== spot, "a territory that really moved kept its old name spot");
expect(label.at !== label.target && planetSpotNearness(label.at, spot) > planetSpotNearness(label.target, spot),
       "a real move must glide rather than jump");
expect(planetEmpireLabelsSettling(), "the map went idle during a glide");
for (let step = 0; step < 400; step++) { T += 16; paint(.2); }
expect(label.at === label.target, "a glide never arrived");
expect(!planetEmpireLabelsSettling(), "settled names kept the painter awake");

store.list = [];
world(patch);
const seen = new Set(["18,7", "18,8"]);
T += 16; paint(.2, seen, false);
const rim = cells.get("18,7").center;
expect(store.list.length === 1 && planetSpotNearness(store.list[0].target, rim) > Math.cos(.15),
       "a name's spot drew on tiles this viewer cannot see");

store.list = [];
players = [{civ:"Rome", met:false}];
world(patch);
T += 16; paint(.2);
expect(store.list.length === 0 && drawn.length === 0, "an unmet civilization was named");

store.list = [];
players = [{civ:"Rome"}, {civ:"Geneva", is_minor:true}, {civ:"Barbarians", is_minor:true, is_barbarian:true}];
world((i, j) => i >= 20 && i <= 23 && j >= 7 && j <= 10 ? 0
  : i >= 20 && i <= 22 && j >= 14 && j <= 16 ? 1
  : i >= 24 && i <= 26 && j >= 3 && j <= 5 ? 2 : null);
function paintAt(scale) {
  drawn = [];
  drawPlanetEmpireLabels(fakeContext(), cameraAt(.2), scale, RADIUS, CENTER, CENTER, new Set(), true);
}
T += 16; paintAt(.25);
expect(store.list.map(item => item.name).join() === "ROME,GENEVA" && drawn.length === 2,
       "a city-state must be named as a civilization is, and a barbarian must not");
expect(store.sprites.has("9|#fff|ROME") && store.sprites.has("6|#fff|GENEVA"),
       "a city-state's name was not lettered at 60% of a civilization's");
const geneva = store.list.find(item => item.minor);
T += 16; paintAt(.7);
const genevaSpot = planetProject(geneva.at, cameraAt(.2), .7, CENTER, CENTER);
expect(drawn.length === 1 && Math.abs(drawn[0].x - genevaSpot.x) < 1e-6,
       "between the two hand-overs only the city-state keeps its territory's name");
T += 16; paintAt(1);
expect(drawn.length === 0, "a city-state's territory and its city both carried its name");

store.list = [];
world((i, j) => i === 22 && j >= 8 && j <= 11 ? 0
  : i >= 18 && i <= 21 && j >= 8 && j <= 11 ? 1 : null);
T += 16; paintAt(.25);
expect(!store.list[0].minor && store.list[0].shown && !store.list[1].shown,
       "a large city-state's name took the place of a small civilization's");
store.list = [];

players = [{civ:"Rome"}];
world((i, j) => j >= 8 && j <= 11 ? 0 : null);
T += 16; paint(.2);
expect(store.list.length >= 3, "a territory round the whole world had nothing to name its far side");
'''
        done = subprocess.run([NODE, "-e", project + names + scenarios], capture_output=True, text=True, timeout=15)
        self.assertEqual(done.returncode, 0, done.stderr)


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class LaunchesBetweenDots(unittest.TestCase):
    """From the Solar System stop the Earth is nine pixels from the Sun.

    A launch drawn at its close-up size, flying a fixed hop, set off looking as
    though it had left the Sun and ended nowhere. These run the route planner
    itself on that picture: a trip ends where it is going, nothing flies over
    the Sun, and the pictures that had room all along are flown as before.
    """

    def test_a_launch_between_dots_leaves_home_and_ends_where_it_is_going(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")

        def cut(start, end):
            begin = source.index(start)
            return source[begin:source.index(end, begin)]

        block = (cut("function skyFade(", "// ---")
                 + cut("function skyLaunchPoint(", "// How much of the straight line")
                 + cut("const SKY_WORLD_IS_A_DOT =", "\n") + "\n"
                 + cut("const SKY_HULL_SHARE =", "// The camera is parked at the destination"))
        scenarios = r'''
const easeIO = t => t;
const SKY_SUN = {id:"sun"};
let sun = {x:0, y:0, r:3.4}, pad = {x:0, y:0, z:0}, goal = {x:0, y:0};
function skyPlacement() { return sun; }
function planetProject() { return pad; }
function skyLaunchPad() { return null; }
function skyLaunchEnd() { return goal; }
function skyThrustLaw() { return clock => clock; }
const skyTransferLaw = clock => clock;
function expect(condition, message) { if (!condition) throw Error(message); }
const far = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
function fly(mission = "mars") {
  const film = skyTransferRoute({mission, route:{kind:"transfer"}}, {id:0}, {}, {}, {}, 1, 0, 0);
  let nearest = Infinity;
  for (let step = 0; step <= 60; step++) nearest = Math.min(nearest, far(film.pointAt(step / 60), sun));
  return {...film, nearest, length:far(film.start, film.end)};
}

// The Solar System stop: one astronomical unit is about nine pixels.
pad = {x:100, y:100, z:0}; sun = {x:100, y:109.3, r:3.4};

// Mars off to one side, a dozen pixels away: the rocket goes there and stops.
goal = {x:111, y:98};
let film = fly();
expect(far(film.start, pad) < 1e-9, "a launch must start at home");
expect(far(film.end, goal) < 1e-9 && !film.leaving, "a Mars rocket must end at Mars: " + JSON.stringify(film.end));
expect(film.hull <= film.length * .68 + 1e-9 && film.hull >= 4.5, "the hull was not fitted to its trip");
expect(film.nearest >= sun.r + 4 - 1e-6, "a clear trip was not left clear");

// Mars straight across the Sun: round it, never over it, and still to Mars.
goal = {x:100, y:123.4};
film = fly();
expect(far(film.end, goal) < 1e-9, "going round the Sun must not move the destination");
expect(film.nearest >= sun.r + 4 - 1e-6, "a Mars rocket flew across the Sun: " + film.nearest);

// The Moon, on top of the Earth from out here, bearing straight at the Sun:
// a short departure that stays in home's own room and off the Sun.
goal = {x:100, y:102};
film = fly("moon");
expect(film.leaving, "there is no trip to a place that is not a second place");
expect(film.length <= 9.3 && film.length >= 7 - 1e-9, "the departure was not scaled to the picture: " + film.length);
expect(film.nearest >= Math.min(sun.r + 4, 9.3 * .8) - 1e-6, "a departure flew at the Sun: " + film.nearest);
expect(film.hull < 6, "a close-up hull in a picture of the system: " + film.hull);

// The expedition departs towards where it is going; its lane begins close by.
goal = {x:94, y:100};
film = fly("expedition");
expect(film.leaving && film.end.x < pad.x - 6, "the expedition must depart along its lane");

// The whole system a speck, as at the Voyage stop: nothing to be in scale
// with, so the departure is the one it always was.
sun = {x:100.0004, y:100.0004, r:3.4}; goal = {x:100, y:100};
film = fly("moon");
expect(Math.abs(film.length - 44) < 1e-9 && film.hull >= 20.4, "the speck's departure changed: " + film.length);
expect(Math.abs(film.end.x - pad.x) < 1e-9 && film.end.y < pad.y, "with no bearing it goes up");

// Home a marble with the Sun far off the stage: also as it always was, and a
// world that is its own place on the stage gets the whole trip at full size.
sun = {x:9000, y:-4000, r:60}; goal = {x:101.5, y:100};
film = fly("moon");
expect(Math.abs(film.length - 44) < 1e-9 && Math.abs(film.end.y - pad.y) < 1e-9, "a clear departure was turned");
goal = {x:160, y:140};
film = fly("moon");
expect(far(film.end, goal) < 1e-9 && film.hull >= 20.4 && !film.leaving, "a trip in the picture changed");
'''
        done = subprocess.run([NODE, "-e", block + scenarios], capture_output=True, text=True, timeout=15)
        self.assertEqual(done.returncode, 0, done.stderr)

    def test_home_is_a_dot_among_the_dots_and_its_name_keeps_off_the_sun(self):
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        start = source.index("function drawPlanetSky(")
        sky = source[start:source.index("\n}\n", start)]
        # The marble the other planets get, by the rule the other planets get it.
        self.assertIn("const marble = {...SKY_EARTH, x:centerX, y:centerY, r:SKY_EARTH.floor};", sky)
        self.assertIn("skyFade(SKY_EARTH.separation * radius, 2.5, 7)", sky)
        # Its name goes to the side with less under it, and the Sun counts most.
        self.assertIn("const above = covers(true) < covers(false);", sky)
        self.assertIn('cost += place.id === "sun" ? 3 : 1;', sky)
        # A launch's own name is kept off home's name and off the Sun.
        self.assertIn("drawSkyLaunches(crews, placements, camera, radius, centerX, centerY, now, crowd);", sky)


LIVE_CLOCK_SCENARIOS = r"""
let now = 1_000_000;
Date.now = () => now;
const timers = [];
const setInterval = (fn, ms) => { timers.push([fn.name, ms]); };
// The value on the plate: its words and the attribute the stylesheet prints.
const writes = {words: 0, hundredths: 0, lookups: 0};
const value = () => {
  const dataset = {};
  let hundredths = "00", words = "0";
  Object.defineProperty(dataset, "hundredths", {
    get: () => hundredths, set: next => { hundredths = next; writes.hundredths++; }});
  return {isConnected: true, dataset,
    get textContent() { return words; },
    set textContent(next) { words = next; writes.words++; }};
};
let plate = value();
const document = {querySelector: selector => {
  if (selector !== "[data-live-time]") throw new Error(`looked for ${selector}`);
  writes.lookups++;
  return plate;
}};
__BLOCK__
const expect = (ok, note) => { if (!ok) throw new Error(note); };
const reads = (ms, text, hundredths, spoken = text) => {
  liveClock.ms = ms; liveClock.runningSince = null;
  const reading = liveClockReading();
  expect(reading.text === text && reading.hundredths === hundredths,
    `${ms} ms read ${reading.text}.${reading.hundredths}, wanted ${text}.${hundredths}`);
  expect(reading.spoken === spoken, `${ms} ms is spoken as ${reading.spoken}, wanted ${spoken}`);
};
// Under a minute the reading is seconds and hundredths and nothing else.
reads(0, "0", "00", "0 seconds");
reads(9, "0", "00", "0 seconds");
reads(10, "0", "01", "0 seconds");
reads(1_000, "1", "00", "1 second");
reads(9_990, "9", "99", "9 seconds");
reads(12_345, "12", "34", "12 seconds");
reads(59_999, "59", "99", "59 seconds");
// The minutes arrive at 60 seconds, and the hours at 60 minutes.
reads(60_000, "1:00", "00");
reads(65_430, "1:05", "43");
reads(754_560, "12:34", "56");
reads(3_599_990, "59:59", "99");
reads(3_600_000, "1:00:00", "00");
reads(5_025_678, "1:23:45", "67");
// The hundredths have no timer: the half-second tick is the only one.
expect(JSON.stringify(timers) === JSON.stringify([["paintLiveClock", 500]]),
  `the clock's timers are ${JSON.stringify(timers)}`);
// A stopped clock posts nothing with a frame, and does not look for the plate.
liveClock.ms = 4_000; liveClock.runningSince = null;
paintLiveClockWithFrame();
expect(writes.lookups === 0 && writes.words === 0 && writes.hundredths === 0, "a stopped clock posted");
// A running one posts with every frame that has a new hundredth, and writes
// the words only when the second turns.
liveClock.runningSince = now;
paintLiveClockWithFrame();
expect(plate.textContent === "4" && plate.dataset.hundredths === "00" &&
       writes.words === 1 && writes.hundredths === 0 && writes.lookups === 1, "the first frame");
for (const [later, text, hundredths] of [[16, "4", "01"], [33, "4", "03"], [50, "4", "05"]]) {
  now = 1_000_000 + later;
  paintLiveClockWithFrame();
  expect(plate.textContent === text && plate.dataset.hundredths === hundredths, `the frame at ${later} ms`);
}
expect(writes.words === 1 && writes.hundredths === 3 && writes.lookups === 1,
  `three frames wrote ${JSON.stringify(writes)}`);
// Two frames inside one hundredth are one post.
now = 1_000_000 + 52;
paintLiveClockWithFrame();
expect(writes.hundredths === 3, "a frame with nothing new posted");
// The second turns: the words and the fraction come from one reading.
now = 1_000_000 + 1_004;
paintLiveClockWithFrame();
expect(plate.textContent === "5" && plate.dataset.hundredths === "00" && writes.words === 2,
  "the second did not turn with its fraction");
// The plate was rebuilt: the next frame finds the new value.
plate.isConnected = false;
plate = value();
now = 1_000_000 + 1_250;
paintLiveClockWithFrame();
expect(writes.lookups === 2 && plate.textContent === "5" && plate.dataset.hundredths === "25",
  "a rebuilt plate was not followed");
// The minute turns with a frame: the words grow their minutes there.
liveClock.ms = 59_990; liveClock.runningSince = now;
paintLiveClockWithFrame();
expect(plate.textContent === "59" && plate.dataset.hundredths === "99", "the last hundredth under a minute");
now += 10;
paintLiveClockWithFrame();
expect(plate.textContent === "1:00" && plate.dataset.hundredths === "00", "the minute did not arrive at 60 seconds");
// No plate at all (a hand-played game has no stopwatch) is not an error.
plate.isConnected = false;
plate = null;
paintLiveClockWithFrame();
paintLiveClock();
console.log("live clock checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class LiveTimeCountsHundredthsWithTheFramesAlreadyPainting(unittest.TestCase):
    """Operator, 2026-11-21: "add hundredths of a second to the live time count
    (not every one will post... only when frames are already updating".

    The fraction is posted by the two calls that put a picture of the map on
    screen and by the half-second tick the seconds already had. It is an
    attribute the stylesheet prints, because the type fitter answers every
    change of words in the masthead by measuring all of it.

    Operator, later the same day: "live time can just default to second and
    hundredths of a second. if we need minutes, we can add that in at 60s".
    The reading is `12.34` until it has a minute to show, then `1:00.00`.

    The stopwatch is a script of its own, ahead of the renderer: app.js stood
    three lines under this file's ceiling when the hundredths arrived."""

    def setUp(self):
        self.source = (ASSETS / "app.js").read_text(encoding="utf-8")
        self.clock = (ASSETS / "app_clock.js").read_text(encoding="utf-8")
        self.page = INDEX.read_text(encoding="utf-8")

    def cut(self, start, end):
        begin = self.source.index(start)
        return self.source[begin:self.source.index(end, begin)]

    def test_the_stopwatch_loads_ahead_of_the_renderer_that_calls_it(self):
        scripts = page_scripts()
        self.assertLess(scripts.index("app_clock.js"), scripts.index("app.js"))
        self.assertTrue(self.clock.startswith('"use strict";'))
        for name in ("liveClockReading", "syncLiveClockRun", "syncLiveClock", "paintLiveClock",
                     "paintLiveClockWithFrame"):
            self.assertIn(f"\nfunction {name}(", self.clock)
            self.assertNotIn(f"function {name}(", self.source)
        for call in ("syncLiveClockRun();", "syncLiveClock(st, newWorld);", "liveClockReading()"):
            self.assertIn(call, self.source)

    def test_the_reading_and_when_it_is_posted(self):
        clock = self.clock
        block = (clock[clock.index("let liveClock = {seed: undefined"):
                       clock.index("// Start or stop the accumulator")]
                 + clock[clock.index("let liveClockValue = null;"):])
        done = subprocess.run(
            [NODE, "-e", LIVE_CLOCK_SCENARIOS.replace("__BLOCK__", block)],
            capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("live clock checks passed", done.stdout)

    def test_the_frames_that_carry_it_are_the_maps_own(self):
        for painter in ("function draw() {", "function drawPlanetGpuAnimationFrame() {"):
            body = self.cut(painter, "\n}\n")
            self.assertEqual(body.count("paintLiveClockWithFrame();"), 1, painter)
        # Nothing else asks for a frame on the clock's account.
        self.assertEqual(self.source.count("paintLiveClockWithFrame()"), 2)
        self.assertEqual(self.clock.count("paintLiveClockWithFrame()"), 1)
        self.assertEqual((self.source + self.clock).count("setInterval(paintLiveClock"), 1)
        self.assertNotIn("requestAnimationFrame(paintLiveClock", self.source + self.clock)

    def test_the_fraction_is_not_a_change_of_words(self):
        # The fitter follows text and child changes in the masthead, never
        # attributes, and the plate's value carries the fraction as one.
        self.assertIn(
            "responsiveTypeMutationObserver.observe(root, {subtree:true, childList:true, characterData:true});",
            self.source)
        self.assertIn('`<b data-live-time data-hundredths="${liveTime.hundredths}">${liveTimeText}</b></div>`',
                      self.source)
        self.assertIn('.turn-live b[data-hundredths]::after { content: "." attr(data-hundredths); font-size: .78em; }',
                      self.page)
        # What a screen reader is told stays in whole seconds, with the unit a
        # bare number under a minute does not carry.
        self.assertIn("(liveTime ? `live time ${liveTime.spoken}; ` : ``) +", self.source)

    def test_the_row_is_fitted_as_one_reading(self):
        self.assertIn('{selector:"#playerhud .turn-live", min:10, max:14},', self.source)
        self.assertNotIn('"#playerhud .turn-live .turn-label"', self.source)
        row = self.page[self.page.index("  .turn-live {"):self.page.index("  .turn-era {")]
        for piece in ("justify-content: safe center;", "margin: 0 12px 4px 0;", "font-size: 14px;",
                      ".turn-live .turn-label { font-size: .8em; }", "font: 850 1.04em/1 var(--font-ui);",
                      "font-variant-numeric: tabular-nums;"):
            self.assertIn(piece, row)


ARENA_STATS_SCENARIOS = r"""
const stored = {};
const localStorage = {getItem: key => stored[key] ?? null, setItem: (key, value) => { stored[key] = value; }};
// The viewer's own calendar: every date here is built from local parts, so
// the day read back is the day written whatever zone the runner keeps.
let now = new Date(2026, 10, 21, 15, 0).getTime();
Date.now = () => now;
const RULES = {strategies: []};
const isBattlefieldMapScript = script => script === "battlefield";
const reportedTurn = st => st.turn;
const victoryVerdict = type => type;
const escapeAttr = text => text;
const hudFoldButton = (name, label) => `<fold ${name}: ${label}>`;
let hudDraws = 0;
const drawPlayerHud = () => { hudDraws++; };
const world = turn => ({server_instance: "one", turn, victory_type: "Science", map: {script: "continents"},
  players: [{id: 0, civ: "Rome"}, {id: 1, civ: "Greece"}]});
let state = world(120);
__SCRIPT__
const expect = (ok, note) => { if (!ok) throw new Error(note); };
const summary = () => {
  const markup = simulationStatsSection();
  const row = markup.match(/<div class="sim-stats-summary"( title="([^"]*)")?>(.*?)<\/div>/);
  expect(row, `no summary in ${markup}`);
  return {text: row[3].replace(/<[^>]+>/g, ""), parts: row[3], title: row[2] ?? null, markup};
};
const reads = (text, title, note) => {
  const got = summary();
  expect(got.text === text && got.title === title, `${note}: read "${got.text}" titled ${got.title}`);
};
// The panel is named for the product, in its heading and in what it tells a
// screen reader.
const titled = summary().markup;
// Its one break is after CIVVIS, so a narrow panel sets it on two lines.
expect(titled.includes("<span>CIVVIS Arena&nbsp;Stats</span>"), "the heading is not CIVVIS Arena Stats");
expect(titled.includes('aria-label="Move CIVVIS Arena Stats; use arrow keys or drag"'), "the handle");
expect(titled.includes("<fold simstats: CIVVIS Arena Stats>"), "the fold control");
expect(!/[^S] Arena Stats|>Arena Stats/.test(titled), "the old title is still printed somewhere");
// Before any game there is no record, so no day.
reads("No finished games yet", null, "an empty record");

// The first game a record counts stamps the day, and the day stands before
// the games.
recordSimulationResult(state, "first", [0]);
expect(simulationStats.civ.since === now, "the first game did not stamp the start");
reads("11-21 start, 1 game, avg 120 turns", "This record began 2026-11-21", "one game");
// The start and the games are two parts, each kept whole, so a narrow panel
// breaks the line between them. Reset is written ahead of both: it floats
// beside the first line.
expect(summary().parts === "<span>11-21 start,</span> <span>1 game, avg 120 turns</span>",
  `the summary's parts are ${summary().parts}`);
expect(/<div class="sim-stats-summary-row"><button class="sim-stats-clear"[^>]*>Reset<\/button><div class="sim-stats-summary"/
  .test(summary().markup), "Reset is not ahead of the summary it floats beside");
expect(JSON.parse(stored["civvis-simulation-stats-v1"]).civ.since === now, "the start was not saved");

// Later games add to the record and leave its start where it was.
const began = now;
now = new Date(2026, 10, 23, 9, 30).getTime();
state = world(180);
recordSimulationResult(state, "second", [1]);
expect(simulationStats.civ.since === began, "a later game moved the start");
reads("11-21 start, 2 games, avg 150 turns", "This record began 2026-11-21", "two games");

// Reset drops the day with the rest; the next game begins a new record.
clearSimulationStats("civ");
expect(simulationStats.civ === undefined, "Reset left a record behind");
reads("No finished games yet", null, "a reset record");
now = new Date(2027, 0, 5, 0, 5).getTime();
recordSimulationResult(state, "third", [0]);
reads("01-05 start, 1 game, avg 180 turns", "This record began 2027-01-05", "a record begun in January");

// Each mode keeps its own record and its own day.
now = new Date(2027, 0, 9, 23, 55).getTime();
state = {...world(40), map: {script: "battlefield"}};
recordSimulationResult(state, "fourth", [1]);
reads("01-09 start, 1 game, avg 40 turns", "This record began 2027-01-09", "the Tactics record");
expect(simulationStats.civ.since === new Date(2027, 0, 5, 0, 5).getTime(), "Tactics moved the Civ start");

// A record kept before the stamp existed has no day to give. It prints none,
// and counting another game does not date the games already in it.
state = world(200);
simulationStats.civ = {games: 5, turns: 500, civs: {}, agents: {}, victories: {}};
reads("5 games, avg 100 turns", null, "a record with no saved start");
recordSimulationResult(state, "fifth", [0]);
expect(simulationStats.civ.since === undefined, "an old record was dated by a later game");
reads("6 games, avg 117 turns", null, "an old record after another game");
for (const junk of ["soon", -1, 0, null, NaN, 8.7e15])
  expect(simulationStatsStart(junk) === null, `${junk} read as a day`);
console.log("arena stats checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class ArenaStatsNameTheProductAndTheDayTheRecordBegan(unittest.TestCase):
    """Operator, 2026-11-21: "change "arena stats" to CIVVIS Arena Stats. add a
    MM-DD start before games".

    The record is the viewer's own, kept in their browser, so the day is read
    on their calendar. It is stamped with the first game the record counts and
    goes with the rest on Reset. A record that was already being kept when the
    stamp arrived never saved a day, and is given none."""

    def test_a_narrow_panel_breaks_the_line_and_never_a_part(self):
        page = INDEX.read_text(encoding="utf-8")
        for rule in (
            ".sim-stats-summary-row { flex: 0 0 auto; display: flow-root; min-width: 0; overflow: hidden; }",
            ".sim-stats-summary::first-line { line-height: 1.9; }",
            ".sim-stats-summary > span { white-space: nowrap; }",
            ".sim-stats-summary-row .sim-stats-clear { float: right; margin-left: 6px; }",
            # The title stands on two lines where its bar is too narrow for
            # one, and the bar's own width is what decides it.
            "#victoryhud > .sim-stats-region .hud-region-bar { container: sim-stats-bar / inline-size; }",
            "#sim-stats-title > span { min-width: 0; }",
            "@container sim-stats-bar (max-width: 157px) {\n"
            "    #sim-stats-title > span { white-space: normal; text-align: center; line-height: 1.1; }\n  }",
        ):
            self.assertIn(rule, page)
        # It is fitted by the run it really sets, after the rule every heading
        # in the panel shares: the later rule is the one an element keeps.
        source = (ASSETS / "app.js").read_text(encoding="utf-8")
        shared = '{selector:"#victoryhud .hud-section-heading > span", min:8.5, max:12, parentWidth:true},'
        own = '{selector:"#sim-stats-title > span", min:8.5, max:12, parentWidth:true, exact:true},'
        self.assertIn(shared, source)
        self.assertIn(own, source)
        self.assertLess(source.index(shared), source.index(own))

    def test_the_title_and_the_start_before_the_games(self):
        script = (ASSETS / "app_arena.js").read_text(encoding="utf-8")
        done = subprocess.run(
            [NODE, "-e", ARENA_STATS_SCENARIOS.replace("__SCRIPT__", script)],
            capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("arena stats checks passed", done.stdout)


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class LaunchesLeaveTheirSpaceport(unittest.TestCase):
    """A launch starts at its Spaceport and flies its own path, or is not drawn.

    Looking at the whole globe, one satellite launch in six came in over the
    edge of the frame hundreds of pixels from its Spaceport, flew straight
    across the picture and left, and its craft then appeared in orbit somewhere
    else: the pad was round the back, and a climb seen for under a third of its
    film was swapped for a "crossing". Zoomed in, every launch whose pad was off
    the frame did the same. These run the planners themselves.
    """

    def setUp(self):
        self.source = (ASSETS / "app.js").read_text(encoding="utf-8")

    def cut(self, start, end):
        begin = self.source.index(start)
        return self.source[begin:self.source.index(end, begin)]

    def fn(self, name):
        return self.cut(f"function {name}(", "\n}\n") + "\n}\n"

    def line(self, start):
        return self.cut(start, "\n") + "\n"

    def run_node(self, code):
        done = subprocess.run([NODE, "-e", code], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr[-3000:])
        return done.stdout

    STUBS = r'''
const MAPW = 1188, MAPH = 850;
let boxes = [];
function skyWidgetBoxes() { return boxes; }
function spinRunning() { return false; }
let worldSpinPeriodMs = 36000;
const anim = {skySettledOrbits:new Map(), skyOrbitSlots:new Map(), skyLaunches:[]};
function expect(condition, message) { if (!condition) throw Error(message); }
const far = (a, b) => Math.hypot(a.x - b.x, a.y - b.y);
// Places spread evenly over the whole world, the far side included.
function everywhere(count) {
  const out = [];
  for (let index = 0; index < count; index++) {
    const height = 1 - (2 * index + 1) / count, across = Math.sqrt(1 - height * height);
    out.push([Math.cos(index * 2.39996) * across, Math.sin(index * 2.39996) * across, height]);
  }
  return out;
}
'''

    def orbiters(self):
        """Whose orbit a launch flies to, out of app_launches.js."""
        launches = (ASSETS / "app_launches.js").read_text(encoding="utf-8")
        start = launches.index("const SKY_LASER_SEAT")
        return launches[start:launches.index("// Which Spaceports sent up the stations")]

    def projection(self):
        return (self.cut("const PLANET_PROJECT_TRIG", "function planetProjectTrig(")
                + self.fn("planetProjectTrig") + self.fn("planetProject")
                + self.fn("hash2") + self.fn("skyOnStage"))

    def test_a_satellite_starts_on_its_pad_or_comes_round_the_limb(self):
        block = (self.STUBS + self.projection()
                 + self.fn("skyOrbit") + self.line("const SKY_SATELLITE_ALTITUDE_SCALE")
                 + self.fn("skyOrbitSlot") + self.fn("skySatelliteOrbit")
                 + self.fn("skySatelliteAngle") + self.fn("skySatelliteSite")
                 + self.fn("skyCameraIn") + self.fn("skyFlightLens") + self.fn("skySightTest")
                 + self.line("const skyCoastLaw") + self.fn("skySatelliteClimb")
                 + self.line("const SKY_HIDDEN_START") + self.fn("skySatellitePlan")
                 + self.fn("skySatelliteRoute") + self.fn("skyHomeInPicture")
                 + self.line("const SKY_WORLD_IS_A_DOT") + self.line("const SKY_MARKERS")
                 + self.orbiters() + self.fn("skyGlobeFlight"))
        scenarios = r'''
function key(pos) { return pos[0] + "," + pos[1]; }
const PLANET = {cells:new Map([["3,4", {}]])};
let pad = [1, 0, 0];
function skyLaunchPad() { return pad; }
const stage = {left:0, top:0, right:MAPW, bottom:MAPH};
// The whole globe in the picture, as the page opens; pulled back; and zoomed
// in on one face, where most of the world's pads are out of the picture. A
// laser station flies the same climb to an orbit of its own, half a ring out.
for (const mission of ["satellite", "laser"])
for (const [R, least] of [[404.5, .7], [150, .8], [900, .1]]) {
  const CX = MAPW / 2, CY = MAPH / 2;
  const camera = {latitude:.35, longitude:1.1, roll:0}, view = {...camera, radius:1};
  const inSight = skySightTest(R, CX, CY);
  const pads = everywhere(240);
  let drawn = 0, lifted = 0, round = 0;
  pads.forEach((place, index) => {
    pad = place;
    anim.skySettledOrbits.clear(); anim.skyOrbitSlots.clear();
    const player = {id:index % 9 + 1}, now = 1000;
    const launch = {mission, station:2, duration:3200, t0:now, endsAt:now + 3200, originPos:[3, 4]};
    const film = skyGlobeFlight(launch, player, undefined, {satellite:[player]}, camera, R, CX, CY, stage, 36);
    expect(launch.route.kind === (film ? "orbit" : "unseen"), "a launch to orbit is its climb or nothing: " + launch.route.kind);
    // A station settles its own orbit and leaves its civilization's alone, or
    // the satellite already up there would jump onto a new one.
    const flyer = skyOrbiter(launch, player), ring = mission === "laser" ? SKY_LASER_RING : 0;
    expect(anim.skySettledOrbits.has(flyer.id) &&
           (mission === "satellite" || !anim.skySettledOrbits.has(player.id)), "the wrong orbit was settled");
    // Whatever is drawn of it, the climb is the one from this pad to this orbit.
    const climb = skySatelliteRoute(launch, flyer, ring, view, R, CX, CY);
    const site = planetProject(pad, view, R, CX, CY);
    expect(far(climb.pointAt(0), site) < 1e-6, "the climb does not start on the pad");
    const orbit = skySatelliteOrbit(flyer, ring, R);
    const craft = planetProject(skySatelliteSite(orbit, skySatelliteAngle(orbit, launch.endsAt)), view, R, CX, CY);
    expect(far(climb.pointAt(1), craft) < 1e-3, "the climb does not end where the craft is: " + far(climb.pointAt(1), craft));
    if (!film) return;
    drawn++;
    // The film opens with the rocket standing on a pad in sight, or out of sight.
    const clock = (now - launch.t0) / launch.duration, opens = film.pointAt(clock);
    if (inSight(site) && clock === 0) { lifted++; return; }
    expect(!inSight(opens) || far(opens, site) < 30,
      `pad ${index}: the film opens ${far(opens, site).toFixed(0)} px from a hidden pad, in plain sight`);
    // And where it first shows is the limb or the edge of the frame on its own
    // climb, never open sky, and it is in sight for longer than a blink.
    let first = null, shown = 0;
    for (let step = 0; step <= 800; step++) {
      const point = film.pointAt(clock + (1 - clock) * step / 800);
      if (!inSight(point)) continue;
      first = first || point;
      shown += (1 - clock) * launch.duration / 800;
    }
    expect(first && shown >= 200, `pad ${index}: a drawn launch in sight for ${shown.toFixed(0)} ms`);
    const limb = Math.abs(Math.hypot(first.x - CX, first.y - CY) - R);
    const edge = Math.min(first.x, MAPW - first.x, first.y, MAPH - first.y);
    expect(Math.min(limb, edge) < 14 || far(first, site) < 30,
      `pad ${index}: appears in open sky, ${limb.toFixed(0)} px off the limb and ${edge.toFixed(0)} px inside the frame`);
    round++;
  });
  expect(drawn >= pads.length * least, `${mission}, radius ${R}: only ${drawn} of ${pads.length} launches are drawn`);
  expect(lifted > 0 && (R > 500 || round > 0), `${mission}, radius ${R}: ${lifted} lift-offs and ${round} round the limb`);
}
'''
        self.run_node(block + scenarios)

    def test_the_mission_planner_never_flies_a_rocket_in_from_the_edge(self):
        block = (self.STUBS + self.projection()
                 + self.fn("skyLineSpanInBox") + self.fn("skyLaunchVisibleStretch") + self.fn("skyLaunchFilm")
                 + self.line("const SKY_THRUST_POWER") + self.fn("skyThrustLaw")
                 + self.line("const skyCoastLaw") + self.cut("const skyBrakeLaw", "};") + "};\n"
                 + self.fn("skyHomeInPicture") + self.fn("skyLaunchPassesHere")
                 + self.line("const SKY_WORLD_IS_A_DOT") + self.line("const SKY_MARKERS")
                 + self.fn("skyArrivalRoute")
                 + self.fn("skyPlanMission") + self.fn("skyOrbitSlot") + self.orbiters()
                 + self.fn("skyGlobeFlight"))
        scenarios = r'''
function key(pos) { return pos[0] + "," + pos[1]; }
const PLANET = {cells:new Map([["3,4", {center:[0, -1, 0]}]])};
let pad = [0, -1, 0], goal = {x:0, y:0}, ascent = null;
function skyLaunchPad() { return pad; }
function skyLaunchEnd() { return goal; }
function skyBestAscent() { return ascent; }
function skyAscentAt(shape, progress) { return {x:progress * 100, y:0, z:1}; }
function skyLandingApproach() { return null; }
function skyLaunchFlyby() { return {flyby:true}; }
function skyTransferRoute() { return {transfer:true}; }
function skySatellitePlan() { throw Error("not a satellite"); }
function skyDotDeparture() { throw Error("not a satellite"); }
const stage = {left:0, top:0, right:MAPW, bottom:MAPH};
const camera = {latitude:0, longitude:-Math.PI / 2, roll:0};
const crews = {satellite:[], moon:[], mars:[], expedition:[]};
function fly(radius, centerX, centerY, known = true) {
  const launch = {mission:"moon", duration:2800, t0:0, endsAt:2800, originPos:known ? [3, 4] : null};
  const film = skyGlobeFlight(launch, {id:1}, {id:"moon", x:goal.x, y:goal.y, r:6}, crews,
                              camera, radius, centerX, centerY, stage, 36);
  return {launch, film, kind:launch.route.kind};
}
const CX = MAPW / 2, CY = MAPH / 2;

// The world fills its share of the picture, the Moon is far off the frame, and
// no climb from this pad can be seen: nothing is drawn. It was a crossing.
goal = {x:-20000, y:-9000}; ascent = null;
let flown = fly(404.5, CX, CY);
expect(flown.kind === "unseen" && flown.film === null, "an unseen launch was drawn: " + flown.kind);
// One that can be seen is the climb from its pad.
ascent = {from:0, law:clock => clock};
flown = fly(404.5, CX, CY);
expect(flown.kind === "ascent" && flown.film && !flown.film.entering, "a visible climb was not flown");

// Home a marble and the Moon off the frame up and to the left: the rocket
// leaves the marble on the Moon's bearing, whatever way east is at its pad.
ascent = {from:0, law:clock => clock};
flown = fly(8, CX, CY);
expect(flown.kind === "depart", "a marble's rocket climbed east instead of leaving: " + flown.kind);
const out = {x:flown.film.end.x - flown.film.start.x, y:flown.film.end.y - flown.film.start.y};
const bearing = {x:goal.x - CX, y:goal.y - CY};
const cosine = (out.x * bearing.x + out.y * bearing.y) / (Math.hypot(out.x, out.y) * Math.hypot(bearing.x, bearing.y));
expect(far(flown.film.start, {x:CX, y:CY}) <= 8 + 1e-6, "the departure does not start at home");
expect(cosine > .999 && flown.film.leaving, "the departure is not on the Moon's bearing: " + cosine);
expect(flown.film.law(0) === 0 && flown.film.law(.5) < .3, "a departure starts from rest");
// Too small to show its cities is still a marble, and a pad round the back of
// it is behind it: the rocket comes out from behind the world.
pad = [0, 1, 0];
flown = fly(40, CX, CY);
expect(flown.kind === "depart" && flown.film.pointAt(0).z < 0 && flown.film.pointAt(1).z === 0,
  "a far-side departure was drawn across the face of the world");
pad = [0, -1, 0];

// Both ends in the picture: the whole trip, however big home is drawn.
goal = {x:CX + 420, y:CY - 60};
expect(fly(150, CX, CY).kind === "transfer", "a trip in the picture was not flown whole");
expect(fly(3, CX, CY).kind === "transfer", "a trip between dots was not flown whole");

// An empire whose lands were never seen has no pad to be watched leaving.
goal = {x:-20000, y:-9000};
expect(fly(404.5, CX, CY, false).kind === "unseen", "a rocket left a stand-in pad");
expect(fly(40, CX, CY, false).kind === "depart", "a marble needs no pad to leave from");

// Parked at another world with home far out of the picture: it goes by, as
// before. Home back in the picture before it has: nothing is flown across it.
goal = {x:MAPW * 4, y:CY};
flown = fly(40, -6000, CY);
expect(flown.kind === "crossing" && flown.film && flown.film.flyby, "the flyby went away");
const late = {mission:"moon", duration:2800, t0:0, endsAt:2800, originPos:[3, 4], route:{kind:"crossing"}};
expect(skyGlobeFlight(late, {id:1}, {id:"moon", x:goal.x, y:goal.y, r:6}, crews,
                      camera, 404.5, CX, CY, stage, 36) === null, "a crossing was flown over home");
'''
        self.run_node(block + scenarios)

    def test_a_board_shows_only_the_climb_that_is_in_its_frame(self):
        block = (self.STUBS + self.line("const SKY_THRUST_POWER") + self.fn("skyThrustLaw")
                 + self.fn("skyBoardFlight"))
        scenarios = r'''
const stage = {left:-500, right:500, top:-300, bottom:300};
const climb = (x, y) => ({start:{x, y}, law:clock => clock,
                          pointAt:progress => ({x:x + 40 * progress, y:y - 900 * progress})});
// A pad above the frame climbs away from it: not drawn. It used to come up
// through the picture from the bottom edge.
expect(skyBoardFlight({}, climb(0, -400), stage) === null, "a launch off the board was drawn passing through");
// A pad below the frame climbs up into it, already moving.
const entering = skyBoardFlight({}, climb(0, 500), stage);
expect(entering && entering.entering && entering.law(0) > .15, "a climb into the frame did not come in moving");
// A pad in the frame is the route as planned.
const plain = climb(0, 100);
expect(skyBoardFlight({}, plain, stage) === plain, "a launch in the frame was changed");
'''
        self.run_node(block + scenarios)

    def test_the_pad_is_the_spaceport_and_a_craft_keeps_its_ring(self):
        block = (self.STUBS + self.fn("districtIconPositions") + self.fn("citySpaceportPos")
                 + self.fn("skyLaunchOrigin") + self.fn("skyOrbitSlot"))
        scenarios = r'''
const RULES = {districts:{spaceport:{}, campus:{}, cosmodrome:{replaces:"spaceport"}}};
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const project = "launch_earth_satellite";
// The city that ran the project, then any Spaceport of the empire.
let prev = {cities:[{id:1, owner:2, queue:[{unit:"settler"}], districts:{spaceport:[9, 9]}},
                    {id:2, owner:2, queue:[{project}], districts:{campus:[1, 1], spaceport:[5, 6]}}]};
let next = {cities:prev.cities.map(city => ({...city, queue:[]})), map:{tiles:[]}};
expect(same(skyLaunchOrigin(prev, next, 2, project), [5, 6]), "not the Spaceport that ran the project");
expect(same(skyLaunchOrigin({cities:[]}, next, 2, project), [9, 9]), "not a Spaceport of the empire");
// Through another civilization's eyes the rival's cities keep their districts
// to themselves, but the map shows the Spaceport on its tile.
next = {cities:[{id:2, owner:2, queue:[]}],
        map:{tiles:[{pos:[4, 4], owner:2, district:"campus"}, {pos:[7, 2], owner:3, district:"spaceport"},
                    {pos:[8, 3], owner:2, district:"cosmodrome"}]}};
expect(same(skyLaunchOrigin({cities:[]}, next, 2, project), [8, 3]), "the map's own Spaceport was not found");
expect(skyLaunchOrigin({cities:[]}, {cities:[], map:{tiles:[]}}, 2, project) === null, "a pad was made up");

// Two craft up, then a civilization earlier in the roster launches: the two
// keep their rings and the newcomer takes the next one out.
const a = {id:0}, b = {id:1}, c = {id:2};
expect(skyOrbitSlot([b, c], b) === 0 && skyOrbitSlot([b, c], c) === 1, "rings are not dealt in order");
expect(skyOrbitSlot([a, b, c], b) === 0 && skyOrbitSlot([a, b, c], c) === 1, "a launch moved the craft already up");
expect(skyOrbitSlot([a, b, c], a) === 2, "the newcomer did not take the next ring");
expect(skyOrbitSlot([b, c], a) === -1, "a civilization with no craft was given a ring");
'''
        self.run_node(block + scenarios)

    def test_every_laser_station_leaves_the_spaceport_that_built_it(self):
        """A Lagrange Laser Station is the one launch a civilization repeats.

        It is a count, not a row of `science_projects`, and several can finish
        in one frame from different Spaceports. These run the queueing and the
        choice of pad themselves, out of the script the page loads.
        """
        launches = (ASSETS / "app_launches.js").read_text(encoding="utf-8")
        start = launches.index("const SKY_LASER_SEAT")
        block = (self.STUBS + self.fn("districtIconPositions") + self.fn("citySpaceportPos")
                 + self.fn("statePlayerAnchor")
                 + launches[start:launches.index("// How much of one station is out of its rocket")])
        scenarios = r'''
const RULES = {districts:{spaceport:{}, campus:{}, cosmodrome:{replaces:"spaceport"}}};
const SKY_LAUNCH_FLIGHTS = {laser:{project:"lagrange_laser_station", duration:3200, rocket:"orbital", station:true}};
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const laser = {project:"lagrange_laser_station"};
const city = (id, pad, queue, production) =>
  ({id, owner:2, queue, production, districts:{spaceport:pad}});
// Three cities were building stations. One has gone on to something else, one
// started a second station (its production began again), one is still at it.
let prev = {cities:[city(1, [3, 3], [laser], 400), city(2, [5, 6], [laser], 590),
                    city(3, [8, 1], [laser], 580), city(4, [9, 9], [{unit:"tank"}], 50)]};
let next = {cities:[city(1, [3, 3], [laser], 460), city(2, [5, 6], [{unit:"tank"}], 20),
                    city(3, [8, 1], [laser], 12), city(4, [9, 9], [{unit:"tank"}], 90)],
            map:{tiles:[]}};
expect(same(skyLaserOrigins(prev, next, 2, 2), [[5, 6], [8, 1]]),
       "not the two Spaceports that finished: " + JSON.stringify(skyLaserOrigins(prev, next, 2, 2)));
expect(same(skyLaserOrigins(prev, next, 2, 1), [[5, 6]]), "one station, and not from the city that finished it");
// More stations than cities seen finishing: the one still building is the
// likeliest of the rest, then any other Spaceport of the empire.
expect(same(skyLaserOrigins(prev, next, 2, 4), [[5, 6], [8, 1], [3, 3], [9, 9]]),
       "the spare pads are out of order: " + JSON.stringify(skyLaserOrigins(prev, next, 2, 4)));
// Through another civilization's eyes no queue is shown and no districts, but
// the map still has the Spaceport on its tile.
const fogged = {cities:[{id:2, owner:2}],
                map:{tiles:[{pos:[7, 2], owner:3, district:"spaceport"}, {pos:[8, 3], owner:2, district:"cosmodrome"}]}};
expect(same(skyLaserOrigins({cities:[]}, fogged, 2, 2), [[8, 3], [8, 3]]), "the map's own Spaceport was not found");
expect(same(skyLaserOrigins({cities:[]}, {cities:[], map:{tiles:[]}}, 2, 1), [null]), "a pad was made up");

// The count is what launches. One more station is one rocket, from its pad.
const seat = count => ({id:2, laser_stations:count});
queueSkyLaserLaunches(prev, next, seat(1), seat(0), 1000);
expect(anim.skyLaunches.length === 1, "one station did not queue one launch");
let [first] = anim.skyLaunches;
expect(first.mission === "laser" && first.station === 1 && first.playerId === 2, "the launch is not the station's");
expect(same(first.originPos, [5, 6]) && first.t0 === 1000 && first.endsAt === 4200, "wrong pad or clock");
// The same transition seen twice is still one launch, and a count that did
// not move, or a server that sends none, launches nothing.
queueSkyLaserLaunches(prev, next, seat(1), seat(0), 1500);
queueSkyLaserLaunches(prev, next, seat(1), seat(1), 1500);
queueSkyLaserLaunches(prev, next, {id:2}, {id:2}, 1500);
queueSkyLaserLaunches(prev, next, seat(0), seat(1), 1500);
expect(anim.skyLaunches.length === 1, "a launch was queued for no new station");
// Two in one frame go up one after the other, each from its own Spaceport.
anim.skyLaunches.length = 0;
queueSkyLaserLaunches(prev, next, seat(3), seat(1), 2000);
expect(same(anim.skyLaunches.map(l => [l.station, l.t0, l.originPos]),
            [[2, 2000, [5, 6]], [3, 2000 + SKY_LASER_STAGGER, [8, 1]]]), "two stations are not a volley of two");
// A page that fell a long way behind is not owed a rocket for every station
// it missed: the newest few fly and the rest are simply up there.
anim.skyLaunches.length = 0;
queueSkyLaserLaunches(prev, next, seat(9), seat(1), 3000);
expect(same(anim.skyLaunches.map(l => l.station), [7, 8, 9]), "the volley is not the newest stations");
expect(SKY_LASER_VOLLEY === 3 && skyLaserStations(seat(9)) === 9 && skyLaserStations({}) === 0 &&
       skyLaserStations(seat(1e6)) === SKY_LASER_MOST, "the count is not bounded");
// Nobody was seen building it and no Spaceport is in sight: the empire's city.
anim.skyLaunches.length = 0;
queueSkyLaserLaunches({cities:[]}, {cities:[{id:7, owner:2, is_capital:true, pos:[2, 2]}], map:{tiles:[]}},
                      seat(1), seat(0), 0);
expect(same(anim.skyLaunches[0].originPos, [2, 2]), "the fallback is not the empire's own city");

// A station's orbit is its own. No two stations share one and none is a
// civilization's, for any seat a game can hold.
const ids = new Set();
for (let player = 0; player < 64; player++) {
  ids.add(player);
  for (let station = 1; station <= SKY_LASER_MOST; station++)
    ids.add(skyLaserOrbiter(player, station).id);
}
expect(ids.size === 64 * (SKY_LASER_MOST + 1), "two craft share an orbit");
expect(skyOrbiter({mission:"laser", station:3}, {id:5}).id === skyLaserOrbiter(5, 3).id, "a station flew to another orbit");
const civ = {id:5};
expect(skyOrbiter({mission:"satellite"}, civ) === civ, "a satellite did not fly to its civilization's orbit");
'''
        self.run_node(block + scenarios)


GAME_STATS_SCENARIOS = r"""
const SPEC = true;
const document = {querySelector: () => null};
const isBattlefieldMapScript = script => script === "battlefield";
const pcol = id => ["#111111", "#222222", "#333333", "#444444", "#555555", "#666666"][id % 6];
const playerHudEloValue = player => player.ai_elo ?? null;
const playerEloLabel = player => String(Math.round(player.ai_elo));
const escapeAttr = text => String(text ?? "");
__COMPACT__
__BLOCK__
const expect = (condition, note) => { if (!condition) throw new Error(note); };
const eq = (actual, wanted, note) => expect(JSON.stringify(actual) === JSON.stringify(wanted),
  `${note}: got ${JSON.stringify(actual)}, wanted ${JSON.stringify(wanted)}`);

// The arrangement the operator named, and the same rule for every other table.
eq(gameStatsLayout(4), {cols: 2, rows: 2}, "four civilizations");
eq(gameStatsLayout(6), {cols: 3, rows: 2}, "six civilizations");
eq(gameStatsLayout(6, true), {cols: 2, rows: 3}, "six on a card taller than it is wide");
for (let count = 1; count <= 24; count++) {
  const {cols, rows} = gameStatsLayout(count), turned = gameStatsLayout(count, true);
  expect(cols * rows >= count, `${count} civilizations do not fit ${cols} by ${rows}`);
  expect(cols * (rows - 1) < count, `${count} civilizations leave a row of ${cols} by ${rows} empty`);
  expect(cols >= rows, `${count} civilizations are stacked taller than wide`);
  eq(turned, {cols: rows, rows: cols}, `${count} civilizations turned on their side`);
}

// A world: `count` civilizations over `turns` turns, each a little richer
// than the one before it, the second in deficit and the last one fallen.
const world = (count, turns, fallen = false) => {
  const axis = Array.from({length: turns}, (_, i) => i + 1);
  const grow = (rate, gone) => axis.map((turn, i) => gone && i > turns / 2 ? null : Math.round(rate * turn));
  return {winner: 1, winners: [1], map: {script: "continents"},
    players: Array.from({length: count}, (_, id) => ({id, civ: `Civ ${id}`, score: 100 + id, cities: 3 + id,
      population: 20 + id, wonder_count: id % 2, ai_username: `Agent${id}`, ai_elo: 1100 + id,
      alive: !(fallen && id === count - 1)})),
    game_stats: {turns: axis, players: Array.from({length: count}, (_, id) => {
      const gone = fallen && id === count - 1;
      return {id, science: grow(0.5 + id, gone), production: grow(1 + id, gone), culture: grow(0.5 + id, gone),
        gold: grow(id === 1 ? -0.1 : 0.4, gone), faith: grow(0.45, gone), cities: grow(0.05 + id * 0.01, gone)};
    })}};
};

// The markup: the arrangement, one chart each, the winner first, and on a
// watched table the Civ AI Agent that played each civilization.
for (const [count, cols, rows] of [[4, 2, 2], [6, 3, 2]]) {
  const markup = gameStatsMarkup(world(count, 120));
  expect(markup.includes(`style="--gs-cols:${cols};--gs-rows:${rows}"`), `${count} civilizations are not ${cols} by ${rows}`);
  eq(markup.split('<figure class="game-stats-civ"').length - 1, count, "a chart for each civilization");
  eq(markup.split('<svg class="game-stats-chart"').length - 1, count, "a drawing in each chart");
  eq(gameStatsView.rows.map(row => row.id), [1, ...Array.from({length: count}, (_, i) => count - 1 - i).filter(id => id !== 1)],
    "the winner first, then by score");
  expect(markup.includes('<span class="gs-by">Agent1 1101</span>'), "the agent and its rating are not named");
  expect(markup.includes('<span class="gs-won">Winner</span>'), "the winner is not marked");
}
expect(gameStatsMarkup(world(6, 120, true)).includes('<span class="gs-out">Out on turn 61</span>'), "a fallen civilization's last turn");
eq(gameStatsMarkup({...world(4, 120), map: {script: "battlefield"}}), "", "a Tactics battle has no charts");
eq(gameStatsMarkup({...world(4, 1)}), "", "one turn is not a history");
// The card opens small; the charts are behind a Charts button, offered only
// when there is a history to chart.
expect(gameStatsButton(world(4, 120)).includes('id="finale-charts"'), "no Charts button on a finished game");
eq(gameStatsButton({...world(4, 120), map: {script: "battlefield"}}), "", "a Tactics battle offers no charts");
eq(gameStatsButton(world(4, 1)), "", "one turn offers no charts");

// The drawing, at every size a chart is given from a phone to a large screen,
// on both scales: everything drawn is inside the chart, every line is inside
// its plot, and no two last figures are printed over each other.
for (const [w, h] of [[150, 120], [194, 130], [233, 210], [337, 300], [519, 300], [706, 390], [1060, 400]])
  for (const log of [false, true])
    for (const [count, turns, fallen] of [[4, 120, false], [6, 250, true], [12, 500, false]]) {
      const st = world(count, turns, fallen), where = `${w}x${h} ${log ? "log" : "linear"} ${count} civs`;
      gameStatsMarkup(st);
      const view = gameStatsView, scale = gameStatsScales(view, log, w, h), box = scale.box;
      expect(box.left < box.right && box.top < box.bottom && box.bottom < box.stripTop && box.stripBottom < h,
        `${where}: the parts are out of order`);
      const charts = gameStatsCharts(view, log, w, h);
      eq(charts.length, count, `${where}: a drawing each`);
      for (const [index, chart] of charts.entries()) {
        expect(chart.includes(`width="${w}" height="${h}" viewBox="0 0 ${w} ${h}"`), `${where}: not drawn for its pixels`);
        const lines = [...chart.matchAll(/class="gs-line"[^>]* d="([^"]*)"/g)].map(match => match[1]);
        eq(lines.length, 5, `${where}: five yields`);
        for (const path of lines) for (const [, x, y] of path.matchAll(/[ML]([\d.-]+) ([\d.-]+)/g))
          expect(+x >= box.left - 0.1 && +x <= box.right + 0.1 && +y >= box.top - 0.1 && +y <= box.bottom + 0.1,
            `${where}: a line leaves its plot at ${x},${y}`);
        for (const [, x, y] of chart.matchAll(/<text[^>]* x="([\d.-]+)" y="([\d.-]+)"/g))
          expect(+x >= 0 && +x <= w && +y >= box.font * 0.6 && +y <= h, `${where}: a label is off the chart at ${x},${y}`);
        const ends = gameStatsEnds(view.turns, view.rows[index], scale);
        eq(ends.map(end => end.label).sort(), ["Cities", "Culture", "Faith", "Gold", "Production", "Science"], `${where}: a last figure each`);
        const yields = ends.filter(end => end.label !== "Cities").map(end => end.y).sort((a, b) => a - b);
        // Five figures need five lines of type; a plot shorter than that keeps them in order instead.
        if (box.bottom - box.top >= 5 * (box.font + 1)) {
          for (let i = 1; i < yields.length; i++)
            expect(yields[i] - yields[i - 1] >= box.font + 1 - 0.01, `${where}: two last figures overprint`);
          expect(yields[0] >= box.top - 0.01 && yields[4] <= box.bottom + 2.01, `${where}: a last figure left its plot`);
        }
        for (const end of ends) expect(end.x + 6 + end.text.length * box.font * 0.62 <= w + 0.5 || end.x < box.right - 1,
          `${where}: the last figure ${end.text} runs off the chart`);
      }
      // One scale for every chart: the same amount is the same height in each.
      const again = gameStatsScales(view, log, w, h);
      eq([again.y(40), again.cy(2)], [scale.y(40), scale.cy(2)], `${where}: the scale moved`);
      // Linear: the highest figure stands in the top half step, so no step is empty.
      if (!log) {
        const high = Math.max(...view.rows.flatMap(row => ["science", "production", "culture", "gold", "faith"]
          .flatMap(key => row[key].filter(value => value !== null))));
        const steps = scale.ticks.filter(tick => tick > 0).sort((a, b) => a - b);
        expect(scale.y(high) >= box.top && scale.y(high) - box.top <= (box.bottom - box.top) * 0.36,
          `${where}: the lines do not take the plot's height`);
        expect(steps.length >= 2 || h < 200, `${where}: too few rules to read a height from`);
      }
    }
console.log("game stats checks passed");
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class TheResultScreenIsAScreenOfCharts(unittest.TestCase):
    """Operator, 2026-11-21: "4p game should have 2 x 2 enx screen of charts.
    6p game is 3 wide x 2 tall. all on screen. no screen. maximize information
    conveyed cleanly" (read as: end screen, and no scrolling).

    A finished game's card takes the map area. Its verdict is one band across
    the top and the charts share the rest in a fixed arrangement, each drawn
    for the pixels it is given, so nothing is cut off and nothing scrolls."""

    def test_the_arrangement_and_the_drawing(self):
        moments = (ASSETS / "app_moments.js").read_text(encoding="utf-8")
        app = (ASSETS / "app.js").read_text(encoding="utf-8")
        block = moments[moments.index("const GAME_STATS_SERIES = ["):moments.index("// Hover: one turn cursor")]
        start = app.index("function compactStat(")
        compact = app[start:app.index("\n}\n", start)] + "\n}\n"
        done = subprocess.run(
            [NODE, "-e", GAME_STATS_SCENARIOS.replace("__COMPACT__", compact).replace("__BLOCK__", block)],
            capture_output=True, text=True, timeout=120)
        self.assertEqual(done.returncode, 0, done.stderr.strip()[-3000:])
        self.assertIn("game stats checks passed", done.stdout)

    def test_the_charts_open_from_a_charts_button(self):
        """Operator, 2026-11-21: "change the end game charts option to only
        show if the user clicks on a new "Charts" button. Otherwise a smaller
        screen with the next game countdown".

        The card is built without the charts; its Charts button puts them in
        and takes them out. A watched game's count is the simulation's, so
        opening the charts holds the result with Pause here."""
        app = (ASSETS / "app.js").read_text(encoding="utf-8")
        self.assertNotIn("${gameStatsMarkup(st)}", app)
        # Both rows of answers, a watched game's and a played one's.
        self.assertEqual(app.count("${gameStatsButton(st)}"), 2)
        moments = (ASSETS / "app_moments.js").read_text(encoding="utf-8")
        self.assertIn('card.insertAdjacentHTML("beforeend", markup);', moments)
        self.assertIn("if (SPEC && !specPaused) { toggleSpecPause(); gameStatsHeld = true; }", moments)
        self.assertIn("if (gameStatsHeld && SPEC && specPaused) toggleSpecPause();", moments)

    def test_the_card_takes_the_map_area_and_does_not_scroll(self):
        page = INDEX.read_text(encoding="utf-8")
        start = page.index("#winner:has(.game-stats) {")
        card = page[start:page.index("}", start)]
        self.assertIn("height: min(1180px, calc(100% - 24px));", card)
        self.assertIn("overflow: hidden;", card)
        self.assertNotIn("overflow-y", card)
        for rule in (
            # Every chart the same size, and together what the card has left.
            "grid-template-columns: repeat(var(--gs-cols, 3), minmax(0, 1fr));",
            "grid-template-rows: repeat(var(--gs-rows, 2), minmax(0, 1fr));",
            ".game-stats-plot { flex: 1; position: relative; min-height: 0; overflow: hidden; }",
            # The drawing lies over the box it was measured from.
            ".game-stats-chart { position: absolute; inset: 0; display: block; }",
            # The charts are the last row of the card and the one that stretches.
            '"stats   stats   stats   stats   stats   stats";',
        ):
            self.assertIn(rule, page)
        moments = (ASSETS / "app_moments.js").read_text(encoding="utf-8")
        # Drawn again whenever the card is given a different size.
        self.assertIn("gameStatsObserver = new ResizeObserver(gameStatsFit);", moments)
        self.assertIn("queueMicrotask(gameStatsMount);", moments)


A_WORLD_ASKS_ONLY_FOR_ITS_TURNS = r"""
const asked = [], timers = [];
let pane = false;
const group = {style: {}};
const document = {
  documentElement: {classList: {contains: name => pane && name === "strange-pane"}},
  getElementById: () => group,
};
const SPEC = false, readSetting = () => "single", escapeAttr = text => String(text), titleCase = text => String(text);
const fetchJSON = async path => { asked.push(path); return {saves: [], cards: []}; };
const setTimeout = (work, ms) => { timers.push(work); return timers.length; }, clearTimeout = () => {};
let state = {turn: 7, view_player: 0, me: {government: "chiefdom", policies: ["discipline"]}};
let autoplaying = false;
const paintLiveTracker = () => {}, empireOpen = () => false;
let savesKnown = null;
__SAVES__
__LEDGER__
(async () => {
  const check = (held, why) => { if (!held) { console.error(why); process.exit(1); } };
  // A page of the game reads its saves when a turn opens, and prices the
  // deck once the turn has stood a moment.
  await refreshSaves();
  check(asked.join() === "/saves", `a page of the game reads its saves: ${asked}`);
  check(currentPolicyLedger() === null && timers.length === 1, "the deck is priced once the turn has stood");
  timers[0]();
  await null;
  check(asked.join() === "/saves,/policy-effects", `and asks the engine for it: ${asked}`);
  // A world of Doctor Strange's Civ 6 is a map: no lobby to list saves in,
  // no deck in sight. Each turn it is shown it asks for nothing but the turn.
  pane = true; asked.length = 0; timers.length = 0; state = {...state, turn: 8};
  await refreshSaves();
  check(currentPolicyLedger() === null, "a world of Doctor Strange's CIVVIS has no ledger");
  check(asked.length === 0 && timers.length === 0, `and asks for neither: ${asked}, ${timers.length} waiting`);
  console.log("a world asks only for its turns");
})();
"""


@unittest.skipUnless(NODE, "no node on this host; CI runners have one")
class AWorldOfDoctorStrangeAsksOnlyForItsTurns(unittest.TestCase):
    """Operator, 2026-11-24: "Right now Doctor Strange's Civ 6 goes 4 turns and
    pauses. This should not happen. It should be a continuous effort. and
    blazingly fast." Each world is a whole page of the game, and on every
    turn it was shown it read every save again for a lobby it does not show
    (184 requests in 30 s of two worlds, measured on the published page), and
    had the engine price a deck of policy cards nobody can see whenever a
    turn stood for a third of a second (0.6 s of the one engine at nine
    cards), which is what a turn waiting on the engine does."""

    def test_a_world_reads_no_saves_and_prices_no_deck(self):
        setup = (ASSETS / "app_setup.js").read_text(encoding="utf-8")
        saves = setup[setup.index("async function refreshSaves() {"):]
        saves = saves[:saves.index("\n}\n") + 3]
        app = (ASSETS / "app.js").read_text(encoding="utf-8")
        ledger = app[app.index("const POLICY_LEDGER_RETRY_MS ="):]
        ledger = ledger[:ledger.index("\n}\n", ledger.index("function currentPolicyLedger() {")) + 3]
        script = A_WORLD_ASKS_ONLY_FOR_ITS_TURNS.replace("__SAVES__", saves).replace("__LEDGER__", ledger)
        done = subprocess.run([NODE, "-e", script], capture_output=True, text=True, timeout=60)
        self.assertEqual(done.returncode, 0, done.stderr.strip())
        self.assertIn("a world asks only for its turns", done.stdout)

    def test_the_page_is_a_world_before_any_turn_opens(self):
        # The page says it is a world as its last script loads, which is
        # before it is given a state to draw.
        debugger = (ASSETS / "app_debugger.js").read_text(encoding="utf-8")
        self.assertIn('if (STRANGE_PANE) {\n  document.documentElement.classList.add("strange-pane");', debugger)
