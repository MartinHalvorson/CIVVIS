//! Registered internal whole-game comparison. A model result is not native permission.
use civvis::ai::{gene, AdvancedAi, Ai, VictoryTarget, GENES};
use civvis::game::{Action, Game, GameOptions};
use civvis::reasoning::Journal;
use civvis::setup::MapScript;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 5, "compare GENOME SEED off|v1|v2 OUT");
    let genome: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let enabled: BTreeSet<_> = genome["treatments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for tag in &enabled {
        assert!(gene(tag).is_some(), "unknown frozen tag: {tag}");
    }
    let seed: u64 = args[2].parse().unwrap();
    assert!((59002800..=59002807).contains(&seed));
    let arm = args[3].as_str();
    assert!(["off", "v1", "v2"].contains(&arm));
    let out = Path::new(&args[4]);
    std::fs::create_dir(out).unwrap();
    let mut g = Game::new_with(GameOptions {
        speed: "online".to_string(),
        map_script: MapScript::Continents,
        randomize_civs: true,
        difficulty: "emperor".to_string(),
        barbarian_difficulty: "deity".to_string(),
        ..GameOptions::new(6, 74, 46, seed, 300, 9)
    });
    let journal = Journal::recording();
    let mut ais: Vec<AdvancedAi> = g
        .players
        .iter()
        .map(|p| {
            if p.is_minor || p.is_barbarian {
                return AdvancedAi::new();
            }
            let mut ai = if p.id == 0 {
                AdvancedAi::targeting(VictoryTarget::Domination)
            } else {
                AdvancedAi::new()
            };
            ai.enable_engine_repairs_universe();
            for row in GENES {
                if enabled.contains(row.tag) {
                    (row.enable)(&mut ai);
                } else {
                    (row.disable)(&mut ai);
                }
            }
            if p.id == 0 {
                for tag in [
                    "elimination-yields-to-a-launch",
                    "elimination-yields-to-a-launch-2",
                ] {
                    (gene(tag).expect("registered family").disable)(&mut ai);
                }
                if arm != "off" {
                    let tag = if arm == "v1" {
                        "elimination-yields-to-a-launch"
                    } else {
                        "elimination-yields-to-a-launch-2"
                    };
                    (gene(tag).unwrap().enable)(&mut ai);
                }
                ai.attach_journal(journal.handle());
            }
            ai
        })
        .collect();
    g.set_fog_memory(ais.iter().any(Ai::uses_player_observation));
    g.set_war_ledger(false);
    std::fs::write(
        out.join("initial.game.json"),
        serde_json::to_vec(&g).unwrap(),
    )
    .unwrap();
    let mut trace = BufWriter::new(std::fs::File::create(out.join("trace.jsonl")).unwrap());
    let start = Instant::now();
    let mut seen_turn = None;
    let mut thought_cursor = 0;
    while g.winner.is_none() && g.turn <= 300 {
        let turn = g.turn;
        if seen_turn != Some(turn) {
            seen_turn = Some(turn);
            if turn % 5 == 0 {
                eprintln!("seed {seed} arm {arm} turn {turn}");
            }
        }
        let pid = g.current;
        let before = g.log.len();
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            g.apply(pid, &Action::EndTurn).unwrap();
        }
        if pid == 0 {
            let update = journal.since(thought_cursor);
            thought_cursor = update.cursor;
            let thoughts: Vec<_> = update
                .thoughts
                .iter()
                .map(|t| json!({"turn":t.turn,"headline":t.headline,"detail":t.detail}))
                .collect();
            let row = json!({"turn":turn,"actions":g.log.since(before).collect::<Vec<_>>(),
                "plan":format!("{:?}",ais[0].current_plan()), "thoughts":thoughts,
                "thought_cursor":update.cursor,"thoughts_dropped":update.dropped,"thought_turns_truncated":update.truncated_turns,
                "counters":g.players[0].counters});
            serde_json::to_writer(&mut trace, &row).unwrap();
            writeln!(trace).unwrap();
            trace.flush().unwrap();
        }
    }
    std::fs::write(
        out.join("terminal.game.json"),
        serde_json::to_vec(&g).unwrap(),
    )
    .unwrap();
    let scores: Vec<_> = (0..6).map(|pid| json!({"player":pid,"score":g.score(pid),
        "cities":g.player_city_ids(pid).len(),"original_capitals":g.cities.values().filter(|c| c.owner==pid && c.is_capital).map(|c|c.original_owner).collect::<Vec<_>>(),
        "counters":g.players[pid].counters})).collect();
    let result = json!({"scope":"registered internal development game; not a native verification",
        "seed":seed,"arm":arm,"turn":g.turn,"winner":g.winner,"victory":g.victory_label(),
        "wall_seconds":start.elapsed().as_secs_f64(),"scores":scores,"source_genome":genome});
    std::fs::write(
        out.join("result.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
}
