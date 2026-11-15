"use strict";
// Arena Stats: the record that persists between simulations, carved out of
// app.js. The ledger in the viewer's own browser, the three views of it
// (civilizations, player strategies, victory types) and the section of the
// victory panel that prints whichever view was chosen last.
//
// ⚠ THIS SCRIPT IS LOADED BEFORE app.js, for the reason app_palette.js gives
// at length: every script the page loads shares one global scope, and a name
// moved EARLIER can never be read before its declaration has run, where one
// moved later can. Nothing here runs at load beyond reading the saved ledger
// and the saved view out of localStorage. Every function below is called by
// app.js once a world is on screen, so the renderer's own helpers it leans on
// (`state`, `RULES`, `reportedTurn`, `victoryVerdict`, `escapeAttr`,
// `hudFoldButton`, `drawPlayerHud`, and app_setup.js's
// `isBattlefieldMapScript`) all exist by the time anything asks for them.
//
// `tools/test_web_assets.py` holds the rest of the contract: the page, the
// binary's `include_str!` and the serving route all name the same scripts,
// and no `const` is declared at the top level of two of them.

// ------------------------------------------------------- simulation stats
// The record that persists between simulations: every finished game bumps a
// win–loss–draw ledger per civilization and per player strategy, plus a
// count of wins by victory type. It stays per mode so a Tactics record never
// mixes into a Civ one.
//
// A player strategy is a named player: a Civ AI Agent by its handle, or a
// person. Those records live under `agents`. The older `strategies` table
// was keyed by whatever each seat printed, which for the house fleet is a
// name generated for the one game, so it filled with rows no later game
// could add to. Nothing reads it any more and nothing deletes it.
// Aggregates only — the ledger stays a few hundred bytes however many games
// it has seen.
const SIMULATION_STATS_KEY = "civvis-simulation-stats-v1";
const SIM_STATS_VIEW_KEY = "civvis-sim-stats-view-v1";
const SIM_STATS_VIEWS = {
  civs: {label: "Civs", title: "Win–loss record by civilization"},
  strategies: {label: "Strategies", table: "agents",
    title: "Win–loss record by player strategy: each Civ AI Agent, and anyone who sat down to play"},
  victories: {label: "Victory types", title: "Wins by victory type"},
};
let simulationStats = (() => {
  try {
    const saved = JSON.parse(localStorage.getItem(SIMULATION_STATS_KEY) || "{}");
    if (saved && typeof saved === "object" && !Array.isArray(saved)) return saved;
  } catch (_) {}
  return {};
})();
let simStatsView = (() => {
  try {
    const saved = localStorage.getItem(SIM_STATS_VIEW_KEY);
    return SIM_STATS_VIEWS[saved] ? saved : "civs";
  } catch (_) { return "civs"; }
})();
function saveSimulationStats() {
  try { localStorage.setItem(SIMULATION_STATS_KEY, JSON.stringify(simulationStats)); } catch (_) {}
}
function rememberSimStatsView(view) {
  simStatsView = SIM_STATS_VIEWS[view] ? view : "civs";
  try { localStorage.setItem(SIM_STATS_VIEW_KEY, simStatsView); } catch (_) {}
}
function simulationStatsMode(st = state) {
  return st && isBattlefieldMapScript(st.map?.script) ? "tactics" : "civ";
}
function simulationStatsBucket(mode) {
  const bucket = simulationStats[mode] ?? (simulationStats[mode] = {});
  bucket.games ??= 0;
  bucket.turns ??= 0;
  bucket.civs ??= {};
  bucket.agents ??= {};
  bucket.victories ??= {};
  return bucket;
}
// The player strategy in a seat: a registered person, else the Civ AI Agent
// seated there by its handle. The house fleet's generated name is nobody's
// record, so a seat with only that has no strategy to credit.
function simulationStrategyName(p) {
  return p.player_username || p.ai_username || null;
}
function bumpSimulationRecord(table, key, column) {
  const row = table[key] ?? (table[key] = {w:0, l:0, d:0});
  row[column] += 1;
}
function bumpSimulationVictory(table, victory) {
  table[victory] = (Number(table[victory]) || 0) + 1;
}
// Called from the result screens, whose signature guard already runs once per
// distinct result; the stored key repeats that guard across reloads so a page
// reopened over a finished world cannot count the same game twice.
function recordSimulationResult(st, signature, winnerIds) {
  const key = `${st.server_instance ?? ""}|${signature}`;
  if (simulationStats.last === key) return;
  simulationStats.last = key;
  const bucket = simulationStatsBucket(simulationStatsMode(st));
  bucket.games += 1;
  bucket.turns += Number(reportedTurn(st)) || 0;
  const winnerSeats = new Set(winnerIds
    .map(id => st.players[id]?.id)
    .filter(id => id !== undefined));
  if (winnerSeats.size)
    bumpSimulationVictory(bucket.victories, victoryVerdict(st.victory_type, st.victory_label));
  for (const p of st.players) {
    if (p.is_minor || p.is_barbarian) continue;
    const column = winnerSeats.size === 0 ? "d" : winnerSeats.has(p.id) ? "w" : "l";
    bumpSimulationRecord(bucket.civs, p.civ || "Unknown civilization", column);
    const strategy = simulationStrategyName(p);
    if (strategy) bumpSimulationRecord(bucket.agents, strategy, column);
  }
  saveSimulationStats();
  if (state) drawPlayerHud();
}
function clearSimulationStats(mode) {
  delete simulationStats[mode];
  saveSimulationStats();
  if (state) drawPlayerHud();
}

