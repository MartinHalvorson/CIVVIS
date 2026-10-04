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


if __name__ == "__main__":
    unittest.main()


class TheDedicatedDisplayShowsTheHud(unittest.TestCase):
    """The window the keeper opens beside a live game exists to show the player
    HUD. Its Chrome profile keeps this page's localStorage across launches, so
    one stray click that hid the players overlay hid it for every game after —
    on 2026-09-10 the display beside a live Emperor game showed a bare map."""

    def test_dedicated_mode_shows_only_the_player_hud(self):
        source = (REPO / "web" / "assets" / "app.js").read_text(encoding="utf-8")
        gate = source.index('get("display") === "dedicated"')
        # After the saved overlays are read, so the forced values win.
        self.assertGreater(gate, source.index("OVERLAY_STORAGE_KEY) ||"))
        block = source[gate:gate + 400]
        self.assertIn("OVERLAY_VISIBILITY.players = true", block)
        for name in ("victory", "minimap", "controls", "lenses"):
            self.assertIn(f"OVERLAY_VISIBILITY.{name} = false", block)

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
check(activeAreaSubjects(world(1, 10, [...army, guard], [addis]), 0, (t += 5000)).length === 49,
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
