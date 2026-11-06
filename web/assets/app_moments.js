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
const MOMENT_STACK = 3;      // on screen at once; the rest wait their turn
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
function tone(freq, {at = 0, dur = .3, gain = .1, type = "sine", to = 0} = {}) {
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
  notes.forEach((freq, i) => tone(freq, {...options, at: i * step}));
const GAME_SOUNDS = {
  click: () => tone(1320, {dur: .05, gain: .025, type: "triangle"}),
  endTurn: () => { tone(196, {dur: .45, gain: .1}); tone(294, {dur: .45, gain: .06, at: .04}); },
  newTurn: () => arpeggio([392, 587], .09, {dur: .32, gain: .06, type: "triangle"}),
  research: () => arpeggio([523, 659, 784], .08, {dur: .38, gain: .07, type: "triangle"}),
  civic: () => arpeggio([440, 554, 740], .09, {dur: .42, gain: .06}),
  city: () => { for (const freq of [262, 330, 392]) tone(freq, {dur: .8, gain: .06, type: "triangle"}); },
  person: () => arpeggio([659, 880, 1319], .1, {dur: .6, gain: .05}),
  wonder: () => {
    for (const freq of [196, 294, 392]) tone(freq, {dur: 1.6, gain: .06, type: "triangle"});
    arpeggio([587, 784, 988, 1175], .14, {dur: .9, gain: .05, at: .2});
  },
  era: () => {
    for (const freq of [131, 196, 262]) tone(freq, {dur: 2, gain: .07, type: "sawtooth"});
    arpeggio([392, 523, 659, 784], .18, {dur: 1, gain: .05, type: "triangle"});
  },
  war: () => { thud(); thud({at: .24}); tone(98, {dur: .7, gain: .1, type: "sawtooth", to: 73}); },
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
function announceMoments(events) {
  if (SPEC || autoplaying) return;
  const moments = [...events.map(momentFrom).filter(Boolean), ...rivalMoments()]
    .sort((a, b) => a.rank - b.rank).slice(0, MOMENT_PER_TURN);
  momentQueue.push(...moments);
  showNextMoment();
}
