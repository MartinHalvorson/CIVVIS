//! ⭐ Which ARMED combat genes cost our seat units or cities when it is
//! outclassed, the condition it fights in live? A leave-one-out screen on the
//! gap-faithful lane recipes (`docs/COMBAT_GENES_OUTCLASSED.md`).
//!
//! Every arm plays the live armed set as `--live-genome`, with
//! `--rival-host-tactics` and no `--rival-rung`: the settings
//! `docs/EDGE_LEDGER.md` found to reproduce the live strength gap (and, for
//! Rome, `--rival-war-pace 100`). The control arm plays the set as armed;
//! each gene arm turns ONE armed gene off and plays the control's seeds and
//! game indices, so a pair differs in that gene alone. Every arm of a world
//! is played by the same process, so no pair crosses a shard or a build.
//!
//! What a world records: the measured seat's gene-screen row (the combat
//! ledger, `last_orders`, the edge ledger, cities lost and taken, score), the
//! military units an observer saw go between two turns, by distance from the
//! owner's nearest City Center at the start of the turn they went and by
//! kind, and a fingerprint of the row (wall seconds and genome left out) with
//! the action log's length, so a pair that played the same game is told
//! apart from a null.
//!
//! `cargo test --profile ci --features developer-tools --bin gene_screen
//! combat_genes_outclassed_driver -- --ignored --nocapture`, with
//! - `CIVVIS_LIVE_GENOME`: the armed set (default `~/.civvis-live-force-on`);
//! - `CIVVIS_COMBAT_RECIPE`: `gran-colombia` (default) or `rome`;
//! - `CIVVIS_COMBAT_GENES`: the armed tags to leave out, comma separated
//!   (default [`SCREENED`]; `none` plays the control alone);
//! - `CIVVIS_COMBAT_WORLDS` (default 16) seeds from `CIVVIS_COMBAT_SEED`;
//! - `CIVVIS_COMBAT_REPEAT=1`: the control a second time, a determinism check;
//! - `CIVVIS_COMBAT_SHARD=k/K`: only the worlds whose index is k mod K;
//! - `CIVVIS_COMBAT_OUT`: a file each finished world is appended to as JSON;
//! - `CIVVIS_COMBAT_ROWS`: a comma list of such files; the report over them,
//!   and no games.

use super::edge_ledger::metrics;
use super::*;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

const SEED: u64 = 64_710_000;
const CONTROL: &str = "armed";
const REPEAT: &str = "armed-again";

/// The armed genes the screen leaves out, one an arm. Why each is here is in
/// `docs/COMBAT_GENES_OUTCLASSED.md`.
const SCREENED: &[&str] = &[
    "battle-planner-2",
    "safest-stand",
    "shared-danger",
    "ranged-hp-reserve",
    "siege-train",
    "siege-commitment",
    "siege-needs-a-breaker",
    "anvil",
    "war-reinforcement",
    "march-uses-its-moves",
    "capital-defense-holds",
    "defenders-hold-the-walls",
    "threatened-city-buys-its-defender-2",
    "recon-replacement",
];

/// One lane recipe, as its command line spells it.
struct Recipe {
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
    rival_war_pace: Option<u32>,
}

/// `--players 4 --width 60 --height 38 --city-states 6 --map pangaea
/// --difficulty deity --rivals firaxis-faithful --handicap rivals
/// --rival-chairs 3 --rival-pace --target-mix domination
/// --rival-host-tactics`, no `--rival-rung`.
const GRAN_COLOMBIA: Recipe = Recipe {
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
    rival_war_pace: None,
};

/// `--players 6 --width 74 --height 46 --city-states 9 --map continents
/// --difficulty deity --rivals firaxis-faithful --handicap rivals
/// --rival-chairs 5 --rival-pace --rival-war-pace 100 --target-mix score
/// --seat-civ Rome --turns 170 --rival-host-tactics`, no `--rival-rung`.
const ROME: Recipe = Recipe {
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
    rival_war_pace: Some(100),
};

