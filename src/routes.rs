//! The route handlers both front ends serve, written once.
//!
//! Distinct from `protocol.rs`, which owns the versioning envelope that
//! wraps these answers. This module is the answers themselves.
//!
//! # Why this module exists
//!
//! `server.rs` (native) and `wasm.rs` (civvis.ai) each hand-implemented the
//! same protocol. The shared module now owns the game-side behavior for the
//! common read and control routes, while each dispatcher retains only its
//! transport concerns. Two copies of a rule is two chances to change one of
//! them, and the intentional boundary is explicit: `/host-league` and
//! `/next-game` exist only in wasm, `/adjacency` and `/saves` only in native.
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

use crate::game::CIV6_LEADER_POOL;
use crate::server::{self, Params, Session};
use crate::setup::{
    battlefield_map_scripts, battlefield_sizes, scenario_map_scripts, world_map_scripts,
    BASE_RULESETS, CIV6_GAME_SPEEDS, CIV6_MAP_SIZES, FUTURE_ERAS, MAP_POLES, MAP_TOPOLOGIES,
    START_ERAS,
};

const COUNTDOWN_ERROR: &str =
    "between-game countdown must be one of 0, 3000, 5000, or 10000 milliseconds";

/// `GET /rules` — the rules and setup vocabulary for the live game.
///
/// Native adds the Civilization VI desktop vocabulary after this shared
/// response is built. Everything else is the same catalogue in both builds;
/// keeping it here prevents a new rules field from silently reaching only one
/// frontend.
pub fn rules(session: &Session, include_civ6: bool) -> Value {
    let r = &session.game.rules;
    let mut out = json!({
        "techs": r.techs, "civics": r.civics,
        "terrains": r.terrains, "features": r.features,
        "resources": r.resources, "improvements": r.improvements,
        "governments": r.governments, "units": r.units,
        "promotions": r.promotions,
        "buildings": r.buildings, "districts": r.districts,
        "wonders": r.wonders,
        "projects": r.projects,
        "policies": r.policies, "beliefs": r.beliefs, "civs": r.civs,
        "city_state_limit": r.city_states.roster.len(),
        "civ6_leaders": CIV6_LEADER_POOL.as_slice(),
        "leader_pools": crate::leader_roster::browser_pools(),
        "great_people": r.great_people, "governors": r.governors,
        "map_sizes": CIV6_MAP_SIZES,
        "difficulties": r.difficulties, "speeds": r.speeds,
        "base_rulesets": BASE_RULESETS,
        "start_eras": START_ERAS,
        "future_eras": FUTURE_ERAS,
        "map_scripts": world_map_scripts(),
        "battlefield_scripts": battlefield_map_scripts(),
        "battlefield_sizes": battlefield_sizes(),
        "historical_scenarios": crate::historical_scenarios::all(),
        "scenario_scripts": scenario_map_scripts(),
        "map_topologies": MAP_TOPOLOGIES,
        "map_poles": MAP_POLES,
        "game_speeds": CIV6_GAME_SPEEDS,
        "default_setup": server::default_setup_json(),
        "strategies": server::strategy_roster(session),
        "leader_elo_options": server::leader_elo_options(session),
        "seat_strategy": session.seated_strategy_name(0),
    });
    if include_civ6 {
        out["civ6"] = crate::civ6::vocabulary();
    }
    out
}

/// `GET /pedia` — generated reference material for the ruleset in play.
pub fn pedia(session: &Session) -> Value {
    json!({"entries": crate::pedia::entries(&session.game.rules)})
}

/// `POST /intel` — the one-unit planning read shared by both clients.
pub fn intel(session: &Session, parsed: &Value) -> Value {
    match parsed["unit"].as_u64() {
        Some(unit) => crate::obs::unit_intel(&session.game, session.viewing_seat(), unit as u32),
        None => json!({"error": "intel needs a unit"}),
    }
}

/// `GET /save` — the in-memory game save envelope.
pub fn save(session: &Session) -> Value {
    crate::protocol::save_value(&session.game).unwrap_or(Value::Null)
}

/// `POST /pace` — apply the game-side portion of a pace update.
///
/// Atomics and browser cells remain in the callers because they are transport
/// state. The live fog and pause flags, response shape, and validation belong
/// to the protocol and therefore happen once here.
pub fn pace(session: &mut Session, parsed: &Value) -> Value {
    if let Some(paused) = parsed["paused"].as_bool() {
        session.set_spectator_paused(paused);
    }
    if let Some(on) = parsed["tactics_fog"].as_bool() {
        session.game.tactics.fog = on;
        session.params.tactics.fog = on;
    }
    let mut out = session.state();
    let invalid_countdown = parsed["between_game_countdown_ms"]
        .as_u64()
        .filter(|value| server::valid_between_game_countdown_ms(*value).is_none());
    if invalid_countdown.is_some() {
        out["error"] = json!(COUNTDOWN_ERROR);
    }
    out
}

/// The shared answer from `POST /step`. Native uses `advanced` to publish a
/// spectator frame after the lock is released; the browser uses the answer
/// directly and advances its frame sequence in its own single-threaded loop.
pub struct StepOutcome {
    pub out: Value,
    pub advanced: bool,
}

/// `POST /step` — run one or more spectator seats and shape their visible log.
pub fn step(session: &mut Session, parsed: &Value) -> StepOutcome {
    if !session.params.spectate {
        let mut out = session.state();
        out["error"] = json!("not in spectate mode");
        return StepOutcome {
            out,
            advanced: false,
        };
    }
    let count = parsed["count"].as_u64().unwrap_or(1) as usize;
    let (out, advanced) = session.spectator_step_response(count);
    StepOutcome { out, advanced }
}

