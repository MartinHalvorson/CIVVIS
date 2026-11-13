//! `upgrade-card-for-the-backlog` on whole worlds, paired: the same seed with
//! the gene off and on, both sides of the trade printed.
//!
//! Ignored: it plays paired whole worlds. Run with
//! `cargo test --profile ci --lib upgrade_card_worlds -- --ignored --nocapture`
//! (`CIVVIS_UPGRADE_CARD_WORLDS` sets the seeds; 32 by default).
//!
//! The board is the out-teched Immortal shape of `priced_risk::driver_tests`
//! (PR #5196) and `assault_forecast::world_tests` (PR #5211): four players,
//! 44x28, Online, two city-states; the rivals play Immortal (start units and
//! combat) on King yields, keep the handicap and target Domination,
//! Domination and Science. Seat 0 plays the live lane, Score, without the
//! handicap, carrying `deploy/live-force-on.txt`, the live tactical genes and
//! the live seat's Gold and Score genes that file leaves out. The rival chairs
//! fight the way the live host does: each plays inside `HostTacticsSeat` with
//! the gate on against seat 0 (`--rival-host-tactics`, PR #5218). The game
//! ends at turn 170, the live Score lane's limit.
//!
//! Read at turns 100 and 150 (the first seat-0 turn at or past the mark):
//! treasury, the land army's upgradeable share (an unlocked successor),
//! whether the card is held, Science and Culture a turn, cities held and
//! founded, score, techs against the best rival, and the unit ledger. At the
//! end: units killed and lost (`Game::combat_ledger`), cities lost and taken,
//! upgrades bought and the strength they added.

use crate::ai::{AdvancedAi, Ai, VictoryTarget};
use crate::game::{Game, GameOptions};
use crate::host_tactics::{HostTactics, HostTacticsSeat};
use crate::name::Name;
use std::collections::BTreeMap;
use std::time::Instant;

/// The tactical genes the live seat carries beyond the repository's
/// force-on file (the operator's armed set, 2026-11-12; PR #5196's list).
const LIVE_TACTICS: &[&str] = &[
    "battle-planner-2",
    "safest-stand",
    "slots-survive-the-roll",
    "guns-stay-out-of-reach",
    "siege-train",
    "anvil",
];
/// The live Rome seat's Gold, upgrade and Score genes the repository's
/// force-on file leaves out (live run summaries of 2026-11-13).
const LIVE_GOLD: &[&str] = &[
    "parity-upgrades-before-it-buys",
    "gold-buys-the-backlog-builder",
    "gold-buys-the-idle-trader",
    "gold-buys-the-opening-builder",
    "gold-unit-buys-read-the-host-quote",
    "threatened-city-buys-its-defender-2",
    "culture-gold-buys-the-shrines",
    "score-seat-founds-early",
    "score-seat-takes-what-pays",
];
const SEED_BASE: u64 = 51_000;
const DEFAULT_SEEDS: u64 = 32;
const TURNS: u32 = 170;
const MARKS: [u32; 2] = [100, 150];
/// A world past this many seconds stops where it stands (`stopped`).
const BUDGET_SECS: u64 = 12 * 60;
const JOBS: usize = 4;
/// The experiment rounds on CI read the report off a failing test (nextest
/// prints a passing test's output to nobody). False once the report is in.
const REPORT_BY_PANIC: bool = true;

/// One reading of a world row.
type Read<'a> = &'a dyn Fn(&WorldRow) -> f64;

#[derive(Clone, Debug, Default)]
struct Mark {
    reached: bool,
    gold: f64,
    land: usize,
    upgradeable: usize,
    card: bool,
    science: f64,
    culture: f64,
    cities: usize,
    founded: usize,
    score: i64,
    techs: usize,
    rival_techs: usize,
    killed: u32,
    lost: u32,
    lost_to_majors: u32,
    upgrades: usize,
}

#[derive(Clone, Debug, Default)]
struct WorldRow {
    marks: [Mark; 2],
    killed: u32,
    lost: u32,
    lost_to_majors: u32,
    cities_lost: usize,
    cities_taken: usize,
    upgrades: usize,
    strength_bought: f64,
    upgrade_gold: f64,
    card_turns: usize,
    seat_turns: usize,
    alive: bool,
    winner: Option<usize>,
    turns: u32,
    secs: f64,
    stopped: bool,
}

