// Shoot one still of the live spectator in each view, so the look can be chosen
// from pictures rather than from the source.
//
//   node shoot.js <gamePort> <outDir> <view> [view...]
const fs = require("node:fs");
const {Chrome, sleep} = require("./cdp");

async function main() {
  const [port, outDir, ...views] = process.argv.slice(2);
  fs.mkdirSync(outDir, {recursive: true});
  const chrome = await Chrome.launch({width: 1600, height: 1000, profileTag: "shoot"});
  try {
    await chrome.send("Page.enable");
    await chrome.send("Runtime.enable");
    for (const view of views) {
      const url = `http://127.0.0.1:${port}/?view=${view}&viewer=shoot-${view}`;
      await chrome.send("Page.navigate", {url});
      // The page boots off /state and /rules; wait for a real turn number rather
      // than for a load event, and never grep body.textContent for an error —
      // the whole client is one inline script inside <body>.
      let booted = null;
      for (let attempt = 0; attempt < 80; attempt++) {
        await sleep(500);
        booted = await chrome.evaluate(
          "JSON.stringify({turn: typeof state !== 'undefined' && state ? state.turn : null," +
          " w: cv && cv.width, h: cv && cv.height, href: location.href})");
        const info = JSON.parse(booted || "{}");
        if (info.turn) { console.log(view, "booted", booted); break; }
      }
      // A screencast is what makes a headless page actually run frames; without
      // a consumer it fires no rAF at all and the canvas is whatever the first
      // synchronous draw left.
      let frames = 0;
      chrome.on("Page.screencastFrame", async params => {
        frames++;
        try { await chrome.send("Page.screencastFrameAck", {sessionId: params.sessionId}); } catch {}
      });
      await chrome.send("Page.startScreencast", {format: "jpeg", quality: 40, everyNthFrame: 1});
      await sleep(6000);
      await chrome.send("Page.stopScreencast");
      const shot = await chrome.send("Page.captureScreenshot", {format: "png"});
      fs.writeFileSync(`${outDir}/${view}.png`, Buffer.from(shot.data, "base64"));
      console.log(view, "frames", frames, "errors", chrome.errors.slice(0, 3));
    }
  } finally {
    chrome.close();
  }
}

main().then(() => process.exit(0), err => { console.error(err); process.exit(1); });
