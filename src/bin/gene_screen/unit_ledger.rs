//! ⭐ The unit ledger on the screen's row, and its on−off read beside the city
//! ledger (2026-11-12).
//!
//! Each row carries the seat's `CombatLedger` (`game/combat_ledger.rs`):
//! enemy military units it killed, its own military units lost — split by
//! who struck, a major, a city-state or the barbarians — and its own
//! civilians and support units captured or destroyed; and, from the agent's
//! census (`ai/advanced/last_orders.rs`), which step last ordered each of its
//! military units that died. `--analyze` prints the on−off Δ of the ledger
//! beside `cities_taken` and `cities_lost`, so a gene that moves the exchange
//! can be told from one that only moves the game.

use super::{pace_abs_z, pace_cell, pace_json, GeneEstimate, Row};

/// How a column is read off one row.
type Read = fn(&Row) -> Option<f64>;

/// The columns the read sets side by side: the city ledger the rows already
/// carried, then the unit ledger. Each is `None` on a row written before it
/// existed.
pub(super) const COLUMNS: [(&str, Read); 7] = [
    ("cities_taken", |row| row.cities_taken.map(|n| n as f64)),
    ("cities_lost", |row| row.cities_lost.map(|n| n as f64)),
    ("units_killed", |row| row.units_killed.map(f64::from)),
    ("units_lost", |row| row.units_lost.map(f64::from)),
    ("units_lost_to_majors", |row| {
        row.units_lost_to_majors.map(f64::from)
    }),
    ("units_lost_to_barbarians", |row| {
        row.units_lost_to_barbarians.map(f64::from)
    }),
    ("civilians_lost", |row| row.civilians_lost.map(f64::from)),
];

/// Each column's outcome over the screened rows, `Some` only when every row
/// carries it: a file written before the ledger reads as no measurement,
/// never as a Δ of zero.
pub(super) fn outcomes(rows: &[&Row]) -> Vec<Option<Vec<f64>>> {
    COLUMNS
        .iter()
        .map(|(_, read)| {
            rows.iter()
                .map(|row| read(row))
                .collect::<Option<Vec<f64>>>()
                .filter(|values| !values.is_empty())
        })
        .collect()
}

/// The table, only when the rows carry the unit ledger.
pub(super) fn print(genes: &[GeneEstimate]) {
    let units = COLUMNS
        .iter()
        .position(|(name, _)| *name == "units_lost")
        .expect("a units_lost column");
    if !genes
        .iter()
        .any(|e| e.unit_ledger.get(units).copied().flatten().is_some())
    {
        return;
    }
    let mut by_losses: Vec<&GeneEstimate> = genes.iter().collect();
    by_losses.sort_by(|a, b| {
        pace_abs_z(b.unit_ledger[units]).total_cmp(&pace_abs_z(a.unit_ledger[units]))
    });
    println!(
        "\n{:<28} {:>14} {:>7}  {:>14} {:>7}  {:>14} {:>7}  {:>14} {:>7}",
        "unit ledger",
        "cities takenΔ",
        "z",
        "cities lostΔ",
        "z",
        "units killedΔ",
        "z",
        "units lostΔ",
        "z"
    );
    for e in &by_losses {
        println!(
            "{:<28} {}  {}  {}  {}",
            e.tag,
            pace_cell(e.unit_ledger[0]),
            pace_cell(e.unit_ledger[1]),
            pace_cell(e.unit_ledger[2]),
            pace_cell(e.unit_ledger[3]),
        );
    }
    println!(
        "\n{:<28} {:>14} {:>7}  {:>14} {:>7}  {:>14} {:>7}",
        "unit losses by who", "to majorsΔ", "z", "to barbariansΔ", "z", "civilians lostΔ", "z"
    );
    for e in &by_losses {
        println!(
            "{:<28} {}  {}  {}",
            e.tag,
            pace_cell(e.unit_ledger[4]),
            pace_cell(e.unit_ledger[5]),
            pace_cell(e.unit_ledger[6]),
        );
    }
    println!(
        "unit ledger cells are seats-on minus seats-off in cities or units per game ± one \
         clustered standard error, then z; a unit is counted where the engine removes it in \
         combat (`game/combat_ledger.rs`), civilians and support units apart"
    );
}

/// The ledger's contrasts as the analysis JSON carries them: one object per
/// column, `null` where the rows predate it.
pub(super) fn json(e: &GeneEstimate) -> serde_json::Value {
    COLUMNS
        .iter()
        .zip(&e.unit_ledger)
        .map(|((name, _), estimate)| (name.to_string(), pace_json(*estimate, e.n_on, e.n_off)))
        .collect::<serde_json::Map<String, serde_json::Value>>()
        .into()
}
