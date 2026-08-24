//! What a fair-play economic planner loses, measured rather than argued.
//!
//! `docs/AI_GAPS.md` records one number for the end-to-end fog-honest major:
//! 15.0% paired score over 20 maps (95% Wilson 5.2%..36.0%), against a stock
//! `advanced` that keeps the strength gate. It also names the successor —
//! *improve fair-play economic planning before re-running the gate* — and
//! nothing in the repository says **which** economic decisions degrade when
//! belief replaces ground truth. That is what this binary measures.
//!
//! Two readings, on the same seeds, at the screen's own shape (six majors,
//! 74x46 continents, nine city-states, Online, 250 turns):
//!
//! `--diff` — **the matched-state decision diff.** A control game is played
//! with every major on the deployment controller. At sampled turns the world
//! and the acting seat's controller are cloned twice; one clone takes its
//! turn normally, the other takes the same turn with `fog_honest` on. Both
//! start from the identical state, so every difference in the two action
//! tapes is the information contract and nothing else. Counts are bucketed by
//! `action_space::kind_name`, so the buckets are the engine's own action
//! variants.
//!
//! `--census` — **the whole-game A/B.** The same seed is played twice, once
//! with seat 0 on `AdvancedAi::fog_honest()` and once on `AdvancedAi::new()`,
//! with the other five majors identical in both. It reports the economic
//! census of that seat at the end (cities, techs, population, score share)
//! and, for the fog-honest arm, `FogPlanCensus`: how much of the plan the
//! authoritative board actually accepted.
//!
//! Neither reading is a strength screen. The strength screen is
//! `gene_screen`, and the genes this work registers are priced there.
//!
//! ```sh
//! cargo build --profile ci --features closed-experiments --bin fog_planning
//! target/ci/fog_planning --diff   --games 24 --jobs 6 --seed 930000
//! target/ci/fog_planning --census --games 60 --jobs 6 --seed 940000
//! ```

use civvis::action_space::kind_name;
use civvis::ai::{run_game, AdvancedAi, Ai, FogPlanCensus};
use civvis::game::{Action, Game, GameOptions};
use civvis::parallel;
use civvis::setup::{GameSpeed, MapScript};
use std::collections::BTreeMap;

/// The one screen shape, copied from `src/bin/gene_screen.rs`. A reading
/// taken anywhere else is a reading of a different world.
const PLAYERS: usize = 6;
const WIDTH: i32 = 74;
const HEIGHT: i32 = 46;
const CITY_STATES: usize = 9;
const TURNS: u32 = 250;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn world(seed: u64) -> Game {
    Game::new_with(GameOptions {
        speed: GameSpeed::Online.id().to_string(),
        map_script: MapScript::Continents,
        randomize_civs: true,
        ..GameOptions::new(PLAYERS, WIDTH, HEIGHT, seed, TURNS, CITY_STATES)
    })
}

fn majors(g: &Game) -> Vec<usize> {
    g.players
        .iter()
        .filter(|p| !p.is_minor && !p.is_barbarian)
        .map(|p| p.id)
        .collect()
}

// ------------------------------------------------------------ the decision diff

#[derive(Clone, Debug, Default)]
struct Diff {
    /// Decision points sampled.
    probes: u64,
    /// Probes whose two tapes were not identical.
    divergent: u64,
    /// Actions the stock controller took, by kind.
    stock: BTreeMap<String, u64>,
    /// Actions the fog-honest controller took at the same state, by kind.
    fogged: BTreeMap<String, u64>,
    /// The kind of the first action that differed, by the stock side's kind.
    first_difference: BTreeMap<String, u64>,
    /// Probes at which the stock controller founded a city and the
    /// fog-honest one did not, and the reverse.
    lost_found: u64,
    gained_found: u64,
    /// Probes at which stock started a Settler and fog-honest did not.
    lost_settler: u64,
    gained_settler: u64,
}

impl Diff {
    fn merge(&mut self, other: &Diff) {
        self.probes += other.probes;
        self.divergent += other.divergent;
        self.lost_found += other.lost_found;
        self.gained_found += other.gained_found;
        self.lost_settler += other.lost_settler;
        self.gained_settler += other.gained_settler;
        for (into, from) in [
            (&mut self.stock, &other.stock),
            (&mut self.fogged, &other.fogged),
            (&mut self.first_difference, &other.first_difference),
        ] {
            for (kind, count) in from {
                *into.entry(kind.clone()).or_default() += count;
            }
        }
    }
}

