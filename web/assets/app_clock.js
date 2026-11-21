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
// `livePause`, `gameFinished`) exist by the time they are read.
//
// The stopwatch is the turn plate's top section, above the age. It
// reads how long the world on screen has actually been playing: it starts
// with the world's first unpaused frame, stops while the simulation is paused
// or a result is on screen, and a new world starts it over from zero. A
// restart that comes back paused at the opening position therefore sits at
// 0:00 until somebody presses Start — an unstarted game has no live time.
let liveClock = {seed: undefined, ms: 0, runningSince: null};
const LIVE_CLOCK_STORE = "civvis-live-time-v1";
function liveClockMs() {
  return liveClock.ms +
    (liveClock.runningSince !== null ? Math.max(0, Date.now() - liveClock.runningSince) : 0);
}
// One reading of the clock, split where the plate splits it: the whole seconds
// are the value's words and the hundredths of a second follow them (operator
// 2026-11-21: "add hundredths of a second to the live time count").
function liveClockReading() {
  const ms = liveClockMs();
  const total = Math.floor(ms / 1000);
  const seconds = String(total % 60).padStart(2, "0");
  const minutes = Math.floor(total / 60) % 60;
  const hours = Math.floor(total / 3600);
  return {
    text: hours ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}` : `${minutes}:${seconds}`,
    hundredths: String(Math.floor(ms / 10) % 100).padStart(2, "0"),
  };
}
// Start or stop the accumulator against what the page is showing right now.
// Paused covers the unstarted opening as well: a clock that has never run
// and is not running reads 0:00, which is what an unstarted game has played.
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
  }
  syncLiveClockRun();
  saveLiveClock(st);
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
