//! What the simulator debugger promises about a played game's timeline.

use std::collections::BTreeSet;

use serde_json::{json, Value};

use super::super::{stock_opening_params, Session};
use super::Layer;
use crate::game::Action;
use crate::setup::{GameSpeed, MapScript, MapTopology};

/// A small world with a person at seat zero.
fn played() -> Session {
    let mut params = stock_opening_params(20_261_118);
    params.spectate = false;
    params.num_players = 2;
    params.num_city_states = 1;
    params.width = 20;
    params.height = 14;
    params.map_topology = MapTopology::Flat;
    params.map_script = MapScript::Pangaea;
    params.game_speed = GameSpeed::Standard;
    params.max_turns = 500;
    Session::new(params)
}

fn end_turn(session: &mut Session) {
    assert_eq!(session.act(&json!({"type": "end_turn"})), None);
}

/// The whole world as a save would carry it.
fn world(session: &Session) -> String {
    crate::protocol::save_value(&session.game)
        .expect("a game can be saved")
        .to_string()
}

fn marked(session: &Session) -> Vec<u64> {
    session.state()["debug"]["turns"]
        .as_array()
        .expect("the state names the marked turns")
        .iter()
        .map(|turn| turn.as_u64().unwrap())
        .collect()
}

/// The first thing the seat may do that is not ending its turn.
fn something_to_do(session: &Session) -> Value {
    let action = session
        .game
        .legal_actions(0)
        .into_iter()
        .find(|action| !matches!(action, Action::EndTurn))
        .expect("an opening turn offers a decision");
    serde_json::to_value(action).unwrap()
}

#[test]
fn a_played_game_walks_back_and_forward_along_its_own_turns() {
    let mut session = played();
    assert_eq!(marked(&session), [1]);
    let opening = world(&session);
    end_turn(&mut session);
    let second = world(&session);
    end_turn(&mut session);
    end_turn(&mut session);
    let fourth = world(&session);
    assert_eq!(marked(&session), [1, 2, 3, 4]);

    assert_eq!(session.debug_back(), Ok(3));
    assert_eq!(session.debug_jump(2), Ok(2));
    assert_eq!(world(&session), second);
    // Going back forgets nothing: the turns ahead are still there to return to.
    assert_eq!(marked(&session), [1, 2, 3, 4]);
    assert_eq!(session.debug_forward(), Ok(3));
    assert_eq!(session.debug_forward(), Ok(4));
    assert_eq!(world(&session), fourth);
    assert!(session.debug_forward().is_err());

    assert_eq!(session.debug_jump(1), Ok(1));
    assert_eq!(world(&session), opening);
    assert!(session.debug_back().is_err());
    // Played forward again, the old future is replaced by the new one.
    end_turn(&mut session);
    assert_eq!(marked(&session), [1, 2]);
}

#[test]
fn undo_takes_back_the_last_action_and_nothing_else() {
    let mut session = played();
    let untouched = world(&session);
    let first = something_to_do(&session);
    assert_eq!(session.act(&first), None);
    let after_one = world(&session);
    let second = something_to_do(&session);
    assert_eq!(session.act(&second), None);
    assert_eq!(session.state()["debug"]["acted"], json!(2));

    assert_eq!(session.debug_undo(), Ok(1));
    assert_eq!(world(&session), after_one);
    assert_eq!(session.debug_undo(), Ok(0));
    assert_eq!(world(&session), untouched);
    assert!(session.debug_undo().is_err());

    // Back from a turn the seat has acted in is the top of that turn.
    assert_eq!(session.act(&first), None);
    assert_eq!(session.debug_back(), Ok(1));
    assert_eq!(world(&session), untouched);
}

#[test]
fn a_step_plays_one_turn_and_stops_on_the_turn_it_was_told_to() {
    let mut session = played();
    let refused = session.debug(&json!({"op": "breakpoints", "categories": ["Nonsense"]}));
    assert!(refused["error"].is_string());
    let set = session.debug(&json!({"op": "breakpoints", "turns": [3]}));
    assert_eq!(set["error"], Value::Null);
    assert_eq!(set["debug"]["breakpoints"]["turns"], json!([3]));

    let stepped = session.debug(&json!({"op": "step"}));
    assert_eq!(stepped["turn"], json!(2));
    assert_eq!(stepped["debug"]["stopped"], Value::Null);
    let stopped = session.debug(&json!({"op": "step"}));
    assert_eq!(stopped["turn"], json!(3));
    assert_eq!(
        stopped["debug"]["stopped"],
        json!({"kind": "turn", "turn": 3})
    );
    assert_eq!(marked(&session), [1, 2, 3]);
}

