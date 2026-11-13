//! ⭐ `edge-unit-takes-the-soft-city` on whole worlds: the genome the live
//! seat plays, paired by seed, on the Rome lane recipe with a Science target
//! (`docs/EDGE_UNIT_TAKES_THE_SOFT_CITY.md`).
//!
//! `--players 6 --width 74 --height 46 --city-states 9 --map continents
//! --difficulty deity --rivals firaxis-faithful --handicap rivals
//! --rival-chairs 5 --rival-pace --rival-war-pace 100 --rival-host-tactics
//! --target-mix science --seat-civ Rome`, Online speed, no `--rival-rung`.
//!
//! The arms, all on the same seeds and game index, every arm of a seed in one
//! process so no pair crosses a shard or a build:
//! - `control`: a live run's genome header (`why.log` line 1);
//! - `gene`: the control with the gene on;
//! - `stack`: the control with the shipped early-conquest stack
//!   ([`STACK_ON`]) and without the stand-down that holds it shut
//!   ([`STACK_OFF`]);
//! - `gene-free-iron`: `gene` with the Legion's Iron waived on this world's
//!   rules (an upper bound: can the Legions take the city at all).
//!
//! An observer reads the Roman seat at the start of every turn: its Bronze and
//! Iron Working turns, Iron in reach at Bronze Working, its first Iron and
//! first Legion, Legions owned, the nearest rival major city to ours (the
//! live census's reading), cities, techs and Science at marks, cities it
//! founded, every city taken or lost, every war with a major and who
//! declared it, and the gene's `edge_unit:*` counters. The seat's journal
//! gives the gene's stages and holds and the early-conquest lines.
//!
//! `cargo test --profile ci --features developer-tools --bin gene_screen
//! edge_unit_takes_the_soft_city_driver -- --ignored --nocapture`, with
//! - `CIVVIS_EDGE_GENOME`: the control's genome file;
//! - `CIVVIS_EDGE_ARMS`: a comma list of arms (default all four);
//! - `CIVVIS_EDGE_TURNS` (default 150), `CIVVIS_EDGE_WORLDS` (default 16)
//!   seeds from `CIVVIS_EDGE_SEED`;
//! - `CIVVIS_EDGE_SHARD=k/K`: only the worlds whose index is k mod K;
//! - `CIVVIS_EDGE_OUT`: a file each finished world is appended to;
//! - `CIVVIS_EDGE_ROWS`: a comma list of such files; the report over them,
//!   and no games.

use super::*;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

const TAG: &str = "edge-unit-takes-the-soft-city";
const SEED: u64 = 52_670_000;
const CONTROL: &str = "control";
const GENE: &str = "gene";
const STACK: &str = "stack";
const FREE_IRON: &str = "gene-free-iron";
const ARMS: [&str; 4] = [CONTROL, GENE, STACK, FREE_IRON];
/// The shipped early-conquest stack the `stack` arm turns on.
/// `first-capital-strikes-the-open-window` needs
/// `first-capital-before-the-walls`, and both act only on a Domination seat.
const STACK_ON: [&str; 5] = [
    "early-conquest-opening",
    "conquest-takes-the-soft-city",
    "conquest-opening-stays-near",
    "first-capital-strikes-the-open-window",
    "first-capital-before-the-walls",
];
/// Off in `stack`: the live seat's stand-down holds every opening shut.
const STACK_OFF: [&str; 1] = ["early-conquest-stands-down"];
const SEAT_CIV: &str = "Rome";
const EDGE_UNIT: &str = "legion";
/// The live census's marks: the nearest rival city and the Legions owned.
const FIDELITY_MARKS: [u32; 4] = [40, 50, 60, 70];
/// Cities, techs, Science and military read at these turns.
const SAMPLE_MARKS: [u32; 6] = [50, 75, 100, 125, 150, 200];
/// Cities the seat founded, counted at these turns.
const FOUNDED_MARKS: [u32; 2] = [75, 100];
/// A Legion's strength, against the live census's defences.
const LEGION_STRENGTH: f64 = 40.0;
const LAUNCHES: [&str; 5] = [
    "launch_earth_satellite",
    "launch_moon_landing",
    "launch_mars_colony",
    "exoplanet_expedition",
    "lagrange_laser_station",
];
/// Journal events kept a world.
const EVENTS_KEPT: usize = 60;

/// The recipe's profile for `turns`, recording the measured seat's journal
/// for `seed` to `journal_base`.
fn profile(turns: u32, seed: u64, journal_base: String) -> Profile {
    let speed = GameSpeed::Online;
    Profile {
        target_mix: civvis::ai::player::parse_targets("science").expect("a lane"),
        players: 6,
        width: 74,
        height: 46,
        turns,
        city_states: 9,
        speed,
        map: MapScript::Continents,
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
        rival_rung: None,
        rival_pace: true,
        seat_pace: false,
        rival_victory_pace: None,
        rival_late_science: None,
        rival_space_floor: None,
        rival_war_pace: Some(100),
        rival_walls: None,
        rival_host_tactics: true,
        chair_tactics_census: false,
        seat_civ: Some(SEAT_CIV.to_string()),
        rival_chairs: 5,
        handicap_rivals: true,
        journal_seeds: BTreeSet::from([seed]),
        journal_base,
    }
}

/// The nearest rival major city to any of ours.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct Near {
    tiles: i32,
    defence: f64,
    walls: bool,
    pop: i32,
    explored: bool,
    capital: bool,
}

/// One reading of the seat.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct Sample {
    cities: usize,
    techs: usize,
    science: f64,
    military: f64,
}

