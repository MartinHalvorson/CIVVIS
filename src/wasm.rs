//! The browser build's request router.
//!
//! `civvis.ai/beta` serves the viewer as a static page with nothing running
//! behind it: the engine is compiled to WebAssembly and answers, in the page's
//! own address space, the very requests `web/index.html` already makes over
//! HTTP. This module is the answering half.
//!
//! It is a child of [`server`](super) so it can reach the same private helpers
//! the socket handler uses — `request_path`, `query_value`, `new_game_params`,
//! `Session::start_new_game` — without widening any of them to the crate's
//! public surface, and it is `cfg`-gated to wasm so no native build ever
//! compiles a line of it.
//!
//! What it deliberately does *not* carry is everything the socket server does
//! for readers other than the one page: turn pacing, a multi-viewer
//! frame-delivery gate, and the supervisor handoff. One page driving one game
//! on one thread has nobody to synchronise with, so the *page* owns the clock
//! and the JavaScript shim turns its poll into a step. It does retain one
//! acknowledged tile baseline: avoiding a megabyte transfer for terrain that
//! did not change is worthwhile even with exactly one viewer. `pace` and
//! `paused` are kept here because `/state` is expected to carry them back.

use super::*;
use std::cell::{Cell, RefCell};

thread_local! {
    /// The one game this page is playing.
    static SESSION: RefCell<Option<Session>> = const { RefCell::new(None) };
    /// Why the module last died.
    ///
    /// A panic on `wasm32-unknown-unknown` unwinds nowhere: it aborts, and the
    /// caller sees `RuntimeError: unreachable` and a list of function indices.
    /// That is not enough to fix anything, and this module is a long way from
    /// a debugger. The hook below records the message before the trap; the
    /// instance's memory outlives the trap, so [`civvis_last_panic`] can still
    /// read it afterwards.
    static LAST_PANIC: RefCell<String> = const { RefCell::new(String::new()) };
    /// Setup chosen for the next world. On a server this lives on `Shared`,
    /// off the simulation lock; here there is no lock and no second thread, so
    /// it is simply the module's own cell.
    static NEXT_GAME_PARAMS: RefCell<Option<Params>> = const { RefCell::new(None) };
    static HOOKED: Cell<bool> = const { Cell::new(false) };
    /// What the socket build keeps in atomics on `Shared`. Nothing in this
    /// build reads them but the page that set them.
    static PACE: Cell<u64> = const { Cell::new(500) };
    static BETWEEN_GAME_COUNTDOWN_MS: Cell<u64> =
        const { Cell::new(DEFAULT_BETWEEN_GAME_COUNTDOWN_MS) };
    static PAUSED: Cell<bool> = const { Cell::new(false) };
    /// Monotonic identity of the frame most recently completed for this page.
    /// Multiple player turns share one game turn at Blitz and slower, so
    /// `(seed, turn, finished)` no longer identifies every paint boundary.
    static FRAME_SEQUENCE: Cell<u64> = const { Cell::new(0) };
    /// The browser has one viewer and one map baseline. Keep just the compact
    /// per-tile marks here rather than sending its immutable terrain back over
    /// the worker boundary on every rendered turn.
    static BROWSER_TILE_MARKS: RefCell<Option<(SpectatorFrame, Vec<u64>)>> =
        const { RefCell::new(None) };
    /// The seed the opening world is rolled from.
    ///
    /// A module has no clock and no entropy of its own, and this one imports
    /// nothing, so variety has to arrive from outside: left to itself every
    /// visitor to civvis.ai/beta would watch the same six civilizations play
    /// the same map for ever. The page sends one seed per load and the first
    /// request to need a world uses it.
    static OPENING_SEED: Cell<u64> = const { Cell::new(1) };
}

/// The world a page opens on before anybody has visited the lobby.
///
/// The stock opening world — see `stock_opening_params` in the parent for
/// the one description of it — rolled from the seed the page supplied.
fn opening_params() -> Params {
    stock_opening_params(OPENING_SEED.with(Cell::get))
}

fn browser_session(params: Params) -> Session {
    Session::new(params)
}

