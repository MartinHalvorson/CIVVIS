//! `siege-goes-on-the-forecast` on whole worlds: does a hopeless siege kill
//! its units, and does standing it down save them without costing cities?
//!
//! Ignored: it plays paired whole worlds. Run with
//! `cargo test --profile ci --lib forecast_worlds -- --ignored --nocapture`.
//!
//! The board is `priced_risk::driver_tests`' (PR #5196): four players, 44x28,
//! Online, 200 turns; seat 0 plays Domination without the handicap and
//! carries `deploy/live-force-on.txt` plus the live tactical genes, against
//! rivals that keep theirs. The same seed in four arms:
//!
//! - **A** `assault-forecast-is-read` only: the forecast is read and
//!   journaled and changes no decision, so A plays as off;
//! - **B** `siege-goes-on-the-forecast` as merged;
//! - **C** B with the stand-down after one gathered reading under
//!   `HOPELESS_P` and without the "no one more unit lifts it" clause;
//! - **D** B's stand-down alone: the go and the hold-abort are off.
//!
//! C and D are the test-only `variant` switch, not genes. Every arm records
//! the seat's journal (so every arm plays its unit pass serially), and the
//! forecast lines, the stand-downs and the siege stages are read off it and
//! off the seat.

use super::variant::{self, Variant};
use crate::ai::advanced::last_orders::OrderStep;
use crate::ai::advanced::siege_train::SiegeStage;
use crate::ai::{AdvancedAi, Ai, VictoryTarget};
use crate::game::{Action, Game, GameOptions};
use crate::reasoning::{Journal, Level};
use crate::Pos;
use std::collections::BTreeMap;
use std::time::Instant;

/// The tactical genes the live seat carries beyond the repository's
/// force-on file (the operator's armed set, 2026-11-12).
const LIVE_TACTICS: &[&str] = &[
    "battle-planner-2",
    "safest-stand",
    "slots-survive-the-roll",
    "guns-stay-out-of-reach",
    "siege-train",
    "anvil",
];
const SEED_BASE: u64 = 9_100;
const FIRST_SEED: u64 = 16;
const SEEDS: u64 = 8;
const TURNS: u32 = 200;
/// A world past this many seconds stops where it stands (`stopped`).
const BUDGET_SECS: u64 = 16 * 60;
const JOBS: usize = 2;

#[derive(Clone, Copy)]
struct Arm {
    name: &'static str,
    goes: bool,
    variant: Variant,
}

const ARMS: [Arm; 4] = [
    Arm {
        name: "A read",
        goes: false,
        variant: Variant::MERGED,
    },
    Arm {
        name: "B merged",
        goes: true,
        variant: Variant::MERGED,
    },
    Arm {
        name: "C one+any",
        goes: true,
        variant: Variant {
            streak: Some(1),
            any_short_of: true,
            stand_down_only: false,
        },
    },
    Arm {
        name: "D down only",
        goes: true,
        variant: Variant {
            streak: None,
            any_short_of: false,
            stand_down_only: true,
        },
    },
];

#[derive(Clone, Copy)]
struct Shape {
    name: &'static str,
    difficulty: &'static str,
    rival_yields: Option<&'static str>,
    rivals: [VictoryTarget; 3],
}

const EMPEROR: Shape = Shape {
    name: "emperor-mixed",
    difficulty: "emperor",
    rival_yields: None,
    rivals: [
        VictoryTarget::Domination,
        VictoryTarget::Science,
        VictoryTarget::Culture,
    ],
};

const IMMORTAL: Shape = Shape {
    name: "immortal-king-yields",
    difficulty: "immortal",
    rival_yields: Some("king"),
    rivals: [
        VictoryTarget::Domination,
        VictoryTarget::Domination,
        VictoryTarget::Science,
    ],
};

/// One forecast line read off the journal: the turn, the city's tile, and
/// P within ten turns.
#[derive(Clone, Copy, Debug)]
struct Reading {
    turn: u32,
    city: Pos,
    p10: f64,
}

#[derive(Debug, Default)]
struct WorldRow {
    ledger: crate::game::CombatLedger,
    cities_lost: usize,
    cities_taken: usize,
    turns: u32,
    secs: f64,
    stopped: bool,
    stand_downs: usize,
    /// Our siege-turns by stage: Stage, Invest, Reduce, Take.
    stages: [usize; 4],
    readings: Vec<Reading>,
    /// Turns each tile became ours.
    taken_at: BTreeMap<Pos, Vec<u32>>,
    last_orders: BTreeMap<String, [u32; 2]>,
    /// Deaths last ordered by the siege doctrine, by the forecast reading of
    /// their siege when they were last seen: under 0.1, 0.1-0.4, over 0.4,
    /// no reading.
    siege_deaths: [usize; 4],
}

