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
// civilization gets one chart. Its five yields share a scale with every other
// chart, so two empires compare at a glance; under them a strip of cities on
// its own axis, whose ceiling is the most cities any civilization held. One
// switch puts every chart on a log scale, where an early game is readable
// beside a late one.
const GAME_STATS_SERIES = [
  // Ordered so neighbours in the key differ most; checked for colour vision
  // deficiency against the dark card. The hues are the game's own yields.
  {key: "science", label: "Science", color: "#3a95e0"},
  {key: "production", label: "Production", color: "#d95926"},
  {key: "culture", label: "Culture", color: "#a874e6"},
  {key: "gold", label: "Gold", color: "#b88c00"},
  {key: "faith", label: "Faith", color: "#c86e9f"},
];
const GAME_STATS_LOG_KEY = "civvis-game-stats-log-v1";
// Chart geometry in viewBox units: the yields plot, a gap, the cities strip,
// then the turn labels under it.
const GAME_STATS_BOX = {w: 300, left: 34, right: 8, top: 6, plot: 92, gap: 17, strip: 26, axis: 13};
let gameStatsView = null;

function gameStatsLogScale() {
  try { return localStorage.getItem(GAME_STATS_LOG_KEY) === "1"; } catch (e) { return false; }
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
  const score = id => Number(st.players[id]?.score) || 0;
  const rows = stats.players.filter(row => st.players?.[row.id])
    .map(row => ({...row, civ: st.players[row.id].civ, color: pcol(row.id), won: winners.has(row.id),
      score: score(row.id), wonders: st.players[row.id].wonder_count ?? 0}))
    .sort((a, b) => b.won - a.won || b.score - a.score || a.id - b.id);
  gameStatsView = {turns: stats.turns, rows};
  const key = GAME_STATS_SERIES.map(series =>
    `<span><i style="--c:${series.color}"></i>${series.label}</span>`).join("") +
    `<span><i class="gs-strip" style="--c:#cbd3ce"></i>Cities</span>`;
  return `<section class="game-stats" aria-label="Game stats">
    <div class="game-stats-head">
      <span class="game-stats-title">Game stats <small>per turn</small></span>
      <span class="game-stats-key" aria-label="Key">${key}</span>
      <label class="game-stats-log"><input type="checkbox" id="game-stats-log"
        ${gameStatsLogScale() ? "checked" : ""} onchange="setGameStatsLogScale(this.checked)">Log scale</label>
    </div>
    <div class="game-stats-grid">${gameStatsCharts(gameStatsView, gameStatsLogScale())}</div>
    <div class="game-stats-tip" hidden></div>
  </section>`;
}

function setGameStatsLogScale(log) {
  try { localStorage.setItem(GAME_STATS_LOG_KEY, log ? "1" : "0"); } catch (e) {}
  const grid = document.querySelector("#winner .game-stats-grid");
  if (grid && gameStatsView) grid.innerHTML = gameStatsCharts(gameStatsView, log);
}

// A round number at or above `value`: 1, 2, 2.5 or 5 times a power of ten.
function gameStatsNiceCeiling(value) {
  if (!(value > 0)) return 1;
  const power = 10 ** Math.floor(Math.log10(value));
  return power * ([1, 2, 2.5, 5, 10].find(step => step * power >= value) ?? 10);
}

