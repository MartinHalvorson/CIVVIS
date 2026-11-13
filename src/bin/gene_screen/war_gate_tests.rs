//! `declaration-needs-a-capture` off and on, paired by seed, on the two live
//! lane recipes with `--rival-host-tactics` and the live armed set
//! (`war_gate_live_force_on_20261113.txt`, a copy of `~/.civvis-live-force-on`
//! on 2026-11-13) as `--live-genome`:
//!
//! - Gran Colombia domination: `--players 4 --width 60 --height 38
//!   --city-states 6 --map pangaea --difficulty deity --rivals
//!   firaxis-faithful --handicap rivals --rival-chairs 3 --rival-rung -2
//!   --rival-pace --target-mix domination`;
//! - Rome / Score: `--players 6 --width 74 --height 46 --city-states 9 --map
//!   continents --difficulty deity --rivals firaxis-faithful --handicap
//!   rivals --rival-chairs 5 --rival-rung -2 --rival-pace --target-mix score
//!   --seat-civ Rome --turns 170`.
//!
//! The measured seat's wars come from the engine's war ledger (read every
//! turn, so none falls off the bounded `concluded_wars`); which opener
//! declared each one from the seat's journal.
//!
//! `cargo test --profile ci --features developer-tools --bin gene_screen war_gate -- --ignored --nocapture`,
//! with `CIVVIS_WAR_GATE_WORLDS` for the seeds per recipe (default 16).

use super::*;
use civvis::game::WarRecord;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

const GENE: &str = "declaration-needs-a-capture";
const LIVE_ARMS: &str = include_str!("war_gate_live_force_on_20261113.txt");
const SEEDS: [u64; 2] = [63_400_000, 63_500_000];
const WINDOW: u32 = 30;

fn worlds() -> usize {
    std::env::var("CIVVIS_WAR_GATE_WORLDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(16)
}

fn recipe(score: bool, journal_base: &str, seed: u64) -> Profile {
    let (players, width, height, city_states, map, chairs, mix, turns, civ) = if score {
        (
            6,
            74,
            46,
            9,
            MapScript::Continents,
            5,
            "score",
            170,
            Some("Rome".to_string()),
        )
    } else {
        (
            4,
            60,
            38,
            6,
            MapScript::Pangaea,
            3,
            "domination",
            GameSpeed::Online.turn_limit(),
            None,
        )
    };
    Profile {
        target_mix: civvis::ai::player::parse_targets(mix).expect("a lane"),
        players,
        width,
        height,
        turns,
        city_states,
        speed: GameSpeed::Online,
        map,
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
        rival_host_tactics: true,
        chair_tactics_census: false,
        seat_civ: civ,
        rival_chairs: chairs,
        handicap_rivals: true,
        journal_seeds: BTreeSet::from([seed]),
        journal_base: journal_base.to_string(),
    }
}

/// One measured seat's game, as the war ledger and the journal tell it.
#[derive(Clone, Debug, Default)]
struct World {
    seed: u64,
    declared: u32,
    /// Declarations by opener, from the journal line on the turn.
    openers: BTreeMap<String, u32>,
    took_a_city: u32,
    /// Cities the declared wars took within [`WINDOW`] turns, and lost.
    war_taken: u32,
    war_lost: u32,
    capitals_taken: u32,
    held_turns: u32,
    passed: u32,
    cities_taken: usize,
    cities_lost: usize,
    units_killed: u32,
    units_lost: u32,
    score: i64,
    rank: usize,
    share: f64,
    win: bool,
    secs: f64,
}

/// The opener a journal headline names, if it declares.
fn opener(headline: &str, detail: &str) -> Option<&'static str> {
    let h = headline;
    Some(
        if h.starts_with("Striking ") && h.ends_with("to condemn its spreaders") {
            "faith strike"
        } else if h.starts_with("Opening the faith war on")
            || h.starts_with("Opening a faith war on")
        {
            "faith war"
        } else if h.starts_with("Opening a religious interception") {
            "religious interception"
        } else if h.ends_with("to deny the space race") {
            "science denial"
        } else if h.starts_with("Opening the war on") {
            "launcher war"
        } else if h.starts_with("Launching the appointed attack") {
            "appointed attack"
        } else if h.starts_with("Front turns to") {
            "open-city front"
        } else if h.starts_with("Declaring war on") && detail.contains("score-seat-takes-what-pays")
        {
            "score take"
        } else if h.starts_with("Declaring war on") && h.contains(" for ") {
            "soft-city window"
        } else if h.starts_with("Declaring war on") {
            "main path"
        } else if h.starts_with("Opening a conquest") || h.starts_with("Raiding") {
            "conquest or raid"
        } else {
            return None;
        },
    )
}