/// What the observer saw of the Roman seat over a game.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Watch {
    seat: Option<usize>,
    bronze_working: Option<u32>,
    iron_working: Option<u32>,
    /// A visible Iron source inside our borders or within three tiles of a
    /// city of ours, on the turn Bronze Working was first seen known.
    iron_at_bronze: Option<bool>,
    first_iron: Option<u32>,
    first_legion: Option<u32>,
    legions: BTreeMap<u32, usize>,
    nearest: BTreeMap<u32, Near>,
    samples: BTreeMap<u32, Sample>,
    end: Option<(u32, Sample)>,
    founded: BTreeMap<u32, usize>,
    /// (turn seen, from, from a major).
    taken: Vec<(u32, usize, bool)>,
    /// (turn seen, to).
    lost: Vec<(u32, usize)>,
    /// (started, aggressor, defender) for every war with a major.
    wars: Vec<(u32, usize, usize)>,
    /// `edge_unit:<stage>` -> (turn first seen, final count).
    counters: BTreeMap<String, (u32, i64)>,
    #[serde(skip)]
    owners: BTreeMap<u32, usize>,
}

/// One arm of one world.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct World {
    arm: String,
    world: usize,
    seed: u64,
    turns: u32,
    fingerprint: u64,
    watch: Watch,
    /// The seat's journal: the gene's stages and the early-conquest lines.
    events: Vec<(u32, String)>,
    row: Row,
}

fn env_number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// One line straight to the process's stderr, past the harness's capture.
fn progress(line: &str) {
    use std::io::Write as _;
    let _ = writeln!(std::io::stderr().lock(), "{line}");
}

fn is_major(g: &Game, pid: usize) -> bool {
    g.players
        .get(pid)
        .is_some_and(|p| !p.is_minor && !p.is_barbarian && !p.is_free_city)
}

/// A visible Iron source inside the seat's borders or on unowned ground
/// within three tiles of one of its cities (the gene's reach).
fn iron_in_reach(g: &Game, seat: usize) -> bool {
    let explored = &g.players[seat].explored;
    g.player_city_ids(seat).into_iter().any(|cid| {
        let city = &g.cities[&cid];
        g.wdisk(city.pos, 3)
            .into_iter()
            .chain(city.owned_tiles.iter().copied())
            .any(|pos| {
                g.map.get(pos).is_some_and(|tile| {
                    tile.resource.is_some_and(|r| r.as_str() == "iron")
                        && explored.contains(&pos)
                        && tile
                            .owner_city
                            .and_then(|owner| g.cities.get(&owner))
                            .map(|owner| owner.owner)
                            .is_none_or(|owner| owner == seat)
                })
            })
    })
}

/// The nearest rival major city to any city of the seat.
fn nearest(g: &Game, seat: usize) -> Option<Near> {
    let ours: Vec<(i32, i32)> = g
        .player_city_ids(seat)
        .into_iter()
        .map(|cid| g.cities[&cid].pos)
        .collect();
    g.cities
        .values()
        .filter(|city| city.owner != seat && is_major(g, city.owner))
        .filter_map(|city| {
            let tiles = ours.iter().map(|pos| g.wdist(*pos, city.pos)).min()?;
            Some((tiles, city))
        })
        .min_by_key(|(tiles, city)| (*tiles, city.id))
        .map(|(tiles, city)| Near {
            tiles,
            defence: g.city_strength(city.id),
            walls: city.wall_hp > 0,
            pop: city.pop,
            explored: g.players[seat].explored.contains(&city.pos),
            capital: city.is_capital,
        })
}

fn sample(g: &Game, seat: usize) -> Sample {
    let read = civvis::ai::player::PaceSample::observe(g, seat);
    Sample {
        cities: read.cities,
        techs: read.techs,
        science: read.science,
        military: read.military,
    }
}

