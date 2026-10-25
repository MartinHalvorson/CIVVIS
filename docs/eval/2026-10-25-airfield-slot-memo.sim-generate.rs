//! Freeze pre-action simulation boards at registered turn/seat milestones.
use civvis::ai::{gene, AdvancedAi, Ai, VictoryTarget, GENES};
use civvis::game::{Action, Game, GameOptions};
use civvis::setup::MapScript;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 4, "generate GENOME SEED OUT");
    let genome: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let enabled: BTreeSet<&str> = genome["treatments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let unknown: Vec<&str> = enabled
        .iter()
        .copied()
        .filter(|tag| gene(tag).is_none())
        .collect();
    assert!(
        unknown.is_empty(),
        "unregistered enabled tags: {:?}",
        unknown
    );
    let seed: u64 = args[2].parse().unwrap();
    assert!([59002700, 59002701].contains(&seed));
    let out = Path::new(&args[3]);
    std::fs::create_dir(out).unwrap();
    let mut world = Game::new_with(GameOptions {
        speed: "online".to_string(),
        map_script: MapScript::Continents,
        randomize_civs: true,
        difficulty: "emperor".to_string(),
        barbarian_difficulty: "deity".to_string(),
        ..GameOptions::new(6, 74, 46, seed, 250, 9)
    });
    let mut ais: Vec<AdvancedAi> = world
        .players
        .iter()
        .map(|player| {
            if player.is_minor || player.is_barbarian {
                return AdvancedAi::new();
            }
            let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
            ai.enable_engine_repairs_universe();
            for treatment in GENES {
                if enabled.contains(treatment.tag) {
                    (treatment.enable)(&mut ai);
                } else {
                    (treatment.disable)(&mut ai);
                }
            }
            ai
        })
        .collect();
    world.set_fog_memory(ais.iter().any(Ai::uses_player_observation));
    world.set_war_ledger(false);
    let turns = [50, 100, 150, 200];
    let mut cases: Vec<Value> = turns
        .iter()
        .flat_map(|turn| {
            (0..6).map(move |pid| {
                json!({"seed": seed, "turn": turn, "pid": pid,
                                   "status": "not_reached"})
            })
        })
        .collect();
    let mut observed = None;
    while world.winner.is_none() && world.turn <= 200 {
        if observed != Some(world.turn) {
            observed = Some(world.turn);
            if world.turn % 5 == 0 {
                eprintln!("seed {} turn {}", seed, world.turn);
            }
        }
        let pid = world.current;
        if pid < 6 && turns.contains(&world.turn) {
            let case = cases
                .iter_mut()
                .find(|c| c["turn"] == world.turn && c["pid"] == pid)
                .unwrap();
            if case["status"] == "not_reached" {
                let filename = format!("t{}-p{}.game.json", world.turn, pid);
                std::fs::write(out.join(&filename), serde_json::to_vec(&world).unwrap()).unwrap();
                case["status"] = json!("board_saved");
                case["file"] = json!(filename);
                serde_json::to_writer_pretty(
                    std::fs::File::create(out.join("cases.json")).unwrap(),
                    &cases,
                )
                .unwrap();
            }
        }
        ais[pid].take_turn(&mut world, pid);
        if world.winner.is_none() && world.current == pid {
            world.apply(pid, &Action::EndTurn).unwrap();
        }
    }
    let summary = json!({"scope": "registered simulation snapshot generation; not a strength trial",
        "seed": seed, "last_turn": world.turn, "winner": world.winner,
        "enabled_treatments": enabled, "unknown_tags": unknown, "cases": cases});
    serde_json::to_writer_pretty(
        std::fs::File::create(out.join("result.json")).unwrap(),
        &summary,
    )
    .unwrap();
}
