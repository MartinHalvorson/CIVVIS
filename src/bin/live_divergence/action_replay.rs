//! Replay one recorded host request against its independently exported board.
//! Request-boundary evidence is explicitly distinct from settled execution.
use civvis::game::Action;
use civvis::mirror::{self, LiveMirror, Seat, Snapshot, StateSnapshot, TilesChunk};
use serde_json::{json, Value};
use std::io::BufRead;

fn action(order: &Value, mirror: &LiveMirror) -> Option<Action> {
    let verb = order["verb"].as_str()?;
    let name =
        |prefix| civvis::name::Name::new(&verb.strip_prefix(prefix).unwrap_or(verb).to_lowercase());
    match order["kind"].as_str()? {
        "research" => Some(Action::Research {
            tech: name("TECH_"),
        }),
        "civic" => Some(Action::Civic {
            civic: name("CIVIC_"),
        }),
        "unit" => {
            let unit = *mirror.uid_of.get(&order["subject"].as_i64()?)?;
            match verb {
                "MOVE_TO" => Some(Action::MoveTo {
                    unit,
                    to: civvis::hex::offset_to_axial(
                        i32::try_from(order["x"].as_i64()?).ok()?,
                        i32::try_from(order["y"].as_i64()?).ok()?,
                    ),
                }),
                "FOUND_CITY" => Some(Action::FoundCity { unit }),
                "FORTIFY" => Some(Action::Fortify { unit }),
                _ => None,
            }
        }
        _ => None,
    }
}

fn compare(
    begin: &Value,
    before: &StateSnapshot,
    after: &StateSnapshot,
    end: &Value,
    snapshot: &Snapshot,
) -> Value {
    let mut case = json!({"id": begin["sequence"], "turn": before.turn,
        "phase": "request_boundary", "order": begin["order"],
        "same_turn": before.turn == after.turn && begin["turn"] == end["turn"],
        "intervening_actions": 0, "predictions": {}, "observed": {}});
    if snapshot.width <= 0 || snapshot.height <= 0 {
        case["coverage_gap"] = json!("no preceding map export");
        return case;
    }
    let mut mirror = LiveMirror::new(snapshot, before, 6, 1, 250, 0);
    let Some(action) = action(&begin["order"], &mirror) else {
        case["coverage_gap"] = json!("order type or entity mapping is not covered");
        return case;
    };
    let accepted = mirror.game.apply(0, &action).is_ok();
    // A host acceptance is acknowledgement, not completed execution. Keep it
    // beside the replay rather than counting it as an outcome comparison.
    case["model_accepted"] = json!(accepted);
    case["host_request_accepted"] = end["accepted"].clone();
    if !accepted || end["accepted"] != true {
        case["coverage_gap"] = json!("model or host refused; no effect equivalence established");
        return case;
    }
    let (predicted, observed) = match action {
        Action::Research { .. } => (
            json!({"research": mirror.game.players[0].research}),
            json!({"research": after.research.as_ref().map(|s| s.trim_start_matches("TECH_").to_lowercase())}),
        ),
        Action::Civic { .. } => (
            json!({"civic": mirror.game.players[0].civic}),
            json!({"civic": after.civic.as_ref().map(|s| s.trim_start_matches("CIVIC_").to_lowercase())}),
        ),
        Action::FoundCity { .. } => (
            json!({"cities": mirror.game.player_city_ids(0).len()}),
            json!({"cities": after.cities.len()}),
        ),
        Action::MoveTo { unit, .. } | Action::Fortify { unit } => {
            let native = &mirror.game.units[&unit];
            let host = after
                .units
                .iter()
                .find(|u| Some(u.id) == begin["order"]["subject"].as_i64());
            let pos = civvis::hex::axial_to_offset(native.pos.0, native.pos.1);
            (
                json!({"position": [pos.0, pos.1], "moves": native.moves_left}),
                host.map_or(
                    json!({}),
                    |u| json!({"position": [u.x, u.y], "moves": u.moves}),
                ),
            )
        }
        _ => unreachable!(),
    };
    case["predictions"] = predicted;
    case["observed"] = observed;
    case
}

pub fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let file = args
        .iter()
        .find(|arg| arg.ends_with(".jsonl"))
        .ok_or("expected events.jsonl")?;
    let mut snapshot = Snapshot::default();
    let mut seat: Option<Seat> = None;
    let mut pending: Option<(Value, Option<StateSnapshot>, Option<StateSnapshot>)> = None;
    for line in std::io::BufReader::new(std::fs::File::open(file)?).lines() {
        let line = line?;
        let event: Value = serde_json::from_str(&line)?;
        match event["kind"].as_str().unwrap_or("") {
            "seat" => seat = Some(serde_json::from_value(event)?),
            "tiles" => {
                let chunk: TilesChunk = serde_json::from_value(event.clone())?;
                if event["delta"] == true {
                    snapshot.merge_delta(&chunk);
                } else {
                    snapshot.merge_sweep(&chunk);
                }
            }
            "action_transition_begin" => {
                if pending.is_some() {
                    return Err("nested/incomplete action transition".into());
                }
                pending = Some((event, None, None));
            }
            "action_transition_before" | "action_transition_after" => {
                let (_, before, after) =
                    pending.as_mut().ok_or("observation outside transition")?;
                let mut state = mirror::state_from_json(&line)?;
                if let Some(seat) = &seat {
                    state.seat = seat.clone();
                }
                let slot = if event["kind"] == "action_transition_before" {
                    before
                } else {
                    after
                };
                if slot.replace(state).is_some() {
                    return Err("duplicate transition observation".into());
                }
            }
            "action_transition_end" => {
                let (begin, before, after) = pending.take().ok_or("end without begin")?;
                if begin["sequence"] != event["sequence"] {
                    return Err("transition sequence mismatch".into());
                }
                let case = match (before, after) {
                    (Some(before), Some(after))
                        if event["threw"] == false
                            && event["before_export"] == true
                            && event["after_export"] == true =>
                    {
                        compare(&begin, &before, &after, &event, &snapshot)
                    }
                    _ => {
                        json!({"id": begin["sequence"], "coverage_gap": "missing observation or exception"})
                    }
                };
                println!("{case}");
            }
            _ => (),
        }
    }
    if pending.is_some() {
        return Err("truncated action transition".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "action_replay_tests.rs"]
mod tests;
