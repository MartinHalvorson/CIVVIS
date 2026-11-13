//! ⭐ `one-upgrade-lifts-every-city` on whole worlds: the genome the live seat
//! plays with the gene off (the control) and on, paired by seed, on the two
//! gap-faithful lane recipes of `docs/COMBAT_GENES_OUTCLASSED.md`
//! (`docs/ONE_UPGRADE_LIFTS_EVERY_CITY.md`).
//!
//! Both arms play `--rival-host-tactics` with no `--rival-rung` (and, for
//! Rome, `--rival-war-pace 100`), the settings `docs/EDGE_LEDGER.md` found to
//! reproduce the live strength gap. The control's genome is a live run's
//! genome header (`why.log` line 1); the gene arm is that genome with the gene
//! on. A pair plays one seed and game index in one process, so it never
//! crosses a shard or a build.
//!
//! What a world records, besides the measured seat's gene-screen row: what an
//! observer saw at the start of every turn (the seat's upgrades, as units that
//! changed kind; the ones among them that lifted its best melee or ranged
//! strength ever owned by 5 or more; the turns from turn 30 on which that best
//! sat 10 or more below what it had unlocked with the material in stock, the
//! live gap; and the kills no unit made, its city and district strikes), and
//! the upgrades the new pass planned, from its journal lines. The simulated
//! seat replans its turn after a surprise and every frame journals again, so
//! a planned upgrade counts as carried out only when the next turn's look saw
//! a unit go from that kind to that successor.
//!
//! `cargo test --profile ci --features developer-tools --bin gene_screen
//! one_upgrade_lifts_every_city_driver -- --ignored --nocapture`, with
//! - `CIVVIS_ONE_UPGRADE_GENOME`: the control's genome file (a live
//!   `why.log` or its first line; a plain tag list turns every gene it does
//!   not name off, ledger defaults included);
//! - `CIVVIS_ONE_UPGRADE_RECIPE`: `gran-colombia` (default) or `rome`;
//! - `CIVVIS_ONE_UPGRADE_WORLDS` (default 16) seeds from
//!   `CIVVIS_ONE_UPGRADE_SEED`;
//! - `CIVVIS_ONE_UPGRADE_SHARD=k/K`: only the worlds whose index is k mod K;
//! - `CIVVIS_ONE_UPGRADE_OUT`: a file each finished world is appended to;
//! - `CIVVIS_ONE_UPGRADE_ROWS`: a comma list of such files; the report over
//!   them, and no games.

use super::*;
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

const TAG: &str = "one-upgrade-lifts-every-city";
/// The gene's journal headline.
const HEADLINE: &str = "One upgrade lifts every city";
const SEED: u64 = 64_760_000;
const CONTROL: &str = "control";
const GENE_ON: &str = "gene-on";
/// The live census reads the gap from this turn.
const GAP_FROM_TURN: u32 = 30;
/// A best-ever strength this far below the unlocked best is short.
const SHORT: f64 = 10.0;
/// An upgrade that lifts the best-ever strength this much is a new tier
/// (the gene's `CITY_LIFT_MIN`).
const TIER_LIFT: f64 = 5.0;

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

/// The recipe's profile, recording the measured seat's journal for `seed`
/// to `journal_base`.
fn profile(recipe: &Recipe, seed: u64, journal_base: String) -> Profile {
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
        journal_seeds: BTreeSet::from([seed]),
        journal_base,
    }
}

/// What the observer saw of one seat over a game.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct Watch {
    /// Turns from [`GAP_FROM_TURN`] on which the seat held a city.
    turns: u32,
    /// Of them, the best melee strength ever owned [`SHORT`] or more below
    /// the strongest unit unlocked with its material in stock.
    defence_short: u32,
    /// The same for the best ranged strength.
    strike_short: u32,
    /// Either.
    either_short: u32,
    /// Units of the seat that changed kind between two turns.
    upgrades: u32,
    /// Of them, the ones that lifted the best melee or ranged strength the
    /// seat had owned by [`TIER_LIFT`] or more.
    tier_lifts: u32,
    first_tier_lift: Option<u32>,
    /// Kills no unit made: the seat's city and district strikes.
    strike_kills: i64,
    /// Every upgrade seen: the turn of the look that saw it (the turn after
    /// the one it was taken on), and the kinds it went from and to.
    #[serde(default)]
    seen: Vec<(u32, String, String)>,
}

