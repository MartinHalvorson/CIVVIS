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
//   * **Saved games.** There is no disk. IndexedDB holds them, and the engine
//     only ever sees a save as an uploaded game.

(function () {
  "use strict";

  const here = new URL(".", document.currentScript.src);
  const WASM_URL = new URL("civvis.wasm", here).href;
  const WORKER_URL = new URL("worker.js", here).href;
  const BUILD_URL = new URL("build.json", here);

  // Everything the engine answers. A path not in here is a real file, such as
  // a strategic map sprite atlas, and goes to the network untouched.
  const ENGINE_ROUTES = new Set([
    "/state", "/status", "/runtime", "/rules", "/gene-program", "/pedia", "/save", "/saves",
    "/load", "/action", "/step", "/autoplay", "/play-on", "/route", "/forecast", "/city-focus", "/city-citizen", "/view",
    "/intel", "/spectator-status", "/next-game-settings", "/new",
    "/supervisor-new", "/pace", "/next-game",
    "/machine-metrics", "/civ6", "/civ6/start", "/policy-effects", "/map-search-buildings",
    "/live-pause", "/live-new-game",
  ]);
  const LOCAL_DESKTOP_HOST = here.pathname.startsWith("/wasm/");
  // The page's front door is for the public site: the desktop app's local
  // channel was opened by someone who already chose what they are doing.
  window.CIVVIS_PUBLISHED_BUILD = !LOCAL_DESKTOP_HOST;

  // Seed public-site defaults before the viewer reads its preferences. An
  // explicit choice, including spin off or no trail, always wins on return.
  // The spin key is the viewer's `WORLD_SPIN_STORAGE_KEY`: only its button
  // and bar write there, never a drag of the map.
  function applyPublishedViewDefaults(storage) {
    for (const [key, value] of [
      ["civvis-world-spin-2", "1"],
      ["civvis-unit-tail-turns", "2"],
    ]) {
      if (storage.getItem(key) === null) storage.setItem(key, value);
    }
  }
  if (!LOCAL_DESKTOP_HOST) {
    try { applyPublishedViewDefaults(localStorage); } catch (_) {}
  }

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
  // gets for free: the engine plays a frame only for a request that names the
  // frame before it, and the viewer only issues that request after `render()`
  // has finished the previous turn. The read-ahead below asks for exactly one
  // frame early — the one the page is about to ask for — and never a second
  // until the page has taken the first, so frames still reach the page one at
  // a time and in order and none can be played past unseen. The
  // frame-per-turn contract is structural here rather than enforced by a
  // delivery gate. `turns` and `paints` should match.
  const report = (window.__civvisBeta = {
    ready: false,
    error: null,
    requests: 0,
    turns: 0,     // turns the engine played for this page
    paints: 0,    // next-turn requests, each one an acknowledged repaint
    aheadHits: 0,   // frames the page was handed from the read-ahead
    aheadMisses: 0, // read-ahead frames the page turned out not to ask for
    lastTurn: null,
    seenTurns: new Set(),
    fullMapFrames: 0,
    patchFrames: 0,
    fullMapTiles: 0,
    patchTiles: 0,
    fullMapBytes: 0,
    patchBytes: 0,
  });

  // The network as the page found it. Everything the engine does not answer
  // goes here untouched, and so do this file's own reads of the manifest.
  const networkFetch = window.fetch.bind(window);

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
    if (waiting.raw) { waiting.resolve(answer); return; }
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

  function ask(method, path, body, raw = false) {
    return new Promise((resolve, reject) => {
      const id = nextId++;
      pending.set(id, { resolve, reject, path, raw });
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

  // ------------------------------------------------------------ read-ahead

  // A watched turn used to cost the engine's time *plus* the page's: the
  // engine played a frame only when asked, the page asked only once it had
  // painted the frame before, and so each stood idle while the other worked.
  // In the opening turns the two are about the same size — 25 to 50 ms of
  // engine against 30 ms of parse, paint and wait for the display — so the
  // turn counter ran at little more than half the pace either side could
  // keep, and it opened slowest of all: the first step waited behind the
  // first paint, the largest paint of the game.
  //
  // So the frame the page is about to ask for is asked for here, the moment
  // the page is handed the one before it. The request is the page's own next
  // request, predicted: the watching loop names the frame it holds and the
  // last thought it holds. The frame is the one in the answer it was just
  // given. The thought is that answer's too if the page has been taking each
  // answer's reasoning in as it arrives, and otherwise the one it named last
  // time — the viewer only takes reasoning in when it repaints its side
  // panel, and which of the two it is doing shows in what it asks. When the
  // page does ask, the answer is already in hand or already on its way. A
  // turn then costs the larger of the two times instead of their sum.
  //
  // Only at Lightning. At every slower pace a frame is held for its share of
  // the turn budget, the engine is finished long before the page may have it,
  // and there is nothing to gain by asking early.
  //
  // It is one frame and never two. A second is not asked for until the page
  // has taken the first, so the engine is never more than one frame past the
  // glass and every frame still reaches the page in order. And it is only
  // ever a guess at a request: the engine answers what it is actually asked.
  // If the page asks for something else — it resynchronised, or its cursor
  // moved — the guess is dropped and the page's request goes to the engine as
  // written, which answers with the world as it stands and a whole map, the
  // way it answers any page that is not holding the frame on the table.
  // Anything that can change the world or what a state says about it — a
  // pause, a pace, a new game, a seat to watch as — drops the guess the same
  // way, so a frame built before the change is never shown after it.
  //
  // `?ahead=0` turns it off, which is how its effect is measured.
  const READ_AHEAD = new URL(window.location.href).searchParams.get("ahead") !== "0";
  // A page that never asks for the predicted frame is a viewer this file does
  // not understand. Each miss costs a whole map where a patch would have
  // done, so a run of them ends the guessing for this visit.
  const READ_AHEAD_MISS_LIMIT = 3;
  let ahead = null; // { have, think, answer: Promise } — the one frame asked for early
  let aheadMispredicted = 0; // in a row
  // The world and the last thought in the state the page was most recently
  // handed, to tell from its next request whether it took that reasoning in.
  let handedSeed = null;
  let handedCursor = null;

  function queryValue(target, name) {
    const match = new RegExp(`[?&]${name}=([^&]*)`).exec(target);
    return match ? match[1] : null;
  }

  function dropAhead(mispredicted) {
    if (ahead === null) return;
    ahead = null;
    report.aheadMisses++;
    if (mispredicted) aheadMispredicted++;
  }

  // Called with the state the page is about to be handed, and the thought its
  // request for that state named.
  function readAhead(answer, askedThink) {
    if (!answer || typeof answer !== "object") return;
    const cursor = answer.ai_reasoning ? answer.ai_reasoning.cursor : null;
    // Whether the page took in the reasoning it was handed last time. A page
    // that did names that state's last thought; one that did not is still
    // naming an older one, and will go on naming it until it repaints.
    const keepsUp = answer.seed !== handedSeed || handedCursor === null ||
      askedThink === handedCursor;
    handedSeed = answer.seed;
    handedCursor = Number.isSafeInteger(cursor) ? String(cursor) : null;
    if (!READ_AHEAD || ahead !== null || paused || answer.error) return;
    if (aheadMispredicted >= READ_AHEAD_MISS_LIMIT) return;
    // Only a watched world plays itself, and a held or finished one does not.
    if (answer.spectate !== true || answer.paused === true) return;
    if (answer.finished === true || (answer.winner !== undefined && answer.winner !== null)) return;
    if (answer.pace !== 0) return;
    if (![answer.seed, answer.turn, answer.frame_sequence, cursor].every(Number.isSafeInteger)) return;
    const have = `${answer.seed}:${answer.turn}:0:${answer.frame_sequence}`;
    const think = keepsUp || askedThink === null ? String(cursor) : askedThink;
    const reading = { have, think, answer: ask("GET", `/state?have=${have}&think=${think}`, "") };
    // Its failure belongs to the request that is handed it. One nobody takes
    // must not surface as an unhandled rejection.
    reading.answer.catch(() => {});
    ahead = reading;
  }

  // The page asking for a state. The read-ahead is the answer when it is the
  // very request the page made; a whole-world read that names no frame is the
  // engine's to answer and leaves the read-ahead standing, because it changes
  // nothing the read-ahead was built on.
  function takeAhead(target) {
    if (ahead === null) return null;
    const have = queryValue(target, "have");
    if (have === null) return null;
    if (ahead.have === have && ahead.think === queryValue(target, "think")) {
      const reading = ahead;
      ahead = null;
      report.aheadHits++;
      aheadMispredicted = 0;
      return reading.answer;
    }
    dropAhead(true);
    return null;
  }

  // What leaves a frame already asked for still true. Every `GET` is a read.
  // The page's status heartbeat restates the pause it already has, and only
  // changes anything when it says paused; saving reads the engine and writes
  // to this page's own storage.
  function keepsReadAhead(method, path, parsed) {
    if (method === "GET") return true;
    if (path === "/save") return true;
    return path === "/spectator-status" && parsed.paused !== true;
  }

  // ------------------------------------------------------- opening settings

  // The pace and the between-game hold the page is about to ask for, sent
  // ahead of its first read.
  //
  // The module opens every world at Blitz with a ten-second hold; the viewer
  // opens the public site at Lightning with a five-second one, or on whatever
  // the visitor last picked. It used to say so only after its first paint:
  // two `/pace` posts on the first frame, each answered with a whole state
  // the page parsed and drew again, in the same moment the first turns were
  // due. Said here instead, the first state the page reads already agrees
  // with it, nothing is posted, and the first frame after it can be asked for
  // early at the right pace.
  //
  // These are the viewer's rules (`restorePace`, `restoreBetweenGameCountdown`
  // in `web/assets/app.js`) and `beta/shim_test.py` holds the two files to the
  // same keys and values. If they ever did differ, the viewer would post its
  // own choice on its first frame exactly as it always has.
  //
  // A link that configured its own world keeps the module's pace: a Tactics
  // battle opens at Blitz whatever the visitor watches full games at, and the
  // viewer is the one that knows which kind of world arrived.
  const WATCH_PACE_KEY = "civvis-watch-pace";
  const WATCH_PACES_MS = new Set([0, 500, 1000, 2000, 4000]);
  const PUBLISHED_SITE_PACE_MS = 0;
  const FINALE_KEY = "civvis-between-game-countdown";
  const DEFAULT_FINALE_MS = 5000;
  let openingSettingsSent = false;
  function sendOpeningSettings() {
    if (openingSettingsSent) return;
    openingSettingsSent = true;
    const stored = (key) => {
      try {
        const value = localStorage.getItem(key);
        return value === null || value === "" ? NaN : Number(value);
      } catch (_) {
        return NaN;
      }
    };
    const settings = {};
    if (OPENING_PRESET === null) {
      const picked = stored(WATCH_PACE_KEY);
      if (WATCH_PACES_MS.has(picked)) settings.ms = picked;
      else if (window.CIVVIS_PUBLISHED_SITE) settings.ms = PUBLISHED_SITE_PACE_MS;
    }
    const hold = stored(FINALE_KEY);
    settings.between_game_countdown_ms = FINALE_OPTIONS_MS.has(hold) ? hold : DEFAULT_FINALE_MS;
    if (typeof settings.ms === "number") pace = settings.ms;
    betweenGameCountdownMs = settings.between_game_countdown_ms;
    // The answer is a whole state nobody reads; taken as bytes, it is never
    // parsed. Requests reach the engine in the order they are posted, so the
    // page's first read is answered at this pace without waiting here.
    ask("POST", "/pace", JSON.stringify(settings), true).catch(() => {});
  }

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
  //
  // Read once, and asked for while this script is still loading. It used to
  // be asked for by the first engine answer that wanted it, and twice over —
  // the size and the build hour each kept a reading of their own — so the
  // opening `/rules` and the opening `/state` each stood still for a round
  // trip to the network while a ready engine waited: 0.2 to 0.3 s of every
  // visit on a fast line, more on a slow one. The module is megabytes and
  // this is a few hundred bytes, so asked for here it is in hand first.
  const runningBuildManifest = networkFetch(BUILD_URL, { cache: "no-store" })
    .then((response) => response.ok ? response.json() : null)
    .catch(() => null);
  function loadRunningBuildManifest() {
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

  function reloadInstalledSuccessor(commit) {
    const target = new URL(window.location.href);
    // A pinned `game=` is the world that just ended. The newly installed
    // module starts the next default simulation, not a replay of that seed.
    target.searchParams.delete("game");
    target.searchParams.delete("instance");
    target.searchParams.set("build", commit);
    report.refreshingTo = commit;
    window.location.replace(target.href);
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
    // A paused result stays on screen for as long as it is paused, which is
    // what the page says and what a socket server does. Voiding the clock in
    // `/pace` alone was not enough: this ran on the very next poll and armed it
    // again, so the "paused" result was replaced ten seconds later anyway.
    if (!finished || paused) {
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
    const successor = await installedSuccessorBuild();
    // That check crossed the network, and One more turn or a pause can land
    // while it is out. The world as it stands now decides, never the answer
    // this count began from: only a result still on the table is replaced.
    const current = await ask("GET", "/status", "");
    if (paused || current.finished !== true || current.seed !== state.seed) {
      return await ask("GET", "/state", "");
    }
    if (successor !== null) {
      reloadInstalledSuccessor(successor);
      return state;
    }
    return await ask("POST", "/next-game", "{}");
  }

  // ------------------------------------------------------------------ saves

  // A game is megabytes. In localStorage that was a game killer on two counts:
  // the whole browser gets about 5 MB there, which a Tiny map reached by about
  // turn 130, and the write was synchronous on the page's thread every turn.
  // IndexedDB holds as much as the disk will give it, and takes the save as the
  // bytes the engine produced, so nothing here parses or stringifies a world.
  // What the list needs is stored apart, so listing never loads a game.
  const SAVE_DB = "civvis-saves";
  const SAVE_META = "meta";
  const SAVE_BODY = "body";
  let saveDb = null;

  function saveDatabase() {
    if (saveDb === null) {
      saveDb = new Promise((resolve, reject) => {
        const opening = indexedDB.open(SAVE_DB, 1);
        opening.onupgradeneeded = () => {
          opening.result.createObjectStore(SAVE_META, { keyPath: "name" });
          opening.result.createObjectStore(SAVE_BODY);
        };
        opening.onsuccess = () => resolve(opening.result);
        opening.onerror = () => reject(opening.error);
      }).then(moveLegacySaves);
    }
    return saveDb;
  }

  function finished(transaction, value) {
    return new Promise((resolve, reject) => {
      transaction.oncomplete = () => resolve(value());
      transaction.onerror = () => reject(transaction.error);
      transaction.onabort = () => reject(transaction.error);
    });
  }

  function writeRecord(db, meta, bytes) {
    const transaction = db.transaction([SAVE_META, SAVE_BODY], "readwrite");
    transaction.objectStore(SAVE_META).put(meta);
    transaction.objectStore(SAVE_BODY).put(bytes, meta.name);
    return finished(transaction, () => meta);
  }

  // What a save written before this change left behind. It moves once, and the
  // localStorage copy goes only after IndexedDB has confirmed it holds the game.
  async function moveLegacySaves(db) {
    const keys = [];
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key && key.startsWith(SAVE_PREFIX)) keys.push(key);
    }
    for (const key of keys) {
      const text = localStorage.getItem(key);
      try {
        const held = JSON.parse(text);
        const game = held.game ?? held;
        await writeRecord(db, {
          name: key.slice(SAVE_PREFIX.length),
          turn: game.turn ?? 0,
          civ: game.players?.[game.human_seats?.[0] ?? 0]?.civ ?? null,
          difficulty: game.difficulty ?? null,
          savedAt: Date.now(),
        }, new TextEncoder().encode(text));
        localStorage.removeItem(key);
      } catch (error) {
        // A half-written entry is not a save. It stays where it is and costs
        // nothing here; the list no longer reads that storage.
      }
    }
    return db;
  }

  async function storedSaves() {
    const db = await saveDatabase();
    const reading = db.transaction(SAVE_META).objectStore(SAVE_META).getAll();
    const saves = await new Promise((resolve, reject) => {
      reading.onsuccess = () => resolve(reading.result);
      reading.onerror = () => reject(reading.error);
    });
    return saves.sort((a, b) => (b.savedAt ?? 0) - (a.savedAt ?? 0));
  }

  async function readSave(name) {
    const db = await saveDatabase();
    const reading = db.transaction(SAVE_BODY).objectStore(SAVE_BODY).get(name);
    return new Promise((resolve, reject) => {
      reading.onsuccess = () => resolve(reading.result ?? null);
      reading.onerror = () => reject(reading.error);
    });
  }

  // The world a save belongs to, as the last answer described it. Reading it
  // back out of the save would mean parsing the thing this exists to avoid.
  const seenWorld = { turn: 0, civ: null, difficulty: null };
  function noteWorld(answer) {
    // The seat at the keyboard is always the first player.
    if (!answer || answer.spectate !== false || typeof answer.turn !== "number") return;
    seenWorld.turn = answer.turn;
    if (answer.difficulty) seenWorld.difficulty = answer.difficulty;
    const civ = answer.players?.[0]?.civ;
    if (civ) seenWorld.civ = civ;
  }

  async function writeSave(name) {
    const bytes = await ask("GET", "/save", "", true);
    try {
      await writeRecord(await saveDatabase(),
        { name, ...seenWorld, savedAt: Date.now(), bytes: bytes.byteLength }, bytes);
    } catch (error) {
      return { error: `there is no room to save ${name}: ${error.message}` };
    }
    return { error: null, name, turn: seenWorld.turn };
  }

  // --------------------------------------------------------------- dispatch

  // An engine answer, as the `Response` the page's `fetch` expects.
  //
  // The answer is already an object here: it was parsed once, off the bytes
  // the worker sent. It used to be turned back into text for the `Response`
  // and parsed a second time by the page — a megabyte and a half at boot, a
  // few hundred kilobytes every turn after, all on the thread that paints.
  // The page reads these with `.json()`, so that is answered with the object
  // itself; `.text()` still gives the text to anything that wants it.
  function json(value) {
    const response = new Response(null, {
      status: 200,
      headers: { "Content-Type": "application/json" },
    });
    response.json = () => Promise.resolve(value);
    response.text = () => Promise.resolve(JSON.stringify(value));
    response.clone = () => json(value);
    return response;
  }

  async function serve(method, target, body) {
    const path = target.split("?")[0];
    const parsed = body ? safeParse(body) : {};
    if (!keepsReadAhead(method, path, parsed)) dropAhead(false);

    // The routes that need storage the engine cannot reach.
    if (method === "GET" && path === "/saves") {
      try {
        return json({ saves: await storedSaves() });
      } catch (error) {
        return json({ saves: [], error: `saves are unavailable: ${error.message}` });
      }
    }

    if (method === "POST" && path === "/save") {
      const name = String(parsed.name || "");
      if (!/^[A-Za-z0-9_-]+$/.test(name)) {
        return json({ error: "a save name is letters, digits, - and _" });
      }
      return json(await writeSave(name));
    }

    if (method === "POST" && path === "/load" && parsed.name) {
      const held = await readSave(String(parsed.name));
      if (held === null) return json({ error: `there is no save called ${parsed.name}` });
      // Spliced, not parsed: the engine reads this request, and nothing on the
      // page needs to look inside the game on its way there.
      const game = new TextDecoder().decode(held);
      return json(await ask("POST", "/load", `{"game":${game}}`));
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
      // A page that changed how it watches may ask differently from here on,
      // so a read-ahead that had stopped guessing starts again.
      aheadMispredicted = 0;
      return json(await ask("POST", "/pace", body));
    }

    // A spectator asking for the next turn. On a socket this request is held
    // until a stepper thread has played past the frame the page named; here
    // the request *is* the step, so the wait that paces it happens after.
    const wantsNextTurn = path === "/state" && /[?&]have=/.test(target);
    if (wantsNextTurn) report.paints++;
    if (wantsNextTurn && paused) await sleep(PAUSED_POLL_MS);

    const started = performance.now();
    const worldRead = method === "GET" && path === "/state";
    let answer = await ((worldRead && takeAhead(target)) || ask(method, target, body));
    // The page's own read of the world — the watching loop's, or the read it
    // opens on — and so the one whose successor it will ask for next. The
    // planet's geometry is read through `/state` too and is neither.
    if (worldRead && (wantsNextTurn || /[?&]painted=/.test(target))) {
      readAhead(answer, queryValue(target, "think"));
    }
    // Playing on sets the engine's pause from its own request, so this clock
    // has to be told, or the resumed world is priced and held as a paused one.
    if (path === "/play-on" && typeof answer?.paused === "boolean") paused = answer.paused;
    noteWorld(answer);
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

  // Publication records the optimized module after `wasm-opt` has finished.
  // Rust cannot truthfully bake that size into the module itself: changing the
  // embedded number changes the file, and optimization happens afterwards.
  // Read the adjacent manifest once and attach its exact byte count only when
  // it names the same revision as the running module. That prevents a page
  // caught during a deployment from showing the successor's size.
  function loadPublishedBuild() {
    return loadRunningBuildManifest();
  }
  async function withPublishedArtifact(answer) {
    if (!answer || typeof answer !== "object") return answer;
    const build = await loadPublishedBuild();
    const runningCommit = answer.server_commit ?? answer.commit;
    if (typeof build?.commit !== "string" || build.commit !== runningCommit) return answer;
    // When the module was built is the manifest's to say, for the same reason
    // as its size. Published builds no longer stamp the hour into the module:
    // those few bytes gave an unchanged engine a new address on every
    // deployment, and each visitor downloaded and compiled all of it again.
    // A module that does carry its own stamp keeps it.
    const builtAt = build.built_at;
    if (typeof builtAt === "string" && builtAt) {
      if (Object.prototype.hasOwnProperty.call(answer, "server_commit") &&
          (answer.server_built_at === null || answer.server_built_at === undefined)) {
        answer.server_built_at = builtAt;
      }
      if (Object.prototype.hasOwnProperty.call(answer, "commit") &&
          (answer.built_at === null || answer.built_at === undefined)) {
        answer.built_at = builtAt;
      }
    }
    const bytes = build.wasm_bytes;
    if (!Number.isSafeInteger(bytes) || bytes <= 0) return answer;
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
      .then(() => {
        sendOpeningSettings();
        return serve(method, relative, options_.body || "");
      })
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
