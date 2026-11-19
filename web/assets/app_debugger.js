"use strict";
// The simulator debugger's bar.
//
// A played game keeps a mark of every turn its seat has opened (the engine's
// `server/debugger.rs`), and every state it sends says where the world sits
// on that timeline (`state.debug`). This script is the hand on it: step back
// a turn, undo an action, scrub to any marked turn, step forward with the
// seat's agent, run until a breakpoint, and choose which layers of the turn
// the agent decides while the rest stays with the person.
//
// It is a classic script like the others and loads after `app.js`, whose
// `render`, `fetchJSON` and turn loop it uses. Nothing in `app.js` knows it
// is here: the bar redraws by standing behind `render`, the one door every
// state comes through.

const DEBUG_BAR_STYLE = `
  /* The bar stands in the empty middle of the top strip, where a debugger's
     toolbar is looked for. When the strip has no middle left (a narrow
     window, one half of a split) it drops to a row of its own beneath. */
  #debugbar {
    position: absolute; z-index: 10; left: 50%; top: 5px; transform: translateX(-50%);
    display: flex; align-items: center; gap: 3px; height: 28px;
    color: var(--text); font-size: 12px; user-select: none; white-space: nowrap;
  }
  #debugbar[hidden], #debugbar [hidden] { display: none; }
  #debugbar.dbg-low {
    top: calc(var(--civtop-height, 38px) + var(--solo-launch-height, 60px) + 12px);
    left: auto; right: var(--panel-edge, 12px); transform: none; height: auto; padding: 5px 7px;
    border-radius: 12px; border: 1px solid #4d685f; background: #0a1512f2;
    box-shadow: var(--shadow); backdrop-filter: blur(18px);
  }
  #debugbar.dbg-low .dbg-pop, #debugbar.dbg-low .dbg-note { left: auto; right: 0; transform: none; }
  #debugbar button {
    min-width: 25px; height: 26px; padding: 0 4px; border-radius: 8px; cursor: pointer;
    color: var(--text); background: #14231f; border: 1px solid #35504a; font: inherit; line-height: 1;
  }
  #debugbar button:hover:not(:disabled) { border-color: var(--gold-bright, #e7c873); }
  #debugbar button:disabled { opacity: .36; cursor: default; }
  #debugbar button.on { color: #10140f; background: var(--gold-bright, #e7c873); border-color: transparent; font-weight: 700; }
  #debugbar .dbg-sep { width: 1px; height: 18px; background: #35504a; margin: 0 2px; }
  #debugbar #dbg-slider { width: clamp(60px, 5vw, 80px); margin: 0 2px; accent-color: var(--gold-bright, #e7c873); }
  #debugbar .dbg-count { color: var(--gold-bright, #e7c873); font-weight: 700; margin-left: 3px; }
  #debugbar .dbg-count:empty { display: none; }
  #debugbar button.on .dbg-count { color: inherit; }
  #debugbar .dbg-dot { color: #e2574c; }
  #debugbar .dbg-note {
    position: absolute; left: 50%; top: calc(100% + 7px); transform: translateX(-50%);
    max-width: min(560px, 80vw); padding: 5px 11px; border-radius: 999px; white-space: normal; text-align: center;
    color: var(--text); border: 1px solid #4d685f; background: #0a1512f2; box-shadow: var(--shadow);
  }
  #debugbar .dbg-note:empty { display: none; }
  #debugbar .dbg-note button { height: 20px; margin-left: 7px; }
  #debugbar .dbg-pop {
    position: absolute; left: 50%; top: calc(100% + 8px); transform: translateX(-50%); z-index: 1;
    width: min(360px, calc(100vw - 32px)); display: grid; gap: 8px; padding: 12px; white-space: normal;
    border-radius: 14px; border: 1px solid #4d685f; background: #0a1512fa; box-shadow: var(--shadow);
  }
  #debugbar .dbg-pop[hidden] { display: none; }
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
  /* Save points stand over the slider as flags, each at its own turn. */
  #debugbar .dbg-track { position: relative; display: inline-flex; align-items: center; }
  #debugbar .dbg-flags { position: absolute; left: 10px; right: 10px; top: -2px; height: 0; }
  #debugbar .dbg-flags i {
    position: absolute; width: 7px; height: 7px; margin-left: -4px; cursor: pointer;
    border-radius: 2px 2px 50% 50%; border: 1px solid #10140f; background: var(--gold-bright, #e7c873);
  }
  #debugbar .dbg-flags i.auto { background: #8fb4a8; }
  #debugbar #dbg-turn {
    width: 36px; height: 24px; padding: 0 2px; border-radius: 7px; text-align: center;
    color: var(--text); background: #060d0b; border: 1px solid #35504a; font: inherit;
  }
  #debugbar #dbg-turn:disabled { opacity: .5; }
`;