/// One look at the Roman seat at the start of a turn.
fn look(g: &Game, w: &mut Watch) {
    let Some(seat) = w.seat.or_else(|| {
        g.players
            .iter()
            .find(|p| p.civ == SEAT_CIV && !p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
    }) else {
        return;
    };
    w.seat = Some(seat);
    let turn = g.turn;
    let player = &g.players[seat];
    let knows = |tech: &str| player.techs.iter().any(|t| t.as_str() == tech);
    if w.bronze_working.is_none() && knows("bronze_working") {
        w.bronze_working = Some(turn);
        w.iron_at_bronze = Some(iron_in_reach(g, seat));
    }
    if w.iron_working.is_none() && knows("iron_working") {
        w.iron_working = Some(turn);
    }
    let stock = player
        .strategic_resources
        .iter()
        .find(|(name, _)| name.as_str() == "iron")
        .map_or(0.0, |(_, amount)| *amount);
    if w.first_iron.is_none() && (stock > 0.0 || g.strategic_resource_rate(seat, "iron") > 0.0) {
        w.first_iron = Some(turn);
    }
    let legions = g
        .units
        .values()
        .filter(|unit| unit.owner == seat && unit.kind.as_str() == EDGE_UNIT)
        .count();
    if legions > 0 && w.first_legion.is_none() {
        w.first_legion = Some(turn);
    }
    for mark in FIDELITY_MARKS {
        if turn >= mark && !w.legions.contains_key(&mark) {
            w.legions.insert(mark, legions);
            if let Some(near) = nearest(g, seat) {
                w.nearest.insert(mark, near);
            }
        }
    }
    let now = sample(g, seat);
    for mark in SAMPLE_MARKS {
        if turn >= mark && !w.samples.contains_key(&mark) {
            w.samples.insert(mark, now.clone());
        }
    }
    w.end = Some((turn, now));
    for mark in FOUNDED_MARKS {
        if turn >= mark && !w.founded.contains_key(&mark) {
            let founded = g
                .cities
                .values()
                .filter(|city| city.original_owner == seat)
                .count();
            w.founded.insert(mark, founded);
        }
    }
    for city in g.cities.values() {
        let before = w.owners.insert(city.id, city.owner);
        match before {
            Some(before) if before != city.owner && city.owner == seat => {
                w.taken.push((turn, before, is_major(g, before)));
            }
            Some(before) if before != city.owner && before == seat => {
                w.lost.push((turn, city.owner));
            }
            _ => {}
        }
    }
    for war in g.wars.values() {
        if war.aggressor != seat && war.defender != seat {
            continue;
        }
        let other = if war.aggressor == seat {
            war.defender
        } else {
            war.aggressor
        };
        let key = (war.started, war.aggressor, war.defender);
        if is_major(g, other) && !w.wars.contains(&key) {
            w.wars.push(key);
        }
    }
    for (key, value) in &g.players[seat].counters {
        if let Some(stage) = key.strip_prefix("edge_unit:") {
            let entry = w
                .counters
                .entry(stage.to_string())
                .or_insert((turn, *value));
            entry.1 = *value;
        }
    }
}

fn clip(text: &str, chars: usize) -> String {
    text.chars().take(chars).collect()
}

/// The seat's gene stages and early-conquest lines from its journal file
/// (removed once read), consecutive repeats dropped.
fn journal_events(path: &str, seat: usize) -> Vec<(u32, String)> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let _ = std::fs::remove_file(path);
    let conquest = [
        "Opening a conquest",
        "Declaring war on",
        "Releasing the conquest opening",
        "Holding the conquest force",
        "Not opening a conquest",
        "The conquest",
    ];
    let mut events: Vec<(u32, String)> = Vec::new();
    for line in text.lines() {
        let mut words = line.splitn(4, ' ');
        let (Some(turn), Some(player), Some(_topic), Some(rest)) =
            (words.next(), words.next(), words.next(), words.next())
        else {
            continue;
        };
        if player.strip_prefix('p').and_then(|p| p.parse().ok()) != Some(seat) {
            continue;
        }
        let Some(turn) = turn.strip_prefix('t').and_then(|t| t.parse::<u32>().ok()) else {
            continue;
        };
        let event = if let Some((_, detail)) = rest.split_once(&format!("{TAG}: ")) {
            clip(detail, 240)
        } else if conquest.iter().any(|head| rest.starts_with(head)) {
            format!("conquest: {}", clip(rest, 240))
        } else {
            continue;
        };
        if events.last().is_some_and(|(_, last)| *last == event) {
            continue;
        }
        events.push((turn, event));
    }
    if events.len() > EVENTS_KEPT {
        // The first fifty and the last ten.
        let tail = events.split_off(EVENTS_KEPT - 10);
        let skipped = tail.len() - 10;
        events.push((0, format!("... {skipped} more")));
        events.extend(tail.into_iter().skip(skipped));
    }
    events
}

/// The Legion trains and upgrades with no Iron on this world.
fn waive_legion_iron(g: &mut Game) {
    let spec = std::sync::Arc::make_mut(&mut g.rules)
        .units
        .get_mut(EDGE_UNIT)
        .expect("the Legion row");
    spec.requires_resource = None;
    spec.resource_cost = 0.0;
}

/// Play one arm of one world.
fn play(arm: &str, world: usize, seed: u64, turns: u32, genes: &[Gene], genome: &[bool]) -> World {
    let base = std::env::temp_dir()
        .join(format!("edge-unit-{arm}-{seed}-{}", std::process::id()))
        .to_string_lossy()
        .into_owned();
    let profile = profile(turns, seed, base.clone());
    let genomes = vec![genome.to_vec(); profile.players];
    let book: Rc<RefCell<(Watch, usize)>> = Rc::default();
    let sink = Rc::clone(&book);
    GAME_OBSERVER.with(|observer| {
        *observer.borrow_mut() = Some(Box::new(move |g: &Game| {
            let mut book = sink.borrow_mut();
            book.1 = g.log.len();
            look(g, &mut book.0);
        }));
    });
    let free_iron = arm == FREE_IRON;
    GAME_SETUP.with(|setup| {
        *setup.borrow_mut() = free_iron.then(|| Box::new(waive_legion_iron) as GameSetup);
    });
    let rows = play_game(&profile, genes, world, seed, &genomes);
    GAME_OBSERVER.with(|observer| *observer.borrow_mut() = None);
    GAME_SETUP.with(|setup| *setup.borrow_mut() = None);
    let row = rows
        .into_iter()
        .find(|row| row.kind == "game")
        .expect("one measured seat");
    let (watch, log_len) = std::mem::take(&mut *book.borrow_mut());
    let events = journal_events(&format!("{base}.journal.{seed}.txt"), row.seat);
    let mut bare = row.clone();
    bare.secs = 0.0;
    bare.genome = String::new();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(&bare)
        .expect("a row serializes")
        .hash(&mut hasher);
    log_len.hash(&mut hasher);
    World {
        arm: arm.to_string(),
        world,
        seed,
        turns,
        fingerprint: hasher.finish(),
        watch,
        events,
        row,
    }
}

