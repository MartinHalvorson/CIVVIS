//! `risk-is-priced`: the paired on/off evidence, printed.
//!
//! Ignored: it plays arena positions, skirmishes and whole worlds. Run with
//! `cargo test --profile ci --lib priced_risk_paired -- --ignored --nocapture`.
//!
//! - **Arena** (`docs/DOCTRINE_ARENA.md`): every doctrine position, each seed
//!   played twice with the roles swapped, the gene's seat against the same
//!   seat without it. Two seats: the stock `advanced`, and a Domination seat
//!   carrying the live tactical genes (`LIVE_TACTICS`). Each seat's control
//!   (against itself) must read exactly zero, and gives the base rate of
//!   each role's losses and cities.
//! - **Skirmish** (`battle_bench`): the control and the treatment.
//! - **Worlds** (`EMPEROR`, `IMMORTAL`): a Domination seat that plays without
//!   the handicap against rivals that keep it, so it is out-teched and at
//!   war, carrying the repository's live force-on genome plus the live
//!   tactical genes; the same seed in five arms (`ARMS`: off, the full gene,
//!   and three ablations by allowance class through the test-only
//!   `priced_risk::ablation` switch). Read: combat units lost and killed
//!   (`Game::combat_ledger`), cities, refusals by class off the seat's
//!   journal, the `last_orders` census, and what arm B's refused units did
//!   instead.