/// Take one whole turn and return the seat's own action tape.
fn tape(ai: &mut AdvancedAi, g: &mut Game, pid: usize) -> Vec<Action> {
    let start = g.log.len();
    ai.take_turn(g, pid);
    if g.winner.is_none() && g.current == pid {
        let _ = g.apply(pid, &Action::EndTurn);
    }
    g.log
        .since(start)
        .filter(|(seat, _)| *seat == pid)
        .map(|(_, action)| action.clone())
        .collect()
}

fn produces_settler(action: &Action) -> bool {
    matches!(action, Action::Produce { item, .. } if format!("{item:?}").contains("settler"))
}

fn count_by_kind(tape: &[Action], into: &mut BTreeMap<String, u64>) {
    for action in tape {
        *into.entry(kind_name(action).to_string()).or_default() += 1;
    }
}

fn diff_map(seed: u64, probes: usize) -> Diff {
    let mut g = world(seed);
    // ⚠ Remembered tiles are ON in the source line, and that is a fairness
    // condition rather than a convenience. A real fog-honest game calls
    // `set_fog_memory(true)` on its first turn, so by turn T the seat has a
    // remembered map; a clone that switched memory on at turn T would face
    // `fogged_clone` with an EMPTY memory and read every explored-but-unseen
    // plot as unknown ground. That would measure a controller nobody plays.
    // No `AdvancedAi` or `BasicAi` path reads `Player::remembered_tiles`, so
    // the control line's decisions are unchanged by this; only its cost is.
    g.set_fog_memory(true);
    g.set_war_ledger(false);
    let mut fleet: Vec<AdvancedAi> = (0..g.players.len()).map(|_| AdvancedAi::new()).collect();
    let seats = majors(&g);
    let mut reading = Diff::default();
    let interval = (TURNS / probes.max(1) as u32).max(1);
    let mut next = 2;
    while g.winner.is_none() && g.turn <= g.max_turns {
        let pid = g.current;
        let sample = seats.contains(&pid)
            && reading.probes < probes as u64
            && g.turn >= next
            && pid == seats[0];
        if !sample {
            let _ = tape(&mut fleet[pid], &mut g, pid);
            continue;
        }
        next = g.turn.saturating_add(interval);
        reading.probes += 1;

        let mut stock_world = g.clone();
        let mut stock_ai = fleet[pid].clone();
        let stock_tape = tape(&mut stock_ai, &mut stock_world, pid);

        let mut fogged_world = g.clone();
        let mut fogged_ai = fleet[pid].clone();
        fogged_ai.enable_fog_honest();
        let fogged_tape = tape(&mut fogged_ai, &mut fogged_world, pid);

        count_by_kind(&stock_tape, &mut reading.stock);
        count_by_kind(&fogged_tape, &mut reading.fogged);
        if stock_tape != fogged_tape {
            reading.divergent += 1;
            let at = stock_tape
                .iter()
                .zip(&fogged_tape)
                .position(|(l, r)| l != r)
                .unwrap_or(stock_tape.len().min(fogged_tape.len()));
            let label = stock_tape
                .get(at)
                .map(|a| kind_name(a).to_string())
                .unwrap_or_else(|| "tape ended".to_string());
            *reading.first_difference.entry(label).or_default() += 1;
        }
        let founds = |t: &[Action]| {
            t.iter()
                .filter(|a| matches!(a, Action::FoundCity { .. }))
                .count()
        };
        let settlers = |t: &[Action]| t.iter().filter(|a| produces_settler(a)).count();
        if founds(&stock_tape) > founds(&fogged_tape) {
            reading.lost_found += 1;
        } else if founds(&fogged_tape) > founds(&stock_tape) {
            reading.gained_found += 1;
        }
        if settlers(&stock_tape) > settlers(&fogged_tape) {
            reading.lost_settler += 1;
        } else if settlers(&fogged_tape) > settlers(&stock_tape) {
            reading.gained_settler += 1;
        }

        // The source line continues on the control controller, so every later
        // probe is taken from a state the stock agent actually reaches.
        let _ = tape(&mut fleet[pid], &mut g, pid);
    }
    reading
}

