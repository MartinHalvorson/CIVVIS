//! The world is the whole game (`docs/CORE_STATE.md`).
//!
//! A save holds the world the rules play on, and beside it things the rules
//! never read back: history, each seat's last-seen memory, values rebuilt
//! every turn, and the mirror of a live Civilization VI game.
//! `docs/core_state_classes.json` names the layer of every saved field, as
//! measured by the core-state probe (#5749). These tests hold that line: a
//! save with everything outside the world blanked must replay the same turns
//! to the same world, with the same actions legal along the way, and a field
//! the manifest does not know fails until someone says where it belongs.

use super::*;
use crate::ai::{AdvancedAi, Ai};
use crate::setup::{MapScript, MapSize};
use serde_json::{Map, Value};

const MANIFEST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/core_state_classes.json"
));

/// The layers that are not the world.
const OUTSIDE: [&str; 4] = ["history", "last-seen memory", "derived", "mirror"];

fn manifest() -> (BTreeMap<String, String>, BTreeSet<String>) {
    let doc: Value = serde_json::from_str(MANIFEST).expect("the manifest parses");
    let layers = doc["layer"]
        .as_object()
        .expect("a layer per field")
        .iter()
        .map(|(path, layer)| (path.clone(), layer.as_str().unwrap().to_string()))
        .collect();
    let zero = doc["mode"]
        .as_object()
        .map(|modes| {
            modes
                .iter()
                .filter(|(_, mode)| mode.as_str() == Some("zero"))
                .map(|(path, _)| path.clone())
                .collect()
        })
        .unwrap_or_default();
    (layers, zero)
}

fn records(value: &Value) -> Option<&Vec<Value>> {
    value
        .as_array()
        .filter(|items| !items.is_empty() && items.iter().all(Value::is_object))
}

/// Every field of a save at the depth the manifest names: `key`,
/// `key[*].field` for a list of records, `key.list[*].field` one level down.
fn fields(save: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (key, value) in save.as_object().expect("a save is an object") {
        if let Some(items) = records(value) {
            for item in items {
                for field in item.as_object().unwrap().keys() {
                    out.insert(format!("{key}[*].{field}"));
                }
            }
        } else if let Some(object) = value.as_object() {
            let mut nested = false;
            for (sub, inner) in object {
                if let Some(items) = records(inner) {
                    nested = true;
                    for item in items {
                        for field in item.as_object().unwrap().keys() {
                            out.insert(format!("{key}.{sub}[*].{field}"));
                        }
                    }
                }
            }
            if !nested {
                out.insert(key.clone());
            }
        } else {
            out.insert(key.clone());
        }
    }
    out
}

fn empty_like(value: &Value) -> Value {
    match value {
        Value::Array(_) => Value::Array(Vec::new()),
        Value::Object(_) => Value::Object(Map::new()),
        Value::Number(_) => Value::from(0),
        Value::Bool(_) => Value::Bool(false),
        Value::String(_) => Value::String(String::new()),
        Value::Null => Value::Null,
    }
}

fn zero_like(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(zero_like).collect()),
        Value::Object(object) => Value::Object(
            object
                .iter()
                .map(|(key, inner)| (key.clone(), zero_like(inner)))
                .collect(),
        ),
        Value::Number(_) => Value::from(0),
        Value::Bool(_) => Value::Bool(false),
        other => other.clone(),
    }
}

fn blank(save: &mut Value, path: &str, zero: bool) {
    let blank: fn(&Value) -> Value = if zero { zero_like } else { empty_like };
    let set = |slot: &mut Value| *slot = blank(slot);
    match path.split_once("[*].") {
        None => {
            if let Some(slot) = save.get_mut(path) {
                set(slot);
            }
        }
        Some((outer, leaf)) => {
            let list = match outer.split_once('.') {
                Some((key, sub)) => save.get_mut(key).and_then(|object| object.get_mut(sub)),
                None => save.get_mut(outer),
            };
            if let Some(Value::Array(items)) = list {
                for item in items {
                    if let Some(slot) = item.get_mut(leaf) {
                        set(slot);
                    }
                }
            }
        }
    }
}

/// A save with every field outside the world blanked. A list whose every
/// field is outside the world (the event log, turn stats, the war chronicle)
/// goes whole: its length is history too, and the engine merges a repeated
/// note into the last one, so blanked texts would merge differently.
fn world_only(
    mut save: Value,
    layers: &BTreeMap<String, String>,
    zero: &BTreeSet<String>,
) -> Value {
    let mut whole: BTreeMap<&str, bool> = BTreeMap::new();
    for (path, layer) in layers {
        if let Some((top, _)) = path.split_once("[*].") {
            if !top.contains('.') {
                let outside = OUTSIDE.contains(&layer.as_str());
                *whole.entry(top).or_insert(true) &= outside;
            }
        }
    }
    for (top, outside) in whole {
        if outside {
            blank(&mut save, top, false);
        }
    }
    for (path, layer) in layers {
        if OUTSIDE.contains(&layer.as_str()) {
            blank(&mut save, path, zero.contains(path));
        }
    }
    save
}

struct Recorded {
    save: Value,
    actions: Vec<(usize, Action)>,
    played: Value,
    fog: bool,
}

