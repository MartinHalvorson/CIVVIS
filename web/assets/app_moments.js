"use strict";
// What finishing something feels like.
//
// A technology, a wonder or a new era used to arrive as one more icon in the
// notification rail, in silence. These are the payoffs a turn is played for,
// so each now gets a banner and a sound. Nothing here changes the game: it
// reads the turn's events, shows them, and gets out of the way. Banners never
// block the map, and the rail keeps its record of everything.
const MOMENT_LIFE_MS = 5000;
const MOMENT_BIG_LIFE_MS = 7500;
const MOMENT_STACK = 2;      // on screen at once; the rest wait their turn
const MOMENT_PER_TURN = 5;   // a late-game turn can finish a dozen things
const SOUND_PREFERENCE_KEY = "civvis-sound-v1";

// ------------------------------------------------------------------ sound
// Every sound is synthesised here, so the site ships no audio files and a
// player who keeps it muted pays nothing. A browser will not start audio before
// the first gesture, which is why the context is made on demand.
let soundEnabled = true;
try { soundEnabled = localStorage.getItem(SOUND_PREFERENCE_KEY) !== "off"; } catch (_) {}
let audioCtx = null, audioOut = null;
function audioReady() {
  if (!soundEnabled) return null;
  const Context = window.AudioContext || window.webkitAudioContext;
  if (!Context) return null;
  if (!audioCtx) {
    audioCtx = new Context();
    audioOut = audioCtx.createGain();
    audioOut.gain.value = .5;
    audioOut.connect(audioCtx.destination);
  }
  if (audioCtx.state === "suspended") audioCtx.resume().catch(() => {});
  return audioCtx.state === "running" ? audioCtx : null;
}
function sfxTone(freq, {at = 0, dur = .3, gain = .1, type = "sine", to = 0} = {}) {
  const context = audioReady();
  if (!context) return;
  const start = context.currentTime + at;
  const osc = context.createOscillator(), env = context.createGain();
  osc.type = type;
  osc.frequency.setValueAtTime(freq, start);
  if (to) osc.frequency.exponentialRampToValueAtTime(to, start + dur);
  env.gain.setValueAtTime(.0001, start);
  env.gain.exponentialRampToValueAtTime(gain, start + Math.min(.02, dur / 4));
  env.gain.exponentialRampToValueAtTime(.0001, start + dur);
  osc.connect(env).connect(audioOut);
  osc.start(start);
  osc.stop(start + dur + .02);
}
function thud({at = 0, dur = .22, gain = .22, cutoff = 420} = {}) {
  const context = audioReady();
  if (!context) return;
  const start = context.currentTime + at;
  const length = Math.ceil(context.sampleRate * dur);
  const buffer = context.createBuffer(1, length, context.sampleRate);
  const data = buffer.getChannelData(0);
  for (let i = 0; i < length; i++) data[i] = (Math.random() * 2 - 1) * (1 - i / length);
  const source = context.createBufferSource(), filter = context.createBiquadFilter();
  const env = context.createGain();
  source.buffer = buffer;
  filter.type = "lowpass"; filter.frequency.value = cutoff;
  env.gain.setValueAtTime(gain, start);
  env.gain.exponentialRampToValueAtTime(.0001, start + dur);
  source.connect(filter).connect(env).connect(audioOut);
  source.start(start);
}
const arpeggio = (notes, step, options) =>
  notes.forEach((freq, i) => sfxTone(freq, {...options, at: i * step}));
