// How often does the page actually paint, in each view, with and without a
// camera move? A screencast frame count answers "what could be recorded"; a
// counter wrapped round draw() answers "what the page believes it did", and the
// two disagreeing is the interesting case.
//
//   node probe.js <gamePort> <view> [width] [height]
const {Chrome, sleep} = require("./cdp");

async function main() {
  const [port, view, width = "1920", height = "1080"] = process.argv.slice(2);
  const chrome = await Chrome.launch({width: +width, height: +height, profileTag: "probe"});
  try {
    await chrome.send("Page.enable");
    await chrome.send("Runtime.enable");
    await chrome.send("Page.navigate", {url: `http://127.0.0.1:${port}/?view=${view}&viewer=probe-${view}`});
    for (let attempt = 0; attempt < 80; attempt++) {
      await sleep(500);
      const turn = await chrome.evaluate("typeof state !== 'undefined' && state ? state.turn : 0");
      if (turn) break;
    }
    await chrome.evaluate(`(() => {
      window.__paints = 0; window.__minis = 0;
      const realDraw = draw; draw = function(...a) { window.__paints++; return realDraw.apply(this, a); };
      const realMini = drawMini; drawMini = function(...a) { window.__minis++; return realMini.apply(this, a); };
      window.__mode = JSON.stringify({idle: MODE.idle, view: VIEW, spec: SPEC, cinema: cinemaActive(),
                                      w: cv.width, h: cv.height, reduced: REDUCED_MOTION_QUERY.matches});
      return 1;
    })()`);
    console.log("mode", await chrome.evaluate("window.__mode"));

    let frames = 0;
    chrome.on("Page.screencastFrame", async params => {
      frames++;
      try { await chrome.send("Page.screencastFrameAck", {sessionId: params.sessionId}); } catch {}
    });
    await chrome.send("Page.startScreencast", {format: "jpeg", quality: 60, everyNthFrame: 1});

    await chrome.evaluate("window.__paints = 0; window.__minis = 0; 1");
    await sleep(10000);
    let paints = await chrome.evaluate("window.__paints");
    console.log(`idle 10s: screencast=${frames} paints=${paints}`);

    // Now with the globe turning under a hand, which is what the recording will
    // actually be doing whenever it is worth watching.
    frames = 0;
    await chrome.evaluate(`(() => {
      window.__spin = setInterval(() => {
        if (typeof cam === 'undefined' || !cam.basis) return;
        applyPlanetBasis(planetTurn(cam.basis, 2.2, 0));
      }, 16);
      window.__paints = 0; return 1;
    })()`);
    await sleep(10000);
    paints = await chrome.evaluate("window.__paints");
    console.log(`spinning 10s: screencast=${frames} paints=${paints}`);
    await chrome.evaluate("clearInterval(window.__spin); 1");
    console.log("errors", chrome.errors.slice(0, 5));
  } finally {
    chrome.close();
  }
}

main().then(() => process.exit(0), err => { console.error(err); process.exit(1); });