fn play(score: bool, on: bool, world: usize, genes: &[Gene], base: &[bool], dir: &str) -> World {
    let seed = SEEDS[usize::from(score)] + world as u64;
    let gene = genes.iter().position(|g| g.tag == GENE).expect("screened");
    let mut genome = base.to_vec();
    genome[gene] = on;
    let players = if score { 6 } else { 4 };
    let genomes = vec![genome; players];
    let journal_base = format!(
        "{dir}/{}-{}",
        if score { "score" } else { "dom" },
        u8::from(on)
    );
    let profile = recipe(score, &journal_base, seed);
    // Every war record the board carried at any turn, latest copy kept.
    let book: Rc<RefCell<BTreeMap<(u32, usize, usize, u32), WarRecord>>> = Rc::default();
    let sink = Rc::clone(&book);
    GAME_OBSERVER.with(|o| {
        *o.borrow_mut() = Some(Box::new(move |g: &Game| {
            let mut book = sink.borrow_mut();
            for war in g.wars.values().chain(&g.concluded_wars) {
                book.insert(
                    (war.conflict, war.aggressor, war.defender, war.started),
                    war.clone(),
                );
            }
        }));
    });
    let rows = play_game(&profile, genes, world, seed, &genomes);
    GAME_OBSERVER.with(|o| *o.borrow_mut() = None);
    let row = rows
        .into_iter()
        .find(|row| row.kind == "game")
        .expect("one measured seat");
    let seat = row.seat;
    let lines =
        std::fs::read_to_string(format!("{journal_base}.journal.{seed}.txt")).unwrap_or_default();
    let mut out = World {
        seed,
        cities_taken: row.cities_taken.unwrap_or(0),
        cities_lost: row.cities_lost.unwrap_or(0),
        units_killed: row.units_killed.unwrap_or(0),
        units_lost: row.units_lost.unwrap_or(0),
        score: row.score,
        rank: row.rank,
        share: row.score_share,
        win: row.win,
        secs: row.secs,
        ..World::default()
    };
    let parsed: Vec<(u32, &str, &str)> = lines
        .lines()
        .filter_map(|line| {
            let (turn, rest) = line.strip_prefix('t')?.split_once(' ')?;
            let (who, rest) = rest.split_once(' ')?;
            if who != format!("p{seat}") {
                return None;
            }
            let (_, rest) = rest.split_once(' ')?;
            let (head, detail) = rest.split_once(" | ").unwrap_or((rest, ""));
            Some((turn.parse().ok()?, head, detail))
        })
        .collect();
    let mut held: BTreeSet<(u32, &str)> = BTreeSet::new();
    for (turn, head, _) in &parsed {
        if head.starts_with("Holding the war on") && head.ends_with("for a capture") {
            held.insert((*turn, head));
        } else if head.starts_with("The war on") && head.ends_with("has its capture") {
            out.passed += 1;
        }
    }
    out.held_turns = held.len() as u32;
    let book = book.borrow();
    // The declarations the measured seat made on a major: one per conflict.
    let mut conflicts: BTreeMap<u32, &WarRecord> = BTreeMap::new();
    for war in book.values() {
        if war.declarer == seat && war.target < players && war.target != seat {
            conflicts.entry(war.conflict).or_insert(war);
        }
    }
    for (conflict, war) in &conflicts {
        out.declared += 1;
        // The turn has one declaration, so its opener line names it.
        let label = parsed
            .iter()
            .filter(|(turn, _, _)| *turn == war.started)
            .find_map(|(_, head, detail)| opener(head, detail))
            .unwrap_or("unnamed");
        *out.openers.entry(label.to_string()).or_insert(0) += 1;
        let until = war.started + WINDOW;
        let mut taken = 0;
        for front in book.values().filter(|w| w.conflict == *conflict) {
            let theirs = if front.aggressor == seat {
                front.defender
            } else if front.defender == seat {
                front.aggressor
            } else {
                continue;
            };
            if let Some(losses) = front.losses.get(&theirs) {
                for city in losses.city_losses.iter().filter(|c| c.turn <= until) {
                    taken += 1;
                    if city.capital {
                        out.capitals_taken += 1;
                    }
                }
            }
            if let Some(losses) = front.losses.get(&seat) {
                out.war_lost += losses
                    .city_losses
                    .iter()
                    .filter(|c| c.turn <= until)
                    .count() as u32;
            }
        }
        out.war_taken += taken;
        if taken > 0 {
            out.took_a_city += 1;
        }
    }
    out
}

fn mean_se(xs: &[f64]) -> (f64, f64) {
    let n = xs.len().max(1) as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (mean, (var / n).sqrt())
}

