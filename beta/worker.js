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

function boot(wasmUrl) {
  if (!ready) {
    // The module imports nothing, so there is no glue to keep in step with it
    // and nothing to pass in.
    ready = WebAssembly.instantiateStreaming(fetch(wasmUrl), {})
      .catch(() =>
        // `instantiateStreaming` refuses anything not served as
        // application/wasm. A static host that mistypes the file should cost a
        // millisecond, not the whole page.
        fetch(wasmUrl)
          .then((response) => response.arrayBuffer())
          .then((bytes) => WebAssembly.instantiate(bytes, {})),
      )
      .then((result) => {
        engine = result.instance.exports;
        return engine;
      });
  }
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

function request(method, path, body) {
  const encoded = new TextEncoder().encode(
    JSON.stringify({ method, path, body: body || "" }),
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
  const { id, method, path, body, wasmUrl } = event.data;
  try {
    await boot(wasmUrl);
    const answer = request(method, path, body);
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
