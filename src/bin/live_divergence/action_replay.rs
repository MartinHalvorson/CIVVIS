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
        "phase": end.get("phase").and_then(Value::as_str).unwrap_or("request_boundary"), "order": begin["order"],
        "same_turn": before.turn == after.turn && begin["turn"].as_u64() == Some(u64::from(before.turn))
            && end["turn"] == begin["turn"] && before.frame == after.frame
            && begin["frame"].as_u64() == Some(u64::from(before.frame)) && end["frame"] == begin["frame"],
        "intervening_actions": 0, "predictions": {}, "observed": {}});
    if case["phase"] == "settled"
        && (begin["isolated"] != true || end["isolated"] != true || end["settled"] != true)
    {
        case["coverage_gap"] = json!("probe did not establish isolated completion");
        return case;
    }
    if case["same_turn"] != true {
        case["coverage_gap"] = json!("turn or frame changed inside transition");
        return case;
    }
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
        Action::FoundCity { unit } => {
            let host_id = begin["order"]["subject"].as_i64();
            let original = before.units.iter().find(|u| Some(u.id) == host_id);
            let founded_at = original.map(|u| (u.x, u.y));
            let native_city = founded_at.is_some_and(|pos| {
                mirror.game.cities.values().any(|city| {
                    city.owner == 0 && civvis::hex::axial_to_offset(city.pos.0, city.pos.1) == pos
                })
            });
            let host_city = founded_at
                .is_some_and(|pos| after.cities.iter().any(|city| (city.x, city.y) == pos));
            (
                json!({"cities": mirror.game.player_city_ids(0).len(), "settler_present": mirror.game.units.contains_key(&unit), "city_at_settler": native_city}),
                json!({"cities": after.cities.len(), "settler_present": after.units.iter().any(|u| Some(u.id) == host_id), "city_at_settler": host_city}),
            )
        }
        Action::MoveTo { unit, .. } | Action::Fortify { unit } => {
            let native = &mirror.game.units[&unit];
            let host = after
                .units
                .iter()
                .find(|u| Some(u.id) == begin["order"]["subject"].as_i64());
            let pos = civvis::hex::axial_to_offset(native.pos.0, native.pos.1);
            let (mut predicted, mut observed) = (
                json!({"position": [pos.0, pos.1], "moves": native.moves_left}),
                host.map_or(
                    json!({}),
                    |u| json!({"position": [u.x, u.y], "moves": u.moves}),
                ),
            );
            if matches!(action, Action::Fortify { .. }) {
                predicted["fortified"] = json!(native.fortified);
                if let Some(host) = host {
                    observed["fortified"] = json!(host.fortified);
                }
            }
            (predicted, observed)
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
    replay(
        std::io::BufReader::new(std::fs::File::open(file)?),
        |case| println!("{case}"),
    )
}

fn replay(
    reader: impl BufRead,
    mut emit: impl FnMut(Value),
) -> Result<(), Box<dyn std::error::Error>> {
    let mut snapshot = Snapshot::default();
    let mut seat: Option<Seat> = None;
    let mut pending: Option<(
        Value,
        Option<StateSnapshot>,
        Option<StateSnapshot>,
        Snapshot,
    )> = None;
    let mut last_sequence = 0;
    let mut map_turn = None;
    for line in reader.lines() {
        let line = line?;
        let event: Value = serde_json::from_str(&line)?;
        match event["kind"].as_str().unwrap_or("") {
            "seat" => {
                if pending.is_some() {
                    return Err("seat changed inside transition".into());
                }
                // This replay reconstructs Gathering Storm without optional
                // modes. Do not silently default missing/unsupported setup.
                if event["ruleset"] != "RULESET_EXPANSION_2" || event["modes"] != json!([]) {
                    return Err(
                        "action replay requires an explicit Gathering Storm/no-modes seat".into(),
                    );
                }
                let parsed: Seat = serde_json::from_value(event)?;
                let speed = parsed
                    .speed
                    .trim()
                    .trim_start_matches("GAMESPEED_")
                    .to_lowercase();
                let difficulty = parsed
                    .difficulty
                    .trim()
                    .trim_start_matches("DIFFICULTY_")
                    .to_lowercase();
                if parsed.players < 2
                    || parsed.local_player < 0
                    || parsed.local_player as usize >= parsed.players
                    || mirror::civvis_civ_name(&parsed.civ).is_none()
                    || civvis::setup::GameSpeed::from_id(&speed).is_none()
                    || !civvis::rules::Rules::embedded()
                        .difficulties
                        .contains_key(difficulty.as_str())
                {
                    return Err(
                        "action replay seat has missing or unsupported identity/rules".into(),
                    );
                }
                seat = Some(parsed);
                snapshot = Snapshot::default();
                map_turn = None;
            }
            "tiles" => {
                let turn = event["turn"].as_u64().ok_or("map export requires turn")?;
                if map_turn.is_some_and(|previous| turn < previous) {
                    return Err("map export moved backwards in time".into());
                }
                map_turn = Some(turn);
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
                let sequence = event["sequence"]
                    .as_u64()
                    .filter(|id| *id > 0)
                    .ok_or("transition requires a positive sequence")?;
                if sequence <= last_sequence {
                    return Err("duplicate or out-of-order transition sequence".into());
                }
                last_sequence = sequence;
                if event["turn"].as_u64().is_none() || event["frame"].as_u64().is_none() {
                    return Err("transition requires turn and frame".into());
                }
                if map_turn.is_some_and(|turn| Some(turn) > event["turn"].as_u64()) {
                    return Err("map export is from after the requested action".into());
                }
                pending = Some((event, None, None, snapshot.clone()));
            }
            "action_transition_before" | "action_transition_after" => {
                if event["turn"].as_u64().is_none() || event["frame"].as_u64().is_none() {
                    return Err("transition observation requires turn and frame".into());
                }
                let (_, before, after, _) =
                    pending.as_mut().ok_or("observation outside transition")?;
                if event["kind"] == "action_transition_after" && before.is_none() {
                    return Err("after observation precedes before observation".into());
                }
                if after.is_some() {
                    return Err("observation after completed pair".into());
                }
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
                let (begin, before, after, preceding_map) =
                    pending.take().ok_or("end without begin")?;
                if begin["sequence"] != event["sequence"] {
                    return Err("transition sequence mismatch".into());
                }
                let case = match (before, after) {
                    (Some(before), Some(after))
                        if event["threw"] == false
                            && event["before_export"] == true
                            && event["after_export"] == true
                            && seat.is_some() =>
                    {
                        compare(&begin, &before, &after, &event, &preceding_map)
                    }
                    _ => {
                        json!({"id": begin["sequence"], "coverage_gap": "missing seat/observation or exception"})
                    }
                };
                emit(case);
            }
            "state" | "orders" | "turn" if pending.is_some() => {
                return Err("ordinary planner/turn event inside isolated transition".into())
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
