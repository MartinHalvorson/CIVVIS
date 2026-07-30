//! Seat the anchors a running league cannot acquire for itself.
//!
//! The self-improvement loop breeds `StrategyKind::Advanced` and nothing else,
//! so a roster seeded without a searching entry can never acquire one however
//! long it runs. Measured on the live roster: 61 entries, 54 `Advanced` and 7
//! `Builtin`, and not one of them searches. `seed_league` gained a searching
//! anchor, but that is the founding roster only — a league already on disk keeps
//! whatever it was seeded with.
//!
//! That matters because search is the one axis with a reproducible strength
//! result (`strategic`'s compute doubling, p=0.0023) while the genome is a local
//! optimum on wins, 11 of 48 genes produce zero divergence, and about a thousand
//! rounds produced no measurable gain. The loop is not climbing slowly; it
//! cannot see the axis next door.
//!
//! This is a one-shot operator action rather than something a round does,
//! because a permanent extra seat is a reallocation and not a mechanical
//! migration: seating inside `evolve_league` either costs a bred strategy its
//! place or breaks the population cap, both caught by existing tests. Run it
//! deliberately, on a roster you have backed up.
//!
//!     cargo run --bin seat_anchors -- --dry-run <league-dir>
//!     cargo run --bin seat_anchors -- <league-dir>
use std::collections::BTreeMap;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dry_run = args.iter().any(|a| a == "--dry-run");
    let dir = match args.iter().find(|a| !a.starts_with("--")) {
        Some(d) => d.clone(),
        None => {
            eprintln!("usage: seat_anchors [--dry-run] <league-dir>");
            std::process::exit(2);
        }
    };

    let Some(before) = civvis::league::load_league(&dir) else {
        eprintln!("no league.json under {dir}");
        std::process::exit(1);
    };

    // What the roster is made of now, so "it has no searching entry" is shown
    // rather than asserted.
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    let mut searching = Vec::new();
    for strategy in &before.strategies {
        let label = match &strategy.kind {
            civvis::league::StrategyKind::Builtin { ai } => {
                if ai.starts_with("strategic") {
                    searching.push(strategy.name.clone());
                }
                "Builtin"
            }
            civvis::league::StrategyKind::Advanced { .. } => "Advanced",
        };
        *kinds.entry(label).or_default() += 1;
    }
    println!("roster at {dir}");
    println!("  round    {}", before.round);
    println!("  entries  {}", before.strategies.len());
    for (kind, count) in &kinds {
        println!("  {kind:<8} {count}");
    }
    println!(
        "  searching entries: {}",
        if searching.is_empty() {
            "NONE — the loop cannot reach the axis measured to pay".to_string()
        } else {
            searching.join(", ")
        }
    );

    if dry_run {
        println!("\n--dry-run: nothing written");
        return;
    }

    match civvis::league::seat_missing_anchors(&dir) {
        Some(added) if added.is_empty() => println!("\nalready seated; nothing to do"),
        Some(added) => println!("\nseated: {}", added.join(", ")),
        None => {
            eprintln!("could not read the roster back");
            std::process::exit(1);
        }
    }
}