fn start_automatic_browser_next_game(session: &mut Session, queued: Option<Params>) {
    let next_seed = automatic_successor_seed(session.params.seed);
    let mut params = queued.unwrap_or_else(|| session.params.clone());
    params.seed = next_seed;
    *session = browser_session(params);
}

fn reseat_browser_session_from_host(session: &mut Session) {
    let view = session.view_player;
    let params = session.params.clone();
    let mut next = browser_session(params);
    next.view_player = view.filter(|pid| {
        next.game
            .players
            .get(*pid)
            .is_some_and(|player| !player.is_minor && !player.is_barbarian)
    });
    *session = next;
}

/// Run `f` against this page's session, creating the opening world the first
/// time anything asks for it.
fn with_session<T>(f: impl FnOnce(&mut Session) -> T) -> T {
    SESSION.with(|cell| {
        let mut held = cell.borrow_mut();
        if held.is_none() {
            *held = Some(browser_session(opening_params()));
        }
        f(held.as_mut().expect("the session was just created"))
    })
}

/// The frame a page is looking at: what the stepper calls a completed turn.
fn current_frame(session: &Session) -> SpectatorFrame {
    spectator_frame(&session.game, FRAME_SEQUENCE.with(Cell::get))
}

/// Play the world on until it is no longer the frame the page is holding.
///
/// This is [`auto_step_loop`](super::auto_step_loop)'s inner move: take one
/// seat at a time and stop at the next publication boundary. Lightning keeps
/// one frame per round; Blitz and every slower offered pace stop after the
/// first seat, so each major, city-state, and barbarian moves in its own frame.
///
/// It returns without playing anything when the page is holding a frame that
/// is not the one on the table — a stale tab, or a world it has not caught up
/// with — because the turn it is asking for already exists.
///
/// The returned figure is the frame's wall-clock price in milliseconds: the
/// same seat shares [`auto_step_loop`](super::auto_step_loop) sleeps between
/// steps, summed over the steps this frame contains. This build has no thread
/// to spend it on — the shim owns the clock — so the price rides back on the
/// `/state` answer as `frame_budget_ms` and the shim holds the reply that
/// long. Pricing it here rather than in the shim is what keeps Blitz meaning
/// the same two turns a second it means on a socket: only the engine knows
/// how many living seats divide the turn budget, and which structure the
/// world is being played under.
fn advance_one_frame(session: &mut Session, held: SpectatorFrame) -> u64 {
    if !session.params.spectate
        || PAUSED.with(Cell::get)
        || session.game.is_finished()
        || current_frame(session) != held
    {
        return 0;
    }
    let pace = PACE.with(Cell::get);
    let mut budget: u64 = 0;
    // A turn is one step per seat, so the frame is normally a round away. The
    // cap is not a policy, only a promise that a seat which somehow never
    // completes returns control to the page instead of hanging the tab; it
    // sits well above the largest roster the lobby can seat.
    const STEP_CAP: usize = 1024;
    for _ in 0..STEP_CAP {
        let turn_before = session.game.turn;
        let finished_before = session.game.is_finished();
        let (pid, _) = session.step();
        // The socket stepper's own accounting: a seat owes its share of the
        // whole-turn budget, only the living divide it, and a simultaneous
        // step is the whole round so it spends the whole budget at once.
        budget = budget.saturating_add(
            if session.game.turn_structure == TurnStructure::Simultaneous {
                pace
            } else {
                let living: Vec<_> = session.game.players.iter().filter(|p| p.alive).collect();
                let minors = living
                    .iter()
                    .filter(|p| p.is_minor || p.is_barbarian)
                    .count();
                let majors = living.len() - minors;
                let p = &session.game.players[pid];
                seat_delay_ms(pace, majors, minors, p.is_minor || p.is_barbarian)
            },
        );
        if spectator_step_completes_frame(pace, turn_before, finished_before, &session.game) {
            FRAME_SEQUENCE.with(|sequence| sequence.set(sequence.get().wrapping_add(1)));
        }
        if current_frame(session) != held {
            break;
        }
    }
    budget
}