#[test]
fn an_event_breakpoint_reads_what_the_turn_added_to_the_log() {
    let mut session = played();
    // Every category at once: the first turn that says anything stops the run.
    let all: Vec<&str> = crate::game::EVENT_CATEGORIES.to_vec();
    let set = session.debug(&json!({"op": "breakpoints", "categories": all, "every_seat": true}));
    assert_eq!(set["error"], Value::Null);
    let mut hit = Value::Null;
    for _ in 0..40 {
        let before = session.game.events.len();
        let out = session.debug(&json!({"op": "step"}));
        hit = out["debug"]["stopped"].clone();
        if !hit.is_null() {
            assert!(
                session.game.events.len() > before,
                "a stop names a new event"
            );
            break;
        }
        assert_eq!(
            session.game.events.len(),
            before,
            "a turn that logged something should have stopped"
        );
    }
    assert_eq!(hit["kind"], json!("event"), "forty turns logged nothing");
    let text = hit["text"].as_str().unwrap();
    assert!(session.game.events.iter().any(|event| event.text == text));
}

#[test]
fn a_layer_takes_its_own_decisions_and_leaves_the_turn_open() {
    let mut session = played();
    let set = session.debug(&json!({"op": "layers", "layers": ["research"]}));
    assert_eq!(set["error"], Value::Null);
    assert!(session.debug(&json!({"op": "layers", "layers": ["everything"]}))["error"].is_string());

    let before = session.game.log.len();
    let out = session.debug(&json!({"op": "assist"}));
    let done: Vec<Action> = session
        .game
        .log
        .since(before)
        .map(|(seat, action)| {
            assert_eq!(*seat, 0);
            action.clone()
        })
        .collect();
    assert!(!done.is_empty(), "an opening turn has research to choose");
    assert_eq!(out["debug"]["assisted"], json!(done.len()));
    for action in &done {
        assert_eq!(Layer::of(action), Some(Layer::Research), "{action:?}");
    }
    assert_eq!(session.game.current, 0, "the turn is still the person's");
    assert_eq!(session.game.turn, 1);

    // Switched on, the layer takes its share of every turn the person opens.
    end_turn(&mut session);
    assert_eq!(session.game.turn, 2);
    assert_eq!(session.game.current, 0);
}

#[test]
fn every_layer_together_still_leaves_the_end_of_the_turn_to_the_person() {
    let mut session = played();
    let every: Vec<&str> = Layer::ALL.iter().map(|layer| layer.id()).collect();
    session.debug(&json!({"op": "layers", "layers": every}));
    session.debug(&json!({"op": "assist"}));
    assert_eq!(session.game.turn, 1);
    assert_eq!(session.game.current, 0);
    assert_eq!(Layer::of(&Action::EndTurn), None);
}

#[test]
fn a_watched_table_keeps_no_timeline() {
    let mut params = stock_opening_params(3);
    params.num_players = 2;
    params.num_city_states = 0;
    params.width = 20;
    params.height = 14;
    params.map_topology = MapTopology::Flat;
    params.map_script = MapScript::Pangaea;
    let mut session = Session::new(params);
    assert!(session.debug(&json!({"op": "back"}))["error"].is_string());
    assert!(session.state().get("debug").is_none());
}

#[test]
fn both_front_ends_serve_the_debugger() {
    assert!(include_str!("../wasm.rs").contains("(\"POST\", \"/debug\")"));
    assert!(include_str!("../server.rs").contains("(\"POST\", \"/debug\")"));
    assert!(include_str!("../../beta/shim.js").contains("\"/debug\""));
}

