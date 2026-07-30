//! Ask CIVVIS where it would have settled, and compare that to what the Lua
//! agent actually did.
//!
//! # Why this exists before the integration
//!
//! The operator's architecture is "CIVVIS is the logic engine and the harness
//! actuates its decisions". The obstacle is that the control mod has **no inbound
//! channel** — no `io`, and its config is baked in at install time — so a
//! per-turn CIVVIS decision can only reach the game through the harness driving
//! the UI. That is a large job.
//!
//! Before building it, the question worth answering is whether it would change
//! anything. The Lua agent carries a hand-rolled settle score written to resemble
//! CIVVIS's, and nobody has ever checked whether the two agree. If they agree,
//! the approximation is fine and the real gap is elsewhere; if they disagree
//! systematically, that is a measured defect with a direction, and it is the case
//! for doing the actuation work.
//!
//! # What it is honest about
//!
//! ⚠ The mirrored game has TERRAIN but no cities or units: `mirror::rebuild_game`
//! rebuilds the map, not the empire. CIVVIS's settle score includes terms that
//! read existing cities (spacing, own territory), so those terms are evaluated
//! against an empty empire here. The tile-quality half of the comparison is sound;
//! the spacing half is not, and a disagreement that turns out to be spacing-only
//! should be read as this tool's limit rather than the agent's error.
//!
//! ⚠ Only REVEALED plots are considered. Everything else is ocean in the mirror
//! and would score nothing anyway, but the filter is explicit so a future change
//! to the filler cannot silently start recommending unseen ground.
//!
//!     civvis-advise --mirror ~/civvis-civ6-runs/control/<run> [--radius 8]
use std::collections::BTreeSet;
use std::path::Path;

use civvis::ai::Ai;
use civvis::mirror;

