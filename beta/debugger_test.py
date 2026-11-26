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
/* The viewer's result card is there only while a game is over. */
#winner { display: none; } #winner.visible { display: block; }
</style></head>
<body>
<div id="maparea"><div id="winner"></div><div id="moments"></div><div class="minimap-frame"></div><div id="map-controls-dock"></div></div>
<div id="side">
  <div class="spec-controls"><button id="restart-sim" class="primary human-start">Start new game</button></div>
  <div id="humanfooter" style="display:grid">
    <div class="autoplay-bar" id="autoplaybar">
      <div class="autoplay-fields">
        <label class="autoplay-field"><span>Civ AI</span><select id="autoplaystrategy"><option value="jack">1211 - JackOfAllTrades</option><option value="stock">1000 - ByTheBook</option></select></label>
        <label class="autoplay-field"><span>Turns</span><select id="autoplayturns"><option value="10" selected>10</option><option value="all">All</option></select></label>
      </div>
      <button id="autoplaybtn">Auto-play 10 turns</button>
      <div class="autoplay-note" id="autoplaynote"></div>
    </div>
  </div>
</div>
<script>
"use strict";
// ---- what the viewer gives the bar
// Which of the simulator's pages this is, as the viewer's head script says
// it: the address names it (/debugger, /doctor-strange, /edge-of-tomorrow),
// or an old link's `?mode=` does, which is how this page is asked.
window.CIVVIS_PAGE_MODE = ({debugger: "play", "doctor-strange": "strange", "edge-of-tomorrow": "tomorrow"})[
  location.pathname.replace(/\/(?:index\.html)?$/, "").split("/").pop()] || new URLSearchParams(location.search).get("mode");
let state = null, sel = null, selCity = null;
let SPEC = false, autoplaying = false, turnAdvancePending = false;
const AUTOPLAY_BATCH_TIMEOUT_MS = 60000;
// The roster the viewer reads the Civ AIs' names from, and where it keeps its choice.
const RULES = {strategies: [{name: "jack", username: "Jack", elo: 1211}, {name: "stock", username: "Stock", elo: 1000}]};
// More rated strategies, below these two, for a check that wants a roster as
// long as the real one: [[name, elo], ...] kept where every page of the
// origin reads it, the worlds' pages too.
for (const [name, elo] of JSON.parse(localStorage.getItem("civvis-toy-strategies") || "[]")) {
  RULES.strategies.push({name, username: name, elo});
  document.getElementById("autoplaystrategy").insertAdjacentHTML("beforeend", `<option value="${name}">${elo} - ${name}</option>`);
}
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
function gameFinished(st = state) { return !!(st && st.finished); }
function reportedTurn(st) { return (st || state).turn; }
function victoryVerdict(type) { return titleCase(type || "score") + " victory"; }
function reignSummary() { return ""; }
// The Civilization VI screen's Rankings: whether the standings and the
// victory tracker are open, and whether a played game's kept choice of it
// has been read.
let rankingsReportOpen = false, soloRankingsSettled = false;
function toggleRankingsReport(open) { rankingsReportOpen = open === undefined ? !rankingsReportOpen : !!open; }
function drawTurnLoop() {}
// What the viewer's banners compare each turn with (`app_moments.js`), the
// banners waiting to be shown, and how many are up (in `#moments`).
let rivalsWorld = null, boostsWorld = null, momentsShowing = 0;
const momentQueue = [];
// The viewer's own result card, as far as Edge of Tomorrow reads it: a card
// in `#winner` while the seat has fallen or the game is over, drawn again
// only when the result's signature changes, offering another game and
// counting down to it. `finale` is how often that count was started and
// how often it was called off (`clearFinaleCountdown`).
const finale = {armed: 0, cleared: 0};
function clearFinaleCountdown() { finale.cleared += 1; }
function drawResult() {
  const card = document.getElementById("winner"), me = state.players[0];
  const fallen = !state.finished && me.alive === false;
  if (!fallen && !state.finished) {
    card.classList.remove("visible");
    if (card.dataset.signature) { delete card.dataset.signature; card.replaceChildren(); }
    return;
  }
  const signature = fallen ? `fallen|${state.seed}|${state.turn}` : `${state.seed}|${state.winner}|${state.turn}`;
  card.classList.add("visible");
  if (card.dataset.signature === signature) return;
  card.dataset.signature = signature;
  card.innerHTML = `<div class="winner-content"><span class="winner-kicker">${fallen || state.winner !== 0 ? "Defeat" : "Victory"}</span>` +
    `<span class="winner-meta">Turn ${state.turn}</span><button id="finale-restart">Start another game</button></div>`;
  finale.armed += 1;
}
// The page's two views of a held seat, and what the Single player switch asks
// the page to paint again or put away. `painted` is the record of it.
let SOLO_UI = false;
const painted = [];
for (const name of ["drawSoloHud", "drawLaunchBar", "drawPlayerHud", "drawSide", "drawUbar",
  "advanceToNextActionUnit", "clearUnitIntel", "closeCityScreen", "closeEmpire", "closeDiplomacy", "closeTrade"])
  window[name] = () => { painted.push(name); };