const GAME_SOUNDS = {
  click: () => sfxTone(1320, {dur: .05, gain: .025, type: "triangle"}),
  endTurn: () => { sfxTone(196, {dur: .45, gain: .1}); sfxTone(294, {dur: .45, gain: .06, at: .04}); },
  newTurn: () => arpeggio([392, 587], .09, {dur: .32, gain: .06, type: "triangle"}),
  research: () => arpeggio([523, 659, 784], .08, {dur: .38, gain: .07, type: "triangle"}),
  civic: () => arpeggio([440, 554, 740], .09, {dur: .42, gain: .06}),
  city: () => { for (const freq of [262, 330, 392]) sfxTone(freq, {dur: .8, gain: .06, type: "triangle"}); },
  person: () => arpeggio([659, 880, 1319], .1, {dur: .6, gain: .05}),
  wonder: () => {
    for (const freq of [196, 294, 392]) sfxTone(freq, {dur: 1.6, gain: .06, type: "triangle"});
    arpeggio([587, 784, 988, 1175], .14, {dur: .9, gain: .05, at: .2});
  },
  era: () => {
    for (const freq of [131, 196, 262]) sfxTone(freq, {dur: 2, gain: .07, type: "sawtooth"});
    arpeggio([392, 523, 659, 784], .18, {dur: 1, gain: .05, type: "triangle"});
  },
  war: () => { thud(); thud({at: .24}); sfxTone(98, {dur: .7, gain: .1, type: "sawtooth", to: 73}); },
  contact: () => arpeggio([294, 440, 392], .16, {dur: .5, gain: .06, type: "triangle"}),
};
function playSound(name) {
  try { GAME_SOUNDS[name]?.(); } catch (_) {}
}
function setSoundEnabled(on) {
  soundEnabled = on;
  try { localStorage.setItem(SOUND_PREFERENCE_KEY, on ? "on" : "off"); } catch (_) {}
  if (on) playSound("click");
  else if (audioCtx) audioCtx.suspend().catch(() => {});
}
function toggleSound() {
  setSoundEnabled(!soundEnabled);
  drawCivTop();
}
const soundButtonLabel = () => soundEnabled ? "Mute sound" : "Turn sound on";
// A click on any control in a game ticks. The canvas stays quiet: selecting
// tiles is not worth a sound, and it would never stop.
document.addEventListener("click", event => {
  if (!state || SPEC) return;
  if (event.target.closest?.("button, summary, [role=button]")) playSound("click");
}, true);

