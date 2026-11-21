#!/usr/bin/env python3
"""Drive the Sim debugger's header box and split in a real browser against a toy engine.

`web/assets/app_debugger.js` is loaded as it ships, on a page that stands in
for the viewer (the handful of globals the bar reads) and an engine that
stands in for `POST /debug` (a turn counter with marks, save points and turn
breakpoints). Nothing here plays a game: what is checked is the wiring, that
each control sends the operation it is for and that the bar shows the answer.

A split is the same page twice more, in frames, so each half gets a toy engine
of its own, started from the host's. The toy's score rises faster under one
Civ AI than the other, which is all it takes to see that each half's steps
carry its own. A request that names the other half as its twin is answered
the way the real engine answers one: when the two toy worlds are the same,
the other is made a copy of the answer and the answer says so.

Why this exists: on 2026-11-19 the bar's slider stopped doing anything. Its
handler drew the bar before reading where the slider had been dragged to, and
drawing puts a slider that is not being dragged back on the open turn. Both
suites were green, the script parsed, and three walks of the real site missed
it because none of them dragged the slider. A control that is not driven is
not tested.

Set `CIVVIS_TEST_CHROME` to run this with a particular browser binary.
"""

from __future__ import annotations

import functools
import http.server
import json
import os
import pathlib
import shutil
import socketserver
import subprocess
import tempfile
import threading
import time
import urllib.request

from verify import Devtools, find_chrome, free_port, fresh_profile, tether

PAGE = r"""<!doctype html><html><head><meta charset="utf-8"><title>bar</title>
<style>
/* The viewer's own frame: a page exactly the size of the window, which never
   scrolls. Without the last rule the picker under the map makes this page a
   line taller than the window, and a browser whose scrollbars take room (the
   Linux runner's) then has a window 15px wider than anything fixed in it. */
* { box-sizing: border-box; margin: 0; } html, body { height: 100%; } body { overflow: hidden; }
/* The viewer's own layout: the left panel is the first column and the map
   area has the rest. The header box stands over the map area only. */
body { display: flex; }
#side { order: -1; flex: 0 0 200px; height: 100%; }
#maparea { flex: 1; min-width: 0; }
</style></head>
<body>
<div id="maparea"></div>
<div id="side">
  <div id="humanfooter" style="display:grid">the left panel's own auto-play</div>
  <select id="autoplaystrategy"><option value="jack">1211 - JackOfAllTrades</option><option value="stock">1000 - ByTheBook</option></select>
</div>
<script>
"use strict";
// ---- what the viewer gives the bar
let state = null, sel = null, selCity = null;
let SPEC = false, autoplaying = false, turnAdvancePending = false;
const AUTOPLAY_BATCH_TIMEOUT_MS = 60000;
// The roster the viewer reads the Civ AIs' names from, and where it keeps its choice.
const RULES = {strategies: [{name: "jack", username: "Jack"}, {name: "stock", username: "Stock"}]};
const AUTOPLAY_STRATEGY_KEY = "civvis-autoplay-strategy-v1";
const unitOrders = new Map(), cityQueues = new Map();
let noticeLog = [], noticeDismissed = new Set(), noticeSeenTurn = 0;
let cam = {x: 0, y: 0, scale: 1, rot: 0, tilt: 0}, SKY_PAN = {x: 0, y: 0};
let cameraFlight = null, cameraZoom = null;
function saveCityQueues() {}
function clearCameraInertia() {}
function draw() {}
function drawMini() {}
function centerOn() {}
function refreshSaves() {}
function loadSave() {}
function togglePanel() {}
function gameFinished() { return false; }
function drawTurnLoop() {}
// The page's two views of a held seat, and what the Single player switch asks
// the page to paint again or put away. `painted` is the record of it.
let SOLO_UI = false;
const painted = [];
for (const name of ["drawSoloHud", "drawLaunchBar", "drawPlayerHud", "drawSide", "drawUbar",
  "advanceToNextActionUnit", "clearUnitIntel", "closeCityScreen", "closeEmpire", "closeDiplomacy", "closeTrade"])
  window[name] = () => { painted.push(name); };
// How many debugger requests had been sent each time a world was drawn.
const drawn = [];
function render(next) { state = next; drawn.push(engine.sent.filter(call => call.path === "/debug").length); }
// Each time the page was told to price the policy deck on screen (the viewer's
// own `currentPolicyLedger`), and whether a turn was still being played.
const priced = [];
function currentPolicyLedger() { priced.push(autoplaying); }
function escapeAttr(text) {
  return String(text).replace(/[&<>"']/g, c => ({"&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"}[c]));
}
function titleCase(text) { return String(text).replace(/\b\w/g, c => c.toUpperCase()); }
async function send() { return state; }
// A run waits a frame between turns, and a page nobody is looking at has none.
window.requestAnimationFrame = drawn => setTimeout(drawn, 0);

// ---- a toy engine: one line of turns, each kept
// `eras` is the first turn of each era the game has been through, and
// `eraAt` a turn on which the next one begins. `seat` is the Civ AI last
// named for the seat, and `by` the one whose play opened each turn.
const engine = {turn: 1, marks: [1], saves: [], ids: 0, breaks: [], line: null, stopped: null, score: 0,
  seat: null, by: {},
  current: 0, stepped: null, acted: 0, reveal: false, layers: [], wait: 0,
  eras: [{era: 0, name: "ancient", turn: 1}], eraAt: null,
  watch: {categories: [], phrases: [], important: false, every_seat: false, conditions: [], eras: false}, sent: []};
const ERAS = ["ancient", "classical", "medieval"];
// The round: the played seat, one rival, and the Barbarians.
const SEATS = 3;
// Another frame's engine as it stands, for a half that is copied or kept:
// everything but the record of what this frame's own page sent.
function engineOf(frames, pane) {
  const frame = [...frames].find(held => held.src.includes(`pane=${pane}`));
  const {sent, ...world} = JSON.parse(JSON.stringify(frame.contentWindow.eval("engine")));
  return world;
}
// A toy engine's world: everything but the record of what its page sent and
// how slow the test made it.
function worldOf(held) {
  const {sent, wait, ...world} = JSON.parse(JSON.stringify(held));
  return world;
}
// The seat's line, as the engine keeps it: every kept turn up to the open
// one, and the turns ahead for as long as the Civ AI seated played them.
function engineLine() {
  const kept = engine.marks;
  let held = kept.filter(turn => turn <= engine.turn).length;
  const others = turn => engine.seat !== null && !!engine.by[turn] && engine.by[turn] !== engine.seat;
  while (held < kept.length && !others(kept[held])) held += 1;
  return kept.slice(0, held);
}
function engineState(error = null) {
  const marks = engineLine(), kept = engine.marks;
  return {error, turn: engine.turn, seed: 7, current: engine.current, units: [], cities: [],
    ...(engine.limit ? {turn_limit: engine.limit} : {}),
    players: [{id: 0, civ: "rome", score: engine.score, cities: 1, population: 1, military: 1,
      yields: {science: 1, culture: 1, gold: 1, faith: 0}},
      {id: 1, civ: "greece"}, {id: 2, civ: "barbarians", is_barbarian: true}],
    debug: {turn: engine.turn, turns: [...marks], first: marks[0], last: marks[marks.length - 1],
      seat: engine.seat, beyond: marks.length < kept.length
        ? {from: kept[marks.length], to: kept[kept.length - 1], by: engine.by[kept[marks.length]] || null} : null,
      acted: engine.acted, can_undo: engine.acted > 0 && engine.current === 0,
      can_back: engine.acted > 0 || engine.turn > marks[0], can_forward: engine.turn < marks[marks.length - 1],
      breakpoints: {turns: [...engine.breaks], ...JSON.parse(JSON.stringify(engine.watch)),
        known_categories: ["War", "Wonder"], known_tests: [">", "changes"]},
      layers: [["research", "Research"], ["cities", "Cities"]].map(([id, label]) =>
        ({id, label, on: engine.layers.includes(id)})), assisted: 0, stopped: engine.stopped,
      stepped: engine.stepped, reveal: engine.reveal, playing: engine.current, memory_line: engine.line,
      eras: engine.eras.filter(era => era.turn <= marks[marks.length - 1]),
      saves: engine.saves.map(save => ({...save}))}};
}
async function fetchJSON(path, init = {}) {
  const body = init.body ? JSON.parse(init.body) : null;
  engine.sent.push({path, body});
  if (path === "/saves") return {saves: []};
  // A half reads the world it holds after its twin was played for both.
  if (path === "/state") return engineState();
  if (path !== "/debug") return {error: null};
  // A slowed engine, so that a run can be seen on its way.
  if (engine.wait) await new Promise(done => setTimeout(done, engine.wait));
  // A request that names the other half as its twin. The real engine knows
  // from its own record whether the two are the same world; the toy reads
  // it off the two worlds as they stand before the request.
  const twin = body.twin ? [...window.parent.document.querySelectorAll("iframe")]
    .find(held => held.src.includes(`pane=${body.twin}`)).contentWindow.eval("engine") : null;
  const alike = twin !== null && JSON.stringify(worldOf(engine)) === JSON.stringify(worldOf(twin));
  engine.stopped = null;
  engine.stepped = null;
  // A request that names a Civ AI seats it, whatever else it asks.
  if (body.strategy) engine.seat = body.strategy;
  // A turn that opens is whoever's play opened it, and the turns that were
  // ahead of it are gone.
  const played = by => {
    engine.marks = engine.marks.filter(turn => turn < engine.turn).concat(engine.turn);
    engine.by = Object.fromEntries(Object.entries(engine.by).filter(([turn]) => +turn < engine.turn));
    if (by) engine.by[engine.turn] = by;
  };
  // The round comes back to the played seat, and that is the next turn.
  const opens = () => {
    engine.current = 0;
    engine.turn += 1;
    played(null);
  };
  // Anything that plays, in the middle of a round, lets the round finish first.
  if (engine.current !== 0 && (body.op === "step" || body.op === "finish_round")) opens();
  const marks = engineLine(), at = marks.indexOf(engine.turn);
  const others = engine.marks[marks.length];
  let error = null;
  if (body.op === "step_seat") {
    const seat = engine.current, next = (seat + 1) % SEATS;
    if (next === 0) opens(); else engine.current = next;
    engine.stepped = {seat, actions: 2 * seat, next};
  } else if (body.op === "step" && engine.turn >= 400) {
    // A run nobody stopped must still end.
    error = "this game has no turn left to play";
  } else if (body.op === "step") {
    engine.turn += 1;
    // Which Civ AI plays the seat is the one thing that tells two halves apart.
    engine.score += body.strategy === "stock" ? 2 : 1;
    played(engine.seat);
    // A turn played again is played into whatever era the game was in.
    engine.eras = engine.eras.filter(era => era.turn < engine.turn);
    const dawn = engine.turn === engine.eraAt;
    if (dawn) engine.eras.push({era: engine.eras.length, name: ERAS[engine.eras.length], turn: engine.turn});
    if (engine.breaks.includes(engine.turn)) engine.stopped = {kind: "turn", turn: engine.turn};
    else if (dawn && engine.watch.eras) engine.stopped = {kind: "era", turn: engine.turn, era: engine.eras.length - 1};
  } else if (body.op === "jump") {
    const reached = marks.filter(turn => turn <= body.turn).pop();
    if (others !== undefined && others <= body.turn) error = `turn ${others} was played by another Civ AI`;
    else if (reached === undefined) error = "before this timeline begins"; else engine.turn = reached;
  } else if (body.op === "back") {
    if (at > 0) engine.turn = marks[at - 1]; else error = "this is where the timeline begins";
  } else if (body.op === "forward") {
    if (at < marks.length - 1) engine.turn = marks[at + 1];
    else error = others === undefined ? "nothing has been played past this turn" : `turn ${others} was played by another Civ AI`;
  } else if (body.op === "save_point") {
    engine.ids += 1;
    engine.saves.push({id: engine.ids, name: body.name || `Turn ${engine.turn}`, auto: false, turn: engine.turn, acted: 0});
  } else if (body.op === "goto_save") {
    const save = engine.saves.find(held => held.id === body.id);
    if (save) engine.turn = save.turn; else error = "no save point has that id";
  } else if (body.op === "drop_save") {
    engine.saves = engine.saves.filter(held => held.id !== body.id);
  } else if (body.op === "breakpoints") {
    // Whatever a request leaves out is cleared, as the engine does it.
    engine.breaks = body.turns || [];
    engine.watch = {categories: body.categories || [], phrases: body.phrases || [], important: !!body.important,
      every_seat: !!body.every_seat, conditions: body.conditions || [], eras: !!body.eras};
  } else if (body.op === "layers") {
    engine.layers = body.layers || [];
  } else if (body.op === "reveal") {
    engine.reveal = body.on === true;
  } else if (body.op === "undo") {
    if (engine.acted > 0) engine.acted -= 1; else error = "nothing has been done this turn";
  } else if (body.op === "room") {
    engine.line = body.megabytes;
  } else if (body.op === "copy") {
    Object.assign(engine, engineOf(window.parent.document.querySelectorAll("iframe"), body.from));
  } else if (body.op === "adopt") {
    Object.assign(engine, engineOf(document.querySelectorAll("iframe"), body.pane));
  }
  const answer = engineState(error);
  // Played once for both: the twin is made a copy of what this half holds.
  if (twin !== null) {
    if (alike) Object.assign(twin, worldOf(engine));
    answer.debug.twinned = alike;
  }
  return answer;
}

// ---- the bar's script, and a half of a split
// A half starts on its host's world. The real page fetches that through the
// host's engine; here the half takes the toy engine as the host's stands.
const pane = window.parent !== window && new URLSearchParams(location.search).get("pane");
const barScript = document.createElement("script");
barScript.src = "app_debugger.js";
if (!pane) document.body.appendChild(barScript);
else {
  // A half of a split has its left panel folded away, as the viewer's has.
  document.getElementById("side").style.display = "none";
  const {sent, ...world} = JSON.parse(JSON.stringify(window.parent.eval("engine")));
  Object.assign(engine, world);
  if (window.parent.eval("typeof lateScript !== 'undefined' && lateScript")) {
    // The first state is on the table before the bar's script has arrived.
    state = engineState();
    setTimeout(() => document.body.appendChild(barScript), 300);
  } else {
    barScript.onload = () => render(engineState());
    document.body.appendChild(barScript);
  }
}
</script>
</body></html>
"""


