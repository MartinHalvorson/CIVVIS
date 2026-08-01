// Record a whole spectator game to a directory of frames, running a scripted
// camera over the top of it.
//
//   node record.js --port 8930 --seed 2007 --plan plan.json --out frames/ \
//                  [--view balanced] [--width 1600] [--height 900]
//
// Why a screencast and not a screenshot loop: headless Chrome fires no
// animation frames at all unless something is consuming them, so the screencast
// is what makes the page run — and it hands back every presented frame with a
// real timestamp, which is what lets the encode use the durations the picture
// actually held instead of inventing a frame rate.
const fs = require("node:fs");
const path = require("node:path");
const {Chrome, sleep} = require("./cdp");
const DIRECTOR = require("./director");

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 2) out[argv[i].replace(/^--/, "")] = argv[i + 1];
  return out;
}

async function post(port, route, body) {
  const res = await fetch(`http://127.0.0.1:${port}${route}`, {method: "POST", body});
  return res.text();
}
async function getState(port) {
  const res = await fetch(`http://127.0.0.1:${port}/state`);
  return res.json();
}

function majors(snapshot) {
  return (snapshot.players || []).filter(p => !p.is_minor && !p.is_barbarian);
}

async function main() {
  const args = parseArgs(process.argv.slice(2));
  const port = +args.port;
  const view = args.view || "balanced";
  const width = +(args.width || 1600), height = +(args.height || 900);
  const outDir = args.out;
  const plan = JSON.parse(fs.readFileSync(args.plan, "utf8"));
  fs.mkdirSync(outDir, {recursive: true});
  const frameLog = fs.createWriteStream(path.join(outDir, "frames.jsonl"));
  const eventLog = fs.createWriteStream(path.join(outDir, "events.jsonl"));
  const note = (kind, data) => {
    const line = JSON.stringify({t: Date.now(), kind, ...data});
    eventLog.write(line + "\n");
    console.log(line);
  };

  // A fresh board, paused, so the opening shot is turn 1 and not turn 15. The
  // `force` is required: an active spectator game refuses a reset without it.
  await post(port, "/new", JSON.stringify({
    force: true, paused: true, seed: plan.seed, num_players: plan.players || 6,
    map_script: plan.map || "grand_canals_2", map_topology: "planet",
    game_speed: "online", max_turns: plan.maxTurns || 500,
  }));
  await sleep(4000);
  const opening = await getState(port);
  note("opening", {turn: opening.turn, seed: opening.seed,
                   civs: majors(opening).map(p => p.civ)});

  const chrome = await Chrome.launch({width, height, profileTag: "rec"});
  let frames = 0;
  try {
    await chrome.send("Page.enable");
    await chrome.send("Runtime.enable");
    // `--window-size` is the *window*, so the viewport the screencast captures
    // comes out short of it — 1600x900 asked for gave 1600x758 recorded, which
    // is not a 16:9 clip. Override the metrics before navigating and the frames
    // are exactly the size that was asked for.
    await chrome.send("Emulation.setDeviceMetricsOverride",
      {width, height, deviceScaleFactor: 1, mobile: false});
    await chrome.send("Page.navigate",
      {url: `http://127.0.0.1:${port}/?view=${view}&viewer=record-${plan.seed}`});

    let booted = false;
    for (let attempt = 0; attempt < 90 && !booted; attempt++) {
      await sleep(500);
      const probe = await chrome.evaluate(
        "JSON.stringify({turn: typeof state !== 'undefined' && state ? state.turn : null," +
        " w: cv && cv.width, href: location.href})");
      const info = JSON.parse(probe || "{}");
      // A canvas still at 300x150 means the one top-level script died before it
      // sized anything; a real turn number is the only honest proof of a boot.
      booted = !!info.turn && info.w > 400 && info.href.includes(`:${port}/`);
      if (booted) note("booted", info);
    }
    if (!booted) throw new Error("page never booted");

    // Without these two the headless page goes `document.hidden` partway
    // through a long run and Chrome stops issuing animation frames outright —
    // not "slowly", *nothing*: the page's own rAF counter freezes, the
    // screencast goes silent, and restarting the cast does not wake it, while
    // `Runtime.evaluate` keeps answering so every probe still reads healthy.
    // Measured on this exact sequence: hidden flipped to true six seconds into
    // a spin and the recording was over. They are not a nicety here, they are
    // what makes a recording longer than a few seconds possible at all.
    const keepAwake = async () => {
      await chrome.send("Emulation.setFocusEmulationEnabled", {enabled: true}, 8000);
      await chrome.send("Page.setWebLifecycleState", {state: "active"}, 8000);
    };
    await keepAwake();
    note("install", {result: await chrome.evaluate(DIRECTOR)});
    await chrome.evaluate(
      "window.__raf = 0; (function loop(){ window.__raf++; requestAnimationFrame(loop); })(); 1");

    let lastFrameAt = Date.now();
    chrome.on("Page.screencastFrame", async params => {
      const index = frames++;
      lastFrameAt = Date.now();
      fs.writeFileSync(path.join(outDir, String(index).padStart(6, "0") + ".jpg"),
                       Buffer.from(params.data, "base64"));
      frameLog.write(JSON.stringify({index, ts: params.metadata.timestamp}) + "\n");
      try { await chrome.send("Page.screencastFrameAck", {sessionId: params.sessionId}); } catch {}
    });
    const castOptions = {format: "jpeg", quality: +(args.quality || 88), everyNthFrame: 1};
    await chrome.send("Page.startScreencast", castOptions);
    await sleep(1500);

    // Headless produces animation frames only while something consumes them, so
    // a screencast that goes quiet is not a quiet page — it is a page that has
    // stopped running, and it will not restart itself. Restarting the cast is
    // the one lever that reliably wakes it.
    let restarts = 0;
    const castWatchdog = setInterval(async () => {
      if (Date.now() - lastFrameAt < 4000) return;
      lastFrameAt = Date.now();
      restarts++;
      try {
        await keepAwake();
        await chrome.send("Page.stopScreencast", {}, 8000);
        await chrome.send("Page.startScreencast", castOptions, 8000);
        const hidden = await chrome.evaluate("document.hidden", {timeout: 6000}).catch(() => "?");
        note("cast-restart", {restarts, frames, hidden});
      } catch (err) { note("cast-restart-failed", {error: String(err.message)}); }
    }, 1000);

    // A move is started and then *polled*, never awaited inside one long
    // `Runtime.evaluate`. On a Planet spectator an evaluate that is outstanding
    // while the main thread paints occasionally never answers, and the generic
    // "timeout and one retry" rule cannot apply to a call with side effects —
    // retrying it replays the camera move. Starting the move in a call that
    // returns immediately makes every subsequent read cheap and safely
    // retryable, and `__dir.busy` is the page's own account of what it is doing.
    const runStep = async step => {
      const [verb, ...rest] = step.split(":");
      let call, span = 4000;
      if (verb === "hold") { call = `__dir.hold(${rest[0]})`; span = +rest[0]; }
      else if (verb === "spin") { call = `__dir.spin(${rest[0]}, ${rest[1]})`; span = +rest[1]; }
      else if (verb === "zoom") {
        const at = rest[2] ? `[${rest[2]}]` : "null";
        call = `__dir.zoom(${rest[0]}, ${rest[1]}, ${at})`; span = +rest[1];
      } else if (verb === "earth") {
        call = `__dir.earth(${rest[0]}, ${rest[1]})`; span = +rest[1];
      } else if (verb === "fly") {
        call = `__dir.fly(${JSON.stringify(rest.join(":"))})`; span = 6600;
      } else if (verb === "home") { call = `__dir.home()`; span = 4600; }
      else if (verb === "stops") {
        note("step", {step, result: await chrome.evaluate("__dir.skyStops().join(' | ')")});
        return;
      } else throw new Error("unknown step " + step);

      await chrome.evaluate(`(() => { ${call}; return "started"; })()`,
                            {timeout: 15000, retry: false});
      const deadline = Date.now() + span + 30000;
      let busy = true;
      while (busy && Date.now() < deadline) {
        await sleep(350);
        try { busy = !!(await chrome.evaluate("__dir.busy", {timeout: 8000})); }
        catch { /* a poll that does not answer is a busy page, not a lost move */ }
      }
      const status = await chrome.evaluate("__dir.status()", {timeout: 12000}).catch(() => "?");
      const raf = await chrome.evaluate("window.__raf", {timeout: 8000}).catch(() => -1);
      note("step", {step, overran: busy, frames, raf, status});
    };

    const beats = plan.beats.map(beat => ({...beat, fired: false}));
    let defaultPace = plan.pace === undefined ? 0 : plan.pace;
    await post(port, "/pace", JSON.stringify({ms: defaultPace, paused: false}));
    note("running", {pace: defaultPace});

    const deadline = Date.now() + (plan.maxMinutes || 45) * 60000;
    let victorySeen = null;
    while (Date.now() < deadline) {
      const snapshot = await getState(port).catch(() => null);
      if (!snapshot) { await sleep(500); continue; }
      const turn = snapshot.turn;

      if (snapshot.victory_type && !victorySeen) {
        victorySeen = {type: snapshot.victory_type, turn: snapshot.victory_turn || turn,
                       winner: majors(snapshot).find(p => p.id === snapshot.winner)?.civ};
        note("victory", victorySeen);
      }

      for (const beat of beats) {
        if (beat.fired) continue;
        const ready = beat.onVictory ? !!victorySeen : turn >= beat.at;
        if (!ready) continue;
        beat.fired = true;
        note("beat", {at: beat.at, turn, label: beat.label || "", steps: beat.steps});
        const pausing = beat.pause !== false;
        if (pausing) await post(port, "/pace", '{"paused":true}');
        else if (beat.pace !== undefined)
          await post(port, "/pace", JSON.stringify({ms: beat.pace, paused: false}));
        for (const step of beat.steps) await runStep(step);
        if (beat.thenPace !== undefined) defaultPace = beat.thenPace;
        if (!victorySeen)
          await post(port, "/pace", JSON.stringify({ms: defaultPace, paused: false}));
      }

      if (victorySeen && beats.every(beat => beat.fired)) break;
      if (plan.stopAtTurn && turn >= plan.stopAtTurn && beats.every(beat => beat.fired)) break;
      await sleep(400);
    }

    clearInterval(castWatchdog);
    note("finishing", {frames, victory: victorySeen});
    await sleep(1500);
    await chrome.send("Page.stopScreencast");
    note("done", {frames, errors: chrome.errors.slice(0, 5)});
  } finally {
    await sleep(500);
    chrome.close();
    frameLog.end();
    eventLog.end();
  }
  console.log("frames:", frames);
}

main().then(() => process.exit(0), err => { console.error(err); process.exit(1); });