/// One upgrade the new pass planned, from its journal line.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
struct Planned {
    turn: u32,
    from: String,
    to: String,
    gold: f64,
    defence: f64,
    strike: f64,
}

/// One arm of one world.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct World {
    recipe: String,
    arm: String,
    world: usize,
    seed: u64,
    fingerprint: u64,
    /// The turn of every upgrade the new pass planned that the board then
    /// carried out (a planned upgrade the observer saw happen).
    pass: Vec<u32>,
    /// The Gold those cost.
    #[serde(default)]
    pass_gold: f64,
    /// Every distinct upgrade the pass planned. The simulated seat replans
    /// its turn after a surprise (`ai::player::take_turn`), and each frame
    /// journals again, so a line can repeat and need not have been carried
    /// out.
    #[serde(default)]
    planned: Vec<Planned>,
    watch: Watch,
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

/// The best melee and ranged strength `pid` has owned: the engine's counters
/// and the military units it owns now (the gene's basis, less the observed
/// strike a native board does not carry).
fn basis(g: &Game, pid: usize) -> (f64, f64) {
    let counter = |name: &str| {
        g.players[pid]
            .counters
            .get(name)
            .map_or(0.0, |best| *best as f64)
    };
    let mut best = (
        counter("strongest_unit_built"),
        counter("strongest_ranged_built"),
    );
    for unit in g.units.values().filter(|unit| unit.owner == pid) {
        let spec = &g.rules.units[unit.kind];
        if spec.class == "military" {
            best.0 = best.0.max(spec.strength);
            best.1 = best.1.max(spec.ranged_strength);
        }
    }
    best
}

/// The strongest land unit `pid` has unlocked with its material in stock:
/// its strength and its ranged strength.
fn unlocked(g: &Game, pid: usize) -> (f64, f64) {
    let player = &g.players[pid];
    let mut best = (0.0f64, 0.0f64);
    for spec in g.rules.units.values() {
        let open = spec.class == "military"
            && spec.buildable
            && spec.domain.as_deref().is_none_or(|domain| domain == "land")
            && spec.tech.is_none_or(|tech| player.techs.contains(&tech))
            && spec
                .civic
                .is_none_or(|civic| player.civics.contains(&civic))
            && spec
                .unique_to
                .as_deref()
                .is_none_or(|civ| g.owns_civ_unique(pid, civ))
            && spec.requires_resource.is_none_or(|resource| {
                g.strategic_stockpile(pid, resource) + f64::EPSILON >= spec.resource_cost
            });
        if open {
            best.0 = best.0.max(spec.strength);
            best.1 = best.1.max(spec.ranged_strength);
        }
    }
    best
}

/// The observer's memory of one seat between two turns.
#[derive(Default)]
struct Seen {
    kinds: BTreeMap<u32, &'static str>,
    basis: (f64, f64),
    /// The turn of the last look, so a second look at the final board does
    /// not count its turn twice.
    turn: Option<u32>,
    watch: Watch,
}