class Quiet(http.server.SimpleHTTPRequestHandler):
    def log_message(self, fmt, *args):
        pass


class Bar:
    """The bar on the stand-in page, driven the way a hand would drive it."""

    def __init__(self, dev: Devtools):
        self.dev = dev

    def ev(self, expression: str):
        return self.dev.evaluate(expression)

    def until(self, expression: str, why: str, seconds: float = 20.0):
        deadline = time.time() + seconds
        while time.time() < deadline:
            if self.ev(expression):
                return
            time.sleep(0.05)
        # Said without leaning on the page: a page that never loaded has no
        # engine to ask, and that is exactly when this is read.
        seen = self.ev("JSON.stringify({href: location.href, ready: document.readyState,"
                       " scripts: document.scripts.length, bar: typeof drawDebugBar,"
                       " engine: typeof engine === 'undefined' ? null : engine.sent.slice(-4),"
                       " note: (document.getElementById('dbg-note') || {}).textContent})")
        raise AssertionError(f"{why}: never true: {expression}; page {seen}; console {self.dev.console[-4:]}")

    def idle(self, expression: str, why: str):
        self.until(f"!debugBusy && !debugRunning && !autoplaying && ({expression})", why)

    def sent(self) -> list:
        return self.ev("engine.sent.filter(call => call.path === '/debug').map(call => call.body)")

    def forget(self) -> None:
        self.ev("engine.sent.length = 0, true")

    def note(self) -> str:
        return self.ev("document.getElementById('dbg-note').textContent")

    def click(self, selector: str) -> None:
        found = self.ev(f"(() => {{ const el = document.querySelector({json.dumps(selector)});"
                        " if (!el) return 'missing'; if (el.disabled) return 'disabled'; el.click(); return 'ok'; })()")
        assert found == "ok", f"{selector}: {found}"

    def drag(self, slider: str, turn: int) -> None:
        """Move a range input and let go of it: `input`, then `change`."""
        self.ev(f"(() => {{ const el = document.getElementById({json.dumps(slider)}); el.value = {turn};"
                " el.dispatchEvent(new Event('input', {bubbles: true}));"
                " el.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")

    def type_turn(self, turn: int) -> None:
        self.ev(f"(() => {{ const box = document.getElementById('dbg-turn'); box.focus(); box.value = '{turn}';"
                " box.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")

    def wheel(self, selector: str, delta: int) -> None:
        self.ev(f"(() => {{ document.querySelector({json.dumps(selector)}).dispatchEvent("
                f"new WheelEvent('wheel', {{deltaY: {delta}, bubbles: true, cancelable: true}})); return true; }})()")


LANE = '[data-dbg-lane=""]'


# Whether a select is as wide as it would be left to itself: its longest
# option, read whole without opening it.
PICKER_FITS = ("(select => { const twin = select.cloneNode(true); twin.removeAttribute('id');"
               " twin.style.cssText = 'width:auto;max-width:none;flex:none;position:absolute;visibility:hidden';"
               " select.parentNode.appendChild(twin); const need = twin.offsetWidth; twin.remove();"
               " return select.offsetWidth >= need; })")