fn seeds() -> u64 {
    std::env::var("CIVVIS_UPGRADE_CARD_WORLDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(DEFAULT_SEEDS)
}

fn measured_seat(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    let armed = include_str!("../../../../deploy/live-force-on.txt");
    for tag in armed
        .split(|c: char| c == ',' || c.is_whitespace())
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .chain(LIVE_TACTICS.iter().copied())
        .chain(LIVE_GOLD.iter().copied())
    {
        if let Some(gene) = crate::ai::gene(tag) {
            (gene.enable)(&mut ai);
        }
    }
    if on {
        ai.enable_upgrade_card_for_the_backlog();
    } else {
        ai.disable_upgrade_card_for_the_backlog();
    }
    ai
}

fn power(g: &Game, kind: Name) -> f64 {
    let spec = &g.rules.units[kind];
    spec.strength.max(spec.ranged_attack_strength())
}

/// Seat 0's land combat units, and those with an unlocked successor.
fn army(g: &Game) -> (usize, usize) {
    let mut land = 0;
    let mut upgradeable = 0;
    for uid in g.player_unit_ids(0) {
        let kind = g.units[&uid].kind;
        let spec = &g.rules.units[kind];
        if spec.class != "military"
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || power(g, kind) <= 0.0
        {
            continue;
        }
        land += 1;
        if g.unit_upgrade_target(0, kind).is_some() {
            upgradeable += 1;
        }
    }
    (land, upgradeable)
}

fn holds_card(g: &Game) -> bool {
    g.players[0]
        .policies
        .iter()
        .any(|card| matches!(card.as_str(), "professional_army" | "force_modernization"))
}

fn mark(g: &Game, upgrades: usize) -> Mark {
    let (land, upgradeable) = army(g);
    let cities = g.player_city_ids(0);
    let ledger = g.combat_ledger(0);
    Mark {
        reached: true,
        gold: g.players[0].gold,
        land,
        upgradeable,
        card: holds_card(g),
        science: cities.iter().map(|&cid| g.city_yields(cid).science).sum(),
        culture: cities.iter().map(|&cid| g.city_yields(cid).culture).sum(),
        cities: cities.len(),
        founded: g.cities.values().filter(|c| c.original_owner == 0).count(),
        score: g.score(0),
        techs: g.players[0].techs.len(),
        rival_techs: g
            .players
            .iter()
            .filter(|p| p.id != 0 && p.alive && !p.is_minor && !p.is_barbarian)
            .map(|p| p.techs.len())
            .max()
            .unwrap_or(0),
        killed: ledger.units_killed,
        lost: ledger.units_lost,
        lost_to_majors: ledger.units_lost_to_majors,
        upgrades,
    }
}

fn world(seed: u64, on: bool) -> WorldRow {
    let started = Instant::now();
    let mut g = Game::new_with(GameOptions {
        speed: "online".to_string(),
        difficulty: "immortal".to_string(),
        ai_yield_rung: Some("king".to_string()),
        handicap_exempt: [0].into_iter().collect(),
        ..GameOptions::new(4, 44, 28, seed, TURNS, 2)
    });
    let rivals = [
        VictoryTarget::Domination,
        VictoryTarget::Domination,
        VictoryTarget::Science,
    ];
    let mut majors = 0;
    let mut seats: Vec<HostTacticsSeat<AdvancedAi>> = (0..g.players.len())
        .map(|pid| {
            if pid == 0 {
                majors += 1;
                HostTacticsSeat::new(measured_seat(on), None)
            } else if g.players[pid].is_minor || g.players[pid].is_barbarian {
                HostTacticsSeat::new(AdvancedAi::new(), None)
            } else {
                let target = rivals[(majors - 1) % rivals.len()];
                majors += 1;
                HostTacticsSeat::new(
                    AdvancedAi::targeting(target),
                    Some(HostTactics {
                        seed,
                        victims: vec![0],
                        gate: true,
                    }),
                )
            }
        })
        .collect();
    g.set_fog_memory(seats.iter().any(|seat| seat.uses_player_observation()));
    g.set_war_ledger(false);
    let mut row = WorldRow::default();
    let mut owners: BTreeMap<u32, usize> = BTreeMap::new();
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
        if pid != 0 {
            seats[pid].take_turn(&mut g, pid);
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &crate::game::Action::EndTurn);
            }
            continue;
        }
        if !g.players[0].alive {
            break;
        }
        for (slot, at) in MARKS.iter().enumerate() {
            if g.turn >= *at && !row.marks[slot].reached {
                row.marks[slot] = mark(&g, row.upgrades);
            }
        }
        let kinds: BTreeMap<u32, Name> = g
            .player_unit_ids(0)
            .into_iter()
            .map(|uid| (uid, g.units[&uid].kind))
            .collect();
        seats[0].take_turn(&mut g, 0);
        row.seat_turns += 1;
        if holds_card(&g) {
            row.card_turns += 1;
        }
        for (uid, before) in kinds {
            let Some(unit) = g.units.get(&uid) else {
                continue;
            };
            if unit.owner != 0 || unit.kind == before {
                continue;
            }
            row.upgrades += 1;
            row.strength_bought += power(&g, unit.kind) - power(&g, before);
            row.upgrade_gold += g
                .unit_upgrade_price_in_formation(0, before, unit.kind, unit.formation)
                .map_or(0.0, |(gold, _)| gold);
        }
        if g.winner.is_none() && g.current == 0 {
            let _ = g.apply(0, &crate::game::Action::EndTurn);
        }
    }
    let ledger = g.combat_ledger(0);
    row.killed = ledger.units_killed;
    row.lost = ledger.units_lost;
    row.lost_to_majors = ledger.units_lost_to_majors;
    row.alive = g.players[0].alive;
    row.winner = g.winner;
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

