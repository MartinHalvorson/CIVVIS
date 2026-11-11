//! TEMPORARY CI scaffold, removed before ship: the `siege_replay` arms for
//! `first-capital-strikes-the-open-window` on the live boards, played on the
//! CI runner because this host parks compilers. The report is the panic text.

use crate::ai::advanced::AdvancedAi;
use crate::ai::{Ai, VictoryTarget};
use crate::game::{Action, Game};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const TAG: &str = "first-capital-strikes-the-open-window";
const NEEDLES: &[&str] = &[
    "first capital",
    "conquest",
    "declaring war",
    "campaign aimed",
    "countering",
    "interception",
    "holding off war",
    "grand strategy",
    "reserves",
];

fn scaffold(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/ai/advanced/first_capital_strikes_the_open_window/scaffold")
        .join(name)
}

fn unpacked(name: &str) -> PathBuf {
    let out = std::env::temp_dir().join(format!("open-window-{}-{name}.jsonl", std::process::id()));
    if !out.exists() {
        let partial = out.with_extension("partial");
        let data = std::process::Command::new("gzip")
            .arg("-dc")
            .arg(scaffold(&format!("{name}.jsonl.gz")))
            .output()
            .expect("gzip");
        std::fs::write(&partial, data.stdout).unwrap();
        std::fs::rename(&partial, &out).unwrap();
    }
    out
}

fn base(name: &str, turn: u32) -> Game {
    let events = unpacked(name);
    let snapshot = crate::mirror::snapshot_from_events_at(&events, Some(turn)).unwrap();
    let state = crate::mirror::state_from_events(&events, Some(turn)).unwrap();
    let players = if state.seat.players > 0 {
        state.seat.players
    } else {
        4
    };
    let max_turns = if state.seat.max_turns > 0 {
        state.seat.max_turns as u32
    } else {
        250
    };
    let mut game =
        crate::mirror::rebuild_from_state(&snapshot, &state, players, 1, max_turns, 6).game;
    // The live state reports the walls of every city the seat knows
    // (`rivals[].cities[].max_wall_damage`), and the live decision view read
    // them ("walls 100 of at most 100" at t57). The rebuilt seat's city memory
    // was made before those observations and reads 0 until it sees the city
    // again; carry the reported walls into the memory, as live knows them.
    let walls: Vec<(u32, i32, i32)> = game
        .cities
        .values()
        .map(|city| (city.id, game.city_max_wall_hp(city), city.wall_hp))
        .collect();
    for (id, max, hp) in walls {
        if let Some(memory) = game.players[0].remembered_cities.get_mut(&id) {
            memory.wall_max = max;
            memory.wall_hp = hp;
        }
    }
    game
}

fn military(g: &Game, pid: usize) -> usize {
    g.units
        .values()
        .filter(|unit| unit.owner == pid && g.rules.units[unit.kind].class == "military")
        .count()
}

struct Job {
    board: &'static str,
    target: &'static str,
    turn: u32,
    on: bool,
    seed: u64,
    turns: u32,
}

