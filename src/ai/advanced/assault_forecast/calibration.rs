//! The forecast's calibration and the go gene's paired read, on posed
//! sieges played out by the real AI on the engine.
//!
//! Each seed poses one siege on open ground (`world`): our capital in the
//! west, player 1's city at [`TARGET`] with 0/100/200/400 walls (sometimes
//! already damaged), its owner one era behind, level or ahead of ours (its
//! city defence follows its best unit), a garrison and field defenders or
//! none, and a force of 2-9 of our era's melee, shooters and guns between 2
//! and 10 tiles out. Our seat is the live genome as `siege_replay` seats it
//! (`live_seat`: the operator's arms of 2026-11-12 in
//! `live_force_on_20261112.txt` forced onto the deployment genome), aimed
//! at player 1 on the Domination lane; player 1 is the stock agent.
//!
//! - **Calibration**: the forecast is read before our first move, then the
//!   game plays [`HORIZON`] turns. Predicted P against the realized capture,
//!   in five bins, with the Brier score, under every [`FORECASTS`] setting.
//! - **Paired**: the seed with a second city of player 1's beyond the first,
//!   played [`PAIRED_TURNS`] turns twice, the live genome with and without
//!   `siege-goes-on-the-forecast`. A war that ends in peace takes nothing;
//!   the table is printed again over the pairs no arm made peace in.
//!
//! The rows are a fixed, repeatable problem set for this codebase's AI on
//! both sides, not a forecast of live play.
//!
//! ```text
//! cargo test --profile ci --lib -- --ignored assault_forecast_calibration --nocapture
//! ```

use super::*;
use crate::ai::advanced::siege_train::SiegeStage;
use crate::ai::advanced::VictoryTarget;
use crate::ai::Ai;
use crate::game::Action;
use crate::reasoning::Journal;
use crate::rng::Rng;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const LIVE_ARMS: &str = include_str!("live_force_on_20261112.txt");
const TARGET: Pos = (18, 8);
const HOME: Pos = (2, 8);
const CALIBRATION_SEEDS: u64 = 1200;
const PAIRED_SEEDS: u64 = 400;
const PAIRED_TURNS: u32 = 30;
/// The paired arms: the live genome alone, and with the go gene.
const ARMS: [(&str, bool); 2] = [("off", false), ("on", true)];
/// Forecast settings the calibration reads side by side.
const HALF: Knobs = Knobs {
    enemy_aggression: 0.5,
    ..KNOBS
};
const FORECASTS: [(&str, Knobs); 5] = [
    (
        "melee wait for the breach, enemy always acts (shipped)",
        KNOBS,
    ),
    (
        "melee wait for the breach, enemy acts three turns in four",
        Knobs {
            enemy_aggression: 0.75,
            ..KNOBS
        },
    ),
    ("melee wait for the breach, enemy acts half its turns", HALF),
    (
        "melee chip any wall, enemy always acts",
        Knobs {
            melee_breach_share: 0.0,
            ..KNOBS
        },
    ),
    (
        "melee chip any wall, enemy acts half its turns",
        Knobs {
            melee_breach_share: 0.0,
            enemy_aggression: 0.5,
        },
    ),
];
/// Seeds whose siege journal is printed, to show the harness plays a siege.
const TRACED: u64 = 2;

struct Era {
    melee: &'static [&'static str],
    ranged: &'static [&'static str],
    siege: &'static [&'static str],
    defenders: &'static [&'static str],
    /// The owner's strongest unit built, which the city's defence follows.
    best: i64,
}

const ERAS: [Era; 4] = [
    Era {
        melee: &["warrior", "spearman", "swordsman", "horseman"],
        ranged: &["slinger", "archer"],
        siege: &["catapult"],
        defenders: &["warrior", "spearman", "archer"],
        best: 35,
    },
    Era {
        melee: &["swordsman", "man_at_arms", "pikeman", "knight"],
        ranged: &["crossbowman"],
        siege: &["trebuchet"],
        defenders: &["pikeman", "crossbowman", "man_at_arms"],
        best: 50,
    },
    Era {
        melee: &["man_at_arms", "musketman", "pike_and_shot", "knight"],
        ranged: &["crossbowman", "field_cannon"],
        siege: &["bombard"],
        defenders: &["musketman", "crossbowman", "pike_and_shot"],
        best: 55,
    },
    Era {
        melee: &["musketman", "line_infantry", "cavalry"],
        ranged: &["field_cannon"],
        siege: &["artillery"],
        defenders: &["line_infantry", "field_cannon"],
        best: 65,
    },
];