/// A duel on Online speed, saved at turn 40 and played 12 turns further.
fn recorded() -> Recorded {
    let size = MapSize::for_players(2);
    let mut options = GameOptions::new(
        2,
        size.width,
        size.height,
        5_765,
        250,
        size.default_city_states,
    );
    options.speed = "online".into();
    options.map_script = MapScript::Continents;
    let mut g = Game::new_with(options);
    let mut ais = AdvancedAi::fleet(&g);
    let fog = ais.iter().any(Ai::uses_player_observation);
    g.set_fog_memory(fog);
    g.set_war_ledger(false);
    let mut play_to = |g: &mut Game, turn: u32| {
        while g.winner.is_none() && !(g.turn >= turn && g.current == 0) {
            let pid = g.current;
            ais[pid].take_turn(g, pid);
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
    };
    play_to(&mut g, 40);
    let save = as_saved(&g);
    let from = g.log.len();
    play_to(&mut g, 52);
    Recorded {
        save,
        actions: g.log.since(from).cloned().collect(),
        played: as_saved(&g),
        fog,
    }
}

/// Through text, as a save is read back.
fn as_saved(g: &Game) -> Value {
    serde_json::from_str(&serde_json::to_string(g).unwrap()).unwrap()
}

/// Replays `actions` with no agent in the loop; returns the end state and
/// every seat's legal actions at the start of its first turn in the window.
fn replay(save: Value, actions: &[(usize, Action)], fog: bool) -> (Value, Vec<Vec<String>>) {
    let mut g: Game = serde_json::from_value(save).expect("the save loads");
    g.set_fog_memory(fog);
    g.set_war_ledger(false);
    let start = g.turn;
    let mut seen = BTreeSet::new();
    let mut legal = Vec::new();
    for (pid, action) in actions {
        if g.turn == start && g.winner.is_none() && seen.insert(g.current) {
            let mut set: Vec<String> = g
                .legal_actions(g.current)
                .iter()
                .map(|action| serde_json::to_string(action).unwrap())
                .collect();
            set.sort();
            legal.push(set);
        }
        g.apply(*pid, action)
            .unwrap_or_else(|why| panic!("a recorded action is refused: {why}"));
    }
    (as_saved(&g), legal)
}

/// The seats' counters without the agents' notes: the rules' tallies only.
fn drop_agent_notes(save: &mut Value) {
    if let Some(Value::Array(players)) = save.get_mut("players") {
        for player in players {
            if let Some(Value::Object(counters)) = player.get_mut("counters") {
                counters.retain(|key, _| !is_agent_note(key));
            }
        }
    }
}

fn differing_keys(a: &Value, b: &Value) -> Vec<String> {
    let (a, b) = (a.as_object().unwrap(), b.as_object().unwrap());
    a.keys()
        .chain(b.keys())
        .filter(|key| a.get(*key) != b.get(*key))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[test]
fn a_save_stripped_to_the_world_replays_the_same_game() {
    let (layers, zero) = manifest();
    let game = recorded();
    assert!(!game.actions.is_empty(), "the window plays something");
    let (full_end, full_legal) = replay(game.save.clone(), &game.actions, game.fog);
    let (world_end, world_legal) = replay(
        world_only(game.save.clone(), &layers, &zero),
        &game.actions,
        game.fog,
    );
    assert_eq!(
        full_legal, world_legal,
        "a field outside the world changed which actions were legal"
    );
    let (full_world, world_world) = (
        world_only(full_end.clone(), &layers, &zero),
        world_only(world_end, &layers, &zero),
    );
    assert!(
        full_world == world_world,
        "a field outside the world changed the world: {:?}",
        differing_keys(&full_world, &world_world)
    );

    // The save is enough on its own, too: replaying the window reproduces the
    // played world. The stock agents write their own notes into the seats'
    // counters from inside their turns, outside any action; `Counters` keeps
    // those apart (`is_agent_note`), and every tally the rules keep must
    // replay exactly. A note namespace missing from `is_agent_note` fails
    // here, naming the counters.
    let mut played = world_only(game.played, &layers, &zero);
    let mut replayed = full_world;
    drop_agent_notes(&mut played);
    drop_agent_notes(&mut replayed);
    assert!(
        played == replayed,
        "something other than an action wrote the world: {:?}",
        differing_keys(&played, &replayed)
    );
}

#[test]
fn every_saved_field_has_a_layer() {
    let (layers, _) = manifest();
    let game = recorded();
    // A list of records that happens to be empty shows up as its bare key;
    // the manifest names its fields instead.
    let known = |path: &String| {
        layers.contains_key(path)
            || layers.keys().any(|known| {
                known.starts_with(&format!("{path}[*]")) || known.starts_with(&format!("{path}."))
            })
    };
    let unknown: Vec<String> = fields(&game.save)
        .into_iter()
        .chain(fields(&game.played))
        .filter(|path| !known(path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    assert!(
        unknown.is_empty(),
        "saved fields with no layer in docs/core_state_classes.json: {unknown:?}. \
         Add each under \"layer\": \"core\" if the rules read it, otherwise \
         \"history\", \"last-seen memory\", \"derived\", \"mirror\", \"setup\" or \
         \"map geometry\" (docs/CORE_STATE.md explains each); \
         `civvis core-probe` measures it"
    );
}
