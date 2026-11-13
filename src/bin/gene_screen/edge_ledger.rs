//! ⭐ The edge ledger on the screen's row, pooled and read on−off
//! (2026-11-13).
//!
//! Each row carries the seat's `EdgeLedger` (`game/combat_ledger.rs`): every
//! unit-on-unit blow its units took and struck, binned by base gap (the
//! ruleset's attack strength minus the target's Combat strength) and by total
//! gap (the two strengths the blow was resolved with), and the majors' mean
//! base gap by turn bucket. At Online-equivalent turns 50, 100 and 150 it also
//! carries an [`EdgeMark`]: the seat's land army, how much of it could upgrade,
//! its treasury, and the Combat strength of the rival majors' land units in
//! its sight. All of it is observation: the frozen anchor
//! (`advanced_v1_plays_the_same_game_it_always_did`) is unchanged.
//!
//! `--analyze` prints the tables pooled over the measured rows, in the layout
//! `tools/civ6_edge_ledger.py` prints for live runs, then the on−off Δ of
//! seven blow counts per gene. [`metrics`] is the short list both sides print
//! for a side-by-side read (`docs/EDGE_LEDGER.md`).

use super::{pace_abs_z, pace_cell, pace_json, GeneEstimate, Row};
use civvis::game::{
    EdgeCell, EdgeLedger, EdgeTable, Game, Unit, EDGE_BINS, EDGE_SIDES, EDGE_TURN_BUCKETS,
};
use civvis::setup::GameSpeed;
use serde::{Deserialize, Serialize};

/// The marks of (c) and (d), in Online-equivalent turns.
pub(super) const MARKS: [u32; 3] = [50, 100, 150];

/// One major at one mark: its land army and what that army could become, its
/// treasury, and the rival majors' land units it could see.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct EdgeMark {
    /// The Online-equivalent mark, and the raw turn it was read on.
    pub(super) mark: u32,
    pub(super) turn: u32,
    /// Military land units with a Combat strength.
    pub(super) army: u32,
    /// Of `army`: with a successor our techs and civics unlock
    /// (`Game::unit_upgrade_target`); of those, with the furthest unlocked
    /// successor's strategic resource in stock; and the units
    /// `Game::unit_gold_upgrade_offer` would upgrade on the seat's own turn
    /// (territory, Gold and material).
    pub(super) unlocked: u32,
    pub(super) with_material: u32,
    pub(super) offer: u32,
    /// Combat strength gained, summed over `unlocked`: the furthest unlocked
    /// successor's minus the unit's own (the host offers the furthest).
    pub(super) gain_sum: f64,
    pub(super) gold: f64,
    pub(super) gold_per_turn: f64,
    /// Combat strength summed over `army`.
    pub(super) our_strength_sum: f64,
    /// The rival majors' military land units this seat sees, and their
    /// Combat strength summed.
    pub(super) rival_units: u32,
    pub(super) rival_strength_sum: f64,
}

/// `(mark, raw turn)` for each of [`MARKS`] on this speed's clock.
pub(super) fn due_marks(speed: GameSpeed) -> Vec<(u32, u32)> {
    MARKS
        .iter()
        .map(|&mark| (mark, (mark * speed.cost_percent()).div_ceil(50)))
        .collect()
}

fn land_army(g: &Game, unit: &Unit) -> bool {
    let spec = &g.rules.units[unit.kind];
    spec.class == "military"
        && matches!(spec.domain.as_deref(), None | Some("land"))
        && spec.strength > 0.0
}

/// Every living major's [`EdgeMark`] at `mark`. The screen's observer runs
/// once a turn, before most seats have moved, so the units of every seat but
/// the first still carry last turn's moves; the offer is read on a copy of
/// the board with every unit fresh, as at the start of its owner's turn.
pub(super) fn observe(g: &Game, majors: &[usize], mark: u32) -> Vec<(usize, EdgeMark)> {
    let mut fresh = g.clone();
    for unit in fresh.units.values_mut() {
        unit.acted = false;
        unit.moves_left = unit.moves_left.max(1.0);
    }
    majors
        .iter()
        .filter(|&&pid| g.players[pid].alive)
        .map(|&pid| (pid, sample(g, &fresh, pid, majors, mark)))
        .collect()
}