/// One posed siege.
struct World {
    g: Game,
    cid: u32,
    /// Every city of player 1's: the posed one first.
    targets: Vec<Pos>,
    force: Vec<u32>,
    label: String,
    walls: i32,
}

fn free_land(g: &Game, pos: Pos) -> bool {
    g.map
        .get(pos)
        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        && g.city_at(pos).is_none()
        && g.unit_ids_at(pos).is_empty()
}

fn pick_ring(g: &Game, rng: &mut Rng, distance: i32) -> Option<Pos> {
    let mut tiles: Vec<Pos> = g
        .wring(TARGET, distance)
        .into_iter()
        .filter(|pos| free_land(g, *pos) && g.wdist(*pos, HOME) > 2)
        .collect();
    tiles.sort_unstable();
    (!tiles.is_empty()).then(|| tiles[rng.below(tiles.len())])
}

/// `world`, and for a paired seed a second city of player 1's about eight
/// tiles beyond the first, so a siege stood down has somewhere to go. Its
/// draws come from their own stream: the first city's siege is the same.
fn world(seed: u64, second: bool) -> World {
    let mut rng = Rng::new(seed ^ 0x51e6_e0f0_7ca5_7001);
    let era = rng.below(ERAS.len());
    let enemy_era = (era as i32 + [-1, 0, 0, 1][rng.below(4)]).clamp(0, 3) as usize;
    let walls = [0, 100, 200, 400][rng.below(4)];
    let mut g = Game::new_full(2, 36, 22, seed.wrapping_add(17), 400, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pos in positions {
        let near = (pos.0 - TARGET.0).abs().max((pos.1 - TARGET.1).abs()) <= 7;
        let roll = rng.f64();
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        if near && pos != TARGET {
            if roll < 0.14 {
                tile.hills = true;
            } else if roll < 0.26 {
                tile.feature = Some(crate::name!("forest"));
            }
        }
    }
    g.found_city_for(0, HOME, None);
    let cid = g.found_city_for(1, TARGET, None);
    let max_era = |era: usize| {
        g.rules
            .techs
            .iter()
            .filter(|(_, tech)| tech.era <= era)
            .map(|(name, _)| *name)
            .collect::<BTreeSet<Name>>()
    };
    let (ours, theirs) = (max_era(era), max_era(enemy_era));
    g.players[0].techs.extend(ours);
    g.players[1].techs.extend(theirs);
    let all: Vec<Pos> = g.map.tiles.keys().copied().collect();
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].explored.extend(all.iter().copied());
    }
    let damaged = walls > 0 && rng.chance(0.25);
    let wall_hp = if damaged {
        (f64::from(walls) * rng.uniform(0.2, 0.9)).round() as i32
    } else {
        walls
    };
    let city_hp = if rng.chance(0.3) {
        rng.randint(90, 199)
    } else {
        200
    };
    {
        let city = g.cities.get_mut(&cid).unwrap();
        if walls > 0 {
            city.buildings.push(crate::name!("walls"));
        }
        city.wall_hp = wall_hp;
        city.hp = city_hp;
    }
    Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, walls);
    let enemy = &ERAS[enemy_era];
    let mut defenders = Vec::new();
    if rng.chance(0.65) {
        let kind = enemy.defenders[rng.below(enemy.defenders.len())];
        g.spawn_test_unit(kind, 1, TARGET);
        defenders.push(kind);
    }
    for _ in 0..rng.below(3) {
        let distance = 1 + rng.below(3) as i32;
        if let Some(pos) = pick_ring(&g, &mut rng, distance) {
            let kind = enemy.defenders[rng.below(enemy.defenders.len())];
            g.spawn_test_unit(kind, 1, pos);
            defenders.push(kind);
        }
    }
    *g.players[1]
        .counters
        .entry("strongest_unit_built".to_string())
        .or_insert(0) = enemy.best;
    let ours = &ERAS[era];
    let size = 2 + rng.below(8);
    let mut force = Vec::new();
    let mut kinds = Vec::new();
    for _ in 0..size {
        let roll = rng.f64();
        let pool = if roll < 0.55 {
            ours.melee
        } else if roll < 0.8 {
            ours.ranged
        } else {
            ours.siege
        };
        let kind = pool[rng.below(pool.len())];
        let roll = rng.f64();
        let distance = if roll < 0.55 {
            2 + rng.below(3) as i32
        } else if roll < 0.85 {
            5 + rng.below(3) as i32
        } else {
            8 + rng.below(3) as i32
        };
        if let Some(pos) = pick_ring(&g, &mut rng, distance) {
            force.push(g.spawn_test_unit(kind, 0, pos));
            kinds.push(format!("{kind}@{distance}"));
        }
    }
    let mut targets = vec![TARGET];
    let mut beyond = String::new();
    if second {
        let mut rng = Rng::new(seed ^ 0x2c17_5ec0_0d0c_17e5);
        let spot = g
            .wring(TARGET, 8)
            .into_iter()
            .filter(|pos| {
                free_land(&g, *pos)
                    && g.wdisk(*pos, 1)
                        .iter()
                        .all(|n| free_land(&g, *n) || *n == *pos)
            })
            .max_by_key(|pos| (g.wdist(*pos, HOME), std::cmp::Reverse(*pos)));
        if let Some(spot) = spot {
            let other = g.found_city_for(1, spot, None);
            let walls = [0, 0, 100, 200][rng.below(4)];
            {
                let city = g.cities.get_mut(&other).unwrap();
                if walls > 0 {
                    city.buildings.push(crate::name!("walls"));
                }
                city.wall_hp = walls;
            }
            Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(other, walls);
            let garrison = rng.chance(0.5).then(|| {
                let kind = enemy.defenders[rng.below(enemy.defenders.len())];
                g.spawn_test_unit(kind, 1, spot);
                kind
            });
            targets.push(spot);
            beyond = format!(
                "; second city walls {walls}, defence {:.0}, garrison {}",
                g.city_strength(other),
                garrison.unwrap_or("none")
            );
        }
    }
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 60 + 35 * era as u32;
    let label = format!(
        "era {era} v {enemy_era}, walls {wall_hp}/{walls}, city {city_hp}, defence {:.0}, defenders [{}], force [{}]{beyond}",
        g.city_strength(cid),
        defenders.join(" "),
        kinds.join(" ")
    );
    World {
        g,
        cid,
        targets,
        force,
        label,
        walls,
    }
}