/// The fields [`decorate`](super::decorate) adds from `Shared`'s atomics.
fn decorate_browser(o: &mut Value) {
    o["pace"] = json!(PACE.with(Cell::get));
    o["between_game_countdown_ms"] = json!(BETWEEN_GAME_COUNTDOWN_MS.with(Cell::get));
    o["paused"] = json!(PAUSED.with(Cell::get));
    o["frame_sequence"] = json!(FRAME_SEQUENCE.with(Cell::get));
    // The setup panel reads its own staged choices back out of `/state`, and
    // the queue no longer lives in the session, so it is attached here for the
    // same reason the server's `decorate` attaches it.
    o["next_game_settings"] = NEXT_GAME_PARAMS.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(simulation_settings)
            .unwrap_or(Value::Null)
    });
    // No supervisor exists in a page that is the whole runtime.
    o["supervisor_request"] = Value::Null;
}

/// The Civilization VI integration drives an application on the machine
/// serving the native socket. A published page has no access to that machine
/// (and must never pretend that the browser itself is the host), but it still
/// answers the same capability request so the setup panel can explain the
/// boundary instead of sending the call to the public network.
const CIV6_NATIVE_ONLY_MESSAGE: &str =
    "Civilization VI integration is available only in the native desktop build";

fn browser_civ6_status() -> Value {
    json!({
        "ready": false,
        "install": Value::Null,
        "controller": Value::Null,
        "blocked": CIV6_NATIVE_ONLY_MESSAGE,
        "holder": Value::Null,
        "run": Value::Null,
    })
}

/// Replace a browser state map with the tile delta from the exact frame the
/// page says it holds.
///
/// The browser route is single-viewer, but it still needs the same baseline
/// discipline as the socket server: a reply that was cancelled, a page that
/// changed worlds, or a stale `have=` token must receive a complete map rather
/// than a patch it would apply to the wrong terrain. The viewer's
/// `adoptTiles()` already applies this wire shape.
fn deliver_browser_tiles(frame: SpectatorFrame, have: Option<SpectatorFrame>, state: &mut Value) {
    // Lift the array out only while considering a patch. A full response keeps
    // the original values in place; the cache itself is just one u64 per tile.
    let Some(Value::Object(map)) = state.get_mut("map") else {
        return;
    };
    let Some(Value::Array(tiles)) = map.remove("tiles") else {
        return;
    };
    let marks = tiles.iter().map(tile_mark).collect::<Vec<_>>();
    let changed = BROWSER_TILE_MARKS.with(|cached| {
        cached
            .borrow()
            .as_ref()
            .filter(|(held, prior)| {
                held.seed == frame.seed && Some(*held) == have && prior.len() == marks.len()
            })
            .map(|(_, prior)| {
                tiles
                    .iter()
                    .enumerate()
                    .filter(|(at, _)| prior[*at] != marks[*at])
                    .map(|(at, tile)| json!([at, tile]))
                    .collect::<Vec<_>>()
            })
    });

    if let Some(changed) = changed {
        map.insert("tiles_from".to_string(), json!(have.map(|held| held.turn)));
        map.insert("tiles_changed".to_string(), Value::Array(changed));
    } else {
        map.insert("tiles".to_string(), Value::Array(tiles));
    }
    BROWSER_TILE_MARKS.with(|cached| *cached.borrow_mut() = Some((frame, marks)));
}

/// Answer one request, exactly as the socket handler's `match` arm would.
///
/// `target` is the raw request target, query string included, because several
/// routes read parameters off it.
fn route(method: &str, target: &str, body: &str) -> Value {
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if let Err(error) = crate::protocol::validate_request(&parsed) {
        return crate::protocol::version_response(json!({"error": error}));
    }
    crate::protocol::version_response(route_unversioned(method, target, body))
}