fn mean_se(xs: &[f64]) -> (f64, f64) {
    let n = xs.len().max(1) as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (mean, (var / n).sqrt())
}

fn median(mut xs: Vec<f64>) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(f64::total_cmp);
    let mid = xs.len() / 2;
    Some(if xs.len().is_multiple_of(2) {
        (xs[mid - 1] + xs[mid]) / 2.0
    } else {
        xs[mid]
    })
}

fn quantile(mut xs: Vec<f64>, q: f64) -> Option<f64> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(f64::total_cmp);
    let at = ((xs.len() - 1) as f64 * q).round() as usize;
    Some(xs[at])
}

fn cell(x: Option<f64>) -> String {
    x.map_or("-".to_string(), |x| format!("{x:.0}"))
}

fn seat(w: &World) -> usize {
    w.watch.seat.unwrap_or(w.row.seat)
}

fn counter(w: &World, stage: &str) -> i64 {
    w.watch.counters.get(stage).map_or(0, |(_, n)| *n)
}

fn counter_turn(w: &World, stage: &str) -> Option<u32> {
    w.watch.counters.get(stage).map(|(turn, _)| *turn)
}

fn taken_by(w: &World, by: Option<u32>) -> f64 {
    w.watch
        .taken
        .iter()
        .filter(|(turn, _, _)| by.is_none_or(|by| *turn <= by))
        .count() as f64
}

fn lost_by(w: &World, by: Option<u32>) -> f64 {
    w.watch
        .lost
        .iter()
        .filter(|(turn, _)| by.is_none_or(|by| *turn <= by))
        .count() as f64
}

fn wars_we_declared(w: &World) -> Vec<(u32, usize)> {
    let me = seat(w);
    w.watch
        .wars
        .iter()
        .filter(|(_, aggressor, _)| *aggressor == me)
        .map(|(started, _, defender)| (*started, *defender))
        .collect()
}

fn wars_on_us_by(w: &World, by: u32) -> f64 {
    let me = seat(w);
    w.watch
        .wars
        .iter()
        .filter(|(started, aggressor, defender)| {
            *defender == me && *aggressor != me && *started <= by
        })
        .count() as f64
}

/// The game's winner was a rival the seat declared on.
fn winner_was_attacked(w: &World) -> bool {
    w.row.winner.is_some_and(|winner| {
        winner != seat(w)
            && wars_we_declared(w)
                .iter()
                .any(|(_, defender)| *defender == winner)
    })
}

fn launches(w: &World) -> f64 {
    w.row
        .project_turns
        .keys()
        .filter(|project| LAUNCHES.contains(&project.as_str()))
        .count() as f64
}