fn sample(g: &Game, fresh: &Game, seat: usize, majors: &[usize], mark: u32) -> EdgeMark {
    let rules = &g.rules;
    let mut out = EdgeMark {
        mark,
        turn: g.turn,
        gold: g.players[seat].gold,
        gold_per_turn: g.players[seat].gold_per_turn,
        ..EdgeMark::default()
    };
    for unit in g.units.values() {
        if unit.owner != seat || !land_army(g, unit) {
            continue;
        }
        let strength = rules.units[unit.kind].strength;
        out.army += 1;
        out.our_strength_sum += strength;
        let Some(mut furthest) = g.unit_upgrade_target(seat, unit.kind) else {
            continue;
        };
        out.unlocked += 1;
        for _ in 0..8 {
            match g.unit_upgrade_target(seat, furthest) {
                Some(next) if next != furthest => furthest = next,
                _ => break,
            }
        }
        let successor = &rules.units[furthest];
        out.gain_sum += successor.strength - strength;
        if successor
            .requires_resource
            .is_none_or(|resource| g.strategic_stockpile(seat, resource) >= successor.resource_cost)
        {
            out.with_material += 1;
        }
        if fresh.unit_gold_upgrade_offer(seat, unit.id).is_some() {
            out.offer += 1;
        }
    }
    for unit in g.units.values() {
        if unit.owner != seat
            && majors.contains(&unit.owner)
            && land_army(g, unit)
            && g.player_can_see(seat, unit.pos)
            && g.unit_visible_to(unit.id, seat)
        {
            out.rival_units += 1;
            out.rival_strength_sum += rules.units[unit.kind].strength;
        }
    }
    out
}

// --------------------------------------------------------------- the on−off read

/// How a column is read off one row's ledger.
type Read = fn(&EdgeLedger) -> f64;

fn blows(cells: &[EdgeCell]) -> f64 {
    cells.iter().map(|cell| f64::from(cell.blows)).sum()
}

fn every_side(table: &EdgeTable, read: fn(&[EdgeCell; 5]) -> f64) -> f64 {
    table.iter().map(read).sum()
}

/// The counts `--analyze` reads on−off, per game. Gaps are read as counts,
/// never as per-row means, so a row with no blow is a zero and not a gap in
/// the column.
pub(super) const COLUMNS: [(&str, Read); 7] = [
    ("major_blows_taken", |e| blows(&e.taken_base[0])),
    ("major_blows_taken_10", |e| {
        f64::from(e.taken_base[0][4].blows)
    }),
    ("lost_to_major_blows_10", |e| {
        f64::from(e.taken_base[0][4].defender_killed)
    }),
    ("blows_struck", |e| {
        every_side(&e.struck_total, |row| blows(row))
    }),
    ("blows_struck_low", |e| {
        every_side(&e.struck_total, |row| f64::from(row[0].blows))
    }),
    ("blows_struck_high", |e| {
        every_side(&e.struck_total, |row| f64::from(row[4].blows))
    }),
    ("kills_struck_high", |e| {
        every_side(&e.struck_total, |row| f64::from(row[4].defender_killed))
    }),
];

/// Each column over the screened rows, `Some` only when every row carries
/// the ledger: a file written before it reads as no measurement.
pub(super) fn outcomes(rows: &[&Row]) -> Vec<Option<Vec<f64>>> {
    COLUMNS
        .iter()
        .map(|(_, read)| {
            rows.iter()
                .map(|row| row.edge.as_ref().map(read))
                .collect::<Option<Vec<f64>>>()
                .filter(|values| !values.is_empty())
        })
        .collect()
}

