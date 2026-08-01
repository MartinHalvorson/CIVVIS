// Poke one page-side expression at a time against a live spectator, with the
// screencast running so the page actually produces frames.
//   node dbg.js <gamePort> '<expression>' ['<expression>' ...]
const {Chrome, sleep} = require("./cdp");
const DIRECTOR = require("./director");

async function main() {
  const [port, ...exprs] = process.argv.slice(2);
  const chrome = await Chrome.launch({width: 1600, height: 900, profileTag: "dbg"});
  try {
    await chrome.send("Page.enable");
    await chrome.send("Runtime.enable");
    await chrome.send("Page.navigate", {url: `http://127.0.0.1:${port}/?view=balanced&viewer=dbg`});
    for (let attempt = 0; attempt < 90; attempt++) {
      await sleep(500);
      const turn = await chrome.evaluate("typeof state !== 'undefined' && state ? state.turn : 0");
      if (turn) break;
    }
    chrome.on("Page.screencastFrame", async params => {
      try { await chrome.send("Page.screencastFrameAck", {sessionId: params.sessionId}); } catch {}
    });
    await chrome.send("Page.startScreencast", {format: "jpeg", quality: 30, everyNthFrame: 1});
    await sleep(1000);
    console.log("install:", await chrome.evaluate(DIRECTOR));
    for (const expr of exprs) {
      const started = Date.now();
      try {
        const value = await chrome.evaluate(expr, {timeout: 30000, retry: false});
        console.log(`${Date.now() - started}ms  ${expr}\n   → ${JSON.stringify(value)}`);
      } catch (err) {
        console.log(`${Date.now() - started}ms  ${expr}\n   !! ${err.message}`);
      }
    }
    console.log("page errors:", chrome.errors.slice(0, 6));
  } finally { chrome.close(); }
}
main().then(() => process.exit(0), err => { console.error(err); process.exit(1); });