/// One look at every major at the start of a turn.
fn look(g: &Game, players: usize, seen: &mut BTreeMap<usize, Seen>) {
    for pid in 0..players.min(g.players.len()) {
        let seat = seen.entry(pid).or_default();
        let now = basis(g, pid);
        let mut kinds: BTreeMap<u32, &'static str> = BTreeMap::new();
        for (uid, unit) in g.units.iter() {
            if unit.owner == pid && g.rules.units[unit.kind].class == "military" {
                kinds.insert(*uid, unit.kind.as_str());
            }
        }
        // Upgrades since the last look, in id order, against the basis the
        // last look read, which the first of a tier raises.
        let mut running = seat.basis;
        for (uid, kind) in &kinds {
            let Some(before) = seat.kinds.get(uid) else {
                continue;
            };
            if before == kind {
                continue;
            }
            seat.watch.upgrades += 1;
            seat.watch
                .seen
                .push((g.turn, before.to_string(), kind.to_string()));
            let to = &g.rules.units[*kind];
            if to.strength - running.0 >= TIER_LIFT || to.ranged_strength - running.1 >= TIER_LIFT {
                seat.watch.tier_lifts += 1;
                seat.watch.first_tier_lift.get_or_insert(g.turn);
            }
            running = (
                running.0.max(to.strength),
                running.1.max(to.ranged_strength),
            );
        }
        if g.turn >= GAP_FROM_TURN
            && seat.turn != Some(g.turn)
            && !g.player_city_ids(pid).is_empty()
        {
            let open = unlocked(g, pid);
            let defence = open.0 - now.0 >= SHORT;
            let strike = open.1 - now.1 >= SHORT;
            seat.watch.turns += 1;
            seat.watch.defence_short += u32::from(defence);
            seat.watch.strike_short += u32::from(strike);
            seat.watch.either_short += u32::from(defence || strike);
        }
        let counters = &g.players[pid].counters;
        let by_units: i64 = counters
            .iter()
            .filter(|(name, _)| name.starts_with("kill_with:"))
            .map(|(_, n)| *n)
            .sum();
        seat.watch.strike_kills = counters.get("kills").copied().unwrap_or(0) - by_units;
        seat.kinds = kinds;
        seat.basis = now;
        seat.turn = Some(g.turn);
    }
}

/// The number after `key` in `text`, up to the next comma, space or `)`.
fn number_after(text: &str, key: &str) -> Option<f64> {
    let rest = &text[text.find(key)? + key.len()..];
    let end = rest
        .find(|c: char| c == ',' || c == ' ' || c == ')')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// The distinct upgrades the new pass planned for `seat`, from the seat's
/// journal file (removed once read): one per turn, unit kind and successor,
/// however many frames journaled it.
fn planned_upgrades(path: &str, seat: usize) -> Vec<Planned> {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let _ = std::fs::remove_file(path);
    let mut planned: Vec<Planned> = Vec::new();
    for line in text.lines() {
        let mut words = line.splitn(4, ' ');
        let (Some(turn), Some(player), Some(topic), Some(rest)) =
            (words.next(), words.next(), words.next(), words.next())
        else {
            continue;
        };
        if topic != "Military"
            || !rest.starts_with(HEADLINE)
            || player.strip_prefix('p').and_then(|p| p.parse().ok()) != Some(seat)
        {
            continue;
        }
        let Some(turn) = turn.strip_prefix('t').and_then(|t| t.parse().ok()) else {
            continue;
        };
        let Some((_, detail)) = rest.split_once(&format!("{TAG}: ")) else {
            continue;
        };
        let Some((units, bill)) = detail.split_once(" for ") else {
            continue;
        };
        let Some((from, to)) = units.split_once(" to ") else {
            continue;
        };
        let upgrade = Planned {
            turn,
            from: from.to_string(),
            to: to.to_string(),
            gold: number_after(bill, "").unwrap_or(0.0),
            defence: number_after(bill, "defence +").unwrap_or(0.0),
            strike: number_after(bill, "ranged strike +").unwrap_or(0.0),
        };
        if !planned
            .iter()
            .any(|p| p.turn == turn && p.from == upgrade.from && p.to == upgrade.to)
        {
            planned.push(upgrade);
        }
    }
    planned
}

/// The planned upgrades the board carried out: each matched to an upgrade
/// of the same kinds the observer saw on the next turn's look, which no
/// other planned upgrade has claimed.
fn carried_out<'a>(planned: &'a [Planned], seen: &[(u32, String, String)]) -> Vec<&'a Planned> {
    let mut used = vec![false; seen.len()];
    let mut done = Vec::new();
    for upgrade in planned {
        let found = seen.iter().enumerate().position(|(i, (turn, from, to))| {
            !used[i]
                && *turn == upgrade.turn + 1
                && civvis::reasoning::plain(from) == upgrade.from
                && civvis::reasoning::plain(to) == upgrade.to
        });
        if let Some(i) = found {
            used[i] = true;
            done.push(upgrade);
        }
    }
    done
}

