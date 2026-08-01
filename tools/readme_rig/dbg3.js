// Reproduce the freeze the recorder hits, and test one fix at a time.
//   node dbg3.js <gamePort> [--focus] [--lifecycle] [--nopause]
//
// The sequence is the recorder's: fresh board, boot, install, screencast,
// a short hold, let the game run a few turns, pause, then a long spin.
const {Chrome, sleep} = require("./cdp");
const DIRECTOR = require("./director");

const post = (port, route, body) =>
  fetch(`http://127.0.0.1:${port}${route}`, {method: "POST", body}).then(r => r.text());
const getState = port => fetch(`http://127.0.0.1:${port}/state`).then(r => r.json());

async function main() {
  const port = process.argv[2];
  const wantFocus = process.argv.includes("--focus");
  const wantLifecycle = process.argv.includes("--lifecycle");
  const noPause = process.argv.includes("--nopause");

  await post(port, "/new", JSON.stringify({
    force: true, paused: true, seed: 2003, num_players: 6,
    map_script: "grand_canals_2", map_topology: "planet",
    game_speed: "online", max_turns: 500}));
  await sleep(4000);

  const chrome = await Chrome.launch({width: 1600, height: 900, profileTag: "dbg3"});
  try {
    await chrome.send("Page.enable");
    await chrome.send("Runtime.enable");
    await chrome.send("Page.navigate", {url: `http://127.0.0.1:${port}/?view=balanced&viewer=dbg3`});
    for (let attempt = 0; attempt < 90; attempt++) {
      await sleep(500);
      if (await chrome.evaluate("typeof state !== 'undefined' && state ? state.turn : 0")) break;
    }
    if (wantFocus) await chrome.send("Emulation.setFocusEmulationEnabled", {enabled: true});
    if (wantLifecycle) await chrome.send("Page.setWebLifecycleState", {state: "active"});
    await chrome.evaluate(DIRECTOR);
    await chrome.evaluate("window.__raf = 0; (function loop(){ window.__raf++; requestAnimationFrame(loop); })(); 1");

    let frames = 0;
    chrome.on("Page.screencastFrame", async params => {
      frames++;
      try { await chrome.send("Page.screencastFrameAck", {sessionId: params.sessionId}); } catch {}
    });
    await chrome.send("Page.startScreencast", {format: "jpeg", quality: 88, everyNthFrame: 1});
    await sleep(1500);

    const probe = async label => {
      let raf = -1, hidden = "?", vis = "?";
      try {
        const value = await chrome.evaluate(
          'JSON.stringify({raf: window.__raf, hidden: document.hidden,' +
          ' vis: document.visibilityState, busy: __dir.busy, turn: state && state.turn})',
          {timeout: 8000});
        const info = JSON.parse(value);
        raf = info.raf; hidden = info.hidden; vis = info.vis;
        console.log(`${label}: frames=${frames} raf=${raf} hidden=${hidden} vis=${vis} busy=${info.busy} turn=${info.turn}`);
      } catch (err) { console.log(`${label}: frames=${frames} evaluate failed ${err.message}`); }
      return raf;
    };

    await post(port, "/pace", '{"paused":true}');
    await probe("start");
    await chrome.evaluate('(() => { __dir.hold(2000); return 1; })()');
    await sleep(3000);
    await probe("after hold");

    if (!noPause) {
      await post(port, "/pace", '{"ms":0,"paused":false}');
      for (let attempt = 0; attempt < 40; attempt++) {
        await sleep(400);
        if ((await getState(port)).turn >= 4) break;
      }
      await post(port, "/pace", '{"paused":true}');
      await probe("after run+pause");
    }

    await chrome.evaluate('(() => { __dir.spin(120, 9000); return 1; })()');
    for (let tick = 0; tick < 10; tick++) { await sleep(1500); await probe(`spin t+${(tick + 1) * 1.5}s`); }
    console.log("errors", chrome.errors.slice(0, 5));
  } finally { chrome.close(); }
}
main().then(() => process.exit(0), err => { console.error(err); process.exit(1); });