fn profile(recipe: &Recipe) -> Profile {
    let speed = GameSpeed::Online;
    Profile {
        target_mix: civvis::ai::player::parse_targets(recipe.target).expect("a lane"),
        players: recipe.players,
        width: recipe.width,
        height: recipe.height,
        turns: recipe.turns.unwrap_or(speed.turn_limit()),
        city_states: recipe.city_states,
        speed,
        map: recipe.map,
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
        rival_war_pace: recipe.rival_war_pace,
        rival_walls: None,
        rival_host_tactics: true,
        chair_tactics_census: false,
        seat_civ: recipe.seat_civ.map(str::to_string),
        rival_chairs: recipe.rival_chairs,
        handicap_rivals: true,
        journal_seeds: BTreeSet::new(),
        journal_base: String::new(),
    }
}

/// Distance from the owner's nearest City Center, in classes.
const CLASSES: [&str; 6] = ["centre", "1-2", "3-5", "6-9", "10+", "no city"];

fn class_of(distance: Option<i32>) -> usize {
    match distance {
        Some(0) => 0,
        Some(1..=2) => 1,
        Some(3..=5) => 2,
        Some(6..=9) => 3,
        Some(_) => 4,
        None => 5,
    }
}

/// The military units an observer saw go between two turns: by distance
/// class at the start of the turn they went, and by kind. A unit disbanded or
/// merged into a corps goes too; the combat ledger's `units_lost` is the
/// count of the ones an enemy removed.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Gone {
    by_class: [u32; 6],
    by_kind: BTreeMap<String, u32>,
}

/// One arm of one world.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct World {
    recipe: String,
    arm: String,
    world: usize,
    seed: u64,
    fingerprint: u64,
    gone: Gone,
    row: Row,
}

fn env_number(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(default)
}

