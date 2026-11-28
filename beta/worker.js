// The engine, off the main thread.
//
// `civvis.wasm` answers one request at a time and a turn on a large map is not
// a quick call. On the page's own thread that is a freeze: the spectator loop
// paints on `requestAnimationFrame`, so an engine that blocks the thread stops
// the very frames it is producing turns for. Here it blocks nothing anyone can
// see.
//
// The protocol is the module's own: UTF-8 JSON in, UTF-8 JSON out, with the
// answer's byte length as a little-endian `u32` in front of it.

let engine = null;
let ready = null;
// The compiled module. A fork is handed this rather than sent to the network
// for it, so a fork is running the very code its engine was.
let compiled = null;

function boot(wasmUrl) {
  if (!ready) {
    ready = download(wasmUrl).then((result) => {
      compiled = result.module;
      engine = result.instance.exports;
      return engine;
    });
  }
  return ready;
}

// The engine's download, seen through. A first visit draws a baked opening
// before the engine has arrived (`shim.js`, "baked"), and it was asked for
// once: a download that failed kept its failure for every request after it,
// and one that stopped was waited on for ever. Either way the opening stood
// on screen with its clock running and nothing moving (operator 2026-11-27:
// "civvis,.ai doesnt always appear to start a game", "sometimes has map and
// clock starts but nothing moving"). Now a download that goes STALL_MS
// without a byte is given up, and one that broke on the way is asked for
// again after a pause that grows, for as long as the page is open. A slow
// download is never given up while bytes are still arriving.
//
// What it has got so far goes to the page as it comes (`loading`), so a page
// with a world on screen and no engine yet can say so.
const STALL_MS = 15000;
const RETRY_AFTER_MS = [1000, 2000, 4000, 8000, 15000];
const TOLD_EVERY_MS = 250;

async function download(wasmUrl) {
  for (let attempt = 0; ; attempt += 1) {
    try {
      const result = await downloadOnce(wasmUrl, attempt);
      tell({ attempt, done: true });
      return result;
    } catch (error) {
      if (!error || !error.retry) {
        tell({ attempt, failed: String((error && error.message) || error) });
        throw error;
      }
      const pause = RETRY_AFTER_MS[Math.min(attempt, RETRY_AFTER_MS.length - 1)];
      tell({ attempt, retrying: pause, why: error.message });
      await new Promise((resolve) => setTimeout(resolve, pause));
    }
  }
}

let toldAt = -Infinity;
function tell(loading, often = true) {
  const now = performance.now();
  if (!often && now - toldAt < TOLD_EVERY_MS) return;
  toldAt = now;
  self.postMessage({ loading });
}

function retryable(error) {
  const again = new Error((error && error.message) || String(error));
  again.retry = true;
  return again;
}

async function downloadOnce(wasmUrl, attempt) {
  const abort = new AbortController();
  let stalled = false;
  let broken = false;
  let timer = 0;
  const watch = () => {
    clearTimeout(timer);
    timer = setTimeout(() => { stalled = true; abort.abort(); }, STALL_MS);
  };
  watch();
  try {
    let response;
    try {
      // A try after the first goes to the network itself: the browser would
      // otherwise hand it the copy the try before it stopped on.
      response = await fetch(wasmUrl, { signal: abort.signal, cache: attempt ? "no-store" : "default" });
    } catch (error) {
      throw retryable(stalled ? new Error("the engine's download stopped") : error);
    }
    if (!response.ok) {
      const error = new Error(`could not load CIVVIS (${response.status})`);
      error.retry = response.status >= 500 || response.status === 408 || response.status === 429;
      throw error;
    }
    // The module imports nothing, so there is no glue to keep in step with it
    // and nothing to pass in. Decide from the response MIME type before
    // trying streaming: a basic static host that mistypes the module can fall
    // straight back to bytes instead of downloading the whole module again.
    const mime = (response.headers.get("Content-Type") || "")
      .split(";", 1)[0].trim().toLowerCase();
    // The bytes are counted on their way to the compiler: each one puts the
    // stall off again.
    const reader = response.body.getReader();
    let bytes = 0;
    const counted = new ReadableStream({
      async pull(controller) {
        let part;
        try {
          part = await reader.read();
        } catch (error) {
          broken = true;
          controller.error(error);
          return;
        }
        if (part.done) {
          clearTimeout(timer);
          controller.close();
          return;
        }
        bytes += part.value.byteLength;
        watch();
        tell({ attempt, bytes }, false);
        controller.enqueue(part.value);
      },
      cancel(reason) {
        return reader.cancel(reason);
      },
    });
    const body = new Response(counted, { headers: { "Content-Type": "application/wasm" } });
    try {
      if (mime === "application/wasm" &&
          typeof WebAssembly.instantiateStreaming === "function") {
        return await WebAssembly.instantiateStreaming(body, {});
      }
      return await WebAssembly.instantiate(await body.arrayBuffer(), {});
    } catch (error) {
      // A module cut off on its way is not a broken module: it is asked for
      // again. One that arrived whole and does not compile is.
      if (stalled) throw retryable(new Error("the engine's download stopped"));
      if (broken) throw retryable(error);
      throw error;
    }
  } finally {
    clearTimeout(timer);
  }
}