fn measured_seat(arm: Arm) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let armed = include_str!("../../../../deploy/live-force-on.txt");
    for tag in armed
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .chain(LIVE_TACTICS.iter().copied())
    {
        if let Some(gene) = crate::ai::gene(tag) {
            (gene.enable)(&mut ai);
        }
    }
    ai.enable_assault_forecast_is_read();
    if arm.goes {
        ai.enable_siege_goes_on_the_forecast();
    } else {
        ai.disable_siege_goes_on_the_forecast();
    }
    ai
}

/// The enemy city each of our units' siege is aimed at, by the shipped
/// force groups.
fn siege_city_of(ai: &AdvancedAi, g: &Game, uid: u32) -> Option<Pos> {
    ai.force_groups
        .iter()
        .find(|group| group.units.contains(&uid))
        .filter(|group| {
            g.city_at(group.objective).is_some_and(|cid| {
                let owner = g.cities[&cid].owner;
                owner != 0 && g.is_at_war(0, owner)
            })
        })
        .map(|group| group.objective)
}

fn world(shape: Shape, seed: u64, arm: Arm) -> WorldRow {
    variant::set(arm.variant);
    let started = Instant::now();
    let mut g = Game::new_with(GameOptions {
        speed: "online".to_string(),
        difficulty: shape.difficulty.to_string(),
        ai_yield_rung: shape.rival_yields.map(str::to_string),
        handicap_exempt: [0].into_iter().collect(),
        ..GameOptions::new(4, 44, 28, seed, TURNS, 2)
    });
    let mut majors = 0;
    let mut ais: Vec<AdvancedAi> = (0..g.players.len())
        .map(|pid| {
            if pid == 0 {
                majors += 1;
                measured_seat(arm)
            } else if g.players[pid].is_minor || g.players[pid].is_barbarian {
                AdvancedAi::new()
            } else {
                let target = shape.rivals[(majors - 1) % shape.rivals.len()];
                majors += 1;
                AdvancedAi::targeting(target)
            }
        })
        .collect();
    let journal = Journal::recording();
    journal.set_ceiling(Level::Decision);
    ais[0].attach_journal(journal.handle());
    let mut cursor = 0;
    let mut owners: BTreeMap<u32, (usize, Pos)> = BTreeMap::new();
    let mut row = WorldRow::default();
    // The latest reading by city tile, and each of our units' last step and
    // siege city when last seen.
    let mut latest: BTreeMap<Pos, f64> = BTreeMap::new();
    let mut seen: BTreeMap<u32, (Option<OrderStep>, Option<Pos>)> = BTreeMap::new();
    g.set_fog_memory(ais.iter().any(Ai::uses_player_observation));
    g.set_war_ledger(false);
    let mut observed = None;
    while g.winner.is_none() && g.turn <= g.max_turns {
        if observed != Some(g.turn) {
            if started.elapsed().as_secs() > BUDGET_SECS {
                row.stopped = true;
                break;
            }
            observed = Some(g.turn);
            let now: BTreeMap<u32, (usize, Pos)> = g
                .cities
                .values()
                .map(|c| (c.id, (c.owner, c.pos)))
                .collect();
            for (cid, (owner, pos)) in &owners {
                let after = now.get(cid).map(|(owner, _)| *owner);
                if *owner == 0 && after != Some(0) {
                    row.cities_lost += 1;
                }
                if *owner != 0 && after == Some(0) {
                    row.cities_taken += 1;
                    row.taken_at.entry(*pos).or_default().push(g.turn);
                }
            }
            owners = now;
        }
        let pid = g.current;
        if pid == 0 {
            // Our units gone since our last turn: died (or disbanded).
            let alive: std::collections::BTreeSet<u32> = g.player_unit_ids(0).into_iter().collect();
            for (uid, (step, city)) in std::mem::take(&mut seen) {
                if alive.contains(&uid) || step != Some(OrderStep::SiegeDoctrine) {
                    continue;
                }
                let bin = match city.and_then(|pos| latest.get(&pos).copied()) {
                    Some(p) if p < 0.1 => 0,
                    Some(p) if p <= 0.4 => 1,
                    Some(_) => 2,
                    None => 3,
                };
                row.siege_deaths[bin] += 1;
            }
        }
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if pid != 0 {
            continue;
        }
        let delta = journal.since(cursor);
        cursor = delta.cursor;
        for thought in delta.thoughts {
            if thought.headline.starts_with("Assault forecast for ") {
                let curve: Vec<f64> = thought
                    .detail
                    .split_once(": ")
                    .and_then(|(_, rest)| rest.split_once(';'))
                    .map(|(curve, _)| {
                        curve
                            .split('/')
                            .filter_map(|p| p.trim().parse::<f64>().ok())
                            .map(|p| p / 100.0)
                            .collect()
                    })
                    .unwrap_or_default();
                if let (Some(city), Some(p10), Some(p)) =
                    (thought.focus, curve.get(9).copied(), curve.last().copied())
                {
                    latest.insert(city, p);
                    row.readings.push(Reading {
                        turn: thought.turn,
                        city,
                        p10,
                    });
                }
            } else if thought
                .headline
                .contains("stands down: the forecast reads it hopeless")
            {
                row.stand_downs += 1;
            }
        }
        for siege in ais[0].sieges.values() {
            let slot = match siege.stage {
                SiegeStage::Stage => 0,
                SiegeStage::Invest => 1,
                SiegeStage::Reduce => 2,
                SiegeStage::Take => 3,
                SiegeStage::Hold => continue,
            };
            row.stages[slot] += 1;
        }
        for uid in g.player_unit_ids(0) {
            if g.rules.units[g.units[&uid].kind].class != "military" {
                continue;
            }
            let step = ais[0].last_order_step(uid);
            let city = siege_city_of(&ais[0], &g, uid);
            seen.insert(uid, (step, city));
        }
    }
    ais[0].settle_last_orders(&g, 0);
    row.last_orders = ais[0].strategy_census().last_orders.map();
    row.ledger = g.combat_ledger(0);
    row.turns = g.turn;
    row.secs = started.elapsed().as_secs_f64();
    row
}