// ---------------------------------------------------------------- moments
// One line per kind of payoff, in the order they deserve a player's attention
// when a turn has more than it can show.
const MOMENT_KINDS = [
  {kicker: "A new age", test: /^(.+) entered the (.+) era$/i, sound: "era", big: true,
   name: match => `The ${titleCase(match[2])} Era`, category: "General"},
  {kicker: "Wonder completed", test: /^(.+) completed (.+)$/, sound: "wonder", big: true,
   name: match => match[2], category: "Cities", place: true},
  {kicker: "City founded", test: /^(.+) founded (.+)$/, sound: "city",
   name: match => match[2], category: "Cities", place: true},
  {kicker: "City captured", test: /^(.+) (?:captured|took over) (.+) from (.+)$/, sound: "war",
   name: match => match[2], category: "War", place: true, big: true},
  {kicker: "Great person", test: /^(.+) recruited (.+)$/, sound: "person",
   name: match => match[2], category: "People", place: true},
  {kicker: "Technology researched", test: /^(.+) researched (.+)$/, sound: "research",
   name: match => match[2], category: "Science", unlocks: "techs"},
  {kicker: "Civic completed", test: /^(.+) adopted (.+)$/, sound: "civic",
   name: match => match[2], category: "Culture", unlocks: "civics"},
];
function momentUnlocks(table, name) {
  const spec = Object.entries(RULES?.[table] || {})
    .find(([id]) => titleCase(id).toLowerCase() === name.toLowerCase())?.[1];
  const list = (spec?.unlocks || []).map(entry => titleCase(entry.id)).slice(0, 4);
  return list.length ? `Unlocks ${list.join(", ")}` : "";
}
function momentFrom(event) {
  if (event.player !== 0) return null;
  for (const [rank, kind] of MOMENT_KINDS.entries()) {
    if (kind.category !== event.category) continue;
    const match = String(event.text || "").match(kind.test);
    if (!match) continue;
    // One sentence is sent to both sides of a capture, and it reads very
    // differently from the other one.
    const lost = kind.category === "War" && match[1] !== state.players?.[0]?.civ;
    return {rank, kicker: lost ? "City lost" : kind.kicker, sound: kind.sound, big: !!kind.big,
      name: kind.name(match), note: kind.unlocks ? momentUnlocks(kind.unlocks, match[2]) : "",
      pos: kind.place ? event.pos : null};
  }
  return null;
}
// Who the player has met and is fighting. Neither arrives as an event of its
// own: both are facts that change between one turn and the next, so each turn
// is compared with the last. The first look at a world only takes attendance,
// or loading a save would announce every neighbour again.
let rivalsSeen = null, rivalsWorld = null;
function rivalMoments() {
  const rivals = (state.players || []).filter(player =>
    player.id !== 0 && !player.is_minor && !player.is_barbarian && player.alive !== false);
  const known = new Map(rivals.map(player => [player.id, {met: !!player.met, war: !!player.at_war_with_me}]));
  const before = rivalsWorld === state.seed ? rivalsSeen : null;
  rivalsWorld = state.seed;
  rivalsSeen = known;
  if (!before) return [];
  const moments = [];
  for (const rival of rivals) {
    const now = known.get(rival.id), then = before.get(rival.id);
    if (!then) continue;
    if (now.war !== then.war)
      moments.push({rank: 1, kicker: now.war ? "War" : "Peace", sound: now.war ? "war" : "civic",
        big: now.war, name: `${now.war ? "At war with" : "At peace with"} ${rival.civ}`, note: "", pos: null});
    if (now.met && !then.met)
      moments.push({rank: 3, kicker: "First contact", sound: "contact", big: false,
        name: rival.civ, note: rival.leader ? `Led by ${rival.leader}` : "", pos: null});
  }
  return moments;
}
// A boost lands mid-turn with no event of its own, and it is the best news a
// study can get: a player's scout or quarry just saved them turns of research.
// Each turn is compared with the last, like the rivals above, and the first
// look at a world only takes attendance.
let boostsSeen = null, boostsWorld = null;
function boostMoments() {
  const me = state.me;
  if (!me) return [];
  const held = new Set([...(me.boosted_techs || []).map(name => `tech:${name}`),
    ...(me.boosted_civics || []).map(name => `civic:${name}`)]);
  const before = boostsWorld === state.seed ? boostsSeen : null;
  boostsWorld = state.seed;
  boostsSeen = held;
  if (!before) return [];
  return [...held].filter(id => !before.has(id)).map(id => {
    const [kind, name] = id.split(":");
    return {rank: 4, kicker: kind === "tech" ? "Eureka!" : "Inspiration!",
      sound: kind === "tech" ? "research" : "civic", big: false, name, pos: null,
      note: kind === "tech" ? "A head start on this research" : "A head start on this civic"};
  });
}
const momentQueue = [];
let momentsShowing = 0;
function showNextMoment() {
  const host = document.getElementById("moments");
  if (!host || momentsShowing >= MOMENT_STACK) return;
  const moment = momentQueue.shift();
  if (!moment) return;
  momentsShowing++;
  const card = document.createElement("button");
  card.type = "button";
  card.className = `moment${moment.big ? " moment-big" : ""}`;
  card.innerHTML = `<span class="moment-kicker">${escapeAttr(moment.kicker)}</span>` +
    `<b class="moment-name">${escapeAttr(titleCase(moment.name))}</b>` +
    (moment.note ? `<span class="moment-note">${escapeAttr(moment.note)}</span>` : "");
  const leave = () => {
    if (!card.isConnected) return;
    card.remove();
    momentsShowing--;
    showNextMoment();
  };
  card.onclick = () => {
    if (moment.pos) { centerOn(moment.pos); draw(); }
    leave();
  };
  host.append(card);
  playSound(moment.sound);
  setTimeout(leave, moment.big ? MOMENT_BIG_LIFE_MS : MOMENT_LIFE_MS);
  setTimeout(showNextMoment, 420);
}
// Called once for each turn's fresh events. An agent holding the seat is
// playing dozens of turns a minute, and a banner for each would be a blur.
// The debugger's own view is for reading the game, and keeps the map clear.
function announceMoments(events) {
  if (SPEC || autoplaying || debuggerView()) return;
  const moments = [...events.map(momentFrom).filter(Boolean), ...rivalMoments(), ...boostMoments()]
    .sort((a, b) => a.rank - b.rank).slice(0, MOMENT_PER_TURN);
  momentQueue.push(...moments);
  showNextMoment();
}