// ---------------------------------------------------------------------- forks
//
// The module imports nothing and keeps everything it knows in its own memory:
// the game, the turns behind it, both halves of a split. Between two requests
// nothing of it is anywhere else. So a second instance of the same module,
// given a copy of that memory, *is* the same engine, and from there the two
// play their own histories. Copying takes about as long as the memory is big
// (a few tens of milliseconds) where saving a game and loading it takes
// seconds, and a fork runs on its own thread: this is what lets many futures
// of one world be played at the same time (`shim.js`, "forks").

const PAGE_BYTES = 65536;

// This engine's memory as it stands: one half of a fork.
function leave() {
  return { module: compiled, memory: engine.memory.buffer.slice(0) };
}

// Become the engine that memory was taken from. `ready` is replaced before
// anything is awaited, so a request posted straight after this one waits for
// the adopted engine rather than fetching a new one.
function adopt(module, memory) {
  ready = WebAssembly.instantiate(module, {}).then((instance) => {
    const fresh = instance.exports;
    const short = memory.byteLength - fresh.memory.buffer.byteLength;
    if (short < 0 || short % PAGE_BYTES !== 0) {
      throw new Error("that memory was not taken from this engine");
    }
    if (short > 0) fresh.memory.grow(short / PAGE_BYTES);
    new Uint8Array(fresh.memory.buffer).set(new Uint8Array(memory));
    compiled = module;
    engine = fresh;
    return engine;
  });
  return ready;
}

// Read a length-prefixed answer out of the module's memory and free it.
function take(ptr) {
  const length = new DataView(engine.memory.buffer).getUint32(ptr, true);
  const bytes = new Uint8Array(
    engine.memory.buffer.slice(ptr + 4, ptr + 4 + length),
  );
  engine.civvis_free(ptr, 4 + length);
  return bytes;
}

// A Rust panic on wasm32 aborts, and all the caller sees is `unreachable` and
// a list of function indices. The module records the message before it dies,
// and its memory survives the trap, so the real diagnosis is still there to be
// asked for.
function lastPanic() {
  try {
    const text = new TextDecoder().decode(take(engine.civvis_last_panic()));
    return text || null;
  } catch (error) {
    return null;
  }
}

function request(method, path, body, seed) {
  const encoded = new TextEncoder().encode(
    JSON.stringify({ method, path, body: body || "", seed }),
  );
  const inPtr = engine.civvis_alloc(encoded.length);
  // Every view is taken *after* the call that might have grown the module's
  // memory: growth detaches the old ArrayBuffer, and a view built before it
  // reads zeroes.
  new Uint8Array(engine.memory.buffer, inPtr, encoded.length).set(encoded);

  // Copied out of the module's memory before it is freed, and sent on as a
  // transferable so the answer crosses to the page without a second copy.
  return take(engine.civvis_request(inPtr, encoded.length));
}

// How long the engine has spent answering, since it began.
let busyMs = 0;

// The engine cannot see the computer from inside a page, so its answer to
// `/machine-metrics` says nothing of it. What is known here is added: the
// memory the module holds, and the time it has spent at work. A page that
// shows what it costs (Doctor Strange's Civ 6, `strangeMeasure`) reads how
// busy the engine is from two of these.
function metered(answer) {
  try {
    const told = JSON.parse(new TextDecoder().decode(answer));
    if (!told || typeof told !== "object" || Array.isArray(told)) return answer;
    told.engine_bytes = engine.memory.buffer.byteLength;
    told.engine_busy_ms = Math.round(busyMs);
    return new TextEncoder().encode(JSON.stringify(told));
  } catch (error) {
    return answer;
  }
}

self.onmessage = async (event) => {
  const { id, method, path, body, wasmUrl, seed, fork } = event.data;
  try {
    if (fork === "adopt") {
      await adopt(event.data.module, event.data.memory);
      self.postMessage({ id, ok: true, answer: null });
      return;
    }
    await boot(wasmUrl);
    if (fork === "leave") {
      const answer = leave();
      self.postMessage({ id, ok: true, answer }, [answer.memory]);
      return;
    }
    const began = performance.now();
    let answer = request(method, path, body, seed);
    busyMs += performance.now() - began;
    if (String(path).split("?")[0] === "/machine-metrics") answer = metered(answer);
    // What this engine holds and has spent rides on every answer, so a page
    // with many engines can read them all without asking one that is busy
    // (`shim.js`, "a world's own engine").
    const meter = { bytes: engine.memory.buffer.byteLength, busy: Math.round(busyMs) };
    self.postMessage({ id, ok: true, answer, meter }, [answer.buffer]);
  } catch (error) {
    const reported = String((error && error.message) || error);
    const panic = engine ? lastPanic() : null;
    self.postMessage({
      id,
      ok: false,
      error: panic ? `${panic} (${reported})` : reported,
    });
  }
};
