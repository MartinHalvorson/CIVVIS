#!/usr/bin/env python3
"""Drive the Sim debugger's bar and split in a real browser against a toy engine.

`web/assets/app_debugger.js` is loaded as it ships, on a page that stands in
for the viewer (the handful of globals the bar reads) and an engine that
stands in for `POST /debug` (a turn counter with marks, save points and turn
breakpoints). Nothing here plays a game: what is checked is the wiring, that
each control sends the operation it is for and that the bar shows the answer.

A split is the same page twice more, in frames, so each half gets a toy engine
of its own, started from the host's. The toy's score rises faster under one
Civ AI than the other, which is all it takes to see that each half's steps
carry its own.

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
</style></head>
<body>
<div id="maparea" style="height:100%"></div>
<select id="autoplaystrategy"><option value="jack">Jack</option><option value="stock">Stock</option></select>
<script>
"use strict";
// ---- what the viewer gives the bar
let state = null, sel = null, selCity = null;
let SPEC = false, autoplaying = false, turnAdvancePending = false;
const AUTOPLAY_BATCH_TIMEOUT_MS = 60000;
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
function render(next) { state = next; }
function escapeAttr(text) {
  return String(text).replace(/[&<>"']/g, c => ({"&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;"}[c]));
}
function titleCase(text) { return String(text).replace(/\b\w/g, c => c.toUpperCase()); }
async function send() { return state; }
// A run waits a frame between turns, and a page nobody is looking at has none.
window.requestAnimationFrame = drawn => setTimeout(drawn, 0);

// ---- a toy engine: one line of turns, each kept
const engine = {turn: 1, marks: [1], saves: [], ids: 0, breaks: [], line: null, stopped: null, score: 0,
  current: 0, stepped: null, acted: 0, reveal: false, layers: [], wait: 0,
  watch: {categories: [], phrases: [], important: false, every_seat: false, conditions: []}, sent: []};
// The round: the played seat, one rival, and the Barbarians.
const SEATS = 3;
// Another frame's engine as it stands, for a half that is copied or kept:
// everything but the record of what this frame's own page sent.
function engineOf(frames, pane) {
  const frame = [...frames].find(held => held.src.includes(`pane=${pane}`));
  const {sent, ...world} = JSON.parse(JSON.stringify(frame.contentWindow.eval("engine")));
  return world;
}
function engineState(error = null) {
  const marks = engine.marks;
  return {error, turn: engine.turn, current: engine.current, units: [], cities: [],
    players: [{id: 0, civ: "rome", score: engine.score, cities: 1, population: 1, military: 1,
      yields: {science: 1, culture: 1, gold: 1, faith: 0}},
      {id: 1, civ: "greece"}, {id: 2, civ: "barbarians", is_barbarian: true}],
    debug: {turn: engine.turn, turns: [...marks], first: marks[0], last: marks[marks.length - 1],
      acted: engine.acted, can_undo: engine.acted > 0 && engine.current === 0,
      can_back: engine.acted > 0 || engine.turn > marks[0], can_forward: engine.turn < marks[marks.length - 1],
      breakpoints: {turns: [...engine.breaks], ...JSON.parse(JSON.stringify(engine.watch)),
        known_categories: ["War", "Wonder"], known_tests: [">", "changes"]},
      layers: [["research", "Research"], ["cities", "Cities"]].map(([id, label]) =>
        ({id, label, on: engine.layers.includes(id)})), assisted: 0, stopped: engine.stopped,
      stepped: engine.stepped, reveal: engine.reveal, playing: engine.current, memory_line: engine.line,
      saves: engine.saves.map(save => ({...save}))}};
}
async function fetchJSON(path, init = {}) {
  const body = init.body ? JSON.parse(init.body) : null;
  engine.sent.push({path, body});
  if (path === "/saves") return {saves: []};
  if (path !== "/debug") return {error: null};
  // A slowed engine, so that a run can be seen on its way.
  if (engine.wait) await new Promise(done => setTimeout(done, engine.wait));
  engine.stopped = null;
  engine.stepped = null;
  // The round comes back to the played seat, and that is the next turn.
  const opens = () => {
    engine.current = 0;
    engine.turn += 1;
    engine.marks = engine.marks.filter(turn => turn < engine.turn).concat(engine.turn);
  };
  // Anything that plays, in the middle of a round, lets the round finish first.
  if (engine.current !== 0 && (body.op === "step" || body.op === "finish_round")) opens();
  const marks = engine.marks, at = marks.indexOf(engine.turn);
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
    engine.marks = marks.filter(turn => turn < engine.turn).concat(engine.turn);
    if (engine.breaks.includes(engine.turn)) engine.stopped = {kind: "turn", turn: engine.turn};
  } else if (body.op === "jump") {
    const reached = marks.filter(turn => turn <= body.turn).pop();
    if (reached === undefined) error = "before this timeline begins"; else engine.turn = reached;
  } else if (body.op === "back") {
    if (at > 0) engine.turn = marks[at - 1]; else error = "this is where the timeline begins";
  } else if (body.op === "forward") {
    if (at < marks.length - 1) engine.turn = marks[at + 1]; else error = "nothing has been played past this turn";
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
      every_seat: !!body.every_seat, conditions: body.conditions || []};
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
  return engineState(error);
}

// ---- the bar's script, and a half of a split
// A half starts on its host's world. The real page fetches that through the
// host's engine; here the half takes the toy engine as the host's stands.
const pane = window.parent !== window && new URLSearchParams(location.search).get("pane");
const barScript = document.createElement("script");
barScript.src = "app_debugger.js";
if (!pane) document.body.appendChild(barScript);
else {
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


def check_the_bar(bar: Bar) -> None:
    bar.ev("render(engineState()), true")
    assert bar.ev("!document.getElementById('debugbar').hidden"), "the bar is hidden on a played game"
    assert bar.ev("document.getElementById('dbg-turn').value") == "1"

    # The bar is fixed across the very top of the window, the whole width of
    # it, and the page stands beneath: the body is padded by the bar's height.
    left, top, width, height, window = bar.ev(
        "(() => { const box = document.getElementById('debugbar').getBoundingClientRect();"
        " return [box.left, box.top, box.width, Math.ceil(box.height), document.documentElement.clientWidth]; })()")
    assert window == bar.ev("innerWidth"), "the stand-in page scrolls, and the viewer never does"
    assert (left, top, width) == (0, 0, window), (left, top, width, window)
    assert bar.ev("getComputedStyle(document.getElementById('debugbar')).position") == "fixed"
    assert bar.ev("getComputedStyle(document.body).paddingTop") == f"{height}px"
    assert bar.ev("Math.round(document.getElementById('maparea').getBoundingClientRect().top)") == height
    # The timeline is the whole game, 250 turns when the game does not say,
    # and it is nearly as wide as the window.
    assert bar.ev("(() => { const s = document.getElementById('dbg-slider'); return [s.min, s.max, s.disabled]; })()") \
        == ["1", "250", False]
    assert bar.ev("document.getElementById('dbg-slider').getBoundingClientRect().width") > window - 40
    assert bar.ev(f"document.querySelector('{LANE} .dbg-ruler').textContent") == "150100150200250"

    # The turn box, a turn still ahead: played to, one step a request.
    bar.type_turn(6)
    bar.idle("state.turn === 6", "the turn box plays to a turn ahead")
    assert [call["op"] for call in bar.sent()] == ["step"] * 5, bar.sent()
    assert bar.note() == "Played to turn 6", bar.note()
    assert bar.ev("state.debug.turns.length") == 6
    assert bar.ev("document.getElementById('dbg-played').textContent") == "Turns 1 to 6 played, every one kept"

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

    # The two ends of what was played, a button each.
    bar.forget()
    bar.click('[data-dbg="last"]')
    bar.idle("state.turn === 9", "the last turn played is gone to")
    assert bar.ev("document.querySelector('[data-dbg=\"last\"]').disabled")
    bar.click('[data-dbg="first"]')
    bar.idle("state.turn === 1", "the first turn kept is gone to")
    assert bar.ev("document.querySelector('[data-dbg=\"first\"]').disabled")
    assert bar.sent() == [{"op": "jump", "turn": 9}, {"op": "jump", "turn": 1}], bar.sent()
    bar.drag("dbg-slider", 2)
    bar.idle("state.turn === 2", "the slider goes to turn 2")

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
    assert bar.ev("document.getElementById('dbg-played').textContent") == "Turns 1 to 12 played, 4 kept"
    assert hover(6) == "Turn 6 · no longer kept, goes to turn 4", hover(6)
    bar.forget()
    bar.drag("dbg-slider", 6)
    bar.idle("state.turn === 4", "a turn no longer kept answers with the nearest before it")
    assert bar.note() == "Turn 6 is no longer kept. This is turn 4, the nearest before it", bar.note()
    bar.ev(f"engine.marks = {json.dumps(kept)}, engine.turn = 9, render(engineState()), true")
    assert not bar.ev(f"document.querySelector('{LANE}').classList.contains('dbg-thinned')")

    # The step and back buttons, for completeness: one operation each.
    bar.forget()
    bar.click('[data-dbg="back"]')
    bar.idle("state.turn === 8", "back goes one turn back")
    bar.click('[data-dbg="step"]')
    bar.idle("state.turn === 9", "step plays one turn")
    assert [call["op"] for call in bar.sent()] == ["back", "step"], bar.sent()
    assert bar.sent()[-1].get("strategy") == "jack", bar.sent()

    # The Civ AI that plays the seat is picked beside Run, from what the page's
    # own picker lists, and a step carries it.
    agent = "document.getElementById('dbg-agent')"
    assert bar.ev(f"[...{agent}.options].map(option => option.value)") == ["jack", "stock"]
    assert bar.ev(f"{agent}.value") == "jack"

    def seat(name: str) -> None:
        bar.ev(f"(() => {{ const select = {agent}; select.value = '{name}';"
               " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
        assert bar.ev("document.getElementById('autoplaystrategy').value") == name

    seat("stock")
    bar.click('[data-dbg="step"]')
    bar.idle("state.turn === 10", "a step with the Civ AI picked in the bar")
    assert bar.sent()[-1] == {"op": "step", "strategy": "stock"}, bar.sent()
    seat("jack")
    bar.click('[data-dbg="back"]')
    bar.idle("state.turn === 9", "back to turn 9")

    # A bar with room has words on its tools; a narrow one keeps the signs,
    # and the page still stands under whatever height the bar comes to. How
    # wide "room" is depends on the type the browser has, so the two windows
    # here are far to either side of it.
    word = "getComputedStyle(document.querySelector('#debugbar .dbg-word')).display"
    tight = "document.getElementById('debugbar').classList.contains('dbg-tight')"
    under = ("getComputedStyle(document.body).paddingTop ==="
             " Math.ceil(document.getElementById('debugbar').getBoundingClientRect().height) + 'px'")
    tools = "document.querySelector('#debugbar .dbg-tools').offsetHeight"

    def window_of(width: int) -> None:
        bar.dev.call("Emulation.setDeviceMetricsOverride", {"width": width, "height": 900, "deviceScaleFactor": 1, "mobile": False})
        bar.until(f"document.documentElement.clientWidth === {width} && {under}", f"the bar in a window {width} wide")

    window_of(3000)
    bar.until(f"!{tight}", "a bar with room for the words went without them")
    assert bar.ev(word) != "none" and bar.ev(tools) < 40, (bar.ev(word), bar.ev(tools))
    window_of(500)
    bar.until(tight, "a narrow bar kept the words on its tools")
    assert bar.ev(word) == "none"
    assert bar.ev(tools) > 40 and bar.ev(under), "the tools wrap in a narrow window, and the page stays under them"
    bar.dev.call("Emulation.clearDeviceMetricsOverride")
    bar.until(f"document.documentElement.clientWidth === {window} && {under}", "the window is its own size again")

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
                       "important": False, "every_seat": False, "conditions": []}, request
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

    # Run plays turns until the engine says a breakpoint stopped it.
    request = typed("turns", str(turn + 3))
    assert request["turns"] == [turn + 3], request
    bar.forget()
    bar.click('[data-dbg="run"]')
    bar.idle(f"state.turn === {turn + 3}", "a run stops on its breakpoint")
    assert [call["op"] for call in bar.sent()] == ["step"] * 3, bar.sent()
    assert bar.note() == f"Stopped at turn {turn + 3}", bar.note()
    assert bar.ev("document.getElementById('dbg-run-sign').textContent") == "▶▶"

    # Clear all sends a request with nothing in it, and the count goes.
    bar.forget()
    bar.click('[data-dbg="break-clear"]')
    bar.idle("state.debug.breakpoints.turns.length === 0 && !state.debug.breakpoints.important", "the breakpoints are cleared")
    assert bar.sent() == [{"op": "breakpoints"}], bar.sent()
    assert count() == "", count()
    bar.click('[data-dbg-pop="breaks"]')

    # With nothing to stop it a run goes on, the same button is its Stop, and
    # the bar says how far it got.
    bar.click('[data-dbg="run"]')
    bar.until(f"debugRunning && state.turn > {turn + 5}", "a run with no breakpoint keeps playing")
    assert bar.ev("document.getElementById('dbg-run-sign').textContent") == "⏹"
    assert bar.ev("document.getElementById('dbg-run-word').textContent") == "Stop"
    bar.click('[data-dbg="run"]')
    bar.idle("true", "the run stops when told to")
    assert bar.note().startswith("Stopped by hand after "), bar.note()
    assert bar.ev("state.turn") < 400, "the run was stopped, not exhausted"


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

    # Undo, by its button and by the key the hand expects, and not when
    # there is nothing to take back.
    assert bar.ev("document.querySelector('[data-dbg=\"undo\"]').disabled")
    bar.ev("engine.acted = 2, render(engineState()), true")
    bar.forget()
    bar.click('[data-dbg="undo"]')
    bar.idle("state.debug.acted === 1", "the button takes back an action")
    bar.ev("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true, bubbles: true})), true")
    bar.idle("state.debug.acted === 0", "Ctrl+Z takes back an action")
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
    assert bar.note() == "Greece plays next · ▸| steps it, ▶| lets the round finish", bar.note()
    bar.forget()
    bar.ev("send({type: 'move'}), true")
    assert bar.note() == "Greece plays next; your turn comes back when the round finishes", bar.note()
    assert bar.sent() == [], bar.sent()
    bar.ev("send({type: 'end_turn'}), true")
    bar.idle(f"state.current === 0 && state.turn === {turn + 2}", "End Turn lets the round finish")
    assert bar.sent() == [{"op": "finish_round"}], bar.sent()
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_the_split(bar: Bar) -> None:
    """Head to head: two halves, a Civ AI each, played to a turn and compared."""
    half = "debugSplit.panes.{}.frame.contentWindow.eval({})".format
    ops = lambda letter: bar.ev(half(letter, json.dumps(
        "engine.sent.filter(call => call.path === '/debug').map(call => [call.body.op, call.body.strategy])")))
    reported = ("debugSplit && ['a', 'b'].every(letter => debugSplit.panes[letter].turn === {} "
                "&& debugSplit.panes[letter].numbers)").format

    start = bar.ev("state.turn")
    bar.click('[data-dbg-pop="split"]')
    bar.click('[data-dbg="split-versus"]')
    bar.until(reported(start), "both halves say where they stand")
    assert bar.ev("debugSplit.versus && !debugSplit.host.querySelector('[data-dbg-versus]').hidden")

    # The host's bar stays up across the top, over the two halves, and its
    # timeline is two lanes, one for each half. A half carries the tools for
    # its own world and no timeline.
    assert bar.ev("(() => { const top = document.getElementById('debugbar'), box = top.getBoundingClientRect();"
                  " return !top.hidden && box.top === 0 && box.width === document.documentElement.clientWidth"
                  " && Math.round(debugSplit.host.getBoundingClientRect().top) === Math.ceil(box.height); })()"), \
        "the split does not stand under the host's bar"
    shown = lambda lane: bar.ev(f"getComputedStyle(document.querySelector('[data-dbg-lane=\"{lane}\"]')).display") != "none"
    assert (shown(""), shown("a"), shown("b")) == (False, True, True)
    assert bar.ev("getComputedStyle(document.querySelector('#debugbar .dbg-tools')).display") == "none"
    assert bar.ev(half("a", "'getComputedStyle(document.querySelector(\"#debugbar .dbg-time\")).display'")) == "none"
    assert bar.ev(half("a", "'getComputedStyle(document.querySelector(\"#debugbar .dbg-tools\")).display'")) != "none"
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
    assert bar.ev(half("a", "'getComputedStyle(document.getElementById(\"dbg-agent\")).display'")) == "none"
    for name in ("stock", "jack"):
        bar.ev(f"(() => {{ const select = {head('a')}; select.value = '{name}';"
               " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
        bar.until(half("a", "'debugStrategy()'") + f" === '{name}'", f"A is seated with {name} from its lane")
        assert bar.ev(f"debugSplit.host.querySelector('[data-dbg-strategy=\"a\"]').value") == name

    # Linked: A is run to the turn and B follows it turn by turn, each with
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

    # Separate: each half is run by itself.
    bar.click('[data-dbg-split="link"]')
    assert bar.ev("debugSplit.linked") is False
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
    assert bar.ev("debugSplit.linked") is False
    # They are played again from there, still each by itself.
    play_both_to(start + 6)
    assert ops("a")[7:] == [["step", "jack"]] * 6 and ops("b")[7:] == [["step", "stock"]] * 6, (ops("a"), ops("b"))
    assert bar.ev("[debugSplit.history.a.size, debugSplit.history.b.size]") == [7, 7]

    # Separate, a lane is the hand on its own half. B walks back two turns
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
    # Linked, either lane moves both halves: back along what both have played,
    # and on past it with A run and B following turn by turn.
    bar.click('[data-dbg-split="link"]')
    bar.until(both(start + 6, start + 6), "linking brings B to A's turn")
    bar.drag("dbg-slider-b", start + 2)
    bar.until(both(start + 2, start + 2), "a lane walks both halves back when they are linked")
    assert ops("a")[13:] == [["jump", None]] and ops("b")[-1] == ["jump", None], (ops("a"), ops("b"))
    bar.drag("dbg-slider-a", start + 10)
    bar.until(both(start + 10, start + 10), "a lane plays both halves on when they are linked")
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
    # The screen is whole again and the bar is the world's own.
    assert (shown(""), shown("a"), shown("b")) == (True, False, False)
    assert bar.ev("getComputedStyle(document.querySelector('#debugbar .dbg-tools')).display") != "none"
    assert bar.ev("document.getElementById('dbg-slider').value") == str(start + 10)

    # A half that drew its first state before the bar's script arrived still
    # says where it stands. (It used to say nothing until something moved.)
    bar.ev("window.lateScript = true")
    bar.click('[data-dbg-pop="split"]')
    bar.click('[data-dbg="split-row"]')
    bar.until(reported(start + 10), "a half whose script came late still reports")
    bar.click('[data-dbg-split="keep-a"]')
    bar.until("!debugSplit", "the second split closes")
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
        print("the debugger bar stands across the top with the page beneath it and sends what each "
              "control is for: the turn box, the timeline back and ahead, the wheel, both ends, "
              "save points and their flags, a breakpoint picked from the timeline.")
        check_breakpoints_and_a_run(bar)
        check_layers_reveal_and_undo(bar)
        print("breakpoints are set from the form, a run stops on one or by hand, and layers, "
              "reveal and undo send what they are for.")
        check_stepping_a_seat(bar)
        print("a round is stepped a civilization at a time, says who is next, and End Turn finishes it.")
        check_the_split(bar)
        print("the split seats a Civ AI in each half, plays both to a turn linked and separate, "
              "sets their numbers side by side, starts both again from the split, moves each half "
              "or both by the lanes in the host's bar, copies, keeps, and hears a half whose "
              "script came late.")
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