// The game in figures. Civ VI closes a game with graphs of how every empire
// grew, and so does this result card: a finished world sends its whole history
// as `game_stats` (one turn axis, a series per civilization), and each
// civilization gets one chart. The charts fill the card in a fixed
// arrangement, four civilizations 2 by 2 and six 3 by 2, so the whole game is
// on screen at once with nothing to scroll. A chart is drawn for the pixels it
// is given: its five yields share a scale with every other chart, so two
// empires compare at a glance, each line ends in its last figure, and under
// them sits a strip of cities on its own axis, whose ceiling is the most
// cities any civilization held. One switch puts every chart on a log scale,
// where an early game is readable beside a late one.
const GAME_STATS_SERIES = [
  // Ordered so neighbours in the key differ most; checked for colour vision
  // deficiency against the dark card. The hues are the game's own yields.
  {key: "science", label: "Science", color: "#3a95e0"},
  {key: "production", label: "Production", color: "#d95926"},
  {key: "culture", label: "Culture", color: "#a874e6"},
  {key: "gold", label: "Gold", color: "#b88c00"},
  {key: "faith", label: "Faith", color: "#c86e9f"},
];
const GAME_STATS_CITIES = "#cbd3ce";
const GAME_STATS_LOG_KEY = "civvis-game-stats-log-v1";
// The size a chart is drawn at before the card has been measured.
const GAME_STATS_UNMEASURED = {w: 340, h: 240};
let gameStatsView = null, gameStatsFrame = null, gameStatsDrawn = "", gameStatsObserver = null;

function gameStatsLogScale() {
  try { return localStorage.getItem(GAME_STATS_LOG_KEY) === "1"; } catch (e) { return false; }
}

// How many charts across and down. Up to eight civilizations take two rows,
// so four sit 2 by 2 and six sit 3 by 2; a bigger table takes a third row and
// then a fourth. A card that is not wider than it is tall turns the
// arrangement on its side, which keeps each chart wider than it is tall.
function gameStatsLayout(count, upright = false) {
  const rows = count <= 3 ? 1 : count <= 8 ? 2 : count <= 15 ? 3 : 4;
  const cols = Math.ceil(count / rows);
  return upright ? {cols: rows, rows: cols} : {cols, rows};
}

// The last turn of the axis a series has a figure for, or -1.
function gameStatsLastPlayed(values) {
  for (let i = (values?.length ?? 0) - 1; i >= 0; i--)
    if (values[i] !== null && values[i] !== undefined) return i;
  return -1;
}

// Called from the result card's markup. Empty for a running game, a Tactics
// battle (its card must not cover the field) and a world too young to graph.
function gameStatsMarkup(st) {
  const stats = st?.game_stats;
  gameStatsView = null;
  if (!stats || !Array.isArray(stats.turns) || stats.turns.length < 2 ||
      !Array.isArray(stats.players) || !stats.players.length ||
      isBattlefieldMapScript(st.map?.script)) return "";
  const winners = new Set(Array.isArray(st.winners) && st.winners.length ? st.winners : [st.winner]);
  const last = stats.turns.length - 1;
  const rows = stats.players.filter(row => st.players?.[row.id]).map(row => {
    const player = st.players[row.id];
    const played = gameStatsLastPlayed(row.cities);
    return {...row, civ: player.civ, color: pcol(row.id), won: winners.has(row.id),
      score: Number(player.score) || 0, wonders: player.wonder_count ?? 0,
      population: player.population ?? 0, held: player.cities ?? 0,
      // A watched table is a contest between Civ AI Agents, so each chart
      // names the one that played it, and its rating.
      by: SPEC && player.ai_username
        ? player.ai_username + (playerHudEloValue(player) === null ? "" : ` ${playerEloLabel(player)}`) : "",
      out: player.alive === false && played >= 0 && played < last ? stats.turns[played] : null};
  }).sort((a, b) => b.won - a.won || b.score - a.score || a.id - b.id);
  gameStatsView = {turns: stats.turns, rows};
  const layout = gameStatsLayout(rows.length);
  const key = GAME_STATS_SERIES.map(series =>
    `<span><i style="--c:${series.color}"></i>${series.label}</span>`).join("") +
    `<span><i class="gs-strip" style="--c:${GAME_STATS_CITIES}"></i>Cities</span>`;
  const charts = gameStatsCharts(gameStatsView, gameStatsLogScale(), GAME_STATS_UNMEASURED.w, GAME_STATS_UNMEASURED.h);
  gameStatsDrawn = "";
  // The card's markup is in the page by the time this runs, and can be measured.
  queueMicrotask(gameStatsMount);
  const count = (n, one, many) => `${n.toLocaleString()} ${n === 1 ? one : many}`;
  return `<section class="game-stats${layout.rows === 1 ? " game-stats-one-row" : ""}" aria-label="Game stats">
    <div class="game-stats-head">
      <span class="game-stats-title">Game stats <small>per turn</small></span>
      <span class="game-stats-key" aria-label="Key">${key}</span>
      <label class="game-stats-log"><input type="checkbox" id="game-stats-log"
        ${gameStatsLogScale() ? "checked" : ""} onchange="setGameStatsLogScale(this.checked)">Log scale</label>
    </div>
    <div class="game-stats-grid" style="--gs-cols:${layout.cols};--gs-rows:${layout.rows}">${rows.map((row, i) =>
      `<figure class="game-stats-civ" style="--civ:${row.color}" data-player="${row.id}">
        <figcaption><span class="gs-civ">${escapeAttr(row.civ)}</span>${
          row.by ? `<span class="gs-by">${escapeAttr(row.by)}</span>` : ``}${
          row.won ? `<span class="gs-won">Winner</span>` : row.out !== null ? `<span class="gs-out">Out on turn ${row.out}</span>` : ``}
          <small><span>Score ${Math.round(row.score).toLocaleString()}</span><span>${count(row.held, "city", "cities")}</span><span>${
            row.population.toLocaleString()} pop</span><span>${count(row.wonders, "wonder", "wonders")}</span></small></figcaption>
        <div class="game-stats-plot">${charts[i]}</div>
      </figure>`).join("")}</div>
    <div class="game-stats-tip" hidden></div>
  </section>`;
}