// One scale for every chart, so the same height means the same amount in each.
function gameStatsScales(view, log) {
  let high = 1, low = 0, cities = 1;
  for (const row of view.rows) {
    for (const series of GAME_STATS_SERIES)
      for (const value of row[series.key] || []) if (value !== null) {
        if (value > high) high = value;
        if (value < low) low = value;
      }
    for (const value of row.cities || []) if (value !== null && value > cities) cities = value;
  }
  const box = GAME_STATS_BOX;
  const t0 = view.turns[0], t1 = view.turns[view.turns.length - 1];
  const x = turn => box.left + (box.w - box.left - box.right) * (turn - t0) / Math.max(1, t1 - t0);
  const stripTop = box.top + box.plot + box.gap;
  // Log: anything under 1 (a deficit, a turn of anarchy) sits on the floor.
  const logY = (value, top, height, ceiling) =>
    top + height * (1 - Math.log10(Math.max(1, value)) / Math.log10(Math.max(2, ceiling)));
  if (log) {
    const ticks = [];
    for (let tick = 1; tick <= high; tick *= 10) ticks.push(tick);
    return {x, cities, stripTop, ticks, cityTicks: [1, cities],
      y: value => logY(value, box.top, box.plot, high),
      cy: value => logY(value, stripTop, box.strip, cities)};
  }
  const top = gameStatsNiceCeiling(high), bottom = low < 0 ? -gameStatsNiceCeiling(-low) : 0;
  const ticks = bottom < 0 ? [bottom, 0, top] : [0, top / 2, top];
  return {x, cities, stripTop, ticks, cityTicks: [0, cities],
    y: value => box.top + box.plot * (1 - (value - bottom) / (top - bottom)),
    cy: value => stripTop + box.strip * (1 - value / cities)};
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

function gameStatsCharts(view, log) {
  const box = GAME_STATS_BOX, scale = gameStatsScales(view, log);
  const height = scale.stripTop + box.strip + box.axis;
  const right = box.w - box.right, t0 = view.turns[0], t1 = view.turns[view.turns.length - 1];
  const rule = (y, label) => `<line class="gs-grid" x1="${box.left}" x2="${right}" y1="${y.toFixed(1)}" y2="${y.toFixed(1)}"/>` +
    `<text x="${box.left - 4}" y="${(y + 3).toFixed(1)}" text-anchor="end">${label}</text>`;
  const grid = scale.ticks.map(tick => rule(scale.y(tick), compactStat(tick))).join("") +
    scale.cityTicks.map(tick => rule(scale.cy(tick), compactStat(tick))).join("") +
    `<text x="${box.left}" y="${height - 2}">Turn ${t0}</text>` +
    `<text x="${right}" y="${height - 2}" text-anchor="end">${t1}</text>`;
  return view.rows.map(row => {
    // The strip is filled down to its floor so a count reads as an amount.
    const area = gameStatsArea(view.turns, row.cities, scale, scale.cy(log ? 1 : 0).toFixed(1));
    const lines = GAME_STATS_SERIES.map(series =>
      `<path class="gs-line" stroke="${series.color}" d="${gameStatsPath(view.turns, row[series.key], scale.x, scale.y)}"/>`).join("");
    const peak = Math.max(0, ...(row.cities || []).filter(value => value !== null));
    const last = series => [...(row[series.key] || [])].reverse().find(value => value !== null);
    const summary = GAME_STATS_SERIES.map(series => `${series.label} ${compactStat(last(series) ?? 0)}`).join(", ");
    return `<figure class="game-stats-civ" style="--civ:${row.color}" data-player="${row.id}">
      <figcaption><span class="gs-civ">${escapeAttr(row.civ)}</span>${row.won ? `<span class="gs-won">Winner</span>` : ``}
        <small>Score ${Math.round(row.score).toLocaleString()} · ${peak} ${peak === 1 ? "city" : "cities"} at most · ${row.wonders} ${row.wonders === 1 ? "wonder" : "wonders"}</small></figcaption>
      <svg class="game-stats-chart" viewBox="0 0 ${box.w} ${height}" role="img"
        aria-label="${escapeAttr(`${row.civ}, turns ${t0} to ${t1}. Last turn: ${summary}. Most cities: ${peak}.`)}">
        ${grid}${area}${lines}
        <line class="gs-cursor" x1="0" x2="0" y1="${box.top}" y2="${scale.stripTop + box.strip}" visibility="hidden"/>
      </svg>
    </figure>`;
  }).join("");
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

// Hover: a turn cursor on the chart and that turn's figures beside it.
function gameStatsHover(event) {
  const view = gameStatsView, section = event.target.closest?.(".game-stats");
  const tip = section?.querySelector(".game-stats-tip");
  const chart = event.target.closest?.(".game-stats-chart");
  document.querySelectorAll("#winner .gs-cursor").forEach(line => {
    if (!chart || !chart.contains(line)) line.setAttribute("visibility", "hidden");
  });
  if (!view || !tip) return;
  const row = chart && view.rows.find(entry => String(entry.id) === chart.closest("figure")?.dataset.player);
  if (!row) { tip.hidden = true; return; }
  const box = GAME_STATS_BOX, rect = chart.getBoundingClientRect();
  const t0 = view.turns[0], t1 = view.turns[view.turns.length - 1];
  const fraction = ((event.clientX - rect.left) / rect.width * box.w - box.left) / (box.w - box.left - box.right);
  const turn = t0 + Math.min(1, Math.max(0, fraction)) * (t1 - t0);
  let index = 0;
  for (let i = 1; i < view.turns.length; i++)
    if (Math.abs(view.turns[i] - turn) < Math.abs(view.turns[index] - turn)) index = i;
  const x = (box.left + (box.w - box.left - box.right) * (view.turns[index] - t0) / Math.max(1, t1 - t0)).toFixed(1);
  const cursor = chart.querySelector(".gs-cursor");
  cursor.setAttribute("x1", x); cursor.setAttribute("x2", x); cursor.setAttribute("visibility", "visible");
  const figure = value => value === null || value === undefined ? "—" : Math.round(value).toLocaleString();
  tip.innerHTML = `<b>${escapeAttr(row.civ)} · Turn ${view.turns[index]}</b>` +
    GAME_STATS_SERIES.map(series =>
      `<span><em><i style="--c:${series.color}"></i>${series.label}</em>${figure(row[series.key]?.[index])}</span>`).join("") +
    `<span><em><i class="gs-strip" style="--c:#cbd3ce"></i>Cities</em>${figure(row.cities?.[index])}</span>`;
  tip.hidden = false;
  const host = section.getBoundingClientRect();
  const left = event.clientX - host.left + 14, width = tip.offsetWidth;
  tip.style.left = `${Math.max(0, left + width > host.width ? left - width - 28 : left)}px`;
  tip.style.top = `${Math.max(0, event.clientY - host.top - tip.offsetHeight / 2)}px`;
}
document.getElementById("winner")?.addEventListener("pointermove", gameStatsHover);
document.getElementById("winner")?.addEventListener("pointerleave", gameStatsHover);
