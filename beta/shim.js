// CIVVIS in a page with nothing behind it.
//
// `web/index.html` talks to the engine over HTTP: it fetches `/state`, posts
// `/action`, asks `/rules` at boot. The published build keeps every one of
// those calls and removes the server, by answering them here — the engine is
// `civvis.wasm` in a worker, and `fetch` is intercepted before it reaches the
// network.
//
// The viewer is not modified to suit this. It is copied from the repository
// byte for byte apart from the asset paths a subdirectory forces, which is
// what keeps a published build honestly the same program as the one that runs
// on a desktop.
//
// Three things the socket server did are genuinely the page's job now and are
// done here rather than in the module:
//
//   * **The clock.** A spectated turn is paced in wall-clock milliseconds. The
//     module plays the turn; the wait that spaces turns out belongs on a
//     thread that is allowed to be idle.
//   * **The finale countdown.** Ten seconds between a result and the next
//     world, counted where there is a clock to count with.
//   * **Saved games.** There is no disk. `localStorage` holds them, and the
//     engine only ever sees a save as an uploaded game.

(function () {
  "use strict";

  const here = new URL(".", document.currentScript.src);
  const WASM_URL = new URL("civvis.wasm", here).href;
  const WORKER_URL = new URL("worker.js", here).href;

  // Everything the engine answers. A path not in here is a real file — the
  // sprite atlases, the 3D cinematic — and goes to the network untouched.
  const ENGINE_ROUTES = new Set([
    "/state", "/status", "/runtime", "/rules", "/pedia", "/save", "/saves",
    "/load", "/action", "/step", "/autoplay", "/play-on", "/route", "/view",
    "/spectator-status", "/next-game-settings", "/new", "/supervisor-new",
    "/pace", "/next-game",
  ]);

  // Ten seconds of result screen, which is the rule the desktop build counts
  // down from and the window in which "one more turn" can still be pressed.
  const FINALE_MS = 10000;
  // What the server sleeps for while paused, so a paused page asking for the
  // next turn over and over costs a poll every so often instead of one per
  // animation frame.
  const PAUSED_POLL_MS = 150;

  const SAVE_PREFIX = "civvis.save.";
  const AUTOSAVE_NAME = "autosave";

  // What a build check can read from outside the page.
  //
  // Worth stating plainly, because it is the one guarantee this architecture
  // gets for free: a spectated turn is played *by* the request that asks for
  // it, and the viewer only issues that request after `render()` has finished
  // the previous turn. A turn that nobody painted therefore cannot be
  // simulated at all — the frame-per-turn contract is structural here rather
  // than enforced by a delivery gate. `turns` and `paints` should match.
  const report = (window.__civvisBeta = {
    ready: false,
    error: null,
    requests: 0,
    turns: 0,     // turns the engine played for this page
    paints: 0,    // next-turn requests, each one an acknowledged repaint
    lastTurn: null,
    seenTurns: new Set(),
  });

  // ------------------------------------------------------------------ worker

  const worker = new Worker(WORKER_URL);
  const pending = new Map();
  let nextId = 1;

  worker.onmessage = (event) => {
    const { id, ok, answer, error } = event.data;
    const waiting = pending.get(id);
    if (!waiting) return;
    pending.delete(id);
    if (ok) waiting.resolve(JSON.parse(new TextDecoder().decode(answer)));
    else waiting.reject(new Error(error));
  };
  worker.onerror = (event) => {
    for (const waiting of pending.values()) {
      waiting.reject(new Error(event.message || "the engine stopped"));
    }
    pending.clear();
  };

  // The world this visit opens on. The engine is deterministic per seed and
  // imports nothing, so it has no way to vary on its own — without this every
  // visitor watches the same six civilizations play the same map for ever.
  // `?game=<n>` pins it, which is what makes a world worth showing shareable.
  const OPENING_SEED = (() => {
    const asked = Number(new URL(window.location.href).searchParams.get("game"));
    if (Number.isSafeInteger(asked) && asked > 0) return asked;
    return Math.floor(Math.random() * 0xffffffff) + 1;
  })();

  function ask(method, path, body) {
    return new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject });
      worker.postMessage({
        id,
        method,
        path,
        body: body || "",
        wasmUrl: WASM_URL,
        seed: OPENING_SEED,
      });
    });
  }

  const sleep = (ms) => new Promise((done) => setTimeout(done, ms));

  // ------------------------------------------------------- clock and finale

  let pace = 0;
  let paused = false;
  let finaleEndsAt = null;

  // A result holds the screen for ten seconds and then the next world opens.
  // Reading it off each answer keeps the countdown attached to the state the
  // page is actually looking at, rather than to a timer running beside it.
  async function withFinale(state) {
    if (!state || typeof state !== "object") return state;
    const finished = state.winner !== undefined && state.winner !== null;
    if (!finished) {
      finaleEndsAt = null;
      return state;
    }
    if (finaleEndsAt === null) finaleEndsAt = performance.now() + FINALE_MS;
    const left = finaleEndsAt - performance.now();
    if (left > 0) {
      state.restart_in = Math.ceil(left / 1000);
      return state;
    }
    finaleEndsAt = null;
    return await ask("POST", "/next-game", "{}");
  }

  // ------------------------------------------------------------------ saves

  const saveKey = (name) => SAVE_PREFIX + name;

  function storedSaves() {
    const saves = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (!key || !key.startsWith(SAVE_PREFIX)) continue;
      try {
        const held = JSON.parse(localStorage.getItem(key));
        saves.push({ name: key.slice(SAVE_PREFIX.length), turn: held.turn ?? 0 });
      } catch (error) {
        // A half-written entry is not a save. Leave it for `writeSave` to
        // evict rather than break the list every time it is read.
      }
    }
    saves.sort((a, b) => b.turn - a.turn);
    return saves;
  }

  async function writeSave(name) {
    const game = await ask("GET", "/save", "");
    try {
      localStorage.setItem(saveKey(name), JSON.stringify(game));
    } catch (error) {
      // A whole game is megabytes and the quota is not large. Drop the
      // autosave first — it is the one copy nobody asked for by name.
      localStorage.removeItem(saveKey(AUTOSAVE_NAME));
      try {
        localStorage.setItem(saveKey(name), JSON.stringify(game));
      } catch (again) {
        return { error: `there is no room to save ${name}: ${again.message}` };
      }
    }
    return { error: null, name, turn: game.turn ?? 0 };
  }

  // --------------------------------------------------------------- dispatch

  const json = (value) =>
    new Response(JSON.stringify(value), {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });

  async function serve(method, target, body) {
    const path = target.split("?")[0];
    const parsed = body ? safeParse(body) : {};

    // The routes that need storage the engine cannot reach.
    if (method === "GET" && path === "/saves") return json({ saves: storedSaves() });

    if (method === "POST" && path === "/save") {
      const name = String(parsed.name || "");
      if (!/^[A-Za-z0-9_-]+$/.test(name)) {
        return json({ error: "a save name is letters, digits, - and _" });
      }
      return json(await writeSave(name));
    }

    if (method === "POST" && path === "/load" && parsed.name) {
      const held = localStorage.getItem(saveKey(String(parsed.name)));
      if (held === null) return json({ error: `there is no save called ${parsed.name}` });
      return json(await ask("POST", "/load", JSON.stringify({ game: JSON.parse(held) })));
    }

    if (method === "POST" && path === "/pace") {
      if (typeof parsed.ms === "number") pace = parsed.ms;
      if (typeof parsed.paused === "boolean") {
        paused = parsed.paused;
        // Pausing voids a running countdown, exactly as it does on a socket.
        if (paused) finaleEndsAt = null;
      }
      return json(await ask("POST", "/pace", body));
    }

    // A spectator asking for the next turn. On a socket this request is held
    // until a stepper thread has played past the frame the page named; here
    // the request *is* the step, so the wait that paces it happens after.
    const wantsNextTurn = path === "/state" && /[?&]have=/.test(target);
    if (wantsNextTurn) report.paints++;
    if (wantsNextTurn && paused) await sleep(PAUSED_POLL_MS);

    const started = performance.now();
    let answer = await ask(method, target, body);
    report.requests++;
    if (answer && typeof answer.turn === "number") {
      report.lastTurn = answer.turn;
      // Counted per world: a new game restarts at turn one, and turns already
      // seen in the world before it must not make the new one look replayed.
      const stamp = `${answer.seed}:${answer.turn}`;
      if (!report.seenTurns.has(stamp)) {
        report.seenTurns.add(stamp);
        report.turns = report.seenTurns.size;
      }
    }
    if (wantsNextTurn && !paused) {
      const owed = pace - (performance.now() - started);
      if (owed > 0) await sleep(owed);
    }

    answer = await withFinale(answer);

    // Civ 6 autosaves at the top of every turn and so does the desktop build.
    // The engine cannot, so it says when one is due and the page keeps it.
    if (answer && answer.autosave_due !== undefined && answer.autosave_due !== null) {
      const turn = answer.autosave_due;
      delete answer.autosave_due;
      writeSave(AUTOSAVE_NAME).catch(() => {});
      // The page reads `autosaved` — the field the desktop build sets once the
      // write succeeded. The write here is deliberately not awaited: a turn is
      // not held up by storage.
      answer.autosaved = turn;
    }
    return json(answer);
  }

  function safeParse(text) {
    try {
      return JSON.parse(text) || {};
    } catch (error) {
      return {};
    }
  }

  // ------------------------------------------------------- the interception

  const networkFetch = window.fetch.bind(window);

  window.fetch = function (input, options) {
    const options_ = options || {};
    const target = typeof input === "string" ? input : (input && input.url) || "";
    let path;
    try {
      path = new URL(target, window.location.href).pathname;
    } catch (error) {
      return networkFetch(input, options);
    }

    if (!ENGINE_ROUTES.has(path)) return networkFetch(input, options);

    const method = (options_.method || (input && input.method) || "GET").toUpperCase();
    // The engine is given the request target rather than the URL: several
    // routes read their parameters straight off the query string.
    const relative = target.startsWith("http") ? path + new URL(target).search : target;
    return serve(method, relative, options_.body || "").catch(
      (error) => json({ error: String(error.message || error) }),
    );
  };

  // ----------------------------------------------------------- first impression

  // A megabyte and a half of engine arrives before the page can draw anything.
  // Say so, rather than showing a blank screen for the download.
  const curtain = document.createElement("div");
  curtain.id = "civvis-beta-loading";
  curtain.innerHTML =
    '<div class="civvis-beta-mark">CIVVIS</div>' +
    '<div class="civvis-beta-note">loading the engine</div>';
  const style = document.createElement("style");
  style.textContent = `
    #civvis-beta-loading {
      position: fixed; inset: 0; z-index: 99999;
      display: flex; flex-direction: column; gap: 14px;
      align-items: center; justify-content: center;
      background: #07110f; color: #d7b66a;
      font-family: Georgia, "Times New Roman", serif;
      transition: opacity 400ms ease;
    }
    #civvis-beta-loading.gone { opacity: 0; pointer-events: none; }
    .civvis-beta-mark { font-size: 40px; letter-spacing: 0.34em; text-indent: 0.34em; }
    .civvis-beta-note { font-size: 13px; letter-spacing: 0.18em; color: #6f8279; text-transform: uppercase; }
    @media (prefers-reduced-motion: reduce) { #civvis-beta-loading { transition: none; } }
  `;
  const raise = () => {
    document.head.appendChild(style);
    document.body.appendChild(curtain);
  };
  if (document.body) raise();
  else document.addEventListener("DOMContentLoaded", raise, { once: true });

  // The engine is ready when it has answered something. `/runtime` is the
  // cheapest question there is — it builds no observation at all.
  ask("GET", "/runtime", "")
    .then(() => {
      report.ready = true;
    })
    .catch((error) => {
      report.error = String(error.message || error);
    })
    .then(() => {
      curtain.classList.add("gone");
      setTimeout(() => curtain.remove(), 450);
    });
})();