/// The paired table's columns: name, reading (`None` drops the pair from
/// that column), whether a 1.5-se mark is printed.
type Column = (&'static str, fn(&World) -> Option<f64>, bool);

const COLUMNS: &[Column] = &[
    (
        "edge window opened",
        |w| Some(f64::from(u8::from(counter(w, "opened") > 0))),
        true,
    ),
    (
        "edge declarations",
        |w| Some(counter(w, "declared") as f64),
        true,
    ),
    (
        "wars we declared on majors",
        |w| Some(wars_we_declared(w).len() as f64),
        true,
    ),
    ("wars on us by t100", |w| Some(wars_on_us_by(w, 100)), true),
    ("cities taken by t60", |w| Some(taken_by(w, Some(60))), true),
    (
        "cities taken by t100",
        |w| Some(taken_by(w, Some(100))),
        true,
    ),
    ("cities taken (end)", |w| Some(taken_by(w, None)), true),
    (
        "taken from majors (end)",
        |w| Some(w.watch.taken.iter().filter(|(_, _, major)| *major).count() as f64),
        true,
    ),
    ("cities lost by t100", |w| Some(lost_by(w, Some(100))), true),
    ("cities lost (end)", |w| Some(lost_by(w, None)), true),
    (
        "units lost",
        |w| Some(f64::from(w.row.units_lost.unwrap_or(0))),
        true,
    ),
    (
        "units killed",
        |w| Some(f64::from(w.row.units_killed.unwrap_or(0))),
        true,
    ),
    (
        "cities founded by t75",
        |w| w.watch.founded.get(&75).map(|n| *n as f64),
        true,
    ),
    (
        "cities founded by t100",
        |w| w.watch.founded.get(&100).map(|n| *n as f64),
        true,
    ),
    (
        "cities owned t50",
        |w| w.watch.samples.get(&50).map(|s| s.cities as f64),
        true,
    ),
    (
        "cities owned t100",
        |w| w.watch.samples.get(&100).map(|s| s.cities as f64),
        true,
    ),
    (
        "cities owned t150",
        |w| w.watch.samples.get(&150).map(|s| s.cities as f64),
        true,
    ),
    (
        "science t50",
        |w| w.watch.samples.get(&50).map(|s| s.science),
        true,
    ),
    (
        "science t75",
        |w| w.watch.samples.get(&75).map(|s| s.science),
        true,
    ),
    (
        "science t100",
        |w| w.watch.samples.get(&100).map(|s| s.science),
        true,
    ),
    (
        "science t125",
        |w| w.watch.samples.get(&125).map(|s| s.science),
        true,
    ),
    (
        "science t150",
        |w| w.watch.samples.get(&150).map(|s| s.science),
        true,
    ),
    (
        "techs t100",
        |w| w.watch.samples.get(&100).map(|s| s.techs as f64),
        true,
    ),
    (
        "techs t125",
        |w| w.watch.samples.get(&125).map(|s| s.techs as f64),
        true,
    ),
    (
        "military t75",
        |w| w.watch.samples.get(&75).map(|s| s.military),
        true,
    ),
    (
        "science (end)",
        |w| w.watch.end.as_ref().map(|(_, s)| s.science),
        true,
    ),
    (
        "techs (end)",
        |w| w.watch.end.as_ref().map(|(_, s)| s.techs as f64),
        true,
    ),
    (
        "cities (end)",
        |w| w.watch.end.as_ref().map(|(_, s)| s.cities as f64),
        true,
    ),
    (
        "Legions t40",
        |w| w.watch.legions.get(&40).map(|n| *n as f64),
        true,
    ),
    (
        "Legions t50",
        |w| w.watch.legions.get(&50).map(|n| *n as f64),
        true,
    ),
    (
        "Legions t60",
        |w| w.watch.legions.get(&60).map(|n| *n as f64),
        true,
    ),
    ("score", |w| Some(w.row.score as f64), true),
    ("last turn", |w| Some(f64::from(w.row.turn)), false),
    ("seat won", |w| Some(f64::from(u8::from(w.row.win))), true),
    ("launches landed", |w| Some(launches(w)), true),
    (
        "winner was a rival we hit",
        |w| Some(f64::from(u8::from(winner_was_attacked(w)))),
        true,
    ),
];

/// The live census's reading, from the arm's own worlds.
fn fidelity(side: &[&World]) -> Vec<String> {
    let mut out = Vec::new();
    for mark in FIDELITY_MARKS {
        let near: Vec<&Near> = side
            .iter()
            .filter_map(|w| w.watch.nearest.get(&mark))
            .collect();
        if near.is_empty() {
            continue;
        }
        let defence: Vec<f64> = near.iter().map(|n| n.defence).collect();
        let gap: Vec<f64> = defence.iter().map(|d| LEGION_STRENGTH - d).collect();
        let share = |hit: usize| 100.0 * hit as f64 / near.len() as f64;
        let legions: Vec<usize> = side
            .iter()
            .filter_map(|w| w.watch.legions.get(&mark).copied())
            .collect();
        let iron_working = side
            .iter()
            .filter(|w| w.watch.iron_working.is_some_and(|t| t <= mark))
            .count();
        out.push(format!(
            "   t{mark} n={}: nearest rival major city tiles {} defence {} (p25 {}, p75 {}) walls up {:.0}% pop {} capital {:.0}% explored {:.0}% | Legion minus defence median {} at 0+ {:.0}% at +5 {:.0}% at +8 {:.0}% | Iron Working {}/{} | Legions any {}/{} 3+ {}/{}",
            near.len(),
            cell(median(near.iter().map(|n| f64::from(n.tiles)).collect())),
            cell(median(defence.clone())),
            cell(quantile(defence.clone(), 0.25)),
            cell(quantile(defence.clone(), 0.75)),
            share(near.iter().filter(|n| n.walls).count()),
            cell(median(near.iter().map(|n| f64::from(n.pop)).collect())),
            share(near.iter().filter(|n| n.capital).count()),
            share(near.iter().filter(|n| n.explored).count()),
            cell(median(gap.clone())),
            share(gap.iter().filter(|g| **g >= 0.0).count()),
            share(gap.iter().filter(|g| **g >= 5.0).count()),
            share(gap.iter().filter(|g| **g >= 8.0).count()),
            iron_working,
            side.len(),
            legions.iter().filter(|n| **n > 0).count(),
            legions.len(),
            legions.iter().filter(|n| **n >= 3).count(),
            legions.len(),
        ));
    }
    out
}

/// What one arm did, over its own worlds.
fn describe(arm: &str, side: &[&World]) -> Vec<String> {
    let n = side.len();
    let turns = |f: fn(&World) -> Option<u32>| -> (usize, String) {
        let xs: Vec<f64> = side.iter().filter_map(|w| f(w).map(f64::from)).collect();
        (xs.len(), cell(median(xs)))
    };
    let (bw_n, bw) = turns(|w| w.watch.bronze_working);
    let (iw_n, iw) = turns(|w| w.watch.iron_working);
    let (fi_n, fi) = turns(|w| w.watch.first_iron);
    let (fl_n, fl) = turns(|w| w.watch.first_legion);
    let (op_n, op) = turns(|w| counter_turn(w, "opened"));
    let (de_n, de) = turns(|w| counter_turn(w, "declared"));
    let reach = side
        .iter()
        .filter(|w| w.watch.iron_at_bronze == Some(true))
        .count();
    let at_bronze = side
        .iter()
        .filter(|w| w.watch.iron_at_bronze.is_some())
        .count();
    let sum = |stage: &str| side.iter().map(|w| counter(w, stage)).sum::<i64>();
    let worlds_with = |stage: &str| side.iter().filter(|w| counter(w, stage) > 0).count();
    let mut victories: BTreeMap<String, usize> = BTreeMap::new();
    for w in side {
        let who = match w.row.winner {
            Some(winner) if winner == seat(w) => "seat".to_string(),
            Some(_) => "rival".to_string(),
            None => "none".to_string(),
        };
        *victories
            .entry(format!("{who} {}", w.row.victory))
            .or_default() += 1;
    }
    vec![
        format!(
            "   {arm:<14} research: Bronze Working in {bw_n}/{n} (median t{bw}), Iron in reach at Bronze Working {reach}/{at_bronze}, Iron Working {iw_n}/{n} (t{iw}), first Iron {fi_n}/{n} (t{fi}), first Legion {fl_n}/{n} (t{fl})"
        ),
        format!(
            "   {arm:<14} gene: window opened {op_n}/{n} (median t{op}), research goal set {}/{n}, waited for want of a source {}/{n}, tiles bought {}, sources improved {}, upgrades {}, units trained {}, declared {de_n}/{n} (t{de}), captured {}, went on {}, peace asked {}, closed {}",
            worlds_with("goal"),
            worlds_with("no_source"),
            sum("tile_bought"),
            sum("improved"),
            sum("upgraded"),
            sum("trained"),
            sum("captured"),
            sum("continued"),
            sum("peace"),
            sum("closed"),
        ),
        format!(
            "   {arm:<14} holds (worlds): forecast {}, shipped opening refused {}, another war {}, no breaker {}, declaration refused {}",
            worlds_with("held_forecast"),
            worlds_with("held_opening"),
            worlds_with("held_war"),
            worlds_with("held_breaker"),
            worlds_with("held_refused"),
        ),
        format!(
            "   {arm:<14} endings: {} | last turn median {} | winner a rival we declared on {}/{n}",
            victories
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", "),
            cell(median(side.iter().map(|w| f64::from(w.row.turn)).collect())),
            side.iter().filter(|w| winner_was_attacked(w)).count(),
        ),
    ]
}

