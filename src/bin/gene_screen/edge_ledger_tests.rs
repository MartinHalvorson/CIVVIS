//! ⭐ Does the simulator's lane recipe reproduce the strength gap the live
//! seat faces? The edge ledger's driver (`docs/EDGE_LEDGER.md`).
//!
//! Plays both lane recipes with `--rival-host-tactics` and the armed live
//! genome, under the recipe's own handicap and a few others, and prints the
//! simulator's edge metrics beside the live seat's. The live columns are
//! pasted from `tools/civ6_edge_ledger.py --metrics`, because CI cannot read
//! the live logs. Every arm plays the same seeds and game indices, so the
//! chairs sit in the same places and the settings read paired.
//!
//! `cargo test --profile ci --features developer-tools --bin gene_screen
//! edge_ledger_driver -- --ignored --nocapture`, with `CIVVIS_LIVE_GENOME`
//! (the armed set; default `~/.civvis-live-force-on`), `CIVVIS_EDGE_WORLDS`
//! (worlds per arm, default 8), `CIVVIS_EDGE_SETTINGS` (a comma list of
//! setting keys, default all) and `CIVVIS_EDGE_MINUTES` (stop starting worlds
//! after this many minutes, default no limit).

use super::edge_ledger::{metrics, render};
use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

const SEED: u64 = 64_300_000;

/// One lane recipe, as its command line spells it.
struct Lane {
    key: &'static str,
    players: usize,
    width: i32,
    height: i32,
    city_states: usize,
    map: MapScript,
    target: &'static str,
    seat_civ: Option<&'static str>,
    turns: Option<u32>,
    rival_chairs: usize,
    /// The live lane's metrics (`tools/civ6_edge_ledger.py --metrics`).
    live: &'static [(&'static str, f64)],
}

/// One handicap setting on top of the recipe.
struct Setting {
    key: &'static str,
    /// `--rival-rung -N`, or no flag (the rung's full yield bonus).
    rival_rung_below: Option<usize>,
    seat_pace: bool,
    rival_war_pace: Option<u32>,
}

const SETTINGS: [Setting; 3] = [
    Setting {
        key: "recipe",
        rival_rung_below: Some(2),
        seat_pace: false,
        rival_war_pace: None,
    },
    Setting {
        key: "no-rung",
        rival_rung_below: None,
        seat_pace: false,
        rival_war_pace: None,
    },
    Setting {
        key: "seat-pace",
        rival_rung_below: Some(2),
        seat_pace: true,
        rival_war_pace: None,
    },
];

/// `--players 4 --width 60 --height 38 --city-states 6 --map pangaea
/// --difficulty deity --rivals firaxis-faithful --handicap rivals
/// --rival-chairs 3 --rival-rung -2 --rival-pace --target-mix domination`.
const GRAN_COLOMBIA: Lane = Lane {
    key: "gran-colombia",
    players: 4,
    width: 60,
    height: 38,
    city_states: 6,
    map: MapScript::Pangaea,
    target: "domination",
    seat_civ: None,
    turns: None,
    rival_chairs: 3,
    live: LIVE_GRAN_COLOMBIA,
};

/// `--players 6 --width 74 --height 46 --city-states 9 --map continents
/// --difficulty deity --rivals firaxis-faithful --handicap rivals
/// --rival-chairs 5 --rival-rung -2 --rival-pace --target-mix score
/// --seat-civ Rome --turns 170`.
const ROME: Lane = Lane {
    key: "rome",
    players: 6,
    width: 74,
    height: 46,
    city_states: 9,
    map: MapScript::Continents,
    target: "score",
    seat_civ: Some("Rome"),
    turns: Some(170),
    rival_chairs: 5,
    live: LIVE_ROME,
};

