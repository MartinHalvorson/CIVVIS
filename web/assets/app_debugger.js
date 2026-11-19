"use strict";
// The simulator debugger's bar.
//
// A played game keeps a mark of every turn its seat has opened (the engine's
// `server/debugger.rs`), and every state it sends says where the world sits
// on that timeline (`state.debug`). This script is the hand on it: a bar
// fixed across the top of the window with the whole game laid out on it as
// one long timeline. A turn that was played is gone back to as it stood, a
// turn still ahead is played to by the seat's Civ AI, and the tools above the
// timeline step, undo, run to a breakpoint, keep save points, choose which
// layers of the turn the agent decides, and split the history in two.
//
// It is a classic script like the others and loads after `app.js`, whose
// `render`, `fetchJSON` and turn loop it uses. Nothing in `app.js` knows it
// is here: the bar redraws by standing behind `render`, the one door every
// state comes through.

const DEBUG_BAR_STYLE = `
  /* The bar is fixed across the very top of the window, the whole width of
     it, and the page stands beneath: the height the bar takes is measured and
     published as --dbg-top, and the body is padded by that much. */
  html.dbg-docked body { padding-top: var(--dbg-top, 0px); }
  /* What the page fixes to the window at a distance from its top moves down
     by the same. */
  html.dbg-docked #moments, html.dbg-docked #site-pick { margin-top: var(--dbg-top, 0px); }
  @media (max-width: 720px) { html.dbg-docked #side { margin-top: var(--dbg-top, 0px); } }
  #debugbar {
    position: fixed; z-index: 8000; inset: 0 0 auto 0;
    display: grid; grid-template-columns: auto minmax(0, 1fr); gap: 5px 10px; padding: 5px 8px 6px;
    color: var(--text); font-size: 12px; line-height: 1.2; user-select: none;
    border-bottom: 1px solid #4d685f; background: #0a1512; box-shadow: 0 6px 18px #0008;
  }
  /* Hidden is hidden, whatever a later rule says the thing is laid out as. */
  #debugbar[hidden], #debugbar [hidden] { display: none !important; }
  /* While the screen is split the bar stays up, over the two halves. */
  html.dbg-split-open #debugbar { z-index: 100001; }
  #debugbar.dbg-tight { grid-template-columns: minmax(0, 1fr); }
  #debugbar .dbg-tools { display: flex; flex-wrap: wrap; align-items: center; gap: 3px; min-width: 0; }
  #debugbar button {
    display: inline-flex; align-items: center; justify-content: center; gap: 4px;
    min-width: 26px; height: 26px; padding: 0 6px; border-radius: 8px; cursor: pointer; white-space: nowrap;
    color: var(--text); background: #14231f; border: 1px solid #35504a; font: inherit; line-height: 1;
  }
  #debugbar button i { font-style: normal; }
  #debugbar button:hover:not(:disabled) { border-color: var(--gold-bright, #e7c873); }
  #debugbar button:disabled { opacity: .36; cursor: default; }
  #debugbar button.on { color: #10140f; background: var(--gold-bright, #e7c873); border-color: transparent; font-weight: 700; }
  /* A window too narrow for the words keeps the signs. */
  #debugbar.dbg-tight .dbg-word, #debugbar.dbg-tight .dbg-played { display: none; }
  #debugbar .dbg-sep { width: 1px; height: 18px; background: #35504a; margin: 0 3px; }
  #debugbar .dbg-count { color: var(--gold-bright, #e7c873); font-weight: 700; }
  #debugbar .dbg-count:empty { display: none; }
  #debugbar button.on .dbg-count { color: inherit; }
  #debugbar .dbg-dot { color: #e2574c; }
  #debugbar .dbg-turnbox { display: inline-flex; align-items: center; gap: 5px; margin: 0 3px; color: var(--muted); white-space: nowrap; }
  #debugbar #dbg-turn {
    width: 40px; height: 24px; padding: 0 2px; border-radius: 7px; text-align: center;
    color: var(--text); background: #060d0b; border: 1px solid #35504a; font: inherit; font-weight: 700;
  }
  #debugbar #dbg-turn:disabled { opacity: .5; }
  #debugbar .dbg-played { margin-left: 8px; color: var(--muted); white-space: nowrap; }
  /* What the bar has to say: at the end of the tools where there is room,
     and under the bar where there is not. */
  #debugbar .dbg-note { align-self: center; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: right; }
  #debugbar .dbg-note:empty { display: none; }
  #debugbar .dbg-note button { height: 20px; margin-left: 7px; vertical-align: middle; }
  #debugbar.dbg-tight .dbg-note {
    position: absolute; right: 8px; top: calc(100% + 7px); max-width: min(560px, calc(100vw - 16px));
    padding: 5px 11px; border-radius: 999px; white-space: normal; text-align: center;
    border: 1px solid #4d685f; background: #0a1512f2; box-shadow: var(--shadow);
  }

  /* The timeline: a rail as long as the game, one for the world, or one for
     each half of a split. */
  #debugbar .dbg-time { grid-column: 1 / -1; display: grid; gap: 3px; min-width: 0; }
  #debugbar .dbg-lane { display: flex; align-items: center; gap: 6px; min-width: 0; --dbg-own: var(--gold-bright, #e7c873); }
  #debugbar .dbg-lane[data-dbg-lane="a"] { --dbg-own: #ff8f6b; }
  #debugbar .dbg-lane[data-dbg-lane="b"] { --dbg-own: #6fd3e0; }
  #debugbar .dbg-lane-name { flex: none; width: 12px; text-align: center; font-weight: 700; color: var(--dbg-own); }
  #debugbar .dbg-lane-name:empty { display: none; }
  #debugbar .dbg-rail {
    position: relative; flex: 1; min-width: 0; height: 20px; border-radius: 7px;
    background: #060d0b; border: 1px solid #2b403a;
  }
  #debugbar .dbg-rail:focus-within { border-color: var(--gold-bright, #e7c873); }
  /* A slider's thumb stops half its own width short of each end, so what is
     drawn on a rail is drawn in the thumb's travel and not the rail's width. */
  #debugbar .dbg-deco, #debugbar .dbg-marks { position: absolute; inset: 0 7px; pointer-events: none; }
  #debugbar .dbg-marks { z-index: 2; }
  #debugbar .dbg-rail input {
    position: absolute; z-index: 1; inset: 0; width: 100%; height: 100%; margin: 0; padding: 0;
    opacity: 0; cursor: pointer; -webkit-appearance: none; appearance: none; background: transparent;
  }
  #debugbar .dbg-rail input::-webkit-slider-thumb { -webkit-appearance: none; width: 14px; height: 18px; }
  #debugbar .dbg-rail input::-moz-range-thumb { width: 14px; height: 18px; border: 0; }
  #debugbar .dbg-rail input:disabled { pointer-events: none; }
  #debugbar .dbg-ruler i { position: absolute; top: 0; bottom: 0; width: 1px; background: #ffffff12; }
  #debugbar .dbg-ruler i.major { background: #ffffff30; }
  #debugbar .dbg-ruler b {
    position: absolute; top: 50%; transform: translateY(-50%); margin-left: 4px;
    font-size: 9px; font-weight: 400; color: #9fb3ab; font-variant-numeric: tabular-nums;
  }
  #debugbar .dbg-ruler b.end { transform: translate(-100%, -50%); margin-left: -4px; }
  #debugbar .dbg-band { position: absolute; top: 4px; bottom: 4px; min-width: 3px; border-radius: 3px; background: #8a7335; }
  #debugbar .dbg-band.dbg-own { background: var(--dbg-own); opacity: .8; }
  /* When turns were thinned to make room, the band dims and the turns that
     can still be gone back to stand out on it. */
  #debugbar .dbg-thinned .dbg-band { background: #4a4024; }
  #debugbar .dbg-kept-turns i { position: absolute; top: 4px; bottom: 4px; width: 2px; margin-left: -1px; background: #c9ad5c; }
  /* Where a run is headed. */
  #debugbar .dbg-goal {
    position: absolute; top: 4px; bottom: 4px; border-right: 2px solid var(--dbg-own);
    background: repeating-linear-gradient(-45deg, #e7c87340 0 4px, transparent 4px 8px);
  }
  #debugbar .dbg-ghost { position: absolute; top: 0; bottom: 0; width: 1px; background: #ffffffa0; }
  #debugbar .dbg-cursor { position: absolute; top: 0; bottom: 0; width: 0; }
  #debugbar .dbg-cursor b {
    position: absolute; left: 0; top: 50%; transform: translate(-50%, -50%); min-width: 26px; padding: 2px 5px;
    border-radius: 999px; text-align: center; font-size: 11px; font-weight: 700; font-variant-numeric: tabular-nums;
    color: #10140f; background: var(--dbg-own); box-shadow: 0 0 0 2px #060d0b, 0 2px 6px #000a;
  }
  /* Save points stand on a rail as flags and turn breakpoints as red dots,
     each at its own turn. */
  #debugbar .dbg-marks i {
    position: absolute; top: -4px; width: 9px; height: 9px; margin-left: -5px; cursor: pointer; pointer-events: auto;
    border-radius: 2px 2px 50% 50%; border: 1px solid #10140f; background: var(--gold-bright, #e7c873);
  }
  #debugbar .dbg-marks i.auto { background: #8fb4a8; }
  #debugbar .dbg-marks i.stop { top: auto; bottom: -4px; border-radius: 50%; background: #e2574c; }
  /* What letting go of the timeline here would do, said under the pointer. */
  #debugbar .dbg-hover {
    position: absolute; z-index: 4; top: calc(100% + 5px); transform: translateX(-50%); padding: 4px 10px;
    border-radius: 999px; white-space: nowrap; pointer-events: none;
    border: 1px solid #4d685f; background: #0a1512f5; box-shadow: var(--shadow);
  }
  /* The host of a split shows the two halves' lanes and nothing else: each
     half carries the tools for its own world, and no timeline. */
  #debugbar.dbg-split-on .dbg-tools, #debugbar.dbg-split-on .dbg-note, #debugbar.dbg-split-on .dbg-pop { display: none; }
  #debugbar.dbg-pane .dbg-time { display: none; }

  #debugbar .dbg-pop {
    position: absolute; z-index: 3; left: 8px; top: calc(100% + 6px);
    width: min(360px, calc(100vw - 16px)); max-height: calc(100vh - 90px); overflow-y: auto;
    display: grid; gap: 8px; padding: 12px; white-space: normal;
    border-radius: 14px; border: 1px solid #4d685f; background: #0a1512fa; box-shadow: var(--shadow);
  }
  #debugbar .dbg-pop h4 { margin: 0; font: 600 14px Georgia, serif; color: var(--gold-bright, #e7c873); }
  #debugbar .dbg-pop p { margin: 0; color: var(--muted); line-height: 1.35; }
  #debugbar .dbg-pop label { display: flex; align-items: center; gap: 7px; cursor: pointer; }
  #debugbar .dbg-pop .dbg-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 10px; }
  #debugbar .dbg-pop input[type=text] {
    flex: 1; min-width: 0; height: 26px; padding: 0 7px; border-radius: 7px;
    color: var(--text); background: #060d0b; border: 1px solid #35504a; font: inherit;
  }
  #debugbar .dbg-pop select {
    min-width: 0; max-width: 104px; height: 26px; border-radius: 7px;
    color: var(--text); background: #060d0b; border: 1px solid #35504a; font: inherit;
  }
  #debugbar .dbg-pop .dbg-line { display: flex; align-items: center; gap: 6px; }
  #debugbar .dbg-pop .dbg-line > span { flex: none; width: 58px; color: var(--muted); }
`;

