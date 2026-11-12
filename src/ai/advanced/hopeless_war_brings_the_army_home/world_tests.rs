//! `hopeless-war-brings-the-army-home` on whole worlds, paired.
//!
//! Ignored: it plays paired whole worlds. Run with
//! `cargo test --profile ci --lib army_home_worlds -- --ignored --nocapture`.
//!
//! The board is #5196's and #5211's: four players, 44x28, Online, 200 turns,
//! two city-states. Seat 0 plays Domination without the handicap, carrying
//! `deploy/live-force-on.txt` plus the live tactical genes, against rivals
//! that keep theirs. Every arm reads the forecast (`assault-forecast-is-read`,
//! no decision) and holds threatened cities (`defend-force-holds-the-city`):
//!
//! - **A** that base;
//! - **B** A + `hopeless-war-brings-the-army-home`;
//! - **C** B + `siege-goes-on-the-forecast` as merged.
//!
//! Every arm records the seat's journal (so every arm plays its unit pass
//! serially); the turns in force, the stand-downs and where the brought-home
//! units were are read off it and off the seat.

use super::HomeRole;
use crate::ai::advanced::last_orders::OrderStep;
use crate::ai::{AdvancedAi, Ai, VictoryTarget};
use crate::game::{Action, Game, GameOptions};
use crate::reasoning::{Journal, Level};
use std::collections::{BTreeMap, BTreeSet};
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
const FIRST_SEED: u64 = 0;
const SEEDS: u64 = 24;
const TURNS: u32 = 200;
/// A world past this many seconds stops where it stands (`stopped`).
const BUDGET_SECS: u64 = 16 * 60;
const JOBS: usize = 2;

#[derive(Clone, Copy)]
struct Arm {
    name: &'static str,
    home: bool,
    stand_down: bool,
}