fn profile(lane: &Lane, setting: &Setting) -> Profile {
    let speed = GameSpeed::Online;
    Profile {
        target_mix: civvis::ai::player::parse_targets(lane.target).expect("a lane"),
        players: lane.players,
        width: lane.width,
        height: lane.height,
        turns: lane.turns.unwrap_or(speed.turn_limit()),
        city_states: lane.city_states,
        speed,
        map: lane.map,
        randomize_civs: true,
        victories: civvis::game::VictoryConditions::default(),
        field: Vec::new(),
        native_competitions: true,
        field_genes: Vec::new(),
        victory_mask: None,
        difficulty: "deity".to_string(),
        difficulty_rotate: None,
        rivals: true,
        faithful: true,
        trace: false,
        rival_rung: setting.rival_rung_below.map(RivalRung::Below),
        rival_pace: true,
        seat_pace: setting.seat_pace,
        rival_victory_pace: None,
        rival_late_science: None,
        rival_space_floor: None,
        rival_war_pace: setting.rival_war_pace,
        rival_walls: None,
        rival_host_tactics: true,
        chair_tactics_census: false,
        seat_civ: lane.seat_civ.map(str::to_string),
        rival_chairs: lane.rival_chairs,
        handicap_rivals: true,
        journal_seeds: BTreeSet::new(),
        journal_base: String::new(),
    }
}

fn env_number(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The armed set, read and applied, never printed: how many tags it names
/// and how many of them this build does not carry.
fn armed_genome(genes: &mut [Gene]) -> (Vec<bool>, String) {
    let candidates: Vec<String> = std::env::var("CIVVIS_LIVE_GENOME")
        .ok()
        .into_iter()
        .chain(Some(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bin/gene_screen/edge_armed_set.txt").to_string()))
        .chain(
            std::env::var("HOME")
                .ok()
                .map(|home| format!("{home}/.civvis-live-force-on")),
        )
        .collect();
    let (path, text) = candidates
        .iter()
        .find_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| (path.clone(), text))
        })
        .unwrap_or_else(|| panic!("no armed set at any of {candidates:?}"));
    let on = live_genome_tags(&text).expect("the armed set parses");
    assert!(!on.is_empty(), "{path} names no tags");
    let unknown = apply_live_genome(genes, &on);
    let genome: Vec<bool> = genes.iter().map(|gene| gene.default_on).collect();
    let note = format!(
        "armed set {path}: {} tags, {} genes on in this build, {} tag(s) it does not carry",
        on.len(),
        genome.iter().filter(|on| **on).count(),
        unknown.len()
    );
    (genome, note)
}

fn cell(value: Option<f64>) -> String {
    match value {
        Some(v) if v.is_finite() => format!("{v:>9.3}"),
        _ => format!("{:>9}", "-"),
    }
}