/// The armed set, read and applied, never printed: how many tags it names
/// and how many of them this build does not carry.
fn armed_genome(genes: &mut [Gene]) -> (Vec<bool>, String) {
    let candidates: Vec<String> = std::env::var("CIVVIS_LIVE_GENOME")
        .ok()
        .into_iter()
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

/// One line straight to the process's stderr, past the harness's capture.
fn progress(line: &str) {
    use std::io::Write as _;
    let _ = writeln!(std::io::stderr().lock(), "{line}");
}

/// Play one arm of one world, with the observer that books the units gone.
fn play(
    recipe: &Recipe,
    arm: &str,
    world: usize,
    seed: u64,
    genes: &[Gene],
    genome: &[bool],
) -> World {
    let profile = profile(recipe);
    let players = profile.players;
    let genomes = vec![genome.to_vec(); players];
    // By owner: the units gone, and the action log's length at the last look.
    let book: Rc<RefCell<(BTreeMap<usize, Gone>, usize)>> = Rc::default();
    let sink = Rc::clone(&book);
    GAME_OBSERVER.with(|observer| {
        // Each military unit of a major: owner, kind, distance class.
        let mut last: BTreeMap<u32, (usize, &'static str, usize)> = BTreeMap::new();
        *observer.borrow_mut() = Some(Box::new(move |g: &Game| {
            let mut book = sink.borrow_mut();
            book.1 = g.log.len();
            let mut centres: BTreeMap<usize, Vec<_>> = BTreeMap::new();
            for city in g.cities.values() {
                centres.entry(city.owner).or_default().push(city.pos);
            }
            let mut now: BTreeMap<u32, (usize, &'static str, usize)> = BTreeMap::new();
            for (uid, unit) in g.units.iter() {
                if unit.owner >= players
                    || unit.levied_from.is_some()
                    || g.rules.units[unit.kind].class != "military"
                {
                    continue;
                }
                let distance = centres
                    .get(&unit.owner)
                    .and_then(|at| at.iter().map(|pos| g.wdist(*pos, unit.pos)).min());
                now.insert(*uid, (unit.owner, unit.kind.as_str(), class_of(distance)));
            }
            for (uid, (owner, kind, class)) in &last {
                if now
                    .get(uid)
                    .is_none_or(|(now_owner, _, _)| now_owner != owner)
                {
                    let gone: &mut Gone = book.0.entry(*owner).or_default();
                    gone.by_class[*class] += 1;
                    *gone.by_kind.entry((*kind).to_string()).or_insert(0) += 1;
                }
            }
            last = now;
        }));
    });
    let rows = play_game(&profile, genes, world, seed, &genomes);
    GAME_OBSERVER.with(|observer| *observer.borrow_mut() = None);
    let row = rows
        .into_iter()
        .find(|row| row.kind == "game")
        .expect("one measured seat");
    let (mut by_owner, log_len) = std::mem::take(&mut *book.borrow_mut());
    let mut bare = row.clone();
    bare.secs = 0.0;
    bare.genome = String::new();
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    serde_json::to_string(&bare)
        .expect("a row serializes")
        .hash(&mut hasher);
    log_len.hash(&mut hasher);
    World {
        recipe: recipe.key.to_string(),
        arm: arm.to_string(),
        world,
        seed,
        fingerprint: hasher.finish(),
        gone: by_owner.remove(&row.seat).unwrap_or_default(),
        row,
    }
}

fn mean_se(xs: &[f64]) -> (f64, f64) {
    let n = xs.len().max(1) as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (mean, (var / n).sqrt())
}

fn lost(w: &World) -> f64 {
    f64::from(w.row.units_lost.unwrap_or(0))
}

fn killed(w: &World) -> f64 {
    f64::from(w.row.units_killed.unwrap_or(0))
}

/// Deaths the observer saw within five tiles of our nearest City Center.
fn near(w: &World) -> f64 {
    f64::from(w.gone.by_class[..3].iter().sum::<u32>())
}

/// The pooled kill:loss of the gene arm minus the control's over the pairs,
/// and its jackknife standard error over pairs.
fn kill_loss_diff(pairs: &[(&World, &World)]) -> (f64, f64) {
    let ratio = |pairs: &[&(&World, &World)], off: bool| -> f64 {
        let (k, l) = pairs.iter().fold((0.0, 0.0), |(k, l), (on_arm, off_arm)| {
            let w = if off { off_arm } else { on_arm };
            (k + killed(w), l + lost(w))
        });
        if l > 0.0 {
            k / l
        } else {
            f64::NAN
        }
    };
    let all: Vec<&(&World, &World)> = pairs.iter().collect();
    let full = ratio(&all, true) - ratio(&all, false);
    let n = pairs.len();
    if n < 2 {
        return (full, f64::NAN);
    }
    let thetas: Vec<f64> = (0..n)
        .map(|leave| {
            let rest: Vec<&(&World, &World)> = pairs
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != leave)
                .map(|(_, pair)| pair)
                .collect();
            ratio(&rest, true) - ratio(&rest, false)
        })
        .collect();
    let mean = thetas.iter().sum::<f64>() / n as f64;
    let var = thetas.iter().map(|t| (t - mean).powi(2)).sum::<f64>();
    (full, ((n as f64 - 1.0) / n as f64 * var).sqrt())
}

fn cell(d: f64, se: f64, flag: bool) -> String {
    if !d.is_finite() {
        return format!("{:>15}", "-");
    }
    let spread = if se.is_finite() {
        format!("{se:.2}")
    } else {
        "-".to_string()
    };
    let mark = if flag && se > 0.0 && (d / se).abs() >= 1.5 {
        "*"
    } else {
        " "
    };
    let text = format!("{d:+.2}±{spread}{mark}");
    format!("{text:>15}")
}

/// The report over every world given: per recipe, the control arm's losses
/// and where they died, then every gene arm against it, paired by seed.
fn report(worlds: &[World], note: &str) -> String {
    let mut out = vec![note.to_string()];
    let recipes: BTreeSet<&str> = worlds.iter().map(|w| w.recipe.as_str()).collect();
    for recipe in recipes {
        let of = |arm: &str| -> BTreeMap<u64, &World> {
            worlds
                .iter()
                .filter(|w| w.recipe == recipe && w.arm == arm)
                .map(|w| (w.seed, w))
                .collect()
        };
        let control = of(CONTROL);
        let mut arms: Vec<&str> = Vec::new();
        for w in worlds.iter().filter(|w| w.recipe == recipe) {
            if w.arm != CONTROL && !arms.contains(&w.arm.as_str()) {
                arms.push(w.arm.as_str());
            }
        }
        // The repeat first, then the screened genes in their list's order.
        arms.sort_by_key(|arm| {
            let rank = if *arm == REPEAT {
                0
            } else {
                SCREENED
                    .iter()
                    .position(|tag| tag == arm)
                    .map_or(usize::MAX, |i| i + 1)
            };
            (rank, *arm)
        });
        let base: Vec<&World> = control.values().copied().collect();
        out.push(String::new());
        out.push(format!(
            "== {recipe}: control (the armed set), {} worlds, seeds {}..{}",
            base.len(),
            control.keys().next().copied().unwrap_or(0),
            control.keys().last().copied().unwrap_or(0)
        ));
        if base.is_empty() {
            continue;
        }
        let per = |f: &dyn Fn(&World) -> f64| -> String {
            let (m, se) = mean_se(&base.iter().map(|w| f(w)).collect::<Vec<_>>());
            format!("{m:.2}±{se:.2}")
        };
        out.push(format!(
            "   per world: units lost {} (majors {}, city-states {}, barbarians {}), killed {}, civilians lost {}",
            per(&lost),
            per(&|w| f64::from(w.row.units_lost_to_majors.unwrap_or(0))),
            per(&|w| f64::from(w.row.units_lost_to_city_states.unwrap_or(0))),
            per(&|w| f64::from(w.row.units_lost_to_barbarians.unwrap_or(0))),
            per(&killed),
            per(&|w| f64::from(w.row.civilians_lost.unwrap_or(0))),
        ));
        out.push(format!(
            "   per world: cities lost {}, cities taken {}, score {}, score share {}, wins {}, alive {}, last turn {}",
            per(&|w| w.row.cities_lost.unwrap_or(0) as f64),
            per(&|w| w.row.cities_taken.unwrap_or(0) as f64),
            per(&|w| w.row.score as f64),
            per(&|w| w.row.score_share),
            per(&|w| f64::from(u8::from(w.row.win))),
            per(&|w| f64::from(u8::from(w.row.alive))),
            per(&|w| f64::from(w.row.turn)),
        ));
        let rows: Vec<&Row> = base.iter().map(|w| &w.row).collect();
        let gap: Vec<String> = metrics(&rows)
            .into_iter()
            .filter(|(name, _)| {
                [
                    "major_blows_per_game",
                    "major_share_ge10",
                    "major_death_per_blow_ge10",
                    "strength_gap_100",
                    "kill_loss",
                    "cities_lost_per_game",
                ]
                .contains(&name.as_str())
            })
            .map(|(name, value)| match value {
                Some(v) => format!("{name} {v:.3}"),
                None => format!("{name} -"),
            })
            .collect();
        out.push(format!("   the gap: {}", gap.join(", ")));
        // Deaths by the step that last ordered the unit.
        let mut steps: BTreeMap<&str, [u32; 2]> = BTreeMap::new();
        for w in &base {
            for (step, pair) in &w.row.last_orders {
                let cell = steps.entry(step.as_str()).or_default();
                cell[0] += pair[0];
                cell[1] += pair[1];
            }
        }
        let total: u32 = steps.values().map(|p| p[0] + p[1]).sum();
        let mut ranked: Vec<(&str, [u32; 2])> = steps.into_iter().collect();
        ranked.sort_by_key(|(step, pair)| (std::cmp::Reverse(pair[0] + pair[1]), *step));
        out.push(format!(
            "   deaths by the step that last ordered the unit ({total} counted, {:.2} a world; within 2 = ended its last turn within two tiles of a visible hostile):",
            f64::from(total) / base.len() as f64
        ));
        out.push(format!(
            "     {:<22} {:>7} {:>9} {:>7} {:>9}",
            "step", "deaths", "a world", "share", "within 2"
        ));
        for (step, [within, beyond]) in &ranked {
            let n = within + beyond;
            out.push(format!(
                "     {step:<22} {n:>7} {:>9.2} {:>6.1}% {:>8.1}%",
                f64::from(n) / base.len() as f64,
                100.0 * f64::from(n) / f64::from(total.max(1)),
                100.0 * f64::from(*within) / f64::from(n.max(1)),
            ));
        }
        let mut classes = [0u32; 6];
        let mut kinds: BTreeMap<&str, u32> = BTreeMap::new();
        for w in &base {
            for (i, n) in w.gone.by_class.iter().enumerate() {
                classes[i] += n;
            }
            for (kind, n) in &w.gone.by_kind {
                *kinds.entry(kind.as_str()).or_insert(0) += n;
            }
        }
        let gone: u32 = classes.iter().sum();
        let ledger: f64 = base.iter().map(|w| lost(w)).sum();
        out.push(format!(
            "   units gone by distance from our nearest City Center at the start of the turn they went ({gone} gone; the ledger's units lost {ledger:.0}):"
        ));
        out.push(format!(
            "     {}",
            CLASSES
                .iter()
                .zip(classes)
                .map(|(name, n)| format!(
                    "{name} {n} ({:.0}%)",
                    100.0 * f64::from(n) / f64::from(gone.max(1))
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        let mut kinds: Vec<(&str, u32)> = kinds.into_iter().collect();
        kinds.sort_by_key(|(kind, n)| (std::cmp::Reverse(*n), *kind));
        out.push(format!(
            "   units gone by kind: {}",
            kinds
                .iter()
                .take(12)
                .map(|(kind, n)| format!("{kind} {n}"))
                .collect::<Vec<_>>()
                .join(", ")
        ));
        if arms.is_empty() {
            continue;
        }
        out.push(String::new());
        out.push(format!(
            "== {recipe}: one armed gene OFF minus the armed set, paired by seed (mean ± se a world; * = |d| >= 1.5 se on a loss column)"
        ));
        out.push(format!(
            "   {:<36} {:>5} {:>8} {:>15} {:>15} {:>15} {:>15} {:>15} {:>15} {:>15} {:>15}",
            "gene off",
            "pairs",
            "diverged",
            "units lost",
            "to majors",
            "lost <=5 tiles",
            "killed",
            "kill:loss",
            "cities lost",
            "cities taken",
            "score"
        ));
        let mut shifts: Vec<String> = Vec::new();
        for arm in &arms {
            let gene = of(arm);
            let pairs: Vec<(&World, &World)> = control
                .iter()
                .filter_map(|(seed, on)| gene.get(seed).map(|off| (*on, *off)))
                .collect();
            let diverged = pairs
                .iter()
                .filter(|(on, off)| on.fingerprint != off.fingerprint)
                .count();
            let diff = |f: &dyn Fn(&World) -> f64| -> (f64, f64) {
                mean_se(
                    &pairs
                        .iter()
                        .map(|(on, off)| f(off) - f(on))
                        .collect::<Vec<_>>(),
                )
            };
            let (lost_d, lost_se) = diff(&lost);
            let (maj_d, maj_se) = diff(&|w| f64::from(w.row.units_lost_to_majors.unwrap_or(0)));
            let (near_d, near_se) = diff(&near);
            let (kill_d, kill_se) = diff(&killed);
            let (kl_d, kl_se) = kill_loss_diff(&pairs);
            let (cl_d, cl_se) = diff(&|w| w.row.cities_lost.unwrap_or(0) as f64);
            let (ct_d, ct_se) = diff(&|w| w.row.cities_taken.unwrap_or(0) as f64);
            let (sc_d, sc_se) = diff(&|w| w.row.score as f64);
            let diverged = format!("{diverged}/{}", pairs.len());
            out.push(format!(
                "   {arm:<36} {:>5} {diverged:>8} {} {} {} {} {} {} {} {}",
                pairs.len(),
                cell(lost_d, lost_se, true),
                cell(maj_d, maj_se, true),
                cell(near_d, near_se, true),
                cell(kill_d, kill_se, false),
                cell(kl_d, kl_se, false),
                cell(cl_d, cl_se, true),
                cell(ct_d, ct_se, false),
                cell(sc_d, sc_se, false),
            ));
            // Where the deaths moved: the steps whose summed count changed most.
            let mut moved: BTreeMap<&str, i64> = BTreeMap::new();
            for (on, off) in &pairs {
                for (step, pair) in &off.row.last_orders {
                    *moved.entry(step.as_str()).or_insert(0) += i64::from(pair[0] + pair[1]);
                }
                for (step, pair) in &on.row.last_orders {
                    *moved.entry(step.as_str()).or_insert(0) -= i64::from(pair[0] + pair[1]);
                }
            }
            let mut moved: Vec<(&str, i64)> = moved.into_iter().filter(|(_, d)| *d != 0).collect();
            moved.sort_by_key(|(step, d)| (std::cmp::Reverse(d.abs()), *step));
            shifts.push(format!(
                "   {arm:<36} {}",
                moved
                    .iter()
                    .take(5)
                    .map(|(step, d)| format!("{step} {d:+}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        out.push(String::new());
        out.push(format!(
            "== {recipe}: deaths by last order step, gene OFF minus the armed set, summed over the pairs (largest five moves)"
        ));
        out.extend(shifts);
        out.push(String::new());
        out.push(format!(
            "== {recipe}: per pair (seed: control lost/killed/cities lost/taken/score | each arm the same, in the order above)"
        ));
        for (seed, on) in &control {
            let mut line = format!(
                "   {seed}: {}/{}/{}/{}/{}",
                lost(on),
                killed(on),
                on.row.cities_lost.unwrap_or(0),
                on.row.cities_taken.unwrap_or(0),
                on.row.score
            );
            for arm in &arms {
                match of(arm).get(seed) {
                    Some(off) => line.push_str(&format!(
                        " | {}/{}/{}/{}/{}{}",
                        lost(off),
                        killed(off),
                        off.row.cities_lost.unwrap_or(0),
                        off.row.cities_taken.unwrap_or(0),
                        off.row.score,
                        if off.fingerprint == on.fingerprint {
                            "="
                        } else {
                            ""
                        }
                    )),
                    None => line.push_str(" | -"),
                }
            }
            out.push(line);
        }
    }
    out.join("\n")
}

#[test]
#[ignore = "whole Deity games, an arm per screened gene: run with --ignored --nocapture"]
fn combat_genes_outclassed_driver() {
    if let Ok(files) = std::env::var("CIVVIS_COMBAT_ROWS") {
        let mut worlds: Vec<World> = Vec::new();
        for path in files.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                worlds.push(serde_json::from_str(line).expect("a world parses"));
            }
        }
        worlds.sort_by_key(|w| w.world);
        let note = format!(
            "COMBAT-GENES-REPORT: {} worlds read from {} file(s)",
            worlds.len(),
            files.split(',').filter(|p| !p.trim().is_empty()).count()
        );
        println!("{}", report(&worlds, &note));
        return;
    }
    let recipe = match std::env::var("CIVVIS_COMBAT_RECIPE").as_deref() {
        Ok("rome") => &ROME,
        _ => &GRAN_COLOMBIA,
    };
    let n = env_number("CIVVIS_COMBAT_WORLDS", 16) as usize;
    let first_seed = env_number("CIVVIS_COMBAT_SEED", SEED);
    let (shard, shards) = std::env::var("CIVVIS_COMBAT_SHARD")
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
    let mut genes = gene_table();
    let (armed, armed_note) = armed_genome(&mut genes);
    let wanted: Vec<String> = match std::env::var("CIVVIS_COMBAT_GENES") {
        Ok(list) if list.trim() == "none" => Vec::new(),
        Ok(list) => list
            .split(',')
            .map(str::trim)
            .filter(|tag| !tag.is_empty())
            .map(str::to_string)
            .collect(),
        Err(_) => SCREENED.iter().map(|tag| tag.to_string()).collect(),
    };
    let mut arms: Vec<(String, Vec<bool>)> = vec![(CONTROL.to_string(), armed.clone())];
    if std::env::var("CIVVIS_COMBAT_REPEAT").is_ok_and(|v| v.trim() == "1") {
        arms.push((REPEAT.to_string(), armed.clone()));
    }
    let mut skipped: Vec<String> = Vec::new();
    for tag in &wanted {
        match genes.iter().position(|gene| gene.tag == tag.as_str()) {
            Some(i) if armed[i] => {
                let mut genome = armed.clone();
                genome[i] = false;
                arms.push((tag.clone(), genome));
            }
            Some(_) => skipped.push(format!("{tag} (not armed)")),
            None => skipped.push(format!("{tag} (not in this build)")),
        }
    }
    let jobs: Vec<(usize, usize)> = (0..n)
        .filter(|world| world % shards == shard)
        .flat_map(|world| (0..arms.len()).map(move |arm| (world, arm)))
        .collect();
    let out = std::env::var("CIVVIS_COMBAT_OUT").ok().map(|path| {
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
                    let (name, genome) = &arms[arm];
                    let seed = first_seed + world as u64;
                    let played = play(recipe, name, world, seed, &genes, genome);
                    progress(&format!(
                        "COMBAT-WORLD {} {} seed {} lost {} majors {} killed {} cities_lost {} taken {} score {} turn {} fp {:016x} secs {:.0}",
                        played.recipe,
                        played.arm,
                        played.seed,
                        lost(&played),
                        played.row.units_lost_to_majors.unwrap_or(0),
                        killed(&played),
                        played.row.cities_lost.unwrap_or(0),
                        played.row.cities_taken.unwrap_or(0),
                        played.row.score,
                        played.row.turn,
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
        "COMBAT-GENES-REPORT: {} recipe, worlds {n} from seed {first_seed}, shard {shard}/{shards}, arms {:?}, {:.0}s, build {}\n{armed_note}{}",
        recipe.key,
        arms.iter().map(|(name, _)| name.as_str()).collect::<Vec<_>>(),
        started.elapsed().as_secs_f64(),
        std::env::var("GITHUB_SHA").unwrap_or_else(|_| "local".to_string()),
        if skipped.is_empty() {
            String::new()
        } else {
            format!("\nnot screened: {}", skipped.join(", "))
        }
    );
    println!("{}", report(&worlds, &note));
}

/// The report pairs a gene arm with the control by seed, calls a pair that
/// played the same game undiverged, and prints the paired difference.
#[test]
fn the_report_pairs_arms_by_seed_and_tells_an_identical_game_apart() {
    let row = |lost: u32, killed: u32| -> Row {
        let mut row: Row = serde_json::from_str(
            r#"{"kind":"game","game":0,"seed":7,"seat":1,"genome":"1","win":false,"winner":0,
                "victory":"score","turn":250,"score":900,"score_share":0.3,"rank":2,"cities":8,
                "alive":true,"secs":50.0}"#,
        )
        .expect("a row parses");
        row.units_lost = Some(lost);
        row.units_killed = Some(killed);
        row.cities_lost = Some(1);
        row.cities_taken = Some(0);
        row
    };
    let world = |arm: &str, seed: u64, lost: u32, fingerprint: u64| World {
        recipe: "gran-colombia".to_string(),
        arm: arm.to_string(),
        world: (seed - 100) as usize,
        seed,
        fingerprint,
        gone: Gone::default(),
        row: row(lost, 10),
    };
    let worlds = vec![
        world(CONTROL, 100, 10, 1),
        world("some-gene", 100, 6, 2),
        world(CONTROL, 101, 12, 3),
        world("some-gene", 101, 12, 3),
        world("some-gene", 102, 1, 9),
    ];
    let text = report(&worlds, "note");
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("some-gene") && line.contains("1/2"))
        .unwrap_or_else(|| panic!("no paired line in\n{text}"));
    // Two pairs (seed 102 has no control), one diverged; units lost 6-10 and
    // 12-12, a mean of -2.00 with a standard error of 2.00.
    assert!(line.contains("-2.00±2.00"), "{line}");
    assert_eq!(
        kill_loss_diff(&[(&worlds[0], &worlds[1]), (&worlds[2], &worlds[3])]).0,
        20.0 / 18.0 - 20.0 / 22.0
    );
}
