//! The doctrine arena: pose the tactical problems, and say how each agent
//! answered them.
//!
//! `battle_bench` asks one question — two identical armies six tiles apart in
//! open ground — very precisely. This asks seven different ones, each a board
//! painted from an engagement that made a general's reputation, and reports
//! both who won the trade and *how* they fought. See `src/doctrine.rs` for the
//! positions and what the pairing does and does not license.
//!
//! ```bash
//! cargo run --release --bin doctrine_arena -- --list
//! cargo run --release --bin doctrine_arena -- --a advanced --b advanced --seeds 12   # CONTROL FIRST
//! cargo run --release --bin doctrine_arena -- --a advanced --b basic --seeds 60
//! cargo run --release --bin doctrine_arena -- --position the_defile --a advanced --b basic
//! cargo run --release --bin doctrine_arena -- --profile advanced --seeds 20
//! ```
//!
//! **Run the control.** `--a advanced --b advanced` must report a paired mean
//! of exactly 0.00 on every position, with zero seeds diverging. That is what
//! says a treatment number from this harness is the agent rather than the
//! position's own asymmetry.
use civvis::doctrine::{
    matched_position, paired, position, DoctrineLedger, DoctrineProfile, MatchedPosition, Position,
    POSITIONS,
};
use civvis::elo::{builtin_ai, BUILTIN_AIS};
use civvis::parallel::{default_jobs, map};

fn number(args: &[String], key: &str, default: i64) -> i64 {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], key: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

fn known_ai(name: &str) -> bool {
    BUILTIN_AIS.contains(&name)
}

/// A profile field as a column, or a dash when the engagement gave nothing to
/// measure it from. Never a zero — a zero is a fact about the fighting and an
/// absence is a fact about the harness.
fn cell(value: Option<f64>) -> String {
    match value {
        Some(number) => format!("{number:+.2}"),
        None => "  -- ".to_string(),
    }
}

fn share(value: Option<f64>) -> String {
    match value {
        Some(number) => format!("{:.0}%", number * 100.0),
        None => "  -- ".to_string(),
    }
}

const PROFILE_HEADER: &str = "  concentr.  disper.  arrival     foot  absent  vanguard  \
envelop.  focus  ground  screen  contact";

fn profile_row(label: &str, profile: DoctrineProfile) -> String {
    format!(
        "{label:<22}{:>9}{:>9}{:>9}{:>9}{:>8}{:>10}{:>10}{:>7}{:>8}{:>8}{:>9}",
        cell(profile.concentration),
        cell(profile.dispersion),
        cell(profile.arrival),
        cell(profile.foot_arrival),
        share(profile.absent),
        share(profile.vanguard),
        cell(profile.envelopment),
        share(profile.focus),
        share(profile.ground),
        share(profile.screen),
        share(profile.contact),
    )
}

/// Merge a set of ledgers so a profile can be read over a whole run rather
/// than one seed at a time.
fn merged(ledgers: impl Iterator<Item = DoctrineLedger>) -> DoctrineLedger {
    let mut out = DoctrineLedger::default();
    for ledger in ledgers {
        out.absorb(&ledger);
    }
    out
}

fn describe() {
    println!("The doctrine arena: {} positions.", POSITIONS.len());
    let rules = civvis::rules::Rules::embedded();
    for spec in POSITIONS {
        println!();
        println!("{}  --  {}", spec.id, spec.name);
        println!("  {}", spec.provenance);
        println!("  {}", spec.problem);
        println!(
            "  board {}x{}, {} turns",
            spec.width, spec.height, spec.turns
        );
        for role in 0..2 {
            let force: Vec<&str> = spec.forces[role].iter().map(|unit| unit.kind).collect();
            println!(
                "  role {role}: {}  [{} units, {:.0} material]",
                spec.roles[role],
                force.len(),
                spec.material(role, &rules)
            );
            println!("           {}", force.join(" "));
        }
    }
}