fn play(job: &Job, genome: &[String]) -> String {
    let mut g = base(job.board, job.turn);
    let (target_pos, owner) = {
        let city = g
            .cities
            .values()
            .find(|city| city.name.to_lowercase() == job.target.to_lowercase())
            .expect("target city");
        (city.pos, city.owner)
    };
    let head = {
        let city = g
            .cities
            .values()
            .find(|city| city.pos == target_pos)
            .unwrap();
        format!(
            "{} t{}: {} owner {} walls {}/{} (the seat's view {}) hp {} strength {:.0}; our military {} power {:.0} against theirs {:.0}; cities {}",
            job.board,
            g.turn,
            city.name,
            owner,
            city.wall_hp,
            g.city_max_wall_hp(city),
            {
                let view = g.player_decision_view(0);
                view.cities
                    .get(&city.id)
                    .map_or(-1, |seen| view.city_max_wall_hp(seen))
            },
            city.hp,
            g.city_strength(city.id),
            military(&g, 0),
            g.military_power(0),
            g.military_power(owner),
            g.player_city_ids(0).len()
        )
    };
    g.rng = crate::rng::Rng::new(9_100 + job.seed);
    let start = g.turn;
    g.max_turns = g.max_turns.max(start + job.turns + 1);
    let journal = crate::reasoning::Journal::recording();
    let mut forced: Vec<&str> = genome.iter().map(String::as_str).collect();
    if job.on {
        forced.push(TAG);
    }
    let mut ais: Vec<Box<dyn Ai>> = Vec::new();
    for pid in 0..g.players.len() {
        if pid == 0 {
            let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
            ai.enable_live_bridge_universe();
            ai.apply_gene_ledger_with_forced_live(&forced);
            assert_eq!(ai.first_capital_strikes_the_open_window, job.on);
            assert!(ai.first_capital_before_the_walls && ai.early_conquest_opening);
            ai.attach_journal(journal.handle());
            ais.push(Box::new(ai));
        } else {
            ais.push(Box::new(AdvancedAi::new()));
        }
    }
    let units_at_start = military(&g, 0);
    let ours: BTreeSet<(i32, i32)> = g
        .cities
        .values()
        .filter(|city| city.owner == 0)
        .map(|city| city.pos)
        .collect();
    let theirs: BTreeSet<(i32, i32)> = g
        .cities
        .values()
        .filter(|city| city.owner == owner)
        .map(|city| city.pos)
        .collect();
    let (mut war, mut taken, mut walls_down, mut min_hp) = (None, None, None, i32::MAX);
    let mut trace = String::new();
    let traced = job.seed == 0 && job.board == "040704Z";
    let mut last_traced = 0;
    while g.winner.is_none() && g.turn < start + job.turns {
        let pid = g.current;
        if traced && pid == 0 && g.turn != last_traced {
            last_traced = g.turn;
            let breakers: Vec<String> = g
                .units
                .values()
                .filter(|unit| {
                    unit.owner == 0 && matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
                })
                .map(|unit| {
                    let stack = g.unit_ids_at(unit.pos).len();
                    format!(
                        "{}@{:?} d{} hp{} stack{}",
                        unit.kind,
                        unit.pos,
                        g.wdist(unit.pos, target_pos),
                        unit.hp,
                        stack
                    )
                })
                .collect();
            let near: Vec<String> = g
                .units
                .values()
                .filter(|unit| {
                    unit.owner == 0
                        && g.rules.units[unit.kind].class == "military"
                        && g.wdist(unit.pos, target_pos) <= 6
                })
                .map(|unit| {
                    format!(
                        "{}d{}hp{}",
                        unit.kind,
                        g.wdist(unit.pos, target_pos),
                        unit.hp
                    )
                })
                .collect();
            let theirs = g
                .units
                .values()
                .filter(|unit| {
                    unit.owner == owner
                        && g.rules.units[unit.kind].class == "military"
                        && g.wdist(unit.pos, target_pos) <= 3
                })
                .count();
            let city = g.cities.values().find(|city| city.pos == target_pos);
            trace.push_str(&format!(
                "TRACE|{} t{} {}|t{} war {} city hp {} walls {} strength {:.0}; their bodies within 3: {}; ours within 6: {:?}; breakers {:?}\n",
                job.board,
                job.turn,
                if job.on { "on" } else { "off" },
                g.turn,
                g.is_at_war(0, owner),
                city.map_or(-1, |c| c.hp),
                city.map_or(-1, |c| c.wall_hp),
                city.map_or(0.0, |c| g.city_strength(c.id)),
                theirs,
                near,
                breakers
            ));
        }
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if war.is_none() && g.is_at_war(0, owner) {
            war = Some(g.turn);
        }
        match g.cities.values().find(|city| city.pos == target_pos) {
            Some(city) => {
                min_hp = min_hp.min(city.hp);
                if city.wall_hp <= 0 && walls_down.is_none() {
                    walls_down = Some(g.turn);
                }
                if city.owner == 0 && taken.is_none() {
                    taken = Some(g.turn);
                }
            }
            None => {
                if taken.is_none() {
                    taken = Some(g.turn);
                }
            }
        }
    }
    let owner_taken = g
        .cities
        .values()
        .filter(|city| city.owner == 0 && theirs.contains(&city.pos))
        .count();
    let lost = ours
        .iter()
        .filter(|pos| g.city_at(**pos).is_none_or(|cid| g.cities[&cid].owner != 0))
        .count();
    let other_wars: Vec<usize> = (1..g.players.len())
        .filter(|rival| {
            *rival != owner && g.is_at_war(0, *rival) && !g.players[*rival].is_barbarian
        })
        .collect();
    let fmt = |turn: Option<u32>| turn.map_or("-".to_string(), |t| format!("t{t}"));
    let thoughts: Vec<_> = journal
        .since(0)
        .thoughts
        .into_iter()
        .filter(|thought| thought.player == 0)
        .collect();
    let first = |needle: &str| {
        thoughts
            .iter()
            .find(|thought| thought.headline.contains(needle))
            .map(|thought| format!("t{}", thought.turn))
            .unwrap_or_else(|| "-".to_string())
    };
    let count = |needle: &str| {
        thoughts
            .iter()
            .filter(|thought| thought.headline.contains(needle))
            .count()
    };
    let releases: Vec<String> = thoughts
        .iter()
        .filter(|thought| {
            thought
                .headline
                .starts_with("Releasing the conquest opening")
        })
        .map(|thought| {
            format!(
                "t{} {}",
                thought.turn,
                thought.detail.chars().take(70).collect::<String>()
            )
        })
        .collect();
    let declared_by: Vec<String> = thoughts
        .iter()
        .filter(|thought| {
            thought.headline.starts_with("Declaring war on")
                || thought
                    .headline
                    .starts_with("Opening a religious interception")
        })
        .map(|thought| {
            format!(
                "t{} {} | {}",
                thought.turn,
                thought.headline,
                thought.detail.chars().take(60).collect::<String>()
            )
        })
        .collect();
    let mut out = format!(
        "ROW|{}|t{}|{}|seed {}|war {}|taken {}|walls down {}|min hp {}|owner cities taken {}|units lost {}|our cities lost {}|other wars at end {:?}|end t{}|named {}|go {}|keeps {}|faith-no-front {}|held wars {}|soft {}|grace {}|releases {:?}|declarations {:?}\n",
        job.board,
        job.turn,
        if job.on { "on" } else { "off" },
        job.seed,
        fmt(war),
        fmt(taken),
        fmt(walls_down),
        min_hp,
        owner_taken,
        units_at_start.saturating_sub(military(&g, 0)),
        lost,
        other_wars,
        g.turn,
        first("Opening a conquest against"),
        first("the open window"),
        count("the opening keeps the campaign"),
        count("the faith war is no front"),
        count("holding off war"),
        first(" is soft"),
        first("the window holds"),
        releases,
        declared_by
    );
    out.push_str(&trace);
    if job.seed == 0 {
        out.push_str(&format!("HEAD|{head}\n"));
        let mut lines = 0;
        for thought in &thoughts {
            let headline = thought.headline.to_lowercase();
            if !NEEDLES.iter().any(|needle| headline.contains(needle)) {
                continue;
            }
            if headline.starts_with("policy portfolio") || headline.starts_with("adopting") {
                continue;
            }
            let detail: String = thought.detail.chars().take(200).collect();
            out.push_str(&format!(
                "WHY|{} t{} {}|  t{} {} | {}\n",
                job.board,
                job.turn,
                if job.on { "on" } else { "off" },
                thought.turn,
                thought.headline,
                detail
            ));
            lines += 1;
            if lines >= 90 {
                break;
            }
        }
    }
    out
}