fn route_unversioned(method: &str, target: &str, body: &str) -> Value {
    let parsed: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let path = request_path(target);

    match (method, path) {
        ("GET", "/runtime") => json!({
            "server_instance": process_identity(),
            "seed": with_session(|s| s.game.seed),
            "commit": runtime_commit("unknown"),
            "commit_time": runtime_commit_time(),
            "built_at": runtime_built_at(),
            "next_game_settings": NEXT_GAME_PARAMS.with(|cell| {
                cell.borrow()
                    .as_ref()
                    .map(simulation_settings)
                    .unwrap_or(Value::Null)
            }),
        }),

        // The long poll, though, is the whole clock of a spectated game and
        // has to survive the move into the page. On a socket the watching loop
        // names the frame it holds and the response is *withheld* until a
        // separate stepper thread has played past it; here there is no other
        // thread, so naming the frame it holds is what plays the next one. A
        // page that names nothing — the human game's boot read, a resync — is
        // answered with the world as it stands, exactly as before.
        ("GET", "/state") => with_session(|session| {
            let held = query_value(target, "have").and_then(held_frame);
            let budget = held.map_or(0, |held| advance_one_frame(session, held));
            let frame = current_frame(session);
            let mut o = session.state();
            if query_value(target, "planet") == Some("1") {
                if let Some(geometry) = crate::obs::planet_geometry(&session.game) {
                    o["map"]["planet"] = geometry;
                }
            }
            // The reasoning journal rides the same delta contract as on a
            // socket: a page that names its cursor gets what has been
            // recorded since, and a read that names nothing is not reading
            // the log. Left unmirrored, the strategy panel's decision
            // factors were permanently empty on civvis.ai while working
            // everywhere the feature was developed.
            if let Some(cursor) = query_value(target, "think") {
                o["ai_reasoning"] = session.reasoning_json(cursor.parse::<u64>().unwrap_or(0));
            }
            decorate_browser(&mut o);
            // What the frame in this answer costs on the wall clock; the shim
            // spends it, because a wasm module cannot sleep.
            o["frame_budget_ms"] = json!(budget);
            deliver_browser_tiles(frame, held, &mut o);
            o
        }),

        // The countdown that follows a result is wall-clock, and a wasm module
        // has no clock of its own worth trusting; the shim runs it and calls
        // here when it reaches zero.
        ("POST", "/next-game") => with_session(|session| {
            let queued = NEXT_GAME_PARAMS.with(|cell| cell.borrow_mut().take());
            start_automatic_browser_next_game(session, queued);
            let mut o = session.state();
            o["error"] = Value::Null;
            decorate_browser(&mut o);
            o
        }),

        // A published browser build has no permission to inspect the machine
        // that hosts it. Keep the response shape, but say that plainly rather
        // than substituting the tab's own incomplete measurements.
        ("GET", "/machine-metrics") => machine_metrics_json(),

        // Civilization VI is a native desktop integration. The browser still
        // answers its capability and start requests so the viewer's hidden
        // mode cannot fall through to civvis.ai (or report a misleading
        // generic network failure) when a stored setup selects it.
        ("GET", "/civ6") => browser_civ6_status(),
        ("POST", "/civ6/start") => {
            json!({"error": CIV6_NATIVE_ONLY_MESSAGE, "started": Value::Null})
        }

        ("GET", "/status") => with_session(|session| {
            json!({
                "turn": session.game.turn,
                "seed": session.game.seed,
                "winner": session.game.winner,
                "finished": session.game.is_finished(),
                "draw": session.game.is_draw(),
                "paused": PAUSED.with(Cell::get),
                "server_instance": process_identity(),
                "commit": runtime_commit("unknown"),
                "commit_time": runtime_commit_time(),
                "built_at": runtime_built_at(),
            })
        }),

        ("GET", "/rules") => with_session(|session| crate::routes::rules(session, false)),

        // The browser build has no operator force file to read, so nothing is
        // ever armed here; the shared handler still answers with the ledger's
        // own program.
        ("GET", "/gene-program") => with_session(|_session| crate::routes::gene_program(&[])),

        ("GET", "/pedia") => with_session(|session| crate::routes::pedia(session)),

        // A save is handed straight to the page, which owns the storage this
        // build has: there is no disk here, so `/saves` and the named half of
        // `/save` live in the shim's `localStorage` and only ever reach the
        // engine as an uploaded game on `/load`.
        ("GET", "/save") => with_session(|session| crate::routes::save(session)),

        ("POST", "/pace") => {
            if let Some(ms) = parsed["ms"].as_u64() {
                PACE.with(|pace| pace.set(ms));
            }
            if let Some(value) = parsed["between_game_countdown_ms"]
                .as_u64()
                .filter(|value| valid_between_game_countdown_ms(*value).is_some())
            {
                BETWEEN_GAME_COUNTDOWN_MS.with(|countdown| countdown.set(value));
            }
            if let Some(paused) = parsed["paused"].as_bool() {
                PAUSED.with(|held| held.set(paused));
            }
            with_session(|session| {
                let mut out = crate::routes::pace(session, &parsed);
                decorate_browser(&mut out);
                out
            })
        }

        ("POST", "/action") => with_session(|session| {
            let spectating = session.params.spectate;
            let turn = session.game.turn;
            let outcome = crate::routes::action(session, &parsed);
            let autosave = outcome.autosave_due(spectating);
            let mut out = outcome.out;
            // The native build autosaves to disk at the top of every turn. The
            // page is told to do the same into its own storage; it holds the
            // only copy this build has.
            if autosave {
                out["autosave_due"] = json!(turn);
            }
            decorate_browser(&mut out);
            out
        }),

        ("POST", "/step") => with_session(|session| {
            let outcome = crate::routes::step(session, &parsed);
            if outcome.advanced {
                FRAME_SEQUENCE.with(|sequence| sequence.set(sequence.get().wrapping_add(1)));
            }
            let mut out = outcome.out;
            decorate_browser(&mut out);
            out
        }),

        ("POST", "/autoplay") => with_session(|session| {
            let outcome = match crate::routes::autoplay(session, &parsed, None) {
                Ok(outcome) => outcome,
                Err(error) => return error,
            };
            let mut out = outcome.out;
            decorate_browser(&mut out);
            out
        }),

        ("POST", "/play-on") => with_session(|session| {
            let outcome = match crate::routes::play_on(session, &parsed) {
                Ok(outcome) => outcome,
                Err(error) => return error,
            };
            if outcome.played_on {
                if let Some(paused) = outcome.requested_pause {
                    PAUSED.with(|held| held.set(paused));
                }
            }
            let mut out = outcome.out;
            decorate_browser(&mut out);
            out
        }),

        ("POST", "/route") => with_session(|session| crate::routes::route_step(session, &parsed)),

        // The same one-unit question the native server answers, and it has to
        // be answered here too: this router is the whole server on the
        // published build, and a route missing from it is a feature that works
        // everywhere except the site people actually watch.
        ("POST", "/intel") => with_session(|session| crate::routes::intel(session, &parsed)),

        ("POST", "/view") => with_session(|session| {
            let mut out = crate::routes::view(session, &parsed);
            decorate_browser(&mut out);
            out
        }),

        ("POST", "/spectator-status") => with_session(|session| {
            let out = crate::routes::spectator_status(session, &parsed);
            if out["ok"] == json!(true) {
                if let Some(paused) = parsed["paused"].as_bool() {
                    PAUSED.with(|held| held.set(paused));
                }
            }
            out
        }),

        ("POST", "/next-game-settings") => with_session(|session| {
            let (staged, settings) = crate::routes::next_game_settings(&session.params, &parsed);
            NEXT_GAME_PARAMS.with(|cell| *cell.borrow_mut() = Some(staged));
            json!({"ok": true, "next_game_settings": settings})
        }),

        // A page in this build is the whole runtime, so there is no successor
        // process to hand a new world to: the supervised route and the plain
        // one are the same act.
        ("POST", "/new") | ("POST", "/supervisor-new") => with_session(|session| {
            let result = crate::routes::new_game(session, &parsed);
            if result.is_ok() {
                reseat_browser_session_from_host(session);
                let paused = parsed["paused"]
                    .as_bool()
                    .unwrap_or_else(|| PAUSED.with(Cell::get));
                PAUSED.with(|held| held.set(paused));
                session.set_spectator_paused(paused);
            }
            let mut o = session.state();
            o["error"] = match result {
                Ok(()) => Value::Null,
                Err(error) => Value::String(error),
            };
            decorate_browser(&mut o);
            o
        }),

        // Only the uploaded-game half of the native route: the shim resolves a
        // named save out of `localStorage` and posts the game itself.
        ("POST", "/load") => with_session(|session| {
            let result = crate::routes::load_uploaded(session, &parsed);
            let mut out = session.state();
            out["error"] = match result {
                Ok(()) => Value::Null,
                Err(error) => json!(error),
            };
            decorate_browser(&mut out);
            out
        }),

        _ => json!({"error": format!("{method} {path} is not served by the browser build")}),
    }
}