/// One agent against itself across every position, reported as a doctrine
/// fingerprint. There is no treatment here and no p — this says how the agent
/// fights each problem, which is the thing to read before deciding what to
/// change about it.
fn profile_run(name: &str, seeds: usize, start_seed: u64, jobs: usize) {
    println!("doctrine_arena --profile {name}: {seeds} seeds a position");
    println!("no treatment and no p -- this is a description of how one agent fights");
    println!();
    println!("{:<24}{:<22}{PROFILE_HEADER}", "position", "role (swing)");
    for spec in POSITIONS {
        let results: Vec<MatchedPosition> = map(seeds, jobs, |index| {
            matched_position(spec, start_seed + index as u64, name, name, &builtin_ai)
        });
        let played: Vec<&MatchedPosition> = results.iter().filter(|row| !row.skipped).collect();
        if played.is_empty() {
            println!("{:<24}(no seed could be seated)", spec.id);
            continue;
        }
        for role in 0..2 {
            let ledger = merged(played.iter().map(|row| row.a_by_role[role].clone()));
            let swing = ledger.material_swing() / played.len() as f64;
            println!(
                "{:<24}{}",
                if role == 0 { spec.id } else { "" },
                profile_row(&format!("role {role} ({swing:+.0})"), ledger.profile()),
            );
        }
    }
    println!();
    println!("concentr.  own units near the contact less enemy units near it, per contact turn");
    println!("disper.    mean distance between own units, per turn -- low is a body that moves as one");
    println!("arrival    spread in turns of when each unit first reached the enemy -- low is 'fight united'");
    println!("foot       the same, over 2-move units alone: separates a slow line from fast cavalry");
    println!("absent     share of the force that never reached the enemy at all");
    println!("vanguard   share of the force in contact on the turn contact FIRST occurred");
    println!("envelop.   enemy units taken from two or more sides at once, per contact turn");
    println!("focus      share of damage dealt that landed on enemies that died");
    println!("ground     share of own unit-turns on hills or in cover");
    println!("screen     share of own ranged unit-turns with a friendly between them and the enemy");
    println!("contact    share of turns on which the two forces were within two tiles");
    println!();
    println!("The figure beside each role is that role's mean material swing per seed.");
    println!("None of these is a score. An army holding a defile should be dense and static;");
    println!("the same numbers from one that was meant to envelop mean it failed.");
}

/// Does arriving together actually predict winning?
///
/// Every other column in this harness describes an engagement that has already
/// happened, so correlating one against the result restates the outcome rather
/// than explaining it: an army being destroyed stops arriving, which inflates
/// its own arrival spread. `vanguard` is recorded at one instant upstream of
/// the whole engagement, before anything has been decided, which is what makes
/// this correlation a claim about cause.
///
/// Per seed: how much more of its force each agent had up at first contact
/// than the other, against how much material it went on to win by.
fn correlate(name_a: &str, name_b: &str, seeds: usize, start_seed: u64, jobs: usize) {
    println!(
        "doctrine_arena --correlate: {name_a} against {name_b}, {seeds} seeds a position"
    );
    println!(
        "per seed, the vanguard each agent got up at first contact less the other's, \
         against the material it won by"
    );
    println!();
    println!(
        "{:<20}{:>8}{:>7}{:>9}{:>10}{:>10}",
        "position", "r", "n", "p", "clean", "swing/pt"
    );
    let mut pooled_x: Vec<f64> = Vec::new();
    let mut pooled_y: Vec<f64> = Vec::new();
    for spec in POSITIONS {
        let results: Vec<MatchedPosition> = map(seeds, jobs, |index| {
            matched_position(spec, start_seed + index as u64, name_a, name_b, &builtin_ai)
        });
        let mut xs: Vec<f64> = Vec::new();
        let mut ys: Vec<f64> = Vec::new();
        let mut clean = 0.0f64;
        let mut counted = 0.0f64;
        for row in results.iter().filter(|row| !row.skipped) {
            let (Some(va), Some(vb)) = (row.a.profile().vanguard, row.b.profile().vanguard)
            else {
                continue;
            };
            xs.push(va - vb);
            ys.push(row.paired_difference());
            for profile in [row.a.profile(), row.b.profile()] {
                if let Some(share) = profile.vanguard_clean {
                    clean += share;
                    counted += 1.0;
                }
            }
        }
        pooled_x.extend(xs.iter().copied());
        pooled_y.extend(ys.iter().copied());
        match paired::correlation(&xs, &ys) {
            Some((r, n, p)) => {
                // How much material one whole extra unit-share at first
                // contact is worth, as a slope, so the r has a size beside it.
                let slope = slope_of(&xs, &ys);
                println!(
                    "{:<20}{r:>+8.2}{n:>7}{p:>9.4}{:>10}{:>10}",
                    spec.id,
                    share_text(counted > 0.0, clean / counted.max(1.0)),
                    slope.map_or("  --".to_string(), |value| format!("{value:+.0}"))
                );
            }
            None => println!("{:<20}{:>8}", spec.id, "--"),
        }
    }
    match paired::correlation(&pooled_x, &pooled_y) {
        Some((r, n, p)) => println!("{:<20}{r:>+8.2}{n:>7}{p:>9.4}", "ALL POSITIONS"),
        None => println!("{:<20}{:>8}", "ALL POSITIONS", "--"),
    }
    println!();
    println!(
        "r is the correlation across seeds; swing/pt is the material a whole extra \
         share of the force\nat first contact goes with, as a least-squares slope. \
         clean is the share of first-contact\ninstants that had not yet seen a \
         casualty -- the guarantee that this is upstream of the result."
    );
    println!();
    println!(
        "⚠ This is a correlation between two things one agent does, not a demonstration \
         that\nforcing the first would produce the second. It says the lever is worth \
         pulling, not that\npulling it works. Only a treatment run through the gates can \
         say that."
    );
}