// Watches the chart grid, so the charts are drawn for the card as it first
// appears and again whenever the window gives it a different size.
function gameStatsMount() {
  const grid = document.querySelector("#winner .game-stats-grid");
  if (!grid || !gameStatsView) return;
  if (typeof ResizeObserver !== "function") { gameStatsFit(); return; }
  if (!gameStatsObserver) gameStatsObserver = new ResizeObserver(gameStatsFit);
  gameStatsObserver.disconnect();
  gameStatsObserver.observe(grid);
}

// Settles the arrangement for the grid's shape and redraws every chart at the
// size one cell came to. Nothing is drawn while the card is hidden, or twice
// for one size.
function gameStatsFit() {
  const grid = document.querySelector("#winner .game-stats-grid"), view = gameStatsView;
  if (!grid || !view || !grid.clientWidth || !grid.clientHeight) return;
  const layout = gameStatsLayout(view.rows.length, grid.clientWidth < grid.clientHeight * 1.1);
  grid.style.setProperty("--gs-cols", layout.cols);
  grid.style.setProperty("--gs-rows", layout.rows);
  const plots = [...grid.querySelectorAll(".game-stats-plot")];
  const w = plots[0]?.clientWidth || 0, h = plots[0]?.clientHeight || 0, log = gameStatsLogScale();
  const drawn = `${w}x${h}|${log}`;
  if (w < 60 || h < 60 || drawn === gameStatsDrawn) return;
  gameStatsDrawn = drawn;
  const charts = gameStatsCharts(view, log, w, h);
  plots.forEach((plot, i) => { plot.innerHTML = charts[i] || ""; });
}

function setGameStatsLogScale(log) {
  try { localStorage.setItem(GAME_STATS_LOG_KEY, log ? "1" : "0"); } catch (e) {}
  gameStatsDrawn = "";
  gameStatsFit();
}

// A round number at or above `value`: 1, 2, 2.5 or 5 times a power of ten.
function gameStatsNiceCeiling(value) {
  if (!(value > 0)) return 1;
  const power = 10 ** Math.floor(Math.log10(value));
  return power * ([1, 2, 2.5, 5, 10].find(step => step * power >= value) ?? 10);
}

// The gap between two rules for a span of `value`: the same round numbers,
// and whole ones, because every figure drawn is whole.
function gameStatsStep(value) {
  const step = gameStatsNiceCeiling(value);
  return step < 10 ? Math.ceil(step) : step;
}

