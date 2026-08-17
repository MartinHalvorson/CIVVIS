//! The route handlers both front ends serve, written once.
//!
//! Distinct from `protocol.rs`, which owns the versioning envelope that
//! wraps these answers. This module is the answers themselves.
//!
//! # Why this module exists
//!
//! `server.rs` (native) and `wasm.rs` (civvis.ai) each hand-implemented the
//! same protocol. Twenty-two routes are served by both, and 224 of their lines
//! were character-identical — `/action` alone carried a 33-line run that
//! matched exactly, and `/route` matched in full. Two copies of a rule is two
//! chances to change one of them, and the drift is already on the board:
//! `/host-league` and `/next-game` exist only in wasm, `/adjacency` and
//! `/saves` only in native.
//!
//! The roadmap files that as a viewer bug — "panels that read native-only state
//! are silently dead on civvis.ai" — but it is a protocol-duplication bug, and
//! it will keep producing viewer bugs for as long as the shared half is written
//! twice.
//!
//! # What belongs here
//!
//! The part of a route that is a rule about the game: read the request, ask the
//! `Session`, shape the answer. What does NOT belong here is anything about the
//! transport or the deployment — how a session is acquired (a mutex on native,
//! a thread-local in the browser), how a response is delivered (a socket versus
//! a returned `Value`), and the genuinely different things the two builds do
//! with the same outcome. `/action` is the clean example: the whole body is
//! shared, and then native writes an autosave to disk while the page is told to
//! save into its own storage, because the page holds the only copy it has.
//!
//! A handler here takes `&Session` when it only reads and `&mut Session` when
//! it acts, and returns the JSON answer. It never touches a socket, a lock, a
//! file, or a thread-local.

use serde_json::{json, Value};

use crate::server::Session;

/// `POST /route` — the next step of a unit's path toward a destination.
///
/// Read-only, and identical in both front ends down to the last character
/// before this existed.
pub fn route_step(session: &Session, parsed: &Value) -> Value {
    let unit = parsed["unit"].as_u64().map(|unit| unit as u32);
    let to = parsed["to"]
        .as_array()
        .and_then(|pos| Some((pos.first()?.as_i64()? as i32, pos.get(1)?.as_i64()? as i32)));
    match (unit, to) {
        (Some(unit), Some(to)) => {
            let owned = session
                .game
                .units
                .get(&unit)
                .is_some_and(|held| held.owner == 0);
            if !owned {
                json!({"error": "not your unit"})
            } else {
                match session.game.route_step(unit, to, 0) {
                    Some(step) => json!({"step": [step.0, step.1], "error": Value::Null}),
                    None => json!({"step": Value::Null, "error": Value::Null}),
                }
            }
        }
        _ => json!({"error": "route needs a unit and a destination"}),
    }
}

/// `POST /view` — seat the viewer on a player, or on the omniscient spectator.
///
/// Returns the new state with `error` set, which is the shape both front ends
/// already produced. The browser decorates the result afterwards; that is its
/// business, not the protocol's.
pub fn view(session: &mut Session, parsed: &Value) -> Value {
    let result = match parsed.get("player") {
        Some(Value::Null) => session.set_view_player(None),
        Some(value) => value
            .as_u64()
            .ok_or_else(|| "player must be a non-negative integer or null".to_string())
            .and_then(|pid| session.set_view_player(Some(pid as usize))),
        None => Err("missing player".to_string()),
    };
    let mut out = session.state();
    out["error"] = match result {
        Ok(()) => Value::Null,
        Err(error) => Value::String(error),
    };
    out
}

/// What `POST /action` produced, and the two facts its callers act on.
///
/// The autosave decision is deliberately NOT made here. Both builds save at the
/// top of a turn for the same reason — a single-player game that exists only in
/// one process's memory is one crash away from never having happened — but they
/// save to different places, and a handler that knew about either would be a
/// handler that knew about a deployment.
pub struct ActionOutcome {
    /// The state to answer with, carrying `error` and any `movement_paths`.
    pub out: Value,
    /// The request asked to end the turn.
    pub ending_turn: bool,
    /// The action was refused, so nothing changed.
    pub refused: bool,
}

impl ActionOutcome {
    /// Whether this action is the one a build should autosave after.
    ///
    /// A spectated game is the supervisor's business: it is being replayed or
    /// driven from outside, and saving it would write a file per turn that
    /// nobody reads.
    pub fn autosave_due(&self, spectating: bool) -> bool {
        self.ending_turn && !self.refused && !spectating
    }
}

/// `POST /action` — apply one action and answer with the resulting state.
///
/// The movement path is captured BEFORE the action is applied and truncated
/// afterwards to where the unit actually stopped: a move can be cut short by a
/// zone of control or an ambush, and the client draws the path it really took,
/// not the one it asked for.
pub fn action(session: &mut Session, parsed: &Value) -> ActionOutcome {
    let ending_turn = parsed["action"]["type"].as_str() == Some("end_turn");
    let movement_path = serde_json::from_value::<crate::game::Action>(parsed["action"].clone())
        .ok()
        .and_then(|action| match action {
            crate::game::Action::MoveTo { unit, to } => {
                let start = session.game.units.get(&unit)?.pos;
                let mut path = session.game.path_to(unit, to)?;
                path.insert(0, start);
                Some((unit, path))
            }
            _ => None,
        });
    let err = session.act(&parsed["action"]);
    let mut out = session.state();
    if err.is_none() {
        if let Some((unit, mut path)) = movement_path {
            if let Some(actual) = session.game.units.get(&unit).map(|unit| unit.pos) {
                if let Some(end) = path.iter().position(|position| *position == actual) {
                    path.truncate(end + 1);
                } else if let Some(start) = path.first().copied() {
                    path = vec![start, actual];
                }
            }
            if path.len() > 1 {
                out["movement_paths"] = json!({unit.to_string(): path});
            }
        }
    }
    let refused = err.is_some();
    out["error"] = match err {
        Some(e) => Value::String(e),
        None => Value::Null,
    };
    ActionOutcome {
        out,
        ending_turn,
        refused,
    }
}