/// Least-squares slope of y on x, or `None` when x has no spread.
fn slope_of(xs: &[f64], ys: &[f64]) -> Option<f64> {
    let n = xs.len().min(ys.len());
    if n < 3 {
        return None;
    }
    let mean = |values: &[f64]| values[..n].iter().sum::<f64>() / n as f64;
    let (mx, my) = (mean(xs), mean(ys));
    let mut sxy = 0.0;
    let mut sxx = 0.0;
    for index in 0..n {
        sxy += (xs[index] - mx) * (ys[index] - my);
        sxx += (xs[index] - mx).powi(2);
    }
    (sxx > 0.0).then(|| sxy / sxx)
}

fn share_text(present: bool, value: f64) -> String {
    if present {
        format!("{:.0}%", value * 100.0)
    } else {
        "  --".to_string()
    }
}

/// One position, both agents, the roles swapped. Returns the paired
/// differences and the merged ledgers for the report.
struct Outcome {
    played: usize,
    skipped: usize,
    diverged: usize,
    differences: Vec<f64>,
    a: DoctrineLedger,
    b: DoctrineLedger,
    a_by_role: [DoctrineLedger; 2],
    b_by_role: [DoctrineLedger; 2],
}

fn run_position(
    spec: &Position,
    name_a: &str,
    name_b: &str,
    seeds: usize,
    start_seed: u64,
    jobs: usize,
) -> Outcome {
    let results: Vec<MatchedPosition> = map(seeds, jobs, |index| {
        matched_position(spec, start_seed + index as u64, name_a, name_b, &builtin_ai)
    });
    let played: Vec<&MatchedPosition> = results.iter().filter(|row| !row.skipped).collect();
    // The fires-check. Two agents that played identically on every seed
    // produce a paired difference of exactly zero, and a null from that is the
    // harness saying nothing happened rather than the game saying it did not
    // matter.
    let diverged = played
        .iter()
        .filter(|row| {
            row.paired_difference() != 0.0
                || row.a.damage_dealt != row.b.damage_dealt
                || row.a.kills != row.b.kills
        })
        .count();
    Outcome {
        played: played.len(),
        skipped: results.len() - played.len(),
        diverged,
        differences: played.iter().map(|row| row.paired_difference()).collect(),
        a: merged(played.iter().map(|row| row.a.clone())),
        b: merged(played.iter().map(|row| row.b.clone())),
        a_by_role: [
            merged(played.iter().map(|row| row.a_by_role[0].clone())),
            merged(played.iter().map(|row| row.a_by_role[1].clone())),
        ],
        b_by_role: [
            merged(played.iter().map(|row| row.b_by_role[0].clone())),
            merged(played.iter().map(|row| row.b_by_role[1].clone())),
        ],
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "--list") {
        describe();
        return;
    }
    let seeds = number(&args, "--seeds", 24).max(1) as usize;
    let start_seed = number(&args, "--start-seed", 5_100_000) as u64;
    let jobs = number(&args, "--jobs", default_jobs() as i64).max(1) as usize;

    if args.iter().any(|arg| arg == "--correlate") {
        let name_a = text(&args, "--a", "advanced");
        let name_b = text(&args, "--b", "basic");
        for name in [&name_a, &name_b] {
            if !known_ai(name) {
                eprintln!("unknown agent `{name}`");
                std::process::exit(2);
            }
        }
        correlate(&name_a, &name_b, seeds, start_seed, jobs);
        return;
    }

    let profile_only = text(&args, "--profile", "");
    if !profile_only.is_empty() {
        if !known_ai(&profile_only) {
            eprintln!("unknown agent `{profile_only}`");
            std::process::exit(2);
        }
        profile_run(&profile_only, seeds, start_seed, jobs);
        return;
    }

    let name_a = text(&args, "--a", "advanced");
    let name_b = text(&args, "--b", "basic");
    for name in [&name_a, &name_b] {
        if !known_ai(name) {
            eprintln!("unknown agent `{name}`");
            std::process::exit(2);
        }
    }
    let wanted = text(&args, "--position", "");
    let chosen: Vec<&Position> = if wanted.is_empty() {
        POSITIONS.iter().collect()
    } else {
        match position(&wanted) {
            Some(spec) => vec![spec],
            None => {
                eprintln!("unknown position `{wanted}`; --list names them all");
                std::process::exit(2);
            }
        }
    };

    println!(
        "doctrine_arena: {name_a} vs {name_b}, {} position(s), {seeds} seeds x2 role swaps",
        chosen.len()
    );
    println!("paired material swing, {name_a} less {name_b}, one number per seed");
    println!();
    println!(
        "{:<20}{:>9}{:>9}{:>8}{:>9}{:>14}{:>8}",
        "position", "mean", "+/- se", "t", "sign p", "better/worse", "fires"
    );

    let mut totals: Vec<f64> = Vec::new();
    let mut silent: Vec<&str> = Vec::new();
    let mut outcomes: Vec<(&Position, Outcome)> = Vec::new();
    for spec in &chosen {
        let outcome = run_position(spec, &name_a, &name_b, seeds, start_seed, jobs);
        let wins = outcome.differences.iter().filter(|d| **d > 0.0).count();
        let losses = outcome.differences.iter().filter(|d| **d < 0.0).count();
        let (mean, stderr, t, _) = paired::paired_t(&outcome.differences);
        let p_sign = paired::sign_test(wins, losses);
        println!(
            "{:<20}{mean:>+9.1}{stderr:>9.1}{t:>8.2}{p_sign:>9.4}{:>14}{:>8}",
            spec.id,
            format!("{wins}/{losses}"),
            format!("{}/{}", outcome.diverged, outcome.played)
        );
        if outcome.skipped > 0 {
            println!(
                "{:<20}({} seed(s) skipped: the board could not seat both forces)",
                "", outcome.skipped
            );
        }
        if outcome.diverged == 0 {
            silent.push(spec.id);
        }
        totals.extend(outcome.differences.iter().copied());
        outcomes.push((spec, outcome));
    }

    if chosen.len() > 1 {
        let wins = totals.iter().filter(|d| **d > 0.0).count();
        let losses = totals.iter().filter(|d| **d < 0.0).count();
        let (mean, stderr, t, _) = paired::paired_t(&totals);
        println!(
            "{:<20}{mean:>+9.1}{stderr:>9.1}{t:>8.2}{:>9.4}{:>14}{:>8}",
            "ALL POSITIONS",
            paired::sign_test(wins, losses),
            format!("{wins}/{losses}"),
            ""
        );
        println!();
        println!(
            "The pooled row treats every position as one experiment, which they are not: \
             they were chosen to pose different problems, and an agent that is much better \
             at one and worse at another can pool to nothing. Read the rows."
        );
    }

    println!();
    println!("How each side fought. Role 0 and role 1 are the two sides of each position;");
    println!("both agents play both, so a row pair says which side of the problem each is");
    println!("better at.");
    for (spec, outcome) in &outcomes {
        println!();
        println!("{}  --  {}", spec.id, spec.name);
        println!("  role 0: {}", spec.roles[0]);
        println!("  role 1: {}", spec.roles[1]);
        println!("{:<22}{PROFILE_HEADER}", "");
        for role in 0..2 {
            let a = &outcome.a_by_role[role];
            let b = &outcome.b_by_role[role];
            let per_seed = |ledger: &DoctrineLedger| {
                ledger.material_swing() / outcome.played.max(1) as f64
            };
            println!(
                "  {}",
                profile_row(
                    &format!("{name_a} r{role} ({:+.0})", per_seed(a)),
                    a.profile()
                )
            );
            println!(
                "  {}",
                profile_row(
                    &format!("{name_b} r{role} ({:+.0})", per_seed(b)),
                    b.profile()
                )
            );
        }
        println!(
            "  totals: {name_a} {} kills / {} lost, {name_b} {} kills / {} lost",
            outcome.a.kills, outcome.a.losses, outcome.b.kills, outcome.b.losses
        );
    }

    if !silent.is_empty() {
        println!();
        println!(
            "NO DIVERGENCE on {}. The two agents played identically there, so those rows \
             measured nothing about either. A p from a position that never fired is the \
             harness talking.",
            silent.join(", ")
        );
    }
}