fn mean_se(values: &[f64]) -> (f64, f64) {
    let n = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (mean, (var / n).sqrt())
}

#[test]
// WIP-CI-RUN #[ignore = "plays the Emperor forecast worlds; run with --ignored --nocapture"]
fn forecast_worlds_emperor() {
    worlds(EMPEROR);
}

#[test]
// WIP-CI-RUN #[ignore = "plays the Immortal forecast worlds; run with --ignored --nocapture"]
fn forecast_worlds_immortal() {
    worlds(IMMORTAL);
}

fn worlds(shape: Shape) {
    let jobs: Vec<(u64, usize)> = (FIRST_SEED..FIRST_SEED + SEEDS)
        .flat_map(|seed| (0..ARMS.len()).map(move |arm| (seed, arm)))
        .collect();
    let rows = crate::parallel::map(jobs.len(), JOBS, |i| {
        let (seed, arm) = jobs[i];
        world(shape, SEED_BASE + seed, ARMS[arm])
    });
    let row = |seed: u64, arm: usize| &rows[(seed - FIRST_SEED) as usize * ARMS.len() + arm];
    println!(
        "\n== forecast worlds {}: {} (rival yields {}), seat 0 Domination unhandicapped with the live genome, 4p 44x28 Online, {TURNS} turns, seeds {FIRST_SEED}..{} ==",
        shape.name,
        shape.difficulty,
        shape.rival_yields.unwrap_or("the rung's"),
        FIRST_SEED + SEEDS - 1
    );
    // One machine-readable line per seed and arm, for pooling rounds.
    for seed in FIRST_SEED..FIRST_SEED + SEEDS {
        for (arm, spec) in ARMS.iter().enumerate() {
            let r = row(seed, arm);
            println!(
                "ROW {} {seed} {} lost={} majors={} barbs={} killed={} clost={} ctaken={} downs={} stages={:?} sdeaths={:?} turns={} secs={:.0} stopped={}",
                shape.name,
                &spec.name[..1],
                r.ledger.units_lost,
                r.ledger.units_lost_to_majors,
                r.ledger.units_lost_to_barbarians,
                r.ledger.units_killed,
                r.cities_lost,
                r.cities_taken,
                r.stand_downs,
                r.stages,
                r.siege_deaths,
                r.turns,
                r.secs,
                r.stopped
            );
        }
    }
    println!(
        "\n{:<12} {:>14} {:>14} {:>14} {:>14} {:>14} {:>14} {:>10} | siege turns stage/invest/reduce/take",
        "arm", "lost", "lost to majors", "lost to barbs", "killed", "cities lost", "cities taken", "stand-downs"
    );
    for (arm, spec) in ARMS.iter().enumerate() {
        let stat = |read: &dyn Fn(&WorldRow) -> f64| -> String {
            if arm == 0 {
                let total: f64 = (FIRST_SEED..FIRST_SEED + SEEDS)
                    .map(|s| read(row(s, 0)))
                    .sum();
                format!("{:.2}/world", total / SEEDS as f64)
            } else {
                let diffs: Vec<f64> = (FIRST_SEED..FIRST_SEED + SEEDS)
                    .map(|s| read(row(s, arm)) - read(row(s, 0)))
                    .collect();
                let (mean, se) = mean_se(&diffs);
                format!("{mean:+.2}±{se:.2}")
            }
        };
        let downs: usize = (FIRST_SEED..FIRST_SEED + SEEDS)
            .map(|s| row(s, arm).stand_downs)
            .sum();
        let mut stages = [0usize; 4];
        for s in FIRST_SEED..FIRST_SEED + SEEDS {
            for (total, n) in stages.iter_mut().zip(row(s, arm).stages) {
                *total += n;
            }
        }
        println!(
            "{:<12} {:>14} {:>14} {:>14} {:>14} {:>14} {:>14} {:>10} | {stages:?}",
            spec.name,
            stat(&|r| f64::from(r.ledger.units_lost)),
            stat(&|r| f64::from(r.ledger.units_lost_to_majors)),
            stat(&|r| f64::from(r.ledger.units_lost_to_barbarians)),
            stat(&|r| f64::from(r.ledger.units_killed)),
            stat(&|r| r.cities_lost as f64),
            stat(&|r| r.cities_taken as f64),
            downs,
        );
    }
    let stopped = rows.iter().filter(|r| r.stopped).count();
    println!("(rows after A are paired against A, mean ± se over seeds; stopped worlds {stopped})");

    // last_orders: the six steps with the most deaths in A.
    let mut census: Vec<BTreeMap<String, [u32; 2]>> = vec![BTreeMap::new(); ARMS.len()];
    for (arm, total) in census.iter_mut().enumerate() {
        for s in FIRST_SEED..FIRST_SEED + SEEDS {
            for (step, pair) in &row(s, arm).last_orders {
                let slot = total.entry(step.clone()).or_default();
                slot[0] += pair[0];
                slot[1] += pair[1];
            }
        }
    }
    let mut top: Vec<(String, u32)> = census[0]
        .iter()
        .map(|(step, pair)| (step.clone(), pair[0] + pair[1]))
        .collect();
    top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    println!("\nlast_orders, top six steps in A (within-two/beyond, summed over seeds):");
    println!(
        "{:<20} {}",
        "step",
        ARMS.iter()
            .map(|arm| format!("{:>14}", arm.name))
            .collect::<String>()
    );
    for (step, _) in top.iter().take(6) {
        let cells: String = census
            .iter()
            .map(|total| {
                let pair = total.get(step).copied().unwrap_or_default();
                let cell = format!("{}/{}", pair[0], pair[1]);
                format!("{cell:>14}")
            })
            .collect();
        println!("{step:<20} {cells}");
    }

    // siege_doctrine deaths by their siege's reading when last seen.
    println!("\nsiege_doctrine deaths by the forecast reading of their siege (P within the horizon) when last seen: under 0.1 / 0.1-0.4 / over 0.4 / no reading");
    for (arm, spec) in ARMS.iter().enumerate() {
        let mut bins = [0usize; 4];
        for s in FIRST_SEED..FIRST_SEED + SEEDS {
            for (total, n) in bins.iter_mut().zip(row(s, arm).siege_deaths) {
                *total += n;
            }
        }
        println!("  {:<12} {bins:?}", spec.name);
    }

    // The forecast's calibration on these worlds: P within ten turns
    // against the city becoming ours within ten turns, by arm.
    println!(
        "\nforecast calibration: P(capture within 10) bin -> readings, taken within 10, mean P"
    );
    for (arm, spec) in ARMS.iter().enumerate() {
        let mut bins = [(0usize, 0usize, 0.0f64); 10];
        for s in FIRST_SEED..FIRST_SEED + SEEDS {
            let r = row(s, arm);
            for reading in &r.readings {
                let bin = ((reading.p10 * 10.0).floor() as usize).min(9);
                let taken = r.taken_at.get(&reading.city).is_some_and(|turns| {
                    turns
                        .iter()
                        .any(|t| *t > reading.turn && *t <= reading.turn + 10)
                });
                bins[bin].0 += 1;
                bins[bin].1 += usize::from(taken);
                bins[bin].2 += reading.p10;
            }
        }
        let cells: Vec<String> = bins
            .iter()
            .enumerate()
            .filter(|(_, (n, _, _))| *n > 0)
            .map(|(i, (n, taken, p))| {
                format!(
                    "{:.1}-{:.1}: {n} read, {taken} taken ({:.0}% vs mean P {:.0}%)",
                    i as f64 / 10.0,
                    (i + 1) as f64 / 10.0,
                    100.0 * *taken as f64 / *n as f64,
                    100.0 * p / *n as f64
                )
            })
            .collect();
        println!("  {}: {}", spec.name, cells.join("; "));
    }
}
