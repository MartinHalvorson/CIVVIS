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
  #debugbar[hidden] { display: none; }
  #debugbar.dbg-low {
    top: calc(var(--civtop-height, 38px) + 6px); height: auto; padding: 5px 7px;
    border-radius: 12px; border: 1px solid #4d685f; background: #0a1512f2;
    box-shadow: var(--shadow); backdrop-filter: blur(18px);
  }
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
  else debugSay("The game has no turn left to play");
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
    <button data-dbg="save" title="Save this turn under its number">Save</button>
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

function drawDebugBar() {
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
  for (const name of ["layers", "breaks"]) {
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
  debugSeatBar(bar);
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

// `render` is the one door a state comes through, whoever asked for it.
{
  const drawState = render;
  render = function (st, ...rest) {
    const drawn = drawState(st, ...rest);
    try { drawDebugBar(); } catch (error) { console.error("debugger bar", error); }
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