/// The pooled tables of the measured rows, then the on−off Δ table, only
/// when the rows carry the ledger.
pub(super) fn print(genes: &[GeneEstimate], rows: &[Row]) {
    let measured: Vec<&Row> = rows.iter().filter(|row| row.kind == "game").collect();
    if let Some(tables) = render(&measured) {
        println!("\nedge ledger, pooled over the measured rows (docs/EDGE_LEDGER.md)\n{tables}");
    }
    if !genes
        .iter()
        .any(|e| e.edge_ledger.first().copied().flatten().is_some())
    {
        return;
    }
    let mut by_high: Vec<&GeneEstimate> = genes.iter().collect();
    by_high.sort_by(|a, b| pace_abs_z(b.edge_ledger[1]).total_cmp(&pace_abs_z(a.edge_ledger[1])));
    println!(
        "\n{:<28} {:>14} {:>7}  {:>14} {:>7}  {:>14} {:>7}",
        "edge ledger: blows taken", "majorΔ", "z", "major >=+10Δ", "z", "lost >=+10Δ", "z"
    );
    for e in &by_high {
        println!(
            "{:<28} {}  {}  {}",
            e.tag,
            pace_cell(e.edge_ledger[0]),
            pace_cell(e.edge_ledger[1]),
            pace_cell(e.edge_ledger[2]),
        );
    }
    println!(
        "\n{:<28} {:>14} {:>7}  {:>14} {:>7}  {:>14} {:>7}  {:>14} {:>7}",
        "edge ledger: blows struck", "allΔ", "z", "<=-10Δ", "z", ">=+10Δ", "z", "kills >=+10Δ", "z"
    );
    for e in &by_high {
        println!(
            "{:<28} {}  {}  {}  {}",
            e.tag,
            pace_cell(e.edge_ledger[3]),
            pace_cell(e.edge_ledger[4]),
            pace_cell(e.edge_ledger[5]),
            pace_cell(e.edge_ledger[6]),
        );
    }
    println!(
        "edge ledger cells are seats-on minus seats-off in blows per game ± one clustered \
         standard error, then z; taken by base gap from a major, struck by total gap at any \
         unit (`game/combat_ledger.rs`)"
    );
}

/// The ledger's contrasts as the analysis JSON carries them.
pub(super) fn json(e: &GeneEstimate) -> serde_json::Value {
    COLUMNS
        .iter()
        .zip(&e.edge_ledger)
        .map(|((name, _), estimate)| (name.to_string(), pace_json(*estimate, e.n_on, e.n_off)))
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into()
}

// ------------------------------------------------------------------ pooling

/// The ledgers and marks of a set of rows, summed.
#[derive(Default)]
pub(super) struct Pool {
    pub(super) rows: usize,
    pub(super) edge: EdgeLedger,
    pub(super) marks: Vec<EdgeMark>,
    pub(super) units_killed: u64,
    pub(super) units_lost: u64,
    pub(super) cities_lost: u64,
}

impl Pool {
    pub(super) fn of(rows: &[&Row]) -> Option<Pool> {
        let mut pool = Pool::default();
        for row in rows {
            let Some(edge) = &row.edge else {
                continue;
            };
            pool.rows += 1;
            for (into, from) in [
                (&mut pool.edge.taken_base, &edge.taken_base),
                (&mut pool.edge.taken_total, &edge.taken_total),
                (&mut pool.edge.struck_base, &edge.struck_base),
                (&mut pool.edge.struck_total, &edge.struck_total),
            ] {
                for (a, b) in into.iter_mut().flatten().zip(from.iter().flatten()) {
                    a.absorb(b);
                }
            }
            for (into, from) in [
                (&mut pool.edge.major_gap_by_turn, &edge.major_gap_by_turn),
                (
                    &mut pool.edge.major_gap_by_online_turn,
                    &edge.major_gap_by_online_turn,
                ),
            ] {
                for (a, b) in into.iter_mut().zip(from) {
                    a[0] += b[0];
                    a[1] += b[1];
                }
            }
            pool.marks.extend(row.edge_marks.iter().cloned());
            pool.units_killed += u64::from(row.units_killed.unwrap_or(0));
            pool.units_lost += u64::from(row.units_lost.unwrap_or(0));
            pool.cities_lost += row.cities_lost.unwrap_or(0) as u64;
        }
        (pool.rows > 0).then_some(pool)
    }
}

fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let (sum, n) = values.fold((0.0, 0usize), |(sum, n), v| (sum + v, n + 1));
    (n > 0).then(|| sum / n as f64)
}

fn median(values: impl Iterator<Item = f64>) -> Option<f64> {
    let mut values: Vec<f64> = values.collect();
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 1 {
        values[mid]
    } else {
        (values[mid - 1] + values[mid]) / 2.0
    })
}

/// (c) and (d) over the games that reached one mark, as the live ledger
/// reads them: shares and strengths are per-game means averaged over games,
/// treasury and income are medians.
#[derive(Default)]
pub(super) struct MarkSummary {
    pub(super) games: usize,
    pub(super) army: Option<f64>,
    pub(super) unlocked: Option<f64>,
    pub(super) with_material: Option<f64>,
    pub(super) offer: Option<f64>,
    pub(super) gain: Option<f64>,
    pub(super) gold: Option<f64>,
    pub(super) gold_per_turn: Option<f64>,
    pub(super) ours: Option<f64>,
    pub(super) rivals: Option<f64>,
    pub(super) gap: Option<f64>,
}

pub(super) fn mark_summary(marks: &[&EdgeMark]) -> MarkSummary {
    let with_army: Vec<&EdgeMark> = marks.iter().copied().filter(|m| m.army > 0).collect();
    let ours = |m: &EdgeMark| m.our_strength_sum / f64::from(m.army);
    let rivals = |m: &EdgeMark| m.rival_strength_sum / f64::from(m.rival_units);
    let share = |part: fn(&EdgeMark) -> u32| {
        mean(
            with_army
                .iter()
                .map(|&m| f64::from(part(m)) / f64::from(m.army)),
        )
    };
    MarkSummary {
        games: marks.len(),
        army: mean(marks.iter().map(|m| f64::from(m.army))),
        unlocked: share(|m| m.unlocked),
        with_material: share(|m| m.with_material),
        offer: share(|m| m.offer),
        gain: mean(
            marks
                .iter()
                .filter(|m| m.unlocked > 0)
                .map(|m| m.gain_sum / f64::from(m.unlocked)),
        ),
        gold: median(marks.iter().map(|m| m.gold)),
        gold_per_turn: median(marks.iter().map(|m| m.gold_per_turn)),
        ours: mean(with_army.iter().map(|&m| ours(m))),
        rivals: mean(
            marks
                .iter()
                .filter(|m| m.rival_units > 0)
                .map(|&m| rivals(m)),
        ),
        gap: mean(
            with_army
                .iter()
                .filter(|m| m.rival_units > 0)
                .map(|&m| rivals(m) - ours(m)),
        ),
    }
}

fn num(value: Option<f64>, decimals: usize, signed: bool) -> String {
    match value {
        None => "-".to_string(),
        Some(v) if signed => format!("{v:+.decimals$}"),
        Some(v) => format!("{v:.decimals$}"),
    }
}

fn gap_line(name: &str, buckets: &[[i64; 2]; 4]) -> String {
    let cells: Vec<String> = EDGE_TURN_BUCKETS
        .iter()
        .zip(buckets)
        .map(|(label, [n, gap])| {
            let mean = (*n > 0).then(|| *gap as f64 / *n as f64);
            format!("{label} {} (n {n})", num(mean, 1, true))
        })
        .collect();
    format!(
        "    rival-major mean base gap by {name}: {}",
        cells.join(" | ")
    )
}

