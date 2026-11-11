//! TEMPORARY CI scaffold, removed before ship: `siege_replay`'s loop for
//! `breaker-walks-under-a-shield` on the live boards, played on the CI runner
//! because this host parks compilers. The report is the panic text.

use crate::ai::advanced::AdvancedAi;
use crate::ai::{Ai, VictoryTarget};
use crate::game::{Action, Game};
use crate::Pos;
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

const TAG: &str = "breaker-walks-under-a-shield";
/// Armed live since these boards were recorded, so in both arms.
const ARMED_SINCE: &[&str] = &[
    "first-capital-strikes-the-open-window",
    "city-state-before-the-first-capital",
];
const NEEDLES: &[&str] = &[
    "breaker under a shield",
    "siege of",
    "conquest opening",
    "declaring war",
    "city-state before",
    "first capital",
    "opening a conquest",
    "yields",
];

fn scaffold(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src/ai/advanced/breaker_walks_under_a_shield/scaffold")
        .join(name)
}

fn unpacked(name: &str) -> PathBuf {
    let out = std::env::temp_dir().join(format!("shield-{}-{name}.jsonl", std::process::id()));
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

fn genome(board: &str) -> Vec<String> {
    let mut tags: Vec<String> = std::fs::read_to_string(scaffold(&format!("genome-{board}.txt")))
        .unwrap()
        .split(',')
        .map(|tag| tag.trim().to_string())
        .filter(|tag| !tag.is_empty() && tag != TAG)
        .collect();
    for tag in ARMED_SINCE {
        if !tags.iter().any(|have| have == tag) {
            tags.push(tag.to_string());
        }
    }
    tags
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
    // As #5058: the seat's city memory reads the walls the live state reported.
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

fn military(g: &Game, pid: usize) -> BTreeSet<u32> {
    g.units
        .values()
        .filter(|unit| unit.owner == pid && g.rules.units[unit.kind].class == "military")
        .map(|unit| unit.id)
        .collect()
}

fn breakers(g: &Game) -> Vec<u32> {
    g.units
        .values()
        .filter(|unit| {
            unit.owner == 0 && matches!(unit.kind.as_str(), "battering_ram" | "siege_tower")
        })
        .map(|unit| unit.id)
        .collect()
}

struct Job {
    board: &'static str,
    turn: u32,
    on: bool,
    seed: u64,
    turns: u32,
}

fn city_line(g: &Game, pos: Pos) -> String {
    match g.city_at(pos).and_then(|cid| g.cities.get(&cid)) {
        Some(city) => format!(
            "{} hp {} walls {}/{} str {:.0} owner {}",
            city.name,
            city.hp,
            city.wall_hp,
            g.city_max_wall_hp(city),
            g.city_strength(city.id),
            city.owner
        ),
        None => "gone".to_string(),
    }
}

fn play(job: &Job, genome: &[String]) -> String {
    let mut g = base(job.board, job.turn);
    g.rng = crate::rng::Rng::new(9_100 + job.seed);
    let start = g.turn;
    g.max_turns = g.max_turns.max(start + job.turns + 1);
    let journal = crate::reasoning::Journal::recording();
    let mut forced: Vec<&str> = genome.iter().map(String::as_str).collect();
    if job.on {
        forced.push(TAG);
    }
    let mut seat = AdvancedAi::targeting(VictoryTarget::Domination);
    seat.enable_live_bridge_universe();
    seat.apply_gene_ledger_with_forced_live(&forced);
    assert_eq!(seat.breaker_walks_under_a_shield, job.on);
    assert!(seat.early_conquest_opening);
    seat.attach_journal(journal.handle());
    let mut others: Vec<AdvancedAi> = (0..g.players.len()).map(|_| AdvancedAi::new()).collect();

    let mut seen: BTreeSet<u32> = military(&g, 0);
    let mut target: Option<(Pos, usize)> = None;
    let mut target_name = String::from("-");
    let (mut war, mut taken, mut walls_down, mut min_hp) = (None, None, None, i32::MAX);
    let (mut ram_beside, mut first_hit, mut ram_alive_at_hit) = (None, None, None);
    let mut ram_alive_after_war = None;
    let mut ram_lost = None;
    let mut ram_ids: BTreeSet<u32> = breakers(&g).into_iter().collect();
    let mut trace = String::new();
    let traced = job.seed == 0;
    let mut before_ours: Option<(i32, i32)> = None;
    let mut after_ours: Option<(i32, i32)> = None;
    while g.winner.is_none() && g.turn < start + job.turns {
        let pid = g.current;
        if pid == 0 {
            // The opening's city, as the seat names it.
            if war.is_none() {
                if let Some(opening) = seat.conquest_opening.as_ref() {
                    if let Some(city) = g.cities.get(&opening.city) {
                        target = Some((city.pos, opening.target));
                        target_name = city.name.clone();
                    }
                }
            }
            if let Some((pos, _)) = target {
                let state = g
                    .city_at(pos)
                    .and_then(|cid| g.cities.get(&cid))
                    .map(|city| (city.hp, city.wall_hp));
                if traced {
                    if let (Some(after), Some(now)) = (after_ours, state) {
                        trace.push_str(&format!(
                            "TRACE|{} t{} {}|  their turn: hp {} -> {}, walls {} -> {}\n",
                            job.board,
                            job.turn,
                            if job.on { "on" } else { "off" },
                            after.0,
                            now.0,
                            after.1,
                            now.1
                        ));
                    }
                }
                before_ours = state;
            }
            seat.take_turn(&mut g, 0);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        seen.extend(military(&g, 0));
        ram_ids.extend(breakers(&g));
        let Some((pos, owner)) = target else {
            continue;
        };
        if war.is_none() && g.is_at_war(0, owner) {
            war = Some(g.turn);
        }
        let rams_alive: Vec<u32> = ram_ids
            .iter()
            .copied()
            .filter(|id| g.units.contains_key(id))
            .collect();
        if ram_lost.is_none() && war.is_some() && rams_alive.is_empty() && !ram_ids.is_empty() {
            ram_lost = Some(g.turn);
        }
        if pid == 0 {
            let state = g
                .city_at(pos)
                .and_then(|cid| g.cities.get(&cid))
                .map(|city| (city.hp, city.wall_hp));
            after_ours = state;
            if let (Some(before), Some(after)) = (before_ours, state) {
                if first_hit.is_none() && (after.0 < before.0 || after.1 < before.1) {
                    first_hit = Some(g.turn);
                    ram_alive_at_hit = Some(!rams_alive.is_empty());
                }
            }
            if war.is_some() && ram_alive_after_war.is_none() && g.turn > war.unwrap_or(0) {
                ram_alive_after_war = Some(!rams_alive.is_empty());
            }
            if ram_beside.is_none()
                && rams_alive
                    .iter()
                    .any(|id| g.wdist(g.units[id].pos, pos) <= 1)
            {
                ram_beside = Some(g.turn);
            }
            if traced {
                let rams: Vec<String> = rams_alive
                    .iter()
                    .map(|id| {
                        let unit = &g.units[id];
                        let stack: Vec<String> = g
                            .unit_ids_at(unit.pos)
                            .iter()
                            .filter(|other| **other != *id)
                            .map(|other| g.units[other].kind.to_string())
                            .collect();
                        format!(
                            "{}@{:?} d{} linked {} with {:?}",
                            unit.kind,
                            unit.pos,
                            g.wdist(unit.pos, pos),
                            unit.linked_to.is_some(),
                            stack
                        )
                    })
                    .collect();
                let ours: Vec<String> = g
                    .units
                    .values()
                    .filter(|unit| {
                        unit.owner == 0
                            && g.rules.units[unit.kind].class == "military"
                            && g.wdist(unit.pos, pos) <= 4
                    })
                    .map(|unit| format!("{}d{}hp{}", unit.kind, g.wdist(unit.pos, pos), unit.hp))
                    .collect();
                let theirs = g
                    .units
                    .values()
                    .filter(|unit| {
                        unit.owner != 0
                            && g.rules.units[unit.kind].class == "military"
                            && g.wdist(unit.pos, pos) <= 2
                    })
                    .map(|unit| {
                        format!(
                            "{}(p{})d{}hp{}",
                            unit.kind,
                            unit.owner,
                            g.wdist(unit.pos, pos),
                            unit.hp
                        )
                    })
                    .collect::<Vec<_>>();
                trace.push_str(&format!(
                    "TRACE|{} t{} {}|t{} war {} our turn: {:?} -> {} | ours within 4: {:?} | theirs within 2: {:?} | rams {:?}\n",
                    job.board,
                    job.turn,
                    if job.on { "on" } else { "off" },
                    g.turn,
                    g.is_at_war(0, owner),
                    before_ours,
                    city_line(&g, pos),
                    ours,
                    theirs,
                    rams
                ));
            }
        }
        match g.city_at(pos).and_then(|cid| g.cities.get(&cid)) {
            Some(city) => {
                min_hp = min_hp.min(city.hp);
                if city.wall_hp <= 0 && walls_down.is_none() && war.is_some() {
                    walls_down = Some(g.turn);
                }
                if city.owner == 0 && taken.is_none() {
                    taken = Some(g.turn);
                }
            }
            None => {
                if taken.is_none() && war.is_some() {
                    taken = Some(g.turn);
                }
            }
        }
    }
    let lost = seen.iter().filter(|id| !g.units.contains_key(id)).count();
    let fmt = |turn: Option<u32>| turn.map_or("-".to_string(), |t| format!("t{t}"));
    let flag = |value: Option<bool>| value.map_or("-".to_string(), |v| v.to_string());
    let thoughts: Vec<_> = journal
        .since(0)
        .thoughts
        .into_iter()
        .filter(|thought| thought.player == 0)
        .collect();
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
                thought.detail.chars().take(80).collect::<String>()
            )
        })
        .collect();
    let losses: Vec<String> = thoughts
        .iter()
        .filter(|thought| thought.headline.contains("is lost before"))
        .map(|thought| {
            format!(
                "t{} {}",
                thought.turn,
                thought.detail.chars().take(120).collect::<String>()
            )
        })
        .collect();
    let stage_lines = thoughts
        .iter()
        .filter(|thought| {
            thought.headline.starts_with("Siege of")
                && (thought.headline.contains(": stage")
                    || thought.headline.contains("muster")
                    || thought.headline.contains("falls back"))
        })
        .count();
    let mut out = format!(
        "ROW|{}|t{}|{}|seed {}|target {}|war {}|ram alive after war turn {}|ram beside {}|first hit {}|ram alive at first hit {}|walls down {}|taken {}|min hp {}|military lost {}|ram lost {}|strikes beside ram {}|holds {}|stage/muster lines {}|end t{}|releases {:?}|losses {:?}\n",
        job.board,
        job.turn,
        if job.on { "on" } else { "off" },
        job.seed,
        target_name,
        fmt(war),
        flag(ram_alive_after_war),
        fmt(ram_beside),
        fmt(first_hit),
        flag(ram_alive_at_hit),
        fmt(walls_down),
        fmt(taken),
        min_hp,
        lost,
        fmt(ram_lost),
        count("bodies strike the walls"),
        count("holds the assault on"),
        stage_lines,
        g.turn,
        releases,
        losses
    );
    out.push_str(&trace);
    if traced {
        let mut lines = 0;
        for thought in &thoughts {
            let headline = thought.headline.to_lowercase();
            if !NEEDLES.iter().any(|needle| headline.contains(needle)) {
                continue;
            }
            let detail: String = thought.detail.chars().take(220).collect();
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
            if lines >= 120 {
                break;
            }
        }
    }
    out
}