fn live_tags() -> BTreeSet<&'static str> {
    LIVE_ARMS
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .collect()
}

/// The live seat as `siege_replay` seats it: the Domination lane aimed at
/// player 1, the live-bridge universe, then the deployment genome with the
/// live arms forced on (`apply_gene_ledger_with_forced_live`), and the go
/// gene forced on top for the `go` arm.
fn live_seat(tags: &BTreeSet<&'static str>, go: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.retarget_against(VictoryTarget::Domination, 1);
    ai.enable_live_bridge_universe();
    let mut forced: Vec<&str> = tags.iter().copied().collect();
    if go {
        forced.push("siege-goes-on-the-forecast");
    }
    ai.apply_gene_ledger_with_forced_live(&forced);
    assert_eq!(ai.siege_goes_on_the_forecast, go, "the arm seats the gene");
    ai
}

#[derive(Clone, Debug, Default)]
struct Outcome {
    /// Turn of the first capture, 1 = this turn.
    captured: Option<u32>,
    /// Player 1's cities taken.
    cities: usize,
    /// Turn the posed city itself was taken: what its forecast is read
    /// against.
    posed: Option<u32>,
    /// Force members gone by the capture or the end.
    lost: usize,
    peace: bool,
    /// Furthest siege stage reached: 0 none, 1 stage, 2 invest, 3 reduce,
    /// 4 take, 5 hold.
    stage: u8,
    seconds: f64,
    /// The gene's journal lines: go orders, held aborts, stand-downs, and
    /// board turns whose Siege row read no capture.
    goes: u32,
    holds: u32,
    stand_downs: u32,
    zero_rows: u32,
    trace: Vec<String>,
}

