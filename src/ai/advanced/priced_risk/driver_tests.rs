//! `risk-is-priced`: the paired on/off evidence, printed.
//!
//! Ignored: it plays arena positions, skirmishes and whole worlds. Run with
//! `cargo test --profile ci --lib priced_risk_paired -- --ignored --nocapture`.
//!
//! - **Arena** (`docs/DOCTRINE_ARENA.md`): every doctrine position, each seed
//!   played twice with the roles swapped, the gene's seat against the same
//!   seat without it. Two seats: the stock `advanced`, and a Domination seat
//!   carrying the live tactical genes (`LIVE_TACTICS`). A control
//!   (`advanced` against itself) must read exactly zero.
//! - **Skirmish** (`battle_bench`): the control and the treatment.
//! - **Worlds**: a Domination seat at Deity that plays without the handicap
//!   against Domination rivals that keep it (so it is out-teched and at war),
//!   carrying the repository's live force-on genome plus the live tactical
//!   genes, the same seed with the gene off and on.

use crate::ai::{AdvancedAi, Ai, VictoryTarget};
use crate::doctrine::{matched_position, MatchedPosition, POSITIONS};
use crate::elo::seat_ai;
use crate::game::{Action, Game, GameOptions};
use crate::skirmish::{matched_skirmish, SkirmishSetup};
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
/// The positions the brief names, read at the full seed count.
const KEY_POSITIONS: &[&str] = &["the_relief", "the_reserve", "the_storming"];
const KEY_SEEDS: u64 = 48;
const OTHER_SEEDS: u64 = 12;
const CONTROL_SEEDS: u64 = 4;
const SKIRMISH_SEEDS: u64 = 40;
const WORLD_SEEDS: u64 = 10;
const WORLD_TURNS: u32 = 200;
/// A world past this many seconds stops where it stands (`stopped`).
const WORLD_BUDGET_SECS: u64 = 16 * 60;
const JOBS: usize = 3;

fn live_seat(base: &str) -> String {
    let mut seat = base.to_string();
    for tag in LIVE_TACTICS {
        seat.push('+');
        seat.push_str(tag);
    }
    seat
}

#[derive(Default)]
struct ArenaRow {
    seeds: usize,
    fires: usize,
    diffs: Vec<f64>,
    on_kills: usize,
    on_losses: usize,
    off_kills: usize,
    off_losses: usize,
    on_taken: usize,
    on_lost: usize,
    off_taken: usize,
    off_lost: usize,
}

fn mean_se(values: &[f64]) -> (f64, f64) {
    let n = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (mean, (var / n).sqrt())
}

