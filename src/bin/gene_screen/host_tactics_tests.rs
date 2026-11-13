//! The rival chairs' tactics beside the live host's, with
//! `--rival-host-tactics` off and on, and one paired gene read under each.
//!
//! The lane's recipe (`--players 4 --width 60 --height 38 --city-states 6
//! --map pangaea --difficulty deity --rivals firaxis-faithful --handicap rivals
//! --rival-chairs 3 --rival-rung -2 --rival-pace --target-mix domination`),
//! with `--chair-tactics-census` on every arm so the off arm is read too. The
//! measured seat plays the deployment genome with `defend-force-holds-the-city`
//! off and on. A demonstration at small n: the lane runs the 60 seeds. The
//! first run's output is in `docs/HOST_ENEMY_MODEL.md`.
//!
//! `cargo test --features developer-tools --bin gene_screen host_tactics -- --ignored --nocapture`,
//! with `CIVVIS_HOST_TACTICS_WORLDS` for the seeds per arm.

use super::*;
use civvis::host_tactics::compare;
use std::sync::atomic::{AtomicUsize, Ordering};

const SEED: u64 = 63_100_000;
const GENE: &str = "defend-force-holds-the-city";

fn lane_profile(host: bool) -> Profile {
    Profile {
        target_mix: civvis::ai::player::parse_targets("domination").expect("a lane"),
        players: 4,
        width: 60,
        height: 38,
        turns: GameSpeed::Online.turn_limit(),
        city_states: 6,
        speed: GameSpeed::Online,
        map: MapScript::Pangaea,
        randomize_civs: true,
        victories: civvis::game::VictoryConditions::default(),
        field: Vec::new(),
        native_competitions: false,
        field_genes: Vec::new(),
        victory_mask: None,
        difficulty: "deity".to_string(),
        difficulty_rotate: None,
        rivals: true,
        faithful: true,
        trace: false,
        rival_rung: Some(RivalRung::Below(2)),
        rival_pace: true,
        seat_pace: false,
        rival_victory_pace: None,
        rival_late_science: None,
        rival_space_floor: None,
        rival_war_pace: None,
        rival_walls: None,
        rival_host_tactics: host,
        chair_tactics_census: true,
        seat_civ: None,
        rival_chairs: 3,
        handicap_rivals: true,
        journal_seeds: BTreeSet::new(),
        journal_base: String::new(),
    }
}

/// One arm's sums over its worlds.
#[derive(Default)]
struct Arm {
    tactics: BTreeMap<String, [f64; 2]>,
    killed: u32,
    lost: u32,
    cities_lost: usize,
    cities_taken: usize,
    /// Per world: `(seed, killed, lost, cities lost)`.
    worlds: Vec<(u64, u32, u32, usize)>,
}

fn worlds() -> usize {
    std::env::var("CIVVIS_HOST_TACTICS_WORLDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8)
}

#[test]
#[ignore = "whole Deity games: about 7 minutes on 4 cores; run with --ignored --nocapture"]
fn host_tactics_table_and_paired_demonstration() {
    let genes = gene_table();
    let gene = genes
        .iter()
        .position(|g| g.tag == GENE)
        .unwrap_or_else(|| panic!("{GENE} is not screened"));
    let base = faithful_genome(&genes);
    let n = worlds();
    // (switch, gene) per arm; every arm plays the same seeds and game indices,
    // so the chairs sit in the same places.
    let arms = [(false, false), (false, true), (true, false), (true, true)];
    let jobs: Vec<(usize, usize)> = (0..arms.len())
        .flat_map(|arm| (0..n).map(move |world| (arm, world)))
        .collect();
    let next = AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..std::thread::available_parallelism().map_or(2, |c| c.get()) {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(scope, || loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(arm, world)) = jobs.get(k) else {
                        break;
                    };
                    let (host, on) = arms[arm];
                    let mut genome = base.clone();
                    genome[gene] = on;
                    let genomes = vec![genome; 4];
                    let seed = SEED + world as u64;
                    let rows = play_game(&lane_profile(host), &genes, world, seed, &genomes);
                    let measured = rows
                        .into_iter()
                        .find(|row| row.kind == "game")
                        .expect("one measured seat");
                    results.lock().unwrap().push((arm, seed, measured));
                })
                .expect("a worker");
        }
    });
    let mut sums: Vec<Arm> = (0..arms.len()).map(|_| Arm::default()).collect();
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(arm, seed, _)| (*arm, *seed));
    for (arm, seed, row) in results {
        let sum = &mut sums[arm];
        for (key, [hits, rows]) in &row.chair_tactics {
            let cell = sum.tactics.entry(key.clone()).or_insert([0.0, 0.0]);
            cell[0] += hits;
            cell[1] += rows;
        }
        let (killed, lost) = (row.units_killed.unwrap_or(0), row.units_lost.unwrap_or(0));
        let cities_lost = row.cities_lost.unwrap_or(0);
        sum.killed += killed;
        sum.lost += lost;
        sum.cities_lost += cities_lost;
        sum.cities_taken += row.cities_taken.unwrap_or(0);
        sum.worlds.push((seed, killed, lost, cities_lost));
    }
    let mut report = vec![format!(
        "HOST-TACTICS-REPORT: lane recipe, {n} worlds per arm (seeds {SEED}..{}), {:.0}s",
        SEED + n as u64 - 1,
        started.elapsed().as_secs_f64()
    )];
    report.push(String::new());
    report.push(format!(
        "The rival chairs' tactics against the measured seat ({GENE} off), beside the live host's:"
    ));
    report.push(compare(&[
        ("chairs, switch off", &sums[0].tactics),
        ("chairs, switch on", &sums[2].tactics),
    ]));
    report.push("What the switch did (count, governed unit-turns):".to_string());
    for (key, [count, units]) in sums[2]
        .tactics
        .iter()
        .filter(|(k, _)| k.starts_with("gate"))
    {
        report.push(format!("  {key:<24} {count:>7} of {units}"));
    }
    report.push(String::new());
    report.push(format!(
        "Paired: the measured seat with {GENE} off and on (sums over the worlds):"
    ));
    for (host, label) in [(false, "switch off"), (true, "switch on ")] {
        let off = &sums[if host { 2 } else { 0 }];
        let on = &sums[if host { 3 } else { 1 }];
        let ratio = |arm: &Arm| arm.killed as f64 / arm.lost.max(1) as f64;
        report.push(format!(
            "  {label}: gene off killed {:>4} lost {:>4} (kill:loss {:.2}) cities lost {:>3} taken {:>3} | gene on killed {:>4} lost {:>4} (kill:loss {:.2}) cities lost {:>3} taken {:>3}",
            off.killed, off.lost, ratio(off), off.cities_lost, off.cities_taken,
            on.killed, on.lost, ratio(on), on.cities_lost, on.cities_taken,
        ));
        for (a, b) in off.worlds.iter().zip(&on.worlds) {
            report.push(format!(
                "    seed {}: off {}/{} lost {} city | on {}/{} lost {} city",
                a.0, a.1, a.2, a.3, b.1, b.2, b.3
            ));
        }
    }
    println!("{}", report.join("\n"));
}
