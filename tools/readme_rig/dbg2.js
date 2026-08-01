// Does the frame stream survive a long camera move? Count screencast frames and
// the page's own rAF ticks side by side while a spin runs.
//   node dbg2.js <gamePort> [--write 1]
const fs = require("node:fs");
const {Chrome, sleep} = require("./cdp");
const DIRECTOR = require("./director");

async function main() {
  const port = process.argv[2];
  const writing = process.argv.includes("--write");
  const dir = "/private/tmp/claude-501/-Users-martin/abcd3e00-d93e-4193-9f74-a9915019a3aa/scratchpad/dbg2";
  if (writing) fs.mkdirSync(dir, {recursive: true});
  const chrome = await Chrome.launch({width: 1600, height: 900, profileTag: "dbg2"});
  try {
    await chrome.send("Page.enable");
    await chrome.send("Runtime.enable");
    await chrome.send("Page.navigate", {url: `http://127.0.0.1:${port}/?view=balanced&viewer=dbg2`});
    for (let attempt = 0; attempt < 90; attempt++) {
      await sleep(500);
      if (await chrome.evaluate("typeof state !== 'undefined' && state ? state.turn : 0")) break;
    }
    await chrome.evaluate(DIRECTOR);
    await chrome.evaluate("window.__raf = 0; (function loop(){ window.__raf++; requestAnimationFrame(loop); })(); 1");

    let frames = 0;
    chrome.on("Page.screencastFrame", async params => {
      const index = frames++;
      if (writing) fs.writeFileSync(`${dir}/${String(index).padStart(6, "0")}.jpg`,
                                    Buffer.from(params.data, "base64"));
      try { await chrome.send("Page.screencastFrameAck", {sessionId: params.sessionId}); } catch {}
    });
    await chrome.send("Page.startScreencast", {format: "jpeg", quality: 88, everyNthFrame: 1});
    await sleep(2000);
    console.log(`before: frames=${frames} raf=${await chrome.evaluate("window.__raf")}`);

    await chrome.evaluate('(() => { __dir.spin(120, 9000); return "started"; })()', {retry: false});
    for (let tick = 0; tick < 20; tick++) {
      await sleep(1500);
      let busy = "??", raf = "??", turn = "??";
      try {
        const probe = await chrome.evaluate(
          'JSON.stringify({busy: __dir.busy, raf: window.__raf, turn: state && state.turn})',
          {timeout: 6000});
        ({busy, raf, turn} = JSON.parse(probe));
      } catch (err) { busy = "(evaluate " + err.message + ")"; }
      console.log(`t+${((tick + 1) * 1.5).toFixed(1)}s frames=${frames} raf=${raf} busy=${busy} turn=${turn}`);
      if (busy === null) break;
    }
    console.log("errors", chrome.errors.slice(0, 6));
  } finally { chrome.close(); }
}
main().then(() => process.exit(0), err => { console.error(err); process.exit(1); });
