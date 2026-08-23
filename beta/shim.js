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
//   * **The finale countdown.** The viewer-selected interval between a result
//     and the next world, counted where there is a clock to count with.
//   * **Saved games.** There is no disk. `localStorage` holds them, and the
//     engine only ever sees a save as an uploaded game.

(function () {
  "use strict";

  const here = new URL(".", document.currentScript.src);
  const WASM_URL = new URL("civvis.wasm", here).href;
  const WORKER_URL = new URL("worker.js", here).href;
  const BUILD_URL = new URL("build.json", here);

  // Everything the engine answers. A path not in here is a real file, such as
  // a strategic map sprite atlas, and goes to the network untouched.
  const ENGINE_ROUTES = new Set([
    "/state", "/status", "/runtime", "/rules", "/pedia", "/save", "/saves",
    "/load", "/action", "/step", "/autoplay", "/play-on", "/route", "/view",
    "/intel", "/spectator-status", "/next-game-settings", "/new",
    "/supervisor-new", "/pace", "/next-game",
    "/machine-metrics", "/civ6", "/civ6/start",
  ]);
  const LOCAL_DESKTOP_HOST = here.pathname.startsWith("/wasm/");

  // Keep the browser build to the same four choices as the socket server.
  // It starts at ten seconds until the page posts its persisted preference.
  const FINALE_OPTIONS_MS = new Set([0, 3000, 5000, 10000]);
  let betweenGameCountdownMs = 10000;
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
    fullMapFrames: 0,
    patchFrames: 0,
    fullMapTiles: 0,
    patchTiles: 0,
    fullMapBytes: 0,
    patchBytes: 0,
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
    if (!ok) { waiting.reject(new Error(error)); return; }
    const parsed = JSON.parse(new TextDecoder().decode(answer));
    if (waiting.path.startsWith("/state")) {
      const bytes = answer.byteLength;
      if (Array.isArray(parsed?.map?.tiles)) {
        report.fullMapFrames++;
        report.fullMapTiles += parsed.map.tiles.length;
        report.fullMapBytes += bytes;
      } else if (Array.isArray(parsed?.map?.tiles_changed)) {
        report.patchFrames++;
        report.patchTiles += parsed.map.tiles_changed.length;
        report.patchBytes += bytes;
      }
    }
    waiting.resolve(parsed);
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

  // The lobby choices a link made before the page loaded. The home page's
  // preset cards — and anybody sharing a configured world — arrive with
  // settings in the query string; they become the one `/new` request the
  // setup screen would have posted, so the first world such a visit watches
  // is the one the link named rather than the stock exhibition. The worlds
  // after it stay on theme too: the engine rolls each successor from the
  // session's own parameters. A value the engine does not recognise is left
  // where the stock setting stood, exactly as the lobby's own request would
  // be, so a mistyped link still opens on a world.
  const OPENING_PRESET = (() => {
    const search = new URL(window.location.href).searchParams;
    const whole = (name) => {
      const value = Number(search.get(name));
      return Number.isSafeInteger(value) && value > 0 ? value : null;
    };
    const word = (name) => {
      const value = (search.get(name) || "").trim().toLowerCase();
      return /^[a-z0-9_]+$/.test(value) ? value : null;
    };
    const payload = {};
    // The bounds are the tab's own protection, not rules of the game: the
    // engine happily builds what a URL asks for, and a hand-typed
    // `players=5000` would freeze the one machine the game runs on — the
    // visitor's. A hundred seats is the largest world the lobby offers.
    const players = whole("players");
    if (players && players <= 100) payload.num_players = players;
    const map = word("map");
    if (map) payload.map_script = map;
    const shape = word("shape");
    if (shape) payload.map_topology = shape;
    const poles = word("poles");
    if (poles) payload.map_poles = poles;
    const speed = word("speed");
    if (speed) payload.game_speed = speed;
    // `era` names both ends of the same question, so it travels as both: the
    // era the world opens in, and — on a battlefield — the era its armies are
    // drawn from, which is a Tactics rule of its own rather than a start-era
    // spelling. Sending only the first is what made every linked battle an
    // Information-era one: `tactics_era` stayed on the previous world's rule,
    // which on a fresh engine is Start, so the armies followed `start_era`
    // forever. Sending both also means a link naming Medieval cannot inherit
    // a Random rolled by the battle before it.
    //
    // `era=random` is the roll itself — a fresh era for every battle of the
    // series — and is a Tactics rule only. There is no such start era, so it
    // deliberately does not travel as one; a full game asked for it opens
    // where it would have anyway.
    const era = word("era");
    if (era && era !== "random") payload.start_era = era;
    if (era) payload.tactics_era = era;
    const turns = whole("turns");
    if (turns) payload.max_turns = turns;
    // Who is at the keyboard. `mode=play` seats the visitor in the first
    // chair — a single-player game on the settings the link names — and
    // `mode=watch` leaves the world to its AIs. Anything else leaves the
    // stock setting standing, which is the spectated exhibition. This is
    // how the home page's "Play" quadrants differ from its "Watch" ones.
    const mode = word("mode");
    if (mode === "play") payload.spectate = false;
    else if (mode === "watch") payload.spectate = true;
    // A battlefield's dimensions are their own setting — no seat count
    // implies an arena size — so they travel as the one token the lobby's
    // size control uses to name them: `arena=20x20`.
    const arena = /^(\d{1,2})x(\d{1,2})$/.exec((search.get("arena") || "").trim());
    if (arena && Number(arena[1]) > 0 && Number(arena[2]) > 0) {
      payload.width = Number(arena[1]);
      payload.height = Number(arena[2]);
    }
    // `victories=domination,score` enables exactly the named tracks and
    // disables the rest. The engine re-checks what a mode allows, so a
    // battlefield link may name none and still end in a battle's two lanes.
    const victories = search.get("victories");
    if (victories) {
      const asked = new Set(victories.split(",").map((track) => track.trim().toLowerCase()));
      payload.victory_conditions = Object.fromEntries(
        ["science", "culture", "religious", "diplomatic", "domination", "score"]
          .map((track) => [track, asked.has(track)]));
    }
    if (!Object.keys(payload).length) return null;
    // By the time this request lands the opening world is a live spectator
    // game, and replacing one of those is deliberately an explicit act.
    payload.force = true;
    return payload;
  })();

  // The one request that applies the link's settings. Every engine request
  // waits behind it, so the first `/state` cannot answer with the stock
  // world a preset visit never asked to see; on failure the stock world is
  // exactly what should play, and the report says why.
  let openingPresetStarted = null;
  function ensureOpeningPreset() {
    if (OPENING_PRESET === null) return Promise.resolve(null);
    if (openingPresetStarted === null) {
      openingPresetStarted = ask("POST", "/new", JSON.stringify(OPENING_PRESET))
        .then((answer) => {
          if (answer && answer.error) report.presetError = String(answer.error);
          return null;
        })
        .catch((error) => {
          report.presetError = String(error.message || error);
          return null;
        });
    }
    return openingPresetStarted;
  }

  function ask(method, path, body) {
    return new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject, path });
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

  async function installedSuccessorBuild() {
    try {
      const [runtime, response] = await Promise.all([
        ask("GET", "/runtime", ""),
        networkFetch(new URL(`build.json?fresh=${Date.now()}`, here), {
          cache: "no-store",
        }),
      ]);
      if (!response.ok) return null;
      const manifest = await response.json();
      if (typeof runtime.commit !== "string" || typeof manifest.commit !== "string" ||
          runtime.commit === manifest.commit) return null;
      return manifest.commit;
    } catch (error) {
      return null;
    }
  }

  // The manifest belongs to the module this document loaded. Keep that first
  // reading even if a deployment replaces build.json underneath a long-lived
  // game; installedSuccessorBuild deliberately performs its own fresh reads
  // to notice that separate event. A matching commit is mandatory so a page
  // caught during publication never assigns the successor's size to the
  // engine it is still running.
  let runningBuildManifest = null;
  function loadRunningBuildManifest() {
    if (runningBuildManifest === null) {
      runningBuildManifest = networkFetch(new URL("build.json", here), {
        cache: "no-store",
      }).then((response) => response.ok ? response.json() : null)
        .catch(() => null);
    }
    return runningBuildManifest;
  }

  async function attachPublishedBuildMetadata(answer) {
    if (!answer || typeof answer !== "object" ||
        typeof answer.server_commit !== "string") return answer;
    const manifest = await loadRunningBuildManifest();
    if (manifest?.commit === answer.server_commit &&
        Number.isSafeInteger(manifest.wasm_bytes) && manifest.wasm_bytes > 0) {
      answer.server_wasm_bytes = manifest.wasm_bytes;
    }
    return answer;
  }

  async function reloadInstalledSuccessor() {
    const commit = await installedSuccessorBuild();
    if (commit === null) return false;
    const target = new URL(window.location.href);
    // A pinned `game=` is the world that just ended. The newly installed
    // module starts the next default simulation, not a replay of that seed.
    target.searchParams.delete("game");
    target.searchParams.delete("instance");
    target.searchParams.set("build", commit);
    report.refreshingTo = commit;
    window.location.replace(target.href);
    return true;
  }

  // ------------------------------------------------------- clock and finale

  let pace = 0;
  let paused = false;
  let finaleEndsAt = null;
  // Which hold the countdown belongs to, exactly as the socket server counts
  // its own: bumped when a hold starts or starts over, and published as
  // `restart_hold` so the viewer's clock re-anchors to a re-armed hold.
  let finaleHold = 0;

  // A result holds the screen for the selected interval and then the next
  // world opens. Reading it off each answer keeps the countdown attached to
  // the state the page is actually looking at, rather than to a timer running
  // beside it.
  async function withFinale(state) {
    if (!state || typeof state !== "object") return state;
    const configured = state.between_game_countdown_ms;
    if (typeof configured === "number" && FINALE_OPTIONS_MS.has(configured))
      betweenGameCountdownMs = configured;
    // A finished world is held whether or not it has a winner: a Tactics
    // battle that runs out its clock is drawn, and a draw once held the
    // published build's screen for ever because only a winner counted.
    const finished = state.finished === true || (state.winner !== undefined && state.winner !== null);
    if (!finished) {
      finaleEndsAt = null;
      return state;
    }
    if (finaleEndsAt === null) {
      finaleEndsAt = performance.now() + betweenGameCountdownMs;
      finaleHold += 1;
    }
    const left = finaleEndsAt - performance.now();
    if (left > 0) {
      state.restart_in = Math.ceil(left / 1000);
      state.restart_in_ms = Math.ceil(left);
      state.restart_hold = finaleHold;
      return state;
    }
    finaleEndsAt = null;
    if (await reloadInstalledSuccessor()) return state;
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
      if (typeof parsed.between_game_countdown_ms === "number" &&
          FINALE_OPTIONS_MS.has(parsed.between_game_countdown_ms)) {
        // Changed while a result is being held: the new length counts from
        // now, exactly as the socket server re-arms its own hold. A shorter
        // choice asks for the next world sooner, not for it this instant.
        const changed = betweenGameCountdownMs !== parsed.between_game_countdown_ms;
        betweenGameCountdownMs = parsed.between_game_countdown_ms;
        if (changed && finaleEndsAt !== null) { finaleEndsAt = performance.now() + betweenGameCountdownMs; finaleHold += 1; }
      }
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
      // The engine prices the frame it just played: a seat's share of the
      // whole-turn budget, so Blitz here is the socket server's two turns a
      // second whether the frame is one army of two or one seat of twenty.
      // Waiting the whole `pace` per frame was wrong twice over — every seat
      // frame charged the full turn budget, and until a hand touched the
      // control this clock still held its opening zero while the module
      // reported Blitz, so the exhibition ran unpaced under a Blitz label.
      // An engine too old to price frames keeps the prior whole-pace wait.
      const budget = answer && typeof answer.frame_budget_ms === "number"
        ? answer.frame_budget_ms : pace;
      const owed = budget - (performance.now() - started);
      if (owed > 0) await sleep(owed);
    }

    // Only a world-state response is allowed to start, advance, or clear the
    // finale clock. The viewer asks for metadata such as `/runtime` while the
    // result is on screen; those answers have no `winner` field and used to
    // clear the countdown on every poll, leaving the finished world forever.
    if (path === "/state") answer = await withFinale(answer);

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
    await attachPublishedBuildMetadata(answer);
    answer = await withPublishedArtifact(answer);
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

  // Publication records the optimized module after `wasm-opt` has finished.
  // Rust cannot truthfully bake that size into the module itself: changing the
  // embedded number changes the file, and optimization happens afterwards.
  // Load the adjacent manifest once and attach its exact byte count only when
  // it names the same revision as the running module. That prevents a page
  // caught during a deployment from showing the successor's size.
  let publishedBuildPromise = null;
  function loadPublishedBuild() {
    if (publishedBuildPromise === null) {
      publishedBuildPromise = networkFetch(BUILD_URL, {cache: "no-store"})
        .then(response => response.ok ? response.json() : null)
        .catch(() => null);
    }
    return publishedBuildPromise;
  }
  async function withPublishedArtifact(answer) {
    if (!answer || typeof answer !== "object") return answer;
    const build = await loadPublishedBuild();
    const bytes = build?.wasm_bytes;
    const runningCommit = answer.server_commit ?? answer.commit;
    if (!Number.isSafeInteger(bytes) || bytes <= 0 ||
        typeof build?.commit !== "string" || build.commit !== runningCommit) {
      return answer;
    }
    if (Object.prototype.hasOwnProperty.call(answer, "server_commit")) {
      answer.server_artifact_bytes = bytes;
      answer.server_artifact_kind = "WASM";
    }
    if (Object.prototype.hasOwnProperty.call(answer, "commit")) {
      answer.artifact_bytes = bytes;
      answer.artifact_kind = "WASM";
    }
    report.artifactBytes = bytes;
    return answer;
  }

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
    return ensureOpeningPreset()
      .then(() => serve(method, relative, options_.body || ""))
      .catch((error) => json({ error: String(error.message || error) }));
  };

  // ----------------------------------------------------------- first impression

  // The local game can take a moment to prepare on a first visit. Say what is
  // actually happening in visitor language rather than exposing the engine
  // implementation or implying that a server is being started.
  const curtain = document.createElement("div");
  curtain.id = "civvis-beta-loading";
  curtain.setAttribute("role", "status");
  curtain.setAttribute("aria-live", "polite");
  curtain.innerHTML =
    '<div class="civvis-beta-mark">CIVVIS</div>' +
    '<div class="civvis-beta-note">Starting a new world</div>' +
    '<div class="civvis-beta-detail">Runs in this browser</div>';
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
    .civvis-beta-detail { font: 12px/1.4 system-ui, sans-serif; color: #9eaea6; }
    @media (prefers-reduced-motion: reduce) { #civvis-beta-loading { transition: none; } }
  `;
  const LOADING_NOTICE_DELAY_MS = 350;
  let loadingFinished = false;
  let loadingNoticeTimer = null;
  const raise = () => {
    if (loadingFinished || curtain.isConnected) return;
    document.head.appendChild(style);
    document.body.appendChild(curtain);
  };
  // Fast cached visits should go straight to the game rather than flash a
  // full-screen status card. First visits still get a clear explanation after
  // a short grace period, once there is something worth explaining.
  loadingNoticeTimer = setTimeout(() => {
    if (document.body) raise();
    else document.addEventListener("DOMContentLoaded", raise, { once: true });
  }, LOADING_NOTICE_DELAY_MS);

  // The engine is ready when it has answered something. `/runtime` is the
  // cheapest question there is — it builds no observation at all. A preset
  // visit applies its settings first, so the curtain holds until the world
  // the link named exists rather than dropping on the stock one.
  ensureOpeningPreset()
    .then(() => ask("GET", "/runtime", ""))
    .then(() => {
      report.ready = true;
    })
    .catch((error) => {
      report.error = String(error.message || error);
    })
    .then(() => {
      loadingFinished = true;
      clearTimeout(loadingNoticeTimer);
      if (!curtain.isConnected) return;
      curtain.classList.add("gone");
      setTimeout(() => curtain.remove(), 450);
    });
})();