fn fmt_cell(m: f64, se: f64) -> String {
    if m.abs() < 1.0 && se < 1.0 {
        format!("{m:.3}±{se:.3}")
    } else {
        format!("{m:.2}±{se:.2}")
    }
}

/// The report over every world given: per arm, what it did; the control's
/// fidelity to the live census; each arm minus the control, paired by seed;
/// and every pair in one line.
fn report(worlds: &[World], note: &str) -> String {
    let mut out = vec![note.to_string()];
    let of = |arm: &str| -> BTreeMap<u64, &World> {
        worlds
            .iter()
            .filter(|w| w.arm == arm)
            .map(|w| (w.seed, w))
            .collect()
    };
    let control = of(CONTROL);
    out.push(String::new());
    out.push(format!(
        "== fidelity (control, {} worlds): the nearest rival major city to ours, as the live census reads it",
        control.len()
    ));
    out.extend(fidelity(&control.values().copied().collect::<Vec<_>>()));
    out.push(String::new());
    out.push("== what each arm did, over its own worlds".to_string());
    for arm in ARMS {
        let side: Vec<&World> = of(arm).values().copied().collect();
        if !side.is_empty() {
            out.extend(describe(arm, &side));
        }
    }
    for arm in ARMS {
        if arm == CONTROL {
            continue;
        }
        let other = of(arm);
        let pairs: Vec<(&World, &World)> = control
            .iter()
            .filter_map(|(seed, c)| other.get(seed).map(|o| (*c, *o)))
            .collect();
        if pairs.is_empty() {
            continue;
        }
        let diverged = pairs
            .iter()
            .filter(|(c, o)| c.fingerprint != o.fingerprint)
            .count();
        out.push(String::new());
        out.push(format!(
            "== {arm} minus control, paired by seed: {} pairs, seeds {}..{}, {diverged} diverged (mean ± se a world; * = |d| >= 1.5 se; n = pairs with both readings)",
            pairs.len(),
            pairs.first().map_or(0, |(c, _)| c.seed),
            pairs.last().map_or(0, |(c, _)| c.seed),
        ));
        out.push(format!(
            "   {:<28} {:>4} {:>16} {:>16} {:>16}",
            "", "n", "control", arm, "arm - control"
        ));
        for (name, read, mark) in COLUMNS {
            let both: Vec<(f64, f64)> = pairs
                .iter()
                .filter_map(|(c, o)| Some((read(c)?, read(o)?)))
                .collect();
            if both.is_empty() {
                continue;
            }
            let (cm, cse) = mean_se(&both.iter().map(|(c, _)| *c).collect::<Vec<_>>());
            let (om, ose) = mean_se(&both.iter().map(|(_, o)| *o).collect::<Vec<_>>());
            let (d, dse) = mean_se(&both.iter().map(|(c, o)| o - c).collect::<Vec<_>>());
            let star = if *mark && dse > 0.0 && (d / dse).abs() >= 1.5 {
                "*"
            } else {
                " "
            };
            let diff = if d.abs() < 1.0 && dse < 1.0 {
                format!("{d:+.3}±{dse:.3}")
            } else {
                format!("{d:+.2}±{dse:.2}")
            };
            out.push(format!(
                "   {name:<28} {:>4} {:>16} {:>16} {:>15}{star}",
                both.len(),
                fmt_cell(cm, cse),
                fmt_cell(om, ose),
                diff
            ));
        }
        out.push(format!(
            "== {arm}: per pair (seed: taken/lost/units lost/science t100/score/last turn/winner victory | the same for {arm}; = identical game)"
        ));
        let cells = |w: &World| {
            format!(
                "{}/{}/{}/{}/{}/{}/{} {}",
                taken_by(w, None),
                lost_by(w, None),
                w.row.units_lost.unwrap_or(0),
                cell(w.watch.samples.get(&100).map(|s| s.science)),
                w.row.score,
                w.row.turn,
                w.row
                    .winner
                    .map_or("-".to_string(), |winner| if winner == seat(w) {
                        "seat".to_string()
                    } else {
                        format!("p{winner}")
                    }),
                w.row.victory,
            )
        };
        for (c, o) in &pairs {
            out.push(format!(
                "   {}: {} | {}{}",
                c.seed,
                cells(c),
                cells(o),
                if c.fingerprint == o.fingerprint {
                    " ="
                } else {
                    ""
                }
            ));
            for (turn, event) in o.events.iter().take(12) {
                out.push(format!("      t{turn} {event}"));
            }
        }
    }
    out.iter()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
#[ignore = "whole Deity games, four arms paired by seed: run with --ignored --nocapture"]
fn edge_unit_takes_the_soft_city_driver() {
    if let Ok(files) = std::env::var("CIVVIS_EDGE_ROWS") {
        let mut worlds: Vec<World> = Vec::new();
        for path in files.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                worlds.push(serde_json::from_str(line).expect("a world parses"));
            }
        }
        worlds.sort_by_key(|w| (w.world, w.arm.clone()));
        let note = format!(
            "EDGE-UNIT-REPORT: {} worlds read from {} file(s)",
            worlds.len(),
            files.split(',').filter(|p| !p.trim().is_empty()).count()
        );
        println!("{}", report(&worlds, &note));
        return;
    }
    let turns = env_number("CIVVIS_EDGE_TURNS", 150) as u32;
    let n = env_number("CIVVIS_EDGE_WORLDS", 16) as usize;
    let first_seed = env_number("CIVVIS_EDGE_SEED", SEED);
    let (shard, shards) = std::env::var("CIVVIS_EDGE_SHARD")
        .ok()
        .and_then(|text| {
            let (k, of) = text.split_once('/')?;
            Some((
                k.trim().parse::<usize>().ok()?,
                of.trim().parse::<usize>().ok()?,
            ))
        })
        .filter(|(k, of)| *of > 0 && k < of)
        .unwrap_or((0, 1));
    let arms: Vec<&str> = match std::env::var("CIVVIS_EDGE_ARMS") {
        Ok(list) => ARMS
            .iter()
            .copied()
            .filter(|arm| list.split(',').any(|named| named.trim() == *arm))
            .collect(),
        Err(_) => ARMS.to_vec(),
    };
    assert!(
        !arms.is_empty(),
        "CIVVIS_EDGE_ARMS names no arm of {ARMS:?}"
    );
    let path =
        std::env::var("CIVVIS_EDGE_GENOME").expect("CIVVIS_EDGE_GENOME names a live genome header");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let tags = live_genome_tags(&text).expect("the genome file parses");
    assert!(!tags.is_empty(), "{path} names no tags");
    let mut genes = gene_table();
    let unknown = apply_live_genome(&mut genes, &tags);
    let control: Vec<bool> = genes.iter().map(|gene| gene.default_on).collect();
    let index = |tag: &str| {
        genes
            .iter()
            .position(|gene| gene.tag == tag)
            .unwrap_or_else(|| panic!("{tag} is in this build"))
    };
    assert!(
        !control[index(TAG)],
        "the control genome already plays {TAG}"
    );
    let genome_of = |arm: &str| -> Vec<bool> {
        let mut on = control.clone();
        match arm {
            GENE | FREE_IRON => on[index(TAG)] = true,
            STACK => {
                for tag in STACK_ON {
                    on[index(tag)] = true;
                }
                for tag in STACK_OFF {
                    on[index(tag)] = false;
                }
            }
            _ => {}
        }
        on
    };
    let genomes: Vec<(&str, Vec<bool>)> = arms.iter().map(|arm| (*arm, genome_of(arm))).collect();
    let stack_note = STACK_ON
        .iter()
        .map(|tag| format!("{tag} {}", if control[index(tag)] { "on" } else { "off" }))
        .chain(
            STACK_OFF
                .iter()
                .map(|tag| format!("{tag} {}", if control[index(tag)] { "on" } else { "off" })),
        )
        .collect::<Vec<_>>()
        .join(", ");
    let genome_note = format!(
        "control genome {path}: {} tags, {} genes on in this build, {} tag(s) it does not carry; in the control: {stack_note}",
        tags.len(),
        control.iter().filter(|on| **on).count(),
        unknown.len()
    );
    let jobs: Vec<(usize, usize)> = (0..n)
        .filter(|world| world % shards == shard)
        .flat_map(|world| (0..genomes.len()).map(move |arm| (world, arm)))
        .collect();
    let out = std::env::var("CIVVIS_EDGE_OUT").ok().map(|path| {
        std::sync::Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap_or_else(|e| panic!("{path}: {e}")),
        )
    });
    let next = AtomicUsize::new(0);
    let results = std::sync::Mutex::new(Vec::new());
    let started = Instant::now();
    std::thread::scope(|scope| {
        for _ in 0..std::thread::available_parallelism().map_or(2, |c| c.get()) {
            std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn_scoped(scope, || loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&(world, arm)) = jobs.get(k) else {
                        break;
                    };
                    let (name, genome) = &genomes[arm];
                    let seed = first_seed + world as u64;
                    let played = play(name, world, seed, turns, &genes, genome);
                    progress(&format!(
                        "EDGE-UNIT-WORLD {} seed {} opened {:?} declared {:?} taken {} lost {} wars-ours {} legions@50 {:?} iw {:?} sci@100 {} score {} turn {} winner {:?} {} fp {:016x} secs {:.0}",
                        played.arm,
                        played.seed,
                        counter_turn(&played, "opened"),
                        counter_turn(&played, "declared"),
                        taken_by(&played, None),
                        lost_by(&played, None),
                        wars_we_declared(&played).len(),
                        played.watch.legions.get(&50),
                        played.watch.iron_working,
                        cell(played.watch.samples.get(&100).map(|s| s.science)),
                        played.row.score,
                        played.row.turn,
                        played.row.winner,
                        played.row.victory,
                        played.fingerprint,
                        played.row.secs
                    ));
                    if let Some(file) = &out {
                        use std::io::Write as _;
                        let line = serde_json::to_string(&played).expect("a world serializes");
                        let mut file = file.lock().unwrap();
                        writeln!(file, "{line}").expect("a world is written");
                    }
                    results.lock().unwrap().push((world, arm, played));
                })
                .expect("a worker");
        }
    });
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|(world, arm, _)| (*world, *arm));
    let worlds: Vec<World> = results.into_iter().map(|(_, _, w)| w).collect();
    let note = format!(
        "EDGE-UNIT-REPORT: worlds {n} from seed {first_seed}, {turns} turns, arms {arms:?}, shard {shard}/{shards}, {:.0}s, build {}\n{genome_note}",
        started.elapsed().as_secs_f64(),
        std::env::var("GITHUB_SHA").unwrap_or_else(|_| "local".to_string()),
    );
    println!("{}", report(&worlds, &note));
}