#[test]
fn edge_ledger_driver() {
    let lanes = [&GRAN_COLOMBIA, &ROME];
    let chosen: Vec<&Setting> = match std::env::var("CIVVIS_EDGE_SETTINGS") {
        Ok(list) => SETTINGS
            .iter()
            .filter(|setting| list.split(',').any(|key| key.trim() == setting.key))
            .collect(),
        Err(_) => SETTINGS.iter().filter(|s| "recipe,no-rung".split(',').any(|k| k == s.key)).collect(),
    };
    let n = env_number("CIVVIS_EDGE_WORLDS", 8);
    let minutes = env_number("CIVVIS_EDGE_MINUTES", 15);
    let mut genes = gene_table();
    let (genome, note) = armed_genome(&mut genes);
    // World by world, the longer lane first, so a deadline leaves every arm
    // with a similar number of worlds.
    let mut jobs: Vec<(usize, usize, usize)> = Vec::new();
    for world in 0..n {
        for lane in (0..lanes.len()).rev() {
            for setting in 0..chosen.len() {
                jobs.push((lane, setting, world));
            }
        }
    }
    let next = AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    let started = Instant::now();
    let deadline = (minutes > 0).then(|| std::time::Duration::from_secs(60 * minutes as u64));
    std::thread::scope(|scope| {
        for _ in 0..std::thread::available_parallelism().map_or(2, |c| c.get()) {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(scope, || loop {
                    if deadline.is_some_and(|limit| started.elapsed() > limit) {
                        break;
                    }
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(lane, setting, world)) = jobs.get(k) else {
                        break;
                    };
                    let profile = profile(lanes[lane], chosen[setting]);
                    let genomes = vec![genome.clone(); profile.players];
                    let seed = SEED + world as u64;
                    let rows = play_game(&profile, &genes, world, seed, &genomes);
                    let measured = rows
                        .into_iter()
                        .find(|row| row.kind == "game")
                        .expect("one measured seat");
                    results
                        .lock()
                        .unwrap()
                        .push((lane, setting, world, measured));
                })
                .expect("a worker");
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(lane, setting, world, _)| (*lane, *setting, *world));
    let mut report = vec![
        format!(
            "EDGE-LEDGER-REPORT: {n} worlds an arm asked (seeds {SEED}..), {:.0}s, deadline {}",
            started.elapsed().as_secs_f64(),
            if minutes > 0 {
                format!("{minutes} min")
            } else {
                "none".to_string()
            }
        ),
        note,
    ];
    for (l, lane) in lanes.iter().enumerate() {
        let arms: Vec<Vec<&Row>> = (0..chosen.len())
            .map(|s| {
                results
                    .iter()
                    .filter(|(lane, setting, _, _)| *lane == l && *setting == s)
                    .map(|(_, _, _, row)| row)
                    .collect()
            })
            .collect();
        report.push(String::new());
        report.push(format!(
            "== {} lane: live beside the simulator, worlds per arm {:?}",
            lane.key,
            arms.iter().map(Vec::len).collect::<Vec<_>>()
        ));
        let sims: Vec<Vec<(String, Option<f64>)>> =
            arms.iter().map(|rows| metrics(rows.as_slice())).collect();
        let mut head = format!("{:<34} {:>9}", "metric", "live");
        for setting in &chosen {
            head.push_str(&format!(" {:>9}", setting.key));
        }
        report.push(head);
        for (name, live) in lane.live {
            let mut line = format!("{name:<34} {}", cell(Some(*live)));
            for sim in &sims {
                let value = sim
                    .iter()
                    .find(|(metric, _)| metric == name)
                    .and_then(|(_, value)| *value);
                line.push_str(&format!(" {}", cell(value)));
            }
            report.push(line);
        }
        for (s, rows) in arms.iter().enumerate() {
            report.push(String::new());
            report.push(format!(
                "-- {} lane, setting {}: per world (seed, units killed/lost, cities lost/taken, techs)",
                lane.key, chosen[s].key
            ));
            for row in rows {
                report.push(format!(
                    "   seed {} {}/{} lost {} took {} techs {} alive {}",
                    row.seed,
                    row.units_killed.unwrap_or(0),
                    row.units_lost.unwrap_or(0),
                    row.cities_lost.unwrap_or(0),
                    row.cities_taken.unwrap_or(0),
                    row.techs,
                    row.alive
                ));
            }
            if let Some(tables) = render(rows) {
                report.push(tables);
            }
        }
    }
    panic!("{}", report.join("\n"));
}

/// `tools/civ6_edge_ledger.py --since 20261112 --until 20261113T0420 --metrics`
/// (108 runs), the Gran Colombia lane (68 runs).
const LIVE_GRAN_COLOMBIA: &[(&str, f64)] = &[
    ("major_blows_per_game", 21.632),
    ("major_share_ge10", 0.706),
    ("major_death_per_blow_ge10", 0.553),
    ("major_death_per_blow_lt10", 0.150),
    ("major_gap_0_49", 8.704),
    ("major_gap_50_99", 17.256),
    ("major_gap_100_149", 22.850),
    ("major_gap_150", 19.576),
    ("struck_major_share_le_m10", 0.529),
    ("struck_major_share_ge10", 0.158),
    ("struck_major_dmg_le_m10", 8.873),
    ("struck_major_dmg_ge10", 41.940),
    ("struck_major_kill_per_blow_ge10", 0.631),
    ("unlocked_50", 0.076),
    ("material_50", 0.040),
    ("offer_50", 0.012),
    ("gain_50", 12.143),
    ("gold_50", 64.000),
    ("gpt_50", 8.301),
    ("ours_50", 16.562),
    ("rivals_50", 22.793),
    ("strength_gap_50", 6.246),
    ("unlocked_100", 0.293),
    ("material_100", 0.189),
    ("offer_100", 0.042),
    ("gain_100", 19.659),
    ("gold_100", 131.000),
    ("gpt_100", 21.855),
    ("ours_100", 28.802),
    ("rivals_100", 53.099),
    ("strength_gap_100", 24.277),
    ("unlocked_150", 0.513),
    ("material_150", 0.318),
    ("offer_150", 0.100),
    ("gain_150", 22.136),
    ("gold_150", 122.000),
    ("gpt_150", 12.359),
    ("ours_150", 43.988),
    ("rivals_150", 71.819),
    ("strength_gap_150", 28.110),
    ("kill_loss", 1.676),
    ("cities_lost_per_game", 1.571),
];

/// The same, the Rome lane (40 runs).
const LIVE_ROME: &[(&str, f64)] = &[
    ("major_blows_per_game", 11.275),
    ("major_share_ge10", 0.690),
    ("major_death_per_blow_ge10", 0.598),
    ("major_death_per_blow_lt10", 0.214),
    ("major_gap_0_49", 8.256),
    ("major_gap_50_99", 16.526),
    ("major_gap_100_149", 19.229),
    ("major_gap_150", 19.341),
    ("struck_major_share_le_m10", 0.588),
    ("struck_major_share_ge10", 0.092),
    ("struck_major_dmg_le_m10", 10.339),
    ("struck_major_dmg_ge10", 37.696),
    ("struck_major_kill_per_blow_ge10", 0.594),
    ("unlocked_50", 0.199),
    ("material_50", 0.081),
    ("offer_50", 0.026),
    ("gain_50", 17.460),
    ("gold_50", 79.000),
    ("gpt_50", 9.293),
    ("ours_50", 15.848),
    ("rivals_50", 21.450),
    ("strength_gap_50", 5.407),
    ("unlocked_100", 0.655),
    ("material_100", 0.496),
    ("offer_100", 0.176),
    ("gain_100", 18.463),
    ("gold_100", 142.000),
    ("gpt_100", 22.699),
    ("ours_100", 26.970),
    ("rivals_100", 45.783),
    ("strength_gap_100", 18.553),
    ("unlocked_150", 0.526),
    ("material_150", 0.257),
    ("offer_150", 0.035),
    ("gain_150", 28.301),
    ("gold_150", 207.000),
    ("gpt_150", 30.105),
    ("ours_150", 44.365),
    ("rivals_150", 66.682),
    ("strength_gap_150", 21.188),
    ("kill_loss", 1.842),
    ("cities_lost_per_game", 0.900),
];

/// The metric names here and in `tools/civ6_edge_ledger.py` are one list:
/// a live column with no simulated twin would print as dashes forever.
#[test]
fn the_live_columns_name_every_simulated_metric_in_order() {
    let mut row: Row = serde_json::from_str(
        r#"{"kind":"game","game":0,"seed":7,"seat":1,"genome":"1","win":false,"winner":0,
            "victory":"score","turn":250,"score":900,"score_share":0.3,"rank":2,"cities":8,
            "alive":true,"secs":50.0}"#,
    )
    .expect("a row parses");
    row.edge = Some(civvis::game::EdgeLedger::default());
    let names: Vec<String> = metrics(&[&row]).into_iter().map(|(name, _)| name).collect();
    for live in [LIVE_GRAN_COLOMBIA, LIVE_ROME] {
        let live: Vec<&str> = live.iter().map(|(name, _)| *name).collect();
        assert_eq!(live, names);
    }
}