fn taken_table(out: &mut Vec<String>, title: &str, table: &EdgeTable) {
    out.push(title.to_string());
    out.push(format!(
        "    {:<10} {:>7} {:>6} {:>6} {:>8} {:>6} {:>10}",
        "side", "gap", "blows", "share", "dmg/blow", "deaths", "death/blow"
    ));
    for (side, row) in EDGE_SIDES.iter().zip(table) {
        let total = blows(row);
        for (label, cell) in EDGE_BINS.iter().zip(row) {
            if cell.blows == 0 {
                continue;
            }
            let n = f64::from(cell.blows);
            out.push(format!(
                "    {side:<10} {label:>7} {:>6} {:>6.2} {:>8.1} {:>6} {:>10.2}",
                cell.blows,
                n / total,
                f64::from(cell.damage_to_defender) / n,
                cell.defender_killed,
                f64::from(cell.defender_killed) / n,
            ));
        }
    }
}

fn struck_table(out: &mut Vec<String>, title: &str, table: &EdgeTable) {
    out.push(title.to_string());
    out.push(format!(
        "    {:<10} {:>7} {:>6} {:>6} {:>10} {:>10} {:>5} {:>9} {:>10}",
        "side",
        "gap",
        "blows",
        "share",
        "dealt/blow",
        "taken/blow",
        "kills",
        "kill/blow",
        "our deaths"
    ));
    for (side, row) in EDGE_SIDES.iter().zip(table) {
        let total = blows(row);
        for (label, cell) in EDGE_BINS.iter().zip(row) {
            if cell.blows == 0 {
                continue;
            }
            let n = f64::from(cell.blows);
            out.push(format!(
                "    {side:<10} {label:>7} {:>6} {:>6.2} {:>10.1} {:>10.1} {:>5} {:>9.2} {:>10}",
                cell.blows,
                n / total,
                f64::from(cell.damage_to_defender) / n,
                f64::from(cell.damage_to_attacker) / n,
                cell.defender_killed,
                f64::from(cell.defender_killed) / n,
                cell.attacker_killed,
            ));
        }
    }
}

/// The pooled tables, in the live ledger's layout plus the total-gap table
/// of blows taken, which only the simulator can read (the host previews
/// only our own blows). `None` when no row carries the ledger.
pub(super) fn render(rows: &[&Row]) -> Option<String> {
    let pool = Pool::of(rows)?;
    let mut out = vec![format!("== {} row(s)", pool.rows)];
    taken_table(
        &mut out,
        "(a) blows on our units by BASE gap (their attack - our unit's combat):",
        &pool.edge.taken_base,
    );
    out.push(gap_line("raw turn", &pool.edge.major_gap_by_turn));
    out.push(gap_line(
        "Online-equivalent turn",
        &pool.edge.major_gap_by_online_turn,
    ));
    taken_table(
        &mut out,
        "(a') the same blows by TOTAL gap (the resolver's strengths; no live twin):",
        &pool.edge.taken_total,
    );
    struck_table(
        &mut out,
        "(b) our blows on units by TOTAL gap (the resolver's strengths):",
        &pool.edge.struck_total,
    );
    struck_table(
        &mut out,
        "(b) our blows on units by BASE gap:",
        &pool.edge.struck_base,
    );
    out.push(
        "(c) upgrades and (d) strength at Online-equivalent marks (per-game means; gold medians):"
            .to_string(),
    );
    out.push(format!(
        "    {:>4} {:>5} {:>5} {:>8} {:>9} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
        "mark",
        "games",
        "army",
        "unlocked",
        "+material",
        "offer",
        "gain",
        "gold",
        "gpt",
        "ours",
        "rivals",
        "gap"
    ));
    for mark in MARKS {
        let marks: Vec<&EdgeMark> = pool.marks.iter().filter(|m| m.mark == mark).collect();
        if marks.is_empty() {
            continue;
        }
        let s = mark_summary(&marks);
        out.push(format!(
            "    t{mark:<3} {:>5} {:>5} {:>8} {:>9} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6} {:>6}",
            s.games,
            num(s.army, 1, false),
            num(s.unlocked, 2, false),
            num(s.with_material, 2, false),
            num(s.offer, 2, false),
            num(s.gain, 1, true),
            num(s.gold, 0, false),
            num(s.gold_per_turn, 1, false),
            num(s.ours, 1, false),
            num(s.rivals, 1, false),
            num(s.gap, 1, true),
        ));
    }
    out.push(format!(
        "    combat ledger: units killed {} lost {} (kill:loss {:.2}), cities lost {:.2} a game, over {} row(s)",
        pool.units_killed,
        pool.units_lost,
        pool.units_killed as f64 / pool.units_lost.max(1) as f64,
        pool.cities_lost as f64 / pool.rows as f64,
        pool.rows
    ));
    Some(out.join("\n"))
}

