"use strict";
// Live time: the turn plate's stopwatch, carved out of app.js.
//
// ⚠ THIS SCRIPT IS LOADED BEFORE app.js, for the reason app_palette.js gives
// at length: every script the page loads shares one global scope, and a name
// moved EARLIER can never be read before its declaration has run, where one
// moved later can. The one thing that runs at load is the half-second tick,
// which looks for the plate's value and finds none until app.js has drawn
// one. Everything else is called by app.js once a world is on screen, so the
// renderer's own names these lean on (`state`, `SPEC`, `specPaused`,
// `livePause`, `gameFinished`, `turnGauge`) exist by the time they are read.
//
// The stopwatch is the turn plate's top section, above the age. It
// reads how long the world on screen has actually been playing: it starts
// with the world's first unpaused frame, stops while the simulation is paused
// or a result is on screen, and a new world starts it over from zero. A
// restart that comes back paused at the opening position therefore sits at
// 0.00 until somebody presses Start — an unstarted game has no live time.
let liveClock = {seed: undefined, ms: 0, runningSince: null};
const LIVE_CLOCK_STORE = "civvis-live-time-v1";
function liveClockMs() {
  return liveClock.ms +
    (liveClock.runningSince !== null ? Math.max(0, Date.now() - liveClock.runningSince) : 0);
}
// One reading of the clock, split where the plate splits it: the whole seconds
// are the value's words and the hundredths of a second follow them (operator
// 2026-11-21: "add hundredths of a second to the live time count").
//
// The reading is seconds and hundredths, `12.34`, and grows a part only when
// it has one to show: minutes from 60 seconds, `1:00.00`, and hours from 60
// minutes (operator 2026-11-21: "live time can just default to second and
// hundredths of a second. if we need minutes, we can add that in at 60s").
// `spoken` is the same whole seconds for a screen reader, which is told the
// unit a bare number under a minute does not carry.
function liveClockReading() {
  const ms = liveClockMs();
  const total = Math.floor(ms / 1000);
  const two = part => String(part).padStart(2, "0");
  const minutes = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  const text = hours ? `${hours}:${two(minutes)}:${two(total % 60)}`
    : minutes ? `${minutes}:${two(total % 60)}` : `${total}`;
  return {
    text,
    spoken: total < 60 ? `${total} second${total === 1 ? "" : "s"}` : text,
    hundredths: two(Math.floor(ms / 10) % 100),
  };
}
// Start or stop the accumulator against what the page is showing right now.
// Paused covers the unstarted opening as well: a clock that has never run
// and is not running reads 0.00, which is what an unstarted game has played.
// On the mirror beside a Civilization VI verification game this page's own
// simulation never runs — the world is a picture of the Firaxis game — so the
// clock follows that game instead: it runs while the game plays and stops
// while the operator's Pause holds it (`livePause.held`, the mod's own word).
function syncLiveClockRun() {
  const over = gameFinished(state);
  const paused = livePause.available ? livePause.held : specPaused;
  const shouldRun = SPEC && !!state && !paused && !over;
  if (shouldRun && liveClock.runningSince === null) {
    liveClock.runningSince = Date.now();
  } else if (!shouldRun && liveClock.runningSince !== null) {
    liveClock.ms += Math.max(0, Date.now() - liveClock.runningSince);
    liveClock.runningSince = null;
  }
}
function syncLiveClock(st, newWorld) {
  if (newWorld) {
    // A page's first world may be the one it was already showing — a reload,
    // or a supervised binary swap resuming the same map — so the first world
    // resumes the reading the predecessor document banked. Every later new
    // world is a restart and starts over from zero.
    if (liveClock.seed === undefined) restoreLiveClock(st);
    else { liveClock.ms = 0; liveClock.runningSince = null; }
    liveClock.seed = st.seed ?? null;
    // The game's other clock, the gold bar up the victory tracker, starts
    // over with it.
    settleTurnGauge();
  }
  syncLiveClockRun();
  saveLiveClock(st);
}
// The victory tracker's gold turn bar (`turnGauge` in app.js) glides to each
// new turn, and so a new game began with the bar gliding down, from wherever
// the last game ended to the foot. A new game's bar starts where that game
// is (operator, 2026-11-24: "on civvis have gold bar immediately start at
// 1/250 rather than a motion down", "between games that is").
//
// This runs when the world on screen has become a new one and before the
// tracker is redrawn for it, so the bar in the document still stands at the
// last world's turn. It is put at this world's turn with its motion held off
// for that one change (`turn-gauge-settling` in index.html, and a reading of
// the layout so the turn is styled while it holds). The redraw that follows
// finds the bar where it belongs and has nothing to move, and every turn
// after that glides up as before.
//
// A new world is what the page takes for one everywhere else: another game,
// or the same game wound back. So the bar never glides down. A world with no
// turn cap draws no bar, and the redraw takes the old one away.
function settleTurnGauge() {
  const gauge = document.querySelector("#victoryhud .turn-gauge");
  const rise = gauge && /--rise:([\d.]+%)/.exec(turnGauge())?.[1];
  if (!rise) return;
  gauge.classList.add("turn-gauge-settling");
  gauge.style.setProperty("--rise", rise);
  void gauge.offsetHeight;
  gauge.classList.remove("turn-gauge-settling");
}
function saveLiveClock(st) {
  try {
    sessionStorage.setItem(LIVE_CLOCK_STORE, JSON.stringify({
      seed: st.seed ?? null, turn: st.turn ?? 0, wall: Date.now(),
      ms: liveClockMs(), running: liveClock.runningSince !== null,
    }));
  } catch (e) {}
}
function restoreLiveClock(st) {
  liveClock.ms = 0;
  liveClock.runningSince = null;
  let stored = null;
  try { stored = JSON.parse(sessionStorage.getItem(LIVE_CLOCK_STORE)); } catch (e) {}
  // The banked reading belongs to this world only while the world is the same
  // one and has not been wound back: a same-seed restart returns to turn 1,
  // and its clock must return to zero with it.
  if (!stored || stored.seed !== (st.seed ?? null) || stored.turn > (st.turn ?? 0)) return;
  liveClock.ms = Math.max(0, Number(stored.ms) || 0);
  // The world kept playing while no document watched it, so the time away is
  // live time too — but only when the clock was running on both sides of the
  // gap; a pause of unknown length inside it would otherwise be counted.
  if (stored.running && st.paused === false && !gameFinished(st))
    liveClock.ms += Math.max(0, Date.now() - (Number(stored.wall) || Date.now()));
}
// The plate is only rebuilt when a frame changes it, which at a slow watch
// pace is many seconds apart; the clock ticks between those rebuilds by
// repainting its one value in place, the way the restart countdown does.
//
// The hundredths are an attribute the stylesheet prints after the seconds,
// not words in the plate. Every change of words inside the masthead is
// answered by the type fitter, which measures the whole masthead; an
// attribute is not one of the changes it follows, so the fraction can move
// with every painted frame and the fitter still runs once a second.
let liveClockValue = null;
function paintLiveClock() {
  if (!liveClockValue?.isConnected) liveClockValue = document.querySelector("[data-live-time]");
  const value = liveClockValue;
  if (!value) return;
  const {text, hundredths} = liveClockReading();
  if (value.textContent !== text) value.textContent = text;
  if (value.dataset.hundredths !== hundredths) value.dataset.hundredths = hundredths;
}
setInterval(paintLiveClock, 500);
// The hundredths have no timer of their own (operator 2026-11-21: "not every
// one will post... only when frames are already updating"). A frame the map
// is painting anyway carries the clock's reading with it, so a busy world
// shows the fraction running and a still one keeps the half-second tick
// above. A stopped clock has nothing new to post.
function paintLiveClockWithFrame() {
  if (liveClock.runningSince !== null) paintLiveClock();
}