/// Play one arm of one world.
fn play(
    recipe: &Recipe,
    arm: &str,
    world: usize,
    seed: u64,
    genes: &[Gene],
    genome: &[bool],
) -> World {
    let base = std::env::temp_dir()
        .join(format!(
            "one-upgrade-{}-{arm}-{seed}-{}",
            recipe.key,
            std::process::id()
        ))
        .to_string_lossy()
        .into_owned();
    let profile = profile(recipe, seed, base.clone());
    let players = profile.players;
    let genomes = vec![genome.to_vec(); players];
    let book: Rc<RefCell<(BTreeMap<usize, Seen>, usize)>> = Rc::default();
    let sink = Rc::clone(&book);
    GAME_OBSERVER.with(|observer| {
        *observer.borrow_mut() = Some(Box::new(move |g: &Game| {
            let mut book = sink.borrow_mut();
            book.1 = g.log.len();
            look(g, players, &mut book.0);
        }));
    });
    let rows = play_game(&profile, genes, world, seed, &genomes);
    GAME_OBSERVER.with(|observer| *observer.borrow_mut() = None);
    let row = rows
        .into_iter()
        .find(|row| row.kind == "game")
        .expect("one measured seat");
    let (mut seen, log_len) = std::mem::take(&mut *book.borrow_mut());
    let watch = seen
        .remove(&row.seat)
        .map(|seat| seat.watch)
        .unwrap_or_default();
    let planned = planned_upgrades(&format!("{base}.journal.{seed}.txt"), row.seat);
    let done = carried_out(&planned, &watch.seen);
    let pass: Vec<u32> = done.iter().map(|upgrade| upgrade.turn).collect();
    let pass_gold: f64 = done.iter().map(|upgrade| upgrade.gold).sum();
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
        pass,
        pass_gold,
        planned,
        watch,
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

fn share(part: u32, whole: u32) -> f64 {
    if whole == 0 {
        0.0
    } else {
        f64::from(part) / f64::from(whole)
    }
}

/// The columns of the paired table: name, reading, whether a 1.5-se mark is
/// printed.
type Column = (&'static str, fn(&World) -> f64, bool);

const COLUMNS: &[Column] = &[
    (
        "units lost",
        |w| f64::from(w.row.units_lost.unwrap_or(0)),
        true,
    ),
    (
        "units lost to majors",
        |w| f64::from(w.row.units_lost_to_majors.unwrap_or(0)),
        true,
    ),
    (
        "cities lost",
        |w| w.row.cities_lost.unwrap_or(0) as f64,
        true,
    ),
    (
        "cities taken",
        |w| w.row.cities_taken.unwrap_or(0) as f64,
        true,
    ),
    (
        "units killed",
        |w| f64::from(w.row.units_killed.unwrap_or(0)),
        true,
    ),
    ("city-strike kills", |w| w.watch.strike_kills as f64, true),
    ("score", |w| w.row.score as f64, true),
    ("last turn", |w| f64::from(w.row.turn), false),
    (
        "alive at the end",
        |w| f64::from(u8::from(w.row.alive)),
        true,
    ),
    ("pass upgrades carried out", |w| w.pass.len() as f64, false),
    ("pass Gold", |w| w.pass_gold, false),
    ("upgrades seen", |w| f64::from(w.watch.upgrades), true),
    (
        "tier-lift upgrades",
        |w| f64::from(w.watch.tier_lifts),
        true,
    ),
    (
        "defence 10+ short (share)",
        |w| share(w.watch.defence_short, w.watch.turns),
        true,
    ),
    (
        "strike 10+ short (share)",
        |w| share(w.watch.strike_short, w.watch.turns),
        true,
    ),
    (
        "either 10+ short (share)",
        |w| share(w.watch.either_short, w.watch.turns),
        true,
    ),
];

/// The report over every world given: per recipe, the pairs, what the new
/// pass did, the live gap in each arm, and gene on minus control.
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
        let on = of(GENE_ON);
        let pairs: Vec<(&World, &World)> = control
            .iter()
            .filter_map(|(seed, c)| on.get(seed).map(|o| (*c, *o)))
            .collect();
        out.push(String::new());
        out.push(format!(
            "== {recipe}: {} pairs, seeds {}..{} (control: the live genome; gene on: the same with {TAG})",
            pairs.len(),
            pairs.first().map_or(0, |(c, _)| c.seed),
            pairs.last().map_or(0, |(c, _)| c.seed),
        ));
        if pairs.is_empty() {
            continue;
        }
        let n = pairs.len() as f64;
        let diverged = pairs
            .iter()
            .filter(|(c, o)| c.fingerprint != o.fingerprint)
            .count();
        let silent: Vec<&(&World, &World)> =
            pairs.iter().filter(|(_, o)| o.pass.is_empty()).collect();
        let silent_diverged = silent
            .iter()
            .filter(|(c, o)| c.fingerprint != o.fingerprint)
            .count();
        out.push(format!(
            "   pairs diverged {diverged}/{}; of the {} pairs whose pass took nothing, diverged {silent_diverged}",
            pairs.len(),
            silent.len()
        ));
        let fired = pairs.len() - silent.len();
        let (taken, taken_se) = mean_se(
            &pairs
                .iter()
                .map(|(_, o)| o.pass.len() as f64)
                .collect::<Vec<_>>(),
        );
        let firsts: Vec<f64> = pairs
            .iter()
            .filter_map(|(_, o)| o.pass.first().map(|t| f64::from(*t)))
            .collect();
        let control_leaks = pairs.iter().filter(|(c, _)| !c.planned.is_empty()).count();
        let (planned, planned_se) = mean_se(
            &pairs
                .iter()
                .map(|(_, o)| o.planned.len() as f64)
                .collect::<Vec<_>>(),
        );
        out.push(format!(
            "   the new pass (gene on): carried out in {fired}/{} worlds, {taken:.2}±{taken_se:.2} upgrades a world (planned {planned:.2}±{planned_se:.2}), first at turn median {} (range {}..{}); control journal lines in {control_leaks} worlds",
            pairs.len(),
            median(firsts.clone()).map_or("-".to_string(), |m| format!("{m:.0}")),
            firsts.iter().copied().reduce(f64::min).map_or("-".to_string(), |m| format!("{m:.0}")),
            firsts.iter().copied().reduce(f64::max).map_or("-".to_string(), |m| format!("{m:.0}")),
        ));
        for arm in [CONTROL, GENE_ON] {
            let side: Vec<&World> = pairs
                .iter()
                .map(|(c, o)| if arm == CONTROL { *c } else { *o })
                .collect();
            let pooled = |f: fn(&Watch) -> u32| -> f64 {
                let part: u32 = side.iter().map(|w| f(&w.watch)).sum();
                let whole: u32 = side.iter().map(|w| w.watch.turns).sum();
                100.0 * share(part, whole)
            };
            let first_lift = median(
                side.iter()
                    .filter_map(|w| w.watch.first_tier_lift.map(f64::from))
                    .collect(),
            );
            out.push(format!(
                "   {arm:<8} the live gap, seat turns from t{GAP_FROM_TURN} holding a city ({} turns): defence 10+ short {:.1}%, strike {:.1}%, either {:.1}%; tier-lift upgrades in {}/{} worlds, first at turn median {}",
                side.iter().map(|w| w.watch.turns).sum::<u32>(),
                pooled(|w| w.defence_short),
                pooled(|w| w.strike_short),
                pooled(|w| w.either_short),
                side.iter().filter(|w| w.watch.tier_lifts > 0).count(),
                side.len(),
                first_lift.map_or("-".to_string(), |m| format!("{m:.0}")),
            ));
        }
        out.push(String::new());
        out.push(format!(
            "== {recipe}: gene ON minus control, paired by seed (mean ± se a world; * = |d| >= 1.5 se)"
        ));
        out.push(format!(
            "   {:<28} {:>16} {:>16} {:>16}",
            "", "control", "gene on", "on - control"
        ));
        for (name, read, mark) in COLUMNS {
            let (cm, cse) = mean_se(&pairs.iter().map(|(c, _)| read(c)).collect::<Vec<_>>());
            let (om, ose) = mean_se(&pairs.iter().map(|(_, o)| read(o)).collect::<Vec<_>>());
            let (d, dse) = mean_se(
                &pairs
                    .iter()
                    .map(|(c, o)| read(o) - read(c))
                    .collect::<Vec<_>>(),
            );
            let star = if *mark && dse > 0.0 && (d / dse).abs() >= 1.5 {
                "*"
            } else {
                " "
            };
            let fmt = |m: f64, se: f64| {
                if m.abs() < 1.0 && se < 1.0 {
                    format!("{m:.3}±{se:.3}")
                } else {
                    format!("{m:.2}±{se:.2}")
                }
            };
            out.push(format!(
                "   {name:<28} {:>16} {:>16} {:>15}{star}",
                fmt(cm, cse),
                fmt(om, ose),
                if d.abs() < 1.0 && dse < 1.0 {
                    format!("{d:+.3}±{dse:.3}")
                } else {
                    format!("{d:+.2}±{dse:.2}")
                }
            ));
        }
        let wins = |side: usize| {
            pairs
                .iter()
                .filter(|(c, o)| {
                    let w = if side == 0 { c } else { o };
                    w.row.win
                })
                .count()
        };
        out.push(format!(
            "   seat won: control {}/{}, gene on {}/{}; n = {n:.0}",
            wins(0),
            pairs.len(),
            wins(1),
            pairs.len()
        ));
        out.push(String::new());
        out.push(format!(
            "== {recipe}: per pair (seed: pass upgrades [turns] | control lost/killed/cities lost/strike kills/score/turn | gene on the same; = identical game)"
        ));
        let cells = |w: &World| {
            format!(
                "{}/{}/{}/{}/{}/{}",
                w.row.units_lost.unwrap_or(0),
                w.row.units_killed.unwrap_or(0),
                w.row.cities_lost.unwrap_or(0),
                w.watch.strike_kills,
                w.row.score,
                w.row.turn
            )
        };
        for (c, o) in &pairs {
            out.push(format!(
                "   {}: {} {:?} | {} | {}{}",
                c.seed,
                o.pass.len(),
                o.pass,
                cells(c),
                cells(o),
                if c.fingerprint == o.fingerprint {
                    " ="
                } else {
                    ""
                }
            ));
        }
        // What the pass upgraded, summed over the gene arm's worlds.
        let mut kinds: BTreeMap<String, [f64; 5]> = BTreeMap::new();
        for (_, o) in &pairs {
            let done = carried_out(&o.planned, &o.watch.seen);
            for upgrade in &o.planned {
                let cell = kinds
                    .entry(format!("{} to {}", upgrade.from, upgrade.to))
                    .or_default();
                cell[1] += 1.0;
                if done.iter().any(|d| std::ptr::eq(*d, upgrade)) {
                    cell[0] += 1.0;
                    cell[2] += upgrade.gold;
                    cell[3] += upgrade.defence;
                    cell[4] += upgrade.strike;
                }
            }
        }
        if !kinds.is_empty() {
            let mut kinds: Vec<(String, [f64; 5])> = kinds.into_iter().collect();
            kinds.sort_by(|a, b| b.1[0].total_cmp(&a.1[0]).then(a.0.cmp(&b.0)));
            out.push(format!(
                "== {recipe}: the pass's upgrades over the gene arm's worlds (carried out / planned; mean Gold, defence and strike lift of those carried out)"
            ));
            for (kind, [done, planned, gold, defence, strike]) in kinds.iter().take(16) {
                let per = done.max(1.0);
                out.push(format!(
                    "   {kind:<40} {done:>3.0} / {planned:<3.0} {:>5.0} Gold  defence +{:.1}  strike +{:.1}",
                    gold / per,
                    defence / per,
                    strike / per
                ));
            }
        }
    }
    out.iter()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
#[ignore = "whole Deity games, the gene off and on: run with --ignored --nocapture"]
fn one_upgrade_lifts_every_city_driver() {
    if let Ok(files) = std::env::var("CIVVIS_ONE_UPGRADE_ROWS") {
        let mut worlds: Vec<World> = Vec::new();
        for path in files.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            for line in text.lines().filter(|line| !line.trim().is_empty()) {
                worlds.push(serde_json::from_str(line).expect("a world parses"));
            }
        }
        worlds.sort_by_key(|w| (w.recipe.clone(), w.world, w.arm.clone()));
        let note = format!(
            "ONE-UPGRADE-REPORT: {} worlds read from {} file(s)",
            worlds.len(),
            files.split(',').filter(|p| !p.trim().is_empty()).count()
        );
        println!("{}", report(&worlds, &note));
        return;
    }
    let recipe = match std::env::var("CIVVIS_ONE_UPGRADE_RECIPE").as_deref() {
        Ok("rome") => &ROME,
        _ => &GRAN_COLOMBIA,
    };
    let n = env_number("CIVVIS_ONE_UPGRADE_WORLDS", 16) as usize;
    let first_seed = env_number("CIVVIS_ONE_UPGRADE_SEED", SEED);
    let (shard, shards) = std::env::var("CIVVIS_ONE_UPGRADE_SHARD")
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
    let path = std::env::var("CIVVIS_ONE_UPGRADE_GENOME")
        .expect("CIVVIS_ONE_UPGRADE_GENOME names a live genome header");
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let tags = live_genome_tags(&text).expect("the genome file parses");
    assert!(!tags.is_empty(), "{path} names no tags");
    let mut genes = gene_table();
    let unknown = apply_live_genome(&mut genes, &tags);
    let control: Vec<bool> = genes.iter().map(|gene| gene.default_on).collect();
    let index = genes
        .iter()
        .position(|gene| gene.tag == TAG)
        .expect("the gene is in this build");
    assert!(!control[index], "the control genome already plays {TAG}");
    let mut on = control.clone();
    on[index] = true;
    let arms: [(&str, Vec<bool>); 2] = [(CONTROL, control.clone()), (GENE_ON, on)];
    let genome_note = format!(
        "control genome {path} ({}): {} tags, {} genes on in this build, {} tag(s) it does not carry",
        if text.trim_start().starts_with('{') {
            "a genome header"
        } else {
            "a tag list: every gene it does not name is off"
        },
        tags.len(),
        control.iter().filter(|on| **on).count(),
        unknown.len()
    );
    let jobs: Vec<(usize, usize)> = (0..n)
        .filter(|world| world % shards == shard)
        .flat_map(|world| (0..arms.len()).map(move |arm| (world, arm)))
        .collect();
    let out = std::env::var("CIVVIS_ONE_UPGRADE_OUT").ok().map(|path| {
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
                        "ONE-UPGRADE-WORLD {} {} seed {} pass {} {:?} (planned {}) lost {} killed {} cities_lost {} strike_kills {} score {} turn {} short {}/{}/{} of {} fp {:016x} secs {:.0}",
                        played.recipe,
                        played.arm,
                        played.seed,
                        played.pass.len(),
                        played.pass,
                        played.planned.len(),
                        played.row.units_lost.unwrap_or(0),
                        played.row.units_killed.unwrap_or(0),
                        played.row.cities_lost.unwrap_or(0),
                        played.watch.strike_kills,
                        played.row.score,
                        played.row.turn,
                        played.watch.defence_short,
                        played.watch.strike_short,
                        played.watch.either_short,
                        played.watch.turns,
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
        "ONE-UPGRADE-REPORT: {} recipe, worlds {n} from seed {first_seed}, shard {shard}/{shards}, {:.0}s, build {}\n{genome_note}",
        recipe.key,
        started.elapsed().as_secs_f64(),
        std::env::var("GITHUB_SHA").unwrap_or_else(|_| "local".to_string()),
    );
    println!("{}", report(&worlds, &note));
}

/// The report pairs the arms by seed, reads the pass from the gene arm, and
/// prints the paired difference.
#[test]
fn the_report_pairs_the_arms_by_seed() {
    let row = |lost: u32| -> Row {
        let mut row: Row = serde_json::from_str(
            r#"{"kind":"game","game":0,"seed":7,"seat":1,"genome":"1","win":false,"winner":0,
                "victory":"score","turn":150,"score":300,"score_share":0.2,"rank":2,"cities":6,
                "alive":true,"secs":50.0}"#,
        )
        .expect("a row parses");
        row.units_lost = Some(lost);
        row.units_killed = Some(10);
        row.cities_lost = Some(1);
        row
    };
    let world = |arm: &str, seed: u64, lost: u32, pass: Vec<u32>, fingerprint: u64| World {
        recipe: "rome".to_string(),
        arm: arm.to_string(),
        world: (seed - 100) as usize,
        seed,
        fingerprint,
        pass,
        pass_gold: 0.0,
        planned: Vec::new(),
        watch: Watch {
            turns: 10,
            defence_short: 4,
            ..Watch::default()
        },
        row: row(lost),
    };
    let worlds = vec![
        world(CONTROL, 100, 10, vec![], 1),
        world(GENE_ON, 100, 6, vec![41, 77], 2),
        world(CONTROL, 101, 12, vec![], 3),
        world(GENE_ON, 101, 12, vec![], 3),
        world(GENE_ON, 102, 1, vec![9], 9),
    ];
    let text = report(&worlds, "note");
    assert!(text.contains("2 pairs, seeds 100..101"), "{text}");
    assert!(text.contains("pairs diverged 1/2"), "{text}");
    assert!(text.contains("carried out in 1/2 worlds"), "{text}");
    assert!(text.contains("first at turn median 41"), "{text}");
    // Units lost 6-10 and 12-12: a mean of -2.00, a standard error of 2.00.
    let line = text
        .lines()
        .find(|line| line.trim_start().starts_with("units lost "))
        .unwrap_or_else(|| panic!("no units lost line in\n{text}"));
    assert!(line.contains("-2.00±2.00"), "{line}");
    assert!(text.contains("defence 10+ short 40.0%"), "{text}");
}