#[test]
fn the_page_loads_the_bar_and_sends_only_operations_the_engine_answers() {
    let page = super::super::EMBEDDED_INDEX_HTML;
    let door = page.find("/assets/app_frontdoor.js").unwrap();
    let bar = page.find("/assets/app_debugger.js").unwrap();
    assert!(door < bar, "the bar reads what every other script declares");
    let served = String::from_utf8(super::super::index_with_app_instance(page, 7)).unwrap();
    assert!(served.contains(r#"<script src="/assets/app_debugger.js?instance=7"></script>"#));
    let script = super::super::EMBEDDED_APP_DEBUGGER_JS;
    assert!(script.starts_with("\"use strict\";"));

    // Read the operations out of the script and put each to the engine, so
    // one renamed on either side fails here and not in somebody's browser.
    let mut session = played();
    let mut rest = script;
    let mut sent = BTreeSet::new();
    while let Some(at) = rest.find("op: \"") {
        rest = &rest[at + 5..];
        let op = &rest[..rest.find('"').expect("a closed string")];
        let answer = session.debug(&json!({"op": op}));
        let refused = answer["error"].as_str().unwrap_or("");
        assert!(
            !refused.starts_with("no debugger operation"),
            "{op}: {refused}"
        );
        sent.insert(op.to_string());
    }
    for op in [
        "back",
        "forward",
        "jump",
        "undo",
        "step",
        "step_action",
        "reveal",
        "assist",
        "layers",
        "breakpoints",
    ] {
        assert!(sent.contains(op), "the bar never sends {op}");
    }
}

#[test]
fn a_fork_is_the_same_world_with_the_same_past_and_a_future_of_its_own() {
    let mut session = played();
    end_turn(&mut session);
    end_turn(&mut session);
    let mut fork = session.fork();
    assert_eq!(world(&fork), world(&session));
    assert_eq!(marked(&fork), [1, 2, 3]);

    end_turn(&mut fork);
    assert_eq!(fork.game.turn, 4);
    assert_eq!(fork.debug_jump(1), Ok(1));
    // Nothing the fork does reaches the session it was taken from.
    assert_eq!(session.game.turn, 3);
    assert_eq!(marked(&session), [1, 2, 3]);
    end_turn(&mut session);
    assert_eq!(marked(&session), [1, 2, 3, 4]);
    assert_eq!(marked(&fork), [1, 2, 3, 4], "the fork kept its own turn 4");
    assert_eq!(fork.game.turn, 1);
}

#[test]
fn the_split_speaks_one_set_of_messages_on_both_sides() {
    let page = super::super::EMBEDDED_APP_DEBUGGER_JS;
    let shim = include_str!("../../beta/shim.js");
    // On the published build a half has no engine: its shim asks the host's.
    // The page script takes no part in that, and each side of it is the shim.
    for message in ["pane-ask", "pane-answer"] {
        let quoted = format!("\"{message}\"");
        assert!(
            shim.matches(&quoted).count() >= 2,
            "the shim sends or hears {message}, not both"
        );
        assert!(!page.contains(&quoted), "{message} is the shim's");
    }
    // Everything else passes between the host and a pane inside the page
    // script: each message has a sender and a listener there.
    for message in [
        "pane-turn",
        "pane-follow",
        "pane-copy",
        "pane-camera",
        "pane-look",
    ] {
        let quoted = format!("\"{message}\"");
        assert!(
            page.matches(&quoted).count() >= 2,
            "{message} is sent or heard, not both"
        );
    }
    // The split closes and copies through the engine, and the engine names
    // those two operations as a split's when no half is named.
    let mut session = played();
    for op in ["adopt", "copy"] {
        assert!(page.contains(&format!("op: \"{op}\"")), "{op}");
        let refused = session.debug(&json!({"op": op}));
        assert!(refused["error"].as_str().unwrap().contains("split"), "{op}");
    }
}

#[test]
fn stepping_in_takes_the_agents_turn_one_action_at_a_time() {
    let mut apart = played();
    let mut whole = played();
    assert!(whole.debug_step());
    let mut actions = 0;
    for _ in 0..400 {
        let before = apart.game.log.len();
        apart.debug_step_action().expect("a live game steps");
        if apart.game.turn == 2 {
            assert_eq!(
                apart.state()["debug"]["stepped"]["action"]["type"],
                json!("end_turn")
            );
            break;
        }
        actions += 1;
        assert_eq!(apart.game.log.len(), before + 1, "one action a step");
        assert_eq!(apart.game.log.last().map(|(seat, _)| *seat), Some(0));
        assert_eq!(apart.state()["debug"]["acted"], json!(actions));
    }
    assert_eq!(apart.game.turn, 2);
    assert!(actions > 0, "an opening turn has something to do");
    // Taken apart or taken whole, it is the same turn.
    assert_eq!(world(&apart), world(&whole));
}

#[test]
fn a_step_in_plans_again_once_the_person_has_acted() {
    let mut session = played();
    session.debug_step_action().expect("the first step");
    let by_hand = something_to_do(&session);
    assert_eq!(session.act(&by_hand), None);
    // The plan was made before that action; the next step is made on the
    // world as it now stands, and can be undone like any other.
    session
        .debug_step_action()
        .expect("a step after the person's own");
    if session.game.turn == 1 {
        assert_eq!(session.state()["debug"]["acted"], json!(3));
        assert_eq!(session.debug_undo(), Ok(2));
    }
}

#[test]
fn reveal_lifts_the_fog_from_the_page_and_changes_nothing_else() {
    let mut session = played();
    for _ in 0..3 {
        assert!(session.debug_step());
    }
    let fogged = session.state();
    let before = world(&session);
    assert!(session.debug(&json!({"op": "reveal"}))["error"].is_string());
    let answer = session.debug(&json!({"op": "reveal", "on": true}));
    assert_eq!(answer["error"], Value::Null);
    let revealed = session.state();
    assert_eq!(revealed["debug"]["reveal"], json!(true));
    let count = |state: &Value, key: &str| state[key].as_array().map_or(0, Vec::len);
    assert!(
        count(&revealed, "units") > count(&fogged, "units"),
        "a rival's units come into view"
    );
    assert_eq!(revealed["legal_actions"], fogged["legal_actions"]);
    assert_eq!(world(&session), before, "looking is not playing");
    // A world walked to, and a fork, are still shown revealed.
    assert_eq!(session.debug_jump(1), Ok(1));
    assert_eq!(session.state()["debug"]["reveal"], json!(true));
    assert_eq!(session.fork().state()["debug"]["reveal"], json!(true));
    session.debug(&json!({"op": "reveal", "on": false}));
    assert_eq!(session.state()["debug"]["reveal"], json!(false));
}

#[test]
fn a_condition_stops_a_run_when_the_state_crosses_its_line() {
    let mut session = played();
    for refused in [
        json!({"seat": 99, "key": "cities", "test": ">=", "value": 1}),
        json!({"seat": 0, "key": "no_such_reading", "test": ">", "value": 1}),
        json!({"seat": 0, "key": "cities", "test": "~", "value": 1}),
        json!({"seat": 0, "key": "cities", "test": ">="}),
        // A name can change, but it is not a number to hold to a line.
        json!({"seat": 0, "key": "civ", "test": ">", "value": 1}),
    ] {
        let answer = session.debug(&json!({"op": "breakpoints", "conditions": [refused.clone()]}));
        assert!(answer["error"].is_string(), "{refused}");
    }
    let set = session.debug(&json!({"op": "breakpoints", "conditions": [
        {"seat": 0, "key": "cities", "test": ">=", "value": 1},
        {"seat": 0, "key": "civ", "test": "changes"},
    ]}));
    assert_eq!(set["error"], Value::Null);
    assert_eq!(
        set["debug"]["breakpoints"]["conditions"][0],
        json!({"seat": 0, "key": "cities", "test": ">=", "value": 1.0})
    );
    assert!(set["debug"]["breakpoints"]["known_tests"]
        .as_array()
        .unwrap()
        .contains(&json!("changes")));

    let mut hit = Value::Null;
    for _ in 0..20 {
        let out = session.debug(&json!({"op": "step"}));
        hit = out["debug"]["stopped"].clone();
        if !hit.is_null() {
            break;
        }
    }
    assert_eq!(
        hit["kind"],
        json!("condition"),
        "twenty turns founded no city"
    );
    assert_eq!(hit["key"], json!("cities"));
    assert_eq!(hit["turn"], json!(session.game.turn));
    assert_eq!(hit["was"], json!(0.0));
    assert!(hit["now"].as_f64().unwrap() >= 1.0);
    // The line stays crossed from here on, and that does not stop every turn
    // after: a condition is the moment it becomes true.
    let next = session.debug(&json!({"op": "step"}));
    assert_eq!(next["debug"]["stopped"], Value::Null);
    // Clearing the breakpoints clears the conditions with them.
    let cleared = session.debug(&json!({"op": "breakpoints"}));
    assert_eq!(cleared["debug"]["breakpoints"]["conditions"], json!([]));
}