// ---------------------------------------------------------------------------
// The module's ABI.
//
// Strings cross in both directions as UTF-8 in linear memory. The caller
// allocates its request with `civvis_alloc`, hands over the pointer and byte
// length, and receives a pointer to a little-endian `u32` length followed by
// that many bytes of response — one allocation to read and one to free, with
// no bindings generator and so no third-party dependency in the crate.
// ---------------------------------------------------------------------------

/// Hand a buffer to the caller and forget it here.
///
/// Every pointer that crosses the boundary comes from a boxed slice, whose
/// allocation is exactly its length: a `Vec` may reserve more than it was
/// asked for, and freeing one through a length that is not its capacity is
/// undefined behaviour.
fn leak(bytes: Vec<u8>) -> *mut u8 {
    Box::into_raw(bytes.into_boxed_slice()).cast::<u8>()
}

/// Hand back `bytes` with its length in front, which is the shape every answer
/// crosses in: the caller reads a little-endian `u32`, then that many bytes,
/// then frees `4 + len`.
fn sized(bytes: Vec<u8>) -> *mut u8 {
    let mut out = Vec::with_capacity(4 + bytes.len());
    out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(&bytes);
    leak(out)
}

/// Reserve `len` zeroed bytes for the caller to write a request into.
#[no_mangle]
pub extern "C" fn civvis_alloc(len: usize) -> *mut u8 {
    leak(vec![0u8; len])
}