use super::ablation;
use crate::ai::{AdvancedAi, Ai, VictoryTarget};
use crate::doctrine::{matched_position, MatchedPosition, POSITIONS};
use crate::elo::seat_ai;
use crate::game::{Action, Game, GameOptions};
use crate::reasoning::{Journal, Level};
use crate::skirmish::{matched_skirmish, SkirmishSetup};
use crate::Pos;
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
/// The positions the brief names, read at the full seed count.
const KEY_POSITIONS: &[&str] = &["the_relief", "the_reserve", "the_storming"];
const KEY_SEEDS: u64 = 48;
const OTHER_SEEDS: u64 = 12;
const CONTROL_SEEDS: u64 = KEY_SEEDS;
const SKIRMISH_SEEDS: u64 = 40;
const WORLD_SEEDS: u64 = 16;
const WORLD_TURNS: u32 = 200;
/// A world past this many seconds stops where it stands (`stopped`).
const WORLD_BUDGET_SECS: u64 = 16 * 60;
const JOBS: usize = 2;

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
    /// Units lost in each role, by arm.
    on_role_lost: [usize; 2],
    off_role_lost: [usize; 2],
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
        for role in 0..2 {
            row.on_role_lost[role] += m.a_by_role[role].losses;
            row.off_role_lost[role] += m.b_by_role[role].losses;
        }
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
    for (id, row) in rows {
        println!(
            "  {id:<18} units lost by role (role 0 / role 1): on {}/{}  off {}/{}",
            row.on_role_lost[0], row.on_role_lost[1], row.off_role_lost[0], row.off_role_lost[1]
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
#[ignore = "plays the paired arena; run with --ignored --nocapture"]
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
    let live_off = live_seat("advanced_domination");
    let live_on = format!("{live_off}+risk-is-priced");
    // The controls: each seat against itself. Zero by construction, and the
    // base rate each role's losses and cities are read against.
    for seat in ["advanced", live_off.as_str()] {
        let rows = arena(seat, seat, &control);
        print_arena(&format!("CONTROL {seat} vs itself"), &rows);
        for (_, row) in &rows {
            assert!(
                row.diffs.iter().all(|d| *d == 0.0),
                "the control must read zero"
            );
            assert_eq!(row.fires, 0);
        }
    }
    let rows = arena("advanced+risk-is-priced", "advanced", &positions);
    print_arena("advanced+risk-is-priced vs advanced", &rows);
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

/// One arm of the world ablation: off, or the gene reading only `classes`
/// (`priced_risk::ablation`).
#[derive(Clone, Copy)]
struct Arm {
    name: &'static str,
    on: bool,
    classes: u8,
}

const ARMS: [Arm; 5] = [
    Arm {
        name: "A off",
        on: false,
        classes: 0,
    },
    Arm {
        name: "B full",
        on: true,
        classes: ablation::ALL,
    },
    Arm {
        name: "C scout+heal",
        on: true,
        classes: ablation::SCOUT | ablation::HEALING,
    },
    Arm {
        name: "D C+hold-back",
        on: true,
        classes: ablation::SCOUT | ablation::HEALING | ablation::HOLD_BACK,
    },
    Arm {
        name: "E B-march",
        on: true,
        classes: ablation::ALL & !ablation::MARCH,
    },
];

/// The allowance class a refusal's mission label belongs to.
fn class_of(label: &str) -> &'static str {
    match label {
        "scout" => "scout",
        "healing" => "healing",
        "staging" | "muster" | "reserve" | "deter" => "hold-back",
        "march" | "recon" | "clear camp" => "march",
        "engaged" | "destroy" | "escort" => "fight",
        "defend" | "relieve" | "siege post" => "hold",
        _ => "?",
    }
}

const CLASSES: [&str; 6] = ["scout", "healing", "hold-back", "march", "fight", "hold"];

/// One refusal read off the journal: turn, unit, the tile refused, class.
#[derive(Clone, Debug)]
struct Refused {
    turn: u32,
    uid: u32,
    tile: Pos,
    class: &'static str,
}

/// Our seat's turn as the log and the board saw it.
#[derive(Clone, Debug, Default)]
struct SeatTurn {
    /// Our military units and where they stood when our turn began.
    start: BTreeMap<u32, Pos>,
    /// Where they stood when it ended.
    end: BTreeMap<u32, Pos>,
    /// Tiles each unit was ordered onto this turn.
    entered: BTreeMap<u32, Vec<Pos>>,
    /// Units that struck.
    struck: BTreeSet<u32>,
    /// Our and every hostile unit's tile and health when the turn began:
    /// two arms on the same board read the same.
    board: u64,
}

/// One world's reading for the measured seat.
#[derive(Debug, Default)]
struct WorldRow {
    ledger: crate::game::CombatLedger,
    cities_lost: usize,
    cities_taken: usize,
    cities_end: usize,
    techs: usize,
    rival_techs: usize,
    turns: u32,
    secs: f64,
    stopped: bool,
    refusals: BTreeMap<&'static str, usize>,
    refused: Vec<Refused>,
    step_outs: usize,
    truncated: u64,
    last_orders: BTreeMap<String, [u32; 2]>,
    /// By our turn number.
    turns_seen: BTreeMap<u32, SeatTurn>,
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

const EMPEROR: Shape = Shape {
    // Emperor rivals keep their handicap and the seat plays without it: the
    // seat is out-teched, and one rival plays for Domination.
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
    // The live-calibrated shape (`gene_screen --rival-rung -2`): Immortal
    // combat and start units with King yields, two Domination rivals and a
    // Science one. The seat is out-teched and at war.
    name: "immortal-king-yields",
    difficulty: "immortal",
    rival_yields: Some("king"),
    rivals: [
        VictoryTarget::Domination,
        VictoryTarget::Domination,
        VictoryTarget::Science,
    ],
};

fn hash_board(g: &Game) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut units: Vec<(u32, usize, Pos, i32)> = g
        .units
        .values()
        .filter(|unit| g.rules.units[unit.kind].class == "military")
        .map(|unit| (unit.id, unit.owner, unit.pos, unit.hp))
        .collect();
    units.sort_unstable();
    units.hash(&mut hasher);
    hasher.finish()
}

fn our_soldiers(g: &Game) -> BTreeMap<u32, Pos> {
    g.player_unit_ids(0)
        .into_iter()
        .filter(|uid| g.rules.units[g.units[uid].kind].class == "military")
        .map(|uid| (uid, g.units[&uid].pos))
        .collect()
}

fn world(shape: Shape, seed: u64, arm: Arm) -> WorldRow {
    ablation::set(arm.classes);
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
                measured_seat(arm.on)
            } else if g.players[pid].is_minor || g.players[pid].is_barbarian {
                AdvancedAi::new()
            } else {
                let target = shape.rivals[(majors - 1) % shape.rivals.len()];
                majors += 1;
                AdvancedAi::targeting(target)
            }
        })
        .collect();
    // The measured seat's journal, in every arm alike: a recording journal
    // plays the unit phase serially, so no arm differs from another by that.
    let journal = Journal::recording();
    journal.set_ceiling(Level::Decision);
    ais[0].attach_journal(journal.handle());
    let mut cursor = 0;
    let mut owners: BTreeMap<u32, usize> = BTreeMap::new();
    let mut row = WorldRow::default();
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
        }
        let pid = g.current;
        let before = (pid == 0).then(|| (g.log.len(), our_soldiers(&g), hash_board(&g)));
        ais[pid].take_turn(&mut g, pid);
        if let Some((log_start, start, board)) = before {
            let mut seen = SeatTurn {
                start,
                end: our_soldiers(&g),
                board,
                ..SeatTurn::default()
            };
            for (actor, action) in g.log.since(log_start.min(g.log.len())) {
                if *actor != 0 {
                    continue;
                }
                match action {
                    Action::Move { unit, to } | Action::MoveTo { unit, to } => {
                        seen.entered.entry(*unit).or_default().push(*to);
                    }
                    Action::Attack { unit, .. } | Action::Ranged { unit, .. } => {
                        seen.struck.insert(*unit);
                    }
                    _ => {}
                }
            }
            row.turns_seen.insert(g.turn, seen);
        }
        if g.winner.is_none() && g.current == pid {
            let _ = g.apply(pid, &Action::EndTurn);
        }
        if pid == 0 {
            let delta = journal.since(cursor);
            cursor = delta.cursor;
            row.truncated = delta.truncated_turns;
            for thought in delta.thoughts {
                let Some(rest) = thought.detail.strip_prefix("risk-is-priced: ") else {
                    continue;
                };
                let Some(label) = rest
                    .split_once("over the ")
                    .and_then(|(_, tail)| tail.split_once(" allowance"))
                    .map(|(label, _)| label)
                else {
                    row.step_outs += 1;
                    continue;
                };
                let class = class_of(label);
                *row.refusals.entry(class).or_default() += 1;
                // "The warrior 5 does not step to (19, 10): ..."
                let parsed =
                    thought
                        .headline
                        .split_once(" does not step to (")
                        .and_then(|(who, tail)| {
                            let uid = who.rsplit(' ').next()?.parse::<u32>().ok()?;
                            let (tile, _) = tail.split_once(')')?;
                            let (x, y) = tile.split_once(", ")?;
                            Some((uid, (x.parse::<i32>().ok()?, y.parse::<i32>().ok()?)))
                        });
                if let Some((uid, tile)) = parsed {
                    row.refused.push(Refused {
                        turn: thought.turn,
                        uid,
                        tile,
                        class,
                    });
                }
            }
        }
    }
    ais[0].settle_last_orders(&g, 0);
    row.last_orders = ais[0].strategy_census().last_orders.map();
    row.ledger = g.combat_ledger(0);
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
#[ignore = "plays the Emperor world ablation; run with --ignored --nocapture"]
fn priced_risk_paired_worlds_emperor() {
    worlds(EMPEROR);
}