/// The result of shared auto-play. `replayed` means an idempotent retry was
/// answered without simulating another batch, so native must not count it as
/// turns that went past a visible frame.
pub struct AutoplayOutcome {
    pub out: Value,
    pub played: usize,
    pub replayed: bool,
}

/// `POST /autoplay` — validate the world, seat the requested strategy, and run
/// the human seat. `server_instance` is `None` for wasm, which has no process
/// handoff identity; native supplies its process id for stale-page protection.
pub fn autoplay(
    session: &mut Session,
    parsed: &Value,
    server_instance: Option<u32>,
) -> Result<AutoplayOutcome, Value> {
    if session.params.spectate {
        return Err(json!({"error": "a spectated game is already playing itself"}));
    }
    if parsed["seed"]
        .as_u64()
        .is_some_and(|seed| seed != session.game.seed)
        || server_instance.is_some_and(|instance| {
            parsed["server_instance"]
                .as_u64()
                .is_some_and(|provided| provided != instance as u64)
        })
    {
        return Err(json!({"error": "the game changed before auto-play began"}));
    }
    let request_id = parsed["request_id"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 128);
    if let Some(request_id) = request_id {
        if let Some(played) = session.completed_autoplay(request_id) {
            let mut out = session.state();
            out["autoplayed"] = json!(played);
            out["autoplay_strategy"] = json!(session.seated_strategy_name(0));
            return Ok(AutoplayOutcome {
                out,
                played,
                replayed: true,
            });
        }
    }
    if let Some(name) = parsed["strategy"].as_str() {
        if let Err(error) = session.seat_strategy_at(0, name) {
            return Err(json!({"error": error}));
        }
    }
    let turns = match parsed["turns"].as_str() {
        Some("all") => u32::MAX,
        _ => parsed["turns"]
            .as_u64()
            .unwrap_or(1)
            .clamp(1, u32::MAX as u64) as u32,
    };
    let played = session.autoplay(turns);
    if let Some(request_id) = request_id {
        session.remember_autoplay(request_id.to_string(), played);
    }
    let mut out = session.state();
    out["autoplayed"] = json!(played);
    out["autoplay_strategy"] = json!(session.seated_strategy_name(0));
    Ok(AutoplayOutcome {
        out,
        played,
        replayed: false,
    })
}

/// The shared result of `POST /play-on`; callers apply their own pause/frame
/// gates after the simulation lock is released.
pub struct PlayOnOutcome {
    pub out: Value,
    pub played_on: bool,
    pub requested_pause: Option<bool>,
}

/// `POST /play-on` — continue a decided game using the engine's mode parser.
pub fn play_on(session: &mut Session, parsed: &Value) -> Result<PlayOnOutcome, Value> {
    let mode_name = parsed["mode"].as_str().unwrap_or("until_next_victory");
    let Some(mode) = crate::game::PlayOnMode::parse(mode_name) else {
        return Err(json!({"error": format!("unknown play-on mode {mode_name:?}")}));
    };
    let played_on = session.play_on(mode);
    let requested_pause = parsed.get("paused").and_then(Value::as_bool);
    if played_on {
        if let Some(paused) = requested_pause {
            session.set_spectator_paused(paused);
        }
    }
    let mut out = session.state();
    out["error"] = if played_on {
        Value::Null
    } else {
        json!("this game has no result to play on past")
    };
    Ok(PlayOnOutcome {
        out,
        played_on,
        requested_pause,
    })
}

/// `POST /spectator-status` — report/adjust the spectator pause state.
pub fn spectator_status(session: &mut Session, parsed: &Value) -> Value {
    if !session.params.spectate {
        return json!({"error": "not in spectate mode"});
    }
    if let Some(paused) = parsed["paused"].as_bool() {
        session.set_spectator_paused(paused);
    }
    json!({"ok": true})
}

/// Normalize setup changes once; each frontend stores the resulting params in
/// its own queue because the queue has different synchronization semantics.
pub fn next_game_settings(base: &Params, parsed: &Value) -> (Params, Value) {
    let params = server::staged_next_game_params(base, parsed);
    let settings = server::simulation_settings(&params);
    (params, settings)
}

/// `POST /new`'s simulation-side operation. The response decoration and
/// browser reseating/native supervisor bookkeeping stay in the callers.
pub fn new_game(session: &mut Session, parsed: &Value) -> Result<(), String> {
    session.start_new_game(parsed)
}

/// Load an uploaded save (as opposed to native's named-on-disk save). Parsing,
/// mod safety, and replacing the live session are identical in both builds.
pub fn load_uploaded(session: &mut Session, parsed: &Value) -> Result<(), String> {
    let loaded: Result<crate::game::Game, String> = if !parsed["game"].is_null() {
        crate::protocol::game_from_save(parsed["game"].clone())
    } else if parsed.get("save_format_version").is_some() {
        crate::protocol::game_from_save(parsed.clone())
    } else {
        Err("load needs a game".to_string())
    };
    let game = loaded?;
    let active = crate::mods::active_names();
    if game.mods != active {
        return Err(format!(
            "that save was played with mods {:?}, this build has {:?}",
            game.mods, active
        ));
    }
    let params = session.params.clone();
    *session = Session::from_game(params, game);
    Ok(())
}

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