/// The short list a live run and a simulated batch are set side by side on.
/// `tools/civ6_edge_ledger.py --metrics` prints the same names in the same
/// order for live runs.
pub(super) fn metrics(rows: &[&Row]) -> Vec<(String, Option<f64>)> {
    let Some(pool) = Pool::of(rows) else {
        return Vec::new();
    };
    let mut out: Vec<(String, Option<f64>)> = Vec::new();
    let mut put = |name: &str, value: Option<f64>| out.push((name.to_string(), value));
    let major = &pool.edge.taken_base[0];
    let major_blows = blows(major);
    let ratio = |a: f64, b: f64| (b > 0.0).then(|| a / b);
    put("major_blows_per_game", ratio(major_blows, pool.rows as f64));
    put(
        "major_share_ge10",
        ratio(f64::from(major[4].blows), major_blows),
    );
    put(
        "major_death_per_blow_ge10",
        ratio(
            f64::from(major[4].defender_killed),
            f64::from(major[4].blows),
        ),
    );
    let below: (f64, f64) = major[..4].iter().fold((0.0, 0.0), |(k, n), cell| {
        (
            k + f64::from(cell.defender_killed),
            n + f64::from(cell.blows),
        )
    });
    put("major_death_per_blow_lt10", ratio(below.0, below.1));
    for (label, [n, gap]) in ["0_49", "50_99", "100_149", "150"]
        .iter()
        .zip(&pool.edge.major_gap_by_online_turn)
    {
        put(&format!("major_gap_{label}"), ratio(*gap as f64, *n as f64));
    }
    let struck = &pool.edge.struck_total[0];
    let struck_blows = blows(struck);
    put(
        "struck_major_share_le_m10",
        ratio(f64::from(struck[0].blows), struck_blows),
    );
    put(
        "struck_major_share_ge10",
        ratio(f64::from(struck[4].blows), struck_blows),
    );
    put(
        "struck_major_dmg_le_m10",
        ratio(
            f64::from(struck[0].damage_to_defender),
            f64::from(struck[0].blows),
        ),
    );
    put(
        "struck_major_dmg_ge10",
        ratio(
            f64::from(struck[4].damage_to_defender),
            f64::from(struck[4].blows),
        ),
    );
    put(
        "struck_major_kill_per_blow_ge10",
        ratio(
            f64::from(struck[4].defender_killed),
            f64::from(struck[4].blows),
        ),
    );
    for mark in MARKS {
        let marks: Vec<&EdgeMark> = pool.marks.iter().filter(|m| m.mark == mark).collect();
        let s = mark_summary(&marks);
        for (name, value) in [
            ("unlocked", s.unlocked),
            ("material", s.with_material),
            ("offer", s.offer),
            ("gain", s.gain),
            ("gold", s.gold),
            ("gpt", s.gold_per_turn),
            ("ours", s.ours),
            ("rivals", s.rivals),
            ("strength_gap", s.gap),
        ] {
            put(&format!("{name}_{mark}"), value);
        }
    }
    put(
        "kill_loss",
        ratio(pool.units_killed as f64, pool.units_lost as f64),
    );
    put(
        "cities_lost_per_game",
        ratio(pool.cities_lost as f64, pool.rows as f64),
    );
    out
}