#[test]
#[ignore = "plays the Immortal world ablation; run with --ignored --nocapture"]
fn priced_risk_paired_worlds_immortal() {
    worlds(IMMORTAL);
}

/// Did `uid` die within two of our turns after `turn`: gone from our units
/// when our turn `turn + 1` or `turn + 2` began, or when the world ended.
fn died_within_two(turns: &BTreeMap<u32, SeatTurn>, turn: u32, uid: u32) -> bool {
    let later: Vec<&SeatTurn> = (turn + 1..=turn + 2)
        .filter_map(|t| turns.get(&t))
        .collect();
    later.iter().any(|seen| !seen.start.contains_key(&uid))
        || (later.is_empty()
            && turns
                .get(&turn)
                .is_some_and(|seen| !seen.end.contains_key(&uid)))
}

fn worlds(shape: Shape) {
    let jobs: Vec<(u64, usize)> = (0..WORLD_SEEDS)
        .flat_map(|seed| (0..ARMS.len()).map(move |arm| (seed, arm)))
        .collect();
    let rows = crate::parallel::map(jobs.len(), JOBS, |i| {
        let (seed, arm) = jobs[i];
        world(shape, 9_100 + seed, ARMS[arm])
    });
    let row = |seed: u64, arm: usize| &rows[seed as usize * ARMS.len() + arm];
    println!(
        "\n== worlds {}: {} (rival yields {}), seat 0 plays Domination unhandicapped with the live genome, 4p 44x28 Online, {WORLD_TURNS} turns, {WORLD_SEEDS} seeds ==",
        shape.name,
        shape.difficulty,
        shape.rival_yields.unwrap_or("the rung's")
    );
    println!("per seed, arm A off: lost(majors/city-states/barbarians) killed cities -lost/+taken end techs rival turns secs; then each arm's lost/killed/-lost/+taken");
    for seed in 0..WORLD_SEEDS {
        let a = row(seed, 0);
        let mut line = format!(
            "{seed:>3} A {:>3}({}/{}/{}) {:>3} -{}/+{} {:>2} {:>2}/{:>2} t{:<3} {:>3.0}s{}",
            a.ledger.units_lost,
            a.ledger.units_lost_to_majors,
            a.ledger.units_lost_to_city_states,
            a.ledger.units_lost_to_barbarians,
            a.ledger.units_killed,
            a.cities_lost,
            a.cities_taken,
            a.cities_end,
            a.techs,
            a.rival_techs,
            a.turns,
            a.secs,
            if a.stopped { " STOPPED" } else { "" }
        );
        for (arm, spec) in ARMS.iter().enumerate().skip(1) {
            let r = row(seed, arm);
            line.push_str(&format!(
                " | {} {}/{} -{}/+{}{}",
                &spec.name[..1],
                r.ledger.units_lost,
                r.ledger.units_killed,
                r.cities_lost,
                r.cities_taken,
                if r.stopped { " STOPPED" } else { "" }
            ));
        }
        println!("{line}");
    }
    println!(
        "\n{:<14} {:>15} {:>15} {:>15} {:>15} {:>15} {:>15} | refusals {:?}",
        "arm",
        "lost",
        "lost to majors",
        "lost to barbs",
        "killed",
        "cities lost",
        "cities taken",
        CLASSES
    );
    for (arm, spec) in ARMS.iter().enumerate() {
        let stat = |read: &dyn Fn(&WorldRow) -> f64| -> String {
            if arm == 0 {
                let total: f64 = (0..WORLD_SEEDS).map(|seed| read(row(seed, 0))).sum();
                format!("{:.2}/world", total / WORLD_SEEDS as f64)
            } else {
                let diffs: Vec<f64> = (0..WORLD_SEEDS)
                    .map(|seed| read(row(seed, arm)) - read(row(seed, 0)))
                    .collect();
                let (mean, se) = mean_se(&diffs);
                format!("{mean:+.2}±{se:.2}")
            }
        };
        let refusals: Vec<usize> = CLASSES
            .iter()
            .map(|class| {
                (0..WORLD_SEEDS)
                    .map(|seed| row(seed, arm).refusals.get(class).copied().unwrap_or(0))
                    .sum()
            })
            .collect();
        let step_outs: usize = (0..WORLD_SEEDS).map(|seed| row(seed, arm).step_outs).sum();
        println!(
            "{:<14} {:>15} {:>15} {:>15} {:>15} {:>15} {:>15} | {refusals:?} gun step-outs {step_outs}",
            spec.name,
            stat(&|r| f64::from(r.ledger.units_lost)),
            stat(&|r| f64::from(r.ledger.units_lost_to_majors)),
            stat(&|r| f64::from(r.ledger.units_lost_to_barbarians)),
            stat(&|r| f64::from(r.ledger.units_killed)),
            stat(&|r| r.cities_lost as f64),
            stat(&|r| r.cities_taken as f64),
        );
    }
    let truncated: u64 = rows.iter().map(|r| r.truncated).sum();
    let stopped = rows.iter().filter(|r| r.stopped).count();
    println!("(rows after A are paired on-off against A, mean ± se over seeds; journal-truncated turns {truncated}; stopped worlds {stopped})");

    // Which step last ordered each unit that died, [ended within two of a
    // visible hostile, beyond], summed over the seeds, for every arm.
    let mut steps: BTreeSet<String> = BTreeSet::new();
    let mut census: Vec<BTreeMap<String, [u32; 2]>> = vec![BTreeMap::new(); ARMS.len()];
    for (arm, total) in census.iter_mut().enumerate() {
        for seed in 0..WORLD_SEEDS {
            for (step, pair) in &row(seed, arm).last_orders {
                steps.insert(step.clone());
                let slot = total.entry(step.clone()).or_default();
                slot[0] += pair[0];
                slot[1] += pair[1];
            }
        }
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

    // Arm B's refusals: what the refused unit did that turn instead, and
    // whether it died within two of our turns; against arm A's same unit on
    // the same turn where A ordered it onto the refused tile (and where the
    // two arms' boards were still the same when that turn began).
    let mut instead: BTreeMap<&str, BTreeMap<&str, [usize; 2]>> = BTreeMap::new();
    let (mut matched, mut matched_dead, mut same_board, mut same_board_dead) = (0, 0, 0, 0);
    let (mut b_matched_dead, mut b_same_board_dead) = (0, 0);
    let mut first_of: BTreeSet<(u64, u32, u32)> = BTreeSet::new();
    for seed in 0..WORLD_SEEDS {
        let b = row(seed, 1);
        let a = row(seed, 0);
        for refusal in &b.refused {
            // One reading per unit and turn: its first refusal.
            if !first_of.insert((seed, refusal.turn, refusal.uid)) {
                continue;
            }
            let Some(seen) = b.turns_seen.get(&refusal.turn) else {
                continue;
            };
            let what = if !seen.end.contains_key(&refusal.uid) {
                "died on our turn"
            } else if seen.struck.contains(&refusal.uid) {
                "struck"
            } else if seen.end.get(&refusal.uid) != seen.start.get(&refusal.uid) {
                "moved elsewhere"
            } else {
                "stood still"
            };
            let died = died_within_two(&b.turns_seen, refusal.turn, refusal.uid);
            let cell = instead
                .entry(refusal.class)
                .or_default()
                .entry(what)
                .or_default();
            cell[0] += 1;
            cell[1] += usize::from(died);
            let Some(seen_a) = a.turns_seen.get(&refusal.turn) else {
                continue;
            };
            let made_the_move = seen_a
                .entered
                .get(&refusal.uid)
                .is_some_and(|tiles| tiles.contains(&refusal.tile));
            if !made_the_move {
                continue;
            }
            let a_died = died_within_two(&a.turns_seen, refusal.turn, refusal.uid);
            matched += 1;
            matched_dead += usize::from(a_died);
            b_matched_dead += usize::from(died);
            if seen_a.board == seen.board {
                same_board += 1;
                same_board_dead += usize::from(a_died);
                b_same_board_dead += usize::from(died);
            }
        }
    }
    println!("\narm B refusals (first per unit and turn): what the unit did instead that turn, [count, died within two of our turns]:");
    for (class, rows) in &instead {
        println!("  {class:<10} {rows:?}");
    }
    println!(
        "matched in arm A (same unit, same turn, A ordered it onto the refused tile): {matched}; died within two turns: A {matched_dead}, B {b_matched_dead}"
    );
    println!(
        "  of which the two boards were the same when the turn began: {same_board}; died within two turns: A {same_board_dead}, B {b_same_board_dead}"
    );
}