def check_the_bar(bar: Bar) -> None:
    bar.ev("render(engineState()), true")
    assert bar.ev("!document.getElementById('debugbar').hidden"), "the bar is hidden on a played game"
    assert bar.ev("document.getElementById('dbg-turn').value") == "1"

    # The header box stands at the top of the window beside the left panel:
    # it begins at the panel's right side and runs to the window's right
    # edge, the panel keeps the window's whole height, and the map area
    # stands under the box.
    left, top, right, height, window = bar.ev(
        "(() => { const box = document.getElementById('debugbar').getBoundingClientRect();"
        " return [box.left, box.top, box.right, Math.ceil(box.height), document.documentElement.clientWidth]; })()")
    assert window == bar.ev("innerWidth"), "the stand-in page scrolls, and the viewer never does"
    side = bar.ev("(() => { const box = document.getElementById('side').getBoundingClientRect();"
                  " return [box.left, box.top, box.right, box.height]; })()")
    assert side == [0, 0, 200, bar.ev("innerHeight")], side
    assert (left, top, right) == (200, 0, window), (left, top, right, window)
    assert bar.ev("getComputedStyle(document.getElementById('debugbar')).position") == "fixed"
    assert bar.ev("getComputedStyle(document.body).paddingTop") == "0px"
    assert bar.ev("getComputedStyle(document.getElementById('maparea')).marginTop") == f"{height}px"
    assert bar.ev("(() => { const box = document.getElementById('maparea').getBoundingClientRect();"
                  " return [Math.round(box.left), Math.round(box.top)]; })()") == [200, height]
    # Auto-play is in the box, and the left panel's own copy is put away.
    assert bar.ev("getComputedStyle(document.getElementById('humanfooter')).display") == "none"
    # The timeline is the top of the box and twice the height it was. Under
    # it the open turn stands in the middle, under the middle of the
    # timeline, with the turn buttons either side of it in their order:
    # turn 1, era start, ten back, one back; one on, ten on. Auto-play (the
    # Civ AI, the turns, the button), layers, breakpoints and Branch Timeline
    # are at the left end in that order, before the turn buttons.
    box = ("(name => { const at = document.querySelector(name).getBoundingClientRect();"
           " return [at.left, at.top, at.right, at.bottom]; })")
    rail = bar.ev(f"{box}('{LANE} .dbg-rail')")
    deck = bar.ev(f"{box}('#debugbar .dbg-deck')")
    assert rail[3] - rail[1] >= 64 and rail[3] <= deck[1], (rail, deck)
    assert rail[0] > left and rail[2] < window, (rail, left, window)
    middle = (rail[0] + rail[2]) / 2
    turn_box = bar.ev(f"{box}('#dbg-turn')")
    assert abs((turn_box[0] + turn_box[2]) / 2 - middle) < 40, (turn_box, middle)
    order = ["#dbg-agent", "#dbg-turns", "[data-dbg=autoplay]", "[data-dbg-pop=layers]", "[data-dbg-pop=breaks]",
             "[data-dbg=split]", "[data-dbg=first]", "[data-dbg=era]", "[data-dbg=back10]", "[data-dbg=back]",
             "#dbg-turn", "[data-dbg=next]", "[data-dbg=next10]"]
    assert bar.ev("(names => { const all = [...document.querySelectorAll('#debugbar .dbg-deck *')];"
                  " const at = names.map(name => all.indexOf(document.querySelector('#debugbar .dbg-deck ' + name)));"
                  " return at.every((place, index) => place >= 0 && (index === 0 || place > at[index - 1])); })"
                  f"({json.dumps(order)})"), "the box's controls are not in their order"
    along = [bar.ev(f"{box}('#debugbar {name}')") for name in order[6:]]
    assert all(before[2] <= after[0] for before, after in zip(along, along[1:])), along
    assert along[3][2] <= middle <= along[5][0], "one back and one on are not either side of the middle"
    assert bar.ev(f"{box}('#debugbar [data-dbg=split]')")[2] <= along[0][0] or \
        bar.ev(f"{box}('#debugbar [data-dbg=split]')")[1] > along[0][1], "Branch Timeline is not left of the turn buttons"
    assert bar.ev("document.querySelector('#debugbar [data-dbg=split]').textContent") == "⑂Branch Timeline"
    # Branch Timeline asks nothing first: it is a button, and has no panel.
    assert bar.ev("!document.getElementById('dbg-pop-split') && !document.querySelector('[data-dbg-pop=split]')")
    # The timeline is the whole game, 250 turns when the game does not say:
    # turn 1 is named at its left end and the last turn at its right, with the
    # round turns between them on the ruler.
    assert bar.ev("(() => { const s = document.getElementById('dbg-slider'); return [s.min, s.max, s.disabled]; })()") \
        == ["1", "250", False]
    assert bar.ev("document.getElementById('dbg-slider').getBoundingClientRect().width") > rail[2] - rail[0] - 40
    assert bar.ev("document.getElementById('dbg-limit').textContent") == ""
    ends = ("(() => { const lane = document.querySelector('" + LANE + "'), flag = lane.querySelector('.dbg-flag');"
            " const start = lane.querySelector('.dbg-start'), rail = lane.querySelector('.dbg-rail').getBoundingClientRect();"
            " return [start.textContent, lane.querySelector('.dbg-finish').textContent,"
            " getComputedStyle(flag).display !== 'none', lane.querySelector('.dbg-ruler').textContent,"
            " start.getBoundingClientRect().right <= rail.left"
            " && lane.querySelector('.dbg-finish').getBoundingClientRect().left >= rail.right,"
            " lane.querySelectorAll('.dbg-finish-line').length, document.getElementById('dbg-limit').textContent]; })()")
    assert bar.ev(ends) == ["1", "250", False, "50100150200", True, 0, ""], bar.ev(ends)
    # A game that says which turn is its last has a chequered flag there: at
    # the right end of the timeline, or, when the game is played on past that
    # turn, as a line across the rail where the turn is.
    bar.ev("engine.limit = 250, render(engineState()), true")
    assert bar.ev(ends) == ["1", "250", True, "50100150200", True, 0, "/ 250"], bar.ev(ends)
    bar.ev("engine.limit = 30, render(engineState()), true")
    assert bar.ev(ends) == ["1", "30", True, "1020", True, 0, "/ 30"], bar.ev(ends)
    bar.ev("engine.limit = null, render(engineState()), true")

    # The turn box, a turn still ahead: played to, one step a request.
    bar.type_turn(6)
    bar.idle("state.turn === 6", "the turn box plays to a turn ahead")
    assert [call["op"] for call in bar.sent()] == ["step"] * 5, bar.sent()
    assert bar.note() == "Played to turn 6", bar.note()
    assert bar.ev("state.debug.turns.length") == 6
    assert bar.ev("document.querySelector('#debugbar .dbg-turnbox').dataset.played") == "Turns 1 to 6 played, every one kept"
    # The marker is on the turn that is open, and it moves right as turns open.
    marker = (f"(() => {{ const pill = document.querySelector('{LANE} .dbg-cursor b').getBoundingClientRect();"
              " return [document.querySelector('" + LANE + " .dbg-cursor b').textContent, (pill.left + pill.right) / 2]; })()")
    at_six = bar.ev(marker)
    bar.ev("engine.limit = 4, render(engineState()), true")
    assert bar.ev(ends) == ["1", "50", False, "10203040", True, 1, "/ 4"], bar.ev(ends)
    bar.ev("engine.limit = null, render(engineState()), true")

    # Letting go of the slider goes to that turn. (The regression this file
    # was written for: the handler read the slider after the bar was redrawn.)
    bar.forget()
    bar.drag("dbg-slider", 3)
    bar.idle("state.turn === 3", "letting go of the slider jumps")
    assert bar.sent() == [{"op": "jump", "turn": 3}], bar.sent()
    assert bar.ev("document.getElementById('dbg-turn').value") == "3"
    assert bar.ev(f"document.querySelector('{LANE} .dbg-cursor b').textContent") == "3"

    # The turn box, a turn already played: put back, nothing played.
    bar.forget()
    bar.type_turn(5)
    bar.idle("state.turn === 5", "the turn box jumps to a played turn")
    assert bar.sent() == [{"op": "jump", "turn": 5}], bar.sent()
    assert bar.ev("document.getElementById('dbg-slider').value") == "5"

    # Letting go of the slider on a turn still ahead plays to it from the turn
    # that is open, and while it plays the timeline shows where it is headed.
    bar.forget()
    bar.ev("engine.wait = 40, true")
    bar.drag("dbg-slider", 9)
    bar.until(f"debugRunning && debugGoal === 9 && !document.querySelector('{LANE} .dbg-goal').hidden",
              "a run to a turn shows where it is headed")
    assert bar.ev("document.getElementById('dbg-slider').disabled"), "the slider waits for the run"
    bar.ev("engine.wait = 0, true")
    bar.idle("state.turn === 9", "the slider plays to a turn ahead")
    assert [call["op"] for call in bar.sent()] == ["step"] * 4, bar.sent()
    assert bar.note() == "Played to turn 9", bar.note()
    at_nine = bar.ev(marker)
    assert (at_six[0], at_nine[0]) == ("6", "9") and at_nine[1] > at_six[1], (at_six, at_nine)
    assert bar.ev(f"document.querySelector('{LANE} .dbg-goal').hidden"), "the goal goes with the run"

    # Under the pointer the timeline says what the turn there is, and what
    # letting go there would do.
    def hover(turn: int) -> str:
        bar.ev(f"(() => {{ const s = document.getElementById('dbg-slider'), box = s.getBoundingClientRect();"
               f" const x = box.left + 7 + ({turn} - s.min) / (s.max - s.min) * (box.width - 14);"
               " s.dispatchEvent(new PointerEvent('pointermove', {clientX: x, clientY: box.top + 4, bubbles: true}));"
               " return true; })()")
        assert bar.ev("!document.getElementById('dbg-hover').hidden"), "nothing is said under the pointer"
        return bar.ev("document.getElementById('dbg-hover').textContent")

    assert hover(4) == "Turn 4", hover(4)
    assert hover(9) == "Turn 9 · open now", hover(9)
    assert hover(40) == "Turn 40 · play to here", hover(40)
    bar.drag("dbg-slider", 2)
    bar.idle("state.turn === 2", "the slider goes back")
    assert hover(40) == "Turn 40 · play on from turn 2 to here", hover(40)
    bar.ev("document.getElementById('debugbar').dispatchEvent(new PointerEvent('pointerleave')), true")
    assert bar.ev("document.getElementById('dbg-hover').hidden"), "the pointer left and the words stayed"

    # The wheel over the timeline walks one played turn each way.
    bar.forget()
    bar.wheel("#dbg-slider", 100)
    bar.idle("state.turn === 3", "the wheel walks forward")
    bar.wheel(f"{LANE} .dbg-rail", -100)
    bar.idle("state.turn === 2", "the wheel walks back")
    assert [call["op"] for call in bar.sent()] == ["forward", "back"], bar.sent()

    # The turn buttons. They walk the turns already played, and past the last
    # of them the Civ AI plays on: ten on from turn 3, with turns 1 to 9
    # played, walks to turn 9 and plays four more.
    sent = lambda: [(call["op"], call.get("turn")) for call in bar.sent()]
    off = lambda name: bar.ev(f"document.querySelector('[data-dbg=\"{name}\"]').disabled")
    bar.forget()
    bar.click('[data-dbg="next"]')
    bar.idle("state.turn === 3", "next turn walks to a turn already played")
    assert sent() == [("forward", None)], sent()
    bar.forget()
    bar.click('[data-dbg="next10"]')
    bar.idle("state.turn === 13", "ten turns on walks to the last turn played and plays the rest")
    assert sent() == [("jump", 9)] + [("step", None)] * 4, sent()
    assert bar.note() == "Played to turn 13", bar.note()
    # What the box has to say stands inside the box, between the turn buttons
    # and the tools at the right, and not over the page beneath.
    said = bar.ev("(() => { const box = name => document.querySelector(name).getBoundingClientRect();"
                  " const note = box('#dbg-note'), whole = box('#debugbar'), on = box('#debugbar [data-dbg=next10]'),"
                  " tools = box('#debugbar .dbg-right .dbg-cluster');"
                  " return [note.width > 60, note.top >= whole.top && note.bottom <= whole.bottom,"
                  " note.left >= on.right, note.right <= tools.left]; })()")
    assert said == [True] * 4, said
    bar.forget()
    bar.click('[data-dbg="next"]')
    bar.idle("state.turn === 14", "next turn, past the last turn played, is one turn of the Civ AI")
    assert bar.sent() == [{"op": "step", "strategy": "jack"}], bar.sent()
    bar.forget()
    bar.click('[data-dbg="back10"]')
    bar.idle("state.turn === 4", "ten turns back")
    bar.click('[data-dbg="back"]')
    bar.idle("state.turn === 3", "one turn back")
    bar.click('[data-dbg="first"]')
    bar.idle("state.turn === 1", "Turn 1 goes to where the timeline begins")
    assert sent() == [("jump", 4), ("back", None), ("jump", 1)], sent()
    assert off("first") and off("back10") and off("back") and off("era"), "there is nothing before turn 1 to go back to"
    assert not off("next") and not off("next10")
    # Ten back from a turn nearer the start than that goes to the start.
    bar.type_turn(6)
    bar.idle("state.turn === 6", "the turn box goes to turn 6")
    bar.forget()
    bar.click('[data-dbg="back10"]')
    bar.idle("state.turn === 1", "ten back stops where the timeline begins")
    assert sent() == [("jump", 1)], sent()

    # The eras the game has been through are named on the timeline at the
    # turn each began. A name is clicked to go there, and Era start goes back
    # to the first turn of the era that is open, then of the era before.
    eras = f"[...document.querySelectorAll('{LANE} .dbg-marks u b')].map(name => name.textContent)"
    assert bar.ev(eras) == ["Ancient"], bar.ev(eras)
    bar.ev("engine.eras.push({era: 1, name: 'classical', turn: 5}), render(engineState()), true")
    assert bar.ev(eras) == ["Ancient", "Classical"], bar.ev(eras)
    places = bar.ev(f"[...document.querySelectorAll('{LANE} .dbg-marks u')].map(line => line.getBoundingClientRect().left)")
    assert places[0] < places[1] < bar.ev(marker)[1] + 400, places
    bar.forget()
    bar.click(f'{LANE} .dbg-marks u b[data-dbg-turn="5"]')
    bar.idle("state.turn === 5", "an era's name goes to the turn it began")
    # The marker is on the era's first turn now, and stands over its line.
    assert bar.ev(f"(() => {{ const pill = document.querySelector('{LANE} .dbg-cursor b').getBoundingClientRect();"
                  " const over = document.elementsFromPoint((pill.left + pill.right) / 2, (pill.top + pill.bottom) / 2);"
                  " const line = over.findIndex(found => found.matches('.dbg-marks u')),"
                  " mark = over.findIndex(found => found.matches('.dbg-cursor b'));"
                  " return getComputedStyle(document.querySelector('" + LANE + " .dbg-cursor')).zIndex === '3'"
                  " && (line < 0 || mark < 0 || mark < line); })()"), "an era's line is drawn over the marker"
    assert off("era") is False and "Ancient" in bar.ev("document.querySelector('[data-dbg=\"era\"]').title")
    bar.type_turn(9)
    bar.idle("state.turn === 9", "the turn box goes to turn 9")
    assert "turn 5" in bar.ev("document.querySelector('[data-dbg=\"era\"]').title")
    bar.forget()
    bar.click('[data-dbg="era"]')
    bar.idle("state.turn === 5", "Era start goes to the first turn of the era that is open")
    bar.click('[data-dbg="era"]')
    bar.idle("state.turn === 1", "and from there to the first turn of the era before")
    assert sent() == [("jump", 5), ("jump", 1)], sent()
    assert off("era")
    # Back to nine turns played and turn 2 open, for what follows.
    bar.ev("engine.eras.length = 1, engine.marks = [1, 2, 3, 4, 5, 6, 7, 8, 9], engine.turn = 2, render(engineState()), true")

    # A save point: set by name, flagged on the timeline, gone to by its flag,
    # and let go from the list. Its panel opens under its own button.
    flags = f"{LANE} .dbg-marks i[data-dbg-goto]"
    bar.forget()
    bar.click('[data-dbg-pop="saves"]')
    placed = bar.ev("(() => { const pop = document.getElementById('dbg-pop-saves').getBoundingClientRect(),"
                    " button = document.querySelector('[data-dbg-pop=\"saves\"]').getBoundingClientRect();"
                    " return [pop.left >= 0, pop.right <= innerWidth, pop.top >= button.bottom, pop.left <= button.left]; })()")
    assert placed == [True] * 4, placed
    bar.ev("document.querySelector('#dbg-pop-saves [data-dbg-field=pointname]').value = 'here', true")
    bar.click('[data-dbg="point"]')
    bar.idle("state.debug.saves.length === 1", "a save point is set")
    assert bar.sent()[-1] == {"op": "save_point", "name": "here"}, bar.sent()
    assert bar.ev("document.getElementById('dbg-point-count').textContent") == "1"
    assert bar.ev(f"document.querySelectorAll('{flags}').length") == 1
    assert hover(2) == "Turn 2 · open now · ⚑ here", hover(2)
    bar.drag("dbg-slider", 6)
    bar.idle("state.turn === 6", "the slider still jumps with a panel open")
    bar.click(flags)
    bar.idle("state.turn === 2", "a flag goes to its save point")
    assert bar.sent()[-1] == {"op": "goto_save", "id": 1}, bar.sent()
    bar.click("#dbg-point-list [data-dbg-drop]")
    bar.idle("state.debug.saves.length === 0", "a save point is let go")
    assert bar.sent()[-1] == {"op": "drop_save", "id": 1}, bar.sent()
    # Escape closes the panel.
    bar.ev("document.querySelector('#dbg-pop-saves [data-dbg-field=pointname]').dispatchEvent("
           "new KeyboardEvent('keydown', {key: 'Escape', bubbles: true})), true")
    assert bar.ev("document.getElementById('dbg-pop-saves').hidden"), "Escape closes a panel"

    # A breakpoint on a turn ahead stands on the timeline as a mark: picked,
    # it is played to from turn 2, and the run stops because the engine said so.
    bar.ev("engine.breaks = [12], render(engineState()), true")
    bar.forget()
    bar.click(f'{LANE} .dbg-marks i.stop[data-dbg-turn="12"]')
    bar.idle("state.turn === 12", "a breakpoint ahead is played to")
    assert [call["op"] for call in bar.sent()] == ["step"] * 10, bar.sent()
    assert bar.note() == "Stopped at turn 12", bar.note()
    bar.ev("engine.breaks = [], render(engineState()), true")

    # A long game does not keep every turn. The timeline shows which it still
    # has, and says where a turn that is gone would land.
    kept = bar.ev("engine.marks.slice()")
    bar.ev("engine.marks = [1, 4, 8, 12], render(engineState()), true")
    assert bar.ev(f"document.querySelector('{LANE}').classList.contains('dbg-thinned')")
    assert bar.ev(f"document.querySelectorAll('{LANE} .dbg-kept-turns i').length") == 4
    assert bar.ev("document.querySelector('#debugbar .dbg-turnbox').dataset.played") == "Turns 1 to 12 played, 4 kept"
    assert hover(6) == "Turn 6 · no longer kept, goes to turn 4", hover(6)
    bar.forget()
    bar.drag("dbg-slider", 6)
    bar.idle("state.turn === 4", "a turn no longer kept answers with the nearest before it")
    assert bar.note() == "Turn 6 is no longer kept. This is turn 4, the nearest before it", bar.note()
    bar.ev(f"engine.marks = {json.dumps(kept)}, engine.turn = 9, render(engineState()), true")
    assert not bar.ev(f"document.querySelector('{LANE}').classList.contains('dbg-thinned')")

    # The Civ AI that plays the seat is picked beside Auto-play, from what the
    # page's own picker lists, and a turn it plays carries it.
    agent = "document.getElementById('dbg-agent')"
    assert bar.ev(f"[...{agent}.options].map(option => option.value)") == ["jack", "stock"]
    assert bar.ev(f"{agent}.value") == "jack"
    # It lists them as the page does, Elo then name, and is wide enough to
    # read the longest of them whole without being opened.
    assert bar.ev(f"[...{agent}.options].map(option => option.textContent)") == ["1211 - JackOfAllTrades", "1000 - ByTheBook"]
    assert bar.ev(f"{PICKER_FITS}({agent})"), "the Civ AI picker cuts its longest listing short"

    def seat(name: str) -> None:
        bar.ev(f"(() => {{ const select = {agent}; select.value = '{name}';"
               " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
        assert bar.ev("document.getElementById('autoplaystrategy').value") == name

    last = bar.ev("Math.max(...state.debug.turns)")
    bar.type_turn(last)
    bar.idle(f"state.turn === {last}", "the turn box goes to the last turn played")
    seat("stock")
    bar.forget()
    bar.click('[data-dbg="next"]')
    bar.idle(f"state.turn === {last + 1}", "a turn with the Civ AI picked in the box")
    assert bar.sent() == [{"op": "step", "strategy": "stock"}], bar.sent()
    seat("jack")

    # Auto-play plays the number of turns picked, with the Civ AI picked, and
    # while it plays the button counts them and is its own Stop.
    turns = "document.getElementById('dbg-turns')"
    label = lambda: [bar.ev(f"document.getElementById('dbg-run-{part}').textContent") for part in ("sign", "word", "count")]
    assert bar.ev(f"[...{turns}.options].map(option => option.value).join()") \
        == "1,2,3,4,5,10,20,30,40,50,100,150,200,250,all"
    assert bar.ev(f"{turns}.value") == "10" and label() == ["▶▶", "Auto-play", ""], label()
    bar.ev(f"{turns}.value = '1', true")
    bar.forget()
    bar.click('[data-dbg="autoplay"]')
    bar.idle(f"state.turn === {last + 2}", "auto-play plays one turn")
    assert bar.sent() == [{"op": "step", "strategy": "jack"}], bar.sent()
    bar.ev(f"{turns}.value = '5', engine.wait = 60, true")
    bar.forget()
    bar.ev("drawn.length = 0, priced.length = 0, true")
    bar.click('[data-dbg="autoplay"]')
    bar.until("debugRunning && debugPlayed >= 2", "auto-play is under way")
    sign, word, count = label()
    assert (sign, word) == ("⏹", "Stop") and count.endswith("/5") and 2 <= int(count[:-2]) <= 5, label()
    assert bar.ev(f"{turns}.disabled && document.getElementById('dbg-agent').disabled"), "the picks are held while it plays"
    assert bar.ev("document.querySelector('[data-dbg=\"autoplay\"]').classList.contains('on')")
    bar.ev("engine.wait = 0, true")
    bar.idle(f"state.turn === {last + 7}", "auto-play plays the five turns picked")
    assert [call["op"] for call in bar.sent()] == ["step"] * 5, bar.sent()
    # Each turn but the last is drawn with the next already asked for: the
    # engine plays while the page draws, and never plays a turn past the count.
    assert bar.ev("drawn.join()") == "2,3,4,5,5", bar.ev("drawn.join()")
    # The policy deck is priced once, for the turn the run ended on, when
    # nothing is being played any more: not for each turn it passed through.
    assert bar.ev("priced.join()") == "false", bar.ev("priced.join()")
    assert bar.note() == f"Played to turn {last + 7}", bar.note()
    assert label() == ["▶▶", "Auto-play", ""], label()
    bar.ev(f"{turns}.value = '1', true")
    bar.click('[data-dbg="back"]')
    bar.idle(f"state.turn === {last + 6}", "one turn back")

    # A box with room has words on its tools; a narrow one keeps the signs,
    # and the map area still stands under whatever height the box comes to.
    # How wide "room" is depends on the type the browser has, so the two
    # windows here are far to either side of it.
    word = "getComputedStyle(document.querySelector('#debugbar .dbg-word')).display"
    tight = "document.getElementById('debugbar').classList.contains('dbg-tight')"
    under = ("getComputedStyle(document.getElementById('maparea')).marginTop ==="
             " Math.ceil(document.getElementById('debugbar').getBoundingClientRect().height) + 'px'")
    deck = "document.querySelector('#debugbar .dbg-deck').offsetHeight"

    def window_of(width: int) -> None:
        bar.dev.call("Emulation.setDeviceMetricsOverride", {"width": width, "height": 900, "deviceScaleFactor": 1, "mobile": False})
        bar.until(f"document.documentElement.clientWidth === {width} && {under}", f"the box in a window {width} wide")

    window_of(3000)
    bar.until(f"!{tight}", "a box with room for the words went without them")
    assert bar.ev(word) != "none" and bar.ev(deck) < 70, (bar.ev(word), bar.ev(deck))
    assert bar.ev("document.getElementById('debugbar').classList.contains('dbg-roomy')")
    window_of(520)
    bar.until(tight, "a narrow box kept the words on its tools")
    assert bar.ev(word) == "none"
    assert bar.ev(under), "the map area does not stand under a narrow box"
    # A box with no room for the tools beside the turn buttons stacks them:
    # the turn buttons first, and the tools under them.
    assert bar.ev("document.getElementById('debugbar').classList.contains('dbg-stacked')")
    assert bar.ev("document.querySelector('#debugbar .dbg-turnbox').getBoundingClientRect().bottom"
                  " <= document.querySelector('#debugbar .dbg-cluster').getBoundingClientRect().top"), \
        "the turn buttons are not over the tools in a narrow window"
    # A stacked box has no room for what it says, and says it under itself.
    bar.ev("debugSay('A word from the box'), true")
    assert bar.ev("document.getElementById('dbg-note').getBoundingClientRect().top"
                  " >= document.getElementById('debugbar').getBoundingClientRect().bottom"), "a stacked box's note is not under it"
    bar.ev("debugSay(''), true")
    assert bar.ev("getComputedStyle(document.getElementById('dbg-note')).display") == "none"
    bar.dev.call("Emulation.clearDeviceMetricsOverride")
    bar.until(f"document.documentElement.clientWidth === {window} && {under}", "the window is its own size again")

    # The left panel is folded away and brought back without the window
    # changing size, and the box follows the map area's left edge.
    # Its tools are fitted to each width afresh: a box that had words
    # before the panel was folded has them again when the panel is back.
    edge = "Math.round(document.getElementById('debugbar').getBoundingClientRect().left)"
    fitted = "document.getElementById('debugbar').className"
    before = bar.ev(fitted)
    bar.ev("document.getElementById('side').style.display = 'none', true")
    bar.until(f"{edge} === 0 && {under}", "the box takes the room a folded panel leaves")
    bar.ev("document.getElementById('side').style.display = '', true")
    bar.until(f"{edge} === 200 && {under}", "the box starts at the panel's right side again")
    assert bar.ev(fitted) == before, (before, bar.ev(fitted))
    # The same through a width with room for the picker's own label.
    window_of(3000)
    bar.until("document.getElementById('debugbar').classList.contains('dbg-roomy')", "a wide box has the picker's label")
    bar.dev.call("Emulation.clearDeviceMetricsOverride")
    bar.until(f"document.documentElement.clientWidth === {window} && {under}", "the window is its own size again")
    assert bar.ev(fitted) == before, (before, bar.ev(fitted))

    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_breakpoints_and_a_run(bar: Bar) -> None:
    """Set breakpoints from the form, run to one, stop a run by hand."""
    form = "document.querySelector('#dbg-pop-breaks [data-dbg-field={}]')".format
    count = lambda: bar.ev("document.getElementById('dbg-break-count').textContent")

    def typed(field: str, text: str) -> dict:
        bar.forget()
        bar.ev(f"(() => {{ const box = {form(field)}; box.value = {json.dumps(text)};"
               " box.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
        bar.idle("true", f"the {field} field is sent")
        return bar.sent()[-1]

    bar.click('[data-dbg-pop="breaks"]')
    # Turns are whole numbers, each once; anything else in the field is dropped.
    request = typed("turns", "12, 15, twelve, 12")
    assert request == {"op": "breakpoints", "turns": [12, 15], "categories": [], "phrases": [],
                       "important": False, "every_seat": False, "eras": False, "conditions": []}, request
    assert count() == "2", count()
    request = typed("phrases", "declared war, captured")
    assert request["phrases"] == ["declared war", "captured"] and request["turns"] == [12, 15], request
    assert count() == "4", count()

    # The categories on offer are the engine's; ticking one sends the lot.
    assert bar.ev("[...document.querySelectorAll('#dbg-category-list [data-dbg-category]')].map(b => b.dataset.dbgCategory)") \
        == ["War", "Wonder"]
    bar.forget()
    bar.click('#dbg-category-list [data-dbg-category="Wonder"]')
    bar.idle("state.debug.breakpoints.categories.length === 1", "a category is ticked")
    assert bar.sent()[-1]["categories"] == ["Wonder"] and bar.sent()[-1]["turns"] == [12, 15], bar.sent()
    bar.click('#dbg-pop-breaks [data-dbg-field=important]')
    bar.idle("state.debug.breakpoints.important === true", "any important event is ticked")
    assert count() == "6", count()

    # A condition is composed in the form and held by the engine; its fields
    # being changed is not a request, adding and removing it are.
    bar.forget()
    bar.ev(f"(() => {{ {form('cond-seat')}.value = '1'; {form('cond-key')}.value = 'score'; {form('cond-test')}.value = '>';"
           f" {form('cond-value')}.value = '5'; {form('cond-test')}.dispatchEvent(new Event('change', {{bubbles: true}})); return true; }})()")
    assert bar.sent() == [], bar.sent()
    bar.click('[data-dbg="cond-add"]')
    bar.idle("state.debug.breakpoints.conditions.length === 1", "a condition is added")
    assert bar.sent()[-1]["conditions"] == [{"seat": 1, "key": "score", "test": ">", "value": 5}], bar.sent()
    assert "Greece · score > 5" in bar.ev("document.getElementById('dbg-condition-list').textContent")
    bar.click('[data-dbg-cond-remove="0"]')
    bar.idle("state.debug.breakpoints.conditions.length === 0", "a condition is removed")

    # +10 adds the turn ten from now to the turns already there.
    turn = bar.ev("state.turn")
    bar.forget()
    bar.click('[data-dbg="break-plus"]')
    bar.idle(f"state.debug.breakpoints.turns.includes({turn + 10})", "+10 adds a turn")
    assert bar.sent()[-1]["turns"] == [12, 15, turn + 10], bar.sent()

    # Auto-play, told to play every turn, plays until the engine says a
    # breakpoint stopped it.
    turns = "document.getElementById('dbg-turns')"
    request = typed("turns", str(turn + 3))
    assert request["turns"] == [turn + 3], request
    bar.ev(f"{turns}.value = 'all', true")
    bar.forget()
    bar.click('[data-dbg="autoplay"]')
    bar.idle(f"state.turn === {turn + 3}", "auto-play stops on its breakpoint")
    assert [call["op"] for call in bar.sent()] == ["step"] * 3, bar.sent()
    assert bar.note() == f"Stopped at turn {turn + 3}", bar.note()
    assert bar.ev("document.getElementById('dbg-run-sign').textContent") == "▶▶"

    # An era breakpoint: ticked in the form, it stops auto-play on the first
    # turn of a new era, and the timeline names the era there.
    bar.forget()
    bar.click('#dbg-pop-breaks [data-dbg-field=eras]')
    bar.idle("state.debug.breakpoints.eras === true", "the era breakpoint is ticked")
    assert bar.sent()[-1]["eras"] is True and bar.sent()[-1]["turns"] == [turn + 3], bar.sent()
    held = count()
    bar.ev(f"engine.eraAt = {turn + 6}, true")
    bar.forget()
    bar.click('[data-dbg="autoplay"]')
    bar.idle(f"state.turn === {turn + 6}", "auto-play stops where the era begins")
    assert [call["op"] for call in bar.sent()] == ["step"] * 3, bar.sent()
    assert bar.note() == f"Stopped on turn {turn + 6}: the Classical era began", bar.note()
    assert bar.ev(f"[...document.querySelectorAll('{LANE} .dbg-marks u b')].map(name => name.textContent + name.dataset.dbgTurn)") \
        == ["Ancient1", f"Classical{turn + 6}"]
    bar.click('#dbg-pop-breaks [data-dbg-field=eras]')
    bar.idle("state.debug.breakpoints.eras === false", "the era breakpoint is unticked")
    assert int(count()) == int(held) - 1, (count(), held)
    bar.ev("engine.eraAt = null, true")

    # Clear all sends a request with nothing in it, and the count goes.
    bar.forget()
    bar.click('[data-dbg="break-clear"]')
    bar.idle("state.debug.breakpoints.turns.length === 0 && !state.debug.breakpoints.important", "the breakpoints are cleared")
    assert bar.sent() == [{"op": "breakpoints"}], bar.sent()
    assert count() == "", count()
    bar.click('[data-dbg-pop="breaks"]')

    # With nothing to stop it auto-play goes on counting, the same button is
    # its Stop, and the box says how far it got.
    bar.click('[data-dbg="autoplay"]')
    bar.until(f"debugRunning && state.turn > {turn + 11}", "auto-play with no breakpoint keeps playing")
    assert bar.ev("document.getElementById('dbg-run-sign').textContent") == "⏹"
    assert bar.ev("document.getElementById('dbg-run-word').textContent") == "Stop"
    assert int(bar.ev("document.getElementById('dbg-run-count').textContent")) >= 5
    bar.click('[data-dbg="autoplay"]')
    bar.idle("true", "auto-play stops when told to")
    assert bar.note().startswith("Stopped by hand after "), bar.note()
    assert bar.ev("state.turn") < 400, "auto-play was stopped, not exhausted"
    bar.ev(f"{turns}.value = '1', true")


def check_layers_reveal_and_undo(bar: Bar) -> None:
    """The rest of the bar: who decides, the fog, and taking an action back."""
    sent = lambda: bar.sent()[-1]
    bar.click('[data-dbg-pop="layers"]')
    bar.click('#dbg-layer-list [data-dbg-layer="cities"]')
    bar.idle("state.debug.layers.find(layer => layer.id === 'cities').on", "a layer is switched on")
    assert sent() == {"op": "layers", "layers": ["cities"]}, sent()
    assert bar.ev("document.getElementById('dbg-layer-count').textContent") == "1"
    bar.click('[data-dbg="layers-all"]')
    bar.idle("state.debug.layers.every(layer => layer.on)", "Everything switches every layer on")
    assert sent() == {"op": "layers", "layers": ["research", "cities"]}, sent()
    bar.click('[data-dbg="assist"]')
    bar.idle("true", "the layers are applied to this turn")
    assert sent() == {"op": "assist", "strategy": "jack"}, sent()
    bar.click('[data-dbg="layers-none"]')
    bar.idle("state.debug.layers.every(layer => !layer.on)", "By hand switches every layer off")
    assert sent() == {"op": "layers", "layers": []}, sent()
    bar.click('[data-dbg-pop="layers"]')

    bar.click('[data-dbg="reveal"]')
    bar.idle("state.debug.reveal === true", "the fog is lifted")
    assert sent() == {"op": "reveal", "on": True}, sent()
    assert bar.ev("document.querySelector('[data-dbg=\"reveal\"]').classList.contains('on')")
    bar.click('[data-dbg="reveal"]')
    bar.idle("state.debug.reveal === false", "the fog is put back")
    assert sent() == {"op": "reveal", "on": False}, sent()

    # The Single player switch is the page's own: it sends the engine nothing,
    # paints the page again in the other view, and keeps the choice. Going to
    # Single player picks up a unit to order; going back puts the selection
    # and the seat's screens away.
    switch = "document.querySelector('[data-dbg=\"solo-ui\"]')"
    asked = len(bar.sent())
    assert bar.ev(f"SOLO_UI === false && !{switch}.classList.contains('on') && "
                  f"{switch}.getAttribute('aria-pressed') === 'false'"), "the debugger opens on the Debugger view"
    bar.ev("painted.length = 0, true")
    bar.click('[data-dbg="solo-ui"]')
    assert bar.ev(f"SOLO_UI === true && {switch}.classList.contains('on') && "
                  f"{switch}.getAttribute('aria-pressed') === 'true'"), "the switch is on"
    assert bar.ev("localStorage.getItem('civvis-debugger-view-v1')") == "single"
    painted = bar.ev("painted")
    assert "drawSoloHud" in painted and "advanceToNextActionUnit" in painted, painted
    assert "closeCityScreen" not in painted, painted
    bar.ev("sel = {id: 7}, selCity = {id: 1}, painted.length = 0, true")
    bar.click('[data-dbg="solo-ui"]')
    assert bar.ev(f"SOLO_UI === false && sel === null && selCity === null && !{switch}.classList.contains('on')")
    assert bar.ev("localStorage.getItem('civvis-debugger-view-v1')") == "debugger"
    painted = bar.ev("painted")
    assert "drawSoloHud" in painted and "closeCityScreen" in painted, painted
    assert "advanceToNextActionUnit" not in painted, painted
    assert len(bar.sent()) == asked, "the switch sent the engine something"

    # Undo, by its button and by the key the hand expects, and not when
    # there is nothing to take back.
    assert bar.ev("document.querySelector('[data-dbg=\"undo\"]').disabled")
    bar.ev("engine.acted = 2, render(engineState()), true")
    bar.forget()
    # The note stood on the action that is about to be taken back.
    bar.ev("debugSay('Civ AI: Move To · Settler · 2 more this turn', {x: 1, y: 1}), true")
    bar.click('[data-dbg="undo"]')
    bar.idle("state.debug.acted === 1", "the button takes back an action")
    bar.until("document.getElementById('dbg-note').textContent === 'Last action undone · 1 more can be undone'",
              "the note says an action was taken back, and no longer names it")
    bar.ev("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true, bubbles: true})), true")
    bar.idle("state.debug.acted === 0", "Ctrl+Z takes back an action")
    bar.until("document.getElementById('dbg-note').textContent === 'Last action undone · nothing left to undo'",
              "the note says there is nothing left to take back")
    assert bar.sent() == [{"op": "undo"}] * 2, bar.sent()
    bar.ev("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true, bubbles: true})), true")
    time.sleep(0.2)
    assert bar.sent() == [{"op": "undo"}] * 2, "Ctrl+Z with nothing to undo sends nothing"
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_stepping_a_seat(bar: Bar) -> None:
    """The round a civilization at a time, and what a turn means in the middle of one."""
    turn = bar.ev("state.turn")
    bar.forget()
    bar.click('[data-dbg="step-seat"]')
    bar.idle("state.current === 1", "the first press ends the seat's own turn")
    assert bar.note() == "Your turn ended · Greece plays next", bar.note()
    bar.click('[data-dbg="step-seat"]')
    bar.idle("state.current === 2", "the next press plays the rival")
    # Every seat is one civilization but the Barbarians, and the note agrees.
    assert bar.note() == "Greece played 2 actions · Barbarians play next", bar.note()
    bar.click('[data-dbg="step-seat"]')
    bar.idle(f"state.current === 0 && state.turn === {turn + 1}", "the last press brings the round back")
    assert bar.note() == f"Barbarians played 4 actions · turn {turn + 1} is yours", bar.note()
    assert bar.sent() == [{"op": "step_seat"}] * 3, bar.sent()

    # In the middle of a round the bar says whose turn it is for as long as
    # nothing newer is being said, an action by hand is not sent, and End
    # Turn lets the rest of the round play.
    bar.click('[data-dbg="step-seat"]')
    bar.idle("state.current === 1", "a round is in the middle")
    bar.ev("debugSay(''), true")
    assert bar.note() == "Greece plays next · ▸| steps it, 1▶ lets the round finish", bar.note()
    bar.forget()
    bar.ev("send({type: 'move'}), true")
    assert bar.note() == "Greece plays next; your turn comes back when the round finishes", bar.note()
    assert bar.sent() == [], bar.sent()
    bar.ev("send({type: 'end_turn'}), true")
    bar.idle(f"state.current === 0 && state.turn === {turn + 2}", "End Turn lets the round finish")
    assert bar.sent() == [{"op": "finish_round"}], bar.sent()
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_a_picked_civ_ai_plays_its_own_turns(bar: Bar) -> None:
    """Walked back and handed to another Civ AI, the seat goes forward by playing.

    Why this exists: on 2026-11-21 the operator walked a game back, picked
    another Civ AI and went on, and saw the turns recorded under the first
    one come back. The turn buttons, the turn box and the timeline all walk
    turns already played, and whose turns they were was not asked.
    """
    agent = "document.getElementById('dbg-agent')"
    beyond_shown = f"!document.querySelector('{LANE} .dbg-beyond').hidden"

    def seat(name: str) -> None:
        bar.forget()
        bar.ev(f"(() => {{ const select = {agent}; select.value = '{name}';"
               " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
        bar.idle(f"engine.seat === '{name}'", f"{name} is said to the engine when it is picked")
        # The pick is said at once, and nothing is played by saying it.
        assert bar.sent() == [{"op": "status", "strategy": name}], bar.sent()

    bar.type_turn(bar.ev("Math.max(...state.debug.turns)"))
    bar.idle("state.turn === state.debug.last && !state.debug.beyond", "the last turn played is open")
    seat("jack")
    assert bar.ev("localStorage.getItem(AUTOPLAY_STRATEGY_KEY)") == "jack", "the pick is the page's choice"
    start = bar.ev("state.turn")
    bar.ev("document.getElementById('dbg-turns').value = '5', true")
    bar.click('[data-dbg="autoplay"]')
    bar.idle(f"state.turn === {start + 5}", "Jack plays five turns")

    # Walked back under the Civ AI that played them, the turns ahead are this
    # timeline's: the turn button walks to the next one.
    bar.drag("dbg-slider", start + 1)
    bar.idle(f"state.turn === {start + 1}", "the timeline is walked back")
    assert bar.ev("[state.debug.last, state.debug.beyond]") == [start + 5, None]
    assert not bar.ev(beyond_shown)
    bar.forget()
    bar.click('[data-dbg="next"]')
    bar.idle(f"state.turn === {start + 2}", "a turn Jack played is walked to while Jack is picked")
    assert bar.sent() == [{"op": "forward"}], bar.sent()
    bar.click('[data-dbg="back"]')
    bar.idle(f"state.turn === {start + 1}", "one turn back")

    # Another Civ AI is picked there. The turns ahead are Jack's game: the
    # timeline's played part ends on the open turn, the kept turns past it
    # are drawn apart, and the box says whose they are.
    seat("stock")
    assert bar.ev("[state.debug.last, state.debug.can_forward, state.debug.beyond]") == \
        [start + 1, False, {"from": start + 2, "to": start + 5, "by": "jack"}]
    assert bar.ev(beyond_shown), "the turns another Civ AI played are not drawn on the lane"
    said = f"Stock plays on from here · turns {start + 2} to {start + 5} are by Jack"
    assert bar.note() == said, bar.note()
    words = bar.ev(f"debugTurnWords(debugLaneModels[''], {start + 3})")
    assert words == f"Turn {start + 3} · play to here, over the turns Jack played", words
    # The wheel walks played turns only, and there is none ahead to walk.
    bar.forget()
    bar.wheel(LANE, 100)
    time.sleep(0.2)
    assert bar.sent() == [] and bar.ev("state.turn") == start + 1, bar.sent()

    # Picked again before anything is played, Jack's turns are the timeline again.
    seat("jack")
    assert bar.ev("[state.debug.last, state.debug.beyond]") == [start + 5, None]
    assert not bar.ev(beyond_shown) and bar.note() == "", bar.note()
    seat("stock")

    # Every way forward now plays, with the Civ AI that is picked: the turn
    # button, ten at once, the turn box and the timeline itself.
    bar.forget()
    bar.click('[data-dbg="next"]')
    bar.idle(f"state.turn === {start + 2}", "1 on plays a turn with the Civ AI picked")
    assert bar.sent() == [{"op": "step", "strategy": "stock"}], bar.sent()
    assert bar.ev("[state.debug.last, state.debug.beyond]") == [start + 2, None], "Jack's turns are played over"
    assert not bar.ev(beyond_shown)

    def played_from(turn: int, forward, goal: int, why: str) -> None:
        """Jack plays `start` to `start + 5` again, the world is walked back, Stock is picked and `forward` is done."""
        seat("jack")
        bar.drag("dbg-slider", start)
        bar.idle(f"state.turn === {start}", "back to where Jack began")
        bar.type_turn(start + 5)
        bar.idle(f"state.turn === {start + 5}", "Jack plays its five turns again")
        assert bar.ev("Object.values(engine.by).slice(-5)") == ["jack"] * 5
        bar.drag("dbg-slider", turn)
        bar.idle(f"state.turn === {turn}", "the timeline is walked back")
        seat("stock")
        bar.forget()
        forward()
        bar.idle(f"state.turn === {goal}", why)
        sent = bar.sent()
        assert sent == [{"op": "step", "strategy": "stock"}] * (goal - turn), sent

    played_from(start + 1, lambda: bar.click('[data-dbg="next10"]'), start + 11, "10 on plays ten turns")
    played_from(start + 2, lambda: bar.type_turn(start + 4), start + 4, "the turn box plays to a turn Jack had played")
    played_from(start + 2, lambda: bar.drag("dbg-slider", start + 5), start + 5, "the timeline plays to a turn Jack had played")
    played_from(start + 1, lambda: bar.click('[data-dbg="autoplay"]'), start + 6, "Auto-play plays the turns picked")

    # The Civ AI picked before the walk back, and not after it, changes nothing:
    # the turns ahead are still another agent's.
    seat("jack")
    bar.drag("dbg-slider", start + 3)
    bar.idle(f"state.turn === {start + 3} && !!state.debug.beyond", "walked back onto turns Stock played")
    assert bar.ev("state.debug.beyond") == {"from": start + 4, "to": start + 6, "by": "stock"}
    bar.forget()
    bar.click('[data-dbg="next"]')
    bar.idle(f"state.turn === {start + 4}", "1 on plays")
    assert bar.sent() == [{"op": "step", "strategy": "jack"}], bar.sent()
    bar.ev("document.getElementById('dbg-turns').value = '1', true")


def check_the_split(bar: Bar) -> None:
    """Head to head: two halves, a Civ AI each, played to a turn and compared."""
    half = "debugSplit.panes.{}.frame.contentWindow.eval({})".format
    # What a half was asked to play or walk. Being told which Civ AI holds its
    # seat is asked of the engine too (`status`), and is looked at by itself.
    ops = lambda letter: bar.ev(half(letter, json.dumps(
        "engine.sent.filter(call => call.path === '/debug' && call.body.op !== 'status')"
        ".map(call => [call.body.op, call.body.strategy])")))
    seated = lambda letter: bar.ev(half(letter, json.dumps(
        "engine.sent.filter(call => call.path === '/debug' && call.body.op === 'status').map(call => call.body.strategy)")))
    reported = ("debugSplit && ['a', 'b'].every(letter => debugSplit.panes[letter].turn === {} "
                "&& debugSplit.panes[letter].numbers)").format

    start = bar.ev("state.turn")
    # Branch Timeline splits the screen at once: side by side, and locked.
    # Head to head is the strip's own button.
    bar.click('[data-dbg="split"]')
    bar.until(reported(start), "both halves say where they stand")
    assert bar.ev("[debugSplit.dir, debugSplit.locked]") == ["row", True]
    bar.click('[data-dbg-split="versus"]')
    assert bar.ev("debugSplit.versus && !debugSplit.host.querySelector('[data-dbg-versus]').hidden")

    # The host's bar stays up across the top, over the two halves, and its
    # timeline is two lanes, one for each half. Locked, as a split opens, the
    # host's deck under the lanes is the one panel for both halves, and each
    # half's own bar is put away.
    deck_shown = "getComputedStyle(document.querySelector('#debugbar .dbg-deck')).display !== 'none'"
    bar_shown = "'getComputedStyle(document.getElementById(\"debugbar\")).display !== \"none\"'"
    bar.until(f"debugBar.classList.contains('dbg-merged') && {deck_shown}", "the host's deck is the one panel of a locked split")
    bar.until("(() => { const top = document.getElementById('debugbar'), box = top.getBoundingClientRect();"
              " return !top.hidden && box.top === 0 && box.width === document.documentElement.clientWidth"
              " && Math.round(debugSplit.host.getBoundingClientRect().top) === Math.ceil(box.height); })()",
              "the split stands under the host's bar")
    shown = lambda lane: bar.ev(f"getComputedStyle(document.querySelector('[data-dbg-lane=\"{lane}\"]')).display") != "none"
    assert (shown(""), shown("a"), shown("b")) == (False, True, True)
    # Each lane has the width of the window to itself, between its two ends.
    wide = bar.ev("['a', 'b'].map(letter => document.getElementById('dbg-slider-' + letter).getBoundingClientRect().width)")
    assert min(wide) > bar.ev("innerWidth") * 0.6, wide
    assert bar.ev("['a', 'b'].map(letter => document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-start`).textContent"
                  " + ' ' + document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-finish`).textContent)") == ["1 250"] * 2
    assert [bar.ev(half(letter, bar_shown)) for letter in "ab"] == [False, False], "a locked half shows a bar of its own"
    lanes = "['a', 'b'].map(letter => +document.getElementById('dbg-slider-' + letter).value)"
    assert bar.ev(lanes) == [start, start], bar.ev(lanes)
    assert bar.ev("[...debugSplit.host.querySelector('[data-dbg-strategy=\"b\"]').options].map(o => o.value)") \
        == ["jack", "stock"], "the pickers offer what the page's own picker lists"

    # A Civ AI of its own for B, told to the half.
    bar.ev("(() => { const select = debugSplit.host.querySelector('[data-dbg-strategy=\"b\"]'); select.value = 'stock';"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    bar.until(half("b", "'debugStrategy()'") + " === 'stock'", "B is told which Civ AI plays its seat")
    assert bar.ev(half("a", "'debugStrategy()'")) == "jack"
    # The same choice stands at the head of each half's lane in the host's
    # bar, and is made there as well. A half has no picker of its own.
    head = "document.querySelector('#debugbar [data-dbg-agent=\"{}\"]')".format
    bar.until(f"{head('a')}.value === 'jack' && {head('b')}.value === 'stock'", "the lanes say which Civ AI each half seats")
    for select in (head("a"), head("b"), "debugSplit.host.querySelector('[data-dbg-strategy=\"a\"]')"):
        assert bar.ev(f"{PICKER_FITS}({select})"), f"{select} cuts its longest listing short"
    assert bar.ev(half("a", "'document.getElementById(\"dbg-agent\").offsetParent === null'")), "a half has a picker of its own"
    for name in ("stock", "jack"):
        bar.ev(f"(() => {{ const select = {head('a')}; select.value = '{name}';"
               " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
        bar.until(half("a", "'debugStrategy()'") + f" === '{name}'", f"A is seated with {name} from its lane")
        assert bar.ev(f"debugSplit.host.querySelector('[data-dbg-strategy=\"a\"]').value") == name
    # A half says the Civ AI it was given to its engine when it is given it,
    # and not first at the next step: its timeline is that agent's from then.
    bar.until(half("a", "'engine.seat'") + " === 'jack' && " + half("b", "'engine.seat'") + " === 'stock'",
              "each half's engine is told which Civ AI holds its seat")
    assert (seated("a"), seated("b")) == (["stock", "jack"], ["stock"]), (seated("a"), seated("b"))

    # Locked: A is run to the turn and B follows it turn by turn, each with
    # its own Civ AI on every step.
    def play_both_to(turn: int) -> None:
        bar.ev(f"debugSplit.host.querySelector('[data-dbg-until]').value = '{turn}', true")
        bar.click('[data-dbg-split="versus-run"]')
        bar.until(f"debugSplit.running === 0 && debugSplit.panes.a.turn === {turn} && debugSplit.panes.b.turn === {turn}"
                  f" && debugSplit.history.a.has({turn}) && debugSplit.history.b.has({turn})",
                  f"both halves are played to turn {turn}")

    play_both_to(start + 4)
    assert ops("a") == [["step", "jack"]] * 4, ops("a")
    assert ops("b") == [["step", "stock"]] * 4, ops("b")
    scores = bar.ev("[debugSplit.panes.a.numbers.score, debugSplit.panes.b.numbers.score]")
    assert scores[1] - scores[0] == 4, scores
    # The numbers are set side by side with the larger marked, and charted
    # from the turn of the split.
    score = bar.ev("[...debugSplit.host.querySelectorAll('[data-dbg-stat=\"score\"] b')].map(b => [b.textContent, b.className])")
    assert score == [[str(scores[0]), ""], [str(scores[1]), "lead"]], score
    assert bar.ev("[debugSplit.history.a.size, debugSplit.history.b.size]") == [5, 5]
    assert bar.ev("Math.min(...debugSplit.history.a.keys())") == start
    # Each lane is on its half's turn, and from the turn of the split on it is
    # drawn in the half's own colour.
    assert bar.ev(lanes) == [start + 4, start + 4], bar.ev(lanes)
    assert bar.ev("[debugSplit.panes.a.first, debugSplit.panes.b.last]") == [1, start + 4]
    assert bar.ev("['a', 'b'].every(letter => !document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-own`).hidden)")

    # Unlocked: the host's deck goes, and each half has the tools for its own
    # world again, with neither a timeline nor a picker: the host has both.
    bar.click('[data-dbg-lock]')
    assert bar.ev("debugSplit.locked") is False
    bar.until(f"!({deck_shown})", "the host of an unlocked split shows no deck")
    for letter in "ab":
        bar.until(half(letter, bar_shown), f"{letter.upper()} has a bar of its own once unlocked")
        assert bar.ev(half(letter, "'getComputedStyle(document.querySelector(\"#debugbar .dbg-time\")).display'")) == "none"
        assert bar.ev(half(letter, "'getComputedStyle(document.querySelector(\"#debugbar .dbg-deck\")).display'")) != "none"
        assert bar.ev(half(letter, "'document.getElementById(\"dbg-agent\").offsetParent === null'")), "a half has a picker of its own"
    # Each half is run by itself.
    play_both_to(start + 6)
    assert ops("a")[4:] == [["step", "jack"]] * 2 and ops("b")[4:] == [["step", "stock"]] * 2, (ops("a"), ops("b"))

    # The same start again: both halves go back to the turn they split on,
    # each by one jump, and the button has nothing left to do there.
    back = "debugSplit.host.querySelector('[data-dbg-split=\"versus-back\"]')"
    assert bar.ev(back + ".disabled") is False and str(start) in bar.ev(back + ".title"), bar.ev(back + ".title")
    bar.click('[data-dbg-split="versus-back"]')
    bar.until(f"debugSplit.panes.a.turn === {start} && debugSplit.panes.b.turn === {start} && {back}.disabled",
              "both halves go back to the turn of the split")
    assert ops("a")[6:] == [["jump", None]] and ops("b")[6:] == [["jump", None]], (ops("a"), ops("b"))
    assert bar.ev("debugSplit.locked") is False
    # They are played again from there, still each by itself.
    play_both_to(start + 6)
    assert ops("a")[7:] == [["step", "jack"]] * 6 and ops("b")[7:] == [["step", "stock"]] * 6, (ops("a"), ops("b"))
    assert bar.ev("[debugSplit.history.a.size, debugSplit.history.b.size]") == [7, 7]

    # Unlocked, a lane is the hand on its own half. B walks back two turns
    # along what it played and A stays where it is.
    both = "debugSplit.running === 0 && debugSplit.panes.a.turn === {} && debugSplit.panes.b.turn === {}".format
    bar.drag("dbg-slider-b", start + 4)
    bar.until(both(start + 6, start + 4), "B's lane walks B back")
    assert ops("b")[13:] == [["jump", None]] and len(ops("a")) == 13, (ops("a"), ops("b"))
    # A turn ahead of what the half has played is played to, by its own Civ AI.
    bar.drag("dbg-slider-b", start + 8)
    bar.until(both(start + 6, start + 8), "B's lane plays B on")
    assert ops("b")[14:] == [["step", "stock"]] * 4 and len(ops("a")) == 13, (ops("a"), ops("b"))
    assert bar.ev(lanes) == [start + 6, start + 8], bar.ev(lanes)
    # The lock closes only on two halves that stand on the same turn: with B
    # two turns ahead of A it cannot be pressed, and asking does nothing.
    lock = "document.querySelector('[data-dbg-lock]')"
    assert bar.ev(f"[{lock}.disabled, {lock}.getAttribute('aria-pressed')]") == [True, "false"]
    bar.ev("debugSplitLock(true), true")
    assert bar.ev("debugSplit.locked") is False
    bar.drag("dbg-slider-b", start + 6)
    bar.until(both(start + 6, start + 6) + f" && !{lock}.disabled", "the lock is offered once both halves stand on one turn")
    bar.click('[data-dbg-lock]')
    assert bar.ev(f"[debugSplit.locked, {lock}.getAttribute('aria-pressed')]") == [True, "true"]
    # Locked, either lane moves both halves: back along what both have played,
    # and on past it with A run and B following turn by turn.
    bar.drag("dbg-slider-b", start + 2)
    bar.until(both(start + 2, start + 2), "a lane walks both halves back when they are locked")
    assert ops("a")[13:] == [["jump", None]] and ops("b")[-2:] == [["jump", None]] * 2, (ops("a"), ops("b"))
    bar.drag("dbg-slider-a", start + 10)
    bar.until(both(start + 10, start + 10), "a lane plays both halves on when they are locked")
    assert ops("a")[14:] == [["step", "jack"]] * 8, ops("a")
    assert ops("b")[-8:] == [["step", "stock"]] * 8, ops("b")
    assert bar.ev("debugSplit.until") is None, "the run is over and the lanes still show where it was headed"

    # A -> B makes B a copy of A, its line in the chart included.
    bar.click('[data-dbg-split="copy-a"]')
    bar.until("debugSplit.panes.b.numbers.score === debugSplit.panes.a.numbers.score", "B becomes a copy of A")
    assert ops("b")[-1][0] == "copy", ops("b")

    # Keep B: the split closes and the host plays on from that half.
    kept = bar.ev("debugSplit.panes.b.id")
    bar.forget()
    bar.click('[data-dbg-split="keep-b"]')
    bar.until("!debugSplit", "the split closes")
    assert bar.sent() == [{"op": "adopt", "pane": kept}], bar.sent()
    assert bar.ev("state.turn") == start + 10
    assert bar.note() == "Playing on from simulator B", bar.note()
    # The half that is kept keeps its Civ AI: the seat is still that agent's,
    # the box says so, and it is the page's choice from here.
    assert bar.ev("[document.getElementById('autoplaystrategy').value, document.getElementById('dbg-agent').value,"
                  " localStorage.getItem(AUTOPLAY_STRATEGY_KEY)]") == ["stock"] * 3
    # The screen is whole again and the bar is the world's own.
    assert (shown(""), shown("a"), shown("b")) == (True, False, False)
    assert bar.ev("getComputedStyle(document.querySelector('#debugbar .dbg-deck')).display") != "none"
    bar.until("Math.round(document.getElementById('debugbar').getBoundingClientRect().left) === 200",
              "the box is beside the left panel again once the screen is whole")
    assert bar.ev("document.getElementById('dbg-slider').value") == str(start + 10)

    # A half that drew its first state before the bar's script arrived still
    # says where it stands. (It used to say nothing until something moved.)
    bar.ev("window.lateScript = true")
    bar.click('[data-dbg="split"]')
    bar.until(reported(start + 10), "a half whose script came late still reports")
    bar.click('[data-dbg-split="keep-a"]')
    bar.until("!debugSplit", "the second split closes")
    bar.ev("window.lateScript = false")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_the_lock(bar: Bar) -> None:
    """The lock that joins the two timelines, the one panel, and identical halves played once."""
    half = "debugSplit.panes.{}.frame.contentWindow.eval({})".format
    # Everything a half's page asked its engine: the path, the operation, and the twin it named.
    calls = lambda letter: bar.ev(half(letter, json.dumps(
        "engine.sent.map(call => [call.path, call.body ? call.body.op : null, call.body ? call.body.twin || null : null])")))
    strategies = lambda letter: bar.ev(half(letter, json.dumps(
        "engine.sent.filter(call => call.body && call.body.op === 'step').map(call => call.body.strategy)")))

    def forget() -> None:
        for letter in "ab":
            bar.ev(half(letter, json.dumps("engine.sent.length = 0, true")))

    both = "debugSplit.panes.a.turn === {} && debugSplit.panes.b.turn === {}".format
    same_score = "debugSplit.panes.a.numbers.score === debugSplit.panes.b.numbers.score"
    deck_idle = "debugSplitDeck() && debugSplitDeck().idle"
    head = "document.querySelector('#debugbar [data-dbg-agent=\"{}\"]')".format

    def seat(letter: str, name: str) -> None:
        bar.ev(f"(() => {{ const select = {head(letter)}; select.value = '{name}';"
               " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
        bar.until(half(letter, "'debugStrategy()'") + f" === '{name}'", f"{letter.upper()} is seated with {name}")

    # The halves of a split start on the Civ AI picked in the box.
    bar.ev("(() => { const select = document.getElementById('dbg-agent'); select.value = 'jack';"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    bar.idle("engine.seat === 'jack'", "the Civ AI picked in the box is said to the engine")
    start = bar.ev("state.turn")
    bar.click('[data-dbg="split"]')
    bar.until(f"debugSplit && {both(start, start)} && debugSplit.panes.a.numbers && debugSplit.panes.b.numbers",
              "both halves say where they stand")
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy]") == ["jack", "jack"]
    a, b = bar.ev("[debugSplit.panes.a.id, debugSplit.panes.b.id]")
    bar.until(f"debugBar.classList.contains('dbg-merged') && {deck_idle}", "the one panel is up")

    # The lock joins the two lanes: it stands between the middles of the two
    # rails, shut, and the two markers are linked while they are on one turn.
    lock = "document.querySelector('[data-dbg-lock]')"
    at = bar.ev("(() => { const middle = name => { const box = document.querySelector(name).getBoundingClientRect();"
                " return [box.width, (box.top + box.bottom) / 2, box.left]; };"
                " return [middle('[data-dbg-lock]'), middle('[data-dbg-lane=\"a\"] .dbg-rail'),"
                " middle('[data-dbg-lane=\"b\"] .dbg-rail')]; })()")
    assert at[0][0] > 0 and at[1][1] < at[0][1] < at[2][1] and at[0][2] < at[1][2], at
    assert bar.ev(f"[{lock}.disabled, {lock}.getAttribute('aria-pressed'), debugBar.classList.contains('dbg-locked'),"
                  " debugBar.classList.contains('dbg-joined')]") == [False, "true", True, True]
    assert bar.ev(f"{lock}.title").startswith("Locked: the two simulators stay on the same turn"), bar.ev(f"{lock}.title")

    # Two locked halves that are the same world, seated with the same Civ AI,
    # are played once: A is asked for both and names B as its twin, and B's
    # page only reads the world it then holds.
    bar.until(half("a", "'debugTwin'") + f" === '{b}'", "A is told who its twin is")
    forget()
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(start + 1, start + 1)} && {same_score} && {deck_idle}", "the one panel plays both halves a turn")
    assert calls("a") == [["/debug", "step", b]], calls("a")
    assert calls("b") == [["/state", None, None]], calls("b")
    assert bar.ev("document.getElementById('dbg-turn').value") == str(start + 1)

    # A change on the one panel that leaves the turn where it was is asked
    # once as well: a layer.
    forget()
    bar.click('[data-dbg-pop="layers"]')
    bar.until("!document.getElementById('dbg-pop-layers').hidden", "the one panel opens its own panels")
    bar.click('#dbg-pop-layers [data-dbg-layer="research"]')
    bar.until(half("b", "'state.debug.layers[0].on'") + " && debugSplitDeck().timeline.layers[0].on",
              "a layer set on the one panel is on in both halves")
    assert calls("a") == [["/debug", "layers", b]], calls("a")
    assert calls("b") == [["/state", None, None]], calls("b")
    assert bar.ev("document.getElementById('dbg-layer-count').textContent") == "1"
    bar.click('[data-dbg-pop="layers"]')

    # A Civ AI of its own in B: the two would not be asked the same thing, so
    # no twin is named, and each half plays its own turn.
    seat("b", "stock")
    bar.until(half("a", "'debugTwin'") + " === null", "A has no twin while B seats another Civ AI")
    forget()
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(start + 2, start + 2)} && {deck_idle}", "the one panel plays both halves, each with its own Civ AI")
    assert calls("a") == [["/debug", "step", None]] and calls("b") == [["/debug", "step", None]], (calls("a"), calls("b"))
    assert (strategies("a"), strategies("b")) == (["jack"], ["stock"])
    assert not bar.ev(same_score), "the two halves have parted"

    # The same Civ AI again, but no longer the same world: A names B, the
    # engine finds them apart, and B is asked to play its own turn.
    seat("b", "jack")
    bar.until(half("a", "'debugTwin'") + f" === '{b}'", "A is told who its twin is again")
    forget()
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(start + 3, start + 3)} && {deck_idle}", "halves that have parted are both played")
    assert calls("a") == [["/debug", "step", b]] and calls("b") == [["/debug", "step", None]], (calls("a"), calls("b"))
    # What leaves the turn where it was is done in both: Reveal, and Undo.
    forget()
    bar.click('[data-dbg="reveal"]')
    bar.until(half("b", "'state.debug.reveal'") + " === true && debugSplitDeck().timeline.reveal === true",
              "Reveal on the one panel reveals both halves")
    assert calls("a") == [["/debug", "reveal", b]] and calls("b") == [["/debug", "reveal", None]], (calls("a"), calls("b"))

    # The turn box and the wheel over it walk both halves: A goes, B follows.
    bar.until(deck_idle, "the one panel is free")
    forget()
    bar.type_turn(start + 1)
    bar.until(f"{both(start + 1, start + 1)} && {deck_idle}", "the turn box walks both halves back")
    assert calls("a") == [["/debug", "jump", b]] and calls("b") == [["/debug", "jump", None]], (calls("a"), calls("b"))
    # B's next turn was played by Stock, while B seated it, and B seats Jack
    # now. On B's lane the turns ahead are drawn apart as another Civ AI's,
    # and A's, which Jack played, are A's timeline as before.
    bar.until("debugSplit.panes.b.marks && debugSplit.panes.b.marks.beyond", "B says the turns ahead are another Civ AI's")
    assert bar.ev("[debugSplit.panes.a.marks.beyond, debugSplit.panes.b.marks.beyond]") == \
        [None, {"from": start + 2, "to": start + 3, "by": "stock"}]
    assert bar.ev("['a', 'b'].map(letter => document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-beyond`).hidden)") \
        == [True, False]
    bar.wheel("#dbg-turn", 100)
    bar.until(f"{both(start + 2, start + 2)} && {deck_idle}", "the wheel over the turn box takes both halves on")
    # A walks to the turn it played. B does not walk to Stock's turn: it
    # plays the turn, with the Civ AI it seats now.
    assert calls("a")[1:] == [["/debug", "forward", b]] and calls("b")[1:] == [["/debug", "step", None]], (calls("a"), calls("b"))
    assert strategies("b") == ["jack"], strategies("b")
    bar.until("!debugSplit.panes.b.marks.beyond", "B has played over the turns that were another Civ AI's")
    # Auto-play on the one panel counts A's turns, and the lanes show where it is headed.
    bar.ev("(() => { const turns = document.getElementById('dbg-turns'); turns.value = '2';"
           " turns.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    bar.click('[data-dbg="autoplay"]')
    bar.until(f"{both(start + 4, start + 4)} && {deck_idle}", "Auto-play on the one panel plays both halves")
    assert bar.note() == f"Played to turn {start + 4}", bar.note()

    # A -> B makes the two the same world again, and they are played once again.
    bar.click('[data-dbg-split="copy-a"]')
    bar.until(same_score, "B becomes a copy of A")
    forget()
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(start + 5, start + 5)} && {same_score} && {deck_idle}", "a copy is a twin")
    assert calls("a") == [["/debug", "step", b]] and calls("b") == [["/state", None, None]], (calls("a"), calls("b"))

    # Unlocked, the one panel goes and each half shows its own. A half's own
    # panel plays that half and nothing else, and names no twin.
    bar_shown = "'getComputedStyle(document.getElementById(\"debugbar\")).display !== \"none\"'"
    deck_shown = "getComputedStyle(document.querySelector('#debugbar .dbg-deck')).display !== 'none'"
    assert bar.ev(deck_shown) and [bar.ev(half(letter, bar_shown)) for letter in "ab"] == [False, False]
    bar.click('[data-dbg-lock]')
    bar.until(f"!debugSplit.locked && !({deck_shown}) && !debugBar.classList.contains('dbg-locked')",
              "the lock opens and the one panel goes")
    for letter in "ab":
        bar.until(half(letter, bar_shown), f"{letter.upper()} shows its own panel once unlocked")
    bar.until(half("a", "'debugTwin'") + " === null", "an unlocked half has no twin")
    forget()
    bar.ev(half("a", json.dumps("document.querySelector('[data-dbg=\"next\"]').click(), true")))
    bar.until(both(start + 6, start + 5), "A's own panel plays A alone")
    assert calls("a") == [["/debug", "step", None]] and calls("b") == [], (calls("a"), calls("b"))
    # Apart, the lock cannot close; B changes a layer of its own meanwhile.
    assert bar.ev(f"[{lock}.disabled, {lock}.getAttribute('aria-pressed')]") == [True, "false"]
    assert "same turn" in bar.ev(f"{lock}.title"), bar.ev(f"{lock}.title")
    bar.ev(half("b", json.dumps("document.querySelector('[data-dbg-pop=\"layers\"]').click(), true")))
    bar.ev(half("b", json.dumps("document.querySelector('#dbg-pop-layers [data-dbg-layer=\"cities\"]').click(), true")))
    bar.until(half("b", "'state.debug.layers[1].on'"), "B switches a layer on by itself")
    bar.ev(half("b", json.dumps("document.querySelector('[data-dbg=\"next\"]').click(), true")))
    bar.until(f"{both(start + 6, start + 6)} && !{lock}.disabled", "the lock is offered on the same turn")
    # Locked again, B is made the same in what the one panel shows: A's layers.
    forget()
    bar.click('[data-dbg-lock]')
    bar.until(f"debugSplit.locked && debugBar.classList.contains('dbg-merged') && {deck_idle}", "the one panel is back")
    bar.until(half("b", "'state.debug.layers.map(layer => layer.on).join()'") + " === 'true,false'",
              "B takes A's layers when the two are locked")
    assert calls("a") == [] and calls("b") == [["/debug", "layers", None]], (calls("a"), calls("b"))
    assert [bar.ev(half(letter, bar_shown)) for letter in "ab"] == [False, False]

    # A lane shows what stands on its half's timeline: its breakpoint turns
    # and its save points. A half's own timeline is put away in a split, so
    # the lanes are the only place to see them, and a mark is its half's own
    # to go to. Locked, the other half is brought along.
    mark = "document.querySelector('#debugbar [data-dbg-lane=\"{}\"] .dbg-marks {}')".format
    stop = f'i.stop[data-dbg-turn="{start + 16}"]'
    flag = lambda name: f'i[title^="{name}:"]'

    def point(name: str) -> None:
        bar.ev(f"document.querySelector('#dbg-pop-saves [data-dbg-field=pointname]').value = '{name}', true")
        bar.click('[data-dbg="point"]')
        bar.until(f"{mark('a', flag(name))} && {mark('b', flag(name))} && {deck_idle}",
                  f"the save point {name}, set on the one panel, stands on both lanes")

    point("fork")
    bar.click('[data-dbg="break-plus"]')
    bar.until(f"{mark('a', stop)} && {mark('b', stop)} && {deck_idle}", "a breakpoint turn stands on both lanes")
    assert "play to it" in bar.ev(f"{mark('b', stop)}.title"), bar.ev(f"{mark('b', stop)}.title")
    bar.ev(f"{mark('b', stop)}.click(), true")
    bar.until(f"{both(start + 16, start + 16)} && {deck_idle}", "a breakpoint clicked on B's lane is played to by both halves")
    # Two lanes stand close. A's breakpoint and B's save point on the same
    # turn each keep to their own rail, so neither covers the other.
    point("there")
    apart = bar.ev(f"(() => {{ const box = held => held.getBoundingClientRect(), dot = box({mark('a', stop)}),"
                   f" pin = box({mark('b', flag('there'))}); return [dot.bottom, pin.top, Math.abs(dot.left - pin.left)]; }})()")
    assert apart[0] <= apart[1] and apart[2] < 1, apart
    forget()
    bar.ev(f"{mark('a', flag('fork'))}.click(), true")
    bar.until(f"{both(start + 6, start + 6)} && {deck_idle}", "a save point clicked on A's lane takes both halves to it")
    assert calls("a")[0][:2] == ["/debug", "goto_save"], calls("a")
    # Unlocked, a lane's marks are that half's alone, and so is going to one.
    bar.click('[data-dbg-lock]')
    bar.until("!debugSplit.locked", "the lock opens")
    bar.until(half("b", bar_shown), "B shows its own panel again")
    bar.ev(half("b", json.dumps("document.querySelector('[data-dbg=\"next\"]').click(), true")))
    bar.until(both(start + 6, start + 7), "B walks on alone")
    bar.ev(half("b", json.dumps("document.querySelector('[data-dbg=\"break-plus\"]').click(), true")))
    own = f'i.stop[data-dbg-turn="{start + 17}"]'
    bar.until(f"{mark('b', own)} && !{mark('a', own)}", "a breakpoint B sets by itself stands on B's lane alone")
    forget()
    bar.ev(f"{mark('b', flag('fork'))}.click(), true")
    bar.until(both(start + 6, start + 6), "a save point clicked on B's lane takes B alone to it")
    bar.until(half("b", "'!debugBusy'"), "B is done")
    assert calls("a") == [] and calls("b") == [["/debug", "goto_save", None]], (calls("a"), calls("b"))

    bar.click('[data-dbg-split="keep-a"]')
    bar.until("!debugSplit", "the split closes")
    assert bar.ev(deck_shown) and not bar.ev("debugBar.classList.contains('dbg-merged')")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def main() -> int:
    here = pathlib.Path(__file__).resolve().parent
    stage = pathlib.Path(tempfile.mkdtemp(prefix="civvis-debugger-"))
    profile = fresh_profile("civvis-debugger-profile-")
    shutil.copy(here.parent / "web" / "assets" / "app_debugger.js", stage / "app_debugger.js")
    (stage / "index.html").write_text(PAGE, encoding="utf-8")
    port = free_port()
    server = socketserver.TCPServer(("127.0.0.1", port), functools.partial(Quiet, directory=str(stage)))
    threading.Thread(target=server.serve_forever, daemon=True).start()
    debug_port = free_port()
    chrome_log_path = stage / "chrome-stderr.log"
    chrome_log = chrome_log_path.open("wb")
    chrome = subprocess.Popen(
        [os.environ.get("CIVVIS_TEST_CHROME") or find_chrome(), "--headless=new",
         f"--remote-debugging-port={debug_port}", f"--user-data-dir={profile}", "--no-first-run",
         "--disable-gpu", "--window-size=1400,900", f"http://127.0.0.1:{port}/"],
        stdout=subprocess.DEVNULL, stderr=chrome_log)
    tether(chrome)
    try:
        target = None
        deadline = time.time() + 60
        while time.time() < deadline and target is None:
            try:
                with urllib.request.urlopen(f"http://127.0.0.1:{debug_port}/json", timeout=2) as answer:
                    pages = [page for page in json.load(answer) if page.get("type") == "page"]
                target = next((page for page in pages if page.get("webSocketDebuggerUrl")), None)
            except OSError:
                pass
            if target is None:
                time.sleep(0.2)
        if target is None:
            tail = chrome_log_path.read_bytes()[-1500:].decode("utf-8", "replace")
            raise AssertionError(f"Chrome never offered a debuggable page within 60s.\n{tail}")
        dev = Devtools(target["webSocketDebuggerUrl"])
        dev.call("Runtime.enable")
        # A headless page is not focused, and then `focus()` fires no focus events.
        dev.call("Emulation.setFocusEmulationEnabled", {"enabled": True})
        bar = Bar(dev)
        # ⚠ The first run of this file on a CI runner failed right here:
        # after ten seconds neither script on the page was defined. The two
        # runs after it passed, and the one that was timed had the page ready
        # in 0.1s, so a slow start is not shown and the cause is not known.
        # So the page is opened
        # again when it has not arrived, and each time that happens the run
        # says what the page looked like, which is how the cause gets found.
        ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"
        opened = time.time()
        for attempt in range(1, 5):
            try:
                bar.until(ready, "the bar's script loaded on the stand-in page", seconds=15)
                break
            except AssertionError as late:
                if attempt == 4:
                    raise
                print(f"attempt {attempt}: {late}; opening the page again.")
                dev.call("Page.navigate", {"url": f"http://127.0.0.1:{port}/"})
        print(f"the stand-in page was ready {time.time() - opened:.1f}s after Chrome answered.")
        check_the_bar(bar)
        print("the header box stands beside the left panel with the map area beneath it, the timeline "
              "on top and the open turn under its middle, and sends what each control is for: the "
              "turn box, the timeline back and ahead, the wheel, the turn buttons, the eras and Era "
              "start, save points and their flags, a breakpoint picked from the timeline, and "
              "Auto-play with its count.")
        check_breakpoints_and_a_run(bar)
        check_layers_reveal_and_undo(bar)
        print("breakpoints are set from the form, auto-play stops on a turn, on a new era or by "
              "hand, layers, reveal and undo send what they are for, and the Single player switch "
              "changes the page's view and keeps the choice.")
        check_stepping_a_seat(bar)
        print("a round is stepped a civilization at a time, says who is next, and End Turn finishes it.")
        check_a_picked_civ_ai_plays_its_own_turns(bar)
        print("a Civ AI picked on an earlier turn is said to the engine at once, the turns another one "
              "played ahead are drawn apart and named, and the turn buttons, the turn box, the "
              "timeline and Auto-play all play from there with the Civ AI picked.")
        check_the_split(bar)
        print("the split opens at once, seats a Civ AI in each half, plays both to a turn locked and "
              "unlocked, sets their numbers side by side, starts both again from the split, moves "
              "each half or both by the lanes in the host's bar, copies, keeps, and hears a half "
              "whose script came late.")
        check_the_lock(bar)
        print("the lock joins the two lanes and closes only on one turn, the host's deck is the one "
              "panel of a locked split and each half's own comes back when it opens, two "
              "identical halves are played once, and a lane shows its half's breakpoints and "
              "save points and goes to the one that is clicked.")
        return 0
    finally:
        chrome.terminate()
        try:
            chrome.wait(timeout=5)
        except subprocess.TimeoutExpired:
            chrome.kill()
        chrome_log.close()
        server.shutdown()
        server.server_close()
        shutil.rmtree(stage, ignore_errors=True)
        shutil.rmtree(profile, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