/// The message from the last panic, length-prefixed, or an empty answer if the
/// module has not died. Readable after the trap that produced it.
#[no_mangle]
pub extern "C" fn civvis_last_panic() -> *mut u8 {
    sized(LAST_PANIC.with(|held| held.borrow().clone()).into_bytes())
}

/// Record panics rather than losing them to an anonymous trap.
fn install_panic_hook() {
    if HOOKED.with(Cell::get) {
        return;
    }
    HOOKED.with(|hooked| hooked.set(true));
    std::panic::set_hook(Box::new(|info| {
        // `to_string` carries both the payload and the source location, which
        // together are usually the whole diagnosis.
        LAST_PANIC.with(|held| *held.borrow_mut() = info.to_string());
    }));
}

/// Release a buffer obtained from [`civvis_alloc`] or [`civvis_request`].
///
/// # Safety
/// `ptr` must be a pointer this module returned, with the exact `len` it was
/// created with, and must not be used afterwards.
#[no_mangle]
pub unsafe extern "C" fn civvis_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
    }
}

/// Answer one request.
///
/// The request is UTF-8 JSON — `{"method": "...", "path": "...", "body": "..."}`
/// — and the answer is a length-prefixed UTF-8 JSON buffer the caller frees.
///
/// # Safety
/// `ptr` must point to `len` initialised bytes from [`civvis_alloc`]; this
/// call takes ownership of them.
#[no_mangle]
pub unsafe extern "C" fn civvis_request(ptr: *mut u8, len: usize) -> *mut u8 {
    install_panic_hook();
    let held = Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len));
    let request = String::from_utf8(held.into_vec()).unwrap_or_default();
    let parsed: Value = serde_json::from_str(&request).unwrap_or(Value::Null);
    let method = parsed["method"].as_str().unwrap_or("GET").to_string();
    let target = parsed["path"].as_str().unwrap_or("/").to_string();
    let body = parsed["body"].as_str().unwrap_or("").to_string();
    // Only the first request to need a world reads this; every later one is
    // answered by the session that request created.
    if let Some(seed) = parsed["seed"].as_u64().filter(|seed| *seed != 0) {
        OPENING_SEED.with(|held| held.set(seed));
    }

    let answer = serde_json::to_string(&route(&method, &target, &body))
        .unwrap_or_else(|error| format!("{{\"error\":\"cannot serialise the answer: {error}\"}}"));
    sized(answer.into_bytes())
}

// The opening world's own contract — that it is the Tiny Lakes globe, sized
// by the shipped table — is tested natively on `stock_opening_params`
// in the parent module, where the suite actually runs; a `#[cfg(test)]`
// module here would only ever be compiled for a target whose tests nobody
// executes.