/// One metric over the seeds: the off arm's mean, the on arm's, and the
/// paired difference (mean ± se) over the seeds where `keep` holds in both.
fn paired(rows: &[(WorldRow, WorldRow)], keep: &dyn Fn(&WorldRow) -> bool, read: Read) -> String {
    let both: Vec<&(WorldRow, WorldRow)> =
        rows.iter().filter(|(a, b)| keep(a) && keep(b)).collect();
    if both.is_empty() {
        return "no pair reached it".to_string();
    }
    let off: Vec<f64> = both.iter().map(|(a, _)| read(a)).collect();
    let on: Vec<f64> = both.iter().map(|(_, b)| read(b)).collect();
    let diffs: Vec<f64> = both.iter().map(|(a, b)| read(b) - read(a)).collect();
    let (d, se) = mean_se(&diffs);
    format!(
        "off {:>8.2} on {:>8.2}  diff {d:+.2} ± {se:.2}  (n={})",
        mean_se(&off).0,
        mean_se(&on).0,
        both.len()
    )
}

#[test]
fn upgrade_card_worlds() {
    let n = seeds();
    let started = Instant::now();
    let rows = crate::parallel::map(2 * n as usize, JOBS, |i| {
        world(SEED_BASE + i as u64 / 2, i % 2 == 1)
    });
    let pairs: Vec<(WorldRow, WorldRow)> = rows
        .chunks(2)
        .map(|pair| (pair[0].clone(), pair[1].clone()))
        .collect();
    let mut out = vec![format!(
        "UPGRADE-CARD-WORLDS: {n} seeds ({SEED_BASE}..{}), gene off vs on, seat 0 Score unhandicapped vs Immortal/King-yield rivals [Dom, Dom, Sci] under host tactics, 4p 44x28 Online, {TURNS} turns; {:.0}s",
        SEED_BASE + n - 1,
        started.elapsed().as_secs_f64()
    )];
    for (k, (a, b)) in pairs.iter().enumerate() {
        let seed = SEED_BASE + k as u64;
        for (arm, r) in [("off", a), ("on", b)] {
            let m = |slot: usize| {
                let x = &r.marks[slot];
                if !x.reached {
                    return "-".to_string();
                }
                format!(
                    "g{:.0} up{}/{} card{} sci{:.0} cul{:.0} c{}/{} sc{} t{}/{} k{} l{}",
                    x.gold,
                    x.upgradeable,
                    x.land,
                    u8::from(x.card),
                    x.science,
                    x.culture,
                    x.cities,
                    x.founded,
                    x.score,
                    x.techs,
                    x.rival_techs,
                    x.killed,
                    x.lost
                )
            };
            out.push(format!(
                "ROW {seed} {arm} killed={} lost={} majors={} clost={} ctaken={} upgrades={} strength={:.0} upgold={:.0} card_turns={}/{} alive={} winner={:?} turns={} secs={:.0} stopped={} | t100 {} | t150 {}",
                r.killed, r.lost, r.lost_to_majors, r.cities_lost, r.cities_taken, r.upgrades,
                r.strength_bought, r.upgrade_gold, r.card_turns, r.seat_turns, r.alive, r.winner,
                r.turns, r.secs, r.stopped, m(0), m(1)
            ));
        }
    }
    let all = |_: &WorldRow| true;
    let at = |slot: usize| move |r: &WorldRow| r.marks[slot].reached;
    out.push(String::new());
    out.push("Paired over seeds (on minus off):".to_string());
    let whole: [(&str, Read); 9] = [
        ("units killed", &|r| f64::from(r.killed)),
        ("units lost", &|r| f64::from(r.lost)),
        ("units lost to majors", &|r| f64::from(r.lost_to_majors)),
        ("cities lost", &|r| r.cities_lost as f64),
        ("cities taken", &|r| r.cities_taken as f64),
        ("upgrades bought", &|r| r.upgrades as f64),
        ("strength bought", &|r| r.strength_bought),
        ("seat turns holding the card", &|r| r.card_turns as f64),
        ("seat alive at the end", &|r| f64::from(u8::from(r.alive))),
    ];
    for (name, read) in whole {
        out.push(format!("  {name:<30} {}", paired(&pairs, &all, read)));
    }
    let killed: f64 = pairs.iter().map(|(a, _)| f64::from(a.killed)).sum();
    let lost: f64 = pairs.iter().map(|(a, _)| f64::from(a.lost)).sum();
    let killed_on: f64 = pairs.iter().map(|(_, b)| f64::from(b.killed)).sum();
    let lost_on: f64 = pairs.iter().map(|(_, b)| f64::from(b.lost)).sum();
    out.push(format!(
        "  {:<30} off {:.2} on {:.2} (pooled)",
        "kill:loss",
        killed / lost.max(1.0),
        killed_on / lost_on.max(1.0)
    ));
    for (slot, turn) in MARKS.iter().enumerate() {
        let keep = at(slot);
        out.push(format!("  at turn {turn}:"));
        let fields: [(&str, Read); 14] = [
            ("treasury", &move |r| r.marks[slot].gold),
            ("upgradeable share", &move |r| {
                let x = &r.marks[slot];
                x.upgradeable as f64 / x.land.max(1) as f64
            }),
            ("land units", &move |r| r.marks[slot].land as f64),
            ("card held", &move |r| {
                f64::from(u8::from(r.marks[slot].card))
            }),
            ("upgrades so far", &move |r| r.marks[slot].upgrades as f64),
            ("science a turn", &move |r| r.marks[slot].science),
            ("culture a turn", &move |r| r.marks[slot].culture),
            ("cities held", &move |r| r.marks[slot].cities as f64),
            ("cities founded", &move |r| r.marks[slot].founded as f64),
            ("score", &move |r| r.marks[slot].score as f64),
            ("techs", &move |r| r.marks[slot].techs as f64),
            ("best rival techs", &move |r| {
                r.marks[slot].rival_techs as f64
            }),
            ("units lost", &move |r| f64::from(r.marks[slot].lost)),
            ("units killed", &move |r| f64::from(r.marks[slot].killed)),
        ];
        for (name, read) in fields {
            out.push(format!("    {name:<28} {}", paired(&pairs, &keep, read)));
        }
        let majors: Vec<f64> = pairs
            .iter()
            .filter(|(a, b)| keep(a) && keep(b))
            .map(|(a, b)| {
                f64::from(b.marks[slot].lost_to_majors) - f64::from(a.marks[slot].lost_to_majors)
            })
            .collect();
        let (d, se) = mean_se(&majors);
        out.push(format!(
            "    {:<28} diff {d:+.2} ± {se:.2}",
            "units lost to majors"
        ));
    }
    let stopped = pairs.iter().filter(|(a, b)| a.stopped || b.stopped).count();
    let diverged = pairs
        .iter()
        .filter(|(a, b)| {
            a.turns != b.turns || a.upgrades != b.upgrades || a.marks[0].score != b.marks[0].score
        })
        .count();
    out.push(format!(
        "  pairs that diverged: {diverged} of {}; stopped pairs: {stopped}",
        pairs.len()
    ));
    let report = out.join("\n");
    println!("{report}");
    if REPORT_BY_PANIC {
        panic!("{report}");
    }
}