fn test_row(seat: usize) -> Row {
    serde_json::from_str(&format!(
        r#"{{"kind":"game","game":0,"seed":7,"seat":{seat},"genome":"1","win":false,"winner":2,
            "victory":"science","turn":150,"score":300,"score_share":0.2,"rank":2,"cities":6,
            "alive":true,"secs":50.0}}"#
    ))
    .expect("a row parses")
}

/// The report pairs each arm with the control by seed, counts the gene's
/// stages from its counters, and attributes wars by their declarer.
#[test]
fn the_report_pairs_each_arm_with_the_control() {
    let world = |arm: &str, seed: u64, taken: usize, opened: bool| {
        let mut watch = Watch {
            seat: Some(1),
            ..Watch::default()
        };
        for k in 0..taken {
            watch.taken.push((50 + k as u32, 2, true));
        }
        if opened {
            watch.counters.insert("opened".to_string(), (35, 1));
            watch.counters.insert("declared".to_string(), (48, 1));
            watch.wars.push((48, 1, 2));
        }
        watch.wars.push((70, 3, 1));
        watch.samples.insert(
            100,
            Sample {
                cities: 6,
                techs: 30,
                science: 80.0,
                military: 200.0,
            },
        );
        World {
            arm: arm.to_string(),
            world: (seed - 100) as usize,
            seed,
            turns: 150,
            fingerprint: seed + u64::from(opened),
            watch,
            events: vec![(48, "declared: 3 legion".to_string())],
            row: test_row(1),
        }
    };
    let worlds = vec![
        world(CONTROL, 100, 0, false),
        world(GENE, 100, 2, true),
        world(CONTROL, 101, 1, false),
        world(GENE, 101, 1, false),
        world(STACK, 102, 1, false),
    ];
    let text = report(&worlds, "note");
    assert!(
        text.contains("== gene minus control, paired by seed: 2 pairs, seeds 100..101, 1 diverged"),
        "{text}"
    );
    assert!(!text.contains("== stack minus control"), "{text}");
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("cities taken (end)"))
        .unwrap_or_else(|| panic!("no taken line in\n{text}"));
    // Taken 2-0 and 1-1: a mean of +1.00, a standard error of 1.00.
    assert!(line.contains("+1.00±1.00"), "{line}");
    assert!(text.contains("window opened 1/2"), "{text}");
    // The rival declared on us at t70: one war on us, none by t100 missed.
    let on_us = text
        .lines()
        .find(|line| line.trim_start().starts_with("wars on us by t100"))
        .unwrap_or_else(|| panic!("no wars-on-us line in\n{text}"));
    assert!(on_us.contains("1.00±0.00"), "{on_us}");
    // Winner p2 is the rival the gene arm declared on in seed 100.
    assert!(text.contains("winner a rival we declared on 1/2"), "{text}");
}