const ARMS: [Arm; 3] = [
    Arm {
        name: "A base",
        home: false,
        stand_down: false,
    },
    Arm {
        name: "B home",
        home: true,
        stand_down: false,
    },
    Arm {
        name: "C home+down",
        home: true,
        stand_down: true,
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

#[derive(Debug, Default)]
struct WorldRow {
    ledger: crate::game::CombatLedger,
    cities_lost: usize,
    cities_taken: usize,
    turns: u32,
    secs: f64,
    stopped: bool,
    /// Turns the gene drew homes.
    in_force: usize,
    stand_downs: usize,
    /// Our tiles standing pillaged, summed over our turns, and at the end.
    pillaged_turns: usize,
    pillaged_end: usize,
    last_orders: BTreeMap<String, [u32; 2]>,
    /// Units last ordered home that died, by where they were when last seen:
    /// at a centre home, at a safe-tile home, on the way.
    home_deaths: [usize; 3],
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
    ai.enable_defend_force_holds_the_city();
    if arm.home {
        ai.enable_hopeless_war_brings_the_army_home();
    } else {
        ai.disable_hopeless_war_brings_the_army_home();
    }
    if arm.stand_down {
        ai.enable_siege_goes_on_the_forecast();
    } else {
        ai.disable_siege_goes_on_the_forecast();
    }
    ai
}

fn pillaged(g: &Game) -> usize {
    g.map
        .tiles
        .values()
        .filter(|tile| {
            tile.pillaged
                && tile
                    .owner_city
                    .and_then(|cid| g.cities.get(&cid))
                    .is_some_and(|city| city.owner == 0)
        })
        .count()
}

fn world(shape: Shape, seed: u64, arm: Arm) -> WorldRow {
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
    let mut owners: BTreeMap<u32, usize> = BTreeMap::new();
    let mut row = WorldRow::default();
    // Each of our soldiers' last step and home when last seen: (step, at a
    // centre home, at a safe home).
    let mut seen: BTreeMap<u32, (Option<OrderStep>, Option<HomeRole>, bool)> = BTreeMap::new();
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
            let now: BTreeMap<u32, usize> = g.cities.values().map(|c| (c.id, c.owner)).collect();
            for (cid, owner) in &owners {
                let after = now.get(cid).copied();
                if *owner == 0 && after != Some(0) {
                    row.cities_lost += 1;
                }
                if *owner != 0 && after == Some(0) {
                    row.cities_taken += 1;
                }
            }
            owners = now;
        }
        let pid = g.current;
        if pid == 0 {
            let alive: BTreeSet<u32> = g.player_unit_ids(0).into_iter().collect();
            for (uid, (step, role, at_home)) in std::mem::take(&mut seen) {
                if alive.contains(&uid) || step != Some(OrderStep::ArmyHome) {
                    continue;
                }
                let bin = match (role, at_home) {
                    (Some(HomeRole::Centre), true) => 0,
                    (Some(HomeRole::Safe), true) => 1,
                    _ => 2,
                };
                row.home_deaths[bin] += 1;
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
            if thought.headline.starts_with("The army comes home") {
                row.in_force += 1;
            } else if thought
                .headline
                .contains("stands down: the forecast reads it hopeless")
            {
                row.stand_downs += 1;
            }
        }
        row.pillaged_turns += pillaged(&g);
        let homes = &ais[0].hopeless_war_home.posts;
        for uid in g.player_unit_ids(0) {
            if g.rules.units[g.units[&uid].kind].class != "military" {
                continue;
            }
            let home = homes.get(&uid).copied();
            let at_home = home.is_some_and(|(at, _)| g.units[&uid].pos == at);
            seen.insert(
                uid,
                (
                    ais[0].last_order_step(uid),
                    home.map(|(_, role)| role),
                    at_home,
                ),
            );
        }
    }
    ais[0].settle_last_orders(&g, 0);
    row.last_orders = ais[0].strategy_census().last_orders.map();
    row.ledger = g.combat_ledger(0);
    row.pillaged_end = pillaged(&g);
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
// WIP-CI-RUN #[ignore = "plays the Emperor army-home worlds; run with --ignored --nocapture"]
fn army_home_worlds_emperor() {
    worlds(EMPEROR);
}

#[test]
// WIP-CI-RUN #[ignore = "plays the Immortal army-home worlds; run with --ignored --nocapture"]
fn army_home_worlds_immortal() {
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
    let seeds = FIRST_SEED..FIRST_SEED + SEEDS;
    println!(
        "\n== army-home worlds {}: {} (rival yields {}), seat 0 Domination unhandicapped with the live genome + defend-force-holds-the-city, 4p 44x28 Online, {TURNS} turns, seeds {FIRST_SEED}..{} ==",
        shape.name,
        shape.difficulty,
        shape.rival_yields.unwrap_or("the rung's"),
        FIRST_SEED + SEEDS - 1
    );
    for seed in seeds.clone() {
        for (arm, spec) in ARMS.iter().enumerate() {
            let r = row(seed, arm);
            println!(
                "ROW {} {seed} {} lost={} majors={} barbs={} killed={} clost={} ctaken={} force={} downs={} pill={} pillend={} hdeaths={:?} turns={} secs={:.0} stopped={}",
                shape.name,
                &spec.name[..1],
                r.ledger.units_lost,
                r.ledger.units_lost_to_majors,
                r.ledger.units_lost_to_barbarians,
                r.ledger.units_killed,
                r.cities_lost,
                r.cities_taken,
                r.in_force,
                r.stand_downs,
                r.pillaged_turns,
                r.pillaged_end,
                r.home_deaths,
                r.turns,
                r.secs,
                r.stopped
            );
        }
    }
    println!(
        "\n{:<12} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} | in force / stand-downs / brought-home deaths (centre, safe, on the way)",
        "arm", "lost", "to majors", "to barbs", "killed", "cities lost", "cities taken", "pillaged-turn", "pillaged end"
    );
    for (arm, spec) in ARMS.iter().enumerate() {
        let stat = |read: &dyn Fn(&WorldRow) -> f64| -> String {
            if arm == 0 {
                let total: f64 = seeds.clone().map(|s| read(row(s, 0))).sum();
                format!("{:.2}/world", total / SEEDS as f64)
            } else {
                let diffs: Vec<f64> = seeds
                    .clone()
                    .map(|s| read(row(s, arm)) - read(row(s, 0)))
                    .collect();
                let (mean, se) = mean_se(&diffs);
                format!("{mean:+.2}±{se:.2}")
            }
        };
        let force: usize = seeds.clone().map(|s| row(s, arm).in_force).sum();
        let downs: usize = seeds.clone().map(|s| row(s, arm).stand_downs).sum();
        let mut deaths = [0usize; 3];
        for s in seeds.clone() {
            for (total, n) in deaths.iter_mut().zip(row(s, arm).home_deaths) {
                *total += n;
            }
        }
        println!(
            "{:<12} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} {:>13} | {force} / {downs} / {deaths:?}",
            spec.name,
            stat(&|r| f64::from(r.ledger.units_lost)),
            stat(&|r| f64::from(r.ledger.units_lost_to_majors)),
            stat(&|r| f64::from(r.ledger.units_lost_to_barbarians)),
            stat(&|r| f64::from(r.ledger.units_killed)),
            stat(&|r| r.cities_lost as f64),
            stat(&|r| r.cities_taken as f64),
            stat(&|r| r.pillaged_turns as f64),
            stat(&|r| r.pillaged_end as f64),
        );
    }
    let stopped = rows.iter().filter(|r| r.stopped).count();
    println!("(rows after A are paired against A, mean ± se over seeds; stopped worlds {stopped})");

    let mut census: Vec<BTreeMap<String, [u32; 2]>> = vec![BTreeMap::new(); ARMS.len()];
    for (arm, total) in census.iter_mut().enumerate() {
        for s in seeds.clone() {
            for (step, pair) in &row(s, arm).last_orders {
                let slot = total.entry(step.clone()).or_default();
                slot[0] += pair[0];
                slot[1] += pair[1];
            }
        }
    }
    let mut steps: BTreeSet<String> = BTreeSet::new();
    for total in &census {
        steps.extend(total.keys().cloned());
    }
    println!("\nlast_orders (deaths by the step that last ordered the unit, within-two/beyond, summed over seeds):");
    println!(
        "{:<22} {}",
        "step",
        ARMS.iter()
            .map(|arm| format!("{:>14}", arm.name))
            .collect::<String>()
    );
    for step in &steps {
        let cells: String = census
            .iter()
            .map(|total| {
                let pair = total.get(step).copied().unwrap_or_default();
                let cell = format!("{}/{}", pair[0], pair[1]);
                format!("{cell:>14}")
            })
            .collect();
        println!("{step:<22} {cells}");
    }
}