fn stage_rank(stage: SiegeStage) -> u8 {
    match stage {
        SiegeStage::Stage => 1,
        SiegeStage::Invest => 2,
        SiegeStage::Reduce => 3,
        SiegeStage::Take => 4,
        SiegeStage::Hold => 5,
    }
}

fn play_world(w: &World, mut ours: AdvancedAi, turns: u32, trace: bool) -> Outcome {
    let clock = Instant::now();
    let mut g = w.g.clone();
    let journal = Journal::recording();
    ours.attach_journal(journal.handle());
    let mut cursor = 0;
    let start = g.turn;
    let mut others: Vec<AdvancedAi> = (0..g.players.len()).map(|_| AdvancedAi::new()).collect();
    // As `ai::run_game` plays a headless world.
    g.set_fog_memory(
        ours.uses_player_observation() || others.iter().any(Ai::uses_player_observation),
    );
    g.set_war_ledger(false);
    let mut out = Outcome::default();
    let mut steps = 0;
    while g.winner.is_none() && g.turn < start + turns && steps < 64 * turns {
        steps += 1;
        let pid = g.current;
        if pid == 0 {
            ours.take_turn(&mut g, 0);
        } else {
            others[pid].take_turn(&mut g, pid);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if pid == 0 {
            let delta = journal.since(cursor);
            cursor = delta.cursor;
            for thought in delta.thoughts.iter().filter(|t| t.player == 0) {
                let head = &thought.headline;
                if head.contains("the forecast goes") {
                    out.goes += 1;
                } else if head.contains("the forecast holds the train") {
                    out.holds += 1;
                } else if head.contains("the forecast reads it hopeless") {
                    out.stand_downs += 1;
                } else if head.starts_with("Siege rows rank on the forecast")
                    && thought.detail.contains(" 0.00 a turn")
                {
                    out.zero_rows += 1;
                }
                if trace
                    && out.trace.len() < 40
                    && (head.starts_with("Siege")
                        || head.starts_with("Assault forecast")
                        || head.starts_with("Grand strategy"))
                {
                    let detail: String = thought.detail.chars().take(160).collect();
                    out.trace
                        .push(format!("t{} {} | {}", thought.turn, head, detail));
                }
            }
            if let Some(siege) = ours.sieges.get(&w.cid) {
                out.stage = out.stage.max(stage_rank(siege.stage));
            }
            let taken = w
                .targets
                .iter()
                .filter(|pos| g.city_at(**pos).is_none_or(|cid| g.cities[&cid].owner == 0))
                .count();
            if taken > 0 && out.captured.is_none() {
                out.captured = Some(g.turn - start + 1);
            }
            if out.posed.is_none()
                && g.city_at(TARGET)
                    .is_none_or(|cid| g.cities[&cid].owner == 0)
            {
                out.posed = Some(g.turn - start + 1);
            }
            out.cities = taken;
            if taken == w.targets.len() {
                break;
            }
        }
        if !g.is_at_war(0, 1) {
            out.peace = true;
            break;
        }
    }
    out.lost = w
        .force
        .iter()
        .filter(|uid| g.units.get(uid).is_none_or(|unit| unit.owner != 0))
        .count();
    out.seconds = clock.elapsed().as_secs_f64();
    out
}

struct CalibrationRow {
    seed: u64,
    label: String,
    walls: i32,
    /// The shipped forecast with its short-of reading, then the curve
    /// under each [`FORECASTS`] entry.
    forecast: AssaultForecast,
    variants: Vec<AssaultForecast>,
    forecast_ms: f64,
    outcome: Outcome,
}

struct PairRow {
    seed: u64,
    label: String,
    /// The forecast before our first move, as the calibration reads it.
    forecast: AssaultForecast,
    /// One outcome per [`ARMS`] entry.
    arms: Vec<Outcome>,
}

enum Job {
    Calibrate(u64),
    Pair(u64),
}

enum Row {
    Calibrated(CalibrationRow),
    Paired(PairRow),
}

fn run(job: &Job, tags: &BTreeSet<&'static str>) -> Row {
    match job {
        Job::Calibrate(seed) => {
            let w = world(*seed, false);
            let clock = Instant::now();
            let forecast = AdvancedAi::new()
                .assault_forecast(&w.g, 0, w.cid, &w.force)
                .expect("a posed city at war reads");
            let forecast_ms = clock.elapsed().as_secs_f64() * 1000.0;
            let variants = FORECASTS
                .iter()
                .map(|(_, knobs)| {
                    super::FORECAST_KNOBS.with(|k| k.set(Some(*knobs)));
                    let read = AdvancedAi::new()
                        .assault_capture_curve(&w.g, 0, w.cid, &w.force)
                        .expect("a posed city at war reads");
                    super::FORECAST_KNOBS.with(|k| k.set(None));
                    read
                })
                .collect();
            let outcome = play_world(&w, live_seat(tags, false), HORIZON as u32, *seed < TRACED);
            Row::Calibrated(CalibrationRow {
                seed: *seed,
                label: w.label.clone(),
                walls: w.walls,
                forecast,
                variants,
                forecast_ms,
                outcome,
            })
        }
        Job::Pair(seed) => {
            let w = world(*seed, true);
            let forecast = AdvancedAi::new()
                .assault_forecast(&w.g, 0, w.cid, &w.force)
                .expect("a posed city at war reads");
            let trace = *seed < 100_000 + TRACED;
            let arms = ARMS
                .iter()
                .map(|(_, go)| play_world(&w, live_seat(tags, *go), PAIRED_TURNS, trace))
                .collect();
            Row::Paired(PairRow {
                seed: *seed,
                label: w.label.clone(),
                forecast,
                arms,
            })
        }
    }
}

fn mean(values: impl Iterator<Item = f64>) -> (f64, usize) {
    let (sum, n) = values.fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    (if n == 0 { 0.0 } else { sum / n as f64 }, n)
}

/// Mean and standard error of paired differences.
fn paired(values: &[f64]) -> (f64, f64) {
    let n = values.len() as f64;
    if values.is_empty() {
        return (0.0, 0.0);
    }
    let m = values.iter().sum::<f64>() / n;
    let var = values.iter().map(|v| (v - m).powi(2)).sum::<f64>() / (n - 1.0).max(1.0);
    (m, (var / n).sqrt())
}

fn taken_by(outcome: &Outcome, k: usize) -> f64 {
    f64::from(u8::from(
        outcome.posed.is_some_and(|turn| turn as usize <= k),
    ))
}

/// One forecast and what the seat it was read for did.
type Read<'a> = (&'a AssaultForecast, &'a Outcome);

fn calibration_table(out: &mut String, rows: &[Read], k: usize) {
    let edges = [0.0, 0.2, 0.4, 0.6, 0.8, 1.000_001];
    let _ = writeln!(
        out,
        "| P(capture within {k}) | sieges | mean predicted | realized | stock train invested |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|");
    for bin in 0..5 {
        let inside: Vec<&Read> = rows
            .iter()
            .filter(|row| {
                let p = f64::from(row.0.within(k));
                p >= edges[bin] && p < edges[bin + 1]
            })
            .collect();
        let (predicted, n) = mean(inside.iter().map(|row| f64::from(row.0.within(k))));
        let (realized, _) = mean(inside.iter().map(|row| taken_by(row.1, k)));
        let (invested, _) = mean(
            inside
                .iter()
                .map(|row| f64::from(u8::from(row.1.stage >= 2))),
        );
        let _ = writeln!(
            out,
            "| {:.1}-{:.1} | {n} | {predicted:.2} | {realized:.2} | {invested:.2} |",
            edges[bin],
            edges[bin + 1].min(1.0)
        );
    }
    let brier = mean(
        rows.iter()
            .map(|row| (f64::from(row.0.within(k)) - taken_by(row.1, k)).powi(2)),
    )
    .0;
    let base = mean(rows.iter().map(|row| taken_by(row.1, k))).0;
    let climatology = mean(rows.iter().map(|row| (base - taken_by(row.1, k)).powi(2))).0;
    let _ = writeln!(
        out,
        "\nBrier score within {k}: {brier:.3} (the base rate {base:.2} for every siege scores {climatology:.3})\n"
    );
}

fn paired_table(out: &mut String, title: &str, kept_pairs: &[&PairRow]) {
    let _ = writeln!(
        out,
        "## Paired, {title}: {} posed sieges x {PAIRED_TURNS} turns\n",
        kept_pairs.len()
    );
    let _ = writeln!(
        out,
        "| arm | cities taken | pairs ahead / behind off | invested | force units lost (vs off) | turns to first capture | go orders | held aborts | stand-downs | zero-rate row turns | s a siege |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|---|---|---|---|---|");
    for (a, (name, _)) in ARMS.iter().enumerate() {
        let taken: usize = kept_pairs.iter().map(|row| row.arms[a].cities).sum();
        let gained = kept_pairs
            .iter()
            .filter(|row| row.arms[a].cities > row.arms[0].cities)
            .count();
        let lost_caps = kept_pairs
            .iter()
            .filter(|row| row.arms[a].cities < row.arms[0].cities)
            .count();
        let invested = kept_pairs
            .iter()
            .filter(|row| row.arms[a].stage >= 2)
            .count();
        let (lost, _) = mean(kept_pairs.iter().map(|row| row.arms[a].lost as f64));
        let diffs: Vec<f64> = kept_pairs
            .iter()
            .map(|row| row.arms[a].lost as f64 - row.arms[0].lost as f64)
            .collect();
        let (lost_d, lost_se) = paired(&diffs);
        let (turn, captures) = mean(
            kept_pairs
                .iter()
                .filter_map(|row| row.arms[a].captured.map(f64::from)),
        );
        let sum =
            |f: fn(&Outcome) -> u32| -> u32 { kept_pairs.iter().map(|row| f(&row.arms[a])).sum() };
        let (secs, _) = mean(kept_pairs.iter().map(|row| row.arms[a].seconds));
        let _ = writeln!(
            out,
            "| {name} | {taken} | +{gained} / -{lost_caps} | {invested} | {lost:.2} ({lost_d:+.2} +/- {lost_se:.2}) | {turn:.1} ({captures}) | {} | {} | {} | {} | {secs:.2} |",
            sum(|o| o.goes),
            sum(|o| o.holds),
            sum(|o| o.stand_downs),
            sum(|o| o.zero_rows)
        );
    }
}

fn report(calibrated: &[CalibrationRow], pairs: &[PairRow], seconds: f64) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "ASSAULT FORECAST REPORT ({seconds:.0} s wall)\n");
    let kept: Vec<&CalibrationRow> = calibrated.iter().filter(|row| !row.outcome.peace).collect();
    let (ms, _) = mean(calibrated.iter().map(|row| row.forecast_ms));
    let max_ms = calibrated
        .iter()
        .map(|row| row.forecast_ms)
        .fold(0.0, f64::max);
    let (play_s, _) = mean(calibrated.iter().map(|row| row.outcome.seconds));
    let _ = writeln!(
        out,
        "## Calibration: {} posed sieges, {} left out for peace; forecast {ms:.0} ms mean, {max_ms:.0} ms max \
         (with the short-of variants); {play_s:.2} s a siege played\n",
        calibrated.len(),
        calibrated.len() - kept.len()
    );
    let reads: Vec<Read> = kept
        .iter()
        .map(|row| (&row.forecast, &row.outcome))
        .collect();
    for k in [HORIZON, GO_HORIZON, 5] {
        calibration_table(&mut out, &reads, k);
    }
    let (lost_p, _) = mean(kept.iter().map(|row| f64::from(row.forecast.units_lost)));
    let (lost_r, _) = mean(kept.iter().map(|row| row.outcome.lost as f64));
    let _ = writeln!(
        out,
        "Units lost within {HORIZON} turns: predicted {lost_p:.2}, realized {lost_r:.2} a siege (realized counts the posed force only)\n"
    );
    let _ = writeln!(out, "| walls | sieges | mean P(12) | realized by 12 |");
    let _ = writeln!(out, "|---|---|---|---|");
    for walls in [0, 100, 200, 400] {
        let inside: Vec<&&CalibrationRow> = kept.iter().filter(|row| row.walls == walls).collect();
        let (p, n) = mean(inside.iter().map(|row| f64::from(row.forecast.p())));
        let (r, _) = mean(inside.iter().map(|row| taken_by(&row.outcome, HORIZON)));
        let _ = writeln!(out, "| {walls} | {n} | {p:.2} | {r:.2} |");
    }
    let _ = writeln!(
        out,
        "\n| forecast settings | Brier within 12 | within 10 | within 5 | mean P(12) | sieges over 0.8, taken |"
    );
    let _ = writeln!(out, "|---|---|---|---|---|---|");
    for (v, (name, _)) in FORECASTS.iter().enumerate() {
        let reads: Vec<Read> = kept
            .iter()
            .map(|row| (&row.variants[v], &row.outcome))
            .collect();
        let brier = |k: usize| {
            mean(
                reads
                    .iter()
                    .map(|(f, o)| (f64::from(f.within(k)) - taken_by(o, k)).powi(2)),
            )
            .0
        };
        let (p, _) = mean(reads.iter().map(|(f, _)| f64::from(f.p())));
        let high: Vec<&Read> = reads.iter().filter(|(f, _)| f.p() >= 0.8).collect();
        let (high_taken, high_n) = mean(high.iter().map(|(_, o)| taken_by(o, HORIZON)));
        let _ = writeln!(
            out,
            "| {name} | {:.3} | {:.3} | {:.3} | {p:.2} | {high_n}, {high_taken:.2} |",
            brier(HORIZON),
            brier(GO_HORIZON),
            brier(5)
        );
    }
    let _ = writeln!(
        out,
        "\n| stock train reached | sieges | mean P(12) | realized by 12 |"
    );
    let _ = writeln!(out, "|---|---|---|---|");
    for (name, test) in [
        ("stage only", 1u8..=1),
        ("invest or further", 2u8..=5),
        ("no siege", 0u8..=0),
    ] {
        let inside: Vec<&&CalibrationRow> = kept
            .iter()
            .filter(|row| test.contains(&row.outcome.stage))
            .collect();
        let (p, n) = mean(inside.iter().map(|row| f64::from(row.forecast.p())));
        let (r, _) = mean(inside.iter().map(|row| taken_by(&row.outcome, HORIZON)));
        let _ = writeln!(out, "| {name} | {n} | {p:.2} | {r:.2} |");
    }
    let mut short: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for row in &kept {
        let entry = short
            .entry(row.forecast.short_of.map_or("unread", ShortOf::as_str))
            .or_default();
        entry.0 += 1;
        entry.1 += usize::from(row.outcome.captured.is_some());
    }
    let _ = writeln!(
        out,
        "\nShort of (sieges, of them taken within {HORIZON}): {}\n",
        short
            .iter()
            .map(|(k, (n, t))| format!("{k} {n} ({t})"))
            .collect::<Vec<_>>()
            .join(", ")
    );

    let peaceful: Vec<&PairRow> = pairs
        .iter()
        .filter(|row| row.arms.iter().all(|arm| !arm.peace))
        .collect();
    let everyone: Vec<&PairRow> = pairs.iter().collect();
    paired_table(
        &mut out,
        "every pair; a war that ends in peace takes nothing",
        &everyone,
    );
    paired_table(&mut out, "the pairs no arm made peace in", &peaceful);
    let kept_pairs = peaceful;
    for (a, title) in [
        (0, "the live genome alone"),
        (1, "the live genome with siege-goes-on-the-forecast"),
    ] {
        let reads: Vec<Read> = kept_pairs
            .iter()
            .map(|row| (&row.forecast, &row.arms[a]))
            .collect();
        let _ = writeln!(
            out,
            "\n### The paired sieges' forecasts against {title} ({} sieges)\n",
            reads.len()
        );
        for k in [HORIZON, GO_HORIZON] {
            calibration_table(&mut out, &reads, k);
        }
    }
    let _ = writeln!(out, "\n## Calibration rows\n");
    for row in calibrated {
        let _ = writeln!(
            out,
            "cal {} | P12 {:.2} P10 {:.2} P5 {:.2} lost {:.1} short {} | taken {:?} lost {} stage {} peace {} | {}",
            row.seed,
            row.forecast.p(),
            row.forecast.within(GO_HORIZON),
            row.forecast.within(5),
            row.forecast.units_lost,
            row.forecast.short_of.map_or("unread", ShortOf::as_str),
            row.outcome.captured,
            row.outcome.lost,
            row.outcome.stage,
            row.outcome.peace,
            row.label
        );
    }
    let _ = writeln!(
        out,
        "\n## Paired rows (taken turn / units lost / stage, by arm)\n"
    );
    for row in pairs {
        let arms: Vec<String> = row
            .arms
            .iter()
            .map(|o| {
                format!(
                    "{}x{}/{}/{}{}",
                    o.captured.map_or("-".to_string(), |t| t.to_string()),
                    o.cities,
                    o.lost,
                    o.stage,
                    if o.peace { " peace" } else { "" }
                )
            })
            .collect();
        let _ = writeln!(
            out,
            "pair {} | {} | {}",
            row.seed,
            arms.join(" | "),
            row.label
        );
    }
    let _ = writeln!(out, "\n## Traces\n");
    for row in calibrated
        .iter()
        .filter(|row| !row.outcome.trace.is_empty())
    {
        let _ = writeln!(out, "cal {}: {}", row.seed, row.label);
        for line in &row.outcome.trace {
            let _ = writeln!(out, "  {line}");
        }
    }
    for row in pairs.iter().filter(|row| !row.arms[1].trace.is_empty()) {
        for (a, (name, _)) in ARMS.iter().enumerate().take(2) {
            let _ = writeln!(out, "pair {} {name}: {}", row.seed, row.label);
            for line in &row.arms[a].trace {
                let _ = writeln!(out, "  {line}");
            }
        }
    }
    out
}