#[test]
fn war_gate_paired_on_the_two_lanes() {
    let mut genes = gene_table();
    let on: BTreeSet<String> = live_genome_tags(LIVE_ARMS).expect("the armed set");
    let unknown = apply_live_genome(&mut genes, &on);
    let base: Vec<bool> = genes.iter().map(|gene| gene.default_on).collect();
    let gene = genes.iter().position(|g| g.tag == GENE).expect("screened");
    assert!(!base[gene], "the armed set does not carry {GENE}");
    let dir = std::env::temp_dir().join(format!("war-gate-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a journal dir");
    let dir = dir.to_string_lossy().to_string();
    let n = worlds();
    let jobs: Vec<(bool, bool, usize)> = [true, false]
        .into_iter()
        .flat_map(|score| {
            (0..n).flat_map(move |world| [false, true].map(move |on| (score, on, world)))
        })
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
                    let Some(&(score, on, world)) = jobs.get(k) else {
                        break;
                    };
                    let row = play(score, on, world, &genes, &base, &dir);
                    results.lock().unwrap().push((score, on, row));
                })
                .expect("a worker");
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(score, on, row)| (*score, *on, row.seed));
    let mut report = vec![format!(
        "WAR-GATE-REPORT: {GENE} off/on, {n} paired worlds per recipe, the live armed set ({} tags, {} this build does not carry), --rival-host-tactics, {:.0}s",
        on.len(),
        unknown.len(),
        started.elapsed().as_secs_f64()
    )];
    for score in [false, true] {
        let lane = if score {
            "Rome / Score (6p continents, t170)"
        } else {
            "Gran Colombia domination (4p pangaea)"
        };
        let arm = |on: bool| -> Vec<&World> {
            results
                .iter()
                .filter(|(s, o, _)| *s == score && *o == on)
                .map(|(_, _, row)| row)
                .collect()
        };
        let (off, onr) = (arm(false), arm(true));
        report.push(String::new());
        report.push(format!(
            "== {lane}, seeds {}..{}",
            SEEDS[usize::from(score)],
            SEEDS[usize::from(score)] + n as u64 - 1
        ));
        let sum =
            |rows: &[&World], f: &dyn Fn(&World) -> f64| rows.iter().map(|r| f(r)).sum::<f64>();
        let metrics: Vec<(&str, Box<dyn Fn(&World) -> f64>)> = vec![
            (
                "wars declared on a major",
                Box::new(|r| f64::from(r.declared)),
            ),
            (
                "  of them took a city <=30",
                Box::new(|r| f64::from(r.took_a_city)),
            ),
            (
                "  cities those wars took <=30",
                Box::new(|r| f64::from(r.war_taken)),
            ),
            (
                "  cities those wars lost <=30",
                Box::new(|r| f64::from(r.war_lost)),
            ),
            (
                "capitals taken (declared wars)",
                Box::new(|r| f64::from(r.capitals_taken)),
            ),
            (
                "gate: turns a war was held",
                Box::new(|r| f64::from(r.held_turns)),
            ),
            (
                "gate: declarations passed",
                Box::new(|r| f64::from(r.passed)),
            ),
            ("cities taken (row)", Box::new(|r| r.cities_taken as f64)),
            ("cities lost (row)", Box::new(|r| r.cities_lost as f64)),
            ("units killed", Box::new(|r| f64::from(r.units_killed))),
            ("units lost", Box::new(|r| f64::from(r.units_lost))),
            ("score", Box::new(|r| r.score as f64)),
            ("placement (1 = best)", Box::new(|r| r.rank as f64)),
            ("score share", Box::new(|r| r.share)),
            ("wins", Box::new(|r| f64::from(u8::from(r.win)))),
        ];
        report.push(format!(
            "  {:<34} {:>9} {:>9}   {:>18}",
            "per world (mean)", "off", "on", "on - off (paired)"
        ));
        for (name, f) in &metrics {
            let diffs: Vec<f64> = off.iter().zip(&onr).map(|(a, b)| f(b) - f(a)).collect();
            let (d, se) = mean_se(&diffs);
            report.push(format!(
                "  {name:<34} {:>9.2} {:>9.2}   {d:>+8.2} +/- {se:<6.2}",
                sum(&off, f.as_ref()) / off.len().max(1) as f64,
                sum(&onr, f.as_ref()) / onr.len().max(1) as f64,
            ));
        }
        for (label, rows) in [("off", &off), ("on", &onr)] {
            let mut openers: BTreeMap<&str, u32> = BTreeMap::new();
            for row in rows.iter() {
                for (k, v) in &row.openers {
                    *openers.entry(k.as_str()).or_insert(0) += v;
                }
            }
            report.push(format!("  declarations by opener, {label}: {openers:?}"));
        }
        for (a, b) in off.iter().zip(&onr) {
            report.push(format!(
                "    seed {}: off wars {} took {} | taken {} lost {} | killed {} lost {} | score {} rank {} || on wars {} took {} held {} | taken {} lost {} | killed {} lost {} | score {} rank {} | {:.0}s/{:.0}s",
                a.seed, a.declared, a.took_a_city, a.cities_taken, a.cities_lost, a.units_killed, a.units_lost, a.score, a.rank,
                b.declared, b.took_a_city, b.held_turns, b.cities_taken, b.cities_lost, b.units_killed, b.units_lost, b.score, b.rank,
                a.secs, b.secs
            ));
        }
    }
    let report = report.join("\n");
    println!("{report}");
    panic!("deliberate failure so CI prints the report:\n{report}");
}