// The bar, once built, and each lane of its timeline: what is drawn on it
// (`debugLaneModels`) and the elements it is drawn with (`debugLanes`). The
// lanes are "" for the world, and "a" and "b" for the halves of a split.
let debugBar = null;
const debugLanes = {}, debugLaneModels = {};
// A slider's thumb is 14px wide; its centre stops this far short of each end.
const DEBUG_THUMB_HALF = 7;
// The narrowest bar that still has room for the words on its tools.
const DEBUG_ROOMY_WIDTH = 1280;
// One request at a time: every debugger operation replaces the world.
let debugBusy = false;
// A run is a loop of single steps; the bar's Run button is also its Stop.
// A run to a turn has a goal, which the timeline shows.
let debugRunning = false, debugStopAsked = false, debugGoal = null;
let debugPopOpen = null;
let debugNoteText = "", debugNotePos = null;
// The lane whose slider is being dragged, or null. A dragged slider is left
// where the hand has it until the hand lets go.
let debugScrubbing = null, debugSliderHeld = false;
// Which half of a split this page is: "a-…" or "b-…" inside the host's frame,
// and null for an ordinary page.
const DEBUG_PANE = (() => {
  try {
    const pane = new URLSearchParams(location.search).get("pane");
    return pane && window.parent !== window ? pane : null;
  } catch (_) { return null; }
})();
// What is moving this pane's world right now, so its report to the host can
// say how the turn was reached: walked to, or played after the other pane.
let debugTravelling = false, debugFollowing = false;