/// The journal reader keeps the gene's lines for the measured seat only,
/// once per turn and kinds however many frames journaled them, and an
/// upgrade counts as carried out only when the next turn's look saw it.
#[test]
fn the_pass_is_read_from_the_seat_journal_and_matched_to_the_board() {
    let path = std::env::temp_dir().join(format!(
        "one-upgrade-journal-test-{}.txt",
        std::process::id()
    ));
    let line = |turn: u32, player: usize, topic: &str, units: &str| {
        format!(
            "t{turn} p{player} {topic} One upgrade lifts every city | {TAG}: {units} for 120 Gold \
             of 137 (+3 a turn); every city's defence +15, ranged strike +0"
        )
    };
    let text = [
        line(40, 1, "Military", "hoplite to pikeman"),
        line(40, 1, "Military", "hoplite to pikeman"),
        line(41, 2, "Military", "slinger to archer"),
        line(42, 1, "Strategy", "slinger to archer"),
        line(57, 1, "Military", "saka horse archer to crossbowman"),
        line(90, 1, "Military", "archer to crossbowman"),
        "t58 p1 Military Modernizing the army before the treasury is spent".to_string(),
    ]
    .join("\n");
    std::fs::write(&path, text).unwrap();
    let planned = planned_upgrades(path.to_str().unwrap(), 1);
    assert!(!path.exists(), "the journal file is removed once read");
    assert_eq!(planned.len(), 3, "{planned:?}");
    assert_eq!(
        planned[0],
        Planned {
            turn: 40,
            from: "hoplite".to_string(),
            to: "pikeman".to_string(),
            gold: 120.0,
            defence: 15.0,
            strike: 0.0,
        }
    );
    assert_eq!(planned[1].from, "saka horse archer");
    // The Pikeman and the Crossbowman were seen on the next turn's look; the
    // turn-90 plan was not carried out (no upgrade seen on turn 91).
    let seen = vec![
        (41, "hoplite".to_string(), "pikeman".to_string()),
        (
            58,
            "saka_horse_archer".to_string(),
            "crossbowman".to_string(),
        ),
        (95, "archer".to_string(), "crossbowman".to_string()),
    ];
    let done: Vec<u32> = carried_out(&planned, &seen)
        .iter()
        .map(|upgrade| upgrade.turn)
        .collect();
    assert_eq!(done, vec![40, 57]);
}
