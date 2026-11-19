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
    display: flex; align-items: center; gap: 4px; height: 28px;
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
    min-width: 26px; height: 26px; padding: 0 5px; border-radius: 8px; cursor: pointer;
    color: var(--text); background: #14231f; border: 1px solid #35504a; font: inherit; line-height: 1;
  }
  #debugbar button:hover:not(:disabled) { border-color: var(--gold-bright, #e7c873); }
  #debugbar button:disabled { opacity: .36; cursor: default; }
  #debugbar button.on { color: #10140f; background: var(--gold-bright, #e7c873); border-color: transparent; font-weight: 700; }
  #debugbar .dbg-sep { width: 1px; height: 18px; background: #35504a; margin: 0 2px; }
  #debugbar #dbg-slider { width: clamp(64px, 7vw, 104px); margin: 0 2px; accent-color: var(--gold-bright, #e7c873); }
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
  #debugbar .dbg-pop .dbg-line { display: flex; align-items: center; gap: 6px; }
  #debugbar .dbg-pop .dbg-line > span { flex: none; width: 58px; color: var(--muted); }
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
const DEBUG_TRAVEL_OPS = new Set(["back", "forward", "jump", "restart", "undo"]);
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
    if (DEBUG_TRAVEL_OPS.has(body.op) || body.op === "step") {
      sel = null; selCity = null;
      if (next.turn !== before && body.op !== "step") debugForgetStandingOrders(next.turn);
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
// Played a turn a request, like auto-play, so every turn reaches the glass
// and the Stop button is never more than one turn away.
async function debugRun() {
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
      await new Promise(drawn => requestAnimationFrame(drawn));
    }
    if (debugStopAsked) debugSay(`Stopped by hand after ${played} turn${played === 1 ? "" : "s"}`);
  } finally {
    debugRunning = false; debugStopAsked = false; autoplaying = false;
    drawDebugBar();
    drawTurnLoop();
  }
}
async function debugSave() {
  const stamp = new Date();
  const name = `t${state.turn}-${String(stamp.getHours()).padStart(2, "0")}${String(stamp.getMinutes()).padStart(2, "0")}`;
  try {
    const done = await fetchJSON("/save", {method: "POST",
      headers: {"Content-Type": "application/json"}, body: JSON.stringify({name})});
    if (done.error) throw new Error(done.error);
    debugSay(`Saved turn ${state.turn} as ${name}`);
    refreshSaves();
  } catch (error) {
    debugSay(`Could not save: ${error.message || error}`);
  }
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
      <p>Two simulators start from this turn. Each one is a whole game of its own and plays its own
        history. Linked, they stay on the same turn; separate, each keeps its own.</p>
      <div class="dbg-line">
        <button data-dbg="split-row" title="A on the left, B on the right">◧ Side by side</button>
        <button data-dbg="split-col" title="A above, B below">⬒ Top and bottom</button>
      </div>
    </div>
    <div class="dbg-pop" id="dbg-pop-breaks" hidden>
      <h4>Breakpoints</h4>
      <p>Run plays turns with the Civ AI until one of these stops it.</p>
      <div class="dbg-line"><span>Turns</span>
        <input type="text" data-dbg-field="turns" inputmode="numeric" placeholder="50, 75" aria-label="Stop when these turns open">
        <button data-dbg="break-plus" title="Add the turn ten from now">+10</button></div>
      <div class="dbg-line"><span>Phrases</span>
        <input type="text" data-dbg-field="phrases" placeholder="declared war, captured" aria-label="Stop when the event log says"></div>
      <div class="dbg-grid" id="dbg-category-list"></div>
      <label><input type="checkbox" data-dbg-field="important"> Any important event</label>
      <label><input type="checkbox" data-dbg-field="every_seat"> Every civilization's events, not only mine</label>
      <div class="dbg-line"><button data-dbg="break-clear">Clear all</button></div>
    </div>
    <button data-dbg="back" title="Back: the top of this turn, then the turn before" aria-label="Back one turn">◀</button>
    <button data-dbg="undo" title="Undo your last action this turn (Ctrl+Z)" aria-label="Undo last action">↶</button>
    <input type="range" id="dbg-slider" step="1" title="Drag to any marked turn" aria-label="Turn">
    <button data-dbg="forward" title="Forward along the turns already played" aria-label="Forward one played turn">▶</button>
    <button data-dbg="step" title="Step: the Civ AI plays one turn" aria-label="Step one turn">▶|</button>
    <button data-dbg="run" title="Run: the Civ AI plays until a breakpoint stops it" aria-label="Run to a breakpoint">▶▶</button>
    <span class="dbg-sep"></span>
    <button data-dbg-pop="layers" title="Layers: which decisions the Civ AI takes for you each turn">Layers<span class="dbg-count" id="dbg-layer-count"></span></button>
    <button data-dbg-pop="breaks" title="Breakpoints: what stops a run"><span class="dbg-dot">●</span><span class="dbg-count" id="dbg-break-count"></span></button>
    <button data-dbg="save" title="Save this turn under its number" aria-label="Save this turn">⤓</button>
    <button data-dbg-pop="split" title="Split: two simulators from this turn, side by side or stacked">Split</button>
    <div class="dbg-note" id="dbg-note" role="status"></div>`;
  document.getElementById("maparea").appendChild(bar);

  bar.addEventListener("click", event => {
    const pop = event.target.closest("[data-dbg-pop]");
    if (pop) { debugTogglePop(pop.dataset.dbgPop); return; }
    const button = event.target.closest("[data-dbg]");
    if (!button || button.disabled) return;
    const timeline = debugTimeline();
    if (!timeline) return;
    const action = button.dataset.dbg;
    if (action === "back") debugOp({op: "back"});
    else if (action === "undo") debugOp({op: "undo"});
    else if (action === "forward") debugOp({op: "forward"});
    else if (action === "step") debugStep();
    else if (action === "run") debugRun();
    else if (action === "save") debugSave();
    else if (action === "split-row" || action === "split-col") {
      debugPopOpen = null;
      debugSplitOpen(action === "split-row" ? "row" : "col");
    }
    else if (action === "show") { if (debugNotePos) centerOn(debugNotePos); }
    else if (action === "assist") debugOp({op: "assist", strategy: debugStrategy()}, AUTOPLAY_BATCH_TIMEOUT_MS);
    else if (action === "layers-none" || action === "layers-all") {
      debugOp({op: "layers", layers: action === "layers-all" ? timeline.layers.map(layer => layer.id) : []});
    } else if (action === "break-clear") {
      debugOp({op: "breakpoints"});
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
      const turn = +target.value;
      debugSay("");
      if (state && turn !== state.turn) debugOp({op: "jump", turn});
    } else if (target.dataset.dbgLayer) {
      debugOp({op: "layers", layers: debugLayersFromForm()});
    } else if (target.closest("#dbg-pop-breaks")) {
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
  bar.addEventListener("keydown", event => {
    if (event.target.matches("input[type=text]")) event.stopPropagation();
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
  set("save", idle);
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
  slider.title = `Turn ${state.turn} of ${first} to ${last}. Drag to any marked turn`;

  const layersOn = timeline.layers.filter(layer => layer.on).length;
  document.getElementById("dbg-layer-count").textContent = layersOn ? `${layersOn}` : "";
  const breaks = timeline.breakpoints;
  const breakCount = breaks.turns.length + breaks.categories.length + breaks.phrases.length +
    (breaks.important ? 1 : 0);
  document.getElementById("dbg-break-count").textContent = breakCount ? `${breakCount}` : "";
  // A pane is already one half of a split, and does not split again.
  bar.querySelector('[data-dbg-pop="split"]').hidden = DEBUG_PANE !== null;
  for (const name of ["layers", "breaks", "split"]) {
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
  if (document.activeElement !== field("turns")) field("turns").value = breaks.turns.join(", ");
  if (document.activeElement !== field("phrases")) field("phrases").value = breaks.phrases.join(", ");
  field("important").checked = breaks.important;
  field("every_seat").checked = breaks.every_seat;

  const note = document.getElementById("dbg-note");
  const layered = timeline.assisted
    ? `The Civ AI took ${timeline.assisted} decision${timeline.assisted === 1 ? "" : "s"} in its layers this turn` : "";
  const text = debugNoteText || layered;
  const html = escapeAttr(text) +
    (debugNoteText && debugNotePos ? `<button data-dbg="show" title="Centre the map there">Show</button>` : "");
  if (note.innerHTML !== html) note.innerHTML = html;
  if (seat) debugSeatBar(bar);
}
// In the top strip when its middle is free, on a row beneath when it is not.
function debugSeatBar(bar) {
  const left = document.querySelector("#civtop .civtop-left");
  const right = document.querySelector("#civtop .civtop-right");
  if (!left || !right) { bar.classList.add("dbg-low"); return; }
  const area = document.getElementById("maparea").getBoundingClientRect();
  const centre = area.left + area.width / 2;
  // The controls' own width, not the box's: the low row has padding the strip does not.
  let width = 0;
  for (const child of bar.children)
    if (!child.classList.contains("dbg-pop") && !child.classList.contains("dbg-note"))
      width += child.getBoundingClientRect().width + 4;
  const room = 2 * Math.min(centre - left.getBoundingClientRect().right,
    right.getBoundingClientRect().left - centre) - 20;
  bar.classList.toggle("dbg-low", width > room);
}

// ------------------------------------------------------------------ split
//
// Two simulators in one window. The page that was playing becomes the host:
// it covers itself with two frames, and each frame is this same page again
// with a simulator of its own (an engine of its own on the published build, a
// session of its own on a socket server: `?pane=`). Both start on the host's
// world as it stood when the screen was split, and from there each plays its
// own history.
//
// Linked, the two stay on the same turn: whatever moves one — ending the
// turn, a step, a walk along the timeline — is repeated in the other the same
// way. Separate, neither knows about the other.
//
// Host and panes talk by `postMessage` on their own origin:
//   pane → host  pane-boot   the published build's shim asking for its world
//   host → pane  pane-world  that world, as save text
//   pane → host  pane-turn   the turn it is on, and how it got there
//   host → pane  pane-follow go to this turn the same way
//   host → pane  pane-save   → pane-saved: the pane's world, as save text
//   host → pane  pane-load   become this world

const DEBUG_SPLIT_STYLE = `
  #debugsplit {
    position: fixed; inset: 0; z-index: 100000; display: grid; background: #050b0a;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr); grid-template-rows: minmax(0, 1fr);
  }
  #debugsplit.dbg-col { grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(0, 1fr) auto minmax(0, 1fr); }
  #debugsplit iframe { width: 100%; height: 100%; border: 0; display: block; background: #07110f; }
  #debugsplit .dbg-divider {
    display: flex; flex-direction: column; align-items: stretch; justify-content: center; gap: 6px;
    width: 68px; padding: 6px 5px; background: #0a1512; border-inline: 1px solid #35504a;
    color: var(--text, #e8e3d2); font: 11px/1.2 system-ui, sans-serif; user-select: none;
  }
  #debugsplit.dbg-col .dbg-divider {
    flex-direction: row; align-items: center; width: auto; height: 36px; padding: 4px 8px;
    border-inline: 0; border-block: 1px solid #35504a;
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
`;
// The host's record of the split, or null when the screen is whole.
let debugSplit = null;
let debugSplitAsks = 0;

function debugSplitPane(letter) { return debugSplit ? debugSplit.panes[letter] : null; }
function debugSplitTell(letter, message) {
  const pane = debugSplitPane(letter);
  if (pane && pane.frame.contentWindow) pane.frame.contentWindow.postMessage(message, location.origin);
}
// A pane's world as save text. Answered by message, so it is awaited by id.
function debugSplitWorld(letter) {
  return new Promise((resolve, reject) => {
    const id = ++debugSplitAsks;
    const timer = setTimeout(() => {
      debugSplit && debugSplit.waiting.delete(id);
      reject(new Error(`simulator ${letter.toUpperCase()} did not answer`));
    }, 30000);
    debugSplit.waiting.set(id, game => { clearTimeout(timer); resolve(game); });
    debugSplitTell(letter, {civvis: "pane-save", id});
  });
}
function drawDebugSplit() {
  if (!debugSplit) return;
  const host = document.getElementById("debugsplit");
  host.classList.toggle("dbg-col", debugSplit.dir === "col");
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
  host.querySelector('[data-dbg-split="flip"]').textContent = debugSplit.dir === "col" ? "◧" : "⬒";
  for (const button of host.querySelectorAll("[data-dbg-split]")) button.disabled = debugSplit.busy;
}
async function debugSplitOpen(dir) {
  if (debugSplit || DEBUG_PANE !== null || !debugTimeline() || debugBusy || autoplaying) return;
  let world;
  try { world = JSON.stringify(await fetchJSON("/save", {}, 30000)); }
  catch (error) { debugSay(`Could not split: ${error.message || error}`); return; }
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
    <div class="dbg-divider" role="toolbar" aria-label="Split simulators">
      <div class="dbg-pane-name" data-dbg-name="a"></div>
      <button data-dbg-split="copy-a" title="Make B a copy of A as it stands now">A → B</button>
      <button data-dbg-split="keep-a" title="Close the split and keep playing A">Keep A</button>
      <span class="dbg-gap"></span>
      <button data-dbg-split="link"></button>
      <button data-dbg-split="flip" title="Switch between side by side and top and bottom"></button>
      <span class="dbg-gap"></span>
      <button data-dbg-split="keep-b" title="Close the split and keep playing B">Keep B</button>
      <button data-dbg-split="copy-b" title="Make A a copy of B as it stands now">B → A</button>
      <div class="dbg-pane-name" data-dbg-name="b"></div>
    </div>
    <iframe data-dbg-frame="b" title="Simulator B"></iframe>`;
  debugSplit = {dir, linked: true, world, busy: false, waiting: new Map(), panes: {}};
  for (const letter of ["a", "b"]) {
    const frame = host.querySelector(`[data-dbg-frame="${letter}"]`);
    debugSplit.panes[letter] = {frame, id: `${letter}-${nonce}`, turn: null};
  }
  host.addEventListener("click", event => {
    const button = event.target.closest("[data-dbg-split]");
    if (!button || button.disabled || !debugSplit) return;
    const action = button.dataset.dbgSplit;
    if (action === "link") {
      debugSplit.linked = !debugSplit.linked;
      // Linking two simulators that have drifted brings B to A's turn.
      const a = debugSplit.panes.a.turn;
      if (debugSplit.linked && a !== null && a !== debugSplit.panes.b.turn)
        debugSplitTell("b", {civvis: "pane-follow", turn: a, how: "end"});
    } else if (action === "flip") debugSplit.dir = debugSplit.dir === "col" ? "row" : "col";
    else if (action === "copy-a") debugSplitCopy("a", "b");
    else if (action === "copy-b") debugSplitCopy("b", "a");
    else if (action === "keep-a") debugSplitClose("a");
    else if (action === "keep-b") debugSplitClose("b");
    drawDebugSplit();
  });
  document.body.appendChild(host);
  drawDebugSplit();
  // Set last: a frame that loads asks for its world at once, and the record
  // it is answered from has to exist by then.
  for (const letter of ["a", "b"]) {
    const pane = debugSplit.panes[letter];
    pane.frame.src = `${location.pathname}?pane=${pane.id}`;
  }
}
async function debugSplitCopy(from, to) {
  if (!debugSplit || debugSplit.busy) return;
  debugSplit.busy = true; drawDebugSplit();
  try {
    const game = await debugSplitWorld(from);
    debugSplitTell(to, {civvis: "pane-load", game});
  } catch (error) {
    console.error("split copy", error);
  } finally {
    if (debugSplit) { debugSplit.busy = false; drawDebugSplit(); }
  }
}
// The screen is whole again, playing the pane that was kept.
async function debugSplitClose(keep) {
  if (!debugSplit || debugSplit.busy) return;
  debugSplit.busy = true; drawDebugSplit();
  let note = "";
  try {
    const game = await debugSplitWorld(keep);
    const next = await fetchJSON("/load", {method: "POST",
      headers: {"Content-Type": "application/json"}, body: `{"game":${game}}`}, 60000);
    if (next.error) throw new Error(next.error);
    debugForgetStandingOrders(next.turn);
    sel = null; selCity = null;
    render(next);
    note = `Playing on from simulator ${keep.toUpperCase()}`;
  } catch (error) {
    note = `Simulator ${keep.toUpperCase()} could not be kept: ${error.message || error}`;
  }
  document.getElementById("debugsplit").remove();
  debugSplit = null;
  debugSay(note);
}
window.addEventListener("message", event => {
  if (!debugSplit || event.origin !== location.origin) return;
  const data = event.data;
  if (!data || typeof data.civvis !== "string") return;
  const letter = ["a", "b"].find(name => debugSplit.panes[name].frame.contentWindow === event.source);
  if (!letter) return;
  if (data.civvis === "pane-boot") {
    event.source.postMessage({civvis: "pane-world", pane: data.pane, game: debugSplit.world}, location.origin);
  } else if (data.civvis === "pane-turn") {
    debugSplit.panes[letter].turn = data.turn;
    drawDebugSplit();
    // A pane that moved because it was following is not followed back.
    if (debugSplit.linked && data.how !== "follow" && data.how !== "boot")
      debugSplitTell(letter === "a" ? "b" : "a", {civvis: "pane-follow", turn: data.turn, how: data.how});
  } else if (data.civvis === "pane-saved") {
    const waiting = debugSplit.waiting.get(data.id);
    if (waiting) { debugSplit.waiting.delete(data.id); waiting(data.game); }
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
if (DEBUG_PANE !== null) {
  window.addEventListener("message", async event => {
    if (event.source !== window.parent || event.origin !== location.origin) return;
    const data = event.data;
    if (!data || typeof data.civvis !== "string") return;
    if (data.civvis === "pane-follow") debugFollow(data.turn, data.how);
    else if (data.civvis === "pane-save") {
      let game = null;
      try { game = JSON.stringify(await fetchJSON("/save", {}, 30000)); } catch (_) {}
      window.parent.postMessage({civvis: "pane-saved", id: data.id, game}, location.origin);
    } else if (data.civvis === "pane-load") {
      try {
        const next = await fetchJSON("/load", {method: "POST",
          headers: {"Content-Type": "application/json"}, body: `{"game":${data.game}}`}, 60000);
        if (next.error) throw new Error(next.error);
        debugForgetStandingOrders(next.turn);
        sel = null; selCity = null;
        debugFollowing = true;
        try { render(next); } finally { debugFollowing = false; }
        debugSay("Copied from the other simulator");
      } catch (error) {
        debugSay(`Could not copy the other simulator: ${error.message || error}`);
      }
    }
  });
}

// `render` is the one door a state comes through, whoever asked for it.
// `drawTurnLoop` is the page's own word that the turn's standing changed —
// an end of turn in flight, a request finished — which is exactly when the
// bar's buttons change what they may do, with no new state to say so.
{
  const drawState = render;
  render = function (st, ...rest) {
    const drawn = drawState(st, ...rest);
    try { drawDebugBar(); debugPaneReport(); } catch (error) { console.error("debugger bar", error); }
    return drawn;
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
if (state) drawDebugBar();