fn arg_text(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|value| value == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(dir) = arg_text(&args, "--mirror") else {
        eprintln!("usage: civvis-advise --mirror <run-dir> [--radius N]");
        std::process::exit(2);
    };
    // `--plan <file>`: write CIVVIS's ranking as JSON for the installer to bake
    // into the mod's config.
    //
    // ⚠ THIS IS THE ONLY INBOUND CHANNEL THAT EXISTS. The mod cannot read a file
    // at runtime (no `io`) and FireTuner does not answer: with a live game and the
    // correct log path, seven plausible framings on ports 4318/4319 executed
    // nothing. Config baked at install time is what is left, and it works because
    // the world is a function of the seed — plan a map once, replay it with the
    // same seed, and the plan still describes that map.
    let plan_out = arg_text(&args, "--plan");
    let radius: i32 = arg_text(&args, "--radius")
        .and_then(|value| value.parse().ok())
        .unwrap_or(8);

    let events = Path::new(&dir).join("events.jsonl");
    let snapshot = match mirror::snapshot_from_events(&events) {
        Ok(snapshot) => snapshot,
        Err(error) => {
            eprintln!("cannot read {}: {error}", events.display());
            std::process::exit(2);
        }
    };
    if snapshot.revealed_count() == 0 {
        eprintln!(
            "no tiles in {} — the run needs --export-state, and before the \
             PlayersVisibility fix the export emitted nothing",
            events.display()
        );
        std::process::exit(2);
    }

    // Where the agent actually founded, straight out of the same stream. These are
    // the ground truth this compares against.
    let founded = founded_positions(&events);

    // ⚠⚠ SEQUENTIAL, and both earlier versions of this tool were wrong about it.
    //
    // v1 placed no cities at all, so spacing was evaluated against an empty world
    // and CIVVIS's best site came out two tiles from the real capital — illegal at
    // Civilization VI's own CITY_MIN_RANGE of 3. It reported a disagreement that
    // was mostly its own doing.
    //
    // v2 placed all of them, which makes every founded plot a CITY rather than a
    // candidate, so "not among the sites CIVVIS would consider" was true by
    // construction for all four. A comparison that cannot come out any other way
    // measures nothing.
    //
    // The question is a counterfactual and has to be posed as one: with the cities
    // that existed AT THE TIME, where would CIVVIS have put the next one, and
    // where does the agent's actual choice sit in that ranking?
    let ai = civvis::ai::AdvancedAi::new();
    let capital = founded.first().copied();

    println!("run           {}", Path::new(&dir).file_name().unwrap().to_string_lossy());
    println!("turn          {}", snapshot.turn);
    println!("world         {}x{}", snapshot.width, snapshot.height);
    println!(
        "revealed      {} plots ({:.1}% of the world)",
        snapshot.revealed_count(),
        snapshot.revealed_fraction() * 100.0
    );
    let unmapped = snapshot.untranslatable(mirror::Vocabulary::embedded());
    if !unmapped.is_empty() {
        println!("⚠ untranslatable types present: {unmapped:?}");
    }
    println!("cities        {} founded over the run", founded.len());
    println!();

    let mut ranks: Vec<usize> = Vec::new();
    let mut considered = 0usize;
    for (index, chosen) in founded.iter().enumerate().skip(1) {
        let prior = &founded[..index];
        let (game, placed) = mirror::rebuild_with_empire(&snapshot, prior, 4, 1);
        let from = capital.unwrap_or(*chosen);
        let ranked: Vec<civvis::Pos> = ai
            .settle_ranking(&game, 0, from, radius)
            .into_iter()
            .filter(|(pos, _)| snapshot.is_revealed(*pos))
            .map(|(pos, _)| pos)
            .collect();
        let total = ranked.len();
        print!("city {} at {chosen:?} (with {placed}/{} prior cities placed): ",
               index + 1, prior.len());
        match ranked.iter().position(|candidate| candidate == chosen) {
            Some(at) => {
                let pct = 100.0 * (at as f64 + 1.0) / total.max(1) as f64;
                println!("CIVVIS rank {} of {total} (top {pct:.0}%)", at + 1);
                ranks.push(at + 1);
                considered += 1;
            }
            None if total == 0 => {
                println!("CIVVIS offered NO legal revealed site — cannot compare");
            }
            None => {
                println!("not in CIVVIS's {total} candidates");
                considered += 1;
            }
        }
    }

    println!();
    if founded.len() < 2 {
        println!("Only one city was founded; there is no siting decision to compare.");
    } else if ranks.is_empty() {
        println!("No comparable decision: CIVVIS offered no legal revealed site at any");
        println!("founding, which usually means the mirror had seen too little ground.");
    } else {
        let mean = ranks.iter().sum::<usize>() as f64 / ranks.len() as f64;
        println!("mean CIVVIS rank of the agent's choice: {mean:.1}                    ({}/{considered} comparable)", ranks.len());
        println!(
            "{}",
            if mean <= 3.0 {
                "→ they broadly AGREE. Settle siting is not where wiring CIVVIS in \
                 would pay; the gap is elsewhere."
            } else {
                "→ they DISAGREE, and the direction is now measured rather than \
                 assumed. This is the case for actuating CIVVIS's choice."
            }
        );
    }
    if let Some(path) = plan_out {
        // Plan from the empire as it stands, which is what the next founding faces.
        let (game, _) = mirror::rebuild_with_empire(&snapshot, &founded, 4, 1);
        let from = capital.unwrap_or((snapshot.width / 2, snapshot.height / 2));
        let sites: Vec<serde_json::Value> = ai
            .settle_ranking(&game, 0, from, radius)
            .into_iter()
            .filter(|(pos, _)| snapshot.is_revealed(*pos))
            .take(24)
            .map(|((x, y), score)| {
                serde_json::json!({ "x": x, "y": y, "score": score })
            })
            .collect();
        let doc = serde_json::json!({
            "seed_world": format!("{}x{}", snapshot.width, snapshot.height),
            "from_turn": snapshot.turn,
            "revealed": snapshot.revealed_count(),
            "origin": [from.0, from.1],
            "sites": sites,
        });
        std::fs::write(&path, serde_json::to_string_pretty(&doc).unwrap())
            .unwrap_or_else(|error| {
                eprintln!("cannot write {path}: {error}");
                std::process::exit(2);
            });
        println!("wrote {} CIVVIS-ranked sites to {path}", doc["sites"].as_array().unwrap().len());
        println!();
    }
    println!();
    println!("⚠ Read with care: the mirror carries the map and the seat's cities, not");
    println!("  rival cities, units, or improvements, so terms reading those are still");
    println!("  short. Tile quality and own-spacing are sound.");
}

/// Every plot the agent founded a city on, in order, from the event stream.
fn founded_positions(events: &Path) -> Vec<civvis::Pos> {
    let Ok(raw) = std::fs::read_to_string(events) else {
        return Vec::new();
    };
    let mut seen = BTreeSet::new();
    let mut order = Vec::new();
    for line in raw.lines() {
        if !line.contains("\"state\"") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(cities) = value.get("cities").and_then(|c| c.as_array()) else {
            continue;
        };
        for city in cities {
            let x = city.get("x").and_then(|v| v.as_i64());
            let y = city.get("y").and_then(|v| v.as_i64());
            if let (Some(x), Some(y)) = (x, y) {
                let pos = (x as i32, y as i32);
                if seen.insert(pos) {
                    order.push(pos);
                }
            }
        }
    }
    order
}