/// The journal reader keeps the measured seat's gene stages and the
/// early-conquest lines, once for consecutive repeats.
#[test]
fn the_journal_keeps_the_seat_s_stages() {
    let path =
        std::env::temp_dir().join(format!("edge-unit-journal-test-{}.txt", std::process::id()));
    let text = [
        "t35 p1 Military Edge unit window opens on Memphis | edge-unit-takes-the-soft-city: opened: legion (40) against Memphis",
        "t36 p1 Research Edge unit research goal: iron working | edge-unit-takes-the-soft-city: research: iron working toward legion",
        "t40 p1 Military Edge unit holds the declaration | edge-unit-takes-the-soft-city: hold: the forecast reads 20%",
        "t40 p1 Military Edge unit holds the declaration | edge-unit-takes-the-soft-city: hold: the forecast reads 20%",
        "t41 p2 Military Edge unit window opens on Thebes | edge-unit-takes-the-soft-city: opened: legion (40) against Thebes",
        "t42 p1 Military Opening a conquest against Egypt | Memphis is 7 tiles from the capital",
        "t43 p1 Military Something else entirely",
    ]
    .join("\n");
    std::fs::write(&path, text).unwrap();
    let events = journal_events(path.to_str().unwrap(), 1);
    assert!(!path.exists(), "the journal file is removed once read");
    assert_eq!(events.len(), 4, "{events:?}");
    assert_eq!(
        events[0],
        (35, "opened: legion (40) against Memphis".to_string())
    );
    assert_eq!(events[2].0, 40);
    assert!(events[3]
        .1
        .starts_with("conquest: Opening a conquest against Egypt"));
}
