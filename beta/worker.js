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
    // The module imports nothing, so there is no glue to keep in step with it
    // and nothing to pass in. Decide from the response MIME type before
    // trying streaming: a basic static host that mistypes the module can fall
    // straight back to bytes instead of downloading the whole module again.
    ready = fetch(wasmUrl)
      .then(async (response) => {
        if (!response.ok) throw new Error(`could not load CIVVIS (${response.status})`);
        const mime = (response.headers.get("Content-Type") || "")
          .split(";", 1)[0].trim().toLowerCase();
        if (mime === "application/wasm" &&
            typeof WebAssembly.instantiateStreaming === "function") {
          return WebAssembly.instantiateStreaming(Promise.resolve(response), {});
        }
        return WebAssembly.instantiate(await response.arrayBuffer(), {});
      })
      .then((result) => {
        compiled = result.module;
        engine = result.instance.exports;
        return engine;
      });
  }
  return ready;
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
    const answer = request(method, path, body, seed);
    self.postMessage({ id, ok: true, answer }, [answer.buffer]);
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
