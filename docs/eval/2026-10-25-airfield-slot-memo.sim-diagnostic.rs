//! Fresh-planner diagnostic over registered full simulation snapshots.
use civvis::ai::{AdvancedAi, Ai, VictoryTarget, GENES};
use civvis::game::Game;
use civvis::reasoning::Journal;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::os::raw::c_long;
use std::time::Instant;

extern "C" {
    fn clock() -> c_long;
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 4, "probe PREPARATION INDEX REPETITIONS");
    let prep: Value = serde_json::from_slice(&std::fs::read(&args[1]).unwrap()).unwrap();
    let index: usize = args[2].parse().unwrap();
    let repetitions: usize = args[3].parse().unwrap();
    assert!(repetitions > 0);
    let case = &prep["cases"][index];
    let pid = case["pid"].as_u64().unwrap() as usize;
    let original: Game =
        serde_json::from_slice(&std::fs::read(case["input_file"].as_str().unwrap()).unwrap())
            .unwrap();
    assert_eq!(original.current, pid);
    assert_eq!(original.turn as u64, case["turn"].as_u64().unwrap());
    let original = original.player_decision_view(pid);
    let genome: Value =
        serde_json::from_slice(&std::fs::read(prep["genome_file"].as_str().unwrap()).unwrap())
            .unwrap();
    let enabled: BTreeSet<&str> = genome["treatments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    let mut reference = None;
    let mut samples = Vec::new();
    for repetition in 0..repetitions {
        eprintln!("call={repetition} phase=clone_begin cpu={}", unsafe {
            clock()
        });
        let mut game = original.clone();
        eprintln!("call={repetition} phase=clone_done cpu={}", unsafe {
            clock()
        });
        let journal = Journal::recording();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_engine_repairs_universe();
        for treatment in GENES {
            if enabled.contains(treatment.tag) {
                (treatment.enable)(&mut ai);
            } else {
                (treatment.disable)(&mut ai);
            }
        }
        ai.attach_journal(journal.handle());
        let start_log = game.log.len();
        eprintln!("call={repetition} phase=plan_begin cpu={}", unsafe {
            clock()
        });
        let before = unsafe { clock() };
        let wall = Instant::now();
        ai.plan_observed_turn(&mut game, pid);
        let nanos = wall.elapsed().as_nanos();
        let after = unsafe { clock() };
        eprintln!("call={repetition} phase=plan_done cpu={after} wall_nanos={nanos}");
        assert!(before >= 0 && after >= before);
        let thoughts: Vec<Value> = journal
            .since(0)
            .thoughts
            .iter()
            .map(|t| {
                json!({
                    "turn": t.turn, "player": t.player, "topic": format!("{:?}",t.topic),
                    "level": format!("{:?}",t.level), "headline": t.headline,
                    "detail": t.detail, "focus": t.focus,
                })
            })
            .collect();
        let active = thoughts.iter().any(|t| {
            t["headline"]
                .as_str()
                .is_some_and(|h| h.contains("last district slot waits for the airfield"))
        });
        eprintln!("call={repetition} phase=serialize_begin cpu={}", unsafe {
            clock()
        });
        let semantic = json!({"game": serde_json::to_value(&game).unwrap(),
            "new_actions": game.log.since(start_log).collect::<Vec<_>>(),
            "plan": format!("{:?}",ai.current_plan()), "reasoning": thoughts});
        eprintln!("call={repetition} phase=serialize_done cpu={}", unsafe {
            clock()
        });
        if let Some(first) = &reference {
            if first != &semantic {
                let directory =
                    std::path::PathBuf::from(std::env::var("CIVVIS_PROBE_DIAGNOSTIC_DIR").unwrap());
                std::fs::write(
                    directory.join("first.semantic.json"),
                    serde_json::to_vec(first).unwrap(),
                )
                .unwrap();
                std::fs::write(
                    directory.join("different.semantic.json"),
                    serde_json::to_vec(&semantic).unwrap(),
                )
                .unwrap();
                panic!("fresh planner repetitions differ; complete values saved in diagnostic directory");
            }
        } else {
            reference = Some(semantic);
        }
        eprintln!("call={repetition} phase=compare_done cpu={}", unsafe {
            clock()
        });
        samples.push(
            json!({"process_cpu_ticks": after-before, "wall_nanoseconds": nanos,
                            "reservation_active": active}),
        );
    }
    println!(
        "{}",
        json!({"case": index, "turn": original.turn, "pid": pid,
                         "samples": samples, "semantic": reference})
    );
}