// Where a chart `w` by `h` pixels puts its parts: the yields plot, a gap that
// holds the strip's own ceiling label, the cities strip, then the turn labels.
// The left margin fits the widest axis label and the right one a line's last
// figure.
function gameStatsBox(w, h, widest) {
  const font = w >= 620 && h >= 340 ? 12 : w >= 430 && h >= 220 ? 11 : 10;
  const left = Math.max(22, Math.ceil(widest * font * 0.6) + 8), right = w - Math.ceil(font * 3.2);
  const axis = font + 6, strip = Math.round(Math.min(48, Math.max(18, h * 0.14))), gap = font + 8;
  const top = Math.ceil(font / 2) + 2, stripTop = h - axis - strip;
  return {w, h, font, left, right, top, bottom: stripTop - gap, stripTop, stripBottom: stripTop + strip};
}

// One scale for every chart, so the same height means the same amount in each.
function gameStatsScales(view, log, w, h) {
  let high = 1, low = 0, cities = 1;
  for (const row of view.rows) {
    for (const series of GAME_STATS_SERIES)
      for (const value of row[series.key] || []) if (value !== null) {
        if (value > high) high = value;
        if (value < low) low = value;
      }
    for (const value of row.cities || []) if (value !== null && value > cities) cities = value;
  }
  const t0 = view.turns[0], t1 = view.turns[view.turns.length - 1];
  // Enough rules to read a height from, never so many that their labels
  // crowd. The plot's height does not depend on how wide its labels are.
  const rough = gameStatsBox(w, h, 3);
  const wanted = Math.min(6, Math.max(2, Math.floor((rough.bottom - rough.top) / 32)));
  let top, bottom = 0, ticks = [];
  if (log) {
    // A little air over the highest figure; the rules are the powers of ten
    // and, where there is room, the threes between them.
    top = high * 1.12;
    for (let tick = 1; tick <= top; tick *= 10) ticks.push(tick);
    if (wanted >= 3) for (let tick = 3; tick <= top; tick *= 10) ticks.push(tick);
  } else {
    // The ceiling is the first half step clear of the highest figure, so the
    // lines take the plot's height and no step is spent on empty space.
    const step = gameStatsStep(high / wanted), half = step / 2;
    top = Math.max(step, Math.ceil(high * 1.02 / half) * half);
    // A deficit is rarely deep, so the floor is its own round number and not
    // a whole step below zero.
    bottom = low < 0 ? -gameStatsNiceCeiling(-low) : 0;
    for (let tick = Math.ceil(bottom / step) * step; tick <= top; tick += step) ticks.push(tick);
    if (!ticks.includes(bottom)) ticks.push(bottom);
  }
  const cityTicks = log ? [1, cities] : [0, cities];
  const box = gameStatsBox(w, h, Math.max(...[...ticks, ...cityTicks].map(tick => compactStat(tick).length)));
  const x = turn => box.left + (box.right - box.left) * (turn - t0) / Math.max(1, t1 - t0);
  // Log: anything under 1 (a deficit, a turn of anarchy) sits on the floor.
  const logY = (value, ceiling, y0, y1) =>
    y1 - (y1 - y0) * Math.log10(Math.max(1, value)) / Math.log10(Math.max(2, ceiling));
  return {box, x, t0, t1, cities, ticks, cityTicks, bottom,
    y: log ? value => logY(value, top, box.top, box.bottom)
      : value => box.bottom - (box.bottom - box.top) * (value - bottom) / (top - bottom),
    cy: log ? value => logY(value, cities, box.stripTop, box.stripBottom)
      : value => box.stripBottom - (box.stripBottom - box.stripTop) * value / cities};
}

// The turns to rule and label: round numbers far enough apart to read, clear
// of the first and last turn's own labels.
function gameStatsTurnTicks(scale) {
  const {box, x, t0, t1} = scale;
  const room = box.font * 5.4;
  const step = [1, 2, 5, 10, 20, 25, 50, 100, 200, 250, 500, 1000]
    .find(size => (box.right - box.left) * size / Math.max(1, t1 - t0) >= room);
  const ticks = [];
  if (step) for (let turn = Math.ceil((t0 + 1) / step) * step; turn < t1; turn += step)
    if (x(turn) - box.left >= room && box.right - x(turn) >= room * 0.7) ticks.push(turn);
  return ticks;
}

