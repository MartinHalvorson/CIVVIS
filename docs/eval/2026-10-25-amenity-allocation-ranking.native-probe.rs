//! Standalone diagnostic, linked against each unmodified library artifact.
//! Reconstruct and clone outside the timed planner call. Never drive a live game.
use civvis::ai::{gene, AdvancedAi, Ai, VictoryTarget};
use civvis::mirror::{snapshot_from_events_at, state_from_events, LiveMirror};
use civvis::reasoning::Journal;
use serde_json::{json, Value};
use std::os::raw::c_long;
use std::path::Path;
use std::time::Instant;

extern "C" {
    fn clock() -> c_long;
}

fn configured(case: &Value, journal: &Journal) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let forced: Vec<&str> = case["forced"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tag| {
            gene(tag.as_str().unwrap())
                .expect("registered forced gene")
                .tag
        })
        .collect();
    ai.enable_live_bridge_universe();
    ai.apply_gene_ledger_with_forced_live(&forced);
    for tag in case["withheld"].as_array().unwrap() {
        (gene(tag.as_str().unwrap())
            .expect("registered withheld gene")
            .disable)(&mut ai);
    }
    ai.attach_journal(journal.handle());
    ai
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 4, "probe PREPARATION CASE_INDEX REPETITIONS");
    let manifest: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let index: usize = args[2].parse().unwrap();
    let repetitions: usize = args[3].parse().unwrap();
    assert!(repetitions > 0);
    let case = &manifest["cases"][index];
    let turn = case["turn"].as_u64().unwrap() as u32;
    let events = Path::new(case["directory"].as_str().unwrap()).join("events.jsonl");
    let snapshot = snapshot_from_events_at(&events, Some(turn)).unwrap();
    let state = state_from_events(&events, Some(turn)).unwrap();
    let players = state.seat.players.max(2);
    let turns = if state.seat.max_turns > 0 {
        state.seat.max_turns as u32
    } else {
        250
    };
    let live = LiveMirror::new(&snapshot, &state, players, 1, turns, 6);
    let mut reference = None;
    let mut samples = Vec::new();
    for _ in 0..repetitions {
        let mut game = live.game.clone();
        let journal = Journal::recording();
        let mut ai = configured(case, &journal);
        let before = unsafe { clock() };
        let wall = Instant::now();
        ai.plan_observed_turn(&mut game, 0);
        let nanos = wall.elapsed().as_nanos();
        let after = unsafe { clock() };
        assert!(before >= 0 && after >= before, "process CPU clock failed");
        let thoughts: Vec<Value> = journal
            .since(0)
            .thoughts
            .iter()
            .map(|thought| {
                json!({
                    "turn": thought.turn, "player": thought.player,
                    "topic": format!("{:?}", thought.topic),
                    "level": format!("{:?}", thought.level),
                    "headline": thought.headline, "detail": thought.detail,
                    "focus": thought.focus,
                })
            })
            .collect();
        let active = thoughts.iter().any(|thought| {
            thought["headline"]
                .as_str()
                .is_some_and(|text| text.contains("last district slot waits for the airfield"))
        });
        let semantic = json!({
            "game": serde_json::to_value(&game).unwrap(),
            "actions": game.log.iter().collect::<Vec<_>>(),
            "plan": format!("{:?}", ai.current_plan()),
            "reasoning": thoughts,
        });
        if let Some(first) = &reference {
            assert_eq!(first, &semantic, "fresh planner repetitions differ");
        } else {
            reference = Some(semantic);
        }
        samples.push(json!({"process_cpu_ticks": after - before,
                            "wall_nanoseconds": nanos, "reservation_active": active}));
    }
    println!(
        "{}",
        json!({"case": index, "turn": turn, "samples": samples, "semantic": reference})
    );
}