// The record row a name has earned: wins–losses, with draws only once one
// exists — a Tactics arena full of decisive battles never prints a zero.
function formatSimulationRecord(row) {
  return row.d ? `${row.w}–${row.l}–${row.d}` : `${row.w}–${row.l}`;
}
function describeSimulationRecord(row) {
  const parts = [`${row.w} win${row.w === 1 ? "" : "s"}`, `${row.l} loss${row.l === 1 ? "" : "es"}`];
  if (row.d) parts.push(`${row.d} draw${row.d === 1 ? "" : "s"}`);
  return parts.join(", ");
}
// Best record first: win rate, then the most wins at that rate, so an
// undefeated name with ten wins stands above one with a single win. The rates
// are cross-multiplied so 1–1 and 2–2 tie exactly. Equal rates and wins can
// still differ in losses against draws, and every winless row ties at zero, so
// the fewer losses read better.
function compareSimulationRecords([aName, a], [bName, b]) {
  const aGames = a.w + a.l + a.d;
  const bGames = b.w + b.l + b.d;
  return b.w * aGames - a.w * bGames
    || b.w - a.w
    || a.l - b.l
    || aName.localeCompare(bName);
}
// The persistent record above the victory tracker: this mode's games and
// average length, then the win–loss ledger by civilization or by player
// strategy, or its wins by victory type — whichever view was chosen last.
function simulationStatsSection() {
  const mode = simulationStatsMode();
  const bucket = simulationStats[mode];
  const games = bucket?.games || 0;
  const modeLabel = mode === "tactics" ? "Tactics" : "Civvis";
  const summary = games
    ? `${games} game${games === 1 ? "" : "s"}, avg ${Math.round(bucket.turns / games)} turns`
    : "No finished games yet";
  const table = bucket?.[SIM_STATS_VIEWS[simStatsView].table || simStatsView] || {};
  // A Civ AI Agent's row carries the rating it plays at beside its record.
  const ratingByHandle = simStatsView !== "strategies" ? null
    : new Map((Array.isArray(RULES?.strategies) ? RULES.strategies : [])
      .filter(entry => entry.elo !== null && entry.elo !== undefined)
      .map(entry => [entry.username, Math.round(Number(entry.elo))]));
  const strategyRating = name => ratingByHandle?.get(name) ?? null;
  const rows = simStatsView === "victories"
    ? Object.entries(table)
      .sort(([aName, a], [bName, b]) => Number(b) - Number(a) || aName.localeCompare(bName))
      .map(([name, wins]) => {
        const count = Math.max(0, Math.round(Number(wins) || 0));
        const label = `${count} win${count === 1 ? "" : "s"}`;
        return `<div class="sim-stat-row" title="${escapeAttr(`${name} — ${label}`)}">` +
          `<span class="sim-stat-name">${name}</span>` +
          `<span class="sim-stat-record">${label}</span></div>`;
      })
      .join("")
    : Object.entries(table)
      .sort(compareSimulationRecords)
      .map(([name, row]) => {
        const rating = strategyRating(name);
        return `<div class="sim-stat-row" title="${escapeAttr(`${name}` +
          `${rating === null ? "" : ` (Elo ${rating})`} — ${describeSimulationRecord(row)}`)}">` +
          `<span class="sim-stat-name">${name}` +
          `${rating === null ? "" : `<span class="sim-stat-elo">${rating}</span>`}</span>` +
          `<span class="sim-stat-record">${formatSimulationRecord(row)}</span></div>`;
      })
      .join("");
  const viewButton = view =>
    `<button class="sim-stats-view${simStatsView === view ? " active" : ""}" type="button" ` +
    `data-sim-stats-view="${view}" aria-pressed="${simStatsView === view}" ` +
    `title="${SIM_STATS_VIEWS[view].title}">${SIM_STATS_VIEWS[view].label}</button>`;
  return `<section class="hud-region sim-stats-region" data-key="sim-stats" aria-labelledby="sim-stats-title">` +
    `<div class="hud-region-bar">` +
    `<div class="hud-section-heading widget-drag-handle" data-widget-drag tabindex="0" role="button" ` +
    `aria-label="Move Arena Stats; use arrow keys or drag" title="Drag to move · arrow keys also move" id="sim-stats-title">` +
    `<span>Arena Stats</span></div>` +
    hudFoldButton("simstats", "Arena Stats") +
    `</div>` +
    `<div class="sim-stats-body">` +
    `<div class="sim-stats-summary-row"><div class="sim-stats-summary">${summary}</div>` +
    (games ? `<button class="sim-stats-clear" type="button" data-sim-stats-clear ` +
      `title="Reset the saved ${modeLabel} record" aria-label="Reset the saved ${modeLabel} record">Reset</button>` : ``) +
    `</div>` +
    // A ledger with nothing in it has no views to choose between, and the panel
    // gives the whole section up until the first game finishes (`sim-stats-none`).
    (games ? `<div class="sim-stats-controls" role="group" aria-label="Arena Stats record view">` +
      viewButton("civs") + viewButton("strategies") + viewButton("victories") +
      `</div>` +
      // Games on record and no player strategy among them: every one was
      // played by the house fleet, whose seats are nobody's record.
      `<div class="sim-stats-rows">${rows ||
        (simStatsView === "strategies"
          ? `<div class="sim-stats-empty">No player strategy has finished a game here yet</div>` : ``)}</div>` : ``) +
    `</div></section>`;
}
