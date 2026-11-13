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
                 + self.line("const SKY_WORLD_IS_A_DOT") + self.fn("skyGlobeFlight"))
        scenarios = r'''
function key(pos) { return pos[0] + "," + pos[1]; }
const PLANET = {cells:new Map([["3,4", {}]])};
let pad = [1, 0, 0];
function skyLaunchPad() { return pad; }
const stage = {left:0, top:0, right:MAPW, bottom:MAPH};
// The whole globe in the picture, as the page opens; pulled back; and zoomed
// in on one face, where most of the world's pads are out of the picture.
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
    const launch = {mission:"satellite", duration:3200, t0:now, endsAt:now + 3200, originPos:[3, 4]};
    const film = skyGlobeFlight(launch, player, undefined, {satellite:[player]}, camera, R, CX, CY, stage, 36);
    expect(launch.route.kind === (film ? "orbit" : "unseen"), "a satellite launch is its climb or nothing: " + launch.route.kind);
    // Whatever is drawn of it, the climb is the one from this pad to this orbit.
    const climb = skySatelliteRoute(launch, player, 0, view, R, CX, CY);
    const site = planetProject(pad, view, R, CX, CY);
    expect(far(climb.pointAt(0), site) < 1e-6, "the climb does not start on the pad");
    const orbit = skySatelliteOrbit(player, 0, R);
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
  expect(drawn >= pads.length * least, `radius ${R}: only ${drawn} of ${pads.length} launches are drawn`);
  expect(lifted > 0 && (R > 500 || round > 0), `radius ${R}: ${lifted} lift-offs and ${round} round the limb`);
  console.log(R, "drawn", drawn, "lifted", lifted, "round the limb or edge", round, "of", pads.length);
}
'''
        self.run_node(block + scenarios)

    def test_the_mission_planner_never_flies_a_rocket_in_from_the_edge(self):
        block = (self.STUBS + self.projection()
                 + self.fn("skyLineSpanInBox") + self.fn("skyLaunchVisibleStretch") + self.fn("skyLaunchFilm")
                 + self.line("const SKY_THRUST_POWER") + self.fn("skyThrustLaw")
                 + self.line("const skyCoastLaw") + self.cut("const skyBrakeLaw", "};") + "};\n"
                 + self.fn("skyHomeInPicture") + self.fn("skyLaunchPassesHere")
                 + self.line("const SKY_WORLD_IS_A_DOT") + self.fn("skyArrivalRoute")
                 + self.fn("skyPlanMission") + self.fn("skyOrbitSlot") + self.fn("skyGlobeFlight"))
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

// Both ends in the picture: the whole trip, however big home is drawn.
goal = {x:CX + 420, y:CY - 60};
expect(fly(150, CX, CY).kind === "transfer", "a trip in the picture was not flown whole");
expect(fly(3, CX, CY).kind === "transfer", "a trip between dots was not flown whole");

// An empire whose lands were never seen has no pad to be watched leaving.
goal = {x:-20000, y:-9000};
expect(fly(404.5, CX, CY, false).kind === "unseen", "a rocket left a stand-in pad");
expect(fly(8, CX, CY, false).kind === "depart", "a marble needs no pad to leave from");

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