// How many debugger requests had been sent each time a world was drawn.
const drawn = [];
function render(next) { state = next; drawn.push(engine.sent.filter(call => call.path === "/debug").length); drawResult(); }
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
// The black swans the toy can strike (`BlackSwanKind`): the name of each and
// what it does in a line. Which one a seed draws is the seed's parity.
const SWANS = {pandemic: ["Pandemic", "Every city in the world loses half its people"],
  nuclear_war: ["Instant nuclear war", "Every civilization is at war with every other"]};
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
// How the seat stands, as the engine reads it for a search (`obs::standing`:
// a lean answer's `seat`, and one for each turn of a `play`). The state's own
// entry for the seat carries the same numbers.
function engineSeat() {
  return {turn: engine.turn, alive: true, score: engine.score, cities: 1, population: 1, gold_per_turn: 1,
    yields: {food: 1, production: 1, science: 1, culture: 1, gold: 1, faith: 0}};
}
function engineState(error = null) {
  const marks = engineLine(), kept = engine.marks, seat = engineSeat();
  // How a game goes against the seat is the test's to say: `engine.raids`,
  // the turns Greece declares war on it; `engine.falls`, the turn it has
  // fallen by; and `engine.over`, the turn the game is over by and who won.
  const fell = typeof engine.falls === "number" && engine.turn >= engine.falls;
  const over = engine.over && engine.turn >= engine.over.turn ? engine.over : null;
  return {error, turn: engine.turn, seed: engine.seed, current: engine.current, units: [], cities: [],
    world_era: engine.eras.filter(era => era.turn <= engine.turn).pop().era,
    ...(engine.limit ? {turn_limit: engine.limit} : {}), seat,
    wars: (engine.raids || []).filter(turn => turn <= engine.turn)
      .map(turn => ({highlights: [{turn, kind: "declared", actor: 1, subject: 0, city: null}]})),
    ...(over ? {finished: true, winner: over.winner, winners: [over.winner], victory_type: "science", victory_turn: over.turn} : {}),
    players: [{id: 0, civ: "rome", ...(fell ? {alive: false} : {}),
      score: seat.score, cities: seat.cities, population: seat.population, military: 1,
      gold_per_turn: seat.gold_per_turn, yields: seat.yields,
      // What the seat has made so far grows with the score, so two worlds
      // played by different Civ AIs have different totals.
      totals: {production: engine.score * 1250, science: engine.score * 3, culture: engine.score * 2,
        gold: engine.score * 4 - 0.4, faith: 0}},
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
      // The black swans that struck the line as far as it is kept, and the
      // ones there are to strike (`Timeline::json`).
      swans: (engine.swans || []).filter(swan => swan.turn <= marks[marks.length - 1]),
      swan_kinds: Object.entries(SWANS).map(([id, [name, about]]) => ({id, name, about})),
      saves: engine.saves.map(save => ({...save}))}};
}
// ---- a search's lists, as the engine reads them (`src/server/autosearch.rs`)
function engineNumber(key) {
  if (key === "won" || key === "lost") return 0;
  const value = String(key).split(".").reduce((held, part) => held && typeof held === "object" ? held[part] : undefined, engineSeat());
  return typeof value === "number" ? value : null;
}
function engineTerms(lists) {
  const holds = term => {
    const now = engineNumber(term.key), line = term.value;
    return now !== null && ({"<": now < line, "<=": now <= line, ">": now > line, ">=": now >= line, "==": now === line, "!=": now !== line})[term.test];
  };
  return {turn: engine.turn, need: (lists.need || []).map(holds), avoid: (lists.avoid || []).map(holds),
    want: (lists.want || []).map(term => engineNumber(term.key)), dont_want: (lists.dont_want || []).map(term => engineNumber(term.key))};
}
// ---- the climb, as `POST /autosearch` runs it in the engine. The toy's is
// the engine's ranking (`Search::better`) with a climb cut short: every Civ
// AI alone, then every other Civ AI handed the rest of the best line from
// halfway, and done. The engine's own climb is held by its tests
// (`server/autosearch_tests.rs`); this one is what the page's pump and panel
// are held against.
const climb = {search: null};
function toyClimb(body) {
  const keyOf = plan => plan.map(([turn, name]) => `${turn}:${name}`).join(" ");
  if (body.op === "begin") {
    const letters = [...new Set([body.seated, ...(body.letters || [])])];
    climb.search = {from: body.from, end: body.end, seated: body.seated, letters, objective: body.objective,
      queue: letters.map(name => [[body.from, name]]), out: 0, lines: [], grown: false};
    return toyReport();
  }
  if (body.op === "end") { climb.search = null; return {ended: true}; }
  const search = climb.search;
  if (!search) return {error: "no search has begun"};
  if (body.op === "next") {
    if (!search.queue.length && !search.out && !search.grown) {
      search.grown = true;
      const best = toyReport().best, turn = search.from + Math.floor((search.end - search.from) / 2);
      if (best) for (const name of search.letters) {
        const held = best.plan.filter(([from]) => from <= turn).pop()[1];
        if (name !== held) search.queue.push([...best.plan.filter(([from]) => from < turn), [turn, name]]);
      }
    }
    const plan = search.queue.shift() || null;
    if (plan) search.out += 1;
    return {plan, from: search.from, done: !plan && !search.out && search.grown};
  }
  if (body.op === "took") {
    search.out -= 1;
    const series = body.series || [], last = series[series.length - 1] || {need: [], avoid: [], want: [], dont_want: []};
    const need = (search.objective.need || []).map((term, at) => term.when === "end" ? !!last.need[at] : series.some(held => held.need[at]));
    const avoid = (search.objective.avoid || []).map((term, at) => term.when === "end" ? !last.avoid[at] : !series.some(held => held.avoid[at]));
    search.lines.push({plan: body.plan, key: keyOf(body.plan), last: body.last, series,
      verdict: {need, avoid, broken: [...need, ...avoid].filter(kept => !kept).length, killed: null, want: last.want, dont_want: last.dont_want}});
  } else if (body.op === "failed") search.out -= 1;
  return toyReport();
}
function toyReport() {
  const search = climb.search, base = search.lines.find(line => line.plan.length === 1 && line.plan[0][1] === search.seated);
  const over = (now, then) => typeof now === "number" && typeof then === "number" ? 100 * (now - then) / Math.max(1, Math.abs(then)) : 0;
  const weights = [...(search.objective.want || []), ...(search.objective.dont_want || [])].map(term => term.weight);
  const shown = line => {
    const overs = base ? [...line.verdict.want.map((now, at) => over(now, base.verdict.want[at])),
      ...line.verdict.dont_want.map((now, at) => -over(now, base.verdict.dont_want[at]))] : [];
    return {plan: line.plan, key: line.key, last: line.last, verdict: line.verdict, feasible: line.verdict.broken === 0,
      gain: overs.reduce((sum, value, at) => sum + value * weights[at], 0), over: overs,
      chart: line.series.map(held => [held.turn, held.want[0] ?? null])};
  };
  const lines = search.lines.map(shown);
  const better = (one, other) => one.feasible !== other.feasible ? one.feasible
    : one.verdict.broken !== other.verdict.broken ? one.verdict.broken < other.verdict.broken
    : one.gain !== other.gain ? one.gain > other.gain : one.plan.length < other.plan.length;
  const best = base ? lines.reduce((held, line) => better(line, held) ? line : held, lines[0]) : null;
  return {from: search.from, end: search.end, lines: lines.length, played: 0, queued: search.queue.length,
    feasible: lines.filter(line => line.feasible).length, best, baseline: base ? shown(base) : null,
    front: best ? [best] : [], front_size: best ? 1 : 0, alone: lines.filter(line => line.plan.length === 1)};
}
// ---- copies of the toy engine, as `POST /forks` makes them of the real one
// (`beta/shim.js`): each holds this page's world and each half's as they
// stood, and from there plays its own history. `refuse` makes this an engine
// that cannot be copied, as the desktop program's is, and `wait` slows every
// copy so that a search can be seen on its way.
const forks = {held: [], refuse: null, wait: 0};
function forkEngines(count) {
  if (forks.refuse) return {error: forks.refuse};
  const frames = [...document.querySelectorAll("iframe")];
  forks.held = Array.from({length: count}, () => {
    const worlds = {"": worldOf(engine)};
    for (const frame of frames) {
      const pane = new URL(frame.src, location.href).searchParams.get("pane");
      // A page still loading (Doctor Strange's next branch, made ahead of
      // time) holds no world yet.
      let held = null;
      try { held = frame.contentWindow.eval("typeof engine === 'object' ? engine : null"); } catch (_) {}
      if (pane && held) worlds[pane] = worldOf(held);
    }
    return worlds;
  });
  return count ? {forks: count, memory_bytes: 1 << 20, ms: 1} : {forks: 0};
}
// One request to one copy. The toy engine answers from whatever `engine`
// holds, so the copy's world is put there for as long as one answer takes,
// which awaits nothing, and what it became is kept as the copy's.
async function forkAnswer(number, path, body) {
  if (forks.wait) await new Promise(done => setTimeout(done, forks.wait));
  const pane = new URLSearchParams(path.split("?")[1] || "").get("pane") || "";
  const worlds = forks.held[number - 1], world = worlds && worlds[pane];
  if (!world) return {error: `there is no fork ${number}`};
  const mine = {...engine};
  const hold = held => {
    for (const key of Object.keys(engine)) delete engine[key];
    Object.assign(engine, held);
  };
  hold({...JSON.parse(JSON.stringify(world)), sent: [], wait: 0});
  try {
    const answer = engineDebug(body);
    worlds[pane] = worldOf(engine);
    return answer;
  } finally {
    hold(mine);
  }
}
// ---- a world's scout, as the published build keeps one for each world of
// Doctor Strange's Civ 6 (`beta/shim.js`): a copy of that half's engine as
// it stood when the scout was asked for, which plays the world's future
// (`foresee`) and is asked nothing else. `made` and `done` count the scouts
// made and let go of, and `wait` slows every answer.
const scouts = {held: {}, made: 0, done: 0, wait: 0};
async function scoutAnswer(pane, how, body) {
  if (scouts.wait) await new Promise(done => setTimeout(done, scouts.wait));
  if (how === "done") {
    if (scouts.held[pane]) scouts.done += 1;
    delete scouts.held[pane];
    return {error: null, scout: "done"};
  }
  if (how === "new" || !scouts.held[pane]) {
    const frame = [...document.querySelectorAll("iframe")].find(held => held.src.includes(`pane=${pane}`));
    if (!frame) return {error: "that world is not there"};
    scouts.held[pane] = worldOf(frame.contentWindow.eval("engine"));
    scouts.made += 1;
  }
  const mine = {...engine};
  const hold = held => {
    for (const key of Object.keys(engine)) delete engine[key];
    Object.assign(engine, held);
  };
  hold({...JSON.parse(JSON.stringify(scouts.held[pane])), sent: [], wait: 0});
  try {
    const answer = engineDebug(body);
    scouts.held[pane] = worldOf(engine);
    return answer;
  } finally {
    hold(mine);
  }
}
async function fetchJSON(path, init = {}) {
  const body = init.body ? JSON.parse(init.body) : null;
  engine.sent.push({path, body});
  if (path === "/saves") return {saves: []};
  // A half reads the world it holds after its twin was played for both.
  if (path === "/state") return engineState();
  if (path === "/forks") return forkEngines(body.count);
  // What the engine's host says of the computer (`/machine-metrics`).
  if (path === "/machine-metrics") return {cpu_percent: 12.5, memory_percent: 40.2};
  if (path === "/autosearch") return toyClimb(body);
  const forked = /^\/debug\?fork=(\d+)/.exec(path);
  if (forked) return forkAnswer(+forked[1], path, body);
  const scouted = /^\/debug\?pane=([a-z]-\w+)&scout=(\w+)$/.exec(path);
  if (scouted) return scoutAnswer(scouted[1], scouted[2], body);
  if (path !== "/debug") return {error: null};
  // A slowed engine, so that a run can be seen on its way.
  if (engine.wait) await new Promise(done => setTimeout(done, engine.wait));
  return engineDebug(body);
}
// One debugger operation on the world `engine` holds, answered at once.
function engineDebug(body) {
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
    // A turn played again is played in a world no swan has struck on it.
    engine.swans = (engine.swans || []).filter(swan => swan.turn < engine.turn);
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
  let error = null, readings = null, terms = null, alerts = null, swan = null, foresight = null;
  // One whole turn by the Civ AI seated.
  const step = () => {
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
    if (engine.scores) engine.scores[engine.turn] = engine.score;
  };
  if (body.op === "step_seat") {
    const seat = engine.current, next = (seat + 1) % SEATS;
    if (next === 0) opens(); else engine.current = next;
    engine.stepped = {seat, actions: 2 * seat, next};
  } else if (body.op === "step" && engine.turn >= 400) {
    // A run nobody stopped must still end.
    error = "this game has no turn left to play";
  } else if (["step", "play", "ahead"].includes(body.op) && engine.over && engine.turn >= engine.over.turn) {
    // Nor is a turn played past the end of a game that is over.
    error = "the game is over";
  } else if (body.op === "step") {
    step();
  } else if (body.op === "play") {
    // A run of steps asked for once, answered with how the seat stood as
    // each turn opened, and what each turn read of a search's lists. It
    // stops where a run stops.
    readings = [];
    terms = engine.objective ? [] : null;
    while (readings.length < body.turns && engine.turn < 400 && !engine.stopped && !(engine.over && engine.turn >= engine.over.turn)) {
      step();
      readings.push(engineSeat());
      if (terms) terms.push(engineTerms(engine.objective));
    }
    if (!readings.length) error = "this game has no turn left to play";
  } else if (body.op === "ahead") {
    // Played on from the end of the seat's line, with the world left on the
    // open turn as it stood. What is seen coming is the test's to say
    // (`engine.nukes`, the turns a device goes off on).
    const home = engine.turn, stood = engine.score, from = marks[marks.length - 1];
    engine.turn = from;
    if (engine.scores && engine.scores[from] !== undefined) engine.score = engine.scores[from];
    readings = [];
    while (readings.length < body.turns && engine.turn < 400 && !engine.stopped) {
      step();
      readings.push(engineSeat());
    }
    alerts = (engine.nukes || []).filter(turn => turn >= from && turn < engine.turn)
      .map(turn => ({turn, kind: "nuclear", text: "Greece detonates a nuclear device"}));
    engine.turn = home;
    engine.score = stood;
    engine.stopped = null;
  } else if (body.op === "foresee") {
    // The world's future, played on a copy from the end of the seat's line
    // to the end of the game, a stretch a request (`debug_foresee`). The
    // world itself is left as it stands. Each turn foreseen has the print
    // the world's own turn is given when the same Civ AI plays it. What is
    // seen coming is the test's to say: `engine.nukes` and `engine.wars`,
    // the turns a device goes off and a war is declared on. The ones on
    // record already when the copy is made are told with it.
    const last = engine.limit || 400, step = engine.seat === "stock" ? 2 : engine.seat ? 1 : 3;
    const noted = turn => [
      ...((engine.nukes || []).includes(turn) ? [{turn, kind: "nuclear", text: "Greece detonates a nuclear device", era: 6, era_name: "atomic"}] : []),
      ...((engine.wars || []).includes(turn) ? [{turn, kind: "war", text: "Rome declares war on Greece", era: 0, era_name: "ancient"}] : [])];
    let made = false;
    if (body.anew || !engine.seer || engine.seer.seat !== engine.seat) {
      const from = marks[marks.length - 1];
      made = true;
      engine.seer = {from, base: engine.worlds[from], seat: engine.seat, turn: from, world: engine.worlds[from],
        prints: [[from, engine.worlds[from]]], ended: false,
        seen: [...new Set([...(engine.nukes || []), ...(engine.wars || [])])].filter(turn => turn < from).sort((one, other) => one - other).flatMap(noted)};
    }
    const seer = engine.seer;
    let played = 0;
    while (played < body.turns && seer.turn < last) {
      seer.seen.push(...noted(seer.turn));
      seer.turn += 1;
      played += 1;
      seer.world = (seer.world * 31 + step) % 2147483647;
      seer.prints.push([seer.turn, seer.world]);
    }
    const done = seer.turn >= last;
    if (done && !seer.ended) { seer.ended = true; seer.seen.push({turn: seer.turn, kind: "over", text: "The game ends", era: 0, era_name: "ancient"}); }
    const known = !made && typeof body.known === "number" ? body.known : null;
    foresight = {from: seer.from, base: seer.base, seat: seer.seat, turn: seer.turn, done, limit: engine.limit || null, played,
      since: known, anew: made, worlds: seer.prints.filter(([turn]) => known === null || turn > known),
      eras: [{turn: seer.from, era: 0, name: "ancient"}], seen: seer.seen.filter(seen => known === null || seen.turn >= known),
      detonations: seer.seen.filter(seen => seen.kind === "nuclear").length, victory: null};
  } else if (body.op === "black_swan") {
    // A black swan strikes the open turn's world (`debug_black_swan`): the
    // turns kept ahead of it are let go of, the open turn is marked again
    // with a print of its own, and whoever played into it still did. Which
    // one is named, or drawn by the seed. The toy's swan halves the score.
    const kind = body.kind || Object.keys(SWANS)[body.seed % 2];
    if (engine.current !== 0) error = "the rivals are playing; a black swan strikes on the seat's own turn";
    else if (!SWANS[kind]) error = `no black swan named ${JSON.stringify(kind)}`;
    else {
      // Swans that struck this turn already are part of the world it is.
      const held = (engine.swans || []).filter(struck => struck.turn <= engine.turn);
      played(engine.by[engine.turn] || null);
      engine.swans = held;
      engine.worlds[engine.turn] = (engine.worlds[engine.turn] * 31 + 7 + body.seed) % 2147483647;
      engine.score = Math.floor(engine.score / 2);
      if (engine.scores) engine.scores[engine.turn] = engine.score;
      swan = {turn: engine.turn, kind, name: SWANS[kind][0], text: `${SWANS[kind][0]}. The score is ${engine.score} now`,
        sites: [], player: null, until: engine.turn, ongoing: false};
      engine.swans.push(swan);
    }
  } else if (body.op === "objective") {
    // A search's lists, read off the toy's seat: a key it has no number for
    // is refused, as the engine refuses it.
    const lists = body.objective || null;
    const keys = lists ? ["need", "avoid", "want", "dont_want"].flatMap(list => (lists[list] || []).map(term => term.key)) : [];
    const missing = keys.find(key => engineNumber(key) === null);
    if (missing !== undefined) error = `seat 0 has no number called ${JSON.stringify(missing)}`;
    else engine.objective = lists;
  } else if (body.op === "remember") {
    // Only a copy made to search is asked this, and only of a half.
    engine.remembers = true;
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
  // The score is a count of the turns played, which the checks of what was
  // and was not played again read, and a walk leaves it alone. A toy asked
  // to keep each turn's score (`engine.scores`) has it put back by a walk,
  // as the real engine puts the whole world back: what a search reads.
  if (engine.scores && ["jump", "back", "forward", "walk", "goto_save"].includes(body.op) &&
      engine.scores[engine.turn] !== undefined) engine.score = engine.scores[engine.turn];
  const answer = engineState(error);
  if (readings !== null) answer.readings = readings;
  if (alerts !== null) answer.alerts = alerts;
  if (swan !== null) answer.black_swan = swan;
  if (foresight !== null) answer.foresight = foresight;
  if (terms !== null) { answer.terms = terms; answer.killed = null; }
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

    def branch(self, name: str = "", wait: bool = True) -> None:
        """Branch: press it, and pick the Civ AI that plays the branch from the list it asks with.

        The branch is made when the Civ AI is picked. Not named here, it is
        the one the timeline has, so the two begin as one world with one Civ
        AI. `wait=False` comes back at once, with the window still held."""
        found = self.ev("(() => { const branch = document.querySelector('[data-dbg=\"split\"]');"
                        " if (!branch) return 'no Branch'; if (branch.disabled) return 'Branch is disabled';"
                        " if (debugAsk) return 'something is already asked';"
                        f" branch.click(); const name = {json.dumps(name)} || debugStrategy();"
                        " const pick = [...document.querySelectorAll('#dbg-ask [data-dbg-ask=\"seat\"]')].find(held => held.value === name);"
                        " if (!pick) return `Branch does not offer ${name}`; if (pick.disabled) return 'the pick is disabled';"
                        " pick.click(); return 'ok'; })()")
        assert found == "ok", f"Branch: {found}"
        if wait:
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

# A game under the debugger opens on its one timeline. Every check but the
# one of the opening view asks for it by name, so that what a game opens on
# is that one check's to say.
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
    # The AI Debugger's seat takes orders by hand; only Edge of Tomorrow's does not.
    assert bar.ev("window.civvisSeatHandsOff") is False

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


#: Whether B holds the turn given, and on every turn it holds the world A
#: holds there: the engine's word for two halves that are the same world.
SAME_WORLDS = (
    "(() => {{ const a = debugSplit.panes.a.marks, b = debugSplit.panes.b.marks; if (!a || !b) return false;"
    " const theirs = new Map(a.turns.map((turn, at) => [turn, a.worlds[at]]));"
    " return b.turns.includes({}) && b.turns.every((turn, at) => theirs.get(turn) === b.worlds[at]); }})()"
).format


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
    # Branch, once a Civ AI is picked for the branch, splits the screen: side
    # by side, and locked. Head to
    # head, the two halves' numbers side by side, is open with it (operator
    # 2026-11-22: "Default the stats window open"): the strip's VS button
    # puts it away and brings it back, and the choice is kept.
    bar.branch()
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
    # A branch that was just made is B, with the Civ AI picked for it when
    # Branch asked (`check_the_box_asks`): two screens are asked nothing.
    assert bar.ev("debugAsk") is None, bar.ev("debugAsk")

    # The host's bar stays up across the top, over the two halves, and its
    # timeline is theirs: one timeline up to the turn the split was made on,
    # and two from there, as the split opens (operator 2026-11-26: "branches
    # should split from 1 to 2... not 2 to 1"), though the two are still the
    # same world. Locked, as a split opens, the host's deck under it
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
    branched(bar, start, start, start, "a split opens as two branches from the turn it was made on")
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
    # The same choice stands at the end of each half's branch in the host's
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
    # Each slider is on its half's turn. The timeline is one up to the turn
    # of the split and branches there, each branch drawn in its half's own
    # colour.
    assert bar.ev(lanes) == [start + 4, start + 4], bar.ev(lanes)
    assert bar.ev("[debugSplit.panes.a.first, debugSplit.panes.b.last]") == [1, start + 4]
    assert bar.ev("['a', 'b'].every(letter => !document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-own`).hidden)")
    bar.until("debugBar.classList.contains('dbg-forked')", "the timeline is branched")
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
    # one world again, turn for turn, and still two branches from the turn of
    # the split.
    bar.click('[data-dbg-split="copy-a"]')
    bar.until("debugSplit.panes.b.numbers.score === debugSplit.panes.a.numbers.score", "B becomes a copy of A")
    assert ops("b")[-1][0] == "copy", ops("b")
    bar.until(SAME_WORLDS(start + 10), "a copy holds the worlds it was copied from")
    branched(bar, start, start + 10, start + 10, "B was made a copy of A")

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
    bar.branch()
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
    bar.branch()
    bar.until(f"debugSplit && {both(start, start)} && debugSplit.panes.a.numbers && debugSplit.panes.b.numbers",
              "both halves say where they stand")
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy]") == ["jack", "jack"]
    a, b = bar.ev("[debugSplit.panes.a.id, debugSplit.panes.b.id]")
    bar.until(f"debugBar.classList.contains('dbg-merged') && {deck_idle}", "the one panel is up")

    # The lock joins the two halves: it stands at the root of the tree,
    # between the middles of their two rows, shut, and the two branch from
    # the turn of the split with one marker there.
    bar.until("debugSplit.panes.a.marks && debugSplit.panes.b.marks", "both halves say what they hold")
    branched(bar, start, start, start, "a split opens as two branches")
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
    # They are one world still, on two branches.
    bar.until(SAME_WORLDS(start + 1), "B says it holds the turn its twin played")
    branched(bar, start, start + 1, start + 1, "twins were played a turn")

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
    # That turn is two worlds. The timeline still branches on the turn of
    # the split, and says so there.
    branched(bar, start, start + 2, start + 2, "another Civ AI played B's turn")
    hover = bar.ev("debugTurnWords(debugLaneModels.b, debugLaneModels.b.branches)")
    assert hover.endswith("A and B part here"), hover

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
    # seats. The toy's worlds are then A's turn for turn, and the two are
    # still two branches from the turn of the split. (They are still not
    # twins: the layers and the fog differ.)
    bar.until(SAME_WORLDS(start + 4), "halves that hold the same worlds again")
    branched(bar, start, start + 4, start + 4, "B played its turns again as A played them")
    # A -> B makes the two the same in everything, and they are played once again.
    bar.click('[data-dbg-split="copy-a"]')
    bar.until(same_score, "B becomes a copy of A")
    branched(bar, start, start + 4, start + 4, "B was made a copy of A")
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
    # A has played a turn B has not, by itself: A's branch reaches a turn
    # further, and each half's marker stands where the half does. What each
    # has played since the split is its own, in its colour.
    bar.until(f"debugSplit.panes.a.marks.turns.includes({start + 6})", "A says it holds the turn it played")
    branched(bar, start, start + 6, start + 5, "A played on alone")
    own = bar.ev("['a', 'b'].map(letter => { const band = document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-own`);"
                 " return !band.hidden && band.getBoundingClientRect().width > 0"
                 " && getComputedStyle(band.parentNode).visibility === 'visible'; })")
    assert own == [True, True], own
    assert bar.ev("debugSplitFork()")["turn"] == start
    # Apart, the lock cannot close; B changes a layer of its own meanwhile.
    assert bar.ev(f"[{lock}.disabled, {lock}.getAttribute('aria-pressed')]") == [True, "false"]
    assert "same turn" in bar.ev(f"{lock}.title"), bar.ev(f"{lock}.title")
    bar.ev(half("b", json.dumps("document.querySelector('[data-dbg-pop=\"layers\"]').click(), true")))
    bar.ev(half("b", json.dumps("document.querySelector('#dbg-pop-layers [data-dbg-layer=\"cities\"]').click(), true")))
    bar.until(half("b", "'state.debug.layers[1].on'"), "B switches a layer on by itself")
    bar.ev(half("b", json.dumps("document.querySelector('[data-dbg=\"next\"]').click(), true")))
    bar.until(f"{both(start + 6, start + 6)} && !{lock}.disabled", "the lock is offered on the same turn")
    # B played the same turn by itself, with the same Civ AI, and holds the
    # world A does, on its own branch.
    bar.until(SAME_WORLDS(start + 6), "B says it holds the turn it played")
    branched(bar, start, start + 6, start + 6, "each half played the same turn by itself")
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
    # go to. Locked, the other half is brought along. Past the turn of the
    # split a mark both halves hold stands on each branch, in its own row
    # (what both hold on the trunk is said once there).
    mark = "document.querySelector('#debugbar [data-dbg-lane=\"{}\"] .dbg-marks {}')".format
    stop = f'i.stop[data-dbg-turn="{start + 16}"]'
    flag = lambda name: f'i[title^="{name}:"]'

    def point(name: str) -> None:
        bar.ev(f"document.querySelector('#dbg-pop-saves [data-dbg-field=pointname]').value = '{name}', true")
        bar.click('[data-dbg="point"]')
        bar.until(f"{mark('a', flag(name))} && {mark('b', flag(name))} && debugSplit.panes.b.marks.flags.some("
                  f"point => point.name === '{name}') && {deck_idle}",
                  f"the save point {name}, set on the one panel, is both halves' and stands on each branch")

    point("fork")
    bar.click('[data-dbg="break-plus"]')
    bar.until(f"{mark('a', stop)} && {mark('b', stop)} && debugSplit.panes.a.marks.stops.includes({start + 16})"
              f" && {deck_idle}", "a breakpoint turn is both halves' and stands on each branch")
    assert "play to it" in bar.ev(f"{mark('b', stop)}.title"), bar.ev(f"{mark('b', stop)}.title")
    bar.ev(f"{mark('b', stop)}.click(), true")
    bar.until(f"{both(start + 16, start + 16)} && {deck_idle}", "a breakpoint clicked on the timeline is played to by both halves")
    branched(bar, start, start + 16, start + 16, "both halves were played to the breakpoint as one")
    # A save point and a breakpoint on the same turn stand at the two edges
    # of a branch, the save point over the breakpoint, as on a whole screen's.
    point("there")
    for letter in "ab":
        apart = bar.ev(f"(() => {{ const box = held => held.getBoundingClientRect(), pin = box({mark(letter, flag('there'))}),"
                       f" dot = box({mark(letter, stop)}), sky = box(document.querySelector('[data-dbg-lane=\"{letter}\"] .dbg-sky'));"
                       " return [pin.bottom, dot.top, Math.abs(pin.left - dot.left), sky.top - pin.top, dot.bottom - sky.bottom]; })()")
        assert apart[0] <= apart[1] and apart[2] < 1 and 0 < apart[3] < 6 and 0 < apart[4] < 6, (letter, apart)
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
    # The save point both hold stands on each branch, A's over B's, and
    # either is pressed.
    apart = bar.ev(f"(() => {{ const box = held => held.getBoundingClientRect(), a = {mark('a', flag('fork'))}, b = {mark('b', flag('fork'))};"
                   " const hit = held => document.elementFromPoint((box(held).left + box(held).right) / 2, (box(held).top + box(held).bottom) / 2);"
                   " return [box(b).top > box(a).bottom, hit(a) === a, hit(b) === b]; })()")
    assert apart == [True, True, True], apart
    # A half's marker stays in the half's own row, A's over the trunk and
    # B's under it.
    pins = bar.ev("['a', 'b'].map(letter => { const cursor = document.querySelector(`[data-dbg-lane=\"${letter}\"] .dbg-cursor`);"
                  " const pill = cursor.querySelector('b').getBoundingClientRect(), row = cursor.closest('.dbg-rail').getBoundingClientRect();"
                  " return [+cursor.querySelector('b').textContent, pill.top >= row.top - 2 && pill.bottom <= row.bottom + 2]; })")
    assert pins == [[start + 6, True], [start + 7, True]], pins
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
    bar.branch()
    bar.until(f"debugSplit && {both(back, back)} && {said} && {deck_idle}", "both branches say where they stand")
    # Two branches are the most there are: a split has no Branch, and each
    # branch's Civ AI is picked at the end of its branch.
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

    # The tree grows from the left, one timeline into two (operator,
    # 2026-11-26: "branches should split from 1 to 2... not 2 to 1"): the
    # lock and the game's first turn at the root of the trunk, and nothing of
    # either branch left of it. Each branch ends in what is its own: past its
    # rail, its letter, its picker and its ✕, and then the game's last turn,
    # which is said once for the tree.
    root = bar.ev("['[data-dbg-lock]', '[data-dbg-lane=\"\"] .dbg-start', '[data-dbg-lane=\"\"] .dbg-rail'].map(name => {"
                  " const box = document.querySelector('#debugbar ' + name).getBoundingClientRect(); return [box.left, box.right]; })")
    assert root[0][1] <= root[1][0] and root[1][1] <= root[2][0], root
    for letter in "ab":
        places = bar.ev(
            f"['[data-dbg-close=\"{letter}\"]', '[data-dbg-lane=\"{letter}\"] .dbg-lane-name', '[data-dbg-agent=\"{letter}\"]',"
            f" '[data-dbg-lane=\"{letter}\"] .dbg-rail', '{LANE} .dbg-finish'].map(name => {{"
            " const box = document.querySelector('#debugbar ' + name).getBoundingClientRect();"
            " return [box.left, box.right, box.top, box.bottom]; })")
        cross, name, picker, rail, finish = places
        assert root[2][0] <= rail[0] and rail[1] <= name[0] and name[1] <= picker[0] and picker[1] <= cross[0], (letter, places)
        assert cross[1] > cross[0] and cross[0] - picker[1] <= 12 and cross[1] <= finish[0], (letter, places)
        assert rail[2] <= (cross[2] + cross[3]) / 2 <= rail[3] and rail[2] <= (picker[2] + picker[3]) / 2 <= rail[3], (letter, places)
        assert bar.ev(f"document.querySelector('[data-dbg-close=\"{letter}\"]').textContent") == "✕"
    # In the page as it is written they follow their rail too, so the keys
    # reach them where the eye finds them.
    order = bar.ev("['a', 'b'].map(letter => [...document.querySelector(`[data-dbg-lane=\"${letter}\"]`).children]"
                   ".map(held => held.className.split(' ')[0]))")
    assert order == [["dbg-start", "dbg-rail", "dbg-lane-name", "dbg-agent", "dbg-close", "dbg-finish"]] * 2, order
    assert bar.ev("[...document.querySelector('#debugbar .dbg-time').children].map(held => held.className.split(' ')[0])") \
        == ["dbg-who", "dbg-tie", "dbg-lane", "dbg-lane", "dbg-lane"]

    # Locked, one turn on: A walks to the turn it holds, and B, which holds
    # none, plays it. Jack plays B's seat too, so the toy's two worlds are the
    # same on that turn, and the fork stays where the branch was made.
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(back + 1, back + 1)} && {deck_idle} && {SAME_WORLDS(back + 1)}", "both branches go one turn on")
    assert (ops("a"), ops("b")) == (["forward"], ["branch", "step"]), (ops("a"), ops("b"))
    branched(bar, back, back + 1, back + 1, "the branch played the turn the timeline holds, the same way")

    # Another Civ AI in the branch, and one more turn: two worlds from there.
    bar.ev("(() => { const select = document.querySelector('#debugbar [data-dbg-agent=\"b\"]'); select.value = 'stock';"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    bar.until(half("b", "'engine.seat'") + f" === 'stock' && {deck_idle}", "Stock holds the branch's seat")
    bar.click('[data-dbg="next"]')
    bar.until(f"{both(back + 2, back + 2)} && {deck_idle} && debugSplit.panes.b.marks.turns.includes({back + 2})",
              "both branches go another turn on")
    assert (ops("a"), ops("b")) == (["forward"] * 2, ["branch", "step", "step"]), (ops("a"), ops("b"))
    branched(bar, back, back + 2, back + 2, "another Civ AI played the branch's turn")
    # B's branch is Jack's for the turn it played as A did, and Stock's after.
    # That one turn is all on the branch's way to its row, where no word is
    # drawn: it is named nowhere, and the branch's letter goes on the first
    # stretch in its row.
    assert names("a") == ["A · Jack"] and names("b") == ["", "B · Stock"], (names("a"), names("b"))
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
    # branch is asked nothing, and the timeline branches there at once.
    bar.branch()
    bar.until(f"debugSplit && {both(back + 2, back + 2)} && {said} && {deck_idle}", "a second branch opens")
    assert (ops("a"), ops("b")) == ([], []), (ops("a"), ops("b"))
    branched(bar, back + 2, back + 2, back + 2, "a branch made on the last turn played")
    # Played on, the two are told of each turn one after the other, so one is
    # a turn ahead for a moment every turn. The timeline stays branched
    # throughout: every change of it is counted here.
    bar.ev("(() => { window.flips = 0; let was = debugBar.classList.contains('dbg-forked');"
           " new MutationObserver(() => { const now = debugBar.classList.contains('dbg-forked');"
           " if (now !== was) { was = now; window.flips += 1; } }).observe(debugBar, {attributes: true, attributeFilter: ['class']});"
           " return true; })()")
    for turn in (back + 3, back + 4):
        bar.click('[data-dbg="next"]')
        bar.until(f"{both(turn, turn)} && {deck_idle} && debugSplit.panes.b.marks.turns.includes({turn})",
                  "both branches are played on as one")
    branched(bar, back + 2, back + 4, back + 4, "two branches played the same way")
    assert bar.ev("window.flips") == 0, "the timeline went back to one for a moment while two branches were played as one"
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
    """What the box asks: Switch or Branch for another Civ AI on the one timeline, and which Civ AI plays the branch Branch makes.

    Operator, 2026-11-22: "editable branches should be able to switch
    strategies. leading strategies should be editable and when edited to a
    different strategy, should ask to switch or branch". Then, 2026-11-24:
    "players can branch if they want (they should choose the new strategy
    when they branch)": Branch asks first, and the branch is made when its
    Civ AI is picked.
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

    # Branch asks which Civ AI plays the branch before it makes one: every
    # Civ AI the page offers, as its picker lists them, the one this timeline
    # has marked as its own, under Branch, which shows it is asking. Nothing
    # is said to the engine and there is no branch yet.
    bar.forget()
    bar.click('[data-dbg="split"]')
    assert bar.ev("[debugAsk, debugSplit]") == [{"kind": "branch"}, None], bar.ev("debugAsk")
    seen = bar.ev(asked)
    assert seen["title"] == f"Branch on turn {turn}: which Civ AI plays it?", seen
    assert seen["buttons"] == [["seat", "jack", "1211 - JackOfAllTradesplays this timeline", False],
                               ["seat", "stock", "1000 - ByTheBook", False], ["close", "", "Cancel", False]], seen
    assert bar.ev(f"{under}('#debugbar [data-dbg=\"split\"]')"), seen
    lit = ("(() => { const branch = document.querySelector('#debugbar [data-dbg=\"split\"]');"
           " return [branch.classList.contains('on'), branch.getAttribute('aria-expanded')]; })()")
    assert bar.ev(lit) == [True, "true"], bar.ev(lit)
    assert bar.sent() == [], bar.sent()
    # Not answered, there is no branch: Cancel, Escape, a press anywhere else
    # on the page, and Branch pressed again each let it go.
    bar.click('#dbg-ask [data-dbg-ask="close"]')
    assert bar.ev(asked) is None and bar.ev("[debugAsk, debugSplit]") == [None, None] and bar.ev(lit) == [False, "false"]
    bar.click('[data-dbg="split"]')
    bar.ev("document.querySelector('#dbg-ask button').dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true})), true")
    assert bar.ev(asked) is None and bar.ev("[debugAsk, debugSplit]") == [None, None]
    bar.click('[data-dbg="split"]')
    bar.ev("document.querySelector('#dbg-ask h4').dispatchEvent(new PointerEvent('pointerdown', {bubbles: true})), true")
    assert bar.ev(asked) is not None
    bar.ev("document.getElementById('maparea').dispatchEvent(new PointerEvent('pointerdown', {bubbles: true})), true")
    assert bar.ev(asked) is None and bar.ev("[debugAsk, debugSplit]") == [None, None]
    bar.click('[data-dbg="split"]')
    assert bar.ev(asked) is not None
    # (A press on Branch is a pointer going down on it and then its click.)
    bar.ev("document.querySelector('#debugbar [data-dbg=\"split\"]').dispatchEvent(new PointerEvent('pointerdown', {bubbles: true})), true")
    assert bar.ev(asked) is not None, "Branch pressed again let go of its question before its click"
    bar.click('[data-dbg="split"]')
    assert bar.ev(asked) is None and bar.ev("[debugAsk, debugSplit]") == [None, None] and bar.sent() == [], bar.sent()
    # Another Civ AI picked in the timeline's picker while Branch asks is
    # asked about instead: one question at a time.
    bar.click('[data-dbg="split"]')
    pick("stock")
    assert bar.ev("debugAsk") == {"kind": "switch", "name": "stock"} and bar.ev(lit) == [False, "false"]
    bar.click('#dbg-ask [data-dbg-ask="close"]')
    assert bar.ev(seated) == ["jack", "jack"] and bar.sent() == [], (bar.ev(seated), bar.sent())

    # Answered, the branch is made for the Civ AI picked, in one step: it
    # begins seated with it, this timeline keeps its own, and the box has
    # nothing left to ask.
    bar.click('[data-dbg="split"]')
    bar.click('#dbg-ask [data-dbg-ask="seat"][value="stock"]')
    bar.until(f"debugSplit && {both(turn)} && {said} && {deck_idle}", "Branch splits the timeline when a Civ AI is picked")
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy, debugAsk]") == ["jack", "stock", None]
    assert bar.ev(asked) is None
    bar.until(half("a", "'debugStrategy()'") + " === 'jack' && " + half("b", "'debugStrategy()'") + " === 'stock'"
              " && " + half("b", "'engine.seat'") + " === 'stock'", "the branch is seated with the Civ AI picked for it")
    assert bar.ev("['a', 'b'].map(letter => debugLanes[letter].agent.value)") == ["jack", "stock"]
    assert [call for call in bar.sent() if call.get("op") != "adopt"] == [], bar.sent()
    # A split has no Branch, and so nothing to ask: each branch's own picker
    # seats another Civ AI in it.
    bar.ev("(() => { const select = document.querySelector('#debugbar [data-dbg-agent=\"b\"]'); select.value = 'jack';"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    bar.until(half("b", "'debugStrategy()'") + f" === 'jack' && {deck_idle}", "the branch is seated from its picker")
    assert bar.ev(asked) is None and bar.ev("debugAsk") is None
    bar.click('[data-dbg-close="b"]')
    bar.until("!debugSplit", "the branch is closed")
    # The Civ AI the timeline has may play the branch as well: the two are
    # then one world until one of them is played another way.
    bar.click('[data-dbg="split"]')
    bar.click('#dbg-ask [data-dbg-ask="seat"][value="jack"]')
    bar.until(f"debugSplit && {both(turn)} && {said} && {deck_idle}", "Branch splits the timeline for the Civ AI it has")
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy, debugAsk]") == ["jack", "jack", None]
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the branch is dropped")
    assert bar.ev(asked) is None and bar.ev("debugAsk") is None
    assert bar.ev("[state.turn, engine.seat]") == [turn, "jack"]
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_the_game_opens_on_one_timeline(bar: Bar, address: str) -> None:
    """The opening view: a game under the debugger is drawn on its one timeline, with the top-rated Civ AI, and asks nothing.

    Operator, 2026-11-24: "ai debugger should start with 1 strategy (maybe
    the best strategy). players can branch if they want (they should choose
    the new strategy when they branch)". Before it, from 2026-11-22, every
    game opened on two tracks ("the opening view should go to the 2 track
    immediately"): a page asked for with `tracks=2` still does, and that
    opening asks nothing either ("branch b should only ask when branch is
    selected. not just auto ask rom the start").
    """
    both = "debugSplit.panes.a.turn === {0} && debugSplit.panes.b.turn === {0}".format
    deck_idle = "!!debugSplitDeck() && debugSplitDeck().idle && !debugSplit.busy"
    said = "debugSplit.panes.a.marks && debugSplit.panes.b.marks"
    ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"
    seated = ("[document.getElementById('autoplaystrategy').value, document.getElementById('dbg-agent').value,"
              " localStorage.getItem(AUTOPLAY_STRATEGY_KEY)]")
    shown = lambda selector: bar.ev(f"(() => {{ const el = document.querySelector({json.dumps(selector)});"
                                    " return !!el && !el.hidden && el.offsetParent !== null; })()")

    def settled() -> None:
        """Long enough for a page that was going to split to have done it."""
        bar.ev("new Promise(done => setTimeout(() => done(true), 250))")

    def opened(search: str) -> None:
        """The page again, loaded, with no game drawn yet."""
        bar.dev.call("Page.navigate", {"url": address + search})
        bar.until(f"{ready} && location.search === {json.dumps(search)}", "the page as it is opened, loaded")
        assert bar.ev("typeof debugSplit !== 'undefined' && debugSplit === null && state === null")

    # Every other check asked for the one timeline. The page as anyone opens
    # it, by somebody whose last game here was played by another Civ AI: the
    # page's picker begins on the one it remembers.
    opened("")
    # The checks before this one clicked the timeline to travel, and a page
    # opened after that does not say Click to travel. This one is opened as
    # by somebody who never has.
    assert bar.ev(f"[debugTravelKnown, localStorage.getItem('{TRAVEL_KEY}')]") == [True, "known"]
    bar.ev(f"localStorage.removeItem('{TRAVEL_KEY}'), debugTravelKnown = false, true")
    bar.ev("localStorage.setItem(AUTOPLAY_STRATEGY_KEY, 'stock'), document.getElementById('autoplaystrategy').value = 'stock', true")
    # Nothing is pressed: the first state is drawn on one screen, with one
    # timeline, and the best Civ AI there is takes the seat, the one with the
    # highest Elo. The engine is told of it, and of nothing else.
    bar.ev("render(engineState()), true")
    assert bar.ev("[debugSplit, debugHeld, document.getElementById('dbg-veil'), document.getElementById('debugsplit')]") == [None, False, None, None]
    bar.idle("debugOpenedGame === '7' && engine.seat === 'jack'", "the game is opened, with the best Civ AI in its seat")
    assert bar.sent() == [{"op": "status", "strategy": "jack"}], bar.sent()
    assert bar.ev("[debugBestAgent(), debugOpenedOnBest]") == ["jack", True]
    assert bar.ev(seated) == ["jack", "jack", "jack"], bar.ev(seated)
    settled()
    assert bar.ev("[debugSplit, debugAsk, document.getElementById('dbg-ask').hidden]") == [None, None, True]
    assert bar.ev("['', 'a', 'b'].map(key => getComputedStyle(document.querySelector(`[data-dbg-lane=\"${key}\"]`)).display !== 'none')") \
        == [True, False, False]
    # A second timeline is the person's to make: Branch is there, under the marker.
    assert shown("#debugbar [data-dbg=split]") and shown("#debugbar .dbg-who")
    # What the timeline says is how it is used: Click to travel.
    assert travel_said(bar) == {"words": "◀Click to travel▶", "inside": True, "clear": True, "size": 15,
                                "calling": "dbg-travel-call", "under": "dbg-slider"}, travel_said(bar)
    # The best Civ AI is seated once a page. After that the seat is whoever
    # is picked for it, in this game and in the next one.
    bar.forget()
    seat_the_timeline(bar, "stock")
    bar.idle("engine.seat === 'stock'", "another Civ AI is picked for the timeline")
    bar.ev("Object.assign(engine, {seed: 8, turn: 1, marks: [1], worlds: {1: 1}, by: {}, score: 0, eras: [{era: 0, name: 'ancient', turn: 1}]}),"
           " engine.sent.length = 0, render(engineState()), true")
    settled()
    assert bar.ev("[debugSplit, debugOpenedGame]") == [None, "8"]
    assert bar.ev(seated) == ["stock", "stock", "stock"] and bar.sent() == [], (bar.ev(seated), bar.sent())
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"

    # A page whose Civ AIs nothing has rated has no best: the seat is left as it is.
    opened("")
    bar.ev("RULES.strategies.forEach(entry => { delete entry.elo; }), document.getElementById('autoplaystrategy').value = 'stock', true")
    bar.ev("render(engineState()), true")
    settled()
    assert bar.ev("[debugBestAgent(), debugOpenedOnBest, debugOpenedGame, debugSplit]") == [None, True, "7", None]
    assert bar.ev(seated) == ["stock", "stock", "stock"] and bar.sent() == [], (bar.ev(seated), bar.sent())

    # Two tracks at once is still a way to open a game, for a page that asks
    # for it. The best Civ AI is seated first, and both tracks begin on it.
    opened("?tracks=2")
    bar.ev("document.getElementById('autoplaystrategy').value = 'stock', render(engineState()), true")
    bar.settled()
    bar.until(f"debugSplit && debugSplit.shown && {both(1)} && {said} && {deck_idle}", "a page that asks for two tracks opens on them")
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy, debugOpenedGame]") == ["jack", "jack", "7"]
    assert bar.sent() == [{"op": "status", "strategy": "jack"}], bar.sent()
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"

    opened("?tracks=2")
    bar.ev(f"localStorage.removeItem('{TRAVEL_KEY}'), debugTravelKnown = false, true")
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
    branched(bar, 1, 1, 1, "a game opens on two tracks that are one world, branched on its first turn")
    # Nothing was asked of the world that waits under the two halves: the
    # best Civ AI was the one its picker began on.
    assert bar.sent() == [], bar.sent()
    # And nothing is asked of the person: nobody pressed Branch.
    assert bar.ev("[debugAsk, document.getElementById('dbg-ask').hidden]") == [None, True]
    # What the timeline says instead is how it is used: Click to travel, on
    # the trunk, ahead of the marker, over the hand on A.
    assert travel_said(bar) == {"words": "◀Click to travel▶", "inside": True, "clear": True, "size": 14,
                                "calling": "dbg-travel-call", "under": "dbg-slider-a"}, travel_said(bar)
    # B's own picker seats another Civ AI in it.
    bar.ev("(() => { const select = document.querySelector('#debugbar [data-dbg-agent=\"b\"]'); select.value = 'stock';"
           " select.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    bar.until("debugSplit.panes.b.frame.contentWindow.eval('engine.seat') === 'stock' && " + deck_idle, "B is seated from its picker")
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
    assert bar.ev("[debugAsk, document.getElementById('dbg-ask').hidden]") == [None, True]
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


# Where the timeline says Click to travel, while it says it (None when it
# does not): its words, whether it stands inside the rail and clear of the
# marker, how large it is and whether it calls, and what a click on it
# reaches, which is the rail's slider under it.
TRAVEL_KEY = "civvis-debug-click-to-travel"


def travel_said(bar: Bar):
    return bar.ev(
        "(() => { const words = document.querySelector('" + LANE + " .dbg-travel');"
        " if (words.hidden || words.offsetParent === null) return null;"
        " const at = words.getBoundingClientRect(), rail = words.closest('.dbg-rail').getBoundingClientRect();"
        " const marker = document.querySelector('" + LANE + " .dbg-cursor b').getBoundingClientRect(), style = getComputedStyle(words);"
        " return {words: words.textContent,"
        " inside: at.top >= rail.top - 4 && at.bottom <= rail.bottom + 4 && at.left >= rail.left && at.right <= rail.right,"
        " clear: marker.width === 0 || at.left > marker.right || at.right < marker.left,"
        " size: parseFloat(style.fontSize), calling: style.animationName,"
        " under: document.elementFromPoint(at.left + at.width / 2, at.top + at.height / 2 - 4).id}; })()")


def check_the_timeline_says_click_to_travel(bar: Bar) -> None:
    """The timeline says Click to travel, loud, until it has been clicked to travel, and then never again.

    Operator, 2026-11-22: "have the initial timeline say click to travel so
    people understand the mechanism. make the click to travel very visible
    at the start and then not appear once they understand".
    """
    assert bar.ev("debugSplit === null && !matchMedia('(prefers-reduced-motion: reduce)').matches")
    bar.ev(f"localStorage.removeItem('{TRAVEL_KEY}'), debugTravelKnown = false, render(engineState()), true")
    bar.idle("true", "the timeline can be travelled")
    # Large, calling, in the open rail beside the marker, and a click on it is the rail's.
    assert travel_said(bar) == {"words": "◀Click to travel▶", "inside": True, "clear": True, "size": 15,
                                "calling": "dbg-travel-call", "under": "dbg-slider"}, travel_said(bar)
    # A turn gone to some other way is not the timeline clicked: it is still said.
    start = bar.ev("state.turn")
    bar.type_turn(start + 2)
    bar.idle(f"state.turn === {start + 2}", "the turn box plays on")
    assert travel_said(bar) is not None
    # It is said while a click would travel, and not while turns are played.
    bar.ev("engine.wait = 120, true")
    bar.click('[data-dbg="next"]')
    bar.until("debugBusy", "a turn is played")
    assert travel_said(bar) is None
    bar.ev("engine.wait = 0, true")
    bar.idle(f"state.turn === {start + 3}", "the turn is played")
    assert travel_said(bar) is not None
    # Clicked to travel once, it goes, and is not said again, in this game or
    # the next: a page opened later reads it (`check_the_game_opens_on_one_timeline`).
    bar.drag("dbg-slider", start + 1)
    bar.idle(f"state.turn === {start + 1}", "the timeline travels")
    assert bar.ev(f"[debugTravelKnown, localStorage.getItem('{TRAVEL_KEY}')]") == [True, "known"]
    assert bar.ev(f"document.querySelector('{LANE} .dbg-travel').classList.contains('dbg-travel-gone')"), "it does not go, it vanishes"
    bar.until(f"document.querySelector('{LANE} .dbg-travel').hidden", "the words go")
    bar.ev("render(engineState()), true")
    assert travel_said(bar) is None
    # A mark on the rail clicked (an era's name) is the timeline clicked too;
    # a save point gone to from its panel is not.
    bar.ev(f"localStorage.removeItem('{TRAVEL_KEY}'), debugTravelKnown = false, render(engineState()), true")
    assert travel_said(bar) is not None
    bar.click(f"{LANE} .dbg-marks [data-dbg-turn]")
    bar.idle("true", "the era is gone to")
    assert bar.ev(f"[debugTravelKnown, localStorage.getItem('{TRAVEL_KEY}')]") == [True, "known"]
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
    bar.branch(wait=False)
    # At once, when the Civ AI is picked: the split is made and its halves are
    # loading, unseen, and the window is as it was. The box is still the one
    # timeline's, takes no press, asks nothing more, and Branch says it is at work.
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
    # The box asks nothing of two screens: the branch's Civ AI was picked
    # when Branch asked, before there was a branch.
    assert bar.ev("[debugAsk, document.getElementById('dbg-ask').hidden]") == [None, True]
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
    bar.branch(wait=False)
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
    bar.branch(wait=False)
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
    bar.branch()
    assert bar.ev("[debugSplit.shown, moves]") == [True, 0]
    bar.click('[data-dbg-split="drop-a"]')
    bar.until("!debugSplit", "the branch is dropped")
    assert bar.ev("moves") == 0
    bar.dev.call("Emulation.setEmulatedMedia", {"features": []})
    bar.until("!matchMedia('(prefers-reduced-motion: reduce)').matches", "motion is as it was")
    bar.branch()
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
    assert bar.ev("debugAsk") == {"kind": "branch"}
    assert bar.ev("(() => { const from = document.querySelector('#debugbar [data-dbg=\"split\"]').getBoundingClientRect(),"
                  " at = document.getElementById('dbg-ask').getBoundingClientRect(); return Math.round(at.top - from.bottom); })()") == 6
    # Moved over the map, it is still asked, and is answered where it stands.
    before = bar.ev(f"{box}('dbg-ask')")
    drag("dbg-ask", 300, 200)
    assert near(bar.ev(f"{box}('dbg-ask')")[:2], [before[0] + 300, before[1] + 200])
    assert bar.ev("debugAsk") == {"kind": "branch"}
    bar.click('#dbg-ask [data-dbg-ask="seat"][value="stock"]')
    bar.until("debugSplit && debugSplit.panes.b.frame.contentWindow.eval('engine.seat') === 'stock'", "the branch is made from the moved question")
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "the branch is dropped")
    bar.ev("for (const key of Object.keys(debugWindowPlace)) delete debugWindowPlace[key]; true")
def check_autosearch(bar: Bar, address: str) -> None:
    """Autosearch: the button, its panel, a search played on copies of the engine, and the line it found played on the page.

    Operator, 2026-11-22: "build out autosearch button in the ai debugger.
    make it a bit bigger than the others. autosearch should allow use to use
    some fraction of the cores of this machine to each run a search up and
    down the timeline. allow a reasonable objective function to be chosen.
    score or prod or city or fpt or gpt".

    The toy's score rises by two a turn under `stock` and by one under
    `jack`, so the best line there is from any turn is `stock` alone.
    """
    ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"
    bar.dev.call("Page.navigate", {"url": f"{address}{ONE_TIMELINE}"})
    bar.until(f"{ready} && location.search === '{ONE_TIMELINE}' && state === null", "the one timeline, loaded again")
    # A walk back puts the score back with the world, as the real engine does.
    bar.ev("localStorage.removeItem('civvis-debug-search-v1'), localStorage.removeItem('civvis-debug-search-v2'), engine.scores = {[engine.turn]: engine.score}, render(engineState()), true")
    for turn in (2, 3, 4):
        bar.click('[data-dbg="next"]')
        bar.idle(f"state.turn === {turn}", f"turn {turn} is played")
    start, score = 4, bar.ev("engine.score")
    forked = "engine.sent.filter(call => call.path.startsWith('/debug?fork='))"
    copies = "engine.sent.filter(call => call.path === '/forks').map(call => call.body.count)"
    pick = lambda field, value: bar.ev(
        f"(() => {{ const held = document.getElementById('dbg-search-{field}'); held.value = '{value}';"
        " held.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    shown = "!document.getElementById('dbg-search').hidden"
    over = "!!debugSearch && !debugSearch.live"

    # The button stands with Layers and Breakpoints, after them, and is a
    # size larger than any other tool at either end of the deck. Auto-play
    # is the one exception in depth: the operator made it 60% deeper than the
    # tools (2026-11-26: "make the autoplay button 60% deeper please"), 42px,
    # and its type stays the deck's.
    sizes = bar.ev("(() => { const mine = document.querySelector('[data-dbg-search=\"open\"]');"
                   " const run = document.querySelector('#debugbar [data-dbg=\"autoplay\"]');"
                   " const tools = [...document.querySelectorAll('#debugbar .dbg-cluster button')].filter(held => held !== mine && held.offsetParent);"
                   " const tall = held => held.getBoundingClientRect().height, type = held => parseFloat(getComputedStyle(held).fontSize);"
                   " return {mine: [tall(mine), type(mine)], most: [Math.max(...tools.filter(held => held !== run).map(tall)), Math.max(...tools.map(type))],"
                   " run: tall(run), word: mine.textContent.trim(), after: mine.previousElementSibling.dataset.dbgPop,"
                   " side: !!mine.closest('.dbg-left .dbg-cluster')}; })()")
    assert sizes["mine"][0] > sizes["most"][0] and sizes["mine"][1] > sizes["most"][1], sizes
    assert round(sizes["run"]) == 42, sizes
    assert (sizes["word"], sizes["after"], sizes["side"]) == ("Autosearch", "breaks", True), sizes

    # Its panel: what to make the most of, in the operator's order first, how
    # far ahead, and how much of this computer, from one core to all of them.
    bar.click('[data-dbg-search="open"]')
    assert bar.ev(shown)
    options = lambda field: bar.ev(f"[...document.getElementById('dbg-search-{field}').options].map(held => [held.value, held.textContent])")
    # The four lists it is judged by (operator 2026-11-23): what the seat
    # Needs, Must Avoid, Wants and Doesn't Want. It opens on win, do not
    # lose, and the most score.
    lists = bar.ev("[...document.querySelectorAll('#dbg-search-lists .dbg-search-list')].map(list => [list.dataset.list,"
                   " [...list.querySelectorAll('.dbg-search-term')].map(row => [...row.querySelectorAll('select, input')].map(field => field.value))])")
    assert lists == [["need", [["won", "==", "1", "end"]]], ["avoid", [["lost", "==", "1", "always"]]],
                     ["want", [["score", "1"]]], ["dont_want", []]], lists
    # A term is added, read as it is typed, and taken out again.
    bar.click('#dbg-search [data-dbg-search="add"][value="dont_want"]')
    term = "document.querySelector('#dbg-search-lists [data-list=dont_want] [data-term={}]')".format
    bar.ev(f"(() => {{ const key = {term('key')}; key.value = 'yields.faith'; key.dispatchEvent(new Event('change', {{bubbles: true}})); return true; }})()")
    bar.ev(f"(() => {{ const weight = {term('weight')}; weight.value = '2'; weight.dispatchEvent(new Event('change', {{bubbles: true}})); return true; }})()")
    assert bar.ev("debugSearchSet.lists.dont_want") == [{"key": "yields.faith", "weight": 2}]
    assert bar.ev("JSON.parse(localStorage.getItem('civvis-debug-search-v2')).lists.dont_want") == [{"key": "yields.faith", "weight": 2}]
    bar.click('#dbg-search [data-dbg-search="remove"][data-list="dont_want"]')
    assert bar.ev("debugSearchSet.lists.dont_want") == []
    cores = bar.ev("Math.min(64, navigator.hardwareConcurrency || 4)")
    offered = options("cores")
    assert offered[0][1] == f"1 of {cores} cores", offered
    assert cores == 1 or offered[-1] == ["1", f"{cores} of {cores} cores · all"], offered
    assert bar.ev("['turns', 'cores'].map(field => document.getElementById('dbg-search-' + field).value)")[:1] == ["30"]
    assert bar.ev("document.getElementById('dbg-search-half').hidden") is True
    about = bar.ev("document.getElementById('dbg-search-about').textContent")
    assert about.startswith(f"From turn {start}: which Civ AIs should hold the seat, and when, to meet these lists by turn {start + 30}."), about
    # A panel of the deck that opens takes its place, and it takes theirs.
    bar.click('[data-dbg-pop="layers"]')
    assert bar.ev(f"[{shown}, debugPopOpen]") == [False, "layers"]
    bar.click('[data-dbg-search="open"]')
    assert bar.ev(f"[{shown}, debugPopOpen]") == [True, None]
    # It is a window as they are (`check_windows_move_by_their_titles`): it
    # opens under its button, is moved by its title, stays where it was put
    # when it is drawn again, and a double click on the title puts it back.
    place = ("(() => { const at = document.getElementById('dbg-search').getBoundingClientRect(),"
             " from = document.querySelector('[data-dbg-search=\"open\"]').getBoundingClientRect();"
             " return [Math.round(at.left - from.left), Math.round(at.top - from.bottom)]; })()")
    under = lambda at: abs(at[0]) <= 1 and at[1] == 6
    home = bar.ev(place)
    assert under(home), home
    assert bar.ev("(() => { const title = document.getElementById('dbg-search').firstElementChild;"
                  " return [title.tagName, title.textContent, getComputedStyle(title).cursor]; })()") == ["H4", "Autosearch", "grab"]
    bar.ev("(() => { const title = document.getElementById('dbg-search').firstElementChild, at = title.getBoundingClientRect();"
           " const x = at.left + 40, y = at.top + 8, said = (kind, to, px, py) => to.dispatchEvent(new PointerEvent(kind,"
           " {bubbles: true, cancelable: true, pointerId: 7, button: 0, clientX: px, clientY: py}));"
           " said('pointerdown', title, x, y); said('pointermove', window, x + 60, y + 40); said('pointermove', window, x + 120, y + 80);"
           " said('pointerup', window, x + 120, y + 80); return true; })()")
    moved = bar.ev(place)
    assert abs(moved[0] - home[0] - 120) <= 1 and abs(moved[1] - home[1] - 80) <= 1, (home, moved)
    bar.ev("drawDebugBar(), true")
    assert bar.ev(place) == moved and bar.ev("Object.keys(debugWindowPlace)") == ["dbg-search"]
    bar.ev("document.getElementById('dbg-search').firstElementChild.dispatchEvent(new MouseEvent('dblclick', {bubbles: true})), true")
    assert under(bar.ev(place)) and bar.ev("Object.keys(debugWindowPlace)") == []

    # A search: ten turns on, on a quarter of the cores. Every copy is asked
    # to forget the page's breakpoints and stand on the turn the search
    # begins on, and a copy of the game itself is not asked to `remember`.
    pick("turns", 10)
    pick("cores", 0.25)
    # A quarter of the cores, rounded as the page rounds it (a half goes up).
    asked = max(1, int(cores * 0.25 + 0.5))
    bar.forget()
    bar.click('#dbg-search [data-dbg-search="go"]')
    bar.until(over, "the search ends", seconds=60)
    found = bar.ev("({state: debugSearch.state, best: debugSearch.report.best.plan, value: debugSearch.report.best.verdict.want[0],"
                   " alone: debugSearch.report.baseline.plan, was: debugSearch.report.baseline.verdict.want[0],"
                   " last: debugSearch.report.best.last, futures: debugSearch.futures, cores: debugSearch.cores})")
    assert found["state"] == "done", bar.ev("debugSearch.note")
    assert (found["best"], found["value"], found["last"]) == ([[start, "stock"]], score + 20, start + 10), found
    assert (found["alone"], found["was"], found["cores"]) == ([[start, "jack"]], score + 10, asked), found
    # The page's own game was asked nothing, and stands where it stood.
    assert bar.sent() == [] and bar.ev("[state.turn, engine.turn, engine.score]") == [start, start, score], bar.sent()
    # The copies were made once and let go when it ended.
    assert bar.ev(copies) == [asked, 0], bar.ev(copies)
    first = bar.ev(f"{forked}.filter(call => call.path === '/debug?fork=1').slice(0, 3).map(call => [call.body.op, call.body.turn ?? null])")
    assert first == [["breakpoints", None], ["objective", None], ["jump", start]], first
    # Each copy reads the four lists the panel was set to, and the climb is
    # the engine's: begun with them, asked for each line, told what each
    # came to, and let go when the search ended.
    assert bar.ev(f"{forked}.find(call => call.body.op === 'objective').body.objective") == {
        "need": [{"seat": 0, "key": "won", "test": "==", "value": 1, "when": "end"}],
        "avoid": [{"seat": 0, "key": "lost", "test": "==", "value": 1, "when": "always"}],
        "want": [{"seat": 0, "key": "score", "weight": 1}], "dont_want": []}
    climbed = bar.ev("engine.sent.filter(call => call.path === '/autosearch').map(call => call.body.op)")
    assert climbed[0] == "begin" and climbed[-1] == "end" and climbed.count("took") == found["futures"], climbed
    begun = bar.ev("engine.sent.find(call => call.path === '/autosearch').body")
    assert (begun["from"], begun["end"], begun["seated"], begun["forks"]) == (start, start + 10, "jack", asked), begun
    assert bar.ev(f"{forked}.every(call => call.body.lean === true)")
    assert bar.ev(f"{forked}.some(call => call.body.op === 'remember')") is False
    plays = bar.ev(f"{forked}.filter(call => call.body.op === 'play').map(call => [call.body.turns, call.body.strategy])")
    assert plays and all(1 <= turns <= 5 and agent in ("jack", "stock") for turns, agent in plays), plays
    # Each line is walked back to with the Civ AI that holds its turn named.
    walks = bar.ev(f"{forked}.filter(call => call.body.op === 'jump' && call.body.strategy).map(call => call.body.turn)")
    assert walks and all(start <= turn < start + 10 for turn in walks), walks
    assert found["futures"] > 2, "only the Civ AIs alone were played: the climb around the best of them never began"

    # What the panel says of it: the line, what it comes to beside the Civ AI
    # seated, the two of them over the turns, and each Civ AI alone.
    said = bar.ev("document.getElementById('dbg-search-best').textContent")
    gain = bar.ev(f"debugSearchPercent(100 * 10 / Math.max(1, {score + 10}))")
    assert said == (f"Best found: it meets 0 of 1 Need, breaks no Must Avoid; SCORE {score + 20} on turn {start + 10}, "
                    f"{gain} on what is wanted against Jack alone (SCORE {score + 10})."), said
    # Nobody wins in ten turns: no line meets the Need, which says so.
    assert bar.ev("document.getElementById('dbg-search-front').textContent").startswith("No line of "), "the Need not met is not said"
    assert bar.ev("document.getElementById('dbg-search-status').textContent").startswith("Done in "), "the search does not say it is done"
    assert bar.ev("[...document.querySelectorAll('#dbg-search-plan span')].map(held => held.textContent)") == [f"{start} Stock"]
    drawn = bar.ev("(() => { const chart = document.querySelector('#dbg-search-chart svg');"
                   " return [chart.querySelectorAll('path').length, chart.querySelectorAll('circle').length,"
                   " [...chart.querySelectorAll('text.dbg-value')].map(held => held.textContent),"
                   " [...document.querySelectorAll('#dbg-search-key span')].map(held => held.textContent)]; })()")
    assert drawn == [2, 2, [str(score + 10), str(score + 20)], ["Best line", "Jack alone"]], drawn
    table = bar.ev("[...document.querySelectorAll('#dbg-search-table span')].map(held => held.textContent)")
    assert table == ["Stock", str(score + 20), "Jack", str(score + 10)], table
    assert bar.ev("document.getElementById('dbg-search-count').textContent") == ""
    # The turn under the pointer is read off the chart, and under the keys.
    bar.ev("(() => { const chart = document.querySelector('#dbg-search-chart svg'); chart.focus();"
           " chart.dispatchEvent(new KeyboardEvent('keydown', {key: 'ArrowLeft', bubbles: true})); return true; })()")
    tip = bar.ev("[...document.querySelectorAll('#dbg-search-chart .dbg-search-tip div')].map(held => held.textContent)")
    assert tip == [f"Turn {start + 9}", f"{score + 18}Best line", f"{score + 9}Jack alone"], tip

    # Played: this timeline walks back to the turn the search began on with
    # the line's Civ AI named, plays the line out, and says what the seat has
    # beside what the search read.
    bar.forget()
    bar.click('#dbg-search [data-dbg-search="play"][value="here"]')
    bar.until("debugSearch.playing === null && !!debugSearch.said", "the line is played", seconds=60)
    ops = [[call["op"], call.get("turn"), call.get("strategy")] for call in bar.sent()]
    assert ops == [["jump", start, "stock"]] + [["step", None, "stock"]] * 10, ops
    done = f"Played to turn {start + 10}: SCORE {score + 20}, as found"
    assert bar.ev("[debugSearch.said, document.getElementById('dbg-note').textContent, state.turn, engine.score,"
                  " document.getElementById('autoplaystrategy').value]") == [done, done, start + 10, score + 20, "stock"]
    # A line that does not come to what was found says so.
    bar.ev("debugSearch.report.best.verdict.want[0] += 1, true")
    bar.click('#dbg-search [data-dbg-search="play"][value="here"]')
    bar.until(f"debugSearch.playing === null && debugSearch.said !== {json.dumps(done)}", "the line is played again", seconds=60)
    assert bar.ev("debugSearch.said") == f"Played to turn {start + 10}: SCORE {score + 20}, not the {score + 21} found"
    # Played from a turn that is no longer the world that was searched, it is refused.
    bar.ev("debugSearch.print = -1, true")
    bar.click('#dbg-search [data-dbg-search="play"][value="here"]')
    bar.until("debugSearch.playing === null && debugSearch.said.includes('no longer the world')", "another world is not played on")

    # Stopped by hand: the same button, and the copies are let go at once.
    bar.ev("engine.sent.length = 0, forks.wait = 40, true")
    bar.click('#dbg-search [data-dbg-search="go"]')
    bar.until("debugSearch.state === 'running' && debugSearch.futures > 0", "a slowed search is on its way", seconds=60)
    assert bar.ev("[document.querySelector('#dbg-search [data-dbg-search=\"go\"]').textContent,"
                  " document.getElementById('dbg-search-run').hidden, document.querySelectorAll('#dbg-search-heads i').length,"
                  " document.getElementById('dbg-search-count').textContent !== '']") == ["⏹ Stop", False, asked, True]
    # The panel put away, the search goes on and the button counts it.
    bar.ev("document.getElementById('dbg-search').dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true})), true")
    assert bar.ev(f"[{shown}, debugSearch.live, document.querySelector('[data-dbg-search=\"open\"]').classList.contains('on')]") == [False, True, True]
    bar.click('[data-dbg-search="open"]')
    bar.click('#dbg-search [data-dbg-search="go"]')
    assert bar.ev("[debugSearch.live, debugSearch.state]") == [False, "stopped"]
    bar.until(f"JSON.stringify({copies}) === '[{asked},0]' && forks.held.length === 0", "a stopped search lets its copies go")
    assert bar.ev("document.getElementById('dbg-search-status').textContent").startswith("Stopped after ")
    bar.ev("forks.wait = 0, true")

    # An engine that cannot be copied has no cores to search with, and says so.
    bar.ev("forks.refuse = '/forks returned HTTP 404', true")
    bar.click('#dbg-search [data-dbg-search="go"]')
    bar.until("debugSearch.state === 'failed'", "an engine with no forks is not searched")
    refusal = bar.ev("document.getElementById('dbg-search-status').textContent")
    assert "this build could not make them (/forks returned HTTP 404)" in refusal, refusal
    assert bar.ev("document.getElementById('dbg-search-found').hidden") is True
    bar.ev("forks.refuse = null, true")

    # Two tracks: the search begins from the half that is picked, on copies
    # of that half, which are asked to keep their agents from there on.
    turn = bar.ev("state.turn")
    reported = (f"debugSplit && ['a', 'b'].every(letter => debugSplit.panes[letter].turn === {turn} && debugSplit.panes[letter].numbers"
                " && debugSplit.panes[letter].marks) && !!debugSplitDeck() && debugSplitDeck().idle")
    bar.branch()
    bar.until(reported, "both halves say where they stand")
    half = "debugSplit.panes.{}.frame.contentWindow.eval({})".format
    sent = lambda letter: bar.ev(half(letter, json.dumps(
        "engine.sent.filter(call => call.path === '/debug' && call.body.op !== 'status').map(call => [call.body.op, call.body.strategy ?? call.body.from ?? null])")))
    # Each half is seated by the host: A with jack, to set beside the line.
    bar.ev("debugSplitSeat('a', 'jack'), debugSplitSeat('b', 'jack'), true")
    bar.until(half("a", "'engine.seat'") + " === 'jack' && " + half("b", "'engine.seat'") + " === 'jack' && debugSplitDeck().idle", "both halves seat jack")
    assert bar.ev(shown), "the panel is the box's own, and stays up when the timeline is branched"
    assert bar.ev("[document.getElementById('dbg-search-half').hidden, [...document.getElementById('dbg-search-half').options].map(held => held.textContent)]") == \
        [False, [f"A · turn {turn} · Jack", f"B · turn {turn} · Jack"]]
    pick("half", "b")
    base = bar.ev(half("b", "'engine.score'"))
    for letter in ("a", "b"):
        bar.ev(half(letter, "'engine.sent.length = 0, true'"))
    bar.ev("engine.sent.length = 0, true")
    bar.click('#dbg-search [data-dbg-search="go"]')
    bar.until(over, "the search from B ends", seconds=60)
    assert bar.ev("[debugSearch.state, debugSearch.letter, debugSearch.pane === debugSplit.panes.b.id, debugSearch.report.best.plan,"
                  " debugSearch.report.best.verdict.want[0]]") == \
        ["done", "b", True, [[turn, "stock"]], base + 20], bar.ev("debugSearch.note")
    assert bar.ev(f"{forked}.every(call => call.path.endsWith('&pane=' + debugSplit.panes.b.id))")
    assert bar.ev(f"{forked}.filter(call => call.body.op === 'remember').length") == asked
    # Neither half was asked anything while it was searched.
    assert sent("a") == [] and sent("b") == [], (sent("a"), sent("b"))
    words = bar.ev("[...document.querySelectorAll('#dbg-search-play button')].map(held => [held.textContent, held.disabled])")
    assert words == [["▶ Play it in A", False], ["▶ Play it in B", False]], words
    # Played in the other half: A is made a copy of B, walks back to the turn
    # and plays the line, each Civ AI of it said to the host as it sits down.
    # B, locked to it, plays the same turns under its own.
    bar.click('#dbg-search [data-dbg-search="play"][value="a"]')
    bar.until("debugSearch.playing === null && !!debugSearch.said", "the line is played in A", seconds=60)
    assert bar.ev("debugSearch.said") == f"Played to turn {turn + 10}: SCORE {base + 20}, as found"
    played_a = sent("a")
    assert played_a[:2] == [["copy", bar.ev("debugSplit.panes.b.id")], ["jump", "stock"]], played_a
    assert played_a[2:] == [["step", "stock"]] * 10, played_a
    bar.until(f"debugSplit.panes.a.turn === {turn + 10} && debugSplit.panes.b.turn === {turn + 10} && debugSplitDeck().idle", "B plays the same turns")
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy]") == ["stock", "jack"]
    assert [bar.ev(half(letter, "'engine.score'")) for letter in ("a", "b")] == [base + 20, base + 10]
    assert all(op == ["step", "jack"] for op in sent("b") if op[0] == "step") and len([op for op in sent("b") if op[0] == "step"]) == 10, sent("b")

    # Unlocked, each half has a panel of its own, and Autosearch on it is the
    # host's, begun from that half.
    bar.ev("debugSearchShow(false), true")
    bar.click("[data-dbg-lock]")
    for letter in ("b", "a"):
        bar.until(half(letter, "'debugBar && !debugBar.hidden && getComputedStyle(debugBar).display !== \"none\"'"), f"{letter.upper()}'s own panel is up")
        bar.ev(half(letter, "'document.querySelector(\"[data-dbg-search=open]\").click(), true'"))
        bar.until(f"{shown} && debugSearchSet.half === '{letter}'", f"Autosearch on {letter.upper()}'s panel opens the host's, from {letter.upper()}")
        assert bar.ev(half(letter, "'document.getElementById(\"dbg-search\").hidden'")) is True
        bar.ev("debugSearchShow(false), true")
    # A branch closed since the search: its line is another world's to play.
    bar.ev("debugSearchShow(true), true")
    bar.click('[data-dbg-split="drop-b"]')
    bar.until("!debugSplit", "B is dropped")
    assert bar.ev("[...document.querySelectorAll('#dbg-search-play button')].map(held => [held.value, held.disabled])") == [["here", True]]
    assert "search again" in bar.ev("document.getElementById('dbg-search-play').textContent")
    # A new game lets go of the search and of what it found.
    bar.ev("engine.seed += 1, render(engineState()), true")
    assert bar.ev("[debugSearch, document.getElementById('dbg-search-found').hidden]") == [None, True]
    bar.ev("debugSearchShow(false), true")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_doctor_strange(bar: Bar, address: str) -> None:
    """Doctor Strange's CIVVIS: the same split, shown as two worlds under the tree of their timeline.

    Operator, 2026-11-23: "it will have a scrolling guitar hero setup. where
    the timeline we are on and following is center screen. forward in time is
    up. forward will branch out more in the tree. bottom left quad and bottom
    right quad, roughly are our world to compare. 2 simulatoins. this is
    limited at 2 for this demo."

    Operator, 2026-11-24: "branches should be labeled with their series of
    player strategies. like MoonshotMaven-150-JackOfAllTrades-160-MoonshotMaven",
    "when we branch, we should select the player strategy and make the
    branch. in one step", "highlight a box in the lower part of the screen
    with the color of the branch. each box should have an exit to close the
    box. the boxes should somewhat automatically resize and the divide",
    "Play button should say Play Time and alternate to Pause Time", "The
    cursor over a world on the bottom should highlight that world's line. we
    should also not spew random lines on the timeline. Move turn count up to
    the center line".
    """
    turns = lambda letters, turn: " && ".join(f"debugSplit.panes.{letter}.turn === {turn}" for letter in letters)
    both = lambda turn: turns("ab", turn)
    ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"
    bar.dev.call("Page.navigate", {"url": f"{address}?mode=strange"})
    bar.until(f"{ready} && location.search === '?mode=strange' && state === null", "the page, asked for as Doctor Strange's CIVVIS")
    assert bar.ev("[STRANGE, STRANGE_PANE, document.documentElement.classList.contains('strange')]") == [True, False, True]
    # The toy keeps each turn's score, as the engine keeps each turn's world.
    # The first part of this is the page with no future played ahead, so
    # that what is played is what is asked for; the futures come after.
    bar.ev("engine.scores = {1: 0}, strange.futures = false, render(engineState()), true")
    bar.settled()
    bar.until(f"debugSplit && debugSplit.shown && {both(1)} && strangeIdle() && !debugSplit.locked", "its two worlds are there")
    # The page opens on the best player strategy and the one to beat it with
    # (operator 2026-11-24: "our top strategy on the right and the challenger
    # on the left"): the picker rates Jack 1211 and Stock 1000, so B, on the
    # right, is Jack, and A, on the left and followed, is Stock. The worlds
    # are locked by the page, and the split's own lock is left open.
    assert bar.ev("[debugSplit.panes.a.strategy, debugSplit.panes.b.strategy, strange.locked, debugSplit.locked, strange.follow]") == \
        ["stock", "jack", True, False, "a"]
    bar.until("debugSplit.panes.a.frame.contentWindow.eval('engine.seat') === 'stock'", "A is seated with the challenger")
    # The two worlds are a box each under the line of the open turn, with
    # the trunk's column between them: the line is the divide between the
    # tree and the boxes, and each box hangs from it under its name plate.
    # The sky they stand on is the whole window, the bar is unseen and the
    # strip between the halves is gone. Each half knows what it is a half
    # of, which is what puts its panels away, its own bar among them, and
    # its world minimap (not drawn either) and map controls (operator
    # 2026-11-26: "lets drop the minimap and controls thing from the maps
    # doctor stranges civ 6"). What
    # the page costs is in the top right corner, at the head of a column that
    # runs down the right of the upper half: the black swan, the predicted
    # paths and the defeat tracker under it (operator 2026-11-26: "cpu and
    # memory can go at the top. then: "Predicted Paths""). The worlds'
    # numbers go across the top between the title and that column ("from
    # the right side of the upper left box to the left side of the
    # resources/predicted paths box"), and the tree is drawn under them
    # ("lets truncate the timelines map below this").
    layout = bar.ev("""(() => {
      const box = letter => debugSplit.panes[letter].frame.getBoundingClientRect(), a = box("a"), b = box("b");
      const sky = document.getElementById("strange-sky").getBoundingClientRect(), line = strangeGeometry().cy;
      const card = name => document.querySelector(`#strange-hud .${name}`).getBoundingClientRect();
      const meter = card("strange-meter"), versus = card("strange-versus"), title = card("strange-title"), column = card("strange-right");
      const order = [...document.querySelector("#strange-hud .strange-right").children].map(held => held.className.split(" ")[0]);
      return {
        a: [a.left === 0, Math.round(a.top) === line + 34, a.right < innerWidth / 2, Math.round(a.bottom) === innerHeight],
        b: [Math.round(b.right) === innerWidth, Math.round(b.top) === line + 34, b.left > innerWidth / 2, Math.round(b.bottom) === innerHeight],
        alike: Math.round(a.width) === Math.round(b.width) && Math.round(a.height) === Math.round(b.height),
        roomy: a.height > innerHeight * .3 && a.width > innerWidth * .4 && line >= strange.floor,
        sky: [sky.left, sky.top, sky.width === innerWidth, sky.height === innerHeight],
        bar: getComputedStyle(debugBar).visibility,
        strip: getComputedStyle(debugSplit.host.querySelector(".dbg-divider")).display,
        meter: [meter.top < 40, meter.right > innerWidth - 40],
        versus: [versus.top < 40, versus.left > title.right, versus.right < column.left, strange.treeTop >= versus.bottom],
        column: [column.top < 40, column.right > innerWidth - 40, column.bottom <= line, meter.top === column.top,
          order.join(" ") === "strange-meter strange-swans strange-paths strange-defeat"],
        halves: ["a", "b"].map(letter => debugSplit.panes[letter].frame.contentWindow.eval(
          "[STRANGE, STRANGE_PANE, document.documentElement.classList.contains('strange-pane'), getComputedStyle(debugBar).display," +
          " ...['.minimap-frame', '#map-controls-dock'].map(s => getComputedStyle(document.querySelector(s)).display)," +
          " !String(drawMini).includes('function drawMini')]")),
      };
    })()""")
    assert layout == {"a": [True] * 4, "b": [True] * 4, "alike": True, "roomy": True, "sky": [0, 0, True, True],
                      "bar": "hidden", "strip": "none", "meter": [True, True], "versus": [True] * 4, "column": [True] * 5,
                      "halves": [[False, True, True, "none", "none", "none", True]] * 2}, layout
    # The highway is painted: the open turn is the line across the window,
    # which is where the followed world's turn is put.
    bar.until("strange.canvas.width === Math.round(innerWidth * Math.min(2, devicePixelRatio || 1)) && strange.cam === 1",
              "the highway is painted")
    geometry = bar.ev("(() => { const g = strangeGeometry(); return [strangeY(g, strange.cam, strange.cam) === g.cy,"
                      " strangeY(g, 11, 1) < strangeY(g, 2, 1), strangeY(g, 2, 1) < g.cy,"
                      " Math.round(strangeTurnAt(g, strangeY(g, 37, 12), 12)), Math.round(strangeTurnAt(g, strangeY(g, 3, 12), 12)),"
                      " strangeSpread(g, 20) > strangeSpread(g, g.cy)]; })()")
    # Forward in time is up, a turn is the turn its height stands for, and the tree is wider ahead.
    assert geometry == [True, True, True, 37, 3, True], geometry
    words = "[...document.querySelectorAll('#strange-hud [data-strange]')].map(held => [held.dataset.strange, held.textContent.trim(), !!held.disabled])"
    said = {name: [text, off] for name, text, off in bar.ev(words) if name != "numbers"}
    # Branch is a picker: the open turn and each player strategy the followed
    # world does not seat. Before it, the base timeline: every open world, by
    # its letter and its name. Each box has its exit.
    assert said == {"play": ["▶ Play Time", False], "back10": ["−10", True], "back1": ["−1", True], "on1": ["+1", False],
                    "on10": ["+10", False], "base": ["A · StockB · Jack", False],
                    "branch": ["⑂ Branch: 1-(player strategy)1-Jack", False], "search": ["Autosearch", False],
                    "fork": ["⏮ To the fork", True], "lock": ["🔒 Locked", False], "futures": ["🔮 Futures: off", False],
                    "swan": ["🦢 Generate Black Swan", False], "swan-kind": ["AnyPandemicInstant nuclear war", False],
                    "omen": ["", False], "all": ["All 14,000,605", True],
                    "say": ["One world so far. Play, and each player strategy takes it its own way", False],
                    "follow-a": ["Following", True], "follow-b": ["Follow", False],
                    "close-a": ["✕", False], "close-b": ["✕", False],
                    "turn": ["1", False], "limit": ["", False], "era": ["Ancient era", False]}, said
    # The base timeline is picked before Branch (operator 2026-11-25: "the
    # option to branch in the simulator should be preceded by a selection of
    # the base timeline"): its picker stands right over Branch's, on the
    # followed world, in that world's colour, and says what it is.
    base = ("(() => { const base = document.querySelector('#strange-hud [data-strange=\"base\"]'),"
            " branch = document.querySelector('#strange-hud [data-strange=\"branch\"]');"
            " const [over, under] = [base.getBoundingClientRect(), branch.getBoundingClientRect()];"
            " return [base.value, [...base.options].map(option => option.textContent), base.style.getPropertyValue('--strange-base'),"
            " getComputedStyle(base).borderLeftColor, base.closest('label').textContent.startsWith('Base timeline'),"
            " !!(base.compareDocumentPosition(branch) & Node.DOCUMENT_POSITION_FOLLOWING), over.bottom <= under.top, under.top - over.bottom < 12,"
            " Math.round(over.left) >= Math.round(under.left), Math.round(over.right) === Math.round(under.right)]; })()")
    assert bar.ev(base) == ["a", ["A · Stock", "B · Jack"], "#ff8f6b", "rgb(255, 143, 107)", True, True, True, True, True, True], bar.ev(base)
    # A branch is named by who plays it, and its box is in its colour. The
    # turn is written on the line of the open turn, not at the foot of the window.
    names = "strange.order.map(letter => document.querySelector(`#strange-hud [data-strange-name=\"${letter}\"]`).textContent)"
    assert bar.ev(names) == ["Stock", "Jack"]
    inks = ("strange.order.map(letter => [strange.tint[letter], getComputedStyle(debugSplit.panes[letter].frame).borderLeftColor,"
            " getComputedStyle(document.querySelector(`#strange-hud [data-strange-world=\"${letter}\"]`)).borderTopColor])")
    assert bar.ev(inks) == [["#ff8f6b", "rgb(255, 143, 107)", "rgb(255, 143, 107)"], ["#6fd3e0", "rgb(111, 211, 224)", "rgb(111, 211, 224)"]]
    assert bar.ev("(() => { const chip = document.querySelector('#strange-hud .strange-turn'), turn = chip.getBoundingClientRect(), g = strangeGeometry();"
                  " return [Math.round(turn.bottom) === g.cy, turn.right < g.cx, turn.right > g.cx - 260, chip.textContent]; })()") == \
        [True, True, True, "TURN1Ancient era"]
    # One world so far, and one line on the tree. A turn played by two player strategies forks it there.
    assert bar.ev("[strangeTree().splits === Infinity, strange.order.join(''), strange.follow]") == [True, "ab", "a"]
    bar.click('#strange-hud [data-strange="on1"]')
    bar.until(f"{both(2)} && strangeIdle() && strangeTree().splits === 1", "the two worlds part")
    assert bar.ev("[document.querySelector('#strange-hud [data-strange=\"say\"]').textContent,"
                  " document.querySelector('#strange-hud [data-strange=\"fork\"]').textContent, strangeTree().worlds.b.parent]") == \
        ["A and B parted on turn 1", "⏮ To the fork · 1", "a"]
    # The band sets each yield a turn over what the world has made so far
    # (operator 2026-11-23: "lets track cumulative prod, science, culture,
    # gold, faith next to the current turn yields"), a row a world across the
    # top (2026-11-26), the totals in whole numbers, the greater of each in gold.
    bar.until("debugSplit.panes.a.numbers.production_total !== debugSplit.panes.b.numbers.production_total",
              "the two worlds' totals part with them")
    scores = bar.ev("[debugSplit.panes.a.numbers.score, debugSplit.panes.b.numbers.score]")
    band = bar.ev("""(() => { const card = document.querySelector('#strange-hud [data-strange="numbers"]');
      const heads = [...card.querySelectorAll('em .strange-long')].map(held => held.textContent);
      const rows = {}; let world = null;
      for (const held of card.children) {
        if (held.classList.contains('strange-letter')) { world = held.textContent; rows[world] = []; continue; }
        if (held.tagName !== 'B' || !world) continue;
        const small = held.querySelector('small');
        rows[world].push([held.firstChild ? held.firstChild.textContent : '', small ? small.textContent : null,
          held.className, small ? small.className : null]);
      }
      return {heads, rows}; })()""")
    assert band["heads"] == ["Score", "Cities", "Population", "Production", "Science", "Culture", "Gold", "Faith",
                             "Military"], band
    assert list(band["rows"]) == ["A", "B"], band
    col = band["heads"].index
    row_a, row_b = band["rows"]["A"], band["rows"]["B"]
    low, high = sorted(scores)
    lead = ["", "lead"] if scores[1] > scores[0] else ["lead", ""]
    total = lambda value: f"{round(value):,}"
    assert row_a[col("Production")] == ["1", total(scores[0] * 1250), "", lead[0]], band
    assert row_b[col("Production")] == ["1", total(scores[1] * 1250), "", lead[1]], band
    assert [row_a[col("Gold")][:2], row_b[col("Gold")][:2]] == [["1", total(scores[0] * 4 - 0.4)], ["1", total(scores[1] * 4 - 0.4)]], band
    assert [row_a[col("Faith")][:2], row_b[col("Faith")][:2]] == [["0", "0"], ["0", "0"]], band
    assert [row_a[col("Score")][:2], row_b[col("Score")][:2]] == [[str(scores[0]), None], [str(scores[1]), None]] and low < high, band
    # The pointer over a world's box, or its name plate, picks out its line.
    over = "[strange.over, ...['a', 'b'].map(letter => debugSplit.panes[letter].frame.classList.contains('strange-over'))]"
    bar.ev("debugSplit.panes.b.frame.dispatchEvent(new MouseEvent('mouseenter')), true")
    assert bar.ev(over) == ["b", False, True]
    bar.ev("debugSplit.panes.b.frame.dispatchEvent(new MouseEvent('mouseleave')), true")
    bar.ev("document.querySelector('#strange-hud [data-strange-world=\"a\"]').dispatchEvent(new MouseEvent('mouseenter')), true")
    assert bar.ev(over) == ["a", True, False]
    bar.ev("document.querySelector('#strange-hud [data-strange-world=\"a\"]').dispatchEvent(new MouseEvent('mouseleave')), true")
    assert bar.ev(over) == [None, False, False]
    # A branch is made in one step, by picking who plays it, and is a world
    # of its own (operator 2026-11-24: "simulating along 3 paths. 2 different
    # player-strategies at the start of the game and 1 branched during the
    # game"): a third page, C, beside the world it was made from, which is A
    # as A stands on the open turn with the picked strategy in the seat, a
    # colour of its own and a name that says who played it and from when.
    pick = ("(name => {{ const picker = document.querySelector('#strange-hud [data-strange=\"branch\"]'); picker.value = name;"
            " picker.dispatchEvent(new Event('change', {{bubbles: true}})); return picker.value; }})('{0}')").format
    assert bar.ev(pick("jack")) == ""
    bar.until(f"strange.order.join('') === 'acb' && debugSplit.panes.c && {turns('abc', 2)} && strangeIdle()"
              " && debugSplit.panes.c.marks && debugSplit.panes.c.marks.by.length === 1"
              " && debugSplit.panes.c.frame.contentWindow.eval('engine.seat') === 'jack'", "a branch is a third world, beside A")
    assert bar.ev(names) == ["Stock", "Stock-2-Jack", "Jack"]
    assert bar.ev(inks)[1] == ["#b79cff", "rgb(183, 156, 255)", "rgb(183, 156, 255)"]
    assert bar.ev("[document.querySelector('#strange-hud [data-strange=\"say\"]').textContent, debugSplit.panes.c.id.slice(2) === debugSplit.panes.a.id.slice(2),"
                  " strangeTree().worlds.c.parent, strangeTree().worlds.c.fork, strangeTree().worlds.c.parted]") == \
        ["Branch 2-Jack is world C, from A", True, "a", 2, False]
    assert bar.ev("debugSplit.panes.c.frame.contentWindow.eval('engine.sent.filter(call => call.path === \"/debug\")"
                  ".map(call => call.body.op)')").count("copy") == 1
    # Three boxes share the width, under a strip left clear below the line
    # where the worlds stand on it; the numbers have a pair of columns each.
    boxes = ("(() => { const g = strangeGeometry(); return strange.order.map(letter => {"
             " const box = debugSplit.panes[letter].frame.getBoundingClientRect();"
             " return [Math.round(box.left), Math.round(box.width), Math.round(box.top) - g.cy, Math.round(box.bottom) === innerHeight]; }); })()")
    wide = bar.ev("innerWidth")
    three = bar.ev(boxes)
    assert [box[2:] for box in three] == [[30 + 34, True]] * 3, three
    assert all(abs(box[1] - wide / 3) <= 1 and abs(box[0] - at * wide / 3) <= 1 for at, box in enumerate(three)), three
    assert bar.ev("[...document.querySelectorAll('#strange-hud [data-strange=\"numbers\"] .strange-letter')].map(cell => cell.textContent)") == ["A", "C", "B"]
    # A turn back is every world, as they stood.
    bar.click('#strange-hud [data-strange="back1"]')
    bar.until(f"{turns('abc', 1)} && strangeIdle()", "the worlds go back a turn")
    # Following C puts its timeline in the middle: a branch is then made from
    # C, which the base timeline's picker says, in C's colour.
    bar.click('#strange-hud [data-strange="follow-c"]')
    assert bar.ev("[strange.follow, document.querySelector('#strange-hud [data-strange=\"branch\"]').textContent]") == \
        ["c", "⑂ Branch: 1-(player strategy)1-Stock"]
    held = bar.ev(base)
    assert held[0] == "c" and held[1] == [f"{letter} · {name}" for letter, name in zip("ACB", bar.ev(names))] and held[2] == "#b79cff", held
    # Picking the base timeline there is the same: B is followed, and Branch
    # offers what B does not seat. C again, for what follows.
    rebase = ("(letter => { const base = document.querySelector('#strange-hud [data-strange=\"base\"]'); base.value = letter;"
              " base.dispatchEvent(new Event('change', {bubbles: true})); return [strange.follow, base.value,"
              " document.querySelector(`#strange-hud [data-strange=\"follow-${letter}\"]`).textContent,"
              " document.querySelector('#strange-hud [data-strange=\"branch\"]').textContent]; })('%s')")
    assert bar.ev(rebase % "b") == ["b", "b", "Following", "⑂ Branch: 1-(player strategy)1-Stock"], bar.ev(rebase % "b")
    assert bar.ev("debugSplit.panes.b.strategy") == "jack"
    assert bar.ev(rebase % "c") == ["c", "c", "Following", "⑂ Branch: 1-(player strategy)1-Stock"]
    # A press on the highway goes to the turn that height stands for: one
    # ahead of the last played is played to.
    bar.ev("(() => { const g = strangeGeometry(); strange.canvas.dispatchEvent(new MouseEvent('click',"
           " {clientX: g.cx, clientY: strangeY(g, 3, strange.cam), bubbles: true})); return true; })()")
    bar.until(f"{turns('abc', 3)} && strangeIdle()", "a press on the highway plays to its turn")
    # Locked, which is how the worlds begin, they go through the turns
    # together (operator 2026-11-23: "the doctor strange tool should step
    # through turns in sync when locked (which is the default). always on the
    # same turn"). Each is asked for the same next turn at once and holds it
    # unshown until the rest have played their own, so none is ever seen a
    # turn ahead, and each plays every turn with its own player strategy.
    steps = "debugSplit.panes.{0}.frame.contentWindow.eval('engine.sent.filter(call => call.path === \"/debug\" && call.body.op === \"step\").length')".format
    stepped = bar.ev(f"[{steps('a')}, {steps('b')}, {steps('c')}]")
    bar.ev("window.strangeGaps = []; window.strangeWatch = setInterval(() => strangeGaps.push("
           "debugSplit.panes.a.turn - debugSplit.panes.b.turn, debugSplit.panes.a.turn - debugSplit.panes.c.turn), 4); true")
    bar.click('#strange-hud [data-strange="on10"]')
    bar.until(f"{turns('abc', 13)} && strangeIdle()", "ten turns on, the worlds together")
    gaps = bar.ev("clearInterval(strangeWatch), strangeGaps")
    assert gaps and max(map(abs, gaps)) <= 1, gaps
    assert bar.ev(f"[{steps('a')}, {steps('b')}, {steps('c')}]") == [count + 10 for count in stepped]
    # Paused, the worlds stop together, on the turn all were asked for.
    bar.click('#strange-hud [data-strange="play"]')
    assert bar.ev("document.querySelector('#strange-hud [data-strange=\"play\"]').textContent") == "⏸ Pause Time"
    bar.until("debugSplit.panes.a.turn >= 16", "Play Time plays on")
    bar.click('#strange-hud [data-strange="play"]')
    bar.until("strange.run === null && strangeIdle()", "Pause stops the worlds")
    paused = bar.ev("[debugSplit.panes.a.turn, debugSplit.panes.b.turn, debugSplit.panes.c.turn, document.querySelector('#strange-hud [data-strange=\"say\"]').textContent]")
    assert paused[0] == paused[1] == paused[2] and paused[3] == f"Paused on turn {paused[0]}", paused
    # Unlocked, each world goes by itself: a turn back is the followed one's
    # (C's, here) alone, and each says its own turn.
    bar.click('#strange-hud [data-strange="lock"]')
    assert bar.ev("[strange.locked, document.querySelector('#strange-hud [data-strange=\"lock\"]').textContent]") == [False, "🔓 Unlocked"]
    turn = paused[0]
    bar.click('#strange-hud [data-strange="back1"]')
    bar.until(f"debugSplit.panes.c.turn === {turn - 1} && {both(turn)} && strangeIdle()", "C alone goes back a turn")
    assert bar.ev("document.querySelector('#strange-hud [data-strange-line=\"a\"]').textContent").startswith(f"Turn {turn}")
    # Locking again brings the other worlds to the followed one's turn first.
    bar.click('#strange-hud [data-strange="lock"]')
    bar.until(f"strange.locked && {turns('abc', turn - 1)} && strangeIdle()", "locked again, on C's turn")
    assert bar.ev("document.querySelector('#strange-hud [data-strange=\"play\"]').textContent") == "▶ Play Time"
    # Only what was played is a line: nothing on the tree is made up. (The
    # fan of ways a game might go was drawn from a dice the page no longer has.)
    assert bar.ev("typeof strangeDice") == "undefined"
    # The divide is dragged by the line of the open turn, and a double-click
    # gives it back to the page.
    placed = bar.ev("strangeGeometry().cy")
    bar.ev("(() => { const hold = document.querySelector('#strange-hud .strange-divide');"
           " for (const [kind, y] of [['pointerdown', %d], ['pointermove', %d], ['pointerup', %d]])"
           " hold.dispatchEvent(new PointerEvent(kind, {clientX: 300, clientY: y, pointerId: 1, bubbles: true})); return true; })()"
           % (placed, placed + 60, placed + 60))
    assert bar.ev("[strangeGeometry().cy, Math.round(debugSplit.panes.a.frame.getBoundingClientRect().top),"
                  " debugSplit.host.classList.contains('strange-dragging')]") == [placed + 60, placed + 60 + 30 + 34, False]
    bar.ev("document.querySelector('#strange-hud .strange-divide').dispatchEvent(new MouseEvent('dblclick', {bubbles: true})), true")
    assert bar.ev("strangeGeometry().cy") == placed
    # A branch is deleted by its box's exit: its line goes from the tree and
    # its box from the window (operator 2026-11-24: "we should be able delete
    # branches and that removes the line and the window"). C is a page this
    # one made, and is let go of whole; the two that are left have the
    # trunk's column between them again.
    bar.click('#strange-hud [data-strange="close-c"]')
    bar.until("strange.order.join('') === 'ab' && strangeIdle()", "C is deleted")
    gone = bar.ev("[debugSplit.panes.c === undefined, document.querySelector('iframe[data-dbg-frame=\"c\"]'),"
                  " document.querySelector('#strange-hud [data-strange-world=\"c\"]'), strange.follow, Object.keys(strangeTree().worlds).join(''),"
                  " document.querySelector('#strange-hud [data-strange=\"say\"]').textContent]")
    assert gone == [True, None, None, "a", "ab", "Jack deleted"], gone
    two = bar.ev(boxes)
    assert two[0][0] == 0 and two[0][1] < wide / 2 and two[1][0] > wide / 2 and [box[2:] for box in two] == [[34, True]] * 2, two
    # A is one of the split's own two, so it is put out of sight and B has
    # the whole width, and more of the height: the line of the open turn
    # goes up. B is followed, the last box stays open, and time runs for B alone.
    paired = bar.ev("strangeGeometry().cy")
    bar.click('#strange-hud [data-strange="close-a"]')
    bar.until("strange.order.join('') === 'b' && strangeIdle()", "A is deleted")
    shut = bar.ev("""(() => {
      const box = debugSplit.panes.b.frame.getBoundingClientRect(), plate = document.querySelector('#strange-hud [data-strange-world="b"]');
      return {follow: strange.follow, wide: [Math.round(box.left), Math.round(box.width) === innerWidth, Math.round(plate.getBoundingClientRect().width) === innerWidth],
        hangs: Math.round(box.top) === strangeGeometry().cy + 34, notch: plate.classList.contains("strange-notch"), line: strangeGeometry().cy,
        gone: [document.querySelector('#strange-hud [data-strange-world="a"]'), getComputedStyle(debugSplit.panes.a.frame).visibility],
        exit: document.querySelector('#strange-hud [data-strange="close-b"]').disabled,
        lock: document.querySelector('#strange-hud [data-strange="lock"]').disabled,
        say: document.querySelector('#strange-hud [data-strange="say"]').textContent};
    })()""")
    assert shut == {"follow": "b", "wide": [0, True, True], "hangs": True, "notch": True, "line": shut["line"],
                    "gone": [None, "hidden"], "exit": True, "lock": True, "say": "Stock deleted"}, shut
    # One box takes the line up, as far as the cards at the top let it. In
    # this window (757 high) the floor under the title card holds two boxes'
    # line as well, since the base timeline's picker made the card a row
    # taller: then the two stand on the same line.
    floor = bar.ev("Math.ceil(Math.min(strange.floor, innerHeight * .6))")
    assert shut["line"] < paired or shut["line"] == paired == floor, (shut["line"], paired, floor)
    stood = bar.ev("[debugSplit.panes.a.turn, debugSplit.panes.b.turn]")
    bar.click('#strange-hud [data-strange="on1"]')
    bar.until(f"debugSplit.panes.b.turn === {stood[1] + 1} && strangeIdle()", "time runs for the open world")
    assert bar.ev("debugSplit.panes.a.turn") == stood[0]
    # A branch opens a box again: A is B as it stands, played by the strategy
    # picked, in another colour, in a box beside B's.
    assert bar.ev(pick("stock")) == ""
    bar.until(f"strange.order.join('') === 'ba' && {both(stood[1] + 1)} && strangeIdle() && debugSplit.panes.a.strategy === 'stock'",
              "a branch opens A's box again")
    opened = bar.ev("[strange.tint.a, strangeGeometry().cy, debugSplit.panes.b.frame.getBoundingClientRect().left === 0,"
                    " debugSplit.panes.a.frame.getBoundingClientRect().left > innerWidth / 2, getComputedStyle(debugSplit.panes.a.frame).visibility]")
    assert opened == ["#8fe07a", paired, True, True, "visible"], opened
    bar.until(f"document.querySelector('#strange-hud [data-strange-name=\"a\"]').textContent.endsWith('-{stood[1] + 1}-Stock')",
              "the branch is named by who played it and who plays it from here")
    # What the page costs is read and shown: the toy's host says the
    # computer is at an eighth of its processors and two fifths of its memory.
    bar.until("document.querySelector('#strange-hud [data-strange-gauge=\"cpu\"]').textContent !== '–'", "what the page costs is read")
    assert bar.ev("[...document.querySelectorAll('#strange-hud [data-strange-gauge]')].map(held => held.textContent)"
                  ".concat(document.querySelector('#strange-hud [data-strange-worlds]').textContent)") == ["13%", "40%", "2 worlds open"]
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"

    # The futures (operator 2026-11-24: "we can compute (a degree in advance)
    # the futures of the paths we are on", "we should have for example
    # nuclear war visual forecasts out in the future (we will compute the
    # future of each path). these should alert on the top section out in the
    # distance"). Each world is played ahead of the open turn along its own
    # player strategy, a few turns at a request, and stays on the open turn:
    # the turns are kept ahead of it, which is the line the tree draws, and
    # what the engine saw coming is kept with the world for its line. In B's
    # world a device goes off five turns on.
    here = stood[1] + 1
    aheads = "debugSplit.panes.{0}.frame.contentWindow.eval('engine.sent.filter(call => call.path === \"/debug\" && call.body.op === \"ahead\").map(call => call.body.turns)')".format
    bar.ev(f"debugSplit.panes.b.frame.contentWindow.eval('engine.nukes = [{here + 5}]'), true")
    # Every world is asked at once, each for its own: none waits for another
    # to answer first.
    asked = bar.ev("(() => { document.querySelector('#strange-hud [data-strange=\"futures\"]').click();"
                   " return [strange.futures, Object.keys(strange.peeks).sort().join(''), Object.values(strange.peeks).map(peek => peek.turns)]; })()")
    assert asked == [True, "ab", [4, 4]], asked
    held = "['a', 'b'].every(letter => strangeKept(letter) - debugSplit.panes[letter].turn >= STRANGE_AHEAD)"
    quiet = "Object.keys(strange.peeks).length === 0"
    bar.until(f"strange.futures && {quiet} && {held}", "each world's future is played ahead of the open turn")
    future = bar.ev(f"[debugSplit.panes.a.turn, debugSplit.panes.b.turn, {aheads('a')}, {aheads('b')}, strangeTree().worlds.a.last, strangeTree().worlds.b.last,"
                    " debugSplit.panes.a.alerts, debugSplit.panes.b.alerts, debugSplit.panes.b.ahead.size,"
                    " document.querySelector('#strange-hud [data-strange=\"futures\"]').textContent]")
    assert future == [here, here, [4, 4, 4], [4, 4, 4], here + 12, here + 12, [],
                      [{"turn": here + 5, "kind": "nuclear", "text": "Greece detonates a nuclear device"}], 12,
                      "🔮 Futures: to the end of the game"], future

    # The whole of each world's future (operator 2026-11-24: "each branch in
    # doctor strange's civ 6 should simulate out to the end of the branch",
    # "The top of each branch should have either: no nuclear war in the next
    # N turns, no nuclear war, or nuclear war in N turns (Era). we should
    # note wars as well", "Can build out the most likely paths at all time").
    # Past the turns a world holds, its future is played to the end of its
    # game on a copy (the engine's `foresee`), asked for a stretch at a time
    # by the page itself and never of the world's own page. This engine has
    # none of its own for it, so each world plays its own, every world asked
    # at once, while the worlds stand still. In A's world a war is declared
    # thirty turns on.
    seen = "['a', 'b'].every(letter => debugSplit.panes[letter].sight && debugSplit.panes[letter].sight.done)"
    began = bar.ev(f"(() => {{ debugSplit.panes.a.frame.contentWindow.eval('engine.wars = [{here + 30}]');"
                   " const stood = [scouts.made, engine.sent.length]; strangeSightDrop('a'); strangeSightDrop('b'); strangeForesee();"
                   " return [...stood, strangeScouts(), Object.keys(strange.seeing).sort().join('')]; })()")
    assert began[2:] == [False, "ab"], began
    foreseen = f"engine.sent.slice({began[1]}).filter(call => /^\\/debug\\?pane=/.test(call.path) && call.body.op === 'foresee')"
    bar.until(seen, "each world's future is played to the end of its game")
    told = bar.ev(f"[{foreseen}.map(call => call.path.split('&scout=')[1]).filter(how => how === 'new').length, scouts.made - {began[0]},"
                  f" {foreseen}.every(call => call.body.lean === true && call.body.turns >= 1 && call.body.turns <= STRANGE_SEE_MOST),"
                  " ['a', 'b'].map(letter => debugSplit.panes[letter].frame.contentWindow.eval('engine.sent.filter(call => call.body && call.body.op === \"foresee\").length')),"
                  " ['a', 'b'].map(letter => [debugSplit.panes[letter].sight.from, debugSplit.panes[letter].sight.turn, debugSplit.panes[letter].sight.asked]),"
                  " ['a', 'b'].map(letter => strangeSightStale(letter))]")
    assert told == [2, 2, True, [0, 0], [[here + 12, 400, "stock"], [here + 12, 400, "jack"]], [False, False]], told
    # What each world's future comes to is listed on the right, under what
    # the page costs, as "Predicted Paths", best first (operator 2026-11-26:
    # "lets list the computed outcomes on the right side of the top scetion
    # of the screen, ranked from best to worst"), and no longer signed over
    # each branch. A's world has no nuclear war to the end of its game and a
    # war in thirty turns; B's has a nuclear war five turns on, so A is first.
    # Each line says it whole: the outcome on one line, what comes on the way
    # under it ("On current path," and the nuclear war, then the next war).
    assert bar.ev("[...document.querySelectorAll('#strange-hud .strange-fate')].every(sign => getComputedStyle(sign).display === 'none')")
    paths = ("[...document.querySelectorAll('#strange-hud [data-strange-paths] li')].map(row => [row.dataset.pathWorld, row.dataset.outcome,"
             " row.querySelector('b').textContent, row.querySelector('strong').textContent, row.querySelector('small').textContent])")
    assert bar.ev("document.querySelector('#strange-hud [data-strange-paths] h3').textContent") == "Predicted Paths"
    PATHS_WANT = [
        ["a", "clear", "1", "🏁 The game ends, turn 400", "☮ No nuclear war · ⚔ War in 30 turns: Rome on Greece"],
        ["b", "nuclear", "2", "🏁 The game ends, turn 400", "☢ Nuclear war in 5 turns (Atomic Era) · No wars ahead"]]
    assert bar.ev(paths) == PATHS_WANT, bar.ev(paths)
    whole = bar.ev("[...document.querySelectorAll('#strange-hud [data-strange-paths] li')].map(row => { const box = row.getBoundingClientRect();"
                   " return [...row.querySelectorAll('span, strong, small')].map(line => { const at = line.getBoundingClientRect();"
                   "  return at.height > 0 && at.top >= box.top && at.bottom <= box.bottom && line.scrollWidth <= line.clientWidth; }); })")
    assert whole == [[True, True, True], [True, True, True]], whole
    told = bar.ev("document.querySelector('#strange-hud [data-path-world=\"b\"]').title")
    assert told.startswith("World B: on current path, nuclear war in 5 turns (Atomic Era)\n"), told
    assert "Turn %d: Greece detonates a nuclear device" % (here + 5) in told, told
    # Until a future has been played to the end, its path says how far it
    # has been played with no nuclear war in it: the three things it says.
    said = bar.ev("(() => { const pane = debugSplit.panes.a, kept = pane.sight, turn = pane.turn;"
                  " const head = sight => { pane.sight = {...kept, ...sight}; return strangeFate('a').head; };"
                  " const heads = [head({done: false, turn: turn + 46, seen: []}), head({done: true, seen: []}),"
                  "  head({done: false, turn: turn + 90, seen: [{turn: turn + 83, kind: 'nuclear', text: 'x', era: 6, era_name: 'atomic'}]}),"
                  "  head({done: true, seen: [{turn: turn - 3, kind: 'nuclear', text: 'x', era: 5, era_name: 'modern'}]}),"
                  "  head({done: false, turn: turn + 1, seen: []})];"
                  " pane.sight = kept; return heads; })()")
    assert said == ["No nuclear war in the next 46 turns", "☮ No nuclear war", "☢ Nuclear war in 83 turns (Atomic Era)",
                    "☢ Nuclear war since turn %d (Modern Era)" % (here - 3), "No nuclear war in the next 1 turn"], said
    # The defeat tracker reads the same future (operator 2026-11-26: "add an
    # inverse victory tracker - track progress to a nuclear war and losing.
    # Same format as the victory track. we can call this defeat tracker.
    # track turns away from a nuclear war"): turns away from a nuclear war and
    # from losing the game, and a meter that fills as it nears.
    near = bar.ev("(() => { const pane = debugSplit.panes.a, kept = pane.sight, turn = pane.turn;"
                  " const read = sight => { pane.sight = {...kept, limit: turn + 200, ...sight}; const at = strangeDefeat('a');"
                  "  return [at.nuclear.value, at.losing.value, Math.round(at.nuclear.fill * 100), Math.round(at.losing.fill * 100)]; };"
                  " const out = [read({done: false, turn: turn + 46, seen: []}),"
                  "  read({done: true, turn: turn + 90, seen: [{turn: turn + 40, kind: 'nuclear', text: 'x', era: 6},"
                  "   {turn: turn + 90, kind: 'over', text: 'Brazil wins the game', era: 7}]}),"
                  "  read({done: true, turn: turn + 120, seen: [{turn: turn - 3, kind: 'nuclear', text: 'x', era: 5},"
                  "   {turn: turn + 120, kind: 'won', text: 'Rome wins the game', era: 8}]}),"
                  "  read({done: true, turn: turn + 200, seen: [{turn: turn + 200, kind: 'over', text: 'The game ends', era: 8}]})];"
                  " pane.sight = kept; return out; })()")
    # 200 turns left in the game: a nuclear war 40 turns away is 80% near,
    # a loss 90 turns away 55%.
    assert near == [[">46t", ">46t", 0, 0], ["40t", "90t", 80, 55], ["Now", "Wins", 100, 0], ["None", "None", 0, 0]], near
    card = bar.ev("(() => { const held = document.querySelector('#strange-hud [data-strange-defeat]'), box = held.getBoundingClientRect(),"
                  " versus = document.querySelector('#strange-hud .strange-versus').getBoundingClientRect();"
                  " return {title: held.querySelector('.hud-section-heading').textContent,"
                  "  paths: [...held.querySelectorAll('.victory-track')].map(path => [path.dataset.defeat, path.querySelector('.victory-heading b').textContent,"
                  "   path.querySelector('.victory-subheading').textContent, [...path.querySelectorAll('[data-defeat-world]')].map(row => row.dataset.defeatWorld)]),"
                  "  beside: held.parentElement.classList.contains('strange-right') && box.top >= document.querySelector('#strange-hud [data-strange-paths]').getBoundingClientRect().bottom && box.left > versus.right,"
                  "  kept: strange.cards.some(card => card.left <= box.left && card.right >= box.right && card.top <= box.top), open: strangeOpen()}; })()")
    # A row a world, in the order the worlds stand.
    assert card == {"title": "Defeat tracker", "paths": [["nuclear", "Nuclear war", "Turns away", card["open"]],
                                                       ["losing", "Losing", "Turns away", card["open"]]],
                    "beside": True, "kept": True, "open": card["open"]} and sorted(card["open"]) == ["a", "b"], card
    # It is the future of the line it was played from. A world given another
    # player strategy has another, which is played out in its place, and the
    # one that kept its strategy keeps what was seen.
    kept = bar.ev("debugSplit.panes.b.sight.base")
    bar.ev("debugSplitSeat('a', 'jack'), true")
    bar.until("debugSplit.panes.a.strategy === 'jack' && debugSplit.panes.a.sight && debugSplit.panes.a.sight.asked === 'jack' && debugSplit.panes.a.sight.done",
              "A's future is played out again under its new player strategy")
    assert bar.ev(f"[scouts.made - {began[0]}, debugSplit.panes.b.sight.base, debugSplit.panes.b.sight.done]") == [3, kept, True]
    bar.ev("debugSplitSeat('a', 'stock'), true")
    bar.until("debugSplit.panes.a.strategy === 'stock' && debugSplit.panes.a.sight && debugSplit.panes.a.sight.asked === 'stock' && debugSplit.panes.a.sight.done"
              f" && strangeIdle() && {quiet} && {held}", "and again under the one it had")
    # Where each world's future has an engine of its own (the published
    # build's scouts), every world is asked at once, as many as the computer
    # has cores to spare beside the worlds and the page, and an engine whose
    # game has ended is let go of. With four cores and two worlds one core is
    # to spare, and the followed world is asked first.
    cores = ("Object.defineProperty(navigator, 'hardwareConcurrency', {{value: {0}, configurable: true}});"
             " window.__civvisBeta = {{scouts: 0}}; scouts.wait = 30; strangeSightDrop('a'); strangeSightDrop('b'); strangeForesee();"
             " return [strangeScouts(), Object.keys(strange.seeing).sort().join('')];").format
    assert bar.ev(f"(() => {{ {cores(4)} }})()") == [True, bar.ev("strange.follow")]
    bar.until(seen, "the futures are played out, one after the other")
    assert bar.ev(f"(() => {{ {cores(16)} }})()") == [True, "ab"]
    bar.until(seen, "both futures are played out at once")
    bar.until("scouts.done >= 4 && Object.keys(scouts.held).length === 0", "and their engines let go of")
    bar.ev("delete window.__civvisBeta, delete navigator.hardwareConcurrency, scouts.wait = 0, true")
    bar.until(f"JSON.stringify({paths}) === {json.dumps(json.dumps(PATHS_WANT, ensure_ascii=False, separators=(',', ':')), ensure_ascii=False)}",
              "the predicted paths read as before")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"
    # Time running on is shown the turns that were played ahead, and none is
    # played again: the worlds are asked for more of their futures as they
    # use them up, and never for a turn of their own.
    #
    # Operator 2026-11-24, later: "Right now Doctor Strange's Civ 6 goes 4
    # turns and pauses. This should not happen. It should be a continuous
    # effort. and blazingly fast." Time stopped every fourth turn while each
    # world in turn was played four turns on. Now a world is asked for the
    # one turn that each turn it is shown takes from what it holds: a turn
    # asked for, for every turn time ran, and never four with time held for
    # them. A world never asked for a turn of its own (`steps`) was never
    # short of the next turn, which is all that time waits on.
    #
    # And the page is not laid out again each time a world speaks, which is a
    # dozen times a turn: the bar is out of sight under the worlds, and in a
    # world there is none, so neither is measured, and the cards are measured
    # when one has changed its shape (what the title's card says, here), not
    # for every figure that changes on them.
    stepped = bar.ev(f"[{steps('a')}, {steps('b')}]")
    # What each world was asked for to hold its dozen turns: B once, and A
    # again under each player strategy it was given above.
    filled = bar.ev(f"[{aheads('a')}, {aheads('b')}]")
    assert filled == [[4, 4, 4] * 3, [4, 4, 4]], filled
    bar.ev("(() => { window.measured = {cards: 0, bar: 0, pane: 0};"
           " const cards = strangeCards; strangeCards = function () { measured.cards++; return cards(); };"
           " const left = debugMapLeft; debugMapLeft = function () { measured.bar++; return left(); };"
           " const pane = debugSplit.panes.a.frame.contentWindow, paneLeft = pane.debugMapLeft;"
           " pane.debugMapLeft = function () { measured.pane++; return paneLeft(); }; return true; })()")
    assert bar.ev("[debugBarUnseen(), debugSplit.panes.a.frame.contentWindow.debugBarUnseen()]") == [True, True]
    bar.click('#strange-hud [data-strange="play"]')
    bar.until(f"debugSplit.panes.b.turn >= {here + 6}", "time runs on through the turns played ahead")
    bar.click('#strange-hud [data-strange="play"]')
    bar.until(f"strange.run === null && strangeIdle() && {quiet} && {held}", "paused, with the futures played out again")
    ran = bar.ev(f"[debugSplit.panes.a.turn === debugSplit.panes.b.turn, {steps('a')}, {steps('b')}, debugSplit.panes.a.turn - {here}, {aheads('a')}, {aheads('b')}]")
    assert ran[:3] == [True, stepped[0], stepped[1]] and ran[3] >= 6, ran
    assert ran[4] == filled[0] + [1] * ran[3] and ran[5] == filled[1] + [1] * ran[3], ran
    assert bar.ev("typeof STRANGE_DWELL_MS") == "undefined", "a turn is held on the screen for no longer than the next takes"
    measured = bar.ev("measured")
    assert measured["bar"] == 0 and measured["pane"] == 0 and measured["cards"] < 2 * ran[3], (measured, ran[3])
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"

    # Black swans (operator 2026-11-24: "add a 'Generate Black Swan' - this
    # will be a random, unpredicted event that spawns and changes the game",
    # "black swans on a civilizational timeline"). The button stands in the
    # column on the right, under what the page costs (2026-11-26), with which
    # one beside it, and has
    # the engine strike one on the worlds as they stand. Locked, it is the
    # same one on every world: the followed world is struck first with a seed
    # and no kind named, and the rest with the kind it drew and the same seed.
    now = bar.ev("debugSplit.panes.a.turn")
    struck = "debugSplit.panes.{0}.frame.contentWindow.eval('engine.sent.filter(call => call.path === \"/debug\" && call.body.op === \"black_swan\").map(call => call.body)')".format
    scores = "strange.order.map(letter => debugSplit.panes[letter].numbers.score)"
    before = bar.ev(f"[{names}, {scores}, strange.locked, strange.follow, strange.order.join('')]")
    # The boxes as they stand, the world that is followed, and the other.
    order, lead = before[4], before[3]
    rest = next(letter for letter in order if letter != lead)
    assert before[2] is True and sorted(order) == ["a", "b"], before
    assert bar.ev("[...document.querySelectorAll('#strange-hud [data-strange=\"swan-kind\"] option')].map(option => [option.value, option.textContent, option.title])") == [
        ["", "Any", "One drawn at random, among those the world has something for"],
        ["pandemic", "Pandemic", "Every city in the world loses half its people"],
        ["nuclear_war", "Instant nuclear war", "Every civilization is at war with every other"]]
    placed = bar.ev("(() => { const box = name => document.querySelector(`#strange-hud .${name}`).getBoundingClientRect();"
                    " const swans = box('strange-swans'), meter = box('strange-meter'), paths = box('strange-paths'), versus = box('strange-versus');"
                    " return [swans.top >= meter.bottom, swans.bottom <= paths.top, Math.abs(swans.right - meter.right) < 1, swans.left > versus.right,"
                    " strange.cards.some(card => card.left <= swans.left && card.right >= swans.right && card.top <= swans.top && card.bottom >= swans.bottom)]; })()")
    assert placed == [True] * 5, placed
    bar.ev("strangeSwan('', 8), true")
    assert bar.ev("[strange.swan !== null, strangeIdle(), document.querySelector('#strange-hud [data-strange=\"swan\"]').textContent,"
                  " document.querySelector('#strange-hud [data-strange=\"swan\"]').disabled]") == [True, False, "🦢 Striking…", True]
    bar.until("strange.swan === null && strange.omen !== null && ['a', 'b'].every(letter => (debugSplit.panes[letter].marks.swans || []).length === 1)",
              "a black swan strikes both worlds")
    asked = bar.ev(f"[{struck(lead)}, {struck(rest)}]")
    assert asked == [[{"op": "black_swan", "seed": 8}], [{"op": "black_swan", "seed": 8, "kind": "pandemic"}]], asked
    # The world is another world from there: its score is the swan's doing,
    # the future that was played ahead of it is let go of and played again,
    # and the worlds still stand on the turn they stood on. The swan is in
    # each line's name, on the turn it struck.
    bar.until(f"strangeIdle() && {quiet} && {held}", "each struck world's future is played again")
    after = bar.ev(f"[{names}, {scores}, debugSplit.panes.a.turn, debugSplit.panes.b.turn]")
    assert after == [[f"{name}-🦢{now}" for name in before[0]], [score // 2 for score in before[1]], now, now], (before, after)
    swans = bar.ev("['a', 'b'].map(letter => strangeSwans(letter).map(swan => [swan.turn, swan.kind, swan.name, swan.ongoing]))")
    assert swans == [[[now, "pandemic", "Pandemic", False]]] * 2, swans
    # What struck is said under the worlds' numbers for as long as it is news, with
    # the followed world's account, and on the deck's line. A press puts it
    # away.
    score = dict(zip(order, before[1]))
    omen = bar.ev("(() => { const omen = document.querySelector('#strange-hud [data-strange=\"omen\"]'), box = omen.getBoundingClientRect();"
                  " return [omen.hidden, [...omen.children].map(part => part.textContent), Math.abs(box.left + box.width / 2 - innerWidth / 2) < 2,"
                  " (versus => box.top > versus.bottom && box.top < versus.bottom + 20)(document.querySelector('#strange-hud .strange-versus').getBoundingClientRect()),"
                  " document.querySelector('#strange-hud [data-strange=\"say\"]').textContent]; })()")
    assert omen == [False, [f"🦢 Black swan · turn {now}", "Pandemic", f"The score is {score[lead] // 2} now",
                            f"Worlds {lead.upper()} and {rest.upper()}, the same on each"],
                    True, True, f"🦢 Black swan on turn {now}: Pandemic"], omen
    bar.click('#strange-hud [data-strange="omen"]')
    assert bar.ev("[strange.omen, document.querySelector('#strange-hud [data-strange=\"omen\"]').hidden]") == [None, True]
    # Gone back a turn the worlds are as they were, with the swan still ahead
    # on their line; back on its turn they are the worlds it struck, and no
    # turn was played again for it.
    bar.click('#strange-hud [data-strange="back1"]')
    bar.until(f"{both(now - 1)} && strangeIdle()", "the worlds go back before the swan")
    assert bar.ev(f"[{names}, strangeSwans('{lead}').length]") == [after[0], 1]
    bar.click('#strange-hud [data-strange="on1"]')
    bar.until(f"{both(now)} && strangeIdle() && {quiet} && {held}", "and forward onto its turn again")
    assert bar.ev(f"[{scores}, {steps('a')}, {steps('b')}]") == [after[1], stepped[0], stepped[1]]
    # Unlocked, the one picked strikes the followed world alone, which parts
    # from the other there.
    bar.click('#strange-hud [data-strange="lock"]')
    assert bar.ev("strange.locked") is False
    bar.ev("(() => { const kinds = document.querySelector('#strange-hud [data-strange=\"swan-kind\"]'); kinds.value = 'nuclear_war'; return kinds.value; })()")
    assert f"world {lead.upper()}" in bar.ev("document.querySelector('#strange-hud [data-strange=\"swan\"]').title")
    bar.click('#strange-hud [data-strange="swan"]')
    bar.until(f"strange.swan === null && strange.omen !== null && strangeSwans('{lead}').length === 2", "the picked swan strikes the followed world")
    asked = bar.ev(f"[{struck(lead)}, {struck(rest)}]")
    assert [len(asked[0]), len(asked[1]), asked[0][1].get("kind")] == [2, 1, "nuclear_war"], asked
    alone = bar.ev(f"[{names}, strangeSwans('{rest}').length, [...document.querySelector('#strange-hud [data-strange=\"omen\"]').children].map(part => part.textContent).slice(1),"
                   f" strangeTree().worlds.{rest}.parent || strangeTree().worlds.{lead}.parent, Math.max(strangeTree().worlds.a.fork, strangeTree().worlds.b.fork)]")
    assert alone[:3] == [[f"{name}-🦢{now}" + (f"-🦢{now}" if letter == lead else "") for letter, name in zip(order, before[0])], 1,
                         ["Instant nuclear war", f"The score is {score[lead] // 4} now", f"World {lead.upper()}"]], alone
    bar.until(f"strangeIdle() && {quiet} && {held}", "its future is played again")
    bar.click('#strange-hud [data-strange="lock"]')
    bar.until("strange.locked && strangeIdle()", "the worlds are locked again")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"

    # With no room for two worlds the game is on its one timeline, which is
    # the AI Debugger, with its bar in sight and saying why.
    bar.dev.call("Emulation.setDeviceMetricsOverride", {"width": 800, "height": 900, "deviceScaleFactor": 1, "mobile": False})
    bar.until("innerWidth === 800", "a narrow window")
    bar.dev.call("Page.navigate", {"url": f"{address}?mode=strange&narrow"})
    bar.until(f"{ready} && location.search === '?mode=strange&narrow' && state === null", "the page again, in a narrow window")
    bar.ev("render(engineState()), true")
    bar.until("debugOpenedGame !== null && strange.sorry", "a narrow window is told why it has one timeline")
    assert bar.ev("[debugSplit, getComputedStyle(debugBar).visibility]") == [None, "visible"]
    assert bar.note().startswith("Doctor Strange's CIVVIS sets two worlds side by side and needs a wider window"), bar.note()
    bar.dev.call("Emulation.clearDeviceMetricsOverride")
    bar.until("innerWidth > 1000", "the window is its own size again")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_doctor_strange_autosearch(bar: Bar, address: str) -> None:
    """Doctor Strange's CIVVIS has Autosearch: it searches from the base timeline on copies of that world, and the path found is made a branch.

    Operator, 2026-11-25: "add the autosearch stuff to doctor strange's civ
    6. we should be able to efficiently search paths to try to find a
    desirable path".

    The toy's score rises by two a turn under `stock` and by one under
    `jack`, so from B, which seats jack, the best path is stock alone.
    """
    turns = lambda letters, turn: " && ".join(f"debugSplit.panes.{letter}.turn === {turn}" for letter in letters)
    ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"
    # What an earlier search's panel was set to is read as the page loads.
    bar.ev("localStorage.removeItem('civvis-debug-search-v1'), localStorage.removeItem('civvis-debug-search-v2'), true")
    bar.dev.call("Page.navigate", {"url": f"{address}?mode=strange"})
    bar.until(f"{ready} && location.search === '?mode=strange' && state === null", "Doctor Strange's CIVVIS, loaded again")
    bar.ev("engine.scores = {1: 0}, strange.futures = false, render(engineState()), true")
    bar.settled()
    bar.until(f"debugSplit && debugSplit.shown && {turns('ab', 1)} && strangeIdle()", "its two worlds are there")
    for turn in (2, 3):
        bar.click('#strange-hud [data-strange="on1"]')
        bar.until(f"{turns('ab', turn)} && strangeIdle()", f"the worlds play turn {turn}")
    half = "debugSplit.panes.{}.frame.contentWindow.eval({})".format
    score = lambda letter: bar.ev(half(letter, "'engine.score'"))
    playing = "['step', 'jump', 'play', 'copy', 'ahead', 'walk', 'back', 'forward']"
    sent = lambda letter: bar.ev(half(letter, json.dumps(
        f"engine.sent.filter(call => call.path === '/debug' && {playing}.includes(call.body.op))"
        ".map(call => [call.body.op, call.body.strategy ?? call.body.from ?? null])")))
    # B is the base timeline: it seats jack, which stock beats.
    bar.ev("(() => { const base = document.querySelector('#strange-hud [data-strange=\"base\"]'); base.value = 'b';"
           " base.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    assert bar.ev("[strange.follow, debugSplit.panes.b.strategy]") == ["b", "jack"]
    start, base, other, b = 3, score("b"), score("a"), bar.ev("debugSplit.panes.b.id")

    # The button is on the deck, under Branch, a size larger than the deck's
    # other buttons but Play Time, and opens the AI Debugger's own panel,
    # shown over the tree beside the card though the bar is unseen.
    button = '#strange-hud [data-strange="search"]'
    sizes = bar.ev(f"(() => {{ const mine = document.querySelector('{button}'), branch = document.querySelector('#strange-hud [data-strange=\"branch\"]');"
                   " const tall = held => held.getBoundingClientRect().height, type = held => parseFloat(getComputedStyle(held).fontSize);"
                   " const others = [...document.querySelectorAll('#strange-hud .strange-deck button')].filter(held => held !== mine && !held.classList.contains('strange-play'));"
                   " return [tall(mine) > Math.max(...others.map(tall)), type(mine) > Math.max(...others.map(type)), mine.previousElementSibling === branch]; })()")
    assert sizes == [True, True, True], sizes
    shown = "!document.getElementById('dbg-search').hidden"
    bar.click(button)
    assert bar.ev(f"[{shown}, getComputedStyle(document.getElementById('dbg-search')).visibility, getComputedStyle(debugBar).visibility,"
                  f" document.querySelector('{button}').classList.contains('on'), document.querySelector('{button}').getAttribute('aria-expanded')]") == \
        [True, "visible", "hidden", True, "true"]
    # Lit, its words are dark on gold, as its own colour says, and not the card's.
    assert bar.ev(f"(() => {{ const mine = document.querySelector('{button}'), word = mine.querySelector('span');"
                  " return [getComputedStyle(mine).backgroundColor, getComputedStyle(word).color]; })()") == ["rgb(243, 210, 122)", "rgb(16, 20, 15)"]
    place = bar.ev("(() => { const at = document.getElementById('dbg-search').getBoundingClientRect(),"
                   " card = document.querySelector('#strange-hud .strange-title').getBoundingClientRect();"
                   " const hit = document.elementFromPoint(at.left + 20, at.top + 20);"
                   " return [Math.round(at.left - card.right), Math.round(at.top - card.top), !!hit && !!hit.closest('#dbg-search')]; })()")
    assert abs(place[0] - 10) <= 1 and place[1:] == [0, True], place
    # It searches from the base timeline: the deck picks it, so the panel
    # has no picker of its own.
    about = bar.ev("document.getElementById('dbg-search-about').textContent")
    assert about.startswith(f"From the base timeline, B, on turn {start}: which player strategies should hold the seat, and when, "
                            f"to meet these lists by turn {start + 30}."), about
    assert bar.ev("document.getElementById('dbg-search-half').hidden") is True
    pick = lambda field, value: bar.ev(
        f"(() => {{ const held = document.getElementById('dbg-search-{field}'); held.value = '{value}';"
        " held.dispatchEvent(new Event('change', {bubbles: true})); return true; })()")
    pick("turns", 10)
    pick("cores", 0.25)
    cores = bar.ev("Math.min(64, navigator.hardwareConcurrency || 4)")
    asked = max(1, int(cores * 0.25 + 0.5))

    # A search: on copies of B's own engine, each asked to keep its agents.
    # No world is asked anything while it runs.
    for letter in "ab":
        bar.ev(half(letter, "'engine.sent.length = 0, true'"))
    bar.ev("engine.sent.length = 0, true")
    bar.click('#dbg-search [data-dbg-search="go"]')
    bar.until("!!debugSearch && !debugSearch.live", "the search from B ends", seconds=60)
    found = bar.ev("[debugSearch.state, debugSearch.letter, debugSearch.pane, debugSearch.seated, debugSearch.report.best.plan,"
                   " debugSearch.report.best.verdict.want[0], debugSearch.report.best.last]")
    assert found == ["done", "b", b, "jack", [[start, "stock"]], base + 20, start + 10], (found, bar.ev("debugSearch.note"))
    assert bar.ev("engine.sent.filter(call => call.path === '/forks').map(call => call.body)") == [{"count": asked, "pane": b}, {"count": 0}]
    forked = "engine.sent.filter(call => call.path.startsWith('/debug?fork='))"
    assert bar.ev(f"{forked}.every(call => call.path.endsWith('&pane=' + {json.dumps(b)}))")
    assert bar.ev(f"{forked}.filter(call => call.body.op === 'remember').length") == asked
    assert sent("a") == [] and sent("b") == [], (sent("a"), sent("b"))
    assert bar.ev(f"[{turns('ab', start)}, strange.order.join('')]") == [True, "ab"]

    # The path found is made a branch: C, a copy of B, plays it from the
    # turn the search began on and comes to what the search read; locked,
    # A and B then play on to that turn beside it, each with its own.
    offer = "[...document.querySelectorAll('#dbg-search-play button')].map(held => [held.dataset.dbgSearch, held.textContent, held.disabled])"
    note = "document.querySelector('#dbg-search-play .dbg-search-status').textContent"
    assert bar.ev(offer) == [["branch", "⑂ Branch it", False]], bar.ev(offer)
    assert bar.ev(note) == f"Stock, from turn {start}."
    bar.click('#dbg-search [data-dbg-search="branch"]')
    bar.until(f"strange.order.length === 3 && strange.planning === null && debugSearch.playing === null && strange.run === null"
              f" && {turns('abc', start + 10)} && strangeIdle()", "the path is a branch, and the worlds stand on its last turn", seconds=60)
    assert bar.ev("[strange.order.join(''), debugSplit.panes.c.strategy, document.querySelector('[data-strange-name=\"c\"]').textContent]") == \
        ["abc", "stock", f"Jack-{start}-Stock"]
    assert [score(letter) for letter in "abc"] == [other + 20, base + 10, base + 20]
    played = sent("c")
    assert played[0] == ["copy", b] and ["jump", "stock"] in played and [op for op in played if op[0] == "step"] == [["step", "stock"]] * 10, played
    done = f"Played to turn {start + 10}: SCORE {base + 20}, as found"
    assert bar.ev("[debugSearch.said, document.querySelector('#strange-hud [data-strange=\"say\"]').textContent]") == \
        [done, f"World C is the path found, Stock. {done}"]
    # B has moved on since: the search was of B on turn 3, so it is not branched again.
    assert bar.ev(offer) == [["branch", "⑂ Branch it", True]], bar.ev(offer)
    assert bar.ev(note).startswith(f"B is no longer on turn {start}, where the search began"), bar.ev(note)

    # While a search runs no world's future is begun beside it: the cores
    # are the search's. The button counts the paths played.
    bar.ev("window.__civvisBeta = {scouts: 0}, strange.futures = true, forks.wait = 40, true")
    bar.click('#dbg-search [data-dbg-search="go"]')
    bar.until("debugSearch.state === 'running' && debugSearch.futures > 0", "a slowed search is on its way", seconds=60)
    assert bar.ev(f"document.querySelector('{button}').textContent").startswith("Autosearch· "), bar.ev(f"document.querySelector('{button}').textContent")
    made = bar.ev("strangeSightDrop('a'), strangeSightDrop('b'), strangeSightDrop('c'), strangeForesee(), scouts.made")
    assert bar.ev("[Object.keys(strange.seeing).length, scouts.made]") == [0, made]
    bar.click('#dbg-search [data-dbg-search="go"]')
    assert bar.ev("[debugSearch.live, debugSearch.state]") == [False, "stopped"]
    bar.ev("forks.wait = 0, true")
    bar.until(f"scouts.made > {made}", "the futures carry on once the search is over", seconds=30)
    bar.ev("document.getElementById('dbg-search').dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape', bubbles: true})), true")
    assert bar.ev(f"[{shown}, document.querySelector('{button}').classList.contains('on')]") == [False, False]
    bar.until("Object.keys(strange.seeing).length === 0", "the futures asked for are answered", seconds=30)
    bar.ev("delete window.__civvisBeta, true")
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_doctor_strange_all(bar: Bar, address: str) -> None:
    """Doctor Strange's CIVVIS has All 14,000,605: the best player strategies branched from the base timeline at once, and time run on for all.

    Operator, 2026-11-26: "add a button to the left side top half but the
    bottom of it in doctor strange that says "All 14,000,605" and when
    clicked every ai-strategy we have (we can say top 10 for now) spawns from
    the current point and all start simulating towards the end. time carries
    forward for all. spawn a window for each".

    The toy's roster is made eleven long here, rated below Jack and Stock,
    so the top ten leave one out.
    """
    more = [[f"s{rank}", 1000 - 10 * (rank - 2)] for rank in range(3, 12)]
    ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"
    button = '#strange-hud [data-strange="all"]'
    said = "document.querySelector('#strange-hud [data-strange=\"say\"]').textContent"
    seats = "strange.order.map(letter => debugSplit.panes[letter].strategy)"
    together = lambda turn: f"strange.order.every(letter => debugSplit.panes[letter].turn >= {turn})"
    bar.ev(f"localStorage.setItem('civvis-toy-strategies', {json.dumps(json.dumps(more))}), true")
    try:
        bar.dev.call("Page.navigate", {"url": f"{address}?mode=strange"})
        bar.until(f"{ready} && location.search === '?mode=strange' && state === null", "Doctor Strange's CIVVIS, with eleven strategies")
        bar.ev("engine.scores = {1: 0}, strange.futures = false, render(engineState()), true")
        bar.settled()
        bar.until("debugSplit && debugSplit.shown && debugSplit.panes.a.turn === 1 && debugSplit.panes.b.turn === 1 && strangeIdle()",
                  "its two worlds are there")
        assert bar.ev("strangeRanked().names") == ["jack", "stock"] + [name for name, _ in more]
        assert bar.ev(f"[{seats}, strange.follow]") == [["stock", "jack"], "a"]
        # At the foot of the tree on the left, standing on the line: under
        # the title's card, clear of the turn written on the line, with the
        # line under all of it, the column on the right included.
        place = bar.ev(f"""(() => {{
          const box = name => document.querySelector(`#strange-hud ${{name}}`).getBoundingClientRect();
          const all = box('[data-strange="all"]'), title = box('.strange-title'), turn = box('.strange-turn'), column = box('.strange-right');
          const line = strangeGeometry().cy;
          return [Math.round(all.left) === 14, Math.round(all.bottom) === line - 12, Math.round(column.bottom) <= line - 12,
            all.top > title.bottom, all.right < turn.left, all.right < innerWidth / 2, line >= strange.floor,
            strange.cards.some(card => card.side < 0 && Math.round(card.top) === Math.round(all.top))];
        }})()""")
        assert place == [True] * 8, place
        # Its words are the operator's, in Doctor Strange's colours. The base,
        # A, seats Stock, and B seats Jack and is still the same world on
        # turn 1: of the top ten that leaves eight to branch.
        look = bar.ev(f"(() => {{ const held = document.querySelector('{button}'), seen = getComputedStyle(held);"
                      " return [held.textContent, held.disabled, seen.color, seen.borderTopColor, seen.backgroundImage.startsWith('linear-gradient'), held.title]; })()")
        assert look[:5] == ["All 14,000,605", False, "rgb(255, 227, 163)", "rgb(255, 154, 46)", True], look
        assert look[5].endswith("each in a window of its own, and time runs on for all the worlds to the end of the game: 8 new worlds"), look[5]
        assert bar.ev("strangeAllNames()") == [name for name, _ in more[:8]]
        # Pressed: a world for each, in a box of its own beside the base, best
        # first, each a copy of A on turn 1 with its own strategy in the seat;
        # then time runs on for all ten, together.
        bar.click(button)
        bar.until(f"strange.order.length === 10 && strange.all === null && strange.run !== null", "every strategy is a world, and time runs",
                  seconds=60)
        assert bar.ev("strange.order.join('')") == "acdefghijb"
        assert bar.ev(seats) == ["stock"] + [name for name, _ in more[:8]] + ["jack"]
        assert bar.ev(said) == "All 14,000,605: 8 branches of A from turn 1, and time runs on for all 10 worlds, to the end.", bar.ev(said)
        assert bar.ev("document.querySelectorAll('#debugsplit iframe:not(.strange-shut)').length") == 10
        bar.until("strange.order.slice(1, 9).every(letter => debugSplit.panes[letter].frame.contentWindow.eval('engine.seat') === debugSplit.panes[letter].strategy)",
                  "each new world is seated with its own strategy")
        copies = bar.ev("strange.order.slice(1, 9).map(letter => debugSplit.panes[letter].frame.contentWindow.eval("
                        "'engine.sent.filter(call => call.path === \"/debug\" && call.body.op === \"copy\").length'))")
        assert copies == [1] * 8, copies
        bar.ev("window.strangeGaps = []; window.strangeWatch = setInterval(() => { const turns = strange.order.map(letter => debugSplit.panes[letter].turn);"
               " strangeGaps.push(Math.max(...turns) - Math.min(...turns)); }, 4); true")
        bar.until(together(bar.ev("Math.max(...strange.order.map(letter => debugSplit.panes[letter].turn))") + 3),
                  "time carries forward for all ten", seconds=60)
        gaps = bar.ev("clearInterval(strangeWatch), strangeGaps")
        assert gaps and max(gaps) <= 1, gaps
        # Pressed while time runs: time stops, the best of those without a
        # world of their own on that turn branch A there, as many as fit in
        # the twelve, and time runs on again for all.
        bar.click(button)
        bar.until("strange.order.length === 12 && strange.all === null && strange.run !== null", "pressed again while time runs", seconds=60)
        assert bar.ev("strange.order.join('')") == "aklcdefghijb"
        assert bar.ev(seats)[:3] == ["stock", "jack", "s3"]
        turn = bar.ev("strangeTree().worlds.k.fork")
        assert bar.ev(said) == (f"All 14,000,605: 2 branches of A from turn {turn}, and time runs on for all 12 worlds, to the end. "
                                "7 more did not fit: 12 worlds is the most."), (bar.ev(said), turn)
        assert bar.ev(f"[document.querySelector('{button}').disabled, document.querySelector('{button}').title]") == \
            [True, "12 worlds are open, which is the most. Delete some to branch every player strategy"]
        bar.until(together(turn + 2), "time carries forward for all twelve", seconds=60)
        bar.click('#strange-hud [data-strange="play"]')
        bar.until("strange.run === null && strangeIdle()", "Pause stops the worlds", seconds=30)
        assert len(set(bar.ev("strange.order.map(letter => debugSplit.panes[letter].turn)"))) == 1
        assert not bar.dev.console, f"the page complained: {bar.dev.console}"
    finally:
        bar.ev("localStorage.removeItem('civvis-toy-strategies'), true")


def check_doctor_strange_world_ends(bar: Bar, address: str) -> None:
    """Doctor Strange's CIVVIS: a world whose game is over says how in its own box, and time runs on for the rest.

    Operator, 2026-11-26: "for doctor strange, when a screen finishes it
    should not halt the game (unless the final screen). we should make a
    quikc summary screen to describe the outcome, with maybe a "view map"
    and a "close" option".
    """
    ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"
    said = "document.querySelector('#strange-hud [data-strange=\"say\"]').textContent"
    pane = lambda letter, code: f"debugSplit.panes.{letter}.frame.contentWindow.eval({json.dumps(code)})"
    card = lambda letter: bar.ev(pane(letter, """(() => {
      const card = document.getElementById('strange-result'), chip = document.getElementById('strange-result-chip');
      if (!card) return null;
      const text = name => (card.querySelector(name) || {}).textContent || '';
      return {shown: !card.hidden, chip: chip.hidden ? null : chip.textContent, kicker: text('.sr-kicker'), title: text('.sr-title'),
        verdict: text('.sr-verdict'), meta: text('.sr-meta'), buttons: [...card.querySelectorAll('button')].map(b => [b.textContent, b.disabled]),
        winner: getComputedStyle(document.getElementById('winner')).display};
    })()"""))
    press = lambda letter, what: bar.ev(pane(letter, f"document.querySelector({json.dumps(what)}).click(), true"))
    bar.dev.call("Page.navigate", {"url": f"{address}?mode=strange"})
    bar.until(f"{ready} && location.search === '?mode=strange' && state === null", "Doctor Strange's CIVVIS")
    bar.ev("engine.scores = {1: 0}, strange.futures = false, render(engineState()), true")
    bar.settled()
    bar.until("debugSplit && debugSplit.shown && debugSplit.panes.a.turn === 1 && debugSplit.panes.b.turn === 1 && strangeIdle()",
              "its two worlds are there")
    # B's game is won on turn 4. Time runs on for A past it; B stays where its
    # game ended, and its box says so: who won, how, on which turn, with what.
    # B is the world followed, and the turn written on the line is time's, not
    # the turn B's game stopped on.
    bar.ev(pane("b", "engine.over = {turn: 4, winner: 0}, true"))
    bar.ev("strange.follow = 'b', strangeDraw(), true")
    bar.click('#strange-hud [data-strange="play"]')
    bar.until("debugSplit.panes.a.turn >= 8 && strange.run !== null", "time runs on for A after B's game is over", seconds=30)
    assert bar.ev("[debugSplit.panes.b.turn, debugSplit.panes.b.over]") == [4, {"turn": 4, "how": "won"}]
    bar.until("+document.querySelector('#strange-hud [data-strange=\"turn\"]').textContent >= 8", "the line says time's turn")
    won = card("b")
    assert won == {"shown": True, "chip": None, "kicker": "Victory", "title": "rome won", "verdict": "Science victory",
                   "meta": won["meta"], "buttons": [["View map", False], ["Close", False]], "winner": "none"}, won
    assert won["meta"].startswith("Turn 4 · Score "), won
    # View map puts the card away and leaves a chip that brings it back.
    press("b", "#strange-result [data-sr=map]")
    assert card("b")["shown"] is False and card("b")["chip"] == "★ Victory · turn 4", card("b")
    press("b", "#strange-result-chip")
    assert card("b")["shown"] is True and card("b")["chip"] is None
    # Close deletes the world while time runs on for the other.
    press("b", "#strange-result [data-sr=close]")
    bar.until("strange.order.join('') === 'a' && debugSplit.panes.b.frame.classList.contains('strange-shut')", "Close deletes B")
    went = bar.ev("debugSplit.panes.a.turn")
    bar.until(f"debugSplit.panes.a.turn >= {went + 2} && strange.run !== null", "and time runs on for A", seconds=30)
    # The last world's game is over: now time stops, and its card says so.
    end = bar.ev("debugSplit.panes.a.turn") + 2
    bar.ev(pane("a", f"engine.over = {{turn: {end}, winner: 1}}, true"))
    bar.until("strange.run === null", "time stops when the last world's game is over", seconds=30)
    assert bar.ev("debugSplit.panes.a.turn") == end
    assert bar.ev(said) == f"A's game is over on turn {end}", bar.ev(said)
    lost = card("a")
    assert [lost["shown"], lost["kicker"], lost["title"], lost["buttons"]] == \
        [True, "Defeat", "greece won", [["View map", False], ["Close", True]]], lost
    assert not bar.dev.console, f"the page complained: {bar.dev.console}"


def check_edge_of_tomorrow(bar: Bar, address: str) -> None:
    """Edge of Tomorrow: the played game with a jump back.

    Operator, 2026-11-25: "above civ 6 and below ai debugger add: Edge of
    Tomorrow. This should be civ 6 with a jump back (like the concept from
    the movie)".
    """
    ready = "typeof drawDebugBar === 'function' && typeof engineState === 'function'"
    # The view the AI Debugger was left in is its own, and this page, which
    # is always the Civilization VI screen, does not write over it.
    view = bar.ev("localStorage.getItem('civvis-debugger-view-v1')")
    bar.dev.call("Page.navigate", {"url": f"{address}?mode=tomorrow"})
    bar.until(f"{ready} && location.search === '?mode=tomorrow' && state === null", "the page, asked for as Edge of Tomorrow")
    assert bar.ev("[TOMORROW, STRANGE, SOLO_UI, document.documentElement.classList.contains('tomorrow')]") == \
        [True, False, True, True]

    def play(turns: int) -> int:
        """Turns played, whoever played them: the seat's turn opens again each time."""
        return bar.ev("(async () => { for (let n = 0; n < %d; n++) render(await fetchJSON('/debug', {method: 'POST',"
                      " body: JSON.stringify({op: 'step', strategy: 'jack'})})); return state.turn; })()" % turns)

    def rail() -> dict:
        return bar.ev("""(() => {
          const marks = kind => [...document.querySelectorAll(`#tmr-rail .${kind}`)].map(mark =>
            ({left: mark.style.left, width: mark.style.width, title: mark.title, text: mark.textContent, hand: mark.classList.contains('tmr-by-hand')}));
          const jump = document.getElementById('tmr-jump'), wake = document.getElementById('tmr-wake');
          return {now: marks('tmr-now'), wake: marks('tmr-wake'), lived: marks('tmr-lived'), done: marks('tmr-done'),
            ends: marks('tmr-end'), mem: marks('tmr-mem'), loop: document.getElementById('tmr-loop').textContent,
            said: document.getElementById('tmr-said').textContent,
            jump: {off: jump.disabled, to: jump.querySelector('small').textContent, due: jump.classList.contains('tmr-due')},
            here: {off: wake.disabled, says: wake.textContent}};
        })()""")

    # A game of forty turns in which Greece declares war on the seat on turn 4.
    bar.ev("engine.limit = 40, engine.raids = [4], render(engineState()), true")
    bar.until("document.getElementById('tomorrowbar') && !document.getElementById('tomorrowbar').hidden",
              "the bar of Edge of Tomorrow is there")
    # Operator, 2026-11-26: "lets move the civvis: edge of tomorrow top bar
    # to the bottom of the screen". The bar stands across the foot of the
    # window from where the map area begins, with the map area ending above
    # it. The debugger's own bar is out of sight and holds no room.
    shape = bar.ev("""(() => {
      const box = id => { const r = document.getElementById(id).getBoundingClientRect();
        return {left: Math.round(r.left), top: Math.round(r.top), right: Math.round(r.right), bottom: Math.round(r.bottom), height: Math.round(r.height)}; };
      const root = document.documentElement, debug = document.getElementById('debugbar');
      return {bar: box('tomorrowbar'), area: box('maparea'), wide: innerWidth, tall: innerHeight, docked: root.classList.contains('tmr-docked'),
        foot: getComputedStyle(root).getPropertyValue('--tmr-bottom').trim(),
        debug: debug ? getComputedStyle(debug).display : 'none', debugDocked: root.classList.contains('dbg-docked')};
    })()""")
    assert shape["bar"]["left"] == 200 and shape["bar"]["bottom"] == shape["tall"] and shape["bar"]["right"] == shape["wide"], shape
    assert shape["bar"]["height"] > 40 and shape["area"]["top"] == 0 and shape["area"]["bottom"] == shape["bar"]["top"], shape
    assert shape["docked"] is True and shape["foot"] == f"{shape['bar']['height']}px", shape
    assert shape["debug"] == "none" and shape["debugDocked"] is False, shape
    # Operator, 2026-11-26: "on the right, the biuttons shold be: (top row):
    # turns, civ ai (bottom row): jump back, auto-play N turns, wake here. new
    # game remains in the left panel." The left panel's Auto-play is in the
    # bar, its pickers over the buttons, and its place in the left panel is
    # gone; Start new game stays there.
    acts = bar.ev("""(() => {
      const box = el => { const r = el.getBoundingClientRect(); return {left: Math.round(r.left), right: Math.round(r.right), top: Math.round(r.top), bottom: Math.round(r.bottom)}; };
      const field = id => box(document.getElementById(id).closest('label'));
      const button = id => box(document.getElementById(id));
      const inBar = id => !!document.getElementById(id).closest('#tomorrowbar .tmr-acts');
      return {turns: field('autoplayturns'), ai: field('autoplaystrategy'),
        jump: button('tmr-jump'), autoplay: button('autoplaybtn'), wake: button('tmr-wake'),
        inBar: ['autoplayturns', 'autoplaystrategy', 'autoplaybtn', 'autoplaynote'].every(inBar),
        footer: getComputedStyle(document.getElementById('humanfooter')).display,
        fresh: !!document.getElementById('restart-sim').closest('#side')};
    })()""")
    assert acts["inBar"] is True and acts["footer"] == "none" and acts["fresh"] is True, acts
    top, low = (acts["turns"], acts["ai"]), (acts["jump"], acts["autoplay"], acts["wake"])
    assert max(box["bottom"] for box in top) <= min(box["top"] for box in low), acts
    assert acts["turns"]["right"] <= acts["ai"]["left"], acts
    assert acts["jump"]["right"] <= acts["autoplay"]["left"] and acts["autoplay"]["right"] <= acts["wake"]["left"], acts
    assert acts["turns"]["left"] == acts["jump"]["left"] and acts["ai"]["right"] == acts["wake"]["right"], acts
    # What Auto-play's note says is said on the bar.
    bar.ev("document.getElementById('autoplaynote').textContent = 'Stopped · 3 turns unplayed', true")
    bar.until("document.getElementById('tmr-said').textContent === 'Stopped · 3 turns unplayed'", "the note is said on the bar")
    bar.ev("document.getElementById('autoplaynote').textContent = '', tomorrowSay(''), tomorrowDraw(false), true")
    # Operator, 2026-11-26: "civvis: edge of tomortrow should not have the
    # world minimap or <...(spin)x thing showing", and "we should also have
    # the victory tracker and player hud showing for this. and the governors
    # / tech / policy screen". The minimap and the map's controls are put
    # away; the Rankings (the standings and the victory tracker) were opened
    # before the first state was drawn, with the kept choice left unread; and
    # the seat's research box is asked for, without the live mirror's Pause
    # and New game.
    shown = bar.ev("""(() => {
      const hidden = [...document.styleSheets].flatMap(sheet => { try { return [...sheet.cssRules]; } catch (_) { return []; } })
        .filter(rule => rule.style && rule.style.display === 'none' && rule.style.getPropertyPriority('display') === 'important')
        .flatMap(rule => rule.selectorText.split(',').map(part => part.trim()));
      return {map: ['.minimap-frame', '#map-controls-dock'].map(pick => getComputedStyle(document.querySelector(pick)).display),
        ranked: [rankingsReportOpen, soloRankingsSettled], tracker: window.civvisSeatTracker,
        live: ['.livepause-buttons', '#livepause-note'].filter(part => hidden.includes('html.tomorrow ' + part))};
    })()""")
    assert shown == {"map": ["none", "none"], "ranked": [True, True], "tracker": True,
                     "live": [".livepause-buttons", "#livepause-note"]}, shown
    # Operator, 2026-11-26: "civ autoplay should be gold and new game grey.
    # in civ edge of tomorrow". The left panel's Auto-play wears the gold and
    # its Start new game the plain grey, where the page's own rules have them
    # the other way round.
    paint = bar.ev("""(() => {
      const look = id => getComputedStyle(document.getElementById(id)).backgroundImage;
      return {autoplay: look('autoplaybtn'), fresh: look('restart-sim')};
    })()""")
    assert paint["autoplay"].startswith("linear-gradient(135deg, rgb(216, 182, 99)"), paint
    assert paint["fresh"].startswith("linear-gradient(rgb(35, 39, 42)"), paint
    # The seat is the person's: nothing is asked of the engine to open the
    # game, where the AI Debugger seats its best Civ AI.
    assert bar.sent() == [], bar.sent()
    # And its only decisions are Auto-play and the jump (operator,
    # 2026-11-26: "a player's only controls are the auto play stuff and the
    # jump back / wake here ... the bottom stuff that player make decisions
    # we gotta remove"): the unit's commands, the turn's action panel, the
    # research and civic prompts are not shown, and End turn sent by hand
    # (a key, a tree) asks the engine nothing and leaves the turn open.
    hands = bar.ev("""(async () => {
      const gone = id => { const held = document.getElementById(id); return held ? getComputedStyle(held).display === 'none' : 'absent'; };
      const turn = state.turn;
      await send({type: 'end_turn'});
      // The stand-in page has no Civilization VI screen, so the rule that
      // takes it away is read from the page's styles.
      const hidden = [...document.styleSheets].flatMap(sheet => { try { return [...sheet.cssRules]; } catch (_) { return []; } })
        .filter(rule => rule.style && rule.style.display === 'none' && rule.style.getPropertyPriority('display') === 'important')
        .flatMap(rule => rule.selectorText.split(',').map(part => part.trim()));
      return {off: window.civvisSeatHandsOff, same: state.turn === turn, pending: turnAdvancePending,
              autoplay: gone('autoplaybtn') === false,
              gone: ['#ubar', '#actionpanel', '#worldtracker', '#notify .notice.tone-action'].filter(part => hidden.includes('html.tomorrow ' + part))};
    })()""")
    assert hands == {"off": True, "same": True, "pending": False, "autoplay": True,
                     "gone": ["#ubar", "#actionpanel", "#worldtracker", "#notify .notice.tone-action"]}, hands
    assert bar.sent() == [], bar.sent()
    # Loop 1, on the first turn: there is nothing to jump back from and the
    # wake point is where it is.
    seen = rail()
    assert seen["loop"] == "1" and [mark["text"] for mark in seen["now"]] == ["1"], seen
    assert seen["jump"] == {"off": True, "to": "to turn 1", "due": False}, seen
    assert seen["here"] == {"off": True, "says": "Wake here"}, seen
    assert seen["ends"] == [] and seen["mem"] == [], seen
    assert "Jump back returns the world to turn 1" in seen["said"], seen

    # Five turns on. The line is the whole game, turn 1 to turn 40: the open
    # turn stands 5 of its 39 turns along, with this loop's turns drawn up to
    # it from the turn woken on. The war of turn 4 is behind, and is this
    # loop's own to have seen.
    assert play(5) == 6
    seen = rail()
    assert seen["now"][0]["text"] == "6" and seen["now"][0]["left"] == "12.82%", seen
    assert seen["wake"][0]["left"] == "0%" and seen["done"][0]["width"] == "12.82%", seen
    assert seen["jump"]["off"] is False and seen["here"] == {"off": False, "says": "Wake here"}, seen
    assert seen["mem"] == [] and seen["ends"] == [], seen
    assert bar.ev("[...tomorrow.now.values()]") == \
        [{"turn": 4, "kind": "declared", "loop": 1, "text": "greece declared war on rome"}]

    # Jump back. The engine is asked for the turn woken on, by name, once.
    # It is loop 2, the next unit waiting for orders is taken up as at the
    # top of any turn, and the banners take attendance again: they compare a
    # turn with the one before it, and announced a peace for a war not begun.
    # A banner that is up or waiting is of the turn that was left, and goes.
    bar.forget()
    bar.ev("""painted.length = 0, rivalsWorld = 7, boostsWorld = 7, momentQueue.push('a banner'), momentsShowing = 1,
      document.getElementById('moments').innerHTML = '<button class="moment">A new age</button>', true""")
    bar.click("#tmr-jump")
    bar.until("state.turn === 1 && tomorrow.loop === 2 && tomorrowIdle()", "the world is back on the turn woken on, in loop 2")
    assert bar.sent() == [{"op": "jump", "turn": 1}], bar.sent()
    assert bar.ev("[painted.includes('advanceToNextActionUnit'), rivalsWorld, boostsWorld, momentQueue.length, momentsShowing,"
                  " document.querySelectorAll('#moments .moment').length]") == [True, None, None, 0, 0, 0]
    # Where loop 1 ended is on the line, as an end by hand, and what it saw
    # done to the seat is ahead of the open turn: the war, on turn 4.
    seen = rail()
    assert seen["loop"] == "2" and seen["now"][0]["text"] == "1", seen
    assert seen["ends"] == [{"left": "12.82%", "width": "", "title": "Loop 1 ended on turn 6: jumped back by hand", "text": "↺", "hand": True}], seen
    assert seen["mem"] == [{"left": "7.69%", "width": "", "title": "Loop 1, turn 4: greece declared war on rome", "text": "⚔", "hand": False}], seen
    assert seen["lived"][0]["width"] == "12.82%" and seen["done"][0]["width"] == "0%", seen
    assert seen["said"] == "Loop 1 ended on turn 6: jumped back by hand", seen
    assert seen["jump"]["off"] is True and seen["here"]["off"] is True, seen
    # A turn on, the bar says what is remembered of the turns ahead.
    assert play(1) == 2
    assert rail()["said"] == "Seen in loop 1: turn 4, greece declared war on rome"

    # Wake here, on turn 3: the world as it stands is kept as a save point of
    # the engine's under the page's one name, and a jump back wakes there.
    assert play(1) == 3
    bar.forget()
    bar.click("#tmr-wake")
    bar.until("tomorrow.wake.turn === 3 && tomorrowIdle()", "the wake point is moved to the open turn")
    assert bar.sent() == [{"op": "save_point", "name": "Edge of Tomorrow"}], bar.sent()
    kept = bar.ev("tomorrow.wake")
    assert kept["save"] == bar.ev("engine.saves.find(save => save.name === 'Edge of Tomorrow').id") and kept["acted"] == 0, kept
    seen = rail()
    assert seen["wake"][0]["left"] == "5.13%" and seen["jump"] == {"off": True, "to": "to turn 3", "due": False}, seen
    # Standing on a wake point that was moved, the button moves it back.
    assert seen["here"] == {"off": False, "says": "Wake on turn 1"}, seen
    assert seen["said"] == "A jump back now wakes here, on turn 3", seen
    # Past turn 4 nothing remembered is ahead any more.
    assert play(2) == 5
    assert rail()["mem"] == []

    # The seat falls. The page draws its own result card, offering another
    # game and counting down to one; a lost game is not the end of this one,
    # so the count is called off and the card is the loop's, with the jump
    # back where the new game was offered and a count of its own.
    cleared = bar.ev("finale.cleared")
    bar.ev("engine.falls = 5, render(engineState()), true")
    bar.until("document.querySelector('#winner .tmr-card') && tomorrow.count", "a lost loop has its card and its count")
    card = bar.ev("""(() => { const card = document.getElementById('winner');
      return {text: card.textContent.replace(/\s+/g, ' ').trim(), restart: !!document.getElementById('finale-restart'),
        visible: card.classList.contains('visible'), jump: document.getElementById('tmr-card-jump').textContent,
        left: tomorrow.count.left, cleared: finale.cleared, fate: tomorrow.fate}; })()""")
    assert card["visible"] is True and card["restart"] is False and card["cleared"] == cleared + 1, card
    assert "Loop 2 ends · Turn 5" in card["text"] and "rome has been defeated" in card["text"], card
    assert "The world goes back to turn 3, as it stood" in card["text"], card
    assert card["jump"] == "Jump back to turn 3 (8)" and card["left"] == 8, card
    assert card["fate"] == {"how": "fell", "words": "rome has been defeated"}, card
    assert rail()["here"]["off"] is True
    # The page draws its card again whenever the result's signature changes,
    # and the loop's card takes its place again, the count to a new game
    # called off again. The count to the jump goes on as it was.
    bar.ev("document.getElementById('winner').dataset.signature = 'another', render(engineState()), true")
    assert bar.ev("[!!document.querySelector('#winner .tmr-card'), !!document.getElementById('finale-restart'), finale.cleared, !!tomorrow.count]") == \
        [True, False, cleared + 2, True]
    # Look around first: the count stops and the card is put away, and the
    # bar's Jump back waits, lit.
    bar.click("#tmr-card-stay")
    looking = bar.ev("""[tomorrow.count, tomorrow.looking, document.documentElement.classList.contains('tmr-looking'),
      getComputedStyle(document.getElementById('winner')).display]""")
    assert looking == [None, True, True, "none"], looking
    assert rail()["jump"] == {"off": False, "to": "to turn 3", "due": True}
    # Jump back from the bar: to the save point the wake point is kept as.
    bar.forget()
    bar.click("#tmr-jump")
    bar.until("state.turn === 3 && tomorrow.loop === 3 && tomorrowIdle()", "the world is back on the moved wake point, in loop 3")
    assert bar.sent() == [{"op": "goto_save", "id": kept["save"]}], bar.sent()
    gone = bar.ev("""[document.getElementById('winner').classList.contains('visible'), document.getElementById('winner').children.length,
      document.documentElement.classList.contains('tmr-looking'), tomorrow.fate, tomorrow.looking]""")
    assert gone == [False, 0, False, None, False], gone
    seen = rail()
    # Loop 1's end, by hand on turn 6, and loop 2's, a defeat on turn 5.
    assert [(end["title"], end["text"], end["hand"]) for end in seen["ends"]] == [
        ("Loop 1 ended on turn 6: jumped back by hand", "↺", True),
        ("Loop 2 ended on turn 5: rome has been defeated", "✕", False)], seen
    assert seen["loop"] == "3" and seen["jump"]["due"] is False, seen

    # Left alone, a lost loop jumps back when its count runs out.
    assert play(2) == 5
    bar.until("tomorrow.count && tomorrow.count.left === 8", "the next defeat counts down again")
    bar.forget()
    bar.ev("tomorrow.count.left = 1, true")
    bar.until("state.turn === 3 && tomorrow.loop === 4 && tomorrowIdle()", "the count ran out and the world went back", seconds=10)
    assert bar.sent() == [{"op": "goto_save", "id": kept["save"]}], bar.sent()
    assert bar.ev("tomorrow.count") is None

    # Somebody else wins: that is a lost loop too, and its card says who
    # won and how. The card's own button jumps at once.
    bar.ev("engine.falls = null, engine.over = {turn: 4, winner: 1}, true")
    assert play(1) == 4
    bar.until("document.querySelector('#winner .tmr-card') && tomorrow.count", "a game somebody else won is a lost loop")
    assert "greece won: Science victory" in bar.ev("document.getElementById('winner').textContent")
    bar.click("#tmr-card-jump")
    bar.until("state.turn === 3 && tomorrow.loop === 5 && tomorrowIdle()", "the card's button jumps back")

    # The seat wins: the page's own card stands, another game on offer, and
    # says which loop won it. Nothing counts down to a jump, and the count
    # to another game is the page's own to keep. Jump back is still there.
    cleared = bar.ev("finale.cleared")
    bar.ev("engine.over = {turn: 4, winner: 0}, true")
    assert play(1) == 4
    won = bar.ev("""[!!document.getElementById('finale-restart'), !!document.querySelector('#winner .tmr-card'),
      [...document.querySelectorAll('#winner .tmr-won')].map(line => line.textContent), tomorrow.count, finale.cleared,
      tomorrow.fate, document.getElementById('tmr-jump').disabled]""")
    assert won == [True, False, ["Won on loop 5"], None, cleared, {"how": "won", "words": "rome won"}, False], won
    # Drawn again, the card says it once.
    bar.ev("render(engineState()), true")
    assert bar.ev("document.querySelectorAll('#winner .tmr-won').length") == 1

    # Back on the moved wake point, the button moves the wake point to the
    # first turn again: the save point is let go of, and a jump goes there.
    bar.ev("engine.over = null, true")
    bar.click("#tmr-jump")
    bar.until("state.turn === 3 && tomorrow.loop === 6 && tomorrowIdle()", "a won game is jumped back from too")
    # A wake point is only as good as the save point the engine holds for
    # it: with that gone from the engine, a jump back wakes on turn 1.
    held = bar.ev("JSON.stringify(engine.saves)")
    bar.ev("engine.saves = [], render(engineState()), true")
    assert bar.ev("tomorrow.wake") == {"turn": 1, "save": None, "acted": 0}
    assert rail()["jump"]["to"] == "to turn 1"
    bar.ev(f"engine.saves = {held}, tomorrow.wake = {json.dumps(kept)}, render(engineState()), true")
    assert rail()["here"] == {"off": False, "says": "Wake on turn 1"}
    bar.forget()
    bar.click("#tmr-wake")
    bar.until("tomorrow.wake.save === null && tomorrowIdle()", "the wake point is back on the first turn")
    assert bar.sent() == [{"op": "drop_save", "id": kept["save"]}], bar.sent()
    assert bar.ev("tomorrow.wake") == {"turn": 1, "save": None, "acted": 0}
    seen = rail()
    assert seen["jump"] == {"off": False, "to": "to turn 1", "due": False} and seen["here"] == {"off": False, "says": "Wake here"}, seen
    bar.forget()
    bar.click("#tmr-jump")
    bar.until("state.turn === 1 && tomorrow.loop === 7 && tomorrowIdle()", "and a jump back wakes on turn 1")
    assert bar.sent() == [{"op": "jump", "turn": 1}], bar.sent()

    # With little room the line goes under the name and the buttons, nothing
    # is cut off, and the map area still ends above the whole bar.
    bar.dev.call("Emulation.setDeviceMetricsOverride", {"width": 760, "height": 800, "deviceScaleFactor": 1, "mobile": False})
    bar.until("innerWidth === 760", "the window is narrow")
    bar.ev("window.dispatchEvent(new Event('resize')), true")
    narrow = bar.ev("""(() => { const top = pick => Math.round(document.querySelector(pick).getBoundingClientRect().top);
      const box = document.getElementById('tomorrowbar'), rect = box.getBoundingClientRect();
      return {name: top('#tomorrowbar .tmr-name'), acts: top('#tomorrowbar .tmr-acts'), track: top('#tomorrowbar .tmr-track'),
        fits: box.scrollWidth <= box.clientWidth, right: Math.round(rect.right), wide: innerWidth,
        jump: Math.round(document.getElementById('tmr-wake').getBoundingClientRect().right),
        area: Math.round(document.getElementById('maparea').getBoundingClientRect().bottom), top: Math.round(rect.top),
        bottom: Math.round(rect.bottom), tall: innerHeight}; })()""")
    assert narrow["track"] > narrow["name"] and narrow["track"] > narrow["acts"], narrow
    assert narrow["fits"] is True and narrow["jump"] <= narrow["right"] == narrow["wide"], narrow
    assert narrow["area"] == narrow["top"] and narrow["bottom"] == narrow["tall"], narrow
    bar.dev.call("Emulation.clearDeviceMetricsOverride")
    bar.until("innerWidth > 760", "the window is wide again")
    bar.ev("window.dispatchEvent(new Event('resize')), true")

    # Another game is loop 1 again, with nothing remembered and the wake
    # point on its first turn.
    bar.ev("""engine.seed = 8, engine.turn = 1, engine.marks = [1], engine.worlds = {1: 1}, engine.by = {}, engine.saves = [],
      engine.raids = [], render(engineState()), true""")
    seen = rail()
    assert seen["loop"] == "1" and seen["ends"] == [] and seen["mem"] == [] and seen["jump"]["off"] is True, seen
    assert bar.ev("[tomorrow.loop, tomorrow.ends.length, tomorrow.seen.size, tomorrow.wake.turn, tomorrow.wake.save]") == [1, 0, 0, 1, None]
    # And the AI Debugger's view is as it was left.
    assert bar.ev("localStorage.getItem('civvis-debugger-view-v1')") == view


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
              "makes a branch for it; Branch asks which Civ AI plays the branch, under itself, before there is "
              "one, makes it when one is picked, and makes none when it is not.")
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
        check_the_timeline_says_click_to_travel(bar)
        print("the timeline says Click to travel, large and calling, in the open rail beside the marker, while a "
              "click would travel; a click on the timeline, or on a mark on it, makes it go for good.")
        check_the_game_opens_on_one_timeline(bar, f"http://127.0.0.1:{port}/")
        print("a game opens on its one timeline with the top-rated Civ AI in its seat, asks nothing and says Click "
              "to travel; the best is seated once a page, and not at all when nothing is rated; a page that asks "
              "for two tracks opens on them at once with head to head open, once a game, unless the window is "
              "narrow or the seat is played in the Single player view.")
        check_autosearch(bar, f"http://127.0.0.1:{port}/")
        print("Autosearch is a size larger than the tools beside it; its panel sets the four lists a line is "
              "judged by (Need, Must avoid, Want, Don't want), how far ahead and how many cores; the climb is the "
              "engine's, and a search plays on copies of the engine, leaves the "
              "page's game alone, finds the best line and lets its copies go; the line is played on the "
              "one timeline or in either half of a split, and says what it came to beside what was found.")
        check_doctor_strange(bar, f"http://127.0.0.1:{port}/")
        print("Doctor Strange's CIVVIS is the split shown as worlds in boxes under the tree of their timelines: it "
              "opens on the top rated player strategy on the right and its challenger on the left; the open turn is "
              "the line across the window, which is the divide between the tree and the boxes and has the turn "
              "written on it; a branch is named by the player strategies that played it, its box is in its colour, "
              "and Branch is a picker that makes one in a step, a third world in a third box, from the base timeline picked over it; the pointer over a box "
              "picks out its line, the divide is dragged, and a box's exit deletes its branch, line and window; "
              "locked, the worlds go through the turns together, never seen a turn apart, and Play Time and Pause "
              "Time start and stop them, and unlocked they go by themselves until locked again; what the page costs "
              "is read and shown; each world's future is played ahead of the open turn and kept, every world asked "
              "at once, what was seen coming is kept with it; past those turns each world's future is played to the "
              "end of its game on a copy, every world at once, on the world itself or on an engine of its own, "
              "and the sign over the far end of each branch says the nuclear war that is coming, in how many turns "
              "and in which era, or that there is none, to the end or as far as has been played, with the next war "
              "under it, and is played out again for a world given another player strategy; "
              "time running on is shown the turns held and plays "
              "none again, each world asked for one turn more for each turn it is shown and time never held for "
              "four, with nothing measured that is out of sight; "
              "Generate Black Swan, at the top of the sky, strikes the same event nothing saw coming on every locked "
              "world and on the followed one alone unlocked, any drawn by a seed or the one picked, says what struck, "
              "marks it in the line's name and has each struck world's future played again; a narrow "
              "window has the one timeline and says why.")
        check_doctor_strange_autosearch(bar, f"http://127.0.0.1:{port}/")
        print("Doctor Strange's CIVVIS has Autosearch on its deck under Branch, a size larger than the deck's other "
              "buttons; it opens the AI Debugger's panel over the tree and searches from the base timeline on copies "
              "of that world's own engine, asking no world anything; the path found is made a branch, a copy of "
              "the base that plays it to what the search read, the other worlds locked to it play on to the same "
              "turn, and no world's future is begun while a search runs.")
        check_doctor_strange_all(bar, f"http://127.0.0.1:{port}/")
        check_doctor_strange_world_ends(bar, f"http://127.0.0.1:{port}/")
        print("Doctor Strange's CIVVIS goes on when one world's game is over: that world stays on the turn it "
              "ended on with a card in its box (who won, how, the turn, the score), View map puts the card away "
              "behind a chip that brings it back, Close deletes the world while time runs on for the rest, and "
              "time stops only when the last world's game is over.")
        print("Doctor Strange's CIVVIS has All 14,000,605 at the foot of the tree on the left: it branches the base "
              "timeline once for each of the top ten player strategies that has no world of its own there, each a "
              "world in a window of its own beside the base, best first, and time runs on for every world together; "
              "pressed while time runs it stops time, branches from that turn as many as there is room for, says "
              "how many did not fit, and runs time on again.")
        check_edge_of_tomorrow(bar, f"http://127.0.0.1:{port}/")
        print("Edge of Tomorrow is the played game on the Civilization VI screen with a bar of its own across the "
              "foot of the window, the map area ending above it, the left panel's Auto-play in it (Turns and Civ AI "
              "over Jump back, Auto-play and Wake here) and Start new game left in the left panel, no minimap or map "
              "controls, the standings, victory tracker and research box asked for, and the seat's only decisions "
              "Auto-play's; Jump back asks the engine for the turn woken on "
              "and begins the next loop, with where each loop ended and what it saw done to the seat kept on the "
              "line; Wake here keeps the open turn as the engine's save point and a jump goes there, and moves "
              "back to the first turn when asked on it; a lost game, the seat fallen or somebody else the winner, "
              "has the loop's card in place of the offer of another game, counts down to the jump and jumps, or "
              "is looked around first; a won game keeps its own card and says which loop won it; another game "
              "is loop 1 again, and a narrow window has the line under the buttons.")
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