function debugTimeline() {
  return state && !SPEC && state.debug ? state.debug : null;
}
function debugSay(text, pos = null) {
  debugNoteText = text || "";
  debugNotePos = pos;
  drawDebugBar();
}
// Standing orders the page keeps for the person — a unit walking somewhere, a
// city's queue — act by themselves when a turn opens. A turn that was walked
// to is shown as it stood, so they are dropped rather than re-issued into it.
function debugForgetStandingOrders(turn) {
  unitOrders.clear();
  cityQueues.clear();
  saveCityQueues();
  noticeLog = [];
  noticeDismissed = new Set();
  noticeSeenTurn = Math.max(0, turn - 1);
}
const DEBUG_TRAVEL_OPS = new Set(["back", "forward", "jump", "restart", "undo", "copy", "goto_save"]);
async function debugOp(body, timeoutMs = 20000) {
  if (debugBusy || turnAdvancePending || !debugTimeline()) return null;
  debugBusy = true;
  drawDebugBar();
  try {
    const before = state.turn;
    debugTravelling = DEBUG_TRAVEL_OPS.has(body.op);
    const next = await fetchJSON("/debug", {
      method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify(body),
    }, timeoutMs);
    if (next.error) { debugSay(next.error); return next; }
    if (DEBUG_TRAVEL_OPS.has(body.op) || body.op === "step" || body.op === "step_action" ||
        body.op === "step_seat" || body.op === "finish_round") {
      sel = null; selCity = null;
      if ((next.turn !== before || body.op === "copy" || body.op === "goto_save") && DEBUG_TRAVEL_OPS.has(body.op))
        debugForgetStandingOrders(next.turn);
    }
    render(next);
    return next;
  } catch (error) {
    debugSay(`The debugger could not ${body.op}: ${error.message || error}`);
    return null;
  } finally {
    debugBusy = false;
    debugTravelling = false;
    drawDebugBar();
  }
}
function debugStrategy() {
  return (document.getElementById("autoplaystrategy") || {}).value || undefined;
}
// Why the engine stopped a run, in a line, with somewhere to look if it has one.
function debugSayStop(stopped) {
  if (!stopped) return;
  if (stopped.kind === "turn") debugSay(`Stopped at turn ${stopped.turn}`);
  else if (stopped.kind === "event")
    debugSay(`Stopped on ${stopped.category}: ${stopped.text}`, stopped.pos || null);
  else if (stopped.kind === "condition") {
    const who = state.players.find(player => player.id === stopped.seat) || {};
    const name = titleCase(String(who.civ || who.name || `seat ${stopped.seat}`));
    const read = value => typeof value === "number" ? String(Math.round(value * 100) / 100) : JSON.stringify(value);
    const what = `${name} ${stopped.key.replaceAll("_", " ")}`;
    debugSay(stopped.test === "changes"
      ? `Stopped on turn ${stopped.turn}: ${what} changed from ${read(stopped.was)} to ${read(stopped.now)}`
      : `Stopped on turn ${stopped.turn}: ${what} is ${read(stopped.now)} (${stopped.test} ${read(stopped.value)}, was ${read(stopped.was)})`);
  }
  else debugSay(`The game ended on turn ${stopped.turn}`);
}
async function debugStep() {
  if (autoplaying) return null;
  debugSay("");
  autoplaying = true;
  try {
    const next = await debugOp({op: "step", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS);
    if (next && !next.error) debugSayStop((next.debug || {}).stopped);
    return next;
  } finally {
    autoplaying = false;
    drawDebugBar();
    drawTurnLoop();
  }
}
// One action of the agent's turn in a few words, and where on the map it was.
function debugActionWords(action) {
  const words = [titleCase(String(action.type || ""))];
  const unit = action.unit === undefined ? null : state.units.find(held => held.id === action.unit);
  if (unit) words.push(titleCase(String(unit.type || "")));
  const city = action.city === undefined ? null : state.cities.find(held => held.id === action.city);
  if (city) words.push(city.name);
  // What was chosen is a name, sometimes one level down (`item: {unit: …}`).
  const named = value => typeof value === "string" ? [value]
    : value && typeof value === "object" && !Array.isArray(value)
      ? Object.values(value).filter(inner => typeof inner === "string") : [];
  for (const [key, value] of Object.entries(action))
    if (key !== "type") words.push(...named(value).map(titleCase));
  return words.filter(Boolean).join(" · ");
}
function debugActionPlace(action) {
  for (const key of ["to", "target", "pos"])
    if (Array.isArray(action[key]) && action[key].length === 2) return action[key];
  const unit = action.unit === undefined ? null : state.units.find(held => held.id === action.unit);
  if (unit) return unit.pos;
  const city = action.city === undefined ? null : state.cities.find(held => held.id === action.city);
  return city ? city.pos : null;
}
// Step in: one action of the agent's turn, named in the bar and centred on
// the map, so a turn can be watched being put together.
async function debugStepAction() {
  if (autoplaying) return null;
  debugSay("");
  autoplaying = true;
  try {
    const next = await debugOp({op: "step_action", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS);
    const stepped = next && !next.error ? (next.debug || {}).stepped : null;
    if (stepped && stepped.seat !== undefined) debugSayRound(stepped);
    else if (stepped) {
      const place = debugActionPlace(stepped.action);
      const left = stepped.left ? ` · ${stepped.left} more this turn` : "";
      debugSay(`Civ AI: ${debugActionWords(stepped.action)}${left}`, place);
      if (place) centerOn(place);
    }
    return next;
  } finally {
    autoplaying = false;
    drawDebugBar();
    drawTurnLoop();
  }
}
// Step one seat: the round, a civilization at a time. On your own turn it
// ends the turn as it stands and stops before any rival has moved; after
// that each step plays whoever is up. It is not the Civ AI playing your
// seat, so a linked half follows it as an ended turn, not as a step.
function debugSeatName(seat) {
  const who = state.players.find(player => player.id === seat);
  return who ? titleCase(String(who.civ || who.name || `seat ${seat}`)) : `seat ${seat}`;
}
// Who is up, as a clause: every seat is one civilization but the Barbarians.
function debugPlaysNext(seat) {
  const who = state.players.find(player => player.id === seat);
  return `${debugSeatName(seat)} ${who && who.is_barbarian ? "play" : "plays"} next`;
}
function debugSayRound(stepped) {
  const did = stepped.seat === 0 ? "Your turn ended"
    : `${debugSeatName(stepped.seat)} played ${stepped.actions} action${stepped.actions === 1 ? "" : "s"}`;
  debugSay(stepped.next === 0 ? `${did} · turn ${state.turn} is yours`
    : `${did} · ${debugPlaysNext(stepped.next)}`);
}
async function debugStepSeat() {
  if (autoplaying) return null;
  debugSay("");
  const next = await debugOp({op: "step_seat"}, AUTOPLAY_BATCH_TIMEOUT_MS);
  const stepped = next && !next.error ? (next.debug || {}).stepped : null;
  if (stepped && stepped.seat !== undefined) debugSayRound(stepped);
  return next;
}
// Played a turn a request, like auto-play, so every turn reaches the glass
// and the Stop button is never more than one turn away.
async function debugRun(until = null) {
  if (debugRunning) { debugStopAsked = true; drawDebugBar(); return; }
  if (autoplaying || !debugTimeline()) return;
  debugRunning = true; debugStopAsked = false; autoplaying = true; debugGoal = until;
  debugSay("");
  let played = 0;
  try {
    while (!debugStopAsked) {
      const next = await debugOp({op: "step", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS);
      if (!next || next.error) break;
      played += 1;
      const stopped = (next.debug || {}).stopped;
      if (stopped) { debugSayStop(stopped); break; }
      if (until !== null && next.turn >= until) { debugSay(`Played to turn ${next.turn}`); break; }
      await new Promise(drawn => requestAnimationFrame(drawn));
    }
    if (debugStopAsked) debugSay(`Stopped by hand after ${played} turn${played === 1 ? "" : "s"}`);
  } finally {
    debugRunning = false; debugStopAsked = false; autoplaying = false; debugGoal = null;
    drawDebugBar();
    drawTurnLoop();
    if (DEBUG_PANE !== null) window.parent.postMessage({civvis: "pane-ran", pane: DEBUG_PANE}, location.origin);
  }
}
// Go to a turn, like a debugger's "run to here": a turn that was played is
// put back as it stood, and a turn still ahead is played to.
async function debugGoToTurn(text) {
  const timeline = debugTimeline(), turn = parseInt(text, 10);
  if (!timeline || !Number.isFinite(turn) || turn === state.turn) { drawDebugBar(false); return; }
  const first = timeline.first ?? state.turn, last = Math.max(timeline.last ?? state.turn, state.turn);
  if (turn > last) { debugRun(turn); return; }
  const next = await debugOp({op: "jump", turn: Math.max(turn, first)});
  if (next && !next.error && next.turn !== turn && turn >= first)
    debugSay(`Turn ${turn} is no longer kept. This is turn ${next.turn}, the nearest before it`);
}
// A save point is the world as it stands, kept beside the timeline with the
// turns behind it (`save_point`). Unlike a save it is gone with the page.
async function debugSavePoint() {
  const box = document.querySelector("#dbg-pop-saves [data-dbg-field=pointname]");
  const next = await debugOp({op: "save_point", name: (box.value || "").trim()});
  if (!next || next.error) return;
  box.value = "";
  debugSay(`Save point set on turn ${next.turn}`);
}
// A phone closes a tab long before a desktop does, so the engine is told to
// keep fewer turns there (`room`). Asked again if a new game forgot it.
const DEBUG_SMALL_DEVICE = (navigator.deviceMemory || 8) <= 4 || /iPhone|iPad|Android/.test(navigator.userAgent);
let debugRoomAskedAt = 0;
// A save is the world at one turn, kept under a name. The list is this
// server's or this browser's own (`/saves`), the same one the deck shows.
async function debugSave() {
  const box = document.querySelector("#dbg-pop-saves [data-dbg-field=savename]");
  const stamp = new Date();
  const name = (box.value || "").trim() ||
    `t${state.turn}-${String(stamp.getHours()).padStart(2, "0")}${String(stamp.getMinutes()).padStart(2, "0")}`;
  try {
    const done = await fetchJSON("/save", {method: "POST",
      headers: {"Content-Type": "application/json"}, body: JSON.stringify({name})});
    if (done.error) throw new Error(done.error);
    box.value = "";
    debugSay(`Saved turn ${state.turn} as ${name}`);
    refreshSaves();
    debugDrawSaves();
  } catch (error) {
    debugSay(`Could not save: ${error.message || error}`);
  }
}
async function debugDrawSaves() {
  const list = document.getElementById("dbg-save-list");
  let saves = [];
  try { saves = (await fetchJSON("/saves", {}, 4000)).saves || []; } catch (_) {}
  list.innerHTML = saves.length
    ? saves.slice(0, 10).map(save =>
        `<div class="dbg-line"><span style="flex:1;width:auto;color:inherit"><b>${escapeAttr(save.name)}</b> · turn ${escapeAttr(String(save.turn))}</span>` +
        `<button data-dbg-load="${escapeAttr(save.name)}">Load</button></div>`).join("")
    : `<p>Nothing saved yet.</p>`;
}

function debugBreakpointsFromForm() {
  const pop = document.getElementById("dbg-pop-breaks");
  const list = text => text.split(",").map(part => part.trim()).filter(Boolean);
  return {
    op: "breakpoints",
    turns: [...new Set(list(pop.querySelector("[data-dbg-field=turns]").value)
      .map(Number).filter(turn => Number.isInteger(turn) && turn > 0))],
    categories: [...pop.querySelectorAll("[data-dbg-category]")]
      .filter(box => box.checked).map(box => box.dataset.dbgCategory),
    phrases: list(pop.querySelector("[data-dbg-field=phrases]").value),
    important: pop.querySelector("[data-dbg-field=important]").checked,
    every_seat: pop.querySelector("[data-dbg-field=every_seat]").checked,
    // Conditions are added and removed one at a time; the engine holds the set.
    conditions: [...(debugTimeline().breakpoints.conditions || [])],
  };
}
function debugLayersFromForm() {
  return [...document.querySelectorAll("#dbg-pop-layers [data-dbg-layer]")]
    .filter(box => box.checked).map(box => box.dataset.dbgLayer);
}
function debugTogglePop(name) {
  debugPopOpen = debugPopOpen === name ? null : name;
  drawDebugBar();
}

// One lane of the timeline: a rail as long as the game, what is drawn on it,
// and the slider that is the hand on it.
function debugLaneMarkup(key, id, label) {
  return `
      <div class="dbg-lane" data-dbg-lane="${key}">
        <span class="dbg-lane-name">${key.toUpperCase()}</span>
        <div class="dbg-rail">
          <div class="dbg-deco"><div class="dbg-ruler"></div><div class="dbg-band"></div><div class="dbg-band dbg-own"></div><div class="dbg-kept-turns"></div><div class="dbg-goal"></div><div class="dbg-ghost" hidden></div><div class="dbg-cursor"><b></b></div></div>
          <input type="range" id="${id}" step="1"${key ? ` data-dbg-half="${key}"` : ""} aria-label="${label}">
          <div class="dbg-marks"></div>
        </div>
      </div>`;
}
function debugBuildBar() {
  const style = document.createElement("style");
  style.textContent = DEBUG_BAR_STYLE;
  document.head.appendChild(style);
  const bar = document.createElement("div");
  bar.id = "debugbar";
  bar.hidden = true;
  bar.setAttribute("role", "toolbar");
  bar.setAttribute("aria-label", "Simulator debugger");
  bar.innerHTML = `
    <div class="dbg-tools">
      <button data-dbg="first" title="Go to where the timeline begins" aria-label="Go to the first turn">⏮</button>
      <button data-dbg="back" title="Back: the top of this turn, then the turn before" aria-label="Back one turn"><i>◀</i><span class="dbg-word">Back</span></button>
      <button data-dbg="undo" title="Undo your last action this turn (Ctrl+Z)" aria-label="Undo last action"><i>↶</i><span class="dbg-word">Undo</span></button>
      <label class="dbg-turnbox" title="Go to a turn: one already played is put back as it stood, one still ahead is played to by the Civ AI. Up and down walk one played turn"><span class="dbg-word">Turn</span>
        <input type="text" id="dbg-turn" inputmode="numeric" maxlength="4" aria-label="Go to turn"><span id="dbg-limit"></span></label>
      <button data-dbg="forward" title="Forward along the turns already played" aria-label="Forward one played turn"><i>▶</i><span class="dbg-word">Forward</span></button>
      <button data-dbg="last" title="Go to the last turn that was played" aria-label="Go to the last turn played">⏭</button>
      <span class="dbg-sep"></span>
      <button data-dbg="step-action" title="Step in: the Civ AI takes its next action this turn, and ends the turn when it has none left" aria-label="Step one action"><i>▸</i><span class="dbg-word">Action</span></button>
      <button data-dbg="step-seat" title="Step one seat: end your turn as it stands, then one civilization's turn at a time" aria-label="Step one seat"><i>▸|</i><span class="dbg-word">Seat</span></button>
      <button data-dbg="step" title="Step: the Civ AI plays one turn. In the middle of a round, the rest of the round plays" aria-label="Step one turn"><i>▶|</i><span class="dbg-word">Turn</span></button>
      <button data-dbg="run" title="Run: the Civ AI plays until a breakpoint stops it" aria-label="Run to a breakpoint"><i id="dbg-run-sign">▶▶</i><span class="dbg-word" id="dbg-run-word">Run</span></button>
      <span class="dbg-sep"></span>
      <button data-dbg-pop="layers" title="Layers: which decisions the Civ AI takes for you each turn">Layers<span class="dbg-count" id="dbg-layer-count"></span></button>
      <button data-dbg-pop="breaks" title="Breakpoints: what stops a run" aria-label="Breakpoints"><span class="dbg-dot">●</span><span class="dbg-word">Breakpoints</span><span class="dbg-count" id="dbg-break-count"></span></button>
      <button data-dbg="reveal" title="Reveal: show the whole map and every civilization, not only what yours can see" aria-label="Reveal the whole map"><i>◉</i><span class="dbg-word">Reveal</span></button>
      <button data-dbg-pop="saves" title="Save points and saves: keep this world to come back to, or load one you kept" aria-label="Save points and saves"><i>⚑</i><span class="dbg-word">Save points</span><span class="dbg-count" id="dbg-point-count"></span></button>
      <button data-dbg-pop="split" title="Split: two simulators from this turn, side by side or stacked, each with a Civ AI of its own if you like">Split</button>
      <span class="dbg-played" id="dbg-played"></span>
    </div>
    <div class="dbg-note" id="dbg-note" role="status"></div>
    <div class="dbg-time">${debugLaneMarkup("", "dbg-slider", "Turn, on the whole game")}${
      debugLaneMarkup("a", "dbg-slider-a", "Turn of simulator A")}${
      debugLaneMarkup("b", "dbg-slider-b", "Turn of simulator B")}
    </div>
    <div class="dbg-hover" id="dbg-hover" hidden></div>
    <div class="dbg-pop" id="dbg-pop-layers" hidden>
      <h4>Who decides</h4>
      <p>A layer that is on is played by the Civ AI at the top of each of your turns.
        Everything else, and ending the turn, stays with you.</p>
      <div id="dbg-layer-list" style="display:grid;gap:4px"></div>
      <div class="dbg-line">
        <button data-dbg="layers-none" title="Every decision by hand">By hand</button>
        <button data-dbg="layers-all" title="The agent takes every decision but the end of the turn">Everything</button>
        <button data-dbg="assist" title="Have the agent take the layers that are on in the turn that is open now">Apply to this turn</button>
      </div>
    </div>
    <div class="dbg-pop" id="dbg-pop-split" hidden>
      <h4>Split the screen</h4>
      <p>Two simulators start from this turn, each with the turns behind it. From here each plays its
        own history. Linked, they stay on the same turn; separate, each keeps its own.</p>
      <div class="dbg-line">
        <button data-dbg="split-row" title="A on the left, B on the right">◧ Side by side</button>
        <button data-dbg="split-col" title="A above, B below">⬒ Top and bottom</button>
      </div>
      <p>Head to head seats a Civ AI of its own in each half and sets their numbers side by side,
        to play two strategies out from this turn.</p>
      <div class="dbg-line">
        <button data-dbg="split-versus" title="Side by side, with a Civ AI to pick for each half and their numbers compared">Head to head</button>
      </div>
    </div>
    <div class="dbg-pop" id="dbg-pop-saves" hidden>
      <h4>Save points</h4>
      <p>A save point keeps this world to come back to, with the turns behind it, however the game
        is played on from here. Going to one keeps the world you left as a save point too. Save points
        last as long as this page is open; a save, below, is kept on this device.</p>
      <div class="dbg-line">
        <input type="text" data-dbg-field="pointname" maxlength="48" placeholder="a name, or leave empty for the turn" aria-label="Save point name">
        <button data-dbg="point" title="Keep the world as it stands now">Save point</button>
      </div>
      <div id="dbg-point-list" style="display:grid;gap:4px"></div>
      <h4>Saves</h4>
      <p>A save is the world at one turn, kept on this device. Loading one starts a timeline of its own from there.</p>
      <div class="dbg-line">
        <input type="text" data-dbg-field="savename" maxlength="64" placeholder="a name, or leave empty for the turn" aria-label="Save name">
        <button data-dbg="save" title="Save the turn that is open now">Save this turn</button>
      </div>
      <div id="dbg-save-list" style="display:grid;gap:4px"></div>
    </div>
    <div class="dbg-pop" id="dbg-pop-breaks" hidden>
      <h4>Breakpoints</h4>
      <p>Run plays turns with the Civ AI until one of these stops it.</p>
      <div class="dbg-line"><span>Turns</span>
        <input type="text" data-dbg-field="turns" inputmode="numeric" placeholder="50, 75" aria-label="Stop when these turns open">
        <button data-dbg="break-plus" title="Add the turn ten from now">+10</button></div>
      <div class="dbg-line"><span>Phrases</span>
        <input type="text" data-dbg-field="phrases" placeholder="declared war, captured" aria-label="Stop when the event log says"></div>
      <div class="dbg-line" style="flex-wrap:wrap" title="Stop when this becomes true of a civilization">
        <span>When</span>
        <select data-dbg-field="cond-seat" aria-label="Civilization"></select>
        <select data-dbg-field="cond-key" aria-label="What to read"></select>
        <select data-dbg-field="cond-test" aria-label="How to hold it"></select>
        <input type="text" data-dbg-field="cond-value" inputmode="decimal" placeholder="value" style="flex:0 0 52px" aria-label="Value">
        <button data-dbg="cond-add" title="Stop a run on the turn this becomes true">Add</button>
      </div>
      <div id="dbg-condition-list" style="display:grid;gap:3px"></div>
      <div class="dbg-grid" id="dbg-category-list"></div>
      <label><input type="checkbox" data-dbg-field="important"> Any important event</label>
      <label><input type="checkbox" data-dbg-field="every_seat"> Every civilization's events, not only mine</label>
      <div class="dbg-line"><button data-dbg="break-clear">Clear all</button></div>
    </div>`;
  document.body.appendChild(bar);
  debugBar = bar;
  for (const box of bar.querySelectorAll(".dbg-lane")) {
    const part = name => box.querySelector(`.dbg-${name}`);
    debugLanes[box.dataset.dbgLane] = {box, input: box.querySelector("input"), ruler: part("ruler"),
      band: part("band"), own: part("own"), kept: part("kept-turns"), goal: part("goal"), ghost: part("ghost"),
      cursor: part("cursor"), pill: box.querySelector(".dbg-cursor b"), marks: part("marks")};
  }

  bar.addEventListener("click", event => {
    const pop = event.target.closest("[data-dbg-pop]");
    if (pop) {
      debugTogglePop(pop.dataset.dbgPop);
      if (debugPopOpen === "saves") debugDrawSaves();
      return;
    }
    const gone = event.target.closest("[data-dbg-cond-remove]");
    if (gone && debugTimeline()) {
      const request = debugBreakpointsFromForm();
      request.conditions.splice(+gone.dataset.dbgCondRemove, 1);
      debugOp(request);
      return;
    }
    const named = event.target.closest("[data-dbg-turn]");
    if (named && debugTimeline()) { debugSay(""); debugGoToTurn(named.dataset.dbgTurn); return; }
    const point = event.target.closest("[data-dbg-goto]");
    if (point && debugTimeline()) { debugSay(""); debugOp({op: "goto_save", id: +point.dataset.dbgGoto}); return; }
    const dropped = event.target.closest("[data-dbg-drop]");
    if (dropped && debugTimeline()) { debugOp({op: "drop_save", id: +dropped.dataset.dbgDrop}); return; }
    const load = event.target.closest("[data-dbg-load]");
    if (load) { debugPopOpen = null; loadSave(load.dataset.dbgLoad); return; }
    const button = event.target.closest("[data-dbg]");
    if (!button || button.disabled) return;
    const timeline = debugTimeline();
    if (!timeline) return;
    const action = button.dataset.dbg;
    if (action === "back") debugOp({op: "back"});
    else if (action === "undo") debugOp({op: "undo"});
    else if (action === "forward") debugOp({op: "forward"});
    else if (action === "first") { debugSay(""); debugGoToTurn(timeline.first ?? state.turn); }
    else if (action === "last") { debugSay(""); debugGoToTurn(Math.max(timeline.last ?? state.turn, state.turn)); }
    else if (action === "step") debugStep();
    else if (action === "step-action") debugStepAction();
    else if (action === "step-seat") debugStepSeat();
    else if (action === "reveal") debugOp({op: "reveal", on: !timeline.reveal});
    else if (action === "run") debugRun();
    else if (action === "save") debugSave();
    else if (action === "point") debugSavePoint();
    else if (action === "split-row" || action === "split-col" || action === "split-versus") {
      debugPopOpen = null;
      debugSplitOpen(action === "split-col" ? "col" : "row", action === "split-versus" ? true : null);
    }
    else if (action === "show") { if (debugNotePos) centerOn(debugNotePos); }
    else if (action === "assist") debugOp({op: "assist", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS);
    else if (action === "layers-none" || action === "layers-all") {
      debugOp({op: "layers", layers: action === "layers-all" ? timeline.layers.map(layer => layer.id) : []});
    } else if (action === "break-clear") {
      debugOp({op: "breakpoints"});
    } else if (action === "cond-add") {
      const pop = document.getElementById("dbg-pop-breaks");
      const field = name => pop.querySelector(`[data-dbg-field=cond-${name}]`);
      const request = debugBreakpointsFromForm();
      const test = field("test").value;
      request.conditions.push({seat: +field("seat").value, key: field("key").value, test,
        value: test === "changes" ? undefined : parseFloat(field("value").value)});
      debugOp(request);
    } else if (action === "break-plus") {
      const request = debugBreakpointsFromForm();
      request.turns = [...new Set([...request.turns, state.turn + 10])];
      debugOp(request);
    }
  });
  bar.addEventListener("change", event => {
    const target = event.target;
    if (target.id === "dbg-slider" || target.dataset.dbgHalf) {
      // Read first: once nothing is being dragged, drawing the bar puts every
      // slider back on the turn its world is on.
      const turn = target.value;
      debugScrubbing = null;
      // A slider let go of by the pointer gives the keys back to the page.
      if (debugSliderHeld) target.blur();
      debugSliderHeld = false;
      debugSay("");
      if (target.dataset.dbgHalf) debugSplitGoTo(target.dataset.dbgHalf, turn);
      else debugGoToTurn(turn);
    } else if (target.id === "dbg-turn") {
      target.blur();
      debugGoToTurn(target.value);
    } else if (target.dataset.dbgLayer) {
      debugOp({op: "layers", layers: debugLayersFromForm()});
    } else if (target.closest("#dbg-pop-breaks")) {
      // The fields of a condition being composed are not breakpoints yet.
      if ((target.dataset.dbgField || "").startsWith("cond-")) { drawDebugBar(false); return; }
      debugOp(debugBreakpointsFromForm());
    }
  });
  bar.addEventListener("input", event => {
    const target = event.target;
    if (target.type !== "range") return;
    debugScrubbing = target.dataset.dbgHalf || "";
    const model = debugLaneModels[debugScrubbing];
    debugSay(model ? debugTurnWords(model, +target.value) : "");
  });
  // The map must not read a click on the bar as a click on a tile, and the
  // page's keys must not fire while a breakpoint is being typed.
  for (const kind of ["pointerdown", "mousedown", "wheel", "dblclick", "contextmenu"])
    bar.addEventListener(kind, event => event.stopPropagation());
  bar.addEventListener("pointerdown", event => {
    if (event.target.type === "range") debugSliderHeld = true;
  });
  // A slider put back where it was picked up says nothing when it is let go
  // of: it is not left looking dragged.
  window.addEventListener("pointerup", () => setTimeout(() => {
    if (debugScrubbing === null) return;
    const lane = debugLanes[debugScrubbing], model = debugLaneModels[debugScrubbing];
    // Anywhere else, letting go is a `change`, and that is what reads it.
    if (model && +lane.input.value !== model.turn) return;
    debugScrubbing = null;
    debugSliderHeld = false;
    debugSay("");
  }, 0));
  // What a place on the timeline is, and what letting go there would do,
  // said under the pointer.
  bar.addEventListener("pointermove", event => {
    const on = event.target instanceof Element ? event.target : null;
    debugHover(on && !on.closest(".dbg-marks i") ? on.closest(".dbg-rail") : null, event.clientX);
  });
  bar.addEventListener("pointerleave", () => debugHover(null));
  // The wheel over the timeline, and the up and down arrows in the turn box,
  // walk the played turns one at a time.
  const walk = forward => {
    const timeline = debugTimeline();
    if (!timeline || debugBusy || autoplaying || turnAdvancePending) return;
    if (!(forward ? timeline.can_forward : timeline.can_back)) return;
    debugSay("");
    debugOp({op: forward ? "forward" : "back"});
  };
  bar.addEventListener("wheel", event => {
    if (!event.target.closest('[data-dbg-lane=""], #dbg-turn')) return;
    event.preventDefault();
    const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
    if (delta) walk(delta > 0);
  }, {passive: false});
  bar.addEventListener("keydown", event => {
    if (event.key === "Escape" && debugPopOpen) {
      event.stopPropagation();
      debugPopOpen = null;
      if (event.target instanceof HTMLElement) event.target.blur();
      drawDebugBar();
      return;
    }
    // A slider's own keys move it a turn at a time; the page does not hear them.
    if (event.target.matches("input[type=range]")) { event.stopPropagation(); return; }
    if (!event.target.matches("input[type=text]")) return;
    event.stopPropagation();
    if (event.target.id === "dbg-turn" && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      walk(event.key === "ArrowUp");
    }
  });
  return bar;
}

// The whole game, end to end: a timeline is as long as the game may be, 250
// turns when the game does not say, and longer when it is played on past that.
function debugAxis(first, last) {
  const limit = [state.turn_limit, state.max_turns].find(turns => Number.isFinite(turns) && turns > 0) || 250;
  const open = Number.isFinite(state.turn_limit) && state.turn_limit > 0 ? 0 : 1;
  return {lo: Math.min(1, first), hi: Math.max(limit, Math.ceil((last + open) / 50) * 50)};
}
// How far along a lane a turn stands, in hundredths of a percent of it.
function debugAlong(turn, lo, hi) {
  return hi > lo ? Math.round(Math.max(0, Math.min(1, (turn - lo) / (hi - lo))) * 10000) / 100 : 0;
}
// The ruler under a lane: a tick every few turns and a number on the round ones.
function debugRulerMarkup(lo, hi) {
  const length = hi - lo;
  const major = length <= 60 ? 10 : length <= 150 ? 25 : length <= 300 ? 50 : length <= 600 ? 100 : 250;
  const minor = major / 5;
  let ruler = `<b style="left:0%">${lo}</b>`;
  for (let turn = Math.ceil((lo + 1) / minor) * minor; turn <= hi; turn += minor) {
    const at = debugAlong(turn, lo, hi), round = turn % major === 0;
    ruler += `<i${round ? ' class="major"' : ""} style="left:${at}%"></i>`;
    if (round) ruler += `<b${turn === hi ? ' class="end"' : ""} style="left:${at}%">${turn}</b>`;
  }
  return ruler;
}
// Drawn only when it changes: a list rebuilt under the pointer loses the click.
function debugDrawn(box, html) {
  if (box.dataset.drawn !== html) { box.dataset.drawn = html; box.innerHTML = html; }
}
function debugPlace(box, css) {
  if (box.dataset.placed !== css) { box.dataset.placed = css; box.style.cssText = css; }
}
// A place on a lane in a few words: what it is, and what letting go of the
// slider there does.
function debugTurnWords(model, turn) {
  const name = model.name ? `${model.name} · turn ${turn}` : `Turn ${turn}`;
  if (turn > model.last)
    return model.turn < model.last ? `${name} · play on from turn ${model.turn} to here` : `${name} · play to here`;
  if (turn < model.first) return `${name} · before the timeline begins, goes to turn ${model.first}`;
  if (model.kept && !model.kept.has(turn)) {
    const near = model.turns.filter(kept => kept <= turn).pop() ?? model.first;
    return `${name} · no longer kept, goes to turn ${near}`;
  }
  const flags = (model.flags || []).filter(point => point.turn === turn).map(point => ` · ⚑ ${point.name}`).join("");
  return `${name}${turn === model.turn ? " · open now" : ""}${flags}`;
}
// One lane, drawn from what it stands for: the turns the game may run to
// (`lo` to `hi`), the turns played (`first` to `last`), the turn that is open,
// and, past `fork`, the part of its history that is one half's own.
function debugDrawLane(key, model) {
  const lane = debugLanes[key];
  debugLaneModels[key] = model;
  lane.box.hidden = !model;
  if (!model) return;
  const {lo, hi, first, last} = model;
  const at = turn => debugAlong(turn, lo, hi);
  const span = (from, to) => `left:${at(from)}%;width:${Math.max(0, at(to) - at(from))}%`;
  lane.input.min = lo; lane.input.max = hi;
  lane.input.disabled = !model.idle;
  if (debugScrubbing !== key) lane.input.value = model.turn;
  const shown = debugScrubbing === key ? +lane.input.value : model.turn;
  lane.input.setAttribute("aria-valuetext", `Turn ${shown} of ${hi}`);
  if (lane.ruler.dataset.axis !== `${lo}:${hi}`) {
    lane.ruler.dataset.axis = `${lo}:${hi}`;
    lane.ruler.innerHTML = debugRulerMarkup(lo, hi);
  }
  const fork = model.fork === null ? last : Math.max(first, Math.min(last, model.fork));
  debugPlace(lane.band, span(first, fork));
  lane.own.hidden = fork >= last;
  debugPlace(lane.own, span(fork, last));
  lane.box.classList.toggle("dbg-thinned", !!model.kept);
  const kept = model.kept ? `${lo}:${hi}:${model.turns.join()}` : "";
  if (lane.kept.dataset.turns !== kept) {
    lane.kept.dataset.turns = kept;
    lane.kept.innerHTML = model.kept ? model.turns.map(turn => `<i style="left:${at(turn)}%"></i>`).join("") : "";
  }
  const goal = model.goal !== null && model.goal > model.turn ? model.goal : null;
  lane.goal.hidden = goal === null;
  if (goal !== null) debugPlace(lane.goal, span(model.turn, goal));
  debugPlace(lane.cursor, `left:${at(shown)}%`);
  if (lane.pill.textContent !== String(shown)) lane.pill.textContent = shown;
  debugDrawn(lane.marks,
    (model.stops || []).filter(turn => turn >= lo && turn <= hi).map(turn =>
      `<i class="stop" data-dbg-turn="${turn}" style="left:${at(turn)}%" title="Breakpoint on turn ${turn}. Click to ${
        turn > last ? "play to it" : "go there"}"></i>`).join("") +
    (model.flags || []).filter(point => point.turn >= lo && point.turn <= hi).map(point =>
      `<i${point.auto ? ' class="auto"' : ""} data-dbg-goto="${point.id}" style="left:${at(point.turn)}%"` +
      ` title="${escapeAttr(point.name)}: turn ${point.turn}. Click to go there"></i>`).join(""));
}
function debugHover(rail, x = 0) {
  const tag = document.getElementById("dbg-hover");
  if (!tag) return;
  for (const lane of Object.values(debugLanes)) lane.ghost.hidden = true;
  const key = rail ? rail.closest(".dbg-lane").dataset.dbgLane : null;
  const model = key === null ? null : debugLaneModels[key];
  if (!model) { tag.hidden = true; return; }
  const lane = debugLanes[key], box = lane.input.getBoundingClientRect();
  const along = (x - box.left - DEBUG_THUMB_HALF) / Math.max(1, box.width - 2 * DEBUG_THUMB_HALF);
  const turn = Math.round(model.lo + Math.max(0, Math.min(1, along)) * (model.hi - model.lo));
  lane.ghost.hidden = false;
  debugPlace(lane.ghost, `left:${debugAlong(turn, model.lo, model.hi)}%`);
  tag.textContent = debugTurnWords(model, turn);
  tag.hidden = false;
  const bar = debugBar.getBoundingClientRect(), half = tag.offsetWidth / 2;
  tag.style.left = `${Math.max(half + 4, Math.min(bar.width - half - 4, x - bar.left))}px`;
}
// The page stands under the bar. How tall the bar is depends on how its tools
// wrap, so it is measured, and the page is told when that changes: the map is
// fitted to the room it has.
let debugDocked = 0;
function debugDock(bar) {
  const height = bar.hidden ? 0 : Math.ceil(bar.getBoundingClientRect().height);
  if (height === debugDocked) return;
  debugDocked = height;
  document.documentElement.classList.toggle("dbg-docked", height > 0);
  document.documentElement.style.setProperty("--dbg-top", `${height}px`);
  window.dispatchEvent(new Event("resize"));
}

function drawDebugBar(measure = true) {
  const bar = debugBar || debugBuildBar();
  const timeline = debugTimeline();
  bar.hidden = !timeline;
  if (!timeline) { debugDock(bar); return; }
  bar.classList.toggle("dbg-split-on", debugSplit !== null);
  bar.classList.toggle("dbg-pane", DEBUG_PANE !== null);
  document.documentElement.classList.toggle("dbg-split-open", debugSplit !== null);
  if (debugSplit) {
    // The host of a split: its own world waits under the two halves, and the
    // bar shows each half's history as a lane, the same colour up to the
    // turn they split on and each half's own from there.
    const panes = debugSplit.panes;
    const played = letter => Math.max(panes[letter].last ?? 0, panes[letter].turn ?? 0, debugSplit.from ?? state.turn);
    const axis = debugAxis(1, Math.max(played("a"), played("b")));
    debugDrawLane("", null);
    for (const letter of ["a", "b"]) {
      const pane = panes[letter], turn = pane.turn ?? debugSplit.from ?? state.turn;
      debugDrawLane(letter, {name: letter.toUpperCase(), ...axis, first: Math.min(pane.first ?? turn, turn),
        last: played(letter), turn, fork: debugSplit.from, kept: null, turns: null, flags: [], stops: [],
        goal: debugSplit.running > 0 ? debugSplit.until : null,
        idle: pane.turn !== null && !debugSplit.busy && debugSplit.running === 0});
    }
    if (measure) debugDock(bar);
    return;
  }
  const idle = !debugBusy && !autoplaying && !turnAdvancePending;
  const set = (action, enabled) => {
    const button = bar.querySelector(`[data-dbg="${action}"]`);
    if (button) button.disabled = !enabled;
  };
  const first = timeline.first ?? state.turn, last = Math.max(timeline.last ?? state.turn, state.turn);
  set("back", idle && timeline.can_back);
  set("undo", idle && timeline.can_undo);
  set("forward", idle && timeline.can_forward);
  set("first", idle && state.turn > first);
  set("last", idle && state.turn < last);
  set("step", idle);
  set("step-action", idle);
  set("step-seat", idle);
  set("reveal", idle);
  bar.querySelector('[data-dbg="reveal"]').classList.toggle("on", timeline.reveal === true);
  set("save", idle);
  set("point", idle);
  set("assist", idle);
  const run = bar.querySelector('[data-dbg="run"]');
  run.disabled = !(debugRunning || idle);
  document.getElementById("dbg-run-sign").textContent = debugRunning ? (debugStopAsked ? "…" : "⏹") : "▶▶";
  document.getElementById("dbg-run-word").textContent = debugRunning ? "Stop" : "Run";
  run.classList.toggle("on", debugRunning);

  const turnBox = document.getElementById("dbg-turn");
  turnBox.disabled = !idle;
  // A number being typed is left alone, unless the turn moved under it.
  if (document.activeElement !== turnBox || turnBox.dataset.turn !== String(state.turn)) turnBox.value = state.turn;
  turnBox.dataset.turn = String(state.turn);
  const limit = Number.isFinite(state.turn_limit) && state.turn_limit > 0 ? state.turn_limit : null;
  document.getElementById("dbg-limit").textContent = limit === null ? "" : `of ${limit}`;
  // The timeline: the whole game, the turns played on it, the save points and
  // the breakpoint turns, and where a run is headed.
  const points = [...(timeline.saves || [])].sort((a, b) => a.turn - b.turn || a.id - b.id);
  const breaks = timeline.breakpoints;
  const kept = timeline.turns || [], thinned = kept.length < last - first + 1;
  debugDrawLane("a", null);
  debugDrawLane("b", null);
  debugDrawLane("", {name: "", ...debugAxis(first, last), first, last, turn: state.turn, fork: null,
    kept: thinned ? new Set(kept) : null, turns: thinned ? kept : null, flags: points, stops: breaks.turns,
    goal: debugRunning ? debugGoal : null, idle});
  const played = document.getElementById("dbg-played");
  played.textContent = first === last ? `Turn ${first} is kept`
    : `Turns ${first} to ${last} played, ${thinned ? `${kept.length} kept` : "every one kept"}`;
  played.title = thinned
    ? `${kept.length} of the ${last - first + 1} turns played can be gone back to; a turn that is no longer kept answers with the nearest before it.`
    : "The start of every turn is kept: any of them can be gone back to as it stood.";
  document.getElementById("dbg-point-count").textContent = points.length ? `${points.length}` : "";
  debugDrawn(document.getElementById("dbg-point-list"), points.length ? points.map(point =>
    `<div class="dbg-line"><span style="flex:1;width:auto;color:inherit">${point.auto ? "" : "<b>"}${escapeAttr(point.name)}${point.auto ? "" : "</b>"}` +
    ` · turn ${point.turn}${point.acted ? `, ${point.acted} action${point.acted === 1 ? "" : "s"} in` : ""}</span>` +
    `<button data-dbg-goto="${point.id}" title="Put this world back on the table">Go</button>` +
    `<button data-dbg-drop="${point.id}" title="Remove this save point" aria-label="Remove this save point">×</button></div>`).join("")
    : `<p>No save points yet.</p>`);
  if (DEBUG_SMALL_DEVICE && idle && timeline.memory_line == null && Date.now() - debugRoomAskedAt > 10000) {
    debugRoomAskedAt = Date.now();
    debugOp({op: "room", megabytes: 384});
  }

  const layersOn = timeline.layers.filter(layer => layer.on).length;
  document.getElementById("dbg-layer-count").textContent = layersOn ? `${layersOn}` : "";
  const conditions = breaks.conditions || [];
  const breakCount = breaks.turns.length + breaks.categories.length + breaks.phrases.length +
    conditions.length + (breaks.important ? 1 : 0);
  document.getElementById("dbg-break-count").textContent = breakCount ? `${breakCount}` : "";
  // A pane is already one half of a split, and does not split again.
  bar.querySelector('[data-dbg-pop="split"]').hidden = DEBUG_PANE !== null;
  for (const name of ["layers", "breaks", "saves", "split"]) {
    const pop = document.getElementById(`dbg-pop-${name}`), button = bar.querySelector(`[data-dbg-pop="${name}"]`);
    pop.hidden = debugPopOpen !== name;
    button.classList.toggle("on", debugPopOpen === name);
    if (pop.hidden) continue;
    // A panel opens under its own button, and stays inside the window.
    const from = button.getBoundingClientRect(), box = bar.getBoundingClientRect();
    pop.style.left = `${Math.max(8, Math.min(from.left - box.left, box.width - pop.offsetWidth - 8))}px`;
    pop.style.top = `${from.bottom - box.top + 6}px`;
  }

  // The lists are the engine's: a layer or an event category added there
  // appears here without this file being told.
  const layerList = document.getElementById("dbg-layer-list");
  if (layerList.childElementCount !== timeline.layers.length) {
    layerList.innerHTML = timeline.layers.map(layer =>
      `<label><input type="checkbox" data-dbg-layer="${escapeAttr(layer.id)}"> ${escapeAttr(layer.label)}</label>`).join("");
  }
  for (const layer of timeline.layers) {
    const box = layerList.querySelector(`[data-dbg-layer="${layer.id}"]`);
    if (box) box.checked = layer.on;
  }
  const categoryList = document.getElementById("dbg-category-list");
  const known = breaks.known_categories || [];
  if (categoryList.childElementCount !== known.length) {
    categoryList.innerHTML = known.map(category =>
      `<label><input type="checkbox" data-dbg-category="${escapeAttr(category)}"> ${escapeAttr(category)}</label>`).join("");
  }
  for (const box of categoryList.querySelectorAll("[data-dbg-category]"))
    box.checked = breaks.categories.includes(box.dataset.dbgCategory);
  const pop = document.getElementById("dbg-pop-breaks");
  const field = name => pop.querySelector(`[data-dbg-field=${name}]`);
  // A condition reads one thing the observation says about one civilization.
  // The readings on offer are the ones this seat's own entry has; the engine
  // reads the same entry for any seat, and refuses one it cannot find.
  const majors = state.players.filter(player => !player.is_minor && !player.is_barbarian);
  const civName = player => titleCase(String(player.civ || player.name || `seat ${player.id}`));
  const mine = state.players.find(player => player.id === 0) || {};
  const readable = Object.keys(mine).filter(key => {
    const value = mine[key];
    return typeof value === "number" || typeof value === "boolean" || Array.isArray(value);
  }).sort();
  const offer = (name, options) => {
    const select = field(`cond-${name}`), drawn = JSON.stringify(options);
    if (select.dataset.drawn === drawn) return;
    const held = select.value;
    select.dataset.drawn = drawn;
    select.innerHTML = options.map(([value, label]) =>
      `<option value="${escapeAttr(String(value))}">${escapeAttr(label)}</option>`).join("");
    if (options.some(([value]) => String(value) === held)) select.value = held;
  };
  offer("seat", majors.map(player => [player.id, civName(player)]));
  offer("key", readable.map(key => [key, key.replaceAll("_", " ")]));
  offer("test", (breaks.known_tests || []).map(test => [test, test]));
  field("cond-value").hidden = field("cond-test").value === "changes";
  const conditionList = document.getElementById("dbg-condition-list");
  const conditionRows = conditions.map((condition, index) => {
    const who = state.players.find(player => player.id === condition.seat);
    const line = condition.test === "changes" ? "changes" : `${condition.test} ${condition.value}`;
    return `<div class="dbg-line"><span style="flex:1;width:auto;color:inherit">` +
      `${escapeAttr(who ? civName(who) : `seat ${condition.seat}`)} · ${escapeAttr(condition.key.replaceAll("_", " "))} ${escapeAttr(line)}</span>` +
      `<button data-dbg-cond-remove="${index}" title="Remove this condition" aria-label="Remove this condition">×</button></div>`;
  }).join("");
  if (conditionList.innerHTML !== conditionRows) conditionList.innerHTML = conditionRows;
  if (document.activeElement !== field("turns")) field("turns").value = breaks.turns.join(", ");
  if (document.activeElement !== field("phrases")) field("phrases").value = breaks.phrases.join(", ");
  field("important").checked = breaks.important;
  field("every_seat").checked = breaks.every_seat;

  const note = document.getElementById("dbg-note");
  const layered = timeline.assisted
    ? `The Civ AI took ${timeline.assisted} decision${timeline.assisted === 1 ? "" : "s"} in its layers this turn` : "";
  // A round being stepped a seat at a time is not the person's turn, and the
  // bar says whose it is for as long as nothing newer is being said.
  const round = timeline.playing
    ? `${debugPlaysNext(timeline.playing)} · ▸| steps it, ▶| lets the round finish` : "";
  const text = debugNoteText || round || layered;
  const html = escapeAttr(text) +
    (debugNoteText && debugNotePos ? `<button data-dbg="show" title="Centre the map there">Show</button>` : "");
  if (note.innerHTML !== html) { note.innerHTML = html; note.title = text; }
  if (!measure) return;
  // The words on the tools are for a window with room for them.
  bar.classList.toggle("dbg-tight", bar.clientWidth < DEBUG_ROOMY_WIDTH);
  debugDock(bar);
}

// ------------------------------------------------------------------ split
//
// Two simulators in one window. The page that was playing becomes the host:
// it covers itself with two frames, and each frame is this same page again,
// playing a session of its own that the engine forks from the host's
// (`src/server/panes.rs`, named by `?pane=`). Both halves start on the host's
// world as it stood when the screen was split, with every turn behind it
// still there to walk back to, and from there each plays its own history.
//
// Linked, the two stay on the same turn: whatever moves one — ending the
// turn, a step, a walk along the timeline — is repeated in the other the same
// way. Separate, neither knows about the other. The view can be linked too,
// so that moving the map in one half moves it in the other.
//
// Host and panes talk by `postMessage` on their own origin:
//   pane → host  pane-turn    the turn it is on, and how it got there
//   host → pane  pane-follow  go to this turn the same way
//   host → pane  pane-copy    become a copy of the other half
//   pane → host  pane-camera  where its map is looking
//   host → pane  pane-look    look there too
// Closing the split is the host's own request to the engine: the half that
// is kept becomes the host's session (`op: "adopt"`), timeline and all.
//
// The host's bar stays up across the top of the window while the screen is
// split, and its timeline becomes two lanes, one for each half's history.
// A lane is the hand on its half, and on both halves while they are linked.

const DEBUG_SPLIT_STYLE = `
  #debugsplit {
    position: fixed; inset: var(--dbg-top, 0px) 0 0 0; z-index: 100000; display: grid; background: #050b0a;
    grid-template-columns: minmax(0, var(--dbg-a, 1fr)) auto minmax(0, var(--dbg-b, 1fr));
    grid-template-rows: minmax(0, 1fr);
  }
  #debugsplit.dbg-col {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(0, var(--dbg-a, 1fr)) auto minmax(0, var(--dbg-b, 1fr));
  }
  #debugsplit iframe { width: 100%; height: 100%; border: 0; display: block; background: #07110f; }
  #debugsplit.dbg-dragging iframe { pointer-events: none; }
  #debugsplit .dbg-divider {
    display: flex; flex-direction: column; align-items: stretch; justify-content: center; gap: 6px;
    width: 68px; padding: 6px 5px; background: #0a1512; border-inline: 1px solid #35504a;
    color: var(--text, #e8e3d2); font: 11px/1.2 system-ui, sans-serif; user-select: none;
    cursor: col-resize; touch-action: none;
  }
  #debugsplit.dbg-col .dbg-divider {
    flex-direction: row; align-items: center; width: auto; height: 36px; padding: 4px 8px;
    border-inline: 0; border-block: 1px solid #35504a; cursor: row-resize;
  }
  #debugsplit .dbg-divider button {
    min-height: 26px; padding: 3px 5px; border-radius: 7px; cursor: pointer; font: inherit;
    color: inherit; background: #14231f; border: 1px solid #35504a; white-space: normal;
  }
  #debugsplit.dbg-col .dbg-divider button { white-space: nowrap; padding: 3px 9px; }
  #debugsplit .dbg-divider button:hover:not(:disabled) { border-color: #e7c873; }
  #debugsplit .dbg-divider button:disabled { opacity: .4; cursor: default; }
  #debugsplit .dbg-divider button.on { color: #10140f; background: #e7c873; border-color: transparent; font-weight: 700; }
  #debugsplit .dbg-pane-name { text-align: center; font-weight: 700; color: #e7c873; font-variant-numeric: tabular-nums; }
  #debugsplit .dbg-gap { flex: 1; }
  #debugsplit [data-dbg-split="flip"] { font-size: 15px; }
  /* Head to head: the strip widens to hold each half's Civ AI and numbers. */
  #debugsplit.dbg-versus-on:not(.dbg-col) .dbg-divider { width: 138px; }
  #debugsplit .dbg-versus { display: flex; flex-direction: column; gap: 5px; cursor: default; min-width: 0; }
  #debugsplit .dbg-versus[hidden] { display: none; }
  #debugsplit.dbg-col .dbg-versus { flex-direction: row; align-items: center; gap: 10px; overflow: hidden; }
  #debugsplit .dbg-versus select, #debugsplit .dbg-versus input {
    min-width: 0; height: 24px; padding: 0 4px; border-radius: 7px; font: inherit; line-height: normal;
    color: inherit; background: #060d0b; border: 1px solid #35504a;
  }
  #debugsplit .dbg-versus select { flex: 1; max-width: 132px; }
  #debugsplit .dbg-versus input { width: 44px; text-align: center; }
  #debugsplit .dbg-vs-line { display: flex; align-items: center; gap: 4px; min-width: 0; }
  #debugsplit .dbg-vs-line > span { font-weight: 700; }
  #debugsplit .dbg-vs-a { color: #ff8f6b; }
  #debugsplit .dbg-vs-b { color: #6fd3e0; }
  #debugsplit .dbg-vs-stat { cursor: pointer; }
  #debugsplit .dbg-vs-stat.charted i { color: inherit; text-decoration: underline; }
  #debugsplit .dbg-vs-chart { width: 100%; height: 48px; border-radius: 6px; background: #060d0b; border: 1px solid #22332e; }
  #debugsplit.dbg-col .dbg-vs-chart { display: none; }
  #debugsplit .dbg-vs-table { display: grid; gap: 3px; font-variant-numeric: tabular-nums; }
  #debugsplit.dbg-col .dbg-vs-table { display: flex; gap: 12px; white-space: nowrap; }
  #debugsplit .dbg-vs-stat { display: grid; grid-template-columns: 1fr 1fr; column-gap: 6px; }
  #debugsplit.dbg-col .dbg-vs-stat { display: inline-flex; gap: 5px; }
  #debugsplit .dbg-vs-stat i { grid-column: 1 / -1; font-style: normal; color: #9fb3ab; }
  #debugsplit .dbg-vs-stat b { font-weight: 400; text-align: center; }
  #debugsplit .dbg-vs-stat b.lead { font-weight: 700; color: #e7c873; }
`;
// The host's record of the split, or null when the screen is whole.
let debugSplit = null;
// Head to head: what each half says about its seat, set side by side. The
// yields are what the seat makes a turn.
const DEBUG_VERSUS_ROWS = [
  ["score", "Score", "Score"], ["cities", "Cities", "Cities"], ["population", "Population", "Population"],
  ["science", "Science", "Science per turn"], ["culture", "Culture", "Culture per turn"],
  ["gold", "Gold", "Gold per turn"], ["faith", "Faith", "Faith per turn"],
  ["military", "Military", "Military strength"],
];
// The two halves' lines in the chart, and the letters that name them.
const DEBUG_VERSUS_COLOURS = {a: "#ff8f6b", b: "#6fd3e0"};
// One of the numbers over the turns since the split, a line for each half.
// A turn a half has not reported is passed over, so a half that is behind
// simply has a shorter line.
function debugVersusChart(canvas) {
  const key = debugSplit.chart, cx = canvas.getContext("2d");
  cx.clearRect(0, 0, canvas.width, canvas.height);
  const turns = [...new Set([...debugSplit.history.a.keys(), ...debugSplit.history.b.keys()])].sort((x, y) => x - y);
  const values = [];
  for (const letter of ["a", "b"])
    for (const numbers of debugSplit.history[letter].values())
      if (typeof numbers[key] === "number") values.push(numbers[key]);
  if (turns.length < 2 || !values.length) return;
  const low = Math.min(...values), span = Math.max(...values) - low || 1;
  const first = turns[0], last = turns[turns.length - 1];
  const x = turn => 5 + (turn - first) / (last - first) * (canvas.width - 10);
  const y = value => canvas.height - 6 - (value - low) / span * (canvas.height - 12);
  cx.lineWidth = 3; cx.lineJoin = "round"; cx.lineCap = "round";
  for (const letter of ["a", "b"]) {
    cx.beginPath();
    let drawing = false;
    for (const turn of turns) {
      const numbers = debugSplit.history[letter].get(turn);
      if (!numbers || typeof numbers[key] !== "number") continue;
      if (drawing) cx.lineTo(x(turn), y(numbers[key])); else cx.moveTo(x(turn), y(numbers[key]));
      drawing = true;
    }
    cx.strokeStyle = DEBUG_VERSUS_COLOURS[letter];
    cx.stroke();
    // Where the half stands now, when it has walked back along its turns.
    const now = debugSplit.panes[letter].turn;
    if (now !== null && now > first && now < last) {
      cx.globalAlpha = .55;
      cx.fillStyle = DEBUG_VERSUS_COLOURS[letter];
      cx.fillRect(Math.round(x(now)) - 1, 0, 2, canvas.height);
      cx.globalAlpha = 1;
    }
  }
  const charted = DEBUG_VERSUS_ROWS.find(row => row[0] === key);
  canvas.title = `${charted ? charted[2] : key}, turn ${first} to ${last}. Click a number's name to chart it`;
}
function debugVersusNumber(value) {
  return typeof value === "number" && Number.isFinite(value) ? String(Math.round(value * 10) / 10) : "–";
}

function debugSplitTell(letter, message) {
  const pane = debugSplit ? debugSplit.panes[letter] : null;
  if (pane && pane.frame.contentWindow) pane.frame.contentWindow.postMessage(message, location.origin);
}
function drawDebugSplit() {
  if (!debugSplit) return;
  const host = debugSplit.host;
  host.classList.toggle("dbg-col", debugSplit.dir === "col");
  host.style.setProperty("--dbg-a", `${debugSplit.share}fr`);
  host.style.setProperty("--dbg-b", `${1 - debugSplit.share}fr`);
  for (const letter of ["a", "b"]) {
    const turn = debugSplit.panes[letter].turn;
    host.querySelector(`[data-dbg-name="${letter}"]`).textContent =
      `${letter.toUpperCase()}${turn === null ? "" : ` · ${turn}`}`;
  }
  const link = host.querySelector('[data-dbg-split="link"]');
  link.classList.toggle("on", debugSplit.linked);
  link.textContent = debugSplit.linked ? "Linked" : "Separate";
  link.title = debugSplit.linked
    ? "Linked: both simulators stay on the same turn. Click to let each keep its own."
    : "Separate: each simulator keeps its own turn. Click to keep them on the same turn.";
  const view = host.querySelector('[data-dbg-split="view"]');
  view.classList.toggle("on", debugSplit.view);
  view.title = debugSplit.view
    ? "The two maps move together. Click to let each look where it likes."
    : "Each map looks where it likes. Click to move the two together.";
  host.querySelector('[data-dbg-split="flip"]').textContent = debugSplit.dir === "col" ? "◧" : "⬒";
  for (const button of host.querySelectorAll("[data-dbg-split]")) button.disabled = debugSplit.busy;
  // The two lanes in the bar are drawn from the same record.
  drawDebugBar();
  host.classList.toggle("dbg-versus-on", debugSplit.versus);
  host.querySelector('[data-dbg-split="versus"]').classList.toggle("on", debugSplit.versus);
  const versus = host.querySelector("[data-dbg-versus]");
  versus.hidden = !debugSplit.versus;
  if (!debugSplit.versus) return;
  // The Civ AIs on offer are the ones this page's own picker lists.
  const offered = [...((document.getElementById("autoplaystrategy") || {}).options || [])]
    .map(option => `<option value="${escapeAttr(option.value)}">${escapeAttr(option.textContent)}</option>`).join("");
  for (const letter of ["a", "b"]) {
    const select = host.querySelector(`[data-dbg-strategy="${letter}"]`);
    if (select.dataset.drawn !== offered) { select.dataset.drawn = offered; select.innerHTML = offered; }
    const held = debugSplit.panes[letter].strategy;
    if (held !== undefined && document.activeElement !== select) select.value = held;
  }
  const a = debugSplit.panes.a.numbers || {}, b = debugSplit.panes.b.numbers || {};
  const table = DEBUG_VERSUS_ROWS.map(([key, label, title]) => {
    const lead = side => typeof a[key] === "number" && typeof b[key] === "number" &&
      (side === "a" ? a[key] > b[key] : b[key] > a[key]) ? ' class="lead"' : "";
    return `<span class="dbg-vs-stat${key === debugSplit.chart ? " charted" : ""}" data-dbg-stat="${key}" title="${title}: A, then B. Click to chart it"><i>${label}</i>` +
      `<b${lead("a")}>${debugVersusNumber(a[key])}</b><b${lead("b")}>${debugVersusNumber(b[key])}</b></span>`;
  }).join("");
  const numbers = host.querySelector("[data-dbg-numbers]");
  if (numbers.dataset.drawn !== table) { numbers.dataset.drawn = table; numbers.innerHTML = table; }
  debugVersusChart(host.querySelector("[data-dbg-chart]"));
  const reached = Math.max(debugSplit.panes.a.turn ?? 0, debugSplit.panes.b.turn ?? 0);
  host.querySelector("[data-dbg-until]").placeholder = String(reached + 25);
  const run = host.querySelector('[data-dbg-split="versus-run"]');
  run.textContent = debugSplit.running > 0 ? "⏹" : "▶▶";
  run.classList.toggle("on", debugSplit.running > 0);
  // The same start again, for another pair of Civ AIs.
  const back = host.querySelector('[data-dbg-split="versus-back"]');
  back.title = `Put both halves back on turn ${debugSplit.from}, where they split, to play it again with other Civ AIs`;
  back.disabled = debugSplit.busy || debugSplit.running > 0 || debugSplit.from === null ||
    ["a", "b"].every(letter => debugSplit.panes[letter].turn === debugSplit.from);
}
function debugSplitOpen(dir, versus = null) {
  if (debugSplit || DEBUG_PANE !== null || !debugTimeline() || debugBusy || autoplaying) return;
  if (!document.getElementById("debugsplit-style")) {
    const style = document.createElement("style");
    style.id = "debugsplit-style";
    style.textContent = DEBUG_SPLIT_STYLE;
    document.head.appendChild(style);
  }
  const nonce = Date.now().toString(36);
  const host = document.createElement("div");
  host.id = "debugsplit";
  host.innerHTML = `
    <iframe data-dbg-frame="a" title="Simulator A"></iframe>
    <div class="dbg-divider" role="toolbar" aria-label="Split simulators" title="Drag to resize; double-click to make the halves equal">
      <div class="dbg-pane-name" data-dbg-name="a"></div>
      <button data-dbg-split="copy-a" title="Make B a copy of A as it stands now, turns behind it included">A → B</button>
      <button data-dbg-split="keep-a" title="Close the split and keep playing A, with every turn it can walk back to">Keep A</button>
      <span class="dbg-gap"></span>
      <button data-dbg-split="link"></button>
      <button data-dbg-split="view">View</button>
      <button data-dbg-split="flip" title="Switch between side by side and top and bottom"></button>
      <button data-dbg-split="versus" title="Head to head: seat a Civ AI of its own in each half, play both and set their numbers side by side">VS</button>
      <div class="dbg-versus" data-dbg-versus hidden>
        <div class="dbg-vs-line"><span class="dbg-vs-a">A</span><select data-dbg-strategy="a" title="The Civ AI that plays A's seat when A is stepped or run" aria-label="Civ AI for A"></select></div>
        <div class="dbg-vs-line"><span class="dbg-vs-b">B</span><select data-dbg-strategy="b" title="The Civ AI that plays B's seat when B is stepped or run" aria-label="Civ AI for B"></select></div>
        <div class="dbg-vs-table" data-dbg-numbers></div>
        <canvas class="dbg-vs-chart" data-dbg-chart width="252" height="96"></canvas>
        <div class="dbg-vs-line" title="Play both halves, each with its own Civ AI, until this turn opens or a breakpoint stops one">
          <button data-dbg-split="versus-back">↺</button>
          <input type="text" data-dbg-until inputmode="numeric" maxlength="4" aria-label="Play both to turn">
          <button data-dbg-split="versus-run"></button>
        </div>
      </div>
      <span class="dbg-gap"></span>
      <button data-dbg-split="keep-b" title="Close the split and keep playing B, with every turn it can walk back to">Keep B</button>
      <button data-dbg-split="copy-b" title="Make A a copy of B as it stands now, turns behind it included">B → A</button>
      <div class="dbg-pane-name" data-dbg-name="b"></div>
    </div>
    <iframe data-dbg-frame="b" title="Simulator B"></iframe>`;
  let remembered = false;
  try { remembered = localStorage.getItem("civvis-debug-versus") === "1"; } catch (_) {}
  debugSplit = {dir, linked: true, view: true, share: .5, busy: false, panes: {}, host,
    versus: versus ?? remembered, running: 0, until: null, chart: "score", history: {a: new Map(), b: new Map()},
    from: state ? state.turn : null};
  for (const letter of ["a", "b"]) {
    const frame = host.querySelector(`[data-dbg-frame="${letter}"]`);
    debugSplit.panes[letter] = {frame, id: `${letter}-${nonce}`, turn: null, first: null, last: null};
  }
  host.addEventListener("click", event => {
    const stat = event.target.closest("[data-dbg-stat]");
    if (stat && debugSplit) { debugSplit.chart = stat.dataset.dbgStat; drawDebugSplit(); return; }
    const button = event.target.closest("[data-dbg-split]");
    if (!button || button.disabled || !debugSplit) return;
    const action = button.dataset.dbgSplit;
    if (action === "link") {
      debugSplit.linked = !debugSplit.linked;
      // Linking two simulators that have drifted brings B to A's turn.
      const a = debugSplit.panes.a.turn;
      if (debugSplit.linked && a !== null && a !== debugSplit.panes.b.turn)
        debugSplitTell("b", {civvis: "pane-follow", turn: a, how: "end"});
    } else if (action === "view") debugSplit.view = !debugSplit.view;
    else if (action === "flip") debugSplit.dir = debugSplit.dir === "col" ? "row" : "col";
    else if (action === "copy-a" || action === "copy-b") {
      // A copy is the other half's whole history, and so is its line in the chart.
      const [from, to] = action === "copy-a" ? ["a", "b"] : ["b", "a"];
      debugSplit.history[to] = new Map(debugSplit.history[from]);
      debugSplitTell(to, {civvis: "pane-copy", from: debugSplit.panes[from].id});
    }
    else if (action === "keep-a") debugSplitClose("a");
    else if (action === "keep-b") debugSplitClose("b");
    else if (action === "versus") {
      debugSplit.versus = !debugSplit.versus;
      try { localStorage.setItem("civvis-debug-versus", debugSplit.versus ? "1" : "0"); } catch (_) {}
    } else if (action === "versus-run") debugVersusRun();
    else if (action === "versus-back") {
      // Each half is told by name, so it makes no difference whether they are linked.
      for (const letter of ["a", "b"])
        if (debugSplit.panes[letter].turn !== debugSplit.from)
          debugSplitTell(letter, {civvis: "pane-follow", turn: debugSplit.from, how: "travel"});
    }
    drawDebugSplit();
  });
  host.addEventListener("change", event => {
    const letter = event.target.dataset.dbgStrategy;
    if (!letter || !debugSplit) return;
    debugSplit.panes[letter].strategy = event.target.value;
    debugSplitTell(letter, {civvis: "pane-strategy", name: event.target.value});
  });
  // The page's keys must not fire while a turn is being typed in the strip.
  host.addEventListener("keydown", event => {
    if (!event.target.matches("input, select")) return;
    event.stopPropagation();
    if (event.key === "Enter" && event.target.matches("[data-dbg-until]")) { debugVersusRun(); drawDebugSplit(); }
  });
  // The strip between the halves is also the handle that sizes them.
  const divider = host.querySelector(".dbg-divider");
  divider.addEventListener("pointerdown", event => {
    if (event.target.closest("button, select, input, [data-dbg-stat]") || !debugSplit) return;
    // Capture keeps the drag when the pointer runs over a frame; the frames
    // are also made deaf to the pointer for as long as it lasts.
    try { divider.setPointerCapture(event.pointerId); } catch (_) {}
    host.classList.add("dbg-dragging");
  });
  divider.addEventListener("pointermove", event => {
    if (!debugSplit || !host.classList.contains("dbg-dragging")) return;
    const box = host.getBoundingClientRect();
    const along = debugSplit.dir === "col"
      ? (event.clientY - box.top) / box.height : (event.clientX - box.left) / box.width;
    debugSplit.share = Math.max(.2, Math.min(.8, along));
    drawDebugSplit();
  });
  for (const kind of ["pointerup", "pointercancel"])
    divider.addEventListener(kind, () => host.classList.remove("dbg-dragging"));
  divider.addEventListener("dblclick", event => {
    if (event.target.closest("button, select, input, [data-dbg-stat]") || !debugSplit) return;
    debugSplit.share = .5;
    drawDebugSplit();
  });
  document.body.appendChild(host);
  drawDebugSplit();
  for (const letter of ["a", "b"]) {
    const pane = debugSplit.panes[letter];
    pane.frame.src = `${location.pathname}?pane=${pane.id}`;
  }
}
// Play halves to a turn, each with its own Civ AI. The turn is where the
// lanes show the run is headed until every half that was run says it is done.
function debugSplitRun(until, runners) {
  debugSplit.until = until;
  debugSplit.running = runners.length;
  for (const letter of runners) debugSplitTell(letter, {civvis: "pane-run", until});
}
// Play both halves to a turn. Linked, A is run and B follows it turn by
// turn; separate, each is run by itself. The same button stops them.
function debugVersusRun() {
  if (!debugSplit || debugSplit.busy) return;
  if (debugSplit.running > 0) {
    for (const letter of ["a", "b"]) debugSplitTell(letter, {civvis: "pane-stop"});
    debugSplit.running = 0;
    debugSplit.until = null;
    return;
  }
  const box = debugSplit.host.querySelector("[data-dbg-until]");
  const until = parseInt(box.value || box.placeholder, 10);
  const reached = Math.max(debugSplit.panes.a.turn ?? 0, debugSplit.panes.b.turn ?? 0);
  if (!Number.isFinite(until) || until <= reached) { box.value = ""; box.focus(); return; }
  debugSplitRun(until, debugSplit.linked ? ["a"] : ["a", "b"]);
}
// A lane of the split's timeline, let go of on a turn. Linked, both halves
// go; separate, only the half whose lane it is. A turn every one of them has
// played is walked to as it stood, and a turn ahead of any is played to.
function debugSplitGoTo(letter, text) {
  const turn = parseInt(text, 10);
  if (!debugSplit || debugSplit.busy || debugSplit.running > 0 || !Number.isFinite(turn)) { drawDebugSplit(); return; }
  const letters = debugSplit.linked ? ["a", "b"] : [letter];
  const played = name => Math.max(debugSplit.panes[name].last ?? 0, debugSplit.panes[name].turn ?? 0);
  if (letters.some(name => turn > played(name))) debugSplitRun(turn, debugSplit.linked ? ["a"] : [letter]);
  else for (const name of letters)
    if (debugSplit.panes[name].turn !== turn) debugSplitTell(name, {civvis: "pane-follow", turn, how: "travel"});
  drawDebugSplit();
}
// The screen is whole again, playing the half that was kept: its session
// takes the host's place in the engine, so nothing it could walk back to is lost.
async function debugSplitClose(keep) {
  if (!debugSplit || debugSplit.busy) return;
  debugSplit.busy = true; drawDebugSplit();
  let note = "";
  try {
    const next = await fetchJSON("/debug", {method: "POST",
      headers: {"Content-Type": "application/json"},
      body: JSON.stringify({op: "adopt", pane: debugSplit.panes[keep].id})}, 60000);
    if (next.error) throw new Error(next.error);
    debugForgetStandingOrders(next.turn);
    sel = null; selCity = null;
    render(next);
    note = `Playing on from simulator ${keep.toUpperCase()}`;
  } catch (error) {
    note = `Simulator ${keep.toUpperCase()} could not be kept: ${error.message || error}`;
  }
  debugSplit.host.remove();
  debugSplit = null;
  debugSay(note);
}
window.addEventListener("message", event => {
  if (!debugSplit || event.origin !== location.origin) return;
  const data = event.data;
  if (!data || typeof data.civvis !== "string") return;
  const letter = ["a", "b"].find(name => debugSplit.panes[name].frame.contentWindow === event.source);
  if (!letter) return;
  const other = letter === "a" ? "b" : "a";
  if (data.civvis === "pane-turn") {
    debugSplit.panes[letter].turn = data.turn;
    drawDebugSplit();
    // A pane that moved because it was following is not followed back.
    if (debugSplit.linked && data.how !== "follow" && data.how !== "boot")
      debugSplitTell(other, {civvis: "pane-follow", turn: data.turn, how: data.how});
  } else if (data.civvis === "pane-camera") {
    if (debugSplit.view) debugSplitTell(other, {civvis: "pane-look", view: data.view});
  } else if (data.civvis === "pane-numbers") {
    const pane = debugSplit.panes[letter];
    pane.numbers = data.numbers;
    pane.first = data.first;
    pane.last = data.last;
    // Turns past the last one the half still keeps belong to a history it
    // has since played over.
    const history = debugSplit.history[letter];
    for (const turn of [...history.keys()]) if (turn > data.last) history.delete(turn);
    history.set(data.turn, data.numbers);
    // A half that was told which Civ AI to seat is told again if it lost it:
    // a copy or a reload starts it on the page's own choice.
    if (pane.strategy === undefined) pane.strategy = data.strategy;
    else if (pane.strategy !== data.strategy) debugSplitTell(letter, {civvis: "pane-strategy", name: pane.strategy});
    drawDebugSplit();
  } else if (data.civvis === "pane-ran") {
    debugSplit.running = Math.max(0, debugSplit.running - 1);
    if (debugSplit.running === 0) debugSplit.until = null;
    drawDebugSplit();
  }
});

// ---- the pane's side
let debugPaneTurn = null;
let debugFollowWanted = null;
function debugPaneReport() {
  if (DEBUG_PANE === null || !state || SPEC || state.turn === debugPaneTurn) return;
  const first = debugPaneTurn === null;
  debugPaneTurn = state.turn;
  // The command deck is the host's to show; half a window has no room for it.
  if (first) togglePanel(true, false);
  const how = first ? "boot" : debugFollowing ? "follow" : debugTravelling ? "travel"
    : autoplaying ? "step" : "end";
  window.parent.postMessage({civvis: "pane-turn", pane: DEBUG_PANE, turn: state.turn, how}, location.origin);
}
// What this half says about its seat, for the host to set beside the other's.
let debugNumbersSaid = "";
function debugPaneNumbers() {
  if (DEBUG_PANE === null || !state || SPEC) return;
  const seat = state.players.find(player => player.id === 0) || {}, yields = seat.yields || {};
  const numbers = {score: seat.score, cities: seat.cities, population: seat.population,
    science: yields.science, culture: yields.culture, gold: yields.gold, faith: yields.faith,
    military: seat.military};
  const last = Math.max((state.debug || {}).last ?? state.turn, state.turn);
  const first = Math.min((state.debug || {}).first ?? state.turn, state.turn);
  const said = JSON.stringify([state.turn, first, last, numbers, debugStrategy() || ""]);
  if (said === debugNumbersSaid) return;
  debugNumbersSaid = said;
  window.parent.postMessage({civvis: "pane-numbers", pane: DEBUG_PANE, numbers, turn: state.turn, first, last,
    strategy: debugStrategy() || ""}, location.origin);
}
// Reach the turn the other pane is on, the way it got there: walked along
// the timeline, stepped by the Civ AI, or by ending the turn as it stands.
async function debugFollow(turn, how) {
  debugFollowWanted = {turn, how};
  if (debugFollowing) return;
  debugFollowing = true;
  try {
    for (let guard = 0; guard < 1000 && debugFollowWanted; guard++) {
      const wanted = debugFollowWanted;
      if (!debugTimeline() || state.turn === wanted.turn || gameFinished(state)) {
        if (debugFollowWanted === wanted) debugFollowWanted = null;
        continue;
      }
      const before = state.turn;
      if (state.turn > wanted.turn || (wanted.how === "travel" && state.debug.last >= wanted.turn)) {
        // A walk lands on the nearest marked turn, and that is where it stays.
        await debugOp({op: "jump", turn: wanted.turn});
        if (debugFollowWanted === wanted) debugFollowWanted = null;
      } else if (wanted.how === "step") await debugStep();
      else await send({type: "end_turn"});
      if (state.turn === before && debugFollowWanted === wanted) debugFollowWanted = null;
    }
  } finally {
    debugFollowing = false;
    debugFollowWanted = null;
  }
}
// Where this half's map is looking, as the few numbers the camera is made of.
// A half says so when it changes; a half told to look somewhere does not say
// it back.
let debugCameraSaid = "";
function debugCameraNow() {
  return {x: cam.x, y: cam.y, scale: cam.scale, rot: cam.rot, tilt: cam.tilt,
    pan: {x: SKY_PAN.x, y: SKY_PAN.y}};
}
function debugCameraLook(view) {
  if (!state || !view || !view.pan) return;
  cameraFlight = null; cameraZoom = null;
  clearCameraInertia();
  cam.x = +view.x; cam.y = +view.y; cam.scale = +view.scale; cam.rot = +view.rot; cam.tilt = +view.tilt;
  SKY_PAN = {x: +view.pan.x, y: +view.pan.y};
  draw(); drawMini();
  debugCameraSaid = JSON.stringify(debugCameraNow());
}
if (DEBUG_PANE !== null) {
  window.addEventListener("message", async event => {
    if (event.source !== window.parent || event.origin !== location.origin) return;
    const data = event.data;
    if (!data || typeof data.civvis !== "string") return;
    if (data.civvis === "pane-follow") debugFollow(data.turn, data.how);
    else if (data.civvis === "pane-look") debugCameraLook(data.view);
    else if (data.civvis === "pane-strategy") {
      const select = document.getElementById("autoplaystrategy");
      if (select && [...select.options].some(option => option.value === data.name)) select.value = data.name;
      debugPaneNumbers();
    } else if (data.civvis === "pane-run") {
      // A half that cannot run says so, or the host would wait on it.
      if (debugRunning || autoplaying || !debugTimeline())
        window.parent.postMessage({civvis: "pane-ran", pane: DEBUG_PANE}, location.origin);
      else debugRun(+data.until || null);
    } else if (data.civvis === "pane-stop") {
      if (debugRunning) { debugStopAsked = true; drawDebugBar(); }
    }
    else if (data.civvis === "pane-copy") {
      // A copy is not a move of this half's own, so the other is not told to follow it.
      debugFollowing = true;
      try {
        const next = await debugOp({op: "copy", from: data.from}, 60000);
        if (next && !next.error) debugSay("Copied from the other simulator");
      } finally {
        debugFollowing = false;
      }
    }
  });
  setInterval(() => {
    if (!state || SPEC) return;
    const said = JSON.stringify(debugCameraNow());
    if (said === debugCameraSaid) return;
    debugCameraSaid = said;
    window.parent.postMessage({civvis: "pane-camera", pane: DEBUG_PANE, view: JSON.parse(said)}, location.origin);
  }, 120);
}

// `render` is the one door a state comes through, whoever asked for it.
// `drawTurnLoop` is the page's own word that the turn's standing changed —
// an end of turn in flight, a request finished — which is exactly when the
// bar's buttons change what they may do, with no new state to say so.
{
  const drawState = render;
  render = function (st, ...rest) {
    const drawn = drawState(st, ...rest);
    try { drawDebugBar(); debugPaneReport(); debugPaneNumbers(); } catch (error) { console.error("debugger bar", error); }
    return drawn;
  };
  // In the middle of a round stepped a seat at a time the turn is not the
  // person's. Ending it means letting the rest of the round play, and
  // nothing else is theirs to do until it comes back.
  const sendAction = send;
  send = function (action, ...rest) {
    const timeline = debugTimeline();
    if (!timeline || !timeline.playing) return sendAction(action, ...rest);
    if (action && action.type === "end_turn")
      return debugOp({op: "finish_round"}, AUTOPLAY_BATCH_TIMEOUT_MS);
    debugSay(`${debugPlaysNext(timeline.playing)}; your turn comes back when the round finishes`);
    return Promise.resolve();
  };
  const drawLoop = drawTurnLoop;
  drawTurnLoop = function (...rest) {
    const drawn = drawLoop(...rest);
    try { drawDebugBar(false); } catch (error) { console.error("debugger bar", error); }
    return drawn;
  };
}
// Ctrl/Cmd+Z takes back the last action, where the hand already expects it.
window.addEventListener("keydown", event => {
  if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey || event.key.toLowerCase() !== "z") return;
  if (event.target instanceof HTMLElement && event.target.matches("input, textarea, select")) return;
  const timeline = debugTimeline();
  if (!timeline || !timeline.can_undo || autoplaying) return;
  event.preventDefault();
  debugOp({op: "undo"});
});
window.addEventListener("resize", () => { if (debugTimeline()) drawDebugBar(); });
// A page whose first state was drawn before this script arrived has had no
// `render` of ours yet: draw the bar now, and a half of a split says where it
// stands, or its host would wait for a report until something moved.
if (state) { drawDebugBar(); debugPaneReport(); debugPaneNumbers(); }