fn arena(on: &str, off: &str, positions: &[(&'static str, u64)]) -> Vec<(String, ArenaRow)> {
    let jobs: Vec<(&'static str, u64)> = positions
        .iter()
        .flat_map(|(id, seeds)| (0..*seeds).map(move |seed| (*id, seed)))
        .collect();
    let played: Vec<(&'static str, MatchedPosition)> =
        crate::parallel::map(jobs.len(), JOBS, |i| {
            let (id, seed) = jobs[i];
            let spec = POSITIONS.iter().find(|p| p.id == id).expect("position");
            (id, matched_position(spec, 1000 + seed, on, off, &seat_ai))
        });
    let mut rows: BTreeMap<&'static str, ArenaRow> = BTreeMap::new();
    for (id, m) in played {
        if m.skipped {
            continue;
        }
        let row = rows.entry(id).or_default();
        row.seeds += 1;
        let diff = m.paired_difference();
        if diff != 0.0 || m.a.kills != m.b.kills || m.a.losses != m.b.losses {
            row.fires += 1;
        }
        row.diffs.push(diff);
        row.on_kills += m.a.kills;
        row.on_losses += m.a.losses;
        row.off_kills += m.b.kills;
        row.off_losses += m.b.losses;
        row.on_taken += m.a.cities_taken;
        row.on_lost += m.a.cities_lost;
        row.off_taken += m.b.cities_taken;
        row.off_lost += m.b.cities_lost;
    }
    rows.into_iter()
        .map(|(id, row)| (id.to_string(), row))
        .collect()
}

fn print_arena(title: &str, rows: &[(String, ArenaRow)]) {
    println!("\n== arena: {title} (on = first seat; each seed both roles) ==");
    println!(
        "{:<20} {:>5} {:>5} {:>9} {:>7} | {:>7} {:>7} {:>7} {:>7} | {:>9} {:>9}",
        "position",
        "seeds",
        "fires",
        "swing",
        "se",
        "on_lost",
        "off_lst",
        "on_kill",
        "off_kil",
        "on_cities",
        "off_cities"
    );
    let (mut all, mut ol, mut fl, mut ok, mut fk) = (Vec::new(), 0, 0, 0, 0);
    for (id, row) in rows {
        let (mean, se) = mean_se(&row.diffs);
        all.extend(row.diffs.iter().copied());
        ol += row.on_losses;
        fl += row.off_losses;
        ok += row.on_kills;
        fk += row.off_kills;
        println!(
            "{:<20} {:>5} {:>5} {:>+9.1} {:>7.1} | {:>7} {:>7} {:>7} {:>7} |    +{}/-{}      +{}/-{}",
            id,
            row.seeds,
            row.fires,
            mean,
            se,
            row.on_losses,
            row.off_losses,
            row.on_kills,
            row.off_kills,
            row.on_taken,
            row.on_lost,
            row.off_taken,
            row.off_lost,
        );
    }
    let (mean, se) = mean_se(&all);
    println!(
        "{:<20} {:>5} {:>5} {:>+9.1} {:>7.1} | {:>7} {:>7} {:>7} {:>7}",
        "ALL",
        all.len(),
        "",
        mean,
        se,
        ol,
        fl,
        ok,
        fk
    );
}

#[test]
// WIP-CI-RUN #[ignore = "plays the paired arena; run with --ignored --nocapture"]
fn priced_risk_paired_arena() {
    let started = Instant::now();
    let positions: Vec<(&'static str, u64)> = POSITIONS
        .iter()
        .map(|p| {
            let seeds = if KEY_POSITIONS.contains(&p.id) {
                KEY_SEEDS
            } else {
                OTHER_SEEDS
            };
            (p.id, seeds)
        })
        .collect();
    let control: Vec<(&'static str, u64)> = KEY_POSITIONS
        .iter()
        .map(|id| (*id, CONTROL_SEEDS))
        .collect();
    let rows = arena("advanced", "advanced", &control);
    print_arena("CONTROL advanced vs advanced", &rows);
    for (_, row) in &rows {
        assert!(
            row.diffs.iter().all(|d| *d == 0.0),
            "the control must read zero"
        );
        assert_eq!(row.fires, 0);
    }
    let rows = arena("advanced+risk-is-priced", "advanced", &positions);
    print_arena("advanced+risk-is-priced vs advanced", &rows);
    let live_off = live_seat("advanced_domination");
    let live_on = format!("{live_off}+risk-is-priced");
    let rows = arena(&live_on, &live_off, &positions);
    print_arena(&format!("{live_on} vs {live_off}"), &rows);

    // battle_bench, control then treatment.
    let setup = SkirmishSetup::default();
    for (on, off, seeds) in [
        ("advanced", "advanced", 4),
        ("advanced+risk-is-priced", "advanced", SKIRMISH_SEEDS),
        (live_on.as_str(), live_off.as_str(), SKIRMISH_SEEDS),
    ] {
        let played = crate::parallel::map(seeds as usize, JOBS, |i| {
            matched_skirmish(i as u64, &setup, on, off, &seat_ai)
        });
        let played: Vec<_> = played.into_iter().filter(|m| !m.skipped).collect();
        let diffs: Vec<f64> = played.iter().map(|m| m.paired_difference()).collect();
        let (mean, se) = mean_se(&diffs);
        let fires = diffs.iter().filter(|d| **d != 0.0).count();
        let on_lost: usize = played.iter().map(|m| m.a.losses).sum();
        let off_lost: usize = played.iter().map(|m| m.b.losses).sum();
        let on_kills: usize = played.iter().map(|m| m.a.kills).sum();
        let off_kills: usize = played.iter().map(|m| m.b.kills).sum();
        println!(
            "\n== battle_bench {on} vs {off}: {} seeds, paired swing {mean:+.2} ± {se:.2}, fires {fires}; lost {on_lost} vs {off_lost}, kills {on_kills} vs {off_kills} ==",
            played.len()
        );
        if on == off {
            assert!(
                diffs.iter().all(|d| *d == 0.0),
                "the control must read 0.00"
            );
        }
    }
    println!("\narena wall {:.0}s", started.elapsed().as_secs_f64());
}

/// One world's reading for the measured seat.
#[derive(Debug, Default)]
struct WorldRow {
    units_lost: u64,
    cities_lost: usize,
    cities_taken: usize,
    cities_end: usize,
    techs: usize,
    rival_techs: usize,
    war_turns: u32,
    turns: u32,
    secs: f64,
    stopped: bool,
}

fn measured_seat(on: bool) -> AdvancedAi {
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
    if on {
        ai.enable_risk_is_priced();
    } else {
        ai.disable_risk_is_priced();
    }
    ai
}

/// One board for the worlds: the rung, the rivals' yield rung, and what the
/// three rivals play for.
#[derive(Clone, Copy)]
struct Shape {
    name: &'static str,
    difficulty: &'static str,
    rival_yields: Option<&'static str>,
    rivals: [VictoryTarget; 3],
}

const SHAPES: [Shape; 2] = [
    // Every rival keeps Deity whole and rushes: the seat is out-teched from
    // the start and fights for its life.
    Shape {
        name: "deity-rush",
        difficulty: "deity",
        rival_yields: None,
        rivals: [VictoryTarget::Domination; 3],
    },
    // The live-calibrated shape (`gene_screen --rival-rung -2`): Immortal
    // combat and start units with King yields, two Domination rivals and a
    // Science one. The seat is out-teched and at war, and lives to fight.
    Shape {
        name: "immortal-king-yields",
        difficulty: "immortal",
        rival_yields: Some("king"),
        rivals: [
            VictoryTarget::Domination,
            VictoryTarget::Domination,
            VictoryTarget::Science,
        ],
    },
];

fn world(shape: Shape, seed: u64, on: bool) -> WorldRow {
    let started = Instant::now();
    let mut g = Game::new_with(GameOptions {
        speed: "online".to_string(),
        difficulty: shape.difficulty.to_string(),
        ai_yield_rung: shape.rival_yields.map(str::to_string),
        handicap_exempt: [0].into_iter().collect(),
        ..GameOptions::new(4, 44, 28, seed, WORLD_TURNS, 2)
    });
    let mut majors = 0;
    let mut ais: Vec<AdvancedAi> = (0..g.players.len())
        .map(|pid| {
            if pid == 0 {
                majors += 1;
                measured_seat(on)
            } else if g.players[pid].is_minor || g.players[pid].is_barbarian {
                AdvancedAi::new()
            } else {
                let target = shape.rivals[(majors - 1) % shape.rivals.len()];
                majors += 1;
                AdvancedAi::targeting(target)
            }
        })
        .collect();
    let mut owners: BTreeMap<u32, usize> = BTreeMap::new();
    let mut row = WorldRow::default();
    // `run_game`'s loop, with a wall-clock stop so a slow runner reports a
    // shorter world instead of losing the job.
    g.set_fog_memory(ais.iter().any(Ai::uses_player_observation));
    g.set_war_ledger(false);
    let mut observed = None;
    while g.winner.is_none() && g.turn <= g.max_turns {
        if observed != Some(g.turn) {
            if started.elapsed().as_secs() > WORLD_BUDGET_SECS {
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
            if g.players
                .iter()
                .any(|p| p.id != 0 && !p.is_barbarian && p.alive && g.is_at_war(0, p.id))
            {
                row.war_turns += 1;
            }
        }
        let pid = g.current;
        ais[pid].take_turn(&mut g, pid);
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
    }
    row.units_lost = g.players[0].unit_lifetimes.values().map(|s| s.units).sum();
    row.cities_end = g.player_city_ids(0).len();
    row.techs = g.players[0].techs.len();
    row.rival_techs = g
        .players
        .iter()
        .filter(|p| p.id != 0 && !p.is_minor && !p.is_barbarian)
        .map(|p| p.techs.len())
        .max()
        .unwrap_or(0);
    row.turns = g.turn;
    row.secs = started.elapsed().as_secs_f64();
    row
}

#[test]
// WIP-CI-RUN #[ignore = "plays paired whole worlds; run with --ignored --nocapture"]
fn priced_risk_paired_worlds() {
    for shape in SHAPES {
        worlds(shape);
    }
}

fn worlds(shape: Shape) {
    let jobs: Vec<(u64, bool)> = (0..WORLD_SEEDS)
        .flat_map(|seed| [(seed, false), (seed, true)])
        .collect();
    let rows = crate::parallel::map(jobs.len(), JOBS, |i| {
        let (seed, on) = jobs[i];
        (seed, on, world(shape, 9_100 + seed, on))
    });
    println!(
        "\n== worlds {}: {} (rival yields {}), measured seat unhandicapped Domination with the live genome, 4p 44x28 Online, {WORLD_TURNS} turns ==",
        shape.name,
        shape.difficulty,
        shape.rival_yields.unwrap_or("the rung's")
    );
    println!("seed arm  units_lost cities_lost cities_taken cities_end techs rival_techs war_turns turns secs stopped");
    let (mut d_lost, mut d_clost, mut d_taken) = (Vec::new(), Vec::new(), Vec::new());
    let mut by_seed: BTreeMap<u64, [Option<&WorldRow>; 2]> = BTreeMap::new();
    for (seed, on, row) in &rows {
        println!(
            "{seed:>4} {:<4} {:>10} {:>11} {:>12} {:>10} {:>5} {:>11} {:>9} {:>5} {:>4.0} {}",
            if *on { "on" } else { "off" },
            row.units_lost,
            row.cities_lost,
            row.cities_taken,
            row.cities_end,
            row.techs,
            row.rival_techs,
            row.war_turns,
            row.turns,
            row.secs,
            row.stopped
        );
        by_seed.entry(*seed).or_default()[usize::from(*on)] = Some(row);
    }
    for pair in by_seed.values() {
        if let [Some(off), Some(on)] = pair {
            d_lost.push(on.units_lost as f64 - off.units_lost as f64);
            d_clost.push(on.cities_lost as f64 - off.cities_lost as f64);
            d_taken.push(on.cities_taken as f64 - off.cities_taken as f64);
        }
    }
    let (a, sa) = mean_se(&d_lost);
    let (b, sb) = mean_se(&d_clost);
    let (c, sc) = mean_se(&d_taken);
    println!(
        "paired on-off over {} seeds: units lost {a:+.2} ± {sa:.2}, cities lost {b:+.2} ± {sb:.2}, cities taken {c:+.2} ± {sc:.2}",
        d_lost.len()
    );
}