// One request at a time: every debugger operation replaces the world.
let debugBusy = false;
// A run is a loop of single steps; the bar's Run button is also its Stop.
let debugRunning = false, debugStopAsked = false;
let debugPopOpen = null;
let debugNoteText = "", debugNotePos = null;
let debugScrubbing = false;
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
function debugSayRound(stepped) {
  const did = stepped.seat === 0 ? "Your turn ended"
    : `${debugSeatName(stepped.seat)} played ${stepped.actions} action${stepped.actions === 1 ? "" : "s"}`;
  debugSay(stepped.next === 0 ? `${did} · turn ${state.turn} is yours`
    : `${did} · ${debugSeatName(stepped.next)} plays next`);
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
  debugRunning = true; debugStopAsked = false; autoplaying = true;
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
    debugRunning = false; debugStopAsked = false; autoplaying = false;
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
        is played on from here. Going to one keeps the world you left as a save point too.</p>
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
    </div>
    <button data-dbg="back" title="Back: the top of this turn, then the turn before" aria-label="Back one turn">◀</button>
    <button data-dbg="undo" title="Undo your last action this turn (Ctrl+Z)" aria-label="Undo last action">↶</button>
    <span class="dbg-track"><input type="range" id="dbg-slider" step="1" title="Drag to any turn that was played; the wheel walks one turn at a time" aria-label="Turn"><span class="dbg-flags" id="dbg-flags"></span></span>
    <input type="text" id="dbg-turn" inputmode="numeric" maxlength="4" title="Go to a turn: one already played is put back as it stood, one still ahead is played to by the Civ AI. Up and down walk one played turn" aria-label="Go to turn">
    <button data-dbg="forward" title="Forward along the turns already played" aria-label="Forward one played turn">▶</button>
    <button data-dbg="step-action" title="Step in: the Civ AI takes its next action this turn, and ends the turn when it has none left" aria-label="Step one action">▸</button>
    <button data-dbg="step-seat" title="Step one seat: end your turn as it stands, then one civilization's turn at a time" aria-label="Step one seat">▸|</button>
    <button data-dbg="step" title="Step: the Civ AI plays one turn. In the middle of a round, the rest of the round plays" aria-label="Step one turn">▶|</button>
    <button data-dbg="run" title="Run: the Civ AI plays until a breakpoint stops it" aria-label="Run to a breakpoint">▶▶</button>
    <span class="dbg-sep"></span>
    <button data-dbg-pop="layers" title="Layers: which decisions the Civ AI takes for you each turn">Layers<span class="dbg-count" id="dbg-layer-count"></span></button>
    <button data-dbg-pop="breaks" title="Breakpoints: what stops a run"><span class="dbg-dot">●</span><span class="dbg-count" id="dbg-break-count"></span></button>
    <button data-dbg="reveal" title="Reveal: show the whole map and every civilization, not only what yours can see" aria-label="Reveal the whole map">◉</button>
    <button data-dbg-pop="saves" title="Save points and saves: keep this world to come back to, or load one you kept" aria-label="Save points and saves">⚑<span class="dbg-count" id="dbg-point-count"></span></button>
    <button data-dbg-pop="split" title="Split: two simulators from this turn, side by side or stacked">Split</button>
    <div class="dbg-note" id="dbg-note" role="status"></div>`;
  document.getElementById("maparea").appendChild(bar);

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
    if (target.id === "dbg-slider") {
      debugScrubbing = false;
      debugSay("");
      debugGoToTurn(target.value);
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
    if (event.target.id !== "dbg-slider") return;
    debugScrubbing = true;
    debugSay(`Turn ${event.target.value}`);
  });
  // The map under the bar must not read a click on it as a click on a tile,
  // and the page's keys must not fire while a breakpoint is being typed.
  for (const kind of ["pointerdown", "mousedown", "wheel", "dblclick", "contextmenu"])
    bar.addEventListener(kind, event => event.stopPropagation());
  // The slider is short and a game is long: the wheel over the timeline, and
  // the up and down arrows in the turn box, walk the played turns one at a time.
  const walk = forward => {
    const timeline = debugTimeline();
    if (!timeline || debugBusy || autoplaying || turnAdvancePending) return;
    if (!(forward ? timeline.can_forward : timeline.can_back)) return;
    debugSay("");
    debugOp({op: forward ? "forward" : "back"});
  };
  bar.addEventListener("wheel", event => {
    if (!event.target.closest(".dbg-track, #dbg-turn")) return;
    event.preventDefault();
    const delta = Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : event.deltaY;
    if (delta) walk(delta > 0);
  }, {passive: false});
  bar.addEventListener("keydown", event => {
    if (!event.target.matches("input[type=text]")) return;
    event.stopPropagation();
    if (event.target.id === "dbg-turn" && (event.key === "ArrowUp" || event.key === "ArrowDown")) {
      event.preventDefault();
      walk(event.key === "ArrowUp");
    }
  });
  return bar;
}

function drawDebugBar(seat = true) {
  const bar = document.getElementById("debugbar") || debugBuildBar();
  const timeline = debugTimeline();
  bar.hidden = !timeline;
  if (!timeline) return;
  const idle = !debugBusy && !autoplaying && !turnAdvancePending;
  const set = (action, enabled) => {
    const button = bar.querySelector(`[data-dbg="${action}"]`);
    if (button) button.disabled = !enabled;
  };
  set("back", idle && timeline.can_back);
  set("undo", idle && timeline.can_undo);
  set("forward", idle && timeline.can_forward);
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
  run.textContent = debugRunning ? (debugStopAsked ? "…" : "⏹") : "▶▶";
  run.classList.toggle("on", debugRunning);

  const slider = document.getElementById("dbg-slider");
  const first = timeline.first ?? state.turn, last = Math.max(timeline.last ?? state.turn, state.turn);
  slider.min = first; slider.max = last;
  slider.disabled = !idle || first === last;
  if (!debugScrubbing) slider.value = state.turn;
  slider.title = `Turn ${state.turn} of ${first} to ${last}. Drag to any turn that was played; the wheel walks one turn at a time`;
  const turnBox = document.getElementById("dbg-turn");
  turnBox.disabled = !idle;
  // A number being typed is left alone, unless the turn moved under it.
  if (document.activeElement !== turnBox || turnBox.dataset.turn !== String(state.turn)) turnBox.value = state.turn;
  turnBox.dataset.turn = String(state.turn);
  // Drawn only when they change: a list rebuilt under the pointer loses the click.
  const drawn = (id, html) => {
    const box = document.getElementById(id);
    if (box.dataset.drawn !== html) { box.dataset.drawn = html; box.innerHTML = html; }
  };
  const points = [...(timeline.saves || [])].sort((a, b) => a.turn - b.turn || a.id - b.id);
  document.getElementById("dbg-point-count").textContent = points.length ? `${points.length}` : "";
  drawn("dbg-flags", points.filter(point => point.turn >= first && point.turn <= last).map(point =>
    `<i${point.auto ? ' class="auto"' : ""} data-dbg-goto="${point.id}" style="left:${last > first ? (point.turn - first) / (last - first) * 100 : 0}%"` +
    ` title="${escapeAttr(point.name)}: turn ${point.turn}. Click to go there"></i>`).join(""));
  drawn("dbg-point-list", points.length ? points.map(point =>
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
  const breaks = timeline.breakpoints;
  const conditions = breaks.conditions || [];
  const breakCount = breaks.turns.length + breaks.categories.length + breaks.phrases.length +
    conditions.length + (breaks.important ? 1 : 0);
  document.getElementById("dbg-break-count").textContent = breakCount ? `${breakCount}` : "";
  // A pane is already one half of a split, and does not split again.
  bar.querySelector('[data-dbg-pop="split"]').hidden = DEBUG_PANE !== null;
  for (const name of ["layers", "breaks", "saves", "split"]) {
    document.getElementById(`dbg-pop-${name}`).hidden = debugPopOpen !== name;
    bar.querySelector(`[data-dbg-pop="${name}"]`).classList.toggle("on", debugPopOpen === name);
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
    ? `${debugSeatName(timeline.playing)} plays next · ▸| steps it, ▶| lets the round finish` : "";
  const text = debugNoteText || round || layered;
  const html = escapeAttr(text) +
    (debugNoteText && debugNotePos ? `<button data-dbg="show" title="Centre the map there">Show</button>` : "");
  if (note.innerHTML !== html) note.innerHTML = html;
  if (seat) debugSeatBar(bar);
}
// In the top strip when the gap between its two ends has room, on a row
// beneath when it has not. The gap is not centred on the map, so the bar is
// centred on the gap.
function debugSeatBar(bar) {
  const left = document.querySelector("#civtop .civtop-left");
  const right = document.querySelector("#civtop .civtop-right");
  if (!left || !right) { bar.classList.add("dbg-low"); bar.style.left = ""; return; }
  // The controls' own width, not the box's: the low row has padding the strip does not.
  let width = 0;
  for (const child of bar.children)
    if (!child.classList.contains("dbg-pop") && !child.classList.contains("dbg-note"))
      width += child.getBoundingClientRect().width + 3;
  const from = left.getBoundingClientRect().right, to = right.getBoundingClientRect().left;
  const low = width > to - from - 24;
  bar.classList.toggle("dbg-low", low);
  bar.style.left = low ? ""
    : `${(from + to) / 2 - document.getElementById("maparea").getBoundingClientRect().left}px`;
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

const DEBUG_SPLIT_STYLE = `
  #debugsplit {
    position: fixed; inset: 0; z-index: 100000; display: grid; background: #050b0a;
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
    versus: versus ?? remembered, running: 0, chart: "score", history: {a: new Map(), b: new Map()}};
  for (const letter of ["a", "b"]) {
    const frame = host.querySelector(`[data-dbg-frame="${letter}"]`);
    debugSplit.panes[letter] = {frame, id: `${letter}-${nonce}`, turn: null};
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
// Play both halves to a turn, each with its own Civ AI. Linked, A is run and
// B follows it turn by turn; separate, each is run by itself. The same
// button stops them.
function debugVersusRun() {
  if (!debugSplit || debugSplit.busy) return;
  if (debugSplit.running > 0) {
    for (const letter of ["a", "b"]) debugSplitTell(letter, {civvis: "pane-stop"});
    debugSplit.running = 0;
    return;
  }
  const box = debugSplit.host.querySelector("[data-dbg-until]");
  const until = parseInt(box.value || box.placeholder, 10);
  const reached = Math.max(debugSplit.panes.a.turn ?? 0, debugSplit.panes.b.turn ?? 0);
  if (!Number.isFinite(until) || until <= reached) { box.value = ""; box.focus(); return; }
  const runners = debugSplit.linked ? ["a"] : ["a", "b"];
  debugSplit.running = runners.length;
  for (const letter of runners) debugSplitTell(letter, {civvis: "pane-run", until});
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
  const said = JSON.stringify([state.turn, last, numbers, debugStrategy() || ""]);
  if (said === debugNumbersSaid) return;
  debugNumbersSaid = said;
  window.parent.postMessage({civvis: "pane-numbers", pane: DEBUG_PANE, numbers, turn: state.turn, last,
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
    debugSay(`${debugSeatName(timeline.playing)} plays next; your turn comes back when the round finishes`);
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