// A line through the turns a civilization played, broken where it did not.
function gameStatsPath(turns, values, x, y) {
  let path = "", open = false;
  for (let i = 0; i < turns.length; i++) {
    const value = values?.[i];
    if (value === null || value === undefined) { open = false; continue; }
    path += `${open ? "L" : "M"}${x(turns[i]).toFixed(1)} ${y(value).toFixed(1)}`;
    open = true;
  }
  return path;
}

// Every civilization's chart as SVG, `w` by `h` pixels each, in the view's order.
function gameStatsCharts(view, log, w, h) {
  const scale = gameStatsScales(view, log, w, h), box = scale.box, turns = view.turns;
  gameStatsFrame = {left: box.left, right: box.right};
  const rule = (y, label, kind = "") =>
    `<line class="gs-grid${kind}" x1="${box.left}" x2="${box.right}" y1="${y.toFixed(1)}" y2="${y.toFixed(1)}"/>` +
    (label === null ? "" : `<text x="${box.left - 5}" y="${(y + box.font * 0.35).toFixed(1)}" text-anchor="end">${label}</text>`);
  // Zero and the ceiling are labelled first, and a rule whose label would
  // touch one already written is drawn without it.
  const written = [];
  const yields = [...scale.ticks].sort((a, b) => (b === 0) - (a === 0) || b - a).map(tick => {
    const y = scale.y(tick), clear = written.every(other => Math.abs(other - y) >= box.font + 2);
    if (clear) written.push(y);
    return rule(y, clear ? compactStat(tick) : null, tick === 0 && scale.bottom < 0 ? " gs-zero" : "");
  }).join("");
  const labelY = box.h - 3;
  // The strip says what it counts, in the corner an empire grows away from.
  const grid = yields + scale.cityTicks.map(tick => rule(scale.cy(tick), compactStat(tick))).join("") +
    `<text class="gs-name" x="${box.left + 5}" y="${(box.stripTop + box.font * 0.85).toFixed(1)}">Cities</text>` +
    gameStatsTurnTicks(scale).map(turn => { const x = scale.x(turn).toFixed(1); return (
      `<line class="gs-grid gs-turn" x1="${x}" x2="${x}" y1="${box.top}" y2="${box.stripBottom}"/>` +
      `<text x="${x}" y="${labelY}" text-anchor="middle">${turn}</text>`); }).join("") +
    `<text x="${box.left}" y="${labelY}">Turn ${scale.t0}</text>` +
    `<text x="${box.right}" y="${labelY}" text-anchor="end">${scale.t1}</text>`;
  return view.rows.map(row => {
    // The strip is filled down to its floor so a count reads as an amount.
    const area = gameStatsArea(turns, row.cities, scale, scale.cy(log ? 1 : 0).toFixed(1));
    const lines = GAME_STATS_SERIES.map(series =>
      `<path class="gs-line" stroke="${series.color}" d="${gameStatsPath(turns, row[series.key], scale.x, scale.y)}"/>`).join("");
    const ends = gameStatsEnds(turns, row, scale);
    const summary = ends.filter(end => end.label).map(end => `${end.label} ${end.text}`).join(", ");
    return `<svg class="game-stats-chart" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" role="img"
      style="font-size:${box.font}px"
      aria-label="${escapeAttr(`${row.civ}, turns ${scale.t0} to ${scale.t1}. Last turn played: ${summary}.`)}">
      ${grid}${area}${lines}${ends.map(end =>
        `<circle class="gs-end" cx="${end.x.toFixed(1)}" cy="${end.at.toFixed(1)}" r="2.5" fill="${end.color}"/>` +
        `<text class="gs-last" x="${(end.x + 6).toFixed(1)}" y="${(end.y + box.font * 0.35).toFixed(1)}" fill="${end.color}">${end.text}</text>`).join("")}
      <line class="gs-cursor" x1="0" x2="0" y1="${box.top}" y2="${box.stripBottom}" visibility="hidden"/>
    </svg>`;
  });
}

