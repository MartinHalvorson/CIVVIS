// A small CDP driver for the CIVVIS spectator: spawn our own headless Chrome on
// a port nobody else holds, attach to its first page, and expose send/evaluate
// plus a screencast recorder.
//
// Every rule in here is one that cost somebody an iteration before:
//   * a fresh --user-data-dir per run, deleted first (a stale profile lock
//     makes the run fail with no output at all)
//   * --remote-debugging-address=127.0.0.1 on a port we proved is free, because
//     this box runs a dozen sibling harnesses and Chrome answers /json on
//     somebody else's port without a murmur
//   * kill the browser in the same process that launched it: a hung headless
//     Chrome is a permanent ~700 MB / ~80% CPU leak that reparents to launchd
//   * ack every screencast frame or the stream stops after a few
const {spawn, execFileSync} = require("node:child_process");
const fs = require("node:fs");
const net = require("node:net");

const CHROME = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";

function sleep(ms) { return new Promise(done => setTimeout(done, ms)); }

function portFree(port) {
  return new Promise(done => {
    const probe = net.createServer();
    probe.once("error", () => done(false));
    probe.once("listening", () => probe.close(() => done(true)));
    probe.listen(port, "127.0.0.1");
  });
}

async function freePort(from, to) {
  for (let port = from; port <= to; port++) if (await portFree(port)) return port;
  throw new Error(`no free port in ${from}..${to}`);
}

class Chrome {
  constructor(opts) { Object.assign(this, opts); this.nextId = 1; this.waiting = new Map();
                      this.handlers = new Map(); this.errors = []; }

  static async launch({width = 1600, height = 1000, scale = 1, profileTag = "rec"} = {}) {
    const port = await freePort(9500, 9700);
    const profile = `/private/tmp/claude-501/-Users-martin/abcd3e00-d93e-4193-9f74-a9915019a3aa/scratchpad/chrome-${profileTag}-${port}`;
    fs.rmSync(profile, {recursive: true, force: true});
    const child = spawn(CHROME, [
      "--headless=new", "--disable-gpu", "--no-sandbox", "--mute-audio",
      "--hide-scrollbars", "--no-first-run", "--no-default-browser-check",
      "--disable-background-timer-throttling",
      "--disable-backgrounding-occluded-windows",
      "--disable-renderer-backgrounding",
      `--force-device-scale-factor=${scale}`,
      `--window-size=${width},${height}`,
      `--user-data-dir=${profile}`,
      "--remote-debugging-address=127.0.0.1",
      `--remote-debugging-port=${port}`,
      "about:blank",
    ], {stdio: ["ignore", "pipe", "pipe"]});
    child.stdout.on("data", () => {});
    child.stderr.on("data", () => {});

    let version = null;
    for (let attempt = 0; attempt < 120; attempt++) {
      await sleep(250);
      try {
        const res = await fetch(`http://127.0.0.1:${port}/json/version`);
        version = await res.json();
        break;
      } catch { /* not up yet */ }
    }
    if (!version) { child.kill("SIGKILL"); throw new Error("chrome never answered /json/version"); }

    // Exactly one listener, and it has to be the child we just started.
    const owners = execFileSync("/usr/sbin/lsof",
      ["-nP", `-iTCP:${port}`, "-sTCP:LISTEN", "-t"], {encoding: "utf8"})
      .split("\n").filter(Boolean);
    if (!owners.includes(String(child.pid))) {
      child.kill("SIGKILL");
      throw new Error(`debug port ${port} is owned by ${owners.join(",")}, not ${child.pid}`);
    }

    const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    const page = targets.find(t => t.type === "page");
    if (!page) { child.kill("SIGKILL"); throw new Error("no page target"); }

    const chrome = new Chrome({port, profile, child, wsUrl: page.webSocketDebuggerUrl});
    await chrome.connect();
    return chrome;
  }

  connect() {
    return new Promise((done, fail) => {
      const ws = new WebSocket(this.wsUrl);
      this.ws = ws;
      ws.addEventListener("open", () => done());
      ws.addEventListener("error", event => fail(new Error("ws error: " + event.message)));
      ws.addEventListener("message", event => {
        const msg = JSON.parse(event.data);
        if (msg.id && this.waiting.has(msg.id)) {
          const {done: ok, fail: bad} = this.waiting.get(msg.id);
          this.waiting.delete(msg.id);
          msg.error ? bad(new Error(JSON.stringify(msg.error))) : ok(msg.result);
        } else if (msg.method) {
          if (msg.method === "Runtime.exceptionThrown")
            this.errors.push(msg.params?.exceptionDetails?.exception?.description
                             || msg.params?.exceptionDetails?.text || "?");
          const handler = this.handlers.get(msg.method);
          if (handler) handler(msg.params);
        }
      });
    });
  }

  send(method, params = {}, timeout = 20000) {
    const id = this.nextId++;
    return new Promise((done, fail) => {
      const timer = setTimeout(() => {
        this.waiting.delete(id);
        fail(new Error(`timeout: ${method}`));
      }, timeout);
      this.waiting.set(id, {done: v => { clearTimeout(timer); done(v); },
                           fail: e => { clearTimeout(timer); fail(e); }});
      this.ws.send(JSON.stringify({id, method, params}));
    });
  }

  on(method, handler) { this.handlers.set(method, handler); }

  // `awaitPromise + returnByValue` gives plain JSON back; anything that is not
  // serializable comes back undefined, so stringify inside the page.
  async evaluate(expression, {timeout = 20000, retry = true} = {}) {
    try {
      const res = await this.send("Runtime.evaluate",
        {expression, awaitPromise: true, returnByValue: true}, timeout);
      if (res.exceptionDetails)
        throw new Error(res.exceptionDetails.exception?.description || res.exceptionDetails.text);
      return res.result?.value;
    } catch (err) {
      if (!retry || !String(err.message).startsWith("timeout")) throw err;
      const res = await this.send("Runtime.evaluate",
        {expression, awaitPromise: true, returnByValue: true}, timeout);
      if (res.exceptionDetails)
        throw new Error(res.exceptionDetails.exception?.description || res.exceptionDetails.text);
      return res.result?.value;
    }
  }

  close() {
    try { this.ws?.close(); } catch {}
    try { this.child.kill("SIGKILL"); } catch {}
    // Renderers exit on their own once the browser process is gone; the profile
    // is never reused, so removing it here keeps the disk leak from accruing.
    setTimeout(() => { try { fs.rmSync(this.profile, {recursive: true, force: true}); } catch {} }, 1500);
  }
}

module.exports = {Chrome, sleep, freePort};