// -------------------------------------------------------------- the whole game

#[derive(Clone, Copy, Debug, Default)]
struct Seat {
    win: bool,
    score: f64,
    share: f64,
    cities: f64,
    techs: f64,
    pop: f64,
    turn: f64,
}

fn read_seat(g: &Game, seat: usize) -> Seat {
    let scores: Vec<i64> = majors(g).iter().map(|&pid| g.score(pid)).collect();
    let total: i64 = scores.iter().sum();
    let score = g.score(seat);
    let cities = g.player_city_ids(seat);
    Seat {
        win: g.winner == Some(seat),
        score: score as f64,
        share: if total > 0 {
            score as f64 / total as f64
        } else {
            0.0
        },
        cities: cities.len() as f64,
        techs: g.players[seat].techs.len() as f64,
        pop: cities
            .iter()
            .filter_map(|cid| g.cities.get(cid))
            .map(|city| city.pop as f64)
            .sum(),
        turn: f64::from(g.reported_turn()),
    }
}

#[derive(Clone, Debug, Default)]
struct Census {
    games: u64,
    arm: Vec<Seat>,
    control: Vec<Seat>,
    plan: FogPlanCensus,
    arm_secs: f64,
    control_secs: f64,
}

fn census_map(seed: u64) -> Census {
    let mut reading = Census {
        games: 1,
        ..Census::default()
    };
    let seat = {
        let probe = world(seed);
        majors(&probe)[0]
    };
    for fogged in [false, true] {
        let mut g = world(seed);
        let mut fleet: Vec<AdvancedAi> = (0..g.players.len())
            .map(|pid| {
                if fogged && pid == seat {
                    AdvancedAi::fog_honest()
                } else {
                    AdvancedAi::new()
                }
            })
            .collect();
        let started = std::time::Instant::now();
        run_game(&mut g, &mut fleet);
        let secs = started.elapsed().as_secs_f64();
        if fogged {
            reading.arm.push(read_seat(&g, seat));
            reading.plan.merge(&fleet[seat].fog_plan_census());
            reading.arm_secs = secs;
        } else {
            reading.control.push(read_seat(&g, seat));
            reading.control_secs = secs;
        }
    }
    reading
}

// ------------------------------------------------------------------ statistics

/// Mean, and the standard error of the paired difference.
fn paired(arm: &[Seat], control: &[Seat], field: impl Fn(&Seat) -> f64) -> (f64, f64, f64) {
    let deltas: Vec<f64> = arm
        .iter()
        .zip(control)
        .map(|(a, c)| field(a) - field(c))
        .collect();
    let n = deltas.len().max(1) as f64;
    let mean = deltas.iter().sum::<f64>() / n;
    let var = if deltas.len() > 1 {
        deltas.iter().map(|d| (d - mean).powi(2)).sum::<f64>() / (n - 1.0)
    } else {
        0.0
    };
    let arm_mean = arm.iter().map(&field).sum::<f64>() / arm.len().max(1) as f64;
    let control_mean = control.iter().map(&field).sum::<f64>() / control.len().max(1) as f64;
    (arm_mean, control_mean, (var / n).sqrt())
}

fn table(name: &str, arm: &[Seat], control: &[Seat], field: impl Fn(&Seat) -> f64) {
    let (a, c, se) = paired(arm, control, field);
    println!(
        "  {name:<14} fog-honest {a:>8.3}   stock {c:>8.3}   Δ {:>+8.3} ± {se:.3}",
        a - c
    );
}