// The last figure of each line and of the cities strip, written where it
// ends: beside the last turn, or beside the turn a fallen civilization left
// on. Figures that would overprint are moved apart, kept in their order.
function gameStatsEnds(turns, row, scale) {
  const box = scale.box, room = box.font + 1;
  const end = (values, color, label, y) => {
    const i = gameStatsLastPlayed(values);
    if (i < 0) return null;
    const at = y(values[i]);
    return {label, color, text: compactStat(values[i]), x: scale.x(turns[i]), at, y: at};
  };
  const ends = GAME_STATS_SERIES.map(series => end(row[series.key], series.color, series.label, scale.y))
    .filter(Boolean).sort((a, b) => a.y - b.y);
  for (let i = 1; i < ends.length; i++) ends[i].y = Math.max(ends[i].y, ends[i - 1].y + room);
  const floor = box.bottom + 2;
  for (let i = ends.length - 1; i >= 0; i--)
    ends[i].y = Math.min(ends[i].y, i === ends.length - 1 ? floor : ends[i + 1].y - room);
  const cities = end(row.cities, GAME_STATS_CITIES, "Cities", scale.cy);
  return cities ? [...ends, cities] : ends;
}

function gameStatsArea(turns, values, scale, floor) {
  let area = "", run = [];
  const close = () => {
    if (run.length) area += `M${run[0][0]} ${floor}` + run.map(([x, y]) => `L${x} ${y}`).join("") +
      `L${run[run.length - 1][0]} ${floor}Z`;
    run = [];
  };
  for (let i = 0; i < turns.length; i++) {
    const value = values?.[i];
    if (value === null || value === undefined) { close(); continue; }
    run.push([scale.x(turns[i]).toFixed(1), scale.cy(value).toFixed(1)]);
  }
  close();
  return area ? `<path class="gs-cities" d="${area}"/>` : "";
}

// Hover: one turn cursor on every chart, so the same turn reads across the
// civilizations, and that turn's figures for the chart under the pointer.
function gameStatsHover(event) {
  const view = gameStatsView, frame = gameStatsFrame, section = document.querySelector("#winner .game-stats");
  const tip = section?.querySelector(".game-stats-tip");
  const chart = event.target.closest?.(".game-stats-chart");
  const cursors = document.querySelectorAll("#winner .gs-cursor");
  const row = view && chart && view.rows.find(entry => String(entry.id) === chart.closest("figure")?.dataset.player);
  if (!row || !tip || !frame) {
    cursors.forEach(line => line.setAttribute("visibility", "hidden"));
    if (tip) tip.hidden = true;
    return;
  }
  const rect = chart.getBoundingClientRect();
  const t0 = view.turns[0], t1 = view.turns[view.turns.length - 1];
  const fraction = (event.clientX - rect.left - frame.left) / (frame.right - frame.left);
  const turn = t0 + Math.min(1, Math.max(0, fraction)) * (t1 - t0);
  let index = 0;
  for (let i = 1; i < view.turns.length; i++)
    if (Math.abs(view.turns[i] - turn) < Math.abs(view.turns[index] - turn)) index = i;
  const x = (frame.left + (frame.right - frame.left) * (view.turns[index] - t0) / Math.max(1, t1 - t0)).toFixed(1);
  cursors.forEach(line => {
    line.setAttribute("x1", x); line.setAttribute("x2", x); line.setAttribute("visibility", "visible");
  });
  const figure = value => value === null || value === undefined ? "—" : Math.round(value).toLocaleString();
  tip.innerHTML = `<b>${escapeAttr(row.civ)} · Turn ${view.turns[index]}</b>` +
    GAME_STATS_SERIES.map(series =>
      `<span><em><i style="--c:${series.color}"></i>${series.label}</em>${figure(row[series.key]?.[index])}</span>`).join("") +
    `<span><em><i class="gs-strip" style="--c:${GAME_STATS_CITIES}"></i>Cities</em>${figure(row.cities?.[index])}</span>`;
  tip.hidden = false;
  const host = section.getBoundingClientRect();
  const left = event.clientX - host.left + 14, width = tip.offsetWidth, height = tip.offsetHeight;
  tip.style.left = `${Math.max(0, left + width > host.width ? left - width - 28 : left)}px`;
  tip.style.top = `${Math.max(0, Math.min(host.height - height, event.clientY - host.top - height / 2))}px`;
}
document.getElementById("winner")?.addEventListener("pointermove", gameStatsHover);
document.getElementById("winner")?.addEventListener("pointerleave", gameStatsHover);
