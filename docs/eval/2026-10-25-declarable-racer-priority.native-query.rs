//! Fresh public planner queries on the frozen observational native prefixes.
use civvis::ai::{gene, AdvancedAi, Ai, VictoryTarget, GENES};
use civvis::mirror::{snapshot_from_events_at, state_from_events, LiveMirror};
use civvis::reasoning::Journal;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 5, "query MANIFEST GENOME INDEX off|v1|v2");
    let manifest: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let genome: Value = serde_json::from_slice(&std::fs::read(&args[2]).unwrap()).unwrap();
    let index: usize = args[3].parse().unwrap();
    let arm = args[4].as_str();
    assert!(["off", "v1", "v2"].contains(&arm));
    let case = &manifest["cases"][index];
    assert_eq!(case["status"], "prefix_frozen");
    let path = Path::new(case["path"].as_str().unwrap());
    let turn = case["turn"].as_u64().unwrap() as u32;
    let snapshot = snapshot_from_events_at(path, Some(turn)).unwrap();
    let state = state_from_events(path, Some(turn)).unwrap();
    assert_eq!(state.turn, turn);
    let mut live = LiveMirror::new(
        &snapshot,
        &state,
        state.seat.players.max(2),
        1,
        if state.seat.max_turns > 0 {
            state.seat.max_turns as u32
        } else {
            250
        },
        6,
    );
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let enabled: BTreeSet<_> = genome["treatments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for tag in &enabled {
        assert!(gene(tag).is_some(), "unknown frozen tag {tag}");
    }
    ai.enable_live_bridge_universe();
    for row in GENES {
        if enabled.contains(row.tag) {
            (row.enable)(&mut ai);
        } else {
            (row.disable)(&mut ai);
        }
    }
    ai.disable_elimination_yields_to_a_launch();
    if let Some(row) = gene("elimination-yields-to-a-launch-2") {
        (row.disable)(&mut ai);
    }
    if arm != "off" {
        let tag = if arm == "v1" {
            "elimination-yields-to-a-launch"
        } else {
            "elimination-yields-to-a-launch-2"
        };
        (gene(tag).expect("arm exists in this build").enable)(&mut ai);
    }
    let journal = Journal::recording();
    ai.attach_journal(journal.handle());
    let before = live.game.log.len();
    ai.plan_observed_turn(&mut live.game, 0);
    let update = journal.since(0);
    let thoughts: Vec<_> = update
        .thoughts
        .iter()
        .map(|t| {
            json!({"turn":t.turn,"player":t.player,
        "headline":t.headline,"detail":t.detail,"focus":t.focus})
        })
        .collect();
    println!(
        "{}",
        json!({"scope":"fresh model controller on observational native prefix; not persistent native replay",
        "index":index,"tag":case["tag"],"turn":turn,"arm":arm,
        "plan":format!("{:?}",ai.current_plan()),"actions":live.game.log.since(before).collect::<Vec<_>>(),
        "thoughts":thoughts,"thoughts_dropped":update.dropped,"thought_turns_truncated":update.truncated_turns,
        "counters":live.game.players[0].counters,"game":serde_json::to_value(&live.game).unwrap()})
    );
}
