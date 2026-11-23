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
carry its own, and each kept turn has a print of its world that two halves
share only when they were played the same way, which is what the host's
timeline branches by. A request that names the other half as its twin is answered
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
// `worlds` is the print of each kept turn's world: the print of the turn it
// was played on from, and who played it. Two toy engines that were played
// the same way from one world give the same prints, as two halves of the
// real engine do (`world_print`).
const engine = {turn: 1, marks: [1], worlds: {1: 1}, saves: [], ids: 0, breaks: [], line: null, stopped: null, score: 0,
  seat: null, by: {}, seed: 7,
  current: 0, stepped: null, acted: 0, reveal: false, layers: [], wait: 0,
  eras: [{era: 0, name: "ancient", turn: 1}], eraAt: null,
  watch: {categories: [], phrases: [], important: false, every_seat: false, conditions: [], eras: false}, sent: []};
const ERAS = ["ancient", "classical", "medieval"];
// The page's own list of the world eras (`app.js`), which the bar names an
// era it has not seen yet from.
const ERA_NAMES = ["ancient", "classical", "medieval", "renaissance", "industrial", "modern", "atomic",
  "information", "future"];
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
// The last turn ahead that playing on from the open turn would only play
// again: the Civ AI seated played every turn from here to it, and the open
// turn stands as it opened. A half of a split has none, as in the engine.
const HALF = new URLSearchParams(location.search).has("pane") && window.parent !== window;
function enginePlayedAhead() {
  if (HALF || engine.seat === null || engine.acted > 0 || engine.current !== 0) return null;
  let last = engine.turn;
  while (engine.marks.includes(last + 1) && engine.by[last + 1] === engine.seat) last += 1;
  return last > engine.turn ? last : null;
}
// Whose play each stretch of the kept turns was, as the engine gives it
// (`Timeline::played_by`): a stretch begins on the turn its first turn was
// played from. Every kept turn is in it, the ones ahead that are not on the
// seat's line as well.
function enginePlayedBy(marks) {
  const stretches = [];
  for (let at = 1; at < marks.length; at++) {
    const by = engine.by[marks[at]] || null, last = stretches[stretches.length - 1];
    if (last && last.by === by) last.to = marks[at];
    else stretches.push({from: marks[at - 1], to: marks[at], by});
  }
  return stretches;
}
function engineState(error = null) {
  const marks = engineLine(), kept = engine.marks;
  return {error, turn: engine.turn, seed: engine.seed, current: engine.current, units: [], cities: [],
    world_era: engine.eras.filter(era => era.turn <= engine.turn).pop().era,
    ...(engine.limit ? {turn_limit: engine.limit} : {}),
    players: [{id: 0, civ: "rome", score: engine.score, cities: 1, population: 1, military: 1,
      yields: {science: 1, culture: 1, gold: 1, faith: 0}},
      {id: 1, civ: "greece"}, {id: 2, civ: "barbarians", is_barbarian: true}],
    debug: {turn: engine.turn, turns: [...marks], worlds: marks.map(turn => engine.worlds[turn]),
      first: marks[0], last: marks[marks.length - 1],
      seat: engine.seat, played_ahead: enginePlayedAhead(), beyond: marks.length < kept.length
        ? {from: kept[marks.length], to: kept[kept.length - 1], by: engine.by[kept[marks.length]] || null} : null,
      acted: engine.acted, can_undo: engine.acted > 0 && engine.current === 0,
      can_back: engine.acted > 0 || engine.turn > marks[0], can_forward: engine.turn < marks[marks.length - 1],
      breakpoints: {turns: [...engine.breaks], ...JSON.parse(JSON.stringify(engine.watch)),
        known_categories: ["War", "Wonder"], known_tests: [">", "changes"]},
      layers: [["research", "Research"], ["cities", "Cities"]].map(([id, label]) =>
        ({id, label, on: engine.layers.includes(id)})), assisted: 0, stopped: engine.stopped,
      stepped: engine.stepped, reveal: engine.reveal, playing: engine.current, memory_line: engine.line,
      eras: engine.eras.filter(era => era.turn <= marks[marks.length - 1]),
      played_by: enginePlayedBy(kept),
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
    const behind = engine.marks.filter(turn => turn < engine.turn);
    engine.marks = behind.concat(engine.turn);
    engine.by = Object.fromEntries(Object.entries(engine.by).filter(([turn]) => +turn < engine.turn));
    if (by) engine.by[engine.turn] = by;
    const from = engine.worlds[behind[behind.length - 1]] || 0;
    engine.worlds = Object.fromEntries(Object.entries(engine.worlds).filter(([turn]) => +turn < engine.turn));
    engine.worlds[engine.turn] = (from * 31 + (by === "stock" ? 2 : by ? 1 : 3)) % 2147483647;
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
    // The game is over at its last turn.
    else if (engine.limit && engine.turn >= engine.limit) engine.stopped = {kind: "end", turn: engine.turn};
  } else if (body.op === "walk") {
    // Along the turns the seated Civ AI already played, as far as asked and
    // no further than the first a breakpoint names. Nothing is played, so
    // nothing kept is let go and the score a step adds to stays as it is.
    const last = Math.min(enginePlayedAhead() ?? engine.turn, body.turn ?? Infinity);
    while (engine.turn < last && !engine.stopped) {
      engine.turn += 1;
      const dawn = engine.eras.find(era => era.turn === engine.turn);
      if (engine.breaks.includes(engine.turn)) engine.stopped = {kind: "turn", turn: engine.turn};
      else if (dawn && (body.era || engine.watch.eras)) engine.stopped = {kind: "era", turn: engine.turn, era: dawn.era};
      else if (engine.limit && engine.turn >= engine.limit) engine.stopped = {kind: "end", turn: engine.turn};
    }
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
  } else if (body.op === "branch") {
    // A branch begins on the open turn: nothing kept past it is its own.
    engine.marks = engine.marks.filter(turn => turn <= engine.turn);
    for (const kept of [engine.by, engine.worlds])
      for (const turn of Object.keys(kept)) if (+turn > engine.turn) delete kept[turn];
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
        self.settled()

    def settled(self) -> None:
        """One screen and two change places in one move, and the window is held until it is made.

        A press that begins one (Branch, Drop, the ✕ of a branch) is over
        when the other picture stands there and takes a press again."""
        self.until("!debugHeld && !debugMoving", "the window is let go of when the move is made")

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

# A game under the debugger opens on its two tracks. Every check but the one
# of that opening view begins on the one timeline, which a page asked for
# this way opens on.
ONE_TIMELINE = "?tracks=1"


def seat_the_timeline(bar: Bar, name: str) -> None:
    """Pick a Civ AI in the box's own picker, on the one timeline, and have the timeline play on with it.

    Another Civ AI than the one seated is asked about first (Switch, or
    Branch); the one already seated is nothing to ask."""
    bar.ev(f"(() => {{ const select = document.getElementById('dbg-agent'); select.value = '{name}';"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    if bar.ev("!!debugAsk"):
        assert bar.ev("debugAsk.kind") == "switch", bar.ev("debugAsk")
        bar.click('#dbg-ask [data-dbg-ask="switch"]')


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
    # The timeline is twice the height it was. Under it the open turn stands
    # in the middle, under the middle of the timeline, with the turn buttons
    # either side of it in their order: turn 1, era start, ten back, one
    # back; one on, ten on. Auto-play (the turns, the button), layers and
    # breakpoints are at the left end in that order, before the turn buttons.
    box = ("(name => { const at = document.querySelector(name).getBoundingClientRect();"
           " return [at.left, at.top, at.right, at.bottom]; })")
    rail = bar.ev(f"{box}('{LANE} .dbg-rail')")
    deck = bar.ev(f"{box}('#debugbar .dbg-deck')")
    assert rail[3] - rail[1] >= 64 and rail[3] <= deck[1], (rail, deck)
    assert rail[0] > left and rail[2] < window, (rail, left, window)
    middle = (rail[0] + rail[2]) / 2
    turn_box = bar.ev(f"{box}('#dbg-turn')")
    assert abs((turn_box[0] + turn_box[2]) / 2 - middle) < 40, (turn_box, middle)
    order = ["#dbg-turns", "[data-dbg=autoplay]", "[data-dbg-pop=layers]", "[data-dbg-pop=breaks]",
             "[data-dbg=first]", "[data-dbg=era]", "[data-dbg=back10]", "[data-dbg=back]",
             "#dbg-turn", "[data-dbg=next]", "[data-dbg=next10]", "[data-dbg=era-next]", "[data-dbg=end]"]
    assert bar.ev("(names => { const all = [...document.querySelectorAll('#debugbar .dbg-deck *')];"
                  " const at = names.map(name => all.indexOf(document.querySelector('#debugbar .dbg-deck ' + name)));"
                  " return at.every((place, index) => place >= 0 && (index === 0 || place > at[index - 1])); })"
                  f"({json.dumps(order)})"), "the box's controls are not in their order"
    along = [bar.ev(f"{box}('#debugbar {name}')") for name in order[4:]]
    assert all(before[2] <= after[0] for before, after in zip(along, along[1:])), along
    assert along[3][2] <= middle <= along[5][0], "one back and one on are not either side of the middle"
    # The Civ AI picker is above the timeline, from where the rail begins
    # (operator, 2026-11-21: "move 1202 ffor... box ... above the timeline").
    picker = bar.ev(f"{box}('#dbg-agent')")
    assert picker[3] <= rail[1] and picker[0] >= rail[0] and picker[1] >= top, (picker, rail)
    assert bar.ev("!document.querySelector('#debugbar .dbg-deck #dbg-agent')"), "the picker is still in the deck"
    assert bar.ev("document.querySelector('#debugbar .dbg-who .dbg-field > span').textContent") == "Civ AI"
    # Branch stands under the timeline's marker, between the rail and the
    # deck, and says its name whatever room the box has (operator: "branch
    # should say \"Branch\" and should move around below where the timeline
    # \"cursor\" is. always rpesenting the option to branch").
    under = ("(() => { const pill = document.querySelector('" + LANE + " .dbg-cursor b').getBoundingClientRect();"
             " const branch = document.querySelector('#debugbar [data-dbg=split]').getBoundingClientRect();"
             " return [(pill.left + pill.right) / 2, (branch.left + branch.right) / 2, branch.top, branch.bottom, branch.width]; })()")
    pill_at, branch_at, branch_top, branch_bottom, branch_width = bar.ev(under)
    assert abs(pill_at - branch_at) < 1.5 and branch_width > 40, (pill_at, branch_at, branch_width)
    assert rail[3] <= branch_top and branch_bottom <= deck[1], (rail, branch_top, branch_bottom, deck)
    assert bar.ev("document.querySelector('#debugbar [data-dbg=split]').textContent.trim()") == "⑂Branch"
    assert bar.ev("!document.querySelector('#debugbar .dbg-deck [data-dbg=split]')"), "Branch is still in the deck"
    assert bar.ev("!document.querySelector('#debugbar [data-dbg=split] .dbg-word')"), "Branch loses its word in a tight box"
    # Branch asks nothing first: it is a button, and has no panel.
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
    # Turn 6 was played before: it is walked to, and the three past it played.
    assert [call["op"] for call in bar.sent()] == ["walk"] + ["step"] * 3, bar.sent()
    assert bar.note() == "Played to turn 9", bar.note()
    at_nine = bar.ev(marker)
    assert (at_six[0], at_nine[0]) == ("6", "9") and at_nine[1] > at_six[1], (at_six, at_nine)
    # Branch has moved with the marker.
    pill_at, branch_at = bar.ev(under)[:2]
    assert abs(pill_at - at_nine[1]) < 1.5 and abs(pill_at - branch_at) < 1.5, (at_nine, pill_at, branch_at)
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

    # A turn that was played is said with whose play opened it.
    assert hover(1) == "Turn 1", hover(1)
    assert hover(4) == "Turn 4 · Jack", hover(4)
    assert hover(9) == "Turn 9 · open now · Jack", hover(9)
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
    # turn each began. A name is clicked to go there, and Era Back goes back
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
    bar.idle("state.turn === 5", "Era Back goes to the first turn of the era that is open")
    bar.click('[data-dbg="era"]')
    bar.idle("state.turn === 1", "and from there to the first turn of the era before")
    assert sent() == [("jump", 5), ("jump", 1)], sent()
    assert off("era")
    # Era Next goes on to the first turn of the next era the turns already
    # played reach, and past the last of them the Civ AI plays until a new
    # era begins: from turn 5, it walks to the last turn played and plays the
    # three turns to the one the next era begins on.
    era_next = "document.querySelector('[data-dbg=\"era-next\"]').title"
    assert off("era-next") is False and "turn 5" in bar.ev(era_next), bar.ev(era_next)
    bar.forget()
    bar.click('[data-dbg="era-next"]')
    bar.idle("state.turn === 5", "Era Next goes to the first turn of the next era already played")
    assert sent() == [("jump", 5)], sent()
    assert "until the Medieval era begins" in bar.ev(era_next), bar.ev(era_next)
    score, last = bar.ev("[engine.score, debugTimeline().last]")
    assert last > 6, last
    bar.ev(f"engine.eraAt = {last + 3}, true")
    bar.forget()
    bar.click('[data-dbg="era-next"]')
    bar.idle(f"state.turn === {last + 3}", "Era Next walks to the last turn played and plays until a new era begins")
    assert sent() == [("jump", last)] + [("step", None)] * 3, sent()
    assert bar.note() == f"Stopped on turn {last + 3}: the Medieval era began", bar.note()
    # Play to End walks to the last turn played and plays the game out.
    last += 3
    bar.ev(f"engine.limit = {last + 4}, true")
    bar.type_turn(3)
    bar.idle("state.turn === 3", "the turn box goes to turn 3")
    assert bar.ev("document.querySelector('[data-dbg=\"end\"] .dbg-word').textContent") == "Play to End"
    bar.forget()
    bar.click('[data-dbg="end"]')
    bar.idle(f"state.turn === {last + 4}", "Play to End plays the game out")
    assert sent() == [("jump", last)] + [("step", None)] * 4, sent()
    assert bar.note() == f"The game ended on turn {last + 4}", bar.note()
    # A game in its last era has no next one.
    bar.ev(f"engine.eras.push({{era: 8, name: 'future', turn: {last + 4}}}), render(engineState()), true")
    assert off("era-next") and "the last era" in bar.ev(era_next), bar.ev(era_next)
    # Back to nine turns played and turn 2 open, for what follows.
    bar.ev(f"engine.eras.length = 1, engine.eraAt = null, engine.limit = null, engine.score = {score},"
           " engine.marks = [1, 2, 3, 4, 5, 6, 7, 8, 9], engine.turn = 2, render(engineState()), true")

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
    assert hover(2) == "Turn 2 · open now · Jack · ⚑ here", hover(2)
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
    # it is played to from turn 2, and the run stops because the engine said
    # so. The turns to 9 were played before and are walked: three are played.
    bar.ev("engine.breaks = [12], render(engineState()), true")
    bar.forget()
    bar.click(f'{LANE} .dbg-marks i.stop[data-dbg-turn="12"]')
    bar.idle("state.turn === 12", "a breakpoint ahead is played to")
    assert [call["op"] for call in bar.sent()] == ["walk"] + ["step"] * 3, bar.sent()
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

    # The Civ AI that plays the seat is picked above the timeline, from what
    # the page's own picker lists, and a turn it plays carries it.
    agent = "document.getElementById('dbg-agent')"
    assert bar.ev(f"[...{agent}.options].map(option => option.value)") == ["jack", "stock"]
    assert bar.ev(f"{agent}.value") == "jack"
    # It lists them as the page does, Elo then name, and is wide enough to
    # read the longest of them whole without being opened.
    assert bar.ev(f"[...{agent}.options].map(option => option.textContent)") == ["1211 - JackOfAllTrades", "1000 - ByTheBook"]
    assert bar.ev(f"{PICKER_FITS}({agent})"), "the Civ AI picker cuts its longest listing short"

    def seat(name: str) -> None:
        seat_the_timeline(bar, name)
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
    # What it offers, in the operator's order, and it is set to Era from the
    # start: nothing has picked anything yet.
    assert bar.ev(f"[...{turns}.options].map(option => option.value).join()") == "1,3,5,10,20,era,100,indefinite"
    assert bar.ev(f"[...{turns}.options].map(option => option.textContent).join()") \
        == "1 turn,3 turns,5 turns,10 turns,20 turns,Era,100 turns,Indefinite"
    assert bar.ev(f"{turns}.value") == "era" and label() == ["▶▶", "Auto-play", ""], label()
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

    # Each stretch of the timeline is named by the Civ AI that played it
    # (operator, 2026-11-21: "lets label ai strats on the timeline too, to make
    # the branching more clear"): the one turn Stock played stands between
    # Jack's, every other stretch a shade darker and a line where the seat
    # changed hands. The last name has the rail ahead of it to run on over,
    # and steps past the marker that stands at the end of its stretch.
    assert bar.ev("state.debug.played_by.slice(-2)") == [
        {"from": last, "to": last + 1, "by": "stock"}, {"from": last + 1, "to": last + 7, "by": "jack"}], \
        bar.ev("state.debug.played_by")
    named = bar.ev(f"[...document.querySelectorAll('{LANE} .dbg-by s')].map(s => [s.textContent, s.className])")
    assert len(named) == bar.ev("state.debug.played_by.length") and named[-2][0] == "Stock" and named[-1][0] == "Jack", named
    assert "dbg-next" in named[-2][1] and "dbg-free" not in named[-2][1], named
    assert "dbg-next" in named[-1][1] and "dbg-free" in named[-1][1], named
    assert ("dbg-alt" in named[-2][1]) != ("dbg-alt" in named[-1][1]), named
    stands = bar.ev(
        "(() => { const s = document.getElementById('dbg-slider'), box = s.getBoundingClientRect();"
        " const x = turn => box.left + 7 + (turn - s.min) / (s.max - s.min) * (box.width - 14);"
        f" const all = document.querySelectorAll('{LANE} .dbg-by s'), stock = all[all.length - 2].getBoundingClientRect();"
        " const name = all[all.length - 1].querySelector('b').getBoundingClientRect();"
        f" const pill = document.querySelector('{LANE} .dbg-cursor b').getBoundingClientRect();"
        f" return [stock.left - x({last}), stock.right - x({last + 1}), name.left >= pill.right || name.right <= pill.left,"
        " name.width]; })()")
    assert abs(stands[0]) < 1.5 and abs(stands[1]) < 1.5, stands
    assert stands[2] and stands[3] > 20, f"the last name stands under the marker: {stands}"
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

    # The runs here are played from the last turn played, so each turn of
    # them is a step: a run from an earlier turn walks the kept turns first
    # (`check_a_run_takes_the_turns_already_played`).
    bar.type_turn(bar.ev("Math.max(...state.debug.turns)"))
    bar.idle("state.turn === state.debug.last", "the last turn played is open")
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

    # Auto-play, set to Indefinite, plays until the engine says a breakpoint
    # stopped it.
    turns = "document.getElementById('dbg-turns')"
    request = typed("turns", str(turn + 3))
    assert request["turns"] == [turn + 3], request
    bar.ev(f"{turns}.value = 'indefinite', true")
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

    # Era picked: auto-play plays until a new era begins with no breakpoint
    # ticked, and counts the turns as it goes, there being no number asked.
    era_at = int(bar.ev("state.turn")) + 4
    bar.ev(f"{turns}.value = 'era', engine.eraAt = {era_at}, true")
    bar.forget()
    bar.click('[data-dbg="autoplay"]')
    bar.idle(f"state.turn === {era_at}", "auto-play picked to an era plays until the era begins")
    assert [call["op"] for call in bar.sent()] == ["step"] * 4, bar.sent()
    assert bar.note() == f"Stopped on turn {era_at}: the Medieval era began", bar.note()
    bar.ev(f"engine.eraAt = null, {turns}.value = 'indefinite', true")

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
    bar.until(f"debugRunning && state.turn > {era_at + 5}", "auto-play with no breakpoint keeps playing")
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
        seat_the_timeline(bar, name)
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


def check_a_run_takes_the_turns_already_played(bar: Bar) -> None:
    """Forward, back, forward again: the turns already played are not played again.

    Why this exists: on 2026-11-21 the operator played 50 turns on, went back
    20 and played 30 on, and the first 20 of those were played a second time
    although every one of them was kept. A run now asks the engine to walk
    to the latest kept turn on its way and plays on from there.
    """
    ops = lambda: [(call["op"], call.get("turn")) for call in bar.sent()]
    pick = lambda turns: bar.ev(f"document.getElementById('dbg-turns').value = '{turns}', true")

    def auto_play(goal: int, why: str) -> None:
        bar.forget()
        bar.click('[data-dbg="autoplay"]')
        bar.idle(f"state.turn === {goal}", why)

    bar.type_turn(bar.ev("Math.max(...state.debug.turns)"))
    bar.idle("state.turn === state.debug.last && !state.debug.beyond", "the last turn played is open")
    start = bar.ev("state.turn")
    pick(5)
    auto_play(start + 5, "five turns are played")
    assert ops() == [("step", None)] * 5 and bar.ev("state.debug.played_ahead") is None, ops()

    # Back three and five on: three of the five are kept, and two are played.
    bar.drag("dbg-slider", start + 2)
    bar.idle(f"state.turn === {start + 2}", "the timeline is walked back")
    assert bar.ev("state.debug.played_ahead") == start + 5
    score = bar.ev("engine.score")
    auto_play(start + 7, "Auto-play takes the kept turns and plays the rest")
    assert ops() == [("walk", start + 7)] + [("step", None)] * 2, ops()
    assert bar.sent()[0] == {"op": "walk", "turn": start + 7, "era": False, "strategy": "jack"}, bar.sent()
    assert bar.ev("engine.score") == score + 2, "a kept turn was played again"
    assert bar.note() == f"Played to turn {start + 7}", bar.note()

    # A run that ends on a kept turn plays nothing, and the turns past it stay.
    bar.drag("dbg-slider", start + 1)
    bar.idle(f"state.turn === {start + 1}", "the timeline is walked back")
    pick(3)
    auto_play(start + 4, "Auto-play to a kept turn goes to it")
    assert ops() == [("walk", start + 4)] and bar.ev("engine.score") == score + 2, ops()
    assert bar.note() == f"Played to turn {start + 4}", bar.note()
    assert bar.ev("[state.debug.last, state.debug.played_ahead]") == [start + 7, start + 7]

    # A breakpoint on a kept turn stops the walk there, as it stopped the run.
    bar.ev(f"engine.breaks = [{start + 5}], render(engineState()), true")
    pick(5)
    auto_play(start + 5, "the walk stops on a breakpoint's turn")
    assert ops() == [("walk", start + 9)], ops()
    assert bar.note() == f"Stopped at turn {start + 5}", bar.note()
    bar.ev("engine.breaks = [], render(engineState()), true")

    # The turn box, typed past the last turn played from an earlier one.
    bar.forget()
    bar.type_turn(start + 9)
    bar.idle(f"state.turn === {start + 9}", "the turn box takes the kept turns and plays the rest")
    assert ops() == [("walk", start + 9)] + [("step", None)] * 2, ops()
    assert bar.ev("engine.score") == score + 4, "a kept turn was played again"

    # A turn with something done in it by hand is not the turn that was
    # played: the Civ AI plays on from it, and no kept turn is taken.
    bar.drag("dbg-slider", start + 6)
    bar.idle(f"state.turn === {start + 6}", "the timeline is walked back")
    bar.ev("engine.acted = 1, render(engineState()), true")
    assert bar.ev("state.debug.played_ahead") is None
    pick(3)
    auto_play(start + 9, "Auto-play plays on from a turn that was touched")
    assert ops() == [("step", None)] * 3, ops()
    bar.ev("engine.acted = 0, render(engineState()), true")
    pick(1)


# The timeline of a split as it is drawn: whether it has branched and what the
# tag at the branch says; where each rail's sky is seen from and to, or None
# for one that is not seen; the turn each marker stands on, or None for one
# that is put away; and where on the page a turn stands. It is a tree on its
# side, so also: the top and bottom of each lane's rail (`rows`); for each
# half the branch on its way from the trunk to the half's row (`arm`: the
# place along the timeline where it is in the row, and how steeply it gets
# there), or None while there is none; and where the tag at the branch is.
TIMELINE = (
    "(() => { const lane = key => document.querySelector(`[data-dbg-lane=\"${key}\"]`);"
    " const box = held => held.getBoundingClientRect();"
    " const sky = key => { const held = lane(key).querySelector('.dbg-sky');"
    " return getComputedStyle(held).backgroundImage === 'none' ? null : [box(held).left, box(held).right]; };"
    " const marker = key => { const held = lane(key).querySelector('.dbg-cursor');"
    " return held.hidden ? null : +held.querySelector('b').textContent; };"
    " const rows = key => { const rail = box(lane(key).querySelector('.dbg-rail')); return [rail.top, rail.bottom]; };"
    " const arm = key => { const held = lane(key).querySelector('.dbg-arm'), style = getComputedStyle(held);"
    " if (style.display === 'none') return null;"
    " const rail = lane(key).querySelector('.dbg-rail');"
    " return {to: box(rail).left + rail.clientLeft + parseFloat(style.transformOrigin), slope: new DOMMatrix(style.transform).b}; };"
    " const tag = lane('').querySelector('.dbg-fork'), slider = document.getElementById('dbg-slider-a'), rail = box(slider);"
    " const at = turn => rail.left + 7 + (turn - slider.min) / (slider.max - slider.min) * (rail.width - 14);"
    " return {forked: debugBar.classList.contains('dbg-forked'), tag: tag.hidden ? null : tag.textContent,"
    " tagged: tag.hidden ? null : [box(tag).left, box(tag).top, box(tag).bottom],"
    " sky: {trunk: sky(''), a: sky('a'), b: sky('b')}, marker: {trunk: marker(''), a: marker('a'), b: marker('b')},"
    " rows: {trunk: rows(''), a: rows('a'), b: rows('b')}, arm: {a: arm('a'), b: arm('b')},"
    " rail: [box(lane('').querySelector('.dbg-rail')).left, box(lane('').querySelector('.dbg-rail')).right],"
    " at: Object.fromEntries(TURNS.map(turn => [turn, at(turn)]))}; })()")
# How far along a branch is in its own row, and how far the trunk is from it.
RUN, RISE = 32, 26


def timeline(bar: Bar, *turns: int) -> dict:
    return bar.ev(TIMELINE.replace("TURNS", json.dumps(list(turns))))


def a_tree(seen: dict, why: str) -> None:
    """The trunk is a rail in the middle, with A's row over it and B's under, and the dark between the two rows."""
    trunk, a, b = (seen["rows"][key] for key in ("trunk", "a", "b"))
    assert a[1] - a[0] == b[1] - b[0] == trunk[1] - trunk[0] > 20, (why, seen["rows"])
    assert b[0] - a[1] >= 20, (why, seen["rows"])
    assert trunk[0] + trunk[1] == a[0] + b[1] and trunk[0] - a[0] == b[0] - trunk[0] == RISE, (why, seen["rows"])


def one_timeline(bar: Bar, turn: int, why: str) -> None:
    """The two halves are one world: one rail the length of the game, and one marker on it."""
    seen = timeline(bar)
    assert (seen["forked"], seen["tag"], seen["sky"]["a"], seen["sky"]["b"]) == (False, None, None, None), (why, seen)
    assert seen["arm"] == {"a": None, "b": None}, (why, seen)
    assert seen["marker"] == {"trunk": turn, "a": None, "b": None}, (why, seen)
    assert seen["sky"]["trunk"][0] == seen["rail"][0] and abs(seen["sky"]["trunk"][1] - seen["rail"][1]) <= 1, (why, seen)
    a_tree(seen, why)


def branched(bar: Bar, fork: int, a: int, b: int, why: str) -> None:
    """The timeline is a tree: one rail up to `fork`, and from there a branch for each half, up to A's row and down to B's."""
    seen = timeline(bar, fork)
    # The turn it branches on is said at the branch, unless both halves
    # stand on that very turn: then their marker is there, and says it.
    assert (seen["forked"], seen["tag"]) == (True, None if a == b == fork else f"⑂ {fork}"), (why, seen)
    a_tree(seen, why)
    joint = seen["at"][str(fork)]
    # The trunk ends on the turn the two share last. A branch leaves it there
    # for each half: where the trunk ends it stands where the trunk does, and
    # a little further along it is in the half's own row, where the half's
    # own rail begins.
    assert abs(seen["sky"]["trunk"][1] - joint) <= 1 and seen["sky"]["trunk"][0] == seen["rail"][0], (why, seen)
    for letter in "ab":
        arm, row = seen["arm"][letter], seen["rows"][letter]
        assert abs(arm["to"] - (joint + RUN)) <= 1 and abs(seen["sky"][letter][0] - arm["to"]) <= 1, (why, letter, seen)
        assert abs(row[0] + arm["slope"] * (joint - arm["to"]) - seen["rows"]["trunk"][0]) <= 1, (why, letter, seen)
        assert abs(seen["sky"][letter][1] - seen["rail"][1]) <= 1, (why, seen)
    # The turn it branches on is said between the two branches, where they
    # have parted.
    if seen["tagged"]:
        left, top, bottom = seen["tagged"]
        assert left >= joint + RUN and top >= seen["rows"]["a"][1] and bottom <= seen["rows"]["b"][0], (why, seen)
    # Two halves on one turn of the trunk have one marker, and a half that is
    # on a branch has its own.
    if a == b and a <= fork:
        assert seen["marker"] == {"trunk": a, "a": None, "b": None}, (why, seen)
    else:
        assert seen["marker"] == {"trunk": None, "a": a, "b": b}, (why, seen)


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
    # Branch splits the screen at once: side by side, and locked. Head to
    # head, the two halves' numbers side by side, is open with it (operator
    # 2026-11-22: "Default the stats window open"): the strip's VS button
    # puts it away and brings it back, and the choice is kept.
    bar.click('[data-dbg="split"]')
    bar.until(reported(start), "both halves say where they stand")
    assert bar.ev("[debugSplit.dir, debugSplit.locked]") == ["row", True]
    versus = "[debugSplit.versus, !debugSplit.host.querySelector('[data-dbg-versus]').hidden, localStorage.getItem('civvis-debug-versus')]"
    assert bar.ev(versus) == [True, True, None], bar.ev(versus)
    bar.click('[data-dbg-split="versus"]')
    assert bar.ev(versus) == [False, False, "0"], bar.ev(versus)
    bar.click('[data-dbg-split="versus"]')
    assert bar.ev(versus) == [True, True, "1"], bar.ev(versus)
    # The strip between the halves, end to end (operator 2026-11-22: "the ai
    # debugger center should have "A=>B" and "A<-B" or something like that.
    # Should have Drop A and Drop B over Keep, as keep isnt informing the
    # user of the action of drop"): each half's name and the button that
    # drops it at that half's end, and the two copies in the middle, both
    # written A then B with the arrow the way the copy goes.
    strip = bar.ev("[...debugSplit.host.querySelectorAll('.dbg-divider [data-dbg-split]:not([data-dbg-split^=\"versus-\"]),"
                   " .dbg-divider .dbg-pane-name')].map(held => held.dataset.dbgSplit || held.dataset.dbgName)")
    assert strip == ["a", "drop-a", "copy-a", "copy-b", "view", "flip", "versus", "drop-b", "b"], strip
    # Head to head is open as a split opens, so the strip holds all of it in
    # the window: nothing it has is past its ends, and what drops a branch
    # is on the screen.
    fits = bar.ev("(() => { const strip = debugSplit.host.querySelector('.dbg-divider'), box = strip.getBoundingClientRect();"
                  " return [...strip.querySelectorAll('.dbg-pane-name, [data-dbg-split^=\"drop-\"], [data-dbg-split^=\"copy-\"]')]"
                  ".every(held => { const at = held.getBoundingClientRect(); return at.height > 0 && at.top >= box.top && at.bottom <= box.bottom"
                  " && at.bottom <= innerHeight; }); })()")
    assert fits, "the strip between the halves runs past the window"
    words = bar.ev("['drop-a', 'copy-a', 'copy-b', 'drop-b'].map(name =>"
                   " debugSplit.host.querySelector(`[data-dbg-split=\"${name}\"]`).textContent)")
    assert words == ["Drop A", "A ⇒ B", "A ⇐ B", "Drop B"], words
    assert bar.ev("debugSplit.host.textContent.includes('Keep')") is False
    # A branch that was just made is B, and the box asks which Civ AI plays
    # it (`check_the_box_asks`). Left unanswered, B keeps the one A has.
    assert bar.ev("debugAsk") == {"kind": "seat", "letter": "b"}, bar.ev("debugAsk")

    # The host's bar stays up across the top, over the two halves, and its
    # timeline is theirs: one timeline, as the split opens, because the two
    # are the same world. Locked, as a split opens, the host's deck under it
    # is the one panel for both halves, and each half's own bar is put away.
    deck_shown = "getComputedStyle(document.querySelector('#debugbar .dbg-deck')).display !== 'none'"
    bar_shown = "'getComputedStyle(document.getElementById(\"debugbar\")).display !== \"none\"'"
    bar.until(f"debugBar.classList.contains('dbg-merged') && {deck_shown}", "the host's deck is the one panel of a locked split")
    bar.until("(() => { const top = document.getElementById('debugbar'), box = top.getBoundingClientRect();"
              " return !top.hidden && box.top === 0 && box.width === document.documentElement.clientWidth"
              " && Math.round(debugSplit.host.getBoundingClientRect().top) === Math.ceil(box.height); })()",
              "the split stands under the host's bar")
    shown = lambda lane: bar.ev(f"getComputedStyle(document.querySelector('[data-dbg-lane=\"{lane}\"]')).display") != "none"
    assert (shown(""), shown("a"), shown("b")) == (True, True, True)
    bar.until("debugSplit.panes.a.marks && debugSplit.panes.b.marks", "both halves say what they hold")
    one_timeline(bar, start, "a split opens on one timeline")
    # Each half's slider has the width of the window to itself, between the
    # timeline's two ends, which are named once.
    wide = bar.ev("['a', 'b'].map(letter => document.getElementById('dbg-slider-' + letter).getBoundingClientRect().width)")
    assert min(wide) > bar.ev("innerWidth") * 0.6, wide
    ends = ("(key => { const lane = document.querySelector(`[data-dbg-lane=\"${key}\"]`);"
            " return [lane.querySelector('.dbg-start'), lane.querySelector('.dbg-finish')]"
            ".map(end => end.offsetParent === null ? null : end.textContent); })")
    assert bar.ev(f"['', 'a', 'b'].map({ends})") == [["1", "250"], [None, None], [None, None]]
    # The timeline stands in the middle, between the two rows the halves'
    # sliders stand in, A's over B's (`a_tree`), and is not itself pressed:
    # a press on its top half is a press on A's slider, and one on its
    # bottom half a press on B's.
    assert bar.ev("getComputedStyle(document.querySelector('[data-dbg-lane=\"\"] .dbg-rail')).pointerEvents") == "none"
    pressed = bar.ev("(() => { const box = document.querySelector('[data-dbg-lane=\"\"] .dbg-rail').getBoundingClientRect();"
                     " const x = box.left + box.width / 2;"
                     " return [box.top + 4, box.bottom - 4].map(y => document.elementFromPoint(x, y).id); })()")
    assert pressed == ["dbg-slider-a", "dbg-slider-b"], pressed
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
    # Each slider is on its half's turn. The two Civ AIs played different
    # turns, so the two halves hold different worlds from the first of them:
    # the timeline is one up to the turn of the split and branches there,
    # each branch drawn in its half's own colour.
    assert bar.ev(lanes) == [start + 4, start + 4], bar.ev(lanes)
    assert bar.ev("[debugSplit.panes.a.first, debugSplit.panes.b.last]") == [1, start + 4]
    assert bar.ev("['a', 'b'].every(letter => !document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-own`).hidden)")
    bar.until("debugBar.classList.contains('dbg-forked')", "the timeline branches once the halves hold different worlds")
    branched(bar, start, start + 4, start + 4, "two Civ AIs played from the split")
    assert bar.ev("debugSplitFork()") == {"turn": start, "forked": True, "first": {"a": 1, "b": 1},
                                          "last": {"a": start + 4, "b": start + 4}}

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
    # They stand on the trunk, on the one turn, and still hold the turns each
    # played: the branches stay, and the two have one marker between them.
    branched(bar, start, start, start, "both halves walked back to where the timeline branches")
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
    branched(bar, start, start + 6, start + 4, "each half on its own branch")
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

    branched(bar, start, start + 10, start + 10, "both halves played on, each on its branch")
    # A ⇒ B makes B a copy of A, its line in the chart included. The two are
    # one world again, turn for turn, and the timeline is one again.
    bar.click('[data-dbg-split="copy-a"]')
    bar.until("debugSplit.panes.b.numbers.score === debugSplit.panes.a.numbers.score", "B becomes a copy of A")
    assert ops("b")[-1][0] == "copy", ops("b")
    bar.until("!debugBar.classList.contains('dbg-forked')", "a copy holds the worlds it was copied from")
    one_timeline(bar, start + 10, "B was made a copy of A")

    # Drop A: the split closes and the host plays on from the other half, B.
    kept = bar.ev("debugSplit.panes.b.id")
    bar.forget()
    bar.click('[data-dbg-split="drop-a"]')
    bar.until("!debugSplit", "the split closes")
    assert bar.sent() == [{"op": "adopt", "pane": kept}], bar.sent()
    assert bar.ev("state.turn") == start + 10
    assert bar.note() == "Branch A closed · B is the timeline", bar.note()
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
    bar.click('[data-dbg-split="drop-b"]')
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
    seat_the_timeline(bar, "jack")
    bar.idle("engine.seat === 'jack'", "the Civ AI picked in the box is said to the engine")
    start = bar.ev("state.turn")
    bar.click('[data-dbg="split"]')
    bar.until(f"debugSplit && {both(start, start)} && debugSplit.panes.a.numbers && debugSplit.panes.b.numbers",
              "both halves say where they stand")
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy]") == ["jack", "jack"]
    a, b = bar.ev("[debugSplit.panes.a.id, debugSplit.panes.b.id]")
    bar.until(f"debugBar.classList.contains('dbg-merged') && {deck_idle}", "the one panel is up")

    # The lock joins the two halves: it stands between the middles of their
    # two rows, shut, and the two are one timeline with one marker on it.
    bar.until("debugSplit.panes.a.marks && debugSplit.panes.b.marks", "both halves say what they hold")
    one_timeline(bar, start, "a split opens on one timeline")
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
    # They are one world still, and one timeline.
    bar.until(f"debugSplit.panes.b.marks.turns.includes({start + 1})", "B says it holds the turn its twin played")
    one_timeline(bar, start + 1, "twins were played a turn")

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
    # That turn is two worlds: the timeline branches on the turn before it,
    # the last one the halves hold as one, and says so there.
    bar.until("debugBar.classList.contains('dbg-forked')", "the timeline branches where the two worlds differ")
    branched(bar, start + 1, start + 2, start + 2, "another Civ AI played B's turn")
    hover = bar.ev("debugTurnWords(debugLaneModels.b, debugLaneModels.b.branches)")
    assert hover.endswith("the last turn A and B are the same world"), hover

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
    # The wheel over the timeline walks them as it walks a whole screen's:
    # back along the trunk, and on again.
    bar.wheel('[data-dbg-lane="b"] .dbg-rail input', -100)
    bar.until(f"{both(start, start)} && {deck_idle}", "the wheel over the timeline walks both halves back")
    bar.wheel('[data-dbg-lane="a"] .dbg-rail input', 100)
    bar.until(f"{both(start + 1, start + 1)} && {deck_idle}", "the wheel over the timeline walks both halves on")
    forget()
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
    assert calls("a") == [["/debug", "forward", b]] and calls("b") == [["/debug", "step", None]], (calls("a"), calls("b"))
    assert strategies("b") == ["jack"], strategies("b")
    bar.until("!debugSplit.panes.b.marks.beyond", "B has played over the turns that were another Civ AI's")
    # Auto-play on the one panel counts A's turns, and the lanes show where it
    # is headed. Set to one turn and pressed twice, it plays two.
    bar.ev("(() => { const turns = document.getElementById('dbg-turns'); turns.value = '1';"
           " turns.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    for turn in (start + 3, start + 4):
        bar.click('[data-dbg="autoplay"]')
        bar.until(f"{both(turn, turn)} && {deck_idle}", "Auto-play on the one panel plays both halves")
    assert bar.note() == f"Played to turn {start + 4}", bar.note()

    # B went back onto the trunk and played its turns again with the Civ AI A
    # seats. The toy's worlds are then A's turn for turn, and where two
    # halves hold the same worlds there is one timeline, however each came
    # by them. (They are still not twins: the layers and the fog differ.)
    bar.until(f"!debugBar.classList.contains('dbg-forked') && debugSplit.panes.b.marks.turns.includes({start + 4})",
              "halves that hold the same worlds again are one timeline again")
    one_timeline(bar, start + 4, "B played its turns again as A played them")
    # A -> B makes the two the same in everything, and they are played once again.
    bar.click('[data-dbg-split="copy-a"]')
    bar.until(same_score, "B becomes a copy of A")
    one_timeline(bar, start + 4, "B was made a copy of A")
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
    # A has played a turn B has not, by itself: that turn is A's own, and the
    # timeline forks on the last turn the two hold as one, with A's turn on
    # A's branch in A's colour and each half's marker where the half stands.
    bar.until(f"debugSplit.panes.a.marks.turns.includes({start + 6}) && debugBar.classList.contains('dbg-forked')",
              "A says it holds the turn it played")
    branched(bar, start + 5, start + 6, start + 5, "A played on alone")
    own = bar.ev("['a', 'b'].map(letter => { const band = document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-own`);"
                 " return !band.hidden && band.getBoundingClientRect().width > 0"
                 " && getComputedStyle(band.parentNode).visibility === 'visible'; })")
    assert own == [True, False], own
    assert bar.ev("debugSplitFork()")["turn"] == start + 5
    # Apart, the lock cannot close; B changes a layer of its own meanwhile.
    assert bar.ev(f"[{lock}.disabled, {lock}.getAttribute('aria-pressed')]") == [True, "false"]
    assert "same turn" in bar.ev(f"{lock}.title"), bar.ev(f"{lock}.title")
    bar.ev(half("b", json.dumps("document.querySelector('[data-dbg-pop=\"layers\"]').click(), true")))
    bar.ev(half("b", json.dumps("document.querySelector('#dbg-pop-layers [data-dbg-layer=\"cities\"]').click(), true")))
    bar.until(half("b", "'state.debug.layers[1].on'"), "B switches a layer on by itself")
    bar.ev(half("b", json.dumps("document.querySelector('[data-dbg=\"next\"]').click(), true")))
    bar.until(f"{both(start + 6, start + 6)} && !{lock}.disabled", "the lock is offered on the same turn")
    # B played the same turn by itself, with the same Civ AI, and holds the
    # world A does: one timeline, though nothing joins the two halves.
    bar.until(f"debugSplit.panes.b.marks.turns.includes({start + 6})", "B says it holds the turn it played")
    one_timeline(bar, start + 6, "each half played the same turn by itself")
    # Locked again, B is made the same in what the one panel shows: A's layers.
    forget()
    bar.click('[data-dbg-lock]')
    bar.until(f"debugSplit.locked && debugBar.classList.contains('dbg-merged') && {deck_idle}", "the one panel is back")
    bar.until(half("b", "'state.debug.layers.map(layer => layer.on).join()'") + " === 'true,false'",
              "B takes A's layers when the two are locked")
    assert calls("a") == [] and calls("b") == [["/debug", "layers", None]], (calls("a"), calls("b"))
    assert [bar.ev(half(letter, bar_shown)) for letter in "ab"] == [False, False]

    # The timeline shows what stands on each half's: its breakpoint turns and
    # its save points. A half's own timeline is put away in a split, so the
    # host's is the only place to see them, and a mark is its half's own to
    # go to. Locked, the other half is brought along, and what both halves
    # hold on the trunk is said once: a save point at the top of the
    # trunk, drawn by A's lane, and a breakpoint turn at its foot, by B's.
    mark = "document.querySelector('#debugbar [data-dbg-lane=\"{}\"] .dbg-marks {}')".format
    stop = f'i.stop[data-dbg-turn="{start + 16}"]'
    flag = lambda name: f'i[title^="{name}:"]'

    def point(name: str) -> None:
        bar.ev(f"document.querySelector('#dbg-pop-saves [data-dbg-field=pointname]').value = '{name}', true")
        bar.click('[data-dbg="point"]')
        bar.until(f"{mark('a', flag(name))} && !{mark('b', flag(name))} && debugSplit.panes.b.marks.flags.some("
                  f"point => point.name === '{name}') && {deck_idle}",
                  f"the save point {name}, set on the one panel, is both halves' and stands once on the timeline")

    point("fork")
    bar.click('[data-dbg="break-plus"]')
    bar.until(f"!{mark('a', stop)} && {mark('b', stop)} && debugSplit.panes.a.marks.stops.includes({start + 16})"
              f" && {deck_idle}", "a breakpoint turn is both halves' and stands once on the timeline")
    assert "play to it" in bar.ev(f"{mark('b', stop)}.title"), bar.ev(f"{mark('b', stop)}.title")
    bar.ev(f"{mark('b', stop)}.click(), true")
    bar.until(f"{both(start + 16, start + 16)} && {deck_idle}", "a breakpoint clicked on the timeline is played to by both halves")
    one_timeline(bar, start + 16, "both halves were played to the breakpoint as one")
    # A save point and a breakpoint on the same turn stand at the two edges
    # of the timeline, the save point over the breakpoint, as on a whole screen's.
    point("there")
    apart = bar.ev(f"(() => {{ const box = held => held.getBoundingClientRect(), pin = box({mark('a', flag('there'))}),"
                   f" dot = box({mark('b', stop)}), sky = box(document.querySelector('[data-dbg-lane=\"\"] .dbg-sky'));"
                   " return [pin.bottom, dot.top, Math.abs(pin.left - dot.left), sky.top - pin.top, dot.bottom - sky.bottom]; })()")
    assert apart[0] <= apart[1] and apart[2] < 1 and 0 < apart[3] < 6 and 0 < apart[4] < 6, apart
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
    # Unlocked, each half's marks stand in its own row, the ones both hold
    # too: each is its own half's to go to.
    assert bar.ev(f"!!{mark('a', stop)} && !!{mark('b', stop)} && !!{mark('a', flag('fork'))} && !!{mark('b', flag('fork'))}")
    # The two halves are one world still, so the trunk is the whole timeline
    # and the save point both hold is on it. Each half's stands on the trunk
    # there and not in the half's row, and the two keep a little apart, A's
    # over B's, so that either is pressed.
    apart = bar.ev(f"(() => {{ const box = held => held.getBoundingClientRect(), a = {mark('a', flag('fork'))}, b = {mark('b', flag('fork'))};"
                   " const hit = held => document.elementFromPoint((box(held).left + box(held).right) / 2, (box(held).top + box(held).bottom) / 2);"
                   " const trunk = box(document.querySelector('[data-dbg-lane=\"\"] .dbg-rail'));"
                   " return [box(b).top - box(a).top, hit(a) === a, hit(b) === b,"
                   " box(a).bottom > trunk.top && box(a).top < trunk.top, box(b).top >= trunk.top && box(b).bottom < trunk.bottom]; })()")
    assert apart == [10, True, True, True, True], apart
    # A half's marker stays in the half's own row, A's over the trunk and
    # B's under it, with a pin from it to the tree where the tree is not in
    # that row: here, on the trunk.
    assert not bar.ev("debugBar.classList.contains('dbg-forked')")
    pins = bar.ev("['a', 'b'].map(letter => { const cursor = document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-cursor`);"
                  " const pill = cursor.querySelector('b').getBoundingClientRect(), row = cursor.closest('.dbg-rail').getBoundingClientRect();"
                  " return [+cursor.querySelector('b').textContent, cursor.classList.contains('dbg-pinned'),"
                  " getComputedStyle(cursor, '::before').height, pill.top >= row.top - 2 && pill.bottom <= row.bottom + 2]; })")
    assert pins == [[start + 6, True, f"{RISE}px", True], [start + 7, True, f"{RISE}px", True]], pins
    forget()
    bar.ev(f"{mark('b', flag('fork'))}.click(), true")
    bar.until(both(start + 6, start + 6), "a save point clicked on B's lane takes B alone to it")
    bar.until(half("b", "'!debugBusy'"), "B is done")
    assert calls("a") == [] and calls("b") == [["/debug", "goto_save", None]], (calls("a"), calls("b"))

    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the split closes")
    assert bar.ev(deck_shown) and not bar.ev("debugBar.classList.contains('dbg-merged')")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_a_branch_forks_the_timeline(bar: Bar) -> None:
    """Branch under the marker, a fork on the turn branched on, an ✕ on each branch, and whose play each stretch was.

    Operator, 2026-11-21: "each branch should have an X on the left side.
    collapsing a branch should reduce the tree appropriately. a branch should
    fork the tree appropriate (1 timeline into 2 at the branch). if the
    initial is deleted though, the branchbecomes the main tree, transitioning
    from one ai strat to another. lets label ai strats on the timeline too".
    And 2026-11-22: "lets label the branches themselves", "move x deletes
    over to the right side please, right after each closable timeline ends".
    """
    half = "debugSplit.panes.{}.frame.contentWindow.eval({})".format
    ops = lambda letter: bar.ev(half(letter, json.dumps(
        "engine.sent.filter(call => call.path === '/debug' && call.body.op !== 'status').map(call => call.body.op)")))
    both = "debugSplit.panes.a.turn === {} && debugSplit.panes.b.turn === {}".format
    deck_idle = "!!debugSplitDeck() && debugSplitDeck().idle && !debugSplit.busy"
    said = "debugSplit.panes.a.marks && debugSplit.panes.b.marks"
    names = lambda lane: bar.ev(f"[...document.querySelectorAll('[data-dbg-lane=\"{lane}\"] .dbg-deco .dbg-by s')].map(s => s.textContent)")
    shown = lambda name: bar.ev(f"document.querySelector({json.dumps(name)}).offsetParent !== null")

    # Jack plays six turns past the last one played, and the world is walked
    # four of them back: that is where the timeline is branched.
    seat_the_timeline(bar, "jack")
    bar.idle("engine.seat === 'jack'", "Jack holds the seat")
    # Nothing an earlier check left behind stops the run.
    bar.ev("debugOp({op: 'breakpoints'}), true")
    bar.idle("state.debug.breakpoints.turns.length === 0", "the breakpoints are cleared")
    bar.type_turn(bar.ev("Math.max(state.debug.last, state.turn)"))
    bar.idle("state.turn === Math.max(state.debug.last, state.turn)", "the world stands on the last turn played")
    end = bar.ev("state.turn") + 6
    bar.drag("dbg-slider", end)
    bar.idle(f"state.turn === {end}", "six turns are played on")
    back = end - 4
    bar.drag("dbg-slider", back)
    bar.idle(f"state.turn === {back} && state.debug.last === {end}", "the world is walked four turns back")
    assert shown("#debugbar [data-dbg=split]") and shown("#debugbar .dbg-who")
    bar.click('[data-dbg="split"]')
    bar.until(f"debugSplit && {both(back, back)} && {said} && {deck_idle}", "both branches say where they stand")
    # Two branches are the most there are: a split has no Branch, and each
    # branch's Civ AI is picked at the head of its lane.
    assert not shown("#debugbar [data-dbg=split]") and not shown("#debugbar .dbg-who")

    # A is the timeline as it was and keeps the turns it holds past the open
    # one. B is the branch: it is told that it begins here, and holds none.
    bar.until(f"Math.max(...debugSplit.panes.b.marks.turns) === {back} && debugBar.classList.contains('dbg-forked')",
              "the branch holds nothing past the turn it was made on")
    assert (ops("a"), ops("b")) == ([], ["branch"]), (ops("a"), ops("b"))
    # So the timeline forks there at once: one rail up to that turn, A's from
    # it with the turns A holds on it, and B's from it with nothing on it yet.
    branched(bar, back, back, back, "a branch made behind the last turn played")
    assert bar.ev("debugSplitFork()") == {"turn": back, "forked": True, "first": {"a": 1, "b": 1},
                                          "last": {"a": end, "b": back}}
    own = ("['a', 'b'].map(letter => { const band = document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-own`);"
           " return !band.hidden && band.getBoundingClientRect().width > 0; })")
    assert bar.ev(own) == [True, False], bar.ev(own)
    # The timeline is a tree on its side (operator, 2026-11-22: "the branching
    # should form more of a distinct y shape. or like 4 or h like (the one can
    # be between the 2). but more of a tree like timeline please"). `branched`
    # has looked at its shape: the trunk in the middle, and from the fork a
    # branch up to A's row and down to B's. What is drawn on a half's rail is
    # drawn over again on its branch's way there, all but the words: the
    # turns A holds past the fork are on A's, and B's has none.
    bent = bar.ev("['a', 'b'].map(letter => { const arm = document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-arm`);"
                  " const band = arm.querySelector('.dbg-bent .dbg-own');"
                  " return [!!band && !band.hidden, arm.querySelectorAll('b').length,"
                  " getComputedStyle(arm).clipPath.startsWith('polygon('), getComputedStyle(arm).pointerEvents]; })")
    assert bent == [[True, 0, True, "none"], [False, 0, True, "none"]], bent
    # A press on a branch is a press on that half's slider.
    pressed = bar.ev("['a', 'b'].map(letter => { const box = document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-rail`)"
                     ".getBoundingClientRect(); return document.elementFromPoint(box.right - 40, (box.top + box.bottom) / 2).id; })")
    assert pressed == ["dbg-slider-a", "dbg-slider-b"], pressed
    # The turns are numbered once for the whole tree, beside its rails: over
    # the trunk up to the turn it forks on, and between the two branches
    # from there.
    numbers = bar.ev("(() => { const box = held => held.getBoundingClientRect();"
                     " const rail = key => box(document.querySelector(`[data-dbg-lane=\"${key}\"] .dbg-rail`));"
                     " return {inside: [...document.querySelectorAll('#debugbar .dbg-ruler b')].filter(held => held.offsetParent !== null).length,"
                     " axis: [...document.querySelectorAll('#debugbar .dbg-axis b')].map(held => [+held.textContent, box(held).top, box(held).bottom]),"
                     " trunk: rail('').top, a: rail('a').bottom, b: rail('b').top}; })()")
    assert numbers["inside"] == 0 and len(numbers["axis"]) >= 3, numbers
    for turn, top, bottom in numbers["axis"]:
        assert turn % 50 == 0, numbers
        assert bottom <= numbers["trunk"] if turn <= back else numbers["a"] <= top and bottom <= numbers["b"], (back, turn, numbers)
    # Who played what is named where it stands: on the trunk, and on A's
    # branch. A branch is named on itself, its letter before the name on its
    # first stretch, and B's, which nothing has been played on yet, by the
    # Civ AI seated in it: the one that will play it.
    assert names("")[-1] == "Jack" and names("a") == ["A · Jack"] and names("b") == ["B · Jack"], (names(""), names("a"), names("b"))
    # A branch's name stands on the branch: in its own row, past where the
    # branch has reached it.
    for letter in "ab":
        name, rail, at = bar.ev(
            f"(() => {{ const lane = document.querySelector('[data-dbg-lane=\"{letter}\"]');"
            " const box = held => { const b = held.getBoundingClientRect(); return [b.left, b.right, b.top, b.bottom]; };"
            " const slider = lane.querySelector('input').getBoundingClientRect();"
            f" return [box(lane.querySelector('.dbg-deco .dbg-by b')), box(lane.querySelector('.dbg-rail')),"
            f" slider.left + 7 + ({back} - 1) / (+lane.querySelector('input').max - 1) * (slider.width - 14)]; }})()")
        assert name[1] - name[0] > 20 and name[0] >= at + RUN - 1 and rail[2] <= name[2] and name[3] <= rail[3], (letter, name, rail, at)

    # Each branch has an ✕ after its right end: its letter and its picker
    # stand before its rail, the ✕ past it, and the game's last turn, which
    # is said once for the tree, past the ✕.
    for letter in "ab":
        places = bar.ev(
            f"['[data-dbg-close=\"{letter}\"]', '[data-dbg-lane=\"{letter}\"] .dbg-lane-name', '[data-dbg-agent=\"{letter}\"]',"
            f" '[data-dbg-lane=\"{letter}\"] .dbg-rail', '{LANE} .dbg-finish'].map(name => {{"
            " const box = document.querySelector('#debugbar ' + name).getBoundingClientRect();"
            " return [box.left, box.right, box.top, box.bottom]; })")
        cross, name, picker, rail, finish = places
        assert name[1] <= picker[0] and picker[1] <= rail[0], (letter, places)
        assert cross[1] > cross[0] and rail[1] <= cross[0] and cross[0] - rail[1] <= 12 and cross[1] <= finish[0], (letter, places)
        assert rail[2] <= (cross[2] + cross[3]) / 2 <= rail[3], (letter, places)
        assert bar.ev(f"document.querySelector('[data-dbg-close=\"{letter}\"]').textContent") == "✕"
    # In the page as it is written the ✕ follows its rail too, so the keys
    # reach it where the eye finds it.
    order = bar.ev("['a', 'b'].map(letter => [...document.querySelector(`[data-dbg-lane=\"${letter}\"]`).children]"
                   ".map(held => held.className.split(' ')[0]))")
    assert order == [["dbg-lane-name", "dbg-agent", "dbg-start", "dbg-rail", "dbg-close", "dbg-finish"]] * 2, order

    # Locked, one turn on: A walks to the turn it holds, and B, which holds
    # none, plays it. Jack plays B's seat too, so the toy's two worlds are the
    # same on that turn, and the fork stands one turn further on.
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(back + 1, back + 1)} && {deck_idle} && debugSplitFork().turn === {back + 1}",
              "both branches go one turn on")
    assert (ops("a"), ops("b")) == (["forward"], ["branch", "step"]), (ops("a"), ops("b"))
    branched(bar, back + 1, back + 1, back + 1, "the branch played the turn the timeline holds, the same way")

    # Another Civ AI in the branch, and one more turn: two worlds from there.
    bar.ev("(() => { const select = document.querySelector('#debugbar [data-dbg-agent=\"b\"]'); select.value = 'stock';"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    bar.until(half("b", "'engine.seat'") + f" === 'stock' && {deck_idle}", "Stock holds the branch's seat")
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(back + 2, back + 2)} && {deck_idle} && debugSplit.panes.b.marks.turns.includes({back + 2})",
              "both branches go another turn on")
    assert (ops("a"), ops("b")) == (["forward"] * 2, ["branch", "step", "step"]), (ops("a"), ops("b"))
    branched(bar, back + 1, back + 2, back + 2, "another Civ AI played the branch's turn")
    assert names("a") == ["A · Jack"] and names("b") == ["B · Stock"], (names("a"), names("b"))
    words = bar.ev(f"debugTurnWords(debugLaneModels.b, {back + 2})")
    assert words == f"B · turn {back + 2} · open now · Stock", words
    # A name on a branch does not stand under the branch's marker.
    clear = bar.ev("['a', 'b'].map(letter => { const lane = document.querySelector(`[data-dbg-lane=\"${letter}\"]`);"
                   " const name = lane.querySelector('.dbg-by b').getBoundingClientRect();"
                   " const pill = lane.querySelector('.dbg-cursor b').getBoundingClientRect();"
                   " return name.width > 20 && (name.left >= pill.right || name.right <= pill.left); })")
    assert clear == [True, True], clear

    # The ✕ on A closes the timeline as it was, and the branch is the
    # timeline: Jack's turns up to the fork, and Stock's from it.
    kept = bar.ev("debugSplit.panes.b.id")
    bar.forget()
    bar.click('[data-dbg-close="a"]')
    bar.until("!debugSplit", "the ✕ on A closes the split")
    assert bar.sent() == [{"op": "adopt", "pane": kept}], bar.sent()
    assert bar.note() == "Branch A closed · B is the timeline", bar.note()
    assert bar.ev("[state.turn, state.debug.last]") == [back + 2, back + 2]
    assert bar.ev("state.debug.played_by.slice(-1)") == [{"from": back + 1, "to": back + 2, "by": "stock"}]
    assert bar.ev("state.debug.played_by.slice(-2)[0].by") == "jack" and bar.ev("state.debug.played_by.slice(-2)[0].to") == back + 1
    assert names("")[-2:] == ["Jack", "Stock"], names("")
    assert bar.ev("document.getElementById('dbg-agent').value") == "stock"
    assert shown("#debugbar [data-dbg=split]") and shown("#debugbar .dbg-who")

    # Branched on the last turn played, there is nothing to leave with A: the
    # branch is asked nothing, and the two are one timeline until they differ.
    bar.click('[data-dbg="split"]')
    bar.until(f"debugSplit && {both(back + 2, back + 2)} && {said} && {deck_idle}", "a second branch opens")
    assert (ops("a"), ops("b")) == ([], []), (ops("a"), ops("b"))
    one_timeline(bar, back + 2, "a branch made on the last turn played")
    # Played on, the two are told of each turn one after the other, so one is
    # a turn ahead for a moment every turn. That is not a fork, and the
    # timeline is never drawn as one: every change of it is counted here.
    bar.ev("(() => { window.flips = 0; let was = debugBar.classList.contains('dbg-forked');"
           " new MutationObserver(() => { const now = debugBar.classList.contains('dbg-forked');"
           " if (now !== was) { was = now; window.flips += 1; } }).observe(debugBar, {attributes: true, attributeFilter: ['class']});"
           " return true; })()")
    for turn in (back + 3, back + 4):
        bar.click('[data-dbg="next"]')
        bar.until(f"{both(turn, turn)} && {deck_idle} && debugSplit.panes.b.marks.turns.includes({turn})",
                  "both branches are played on as one")
    one_timeline(bar, back + 4, "two branches played the same way")
    assert bar.ev("window.flips") == 0, "the timeline forked for a moment while two branches were played as one"
    # The ✕ on B closes the branch, and A is the timeline as it was.
    kept = bar.ev("debugSplit.panes.a.id")
    bar.forget()
    bar.click('[data-dbg-close="b"]')
    bar.until("!debugSplit", "the ✕ on B closes the split")
    assert bar.sent() == [{"op": "adopt", "pane": kept}], bar.sent()
    assert bar.note() == "Branch B closed · A is the timeline", bar.note()
    assert bar.ev("[state.turn, state.debug.last]") == [back + 4, back + 4]
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_the_box_asks(bar: Bar) -> None:
    """What the box asks: Switch or Branch for another Civ AI on the one timeline, and which Civ AI plays a new branch.

    Operator, 2026-11-22: "after this, user should be prompted for the
    strategy for the second branch", "editable branches should be able to
    switch strategies. leading strategies should be editable and when edited
    to a different strategy, should ask to switch or branch".
    """
    half = "debugSplit.panes.{}.frame.contentWindow.eval({})".format
    both = "debugSplit.panes.a.turn === {0} && debugSplit.panes.b.turn === {0}".format
    deck_idle = "!!debugSplitDeck() && debugSplitDeck().idle && !debugSplit.busy"
    said = "debugSplit.panes.a.marks && debugSplit.panes.b.marks"
    seated = "[document.getElementById('autoplaystrategy').value, document.getElementById('dbg-agent').value]"
    asked = ("(() => { const box = document.getElementById('dbg-ask'); if (box.hidden || box.offsetParent === null) return null;"
             " const at = box.getBoundingClientRect();"
             " return {title: box.querySelector('h4').textContent,"
             " buttons: [...box.querySelectorAll('button')].map(held => [held.dataset.dbgAsk, held.value, held.textContent, held.classList.contains('on')]),"
             " left: at.left, top: at.top, right: at.right, bottom: at.bottom}; })()")
    under = ("(name => { const from = document.querySelector(name).getBoundingClientRect(), box = document.getElementById('dbg-ask').getBoundingClientRect();"
             " return box.top >= from.bottom && box.top - from.bottom <= 12 && Math.abs(box.left - from.left) <= 1"
             " && box.right <= innerWidth && box.bottom <= innerHeight; })")

    def pick(name: str) -> None:
        bar.ev(f"(() => {{ const select = document.getElementById('dbg-agent'); select.value = '{name}';"
               " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")

    seat_the_timeline(bar, "jack")
    bar.idle("engine.seat === 'jack'", "Jack holds the seat")
    turn = bar.ev("state.turn")
    assert bar.ev(asked) is None and bar.ev("debugAsk") is None

    # Another Civ AI picked on the one timeline is asked about before
    # anything is done with it: nothing is said to the engine, the seat is
    # still Jack's, and the picker shows what the question is about.
    bar.forget()
    pick("stock")
    assert bar.ev("debugAsk") == {"kind": "switch", "name": "stock"}, bar.ev("debugAsk")
    seen = bar.ev(asked)
    assert seen["title"] == f"Stock from turn {turn}: switch or branch?", seen
    assert [button[0] for button in seen["buttons"]] == ["switch", "branch", "close"], seen
    assert [button[2] for button in seen["buttons"]] == ["Switch", "⑂ Branch", "Cancel"], seen
    assert bar.ev(f"{under}('#dbg-agent')"), seen
    assert bar.ev(seated) == ["jack", "stock"] and bar.sent() == [], (bar.ev(seated), bar.sent())
    # Cancel leaves the timeline as it was, and so does Escape, and a press
    # anywhere else on the page.
    bar.click('#dbg-ask [data-dbg-ask="close"]')
    assert bar.ev(asked) is None and bar.ev(seated) == ["jack", "jack"] and bar.sent() == [], (bar.ev(seated), bar.sent())
    pick("stock")
    assert bar.ev(asked) is not None
    bar.ev("document.querySelector('#dbg-ask button').dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true})), true")
    assert bar.ev(asked) is None and bar.ev(seated) == ["jack", "jack"] and bar.sent() == [], (bar.ev(seated), bar.sent())
    pick("stock")
    # A press on the question itself is not a press elsewhere.
    bar.ev("document.querySelector('#dbg-ask h4').dispatchEvent(new PointerEvent('pointerdown', {bubbles: true})), true")
    assert bar.ev(asked) is not None
    bar.ev("document.getElementById('maparea').dispatchEvent(new PointerEvent('pointerdown', {bubbles: true})), true")
    assert bar.ev(asked) is None and bar.ev(seated) == ["jack", "jack"] and bar.sent() == [], (bar.ev(seated), bar.sent())
    # The Civ AI already seated is nothing to ask: it is said to the engine,
    # as any pick is.
    pick("jack")
    bar.idle("engine.sent.some(call => call.path === '/debug')", "the seated Civ AI is said again")
    assert bar.ev(asked) is None and bar.sent() == [{"op": "status", "strategy": "jack"}], bar.sent()

    # Switch: this timeline plays on with it. It is seated as a pick always
    # was, said to the engine at once.
    bar.forget()
    pick("stock")
    bar.click('#dbg-ask [data-dbg-ask="switch"]')
    bar.idle("engine.seat === 'stock'", "Switch seats the Civ AI picked")
    assert bar.ev(asked) is None and bar.ev(seated) == ["stock", "stock"], bar.ev(seated)
    assert bar.sent() == [{"op": "status", "strategy": "stock"}], bar.sent()
    seat_the_timeline(bar, "jack")
    bar.idle("engine.seat === 'jack'", "Jack holds the seat again")

    # Branch: a second timeline begins here with the Civ AI picked, and this
    # one keeps its own. The branch was made for that Civ AI, so the box does
    # not ask which one plays it, and nothing is asked of this page's world.
    bar.forget()
    pick("stock")
    bar.click('#dbg-ask [data-dbg-ask="branch"]')
    bar.until(f"debugSplit && {both(turn)} && {said} && {deck_idle}", "Branch splits the timeline on the open turn")
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy, debugAsk]") == ["jack", "stock", None]
    assert bar.ev(asked) is None
    bar.until(half("a", "'debugStrategy()'") + " === 'jack' && " + half("b", "'debugStrategy()'") + " === 'stock'"
              " && " + half("b", "'engine.seat'") + " === 'stock'", "the branch is seated with the Civ AI it was made for")
    assert bar.ev("['a', 'b'].map(letter => debugLanes[letter].agent.value)") == ["jack", "stock"]
    assert [call for call in bar.sent() if call.get("op") != "adopt"] == [], bar.sent()
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the branch is dropped")
    assert bar.ev(seated) == ["jack", "jack"], bar.ev(seated)

    # A branch made by Branch begins with A's Civ AI, and the box asks which
    # one plays it: every Civ AI the page offers, as its picker lists them,
    # the one seated lit and marked as A's, under the branch's own picker.
    bar.click('[data-dbg="split"]')
    bar.until(f"debugSplit && {both(turn)} && {said} && {deck_idle}", "Branch splits the timeline")
    assert bar.ev("debugAsk") == {"kind": "seat", "letter": "b"}, bar.ev("debugAsk")
    seen = bar.ev(asked)
    assert seen["title"] == "Branch B: which Civ AI plays it?", seen
    assert seen["buttons"] == [["seat", "jack", "1211 - JackOfAllTradesplays A", True], ["seat", "stock", "1000 - ByTheBook", False]], seen
    assert bar.ev(f"{under}('#debugbar [data-dbg-agent=\"b\"]')"), seen
    # It is the box's own, and stays up while the halves are unlocked and
    # the deck is put away.
    bar.click("[data-dbg-lock]")
    bar.until("!debugSplit.locked && !debugBar.classList.contains('dbg-merged')", "the lock opens")
    assert bar.ev(asked) is not None
    bar.click("[data-dbg-lock]")
    bar.until(f"debugSplit.locked && {deck_idle}", "the lock closes again")
    # Answered, the branch is seated with the Civ AI picked, as from its picker.
    bar.click('#dbg-ask [data-dbg-ask="seat"][value="stock"]')
    bar.until(half("b", "'debugStrategy()'") + " === 'stock' && " + half("b", "'engine.seat'") + f" === 'stock' && {deck_idle}",
              "the Civ AI picked is seated in the branch")
    assert bar.ev(asked) is None and bar.ev("debugAsk") is None
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy]") == ["jack", "stock"]
    assert bar.ev("['a', 'b'].map(letter => debugLanes[letter].agent.value)") == ["jack", "stock"]
    bar.click('[data-dbg-close="b"]')
    bar.until("!debugSplit", "the branch is closed")
    # The branch's own picker answers it as well.
    bar.click('[data-dbg="split"]')
    bar.until(f"debugSplit && {both(turn)} && {said} && {deck_idle}", "Branch splits the timeline again")
    assert bar.ev(asked) is not None
    bar.ev("(() => { const select = document.querySelector('#debugbar [data-dbg-agent=\"b\"]'); select.value = 'stock';"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    bar.until(half("b", "'debugStrategy()'") + f" === 'stock' && {deck_idle}", "the branch is seated from its picker")
    assert bar.ev(asked) is None and bar.ev("debugAsk") is None
    # A question about a branch is gone with the branch.
    bar.click('[data-dbg-close="b"]')
    bar.until("!debugSplit", "the branch is closed")
    bar.click('[data-dbg="split"]')
    bar.until(f"debugSplit && {both(turn)} && {said} && {deck_idle}", "Branch splits the timeline a third time")
    assert bar.ev(asked) is not None
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the branch is dropped")
    assert bar.ev(asked) is None and bar.ev("debugAsk") is None
    assert bar.ev("[state.turn, engine.seat]") == [turn, "jack"]
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_the_game_opens_on_two_tracks(bar: Bar, address: str) -> None:
    """The opening view: a game under the debugger is drawn on its two tracks at once, and the box asks for B's Civ AI.

    Operator, 2026-11-22: "in ai debugger, the opening view should go to the
    2 track immediately. after this, user should be prompted for the strategy
    for the second branch".
    """
    both = "debugSplit.panes.a.turn === {0} && debugSplit.panes.b.turn === {0}".format
    deck_idle = "!!debugSplitDeck() && debugSplitDeck().idle && !debugSplit.busy"
    said = "debugSplit.panes.a.marks && debugSplit.panes.b.marks"
    ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"

    def settled() -> None:
        """Long enough for a page that was going to split to have done it."""
        bar.ev("new Promise(done => setTimeout(() => done(true), 250))")

    # Every other check asked for the one timeline. The page as anyone opens it:
    bar.dev.call("Page.navigate", {"url": address})
    bar.until(f"{ready} && !location.search", "the page as it is opened, loaded")
    assert bar.ev("typeof debugSplit !== 'undefined' && debugSplit === null && state === null")
    # Nothing is pressed: the first state drawn splits the screen as Branch
    # does, side by side, locked, with head to head open. The game is not
    # shown on one screen first: in the task that draws its first state the
    # window goes under a veil, and stays there until the two are shown.
    veiled = bar.ev("(() => { render(engineState()); const veil = document.getElementById('dbg-veil');"
                    " return [!!debugSplit, debugSplit.shown, debugHeld, !!veil"
                    " && veil.getBoundingClientRect().width === innerWidth && veil.getBoundingClientRect().height === innerHeight,"
                    " +getComputedStyle(veil).zIndex > +getComputedStyle(debugBar).zIndex, debugBar.classList.contains('dbg-branching')]; })()")
    assert veiled == [True, False, True, True, True, False], veiled
    bar.settled()
    assert bar.ev("[debugSplit.shown, document.getElementById('dbg-veil')]") == [True, None]
    bar.until(f"debugSplit && {both(1)} && {said} && {deck_idle}", "the game opens on its two tracks")
    assert bar.ev("[debugSplit.dir, debugSplit.locked, debugSplit.versus, debugSplit.from]") == ["row", True, True, 1]
    assert bar.ev("['', 'a', 'b'].map(key => getComputedStyle(document.querySelector(`[data-dbg-lane=\"${key}\"]`)).display !== 'none')") == [True] * 3
    one_timeline(bar, 1, "a game opens on two tracks that are one world")
    # Nothing was asked of the world that waits under the two halves.
    assert bar.sent() == [], bar.sent()
    # And the box asks which Civ AI plays B.
    assert bar.ev("debugAsk") == {"kind": "seat", "letter": "b"}, bar.ev("debugAsk")
    assert bar.ev("(() => { const box = document.getElementById('dbg-ask');"
                  " return !box.hidden && box.offsetParent !== null && box.querySelector('h4').textContent; })()") == "Branch B: which Civ AI plays it?"
    bar.click('#dbg-ask [data-dbg-ask="seat"][value="stock"]')
    bar.until("debugSplit.panes.b.frame.contentWindow.eval('engine.seat') === 'stock' && " + deck_idle, "B is seated from the question")
    assert bar.ev("debugAsk") is None
    # Played on, each track is its own Civ AI's.
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(2)} && {deck_idle} && debugBar.classList.contains('dbg-forked')", "the two tracks part")
    branched(bar, 1, 2, 2, "two Civ AIs played the first turn")
    names = bar.ev("['a', 'b'].map(key => [...document.querySelectorAll(`[data-dbg-lane=\"${key}\"] .dbg-deco .dbg-by s`)].map(s => s.textContent))")
    assert names == [["A · Jack"], ["B · Stock"]], names

    # It is the opening view, given once a game: a track dropped leaves the
    # one timeline, which is not split again when it is next drawn.
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit && state.turn === 2", "B is dropped")
    bar.ev("render(engineState()), true")
    settled()
    assert bar.ev("debugSplit === null && debugAsk === null")
    # A new game opens on its two tracks again.
    bar.ev("Object.assign(engine, {seed: 8, turn: 1, marks: [1], worlds: {1: 1}, by: {}, score: 0, eras: [{era: 0, name: 'ancient', turn: 1}]}),"
           " render(engineState()), true")
    bar.settled()
    bar.until(f"debugSplit && {both(1)} && {said} && {deck_idle}", "a new game opens on its two tracks")
    assert bar.ev("debugAsk") == {"kind": "seat", "letter": "b"}, bar.ev("debugAsk")
    bar.click('[data-dbg-split="drop-a"]')
    bar.until("!debugSplit", "A is dropped")
    assert bar.note() == "Branch A closed · B is the timeline", bar.note()

    # A game with no room for two halves opens on the one timeline, and so
    # does one whose seat is being played by hand in the Single player view.
    bar.dev.call("Emulation.setDeviceMetricsOverride", {"width": 800, "height": 900, "deviceScaleFactor": 1, "mobile": False})
    bar.until("innerWidth === 800", "a narrow window")
    bar.ev("engine.seed = 9, render(engineState()), true")
    settled()
    assert bar.ev("debugSplit === null && debugOpenedGame === '9'")
    bar.dev.call("Emulation.clearDeviceMetricsOverride")
    bar.until("innerWidth > 1000", "the window is its own size again")
    bar.ev("render(engineState()), true")
    settled()
    assert bar.ev("debugSplit === null"), "a game that opened on one timeline was split later, by a window made wider"
    bar.ev("SOLO_UI = true, engine.seed = 10, render(engineState()), true")
    settled()
    assert bar.ev("debugSplit === null && debugOpenedGame === '10'")
    bar.ev("SOLO_UI = false, true")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_one_screen_and_two_change_places_in_one_move(bar: Bar, doc: str) -> None:
    """Branch, and closing a branch, hold the window as it stands and then change it once.

    Operator, 2026-11-22: "i do not like the whole transition between ai
    debugger 1 screen and 2 screens. please clean this up a lot more". A
    half is a whole page and takes a moment to be there. Before this the
    window showed all of it: an empty split, each half loading, the box
    without its deck and then with it.
    """
    half = "debugSplit.panes.{}.frame.contentWindow.eval({})".format
    press = lambda selector: bar.ev(f"document.querySelector({json.dumps(selector)}).click(), true")
    held = ("[debugHeld, document.documentElement.classList.contains('dbg-held'),"
            " getComputedStyle(debugBar).pointerEvents]")
    one_timeline_box = ("[debugBar.classList.contains('dbg-split-on'), document.documentElement.classList.contains('dbg-split-open'),"
                        " document.getElementById('dbg-ask').hidden, debugAsk]")
    assert bar.ev("debugSplit === null && !debugHeld && !debugMoving")
    turn = bar.ev("state.turn")
    # What the split is shown with, taken down as it is shown.
    bar.ev("(() => { const show = window.realReveal = debugSplitReveal; window.shownWith = [];"
           " debugSplitReveal = function () { const {a, b} = debugSplit.panes;"
           " shownWith.push([a, b].map(pane => [pane.turn, !!pane.marks, !!pane.numbers, pane.agreed, pane.painted]).concat([!!a.deck, debugSplit.branches]));"
           " return show(); }; return true; })()")

    # The one screen is looking somewhere, and the halves will open looking there.
    bar.ev("cam.x = 111, cam.y = 222, cam.scale = 2, true")
    # The halves' own script comes late, so there is time to see the window held.
    bar.ev("window.lateScript = true")
    press('[data-dbg="split"]')
    # At once: the split is made and its halves are loading, unseen, and the
    # window is as it was. The box is still the one timeline's, takes no
    # press, asks nothing yet, and Branch says it is at work.
    unseen = "[debugSplit.shown, debugSplit.host.classList.contains('dbg-unseen'), getComputedStyle(debugSplit.host).opacity]"
    assert bar.ev("!!debugSplit") and bar.ev(unseen) == [False, True, "0"], bar.ev(unseen)
    assert bar.ev(held) == [True, True, "none"], bar.ev(held)
    assert bar.ev(one_timeline_box) == [False, False, True, None], bar.ev(one_timeline_box)
    assert bar.ev("debugBar.classList.contains('dbg-branching')")
    assert bar.ev("getComputedStyle(document.querySelector('#debugbar .dbg-branch'), '::after').content") == '"ing…"'
    # The unseen halves take no press, and nothing is asked of the world they are made from.
    assert bar.ev("[...debugSplit.host.children].every(part => getComputedStyle(part).pointerEvents === 'none')")
    assert bar.ev("send({type: 'end_turn'}).then(answer => answer === undefined)"), "a held world was asked to end its turn"
    bar.ev("new Promise(done => setTimeout(() => done(true), 150))")
    assert bar.ev("[debugSplit.shown, debugHeld, shownWith.length]") == [False, True, 0], \
        "the window was let go of before both halves were there"
    assert bar.ev(one_timeline_box) == [False, False, True, None]
    bar.ev("window.lateScript = false")
    bar.settled()
    # Shown once, and with everything a split is drawn from already there:
    # where each half stands, its timeline and its numbers, the deck of the
    # one panel, each half painted looking where it was told.
    assert bar.ev("shownWith") == [[[turn, True, True, True, True], [turn, True, True, True, True], True, False]], bar.ev("shownWith")
    assert bar.ev(unseen) == [True, False, "1"], bar.ev(unseen)
    assert bar.ev(held) == [False, False, "auto"], bar.ev(held)
    assert bar.ev("[debugBar.classList.contains('dbg-split-on'), debugBar.classList.contains('dbg-merged'),"
                  " debugBar.classList.contains('dbg-branching'), document.documentElement.classList.contains('dbg-split-open')]") \
        == [True, True, False, True]
    # And then the box asks about the branch.
    assert bar.ev("[debugAsk, document.getElementById('dbg-ask').hidden]") == [{"kind": "seat", "letter": "b"}, False]
    # Both halves look where the one screen was looking, and are let go of.
    looks = bar.ev("['a', 'b'].map(letter => debugSplit.panes[letter].frame.contentWindow.eval('[cam.x, cam.y, cam.scale]'))")
    assert looks == [[111, 222, 2], [111, 222, 2]], looks
    bar.until("['a', 'b'].every(letter => debugSplit.panes[letter].frame.contentWindow.eval('debugLookKept') === null)",
              "each half is told it is seen")
    assert bar.ev("send({type: 'end_turn'}).then(answer => answer === state)"), "the page's own End Turn is its own again"
    # Nothing is left of the move: no names for the browser to carry, no veil.
    assert bar.ev("[debugBar.style.viewTransitionName, document.getElementById('maparea').style.viewTransitionName,"
                  " debugSplit.panes.a.frame.style.viewTransitionName, document.documentElement.classList.contains('dbg-swapping'),"
                  " document.getElementById('dbg-veil')]") == ["", "", "", False, None]

    # Two screens become one the same way. A looks somewhere of its own.
    bar.ev("debugAsk = null, drawDebugBar(), true")
    bar.ev(half("a", "'cam.x = 333, cam.scale = 3, true'"))
    bar.ev("engine.wait = 150, true")
    greyed = ("[...document.querySelectorAll('#debugsplit .dbg-divider button, #debugbar button, #debugbar select')]"
              ".filter(held => held.disabled).length")
    before = bar.ev(greyed)
    press('[data-dbg-split="drop-b"]')
    # At once: held as they stand. Nothing is greyed for the moment the engine takes, and nothing takes a press.
    assert bar.ev("[debugSplit.busy, debugSplit.host.classList.contains('dbg-closing')]") == [True, True]
    assert bar.ev(held) == [True, True, "none"], bar.ev(held)
    assert bar.ev(greyed) == before, "a split being closed is greyed"
    assert bar.ev("[...debugSplit.host.children].every(part => getComputedStyle(part).pointerEvents === 'none')")
    # What a half says meanwhile draws nothing.
    bar.ev("drawDebugSplit(), drawDebugBar(), true")
    assert bar.ev(f"[debugBar.classList.contains('dbg-split-on'), {greyed}]") == [True, before]
    bar.settled()
    bar.ev("engine.wait = 0, true")
    assert bar.ev("[debugSplit, document.getElementById('debugsplit'), debugBar.classList.contains('dbg-split-on')]") == [None, None, False]
    assert bar.note() == "Branch B closed · A is the timeline", bar.note()
    # The one screen looks where the kept half was looking.
    assert bar.ev("[cam.x, cam.scale]") == [333, 3], bar.ev("[cam.x, cam.scale]")

    # A half that never says it is there is not waited on for ever: the two
    # screens are shown as they are when the wait is up.
    bar.ev("(() => { window.realReady = debugSplitReady; debugSplitReady = () => {}; shownWith.length = 0; return true; })()")
    began = time.time()
    press('[data-dbg="split"]')
    bar.ev("new Promise(done => setTimeout(() => done(true), 600))")
    assert bar.ev("[debugSplit.shown, debugHeld]") == [False, True]
    bar.until("debugSplit.shown && !debugHeld && !debugMoving", "the split is shown when the wait is up", seconds=12)
    waited = time.time() - began
    patience = bar.ev("DEBUG_SPLIT_WAIT_MS")
    assert patience / 1000 - 0.5 <= waited, waited
    # The document gives the wait the script uses.
    said = f"after {patience // 1000} s the two are shown as they are"
    assert said in doc, f"docs/SIM_DEBUGGER.md no longer says: {said}"
    bar.ev("debugSplitReady = realReady, true")
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the branch is dropped")

    # A browser that takes a move and never makes it (a window it is not
    # painting) does not keep the change, nor the word that the move is over.
    bar.ev("document.startViewTransition = () => ({finished: new Promise(() => {}), skipTransition() {}}), true")
    began = time.time()
    press('[data-dbg="split"]')
    bar.until("debugSplit && debugSplit.shown && !debugHeld", "the two screens are shown though the browser never made the move")
    # (A window that is hidden is asked for no move at all, and has none to wait on.)
    seen = not bar.ev("document.hidden")
    assert bar.ev("[debugMoving, debugBar.classList.contains('dbg-split-on'), getComputedStyle(debugSplit.host).opacity]") == [seen, True, "1"]
    bar.until("!debugMoving", "the move is over though the browser never ended it")
    assert time.time() - began < 6, "the change waited on the browser"
    assert bar.ev("[debugBar.style.viewTransitionName, document.documentElement.classList.contains('dbg-swapping')]") == ["", False]
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the branch is dropped")
    bar.ev("delete document.startViewTransition, true")

    # Someone who has asked for less motion gets the change made, and no move.
    bar.dev.call("Emulation.setEmulatedMedia", {"features": [{"name": "prefers-reduced-motion", "value": "reduce"}]})
    bar.until("matchMedia('(prefers-reduced-motion: reduce)').matches", "less motion is asked for")
    bar.ev("(() => { window.moves = 0; const begin = document.startViewTransition.bind(document);"
           " document.startViewTransition = (...given) => { moves++; return begin(...given); }; return true; })()")
    bar.click('[data-dbg="split"]')
    assert bar.ev("[debugSplit.shown, moves]") == [True, 0]
    bar.click('[data-dbg-split="drop-a"]')
    bar.until("!debugSplit", "the branch is dropped")
    assert bar.ev("moves") == 0
    bar.dev.call("Emulation.setEmulatedMedia", {"features": []})
    bar.until("!matchMedia('(prefers-reduced-motion: reduce)').matches", "motion is as it was")
    bar.click('[data-dbg="split"]')
    assert bar.ev("moves") == (1 if seen else 0), "one screen became two without a move"
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the branch is dropped")
    assert bar.ev("moves") == (2 if seen else 0), "two screens became one without a move"
    bar.ev("delete document.startViewTransition, debugSplitReveal = realReveal, true")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_windows_move_by_their_titles(bar: Bar) -> None:
    """A panel of the box, and what the box asks, are moved by pressing their title and dragging.

    Operator, 2026-11-22: "also, important - clicking and dragging on a
    window title allows for repositioning".
    """
    box = ("(id => { const win = document.getElementById(id); if (win.hidden) return null;"
           " const at = win.getBoundingClientRect(); return [Math.round(at.left), Math.round(at.top), Math.round(at.width), Math.round(at.height)]; })")
    # Under its button, or as near under it as the box's right end lets it be.
    under = ("(id => { const win = document.getElementById(id).getBoundingClientRect(), box = debugBar.getBoundingClientRect(),"
             " from = document.querySelector(`[data-dbg-pop=\"${id.slice(8)}\"]`).getBoundingClientRect();"
             " const left = Math.max(box.left + 8, Math.min(from.left, box.right - win.width - 8));"
             " return Math.round(win.top - from.bottom) === 6 && Math.abs(win.left - left) <= 1; })")

    def near(found, wanted) -> bool:
        """The same place, to the pixel a pointer's own rounding allows."""
        return found is not None and all(abs(a - b) <= 1 for a, b in zip(found, wanted))

    def drag(win: str, dx: int, dy: int, let_go: bool = True) -> None:
        """Press the window's title, take the pointer somewhere, and let go."""
        bar.ev(f"(() => {{ const title = document.getElementById('{win}').firstElementChild, at = title.getBoundingClientRect();"
               " const x = at.left + 40, y = at.top + 8, said = (kind, to, px, py) => to.dispatchEvent(new PointerEvent(kind,"
               " {bubbles: true, cancelable: true, pointerId: 5, button: 0, clientX: px, clientY: py}));"
               f" said('pointerdown', title, x, y); said('pointermove', window, x + {dx} / 2, y + {dy} / 2);"
               f" said('pointermove', window, x + {dx}, y + {dy});"
               + (f" said('pointerup', window, x + {dx}, y + {dy});" if let_go else "") + " return true; })()")

    assert bar.ev("debugSplit === null && debugPopOpen === null")
    # Save points open at the right end of the box, and are taken left.
    for name, across in (("layers", 140), ("breaks", 140), ("saves", -140)):
        win = f"dbg-pop-{name}"
        bar.click(f'[data-dbg-pop="{name}"]')
        assert bar.ev(f"{under}('{win}')"), f"{name} does not open under its button"
        # Its title is what it is held by, and says so to the pointer.
        assert bar.ev(f"(() => {{ const title = document.getElementById('{win}').firstElementChild;"
                      " return [title.tagName, getComputedStyle(title).cursor]; })()") == ["H4", "grab"]
        before = bar.ev(f"{box}('{win}')")
        # A press that does not travel is a click: nothing moves.
        drag(win, 1, 1)
        assert bar.ev(f"{box}('{win}')") == before and bar.ev("Object.keys(debugWindowPlace)") == []
        # Dragged, it goes where the pointer went, and shows it is held until it is let go of.
        drag(win, across, 90, let_go=False)
        assert bar.ev(f"document.getElementById('{win}').classList.contains('dbg-moving')")
        bar.ev("window.dispatchEvent(new PointerEvent('pointerup', {pointerId: 5})), true")
        after = bar.ev(f"{box}('{win}')")
        assert near(after, [before[0] + across, before[1] + 90, before[2], before[3]]), (before, after)
        assert bar.ev(f"document.getElementById('{win}').classList.contains('dbg-moving')") is False
        # It stays there when the box is drawn again, and opens there again.
        bar.ev("drawDebugBar(), true")
        assert bar.ev(f"{box}('{win}')") == after
        bar.click(f'[data-dbg-pop="{name}"]')
        assert bar.ev(f"{box}('{win}')") is None
        bar.click(f'[data-dbg-pop="{name}"]')
        assert bar.ev(f"{box}('{win}')") == after
        # It cannot be taken out of the window, nor its title out of reach.
        drag(win, 5000, 5000)
        kept_in = bar.ev(f"{box}('{win}')")
        assert kept_in[0] + kept_in[2] <= bar.ev("innerWidth") and kept_in[1] <= bar.ev("innerHeight") - 44, kept_in
        drag(win, -9000, -9000)
        kept_in = bar.ev(f"{box}('{win}')")
        assert kept_in[0] >= 0 and kept_in[1] >= 0, kept_in
        # A double click on its title puts it back under its button.
        bar.ev(f"document.getElementById('{win}').firstElementChild.dispatchEvent(new MouseEvent('dblclick', {{bubbles: true}})), true")
        assert bar.ev(f"{under}('{win}')") and bar.ev("Object.keys(debugWindowPlace)") == []
        bar.click(f'[data-dbg-pop="{name}"]')
    # Only the title: a press on the rest of a panel moves nothing.
    bar.click('[data-dbg-pop="breaks"]')
    before = bar.ev(f"{box}('dbg-pop-breaks')")
    bar.ev("(() => { const text = document.querySelector('#dbg-pop-breaks p'), at = text.getBoundingClientRect();"
           " for (const [kind, to, x] of [['pointerdown', text, 0], ['pointermove', window, 80], ['pointerup', window, 80]])"
           "   to.dispatchEvent(new PointerEvent(kind, {bubbles: true, pointerId: 6, button: 0, clientX: at.left + 5 + x, clientY: at.top + 5}));"
           " return true; })()")
    assert bar.ev(f"{box}('dbg-pop-breaks')") == before
    bar.click('[data-dbg-pop="breaks"]')

    # What the box asks is a window too. Each of its two questions keeps its
    # own place: the one about another Civ AI on the one timeline, moved, is
    # not where the one about a new branch then opens.
    bar.ev("(() => { const select = document.getElementById('dbg-agent');"
           " select.value = [...select.options].find(option => option.value !== debugStrategy()).value;"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    assert bar.ev("debugAsk.kind") == "switch"
    before = bar.ev(f"{box}('dbg-ask')")
    drag("dbg-ask", 200, 120)
    assert near(bar.ev(f"{box}('dbg-ask')")[:2], [before[0] + 200, before[1] + 120])
    assert bar.ev("debugAsk.kind") == "switch", "moving the question answered it"
    bar.click('#dbg-ask [data-dbg-ask="close"]')
    bar.click('[data-dbg="split"]')
    assert bar.ev("debugAsk") == {"kind": "seat", "letter": "b"}
    assert bar.ev("(() => { const from = document.querySelector('#debugbar [data-dbg-agent=\"b\"]').getBoundingClientRect(),"
                  " at = document.getElementById('dbg-ask').getBoundingClientRect(); return Math.round(at.top - from.bottom); })()") == 6
    # Moved over a half, it is still asked, and is answered where it stands.
    before = bar.ev(f"{box}('dbg-ask')")
    drag("dbg-ask", 300, 200)
    assert near(bar.ev(f"{box}('dbg-ask')")[:2], [before[0] + 300, before[1] + 200])
    assert bar.ev("debugAsk") == {"kind": "seat", "letter": "b"}
    bar.click('#dbg-ask [data-dbg-ask="seat"][value="stock"]')
    bar.until("debugSplit.panes.b.frame.contentWindow.eval('engine.seat') === 'stock'", "B is seated from the moved question")
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the branch is dropped")
    bar.ev("for (const key of Object.keys(debugWindowPlace)) delete debugWindowPlace[key]; true")
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
         "--disable-gpu", "--window-size=1400,900", f"http://127.0.0.1:{port}/{ONE_TIMELINE}"],
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
                dev.call("Page.navigate", {"url": f"http://127.0.0.1:{port}/{ONE_TIMELINE}"})
        print(f"the stand-in page was ready {time.time() - opened:.1f}s after Chrome answered.")
        check_the_bar(bar)
        print("the header box stands beside the left panel with the map area beneath it, the timeline "
              "on top and the open turn under its middle, and sends what each control is for: the "
              "turn box, the timeline back and ahead, the wheel, the turn buttons, the eras, Era Back, "
              "Era Next and Play to End, save points and their flags, a breakpoint picked from the "
              "timeline, and "
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
        check_a_run_takes_the_turns_already_played(bar)
        print("a run takes the turns its Civ AI already played from where they are kept, stops on a "
              "breakpoint among them, and plays only the turns past the last of them.")
        check_the_split(bar)
        print("the split opens at once on one timeline, seats a Civ AI in each half, plays both to a "
              "turn locked and unlocked, branches the timeline on the last turn the two hold as one "
              "world, sets their numbers side by side, starts both again from the split, moves "
              "each half or both by its slider in the host's bar, copies, drops a branch by its name, and "
              "hears a half whose script came late.")
        check_the_lock(bar)
        print("the lock joins the two halves and closes only on one turn, the host's deck is the one "
              "panel of a locked split and each half's own comes back when it opens, two "
              "identical halves are played once and stay one timeline, a half that plays ahead "
              "alone has a branch of its own until the other plays the same turn, and the timeline shows each half's breakpoints and save "
              "points, once while they are locked, and goes to the one that is clicked.")
        check_a_branch_forks_the_timeline(bar)
        print("Branch under the marker forks the timeline on the turn it is pressed on, the branch "
              "holding nothing past it; the fork is drawn as a tree, the trunk in the middle and a branch "
              "up to A's row and down to B's, numbered once and each named on itself by its letter and its "
              "Civ AI; each branch has an ✕ after its right end, and closing the timeline as it was leaves the "
              "branch as the timeline, named by the Civ AI that played each stretch.")
        check_the_box_asks(bar)
        print("another Civ AI picked on the one timeline is asked about first, Switch seats it and Branch "
              "makes a branch for it; a branch made by Branch asks which Civ AI plays it, under its picker, "
              "and is answered from the list or from the picker.")
        doc = " ".join((here.parent / "docs" / "SIM_DEBUGGER.md").read_text(encoding="utf-8").split())
        check_one_screen_and_two_change_places_in_one_move(bar, doc)
        print("Branch holds the window as it stands while the halves load unseen and shows the two screens once, "
              "with everything they are drawn from there and each looking where the one screen looked; closing a "
              "branch holds the two and greys nothing, and the one screen looks where the kept half did; a half "
              "that never arrives is not waited on for ever, a browser that never makes the move does not keep the "
              "change, and less motion asked for is no move at all.")
        check_windows_move_by_their_titles(bar)
        print("each panel of the box, and each thing the box asks, is moved by its title, stays where it was put, "
              "cannot leave the window, and goes back under its button on a double click.")
        check_the_game_opens_on_two_tracks(bar, f"http://127.0.0.1:{port}/")
        print("a game opens on its two tracks at once with head to head open, asks for B's Civ AI, and does "
              "so once a game; a narrow window and the Single player view open on the one timeline.")
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