fn top(label: &str, counts: &BTreeMap<String, u64>, other: &BTreeMap<String, u64>) {
    let mut rows: Vec<(&String, &u64)> = counts.iter().collect();
    rows.sort_by(|l, r| r.1.cmp(l.1).then(l.0.cmp(r.0)));
    println!("  {label}");
    for (kind, count) in rows.into_iter().take(14) {
        let mirror = other.get(kind).copied().unwrap_or(0);
        let delta = *count as i64 - mirror as i64;
        let pct = if mirror > 0 {
            100.0 * delta as f64 / mirror as f64
        } else {
            f64::NAN
        };
        println!("    {kind:<22} {count:>7}   stock {mirror:>7}   Δ {delta:>+7} ({pct:>+7.1}%)");
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let games = number(&args, "--games", 24);
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let seed = number(&args, "--seed", 930_000) as u64;
    let probes = number(&args, "--probes", 16);
    let census = args.iter().any(|a| a == "--census");
    if games == 0 {
        eprintln!("fog_planning: --games must be positive");
        std::process::exit(2);
    }
    println!(
        "fog_planning: {games} games, seed {seed}, jobs {jobs}, \
         {PLAYERS}p {WIDTH}x{HEIGHT} continents/{CITY_STATES}cs online/{TURNS}"
    );

    if census {
        let readings = parallel::map(games, jobs, move |i| census_map(seed + i as u64));
        let mut total = Census::default();
        for reading in &readings {
            total.games += reading.games;
            total.arm.extend(reading.arm.iter().copied());
            total.control.extend(reading.control.iter().copied());
            total.plan.merge(&reading.plan);
            total.arm_secs += reading.arm_secs;
            total.control_secs += reading.control_secs;
        }
        let n = total.arm.len();
        println!("\n  paired games                        {n}");
        table("wins", &total.arm, &total.control, |s| {
            f64::from(u8::from(s.win))
        });
        table("score share", &total.arm, &total.control, |s| s.share);
        table("cities", &total.arm, &total.control, |s| s.cities);
        table("techs", &total.arm, &total.control, |s| s.techs);
        table("population", &total.arm, &total.control, |s| s.pop);
        table("score", &total.arm, &total.control, |s| s.score);
        table("last turn", &total.arm, &total.control, |s| s.turn);
        println!(
            "  wall seconds                        fog-honest {:.1}   stock {:.1}   ({:+.0}%)",
            total.arm_secs,
            total.control_secs,
            100.0 * (total.arm_secs / total.control_secs.max(1e-9) - 1.0)
        );

        let plan = &total.plan;
        let planned: u32 = plan.planned.values().sum();
        let applied: u32 = plan.applied.values().sum();
        let refused: u32 = plan.refused.values().sum();
        println!("\n  the replay boundary");
        println!("    fog-honest turns                  {}", plan.turns);
        println!(
            "    actions planned / applied         {planned} / {applied} ({:.1}% accepted)",
            100.0 * f64::from(applied) / f64::from(planned.max(1))
        );
        println!(
            "    refused                           {refused} ({:.2}% of the plan), on {} of {} turns ({:.1}%)",
            100.0 * f64::from(refused) / f64::from(planned.max(1)),
            plan.refused_turns,
            plan.turns,
            100.0 * f64::from(plan.refused_turns) / f64::from(plan.turns.max(1))
        );
        println!(
            "    abandoned to a lost cursor        {} on {} turns",
            plan.abandoned, plan.truncated_turns
        );
        let mut rows: Vec<(&&str, &u32)> = plan.refused.iter().collect();
        rows.sort_by(|l, r| r.1.cmp(l.1).then(l.0.cmp(r.0)));
        println!("    refusals by action kind");
        for (kind, count) in rows.into_iter().take(12) {
            let tried = plan.planned.get(*kind).copied().unwrap_or(0);
            println!(
                "      {kind:<22} {count:>7} of {tried:>7} planned ({:.1}%)",
                100.0 * f64::from(*count) / f64::from(tried.max(1))
            );
        }
        return;
    }

    let readings = parallel::map(games, jobs, move |i| diff_map(seed + i as u64, probes));
    let mut total = Diff::default();
    for reading in &readings {
        total.merge(reading);
    }
    println!("\n  matched-state decision points        {}", total.probes);
    println!(
        "  probes whose tapes differed          {} ({:.1}%)",
        total.divergent,
        100.0 * total.divergent as f64 / total.probes.max(1) as f64
    );
    println!(
        "  city foundings lost / gained         {} / {}",
        total.lost_found, total.gained_found
    );
    println!(
        "  settler starts lost / gained         {} / {}",
        total.lost_settler, total.gained_settler
    );
    println!();
    top(
        "fog-honest actions at the same state (Δ against stock)",
        &total.fogged,
        &total.stock,
    );
    println!();
    top(
        "where the tapes first parted (stock side's action kind)",
        &total.first_difference,
        &BTreeMap::new(),
    );
}