#[test]
fn scaffold_shield_replays() {
    let mut jobs = Vec::new();
    for (board, turn, turns) in [
        ("072641Z", 36, 24),
        ("040704Z", 38, 28),
        ("040704Z", 42, 28),
    ] {
        for on in [false, true] {
            for seed in 0..10 {
                jobs.push(Job {
                    board,
                    turn,
                    on,
                    seed,
                    turns,
                });
            }
        }
    }
    unpacked("040704Z");
    unpacked("072641Z");
    let genomes: Vec<(&str, Vec<String>)> = ["040704Z", "072641Z"]
        .into_iter()
        .map(|board| (board, genome(board)))
        .collect();
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(usize, String)>> = Mutex::new(Vec::new());
    std::thread::scope(|scope| {
        for _ in 0..4 {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::SeqCst);
                let Some(job) = jobs.get(index) else {
                    break;
                };
                let genome = &genomes
                    .iter()
                    .find(|(board, _)| *board == job.board)
                    .unwrap()
                    .1;
                let text =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| play(job, genome)))
                        .unwrap_or_else(|panic| {
                            let why = panic
                                .downcast_ref::<String>()
                                .cloned()
                                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                                .unwrap_or_default();
                            format!(
                                "ROW|{}|t{}|{}|seed {}|PANICKED {}\n",
                                job.board, job.turn, job.on, job.seed, why
                            )
                        });
                results.lock().unwrap().push((index, text));
            });
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort();
    let report: String = results.into_iter().map(|(_, text)| text).collect();
    panic!("SHIELD-REPORT-BEGIN\n{report}SHIELD-REPORT-END");
}