#[test]
fn scaffold_open_window_replays() {
    let genome: Vec<String> = std::fs::read_to_string(scaffold("genome.txt"))
        .unwrap()
        .split(',')
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty())
        .collect();
    let mut jobs = Vec::new();
    for turn in [30, 34, 38, 42] {
        for on in [false, true] {
            for seed in 0..10 {
                jobs.push(Job {
                    board: "040704Z",
                    target: "Mikisiw-Wacîhk",
                    turn,
                    on,
                    seed,
                    turns: 28,
                });
            }
        }
    }
    for on in [false, true] {
        for seed in 0..6 {
            jobs.push(Job {
                board: "040245Z",
                target: "Wanuku",
                turn: 24,
                on,
                seed,
                turns: 28,
            });
        }
    }
    // Unpack each board once, before any worker reads it.
    unpacked("040704Z");
    unpacked("040245Z");
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(job) = jobs.get(index) else {
                    break;
                };
                let text =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| play(job, &genome)))
                        .unwrap_or_else(|_| {
                            format!(
                                "ROW|{}|t{}|{}|seed {}|PANICKED\n",
                                job.board, job.turn, job.on, job.seed
                            )
                        });
                results.lock().unwrap().push((index, text));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort();
    let report: String = results.into_iter().map(|(_, text)| text).collect();
    panic!("OPEN-WINDOW-REPORT-BEGIN\n{report}OPEN-WINDOW-REPORT-END");
}