/// The report `docs/ASSAULT_FORECAST.md` quotes: about ten minutes on four
/// cores, so it runs only when asked for.
#[test]
#[ignore = "the calibration driver: run with --ignored (docs/ASSAULT_FORECAST.md)"]
fn assault_forecast_calibration() {
    let clock = Instant::now();
    let tags = live_tags();
    let mut jobs: Vec<Job> = (0..PAIRED_SEEDS)
        .map(|seed| Job::Pair(100_000 + seed))
        .collect();
    jobs.extend((0..CALIBRATION_SEEDS).map(Job::Calibrate));
    let next = AtomicUsize::new(0);
    let rows: Mutex<Vec<(usize, Row)>> = Mutex::new(Vec::new());
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 4);
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(job) = jobs.get(index) else {
                    break;
                };
                let row = run(job, &tags);
                rows.lock().unwrap().push((index, row));
            });
        }
    });
    let mut rows = rows.into_inner().unwrap();
    rows.sort_by_key(|(index, _)| *index);
    let mut calibrated = Vec::new();
    let mut pairs = Vec::new();
    for (_, row) in rows {
        match row {
            Row::Calibrated(row) => calibrated.push(row),
            Row::Paired(row) => pairs.push(row),
        }
    }
    calibrated.sort_by_key(|row| row.seed);
    pairs.sort_by_key(|row| row.seed);
    let text = report(&calibrated, &pairs, clock.elapsed().as_secs_f64());
    println!("{text}");
}
