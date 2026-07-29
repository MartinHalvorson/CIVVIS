//! What does an empire buy with a Settler's price when it is short of cities?
//!
//! `Grant::Expansion` is the only grant in `oracle.rs` that has ever returned
//! headroom — 23.0% to 52.3% over 400 maps — and a dozen pull requests on the
//! expansion pipeline rest on it. `Grant::Rebate` is its cost-matched control:
//! same firing schedule, same city, same price, no Settler.
//!
//! But a *win* for the rebate would be ambiguous on its own. The empire might
//! have turned the money into cities anyway, in which case the rebate has
//! simply reproduced the expansion grant honestly; or it might have spent it
//! on something else entirely, in which case a win says the headroom was never
//! about cities. `ablate` reports only whether the granted seat won and how
//! often the grant fired, so it cannot tell those apart.
//!
//! This census can. It records, for each payment:
//!
//! - what the payout city had at the head of its queue when the money landed,
//! - and, at the end of the game, how many cities the seat held and how many
//!   Settlers it ever trained,
//!
//! against a matched control seat on the same map with no grant at all, and
//! against the expansion grant on the same cells. Three arms, one map set.
//!
//! ```text
//! cargo run --release --bin rebate_census -- --games 12 --players 4 \
//!     --turns 500 --seed 470000
//! ```
//!
//! This is a census, not an evaluation. It says what the money bought, never
//! whether buying it was good — twelve games cannot resolve a win rate and no
//! number printed here should be read as one.
//!
//! ## ⚠ It is also the fires-check the rebate grant needs, and it earned that
//!
//! The rebate's first version shared the expansion grant's firing *condition*
//! exactly, and a unit test confirmed the two fire on identical turns when run
//! against one shared position. That test was blind to the thing that matters.
//! A granted Settler occupies the `already_walking` slot for its whole transit
//! and so switches its own trigger off; a lump of banked production switches
//! nothing off. The condition agrees instant by instant and the two diverge
//! over a trajectory. This census measured the consequence at seed 470000:
//!
//! ```text
//! grant none        payout turns 181.7 per game
//! grant rebate      payout turns  66.5 per game
//! grant expansion   payout turns   5.6 per game
//! ```
//!
//! Twelve times the gift, wearing the word "cost-matched". **A firing rate has
//! to be measured over whole games, not over one position** — which is what
//! this binary is for, and why it must be run before any `ablate` batch that
//! compares the two.
use civvis::ai::Ai;
use civvis::elo::builtin_ai;
use civvis::game::{default_difficulty, Action, Game, GameOptions, Item};
use civvis::oracle::{expansion_payout_city, Grant, Oracle, EXPANSION_TARGET};
use civvis::rules::Rules;
use civvis::setup::MapSize;
use std::collections::{BTreeMap, BTreeSet};

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

/// What the payout city was building when a payment landed, coarsened to the
/// distinction the question turns on: a Settler, or anything else.
fn queue_head_label(g: &Game, cid: u32) -> &'static str {
    match g.cities.get(&cid).and_then(|city| city.queue.first()) {
        None => "empty queue",
        Some(Item::Unit { unit }) if unit == "settler" => "a settler",
        Some(Item::Unit { .. }) | Some(Item::Formation { .. }) => "another unit",
        Some(Item::Building { .. }) => "a building",
        Some(Item::District { .. }) => "a district",
        Some(Item::Wonder { .. }) => "a wonder",
        Some(_) => "a project or repair",
    }
}

/// What one game's granted seat did.
#[derive(Default, Clone)]
struct Census {
    /// Times the grant's payout condition held at the top of the seat's turn.
    ///
    /// Eligibility, not payment. `Grant::Expansion` pays on every eligible
    /// turn, but a grant that declines some of them — as the rebate does once
    /// it serializes on unspent money — makes the two diverge, and reading
    /// eligibility as payment is how the first version of this census reported
    /// a *rise* in payments from a change that could only ever cut them.
    eligible: u64,
    /// Times the grant actually did something, straight off `Oracle::fired`.
    /// This is the number a cost-match is a match on.
    payments: u64,
    /// Queue head at each of those moments.
    heads: BTreeMap<String, u64>,
    cities_at_end: usize,
    settlers_trained: i64,
    /// Every unit the seat ever trained, off the engine's `trained:*`
    /// counters. Against `settlers_trained` this says what a windfall was
    /// turned into.
    units_trained: i64,
    /// Cities taken by force. A grant that raises the city count without
    /// raising `settlers_trained` bought its cities here instead.
    captures: i64,
    /// Turn the seat first held `EXPANSION_TARGET` cities, if it ever did.
    /// A seat that reaches the target early stops being eligible, so this is
    /// how long the payout window was open.
    reached_target: Option<u32>,
    /// Turns on which the seat had at least one Settler alive.
    ///
    /// Against `cities_founded` this is the delivery time the whole expansion
    /// axis turns on. `Grant::Expansion` hands over the unit and still pays
    /// transit, so transit is inside its measured headroom rather than
    /// excluded from it — and `docs/OPENINGS.md` measured a Settler covering
    /// **0.81 tiles a turn against a shipped `moves` of 2** at 32x22, with 70%
    /// of its standing-still turns holding no destination at all. That was
    /// read as site scarcity, but #559 later found "no settle site" to be a
    /// pure 24x16 artifact that never fires at deployment density, so the
    /// transit finding needs re-measuring at a roomy map too.
    settler_turns: u64,
    /// Tiles Settlers actually covered, and the subset of turns they did not
    /// move at all.
    settler_tiles: u64,
    settler_still: u64,
    /// Distinct Settlers the seat ever had. Counted by unit id rather than off
    /// `trained:settler`, because the expansion arm trains none — every one of
    /// its Settlers is granted — and dividing lifetime by a training count
    /// would report the one arm the metric exists for as having no Settlers.
    settlers_seen: u64,
    /// Sum of the agent's own `desired_cities` over eligible turns, and the
    /// count it is averaged over.
    target_at_eligible: u64,
    target_samples: u64,
    /// Eligible turns on which the seat already held as many cities as its own
    /// plan asked for. On these the grant is not relieving a shortfall the
    /// agent feels — it is buying a city the agent has not decided to want.
    eligible_but_satisfied: u64,
    /// Eligible turns before the agent had assessed a plan at all.
    eligible_without_plan: u64,
    peak_cities: usize,
    won: bool,
}

/// One game, wrapping `oracle_seat` in `grant` and reading the seat's expansion
/// behaviour off the final position.
///
/// The queue-head observation is taken at the top of the seat's turn *before*
/// the wrapped agent plays, which is exactly when `Oracle::take_turn` applies
/// the grant, so the head recorded is the item the payment actually landed on.
/// It is read for every arm including the control, so the three histograms are
/// comparable rather than being a property of the grant that produced them.
fn play(options: GameOptions, oracle_seat: usize, grant: Grant, ai_name: &str) -> Census {
    let mut game = Game::new_with(options);
    let mut stock: Vec<Box<dyn Ai>> = game
        .players
        .iter()
        .map(|player| {
            let name = if player.is_minor || player.is_barbarian {
                "basic"
            } else {
                ai_name
            };
            builtin_ai(name, game.seed.wrapping_add(player.id as u64))
        })
        .collect();
    let mut oracle = Oracle::new(
        builtin_ai(ai_name, game.seed.wrapping_add(oracle_seat as u64)),
        grant,
    );
    let mut census = Census::default();
    // Settler positions as of this seat's previous turn, so movement is
    // measured over the seat's own turns rather than over every player's.
    let mut settler_was: BTreeMap<u32, civvis::Pos> = BTreeMap::new();
    let mut seen_settlers: BTreeSet<u32> = BTreeSet::new();
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        if pid == oracle_seat {
            if let Some(home) = expansion_payout_city(&game, pid) {
                census.eligible += 1;
                *census
                    .heads
                    .entry(queue_head_label(&game, home).to_string())
                    .or_insert(0) += 1;
                // Both grants pay while the seat holds fewer than the
                // hardcoded `EXPANSION_TARGET`. The agent has a target of its
                // own, and it is not that one: `desired_cities` ramps as
                // `(3 + turn/standard_duration(90)).min(map_capacity).min(6)`,
                // so it asks for three cities for the whole early game. Read
                // off the plan in force, before the agent plays, this says how
                // much of the grant is buying a city the agent *wanted* and
                // could not get, versus one it had not yet decided to want.
                match oracle.plan_report() {
                    Some(plan) => {
                        census.target_at_eligible += plan.desired_cities as u64;
                        census.target_samples += 1;
                        if game.player_city_ids(pid).len() >= plan.desired_cities {
                            census.eligible_but_satisfied += 1;
                        }
                    }
                    None => census.eligible_without_plan += 1,
                }
            }
            oracle.take_turn(&mut game, pid);

            // Read movement after the agent has played, so a Settler that was
            // ordered to move this turn is credited with it.
            let now: BTreeMap<u32, civvis::Pos> = game
                .player_unit_ids(pid)
                .into_iter()
                .filter_map(|uid| game.units.get(&uid))
                .filter(|unit| unit.kind == "settler")
                .map(|unit| (unit.id, unit.pos))
                .collect();
            if !now.is_empty() {
                census.settler_turns += 1;
            }
            for (uid, pos) in &now {
                if !seen_settlers.contains(uid) {
                    seen_settlers.insert(*uid);
                    census.settlers_seen += 1;
                }
                // A Settler seen for the first time has no previous position
                // and is neither moving nor standing still yet.
                if let Some(before) = settler_was.get(uid) {
                    let step = game.wdist(*before, *pos);
                    if step <= 0 {
                        census.settler_still += 1;
                    } else {
                        census.settler_tiles += step as u64;
                    }
                }
            }
            settler_was = now;
        } else {
            stock[pid].take_turn(&mut game, pid);
        }
        let held = game.player_city_ids(oracle_seat).len();
        census.peak_cities = census.peak_cities.max(held);
        if held >= EXPANSION_TARGET && census.reached_target.is_none() {
            census.reached_target = Some(game.turn);
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    census.cities_at_end = game.player_city_ids(oracle_seat).len();
    let counters = &game.players[oracle_seat].counters;
    census.settlers_trained = counters.get("trained:settler").copied().unwrap_or(0);
    census.units_trained = counters
        .iter()
        .filter(|(key, _)| key.starts_with("trained:"))
        .map(|(_, count)| *count)
        .sum();
    census.captures = counters.get("captures").copied().unwrap_or(0);
    census.payments = oracle.fired();
    census.won = game.winner == Some(oracle_seat);
    census
}

fn report(label: &str, runs: &[Census], games: f64) {
    let payments: u64 = runs.iter().map(|run| run.payments).sum();
    let eligible: u64 = runs.iter().map(|run| run.eligible).sum();
    let cities: usize = runs.iter().map(|run| run.cities_at_end).sum();
    let peak: usize = runs.iter().map(|run| run.peak_cities).sum();
    let settlers: i64 = runs.iter().map(|run| run.settlers_trained).sum();
    let units: i64 = runs.iter().map(|run| run.units_trained).sum();
    let captures: i64 = runs.iter().map(|run| run.captures).sum();
    let reached: Vec<u32> = runs.iter().filter_map(|run| run.reached_target).collect();
    let settler_turns: u64 = runs.iter().map(|run| run.settler_turns).sum();
    let settler_tiles: u64 = runs.iter().map(|run| run.settler_tiles).sum();
    let settler_still: u64 = runs.iter().map(|run| run.settler_still).sum();
    let settlers_seen: u64 = runs.iter().map(|run| run.settlers_seen).sum();
    let wins = runs.iter().filter(|run| run.won).count();
    println!("\ngrant {label}");
    println!(
        "  cities at end       {:.2}   peak {:.2}",
        cities as f64 / games,
        peak as f64 / games
    );
    println!(
        "  where they came from: settlers trained {:.2}, captures {:.2}, \
of {:.1} units trained",
        settlers as f64 / games,
        captures as f64 / games,
        units as f64 / games
    );
    if reached.is_empty() {
        println!("  reached the target  never, in any cell");
    } else {
        println!(
            "  reached the target  {}/{} cells, first at turn {:.0} on average",
            reached.len(),
            runs.len(),
            reached.iter().map(|turn| *turn as f64).sum::<f64>() / reached.len() as f64
        );
    }
    println!(
        "  PAYMENTS            {payments} ({:.1} per game) over {eligible} eligible turns ({:.1} per game)",
        payments as f64 / games,
        eligible as f64 / games
    );
    println!("  won                 {wins}/{} — NOT a win rate", runs.len());
    if settlers_seen > 0 {
        let stepped = (settler_tiles + settler_still).max(1);
        println!(
            "  settler transit     {:.2} settlers/game, each alive {:.1} turns; \
{:.2} tiles/turn; stood still on {:.1}% of its turns",
            settlers_seen as f64 / games,
            settler_turns as f64 / settlers_seen as f64,
            settler_tiles as f64 / stepped as f64,
            100.0 * settler_still as f64 / stepped as f64
        );
    }
    let samples: u64 = runs.iter().map(|run| run.target_samples).sum();
    let satisfied: u64 = runs.iter().map(|run| run.eligible_but_satisfied).sum();
    let planless: u64 = runs.iter().map(|run| run.eligible_without_plan).sum();
    if samples > 0 {
        let target: u64 = runs.iter().map(|run| run.target_at_eligible).sum();
        println!(
            "  the agent's OWN target on those turns: {:.2} cities; it already had \
that many on {:.1}% of them{}",
            target as f64 / samples as f64,
            100.0 * satisfied as f64 / samples as f64,
            if planless > 0 {
                format!(" ({planless} more had no plan yet)")
            } else {
                String::new()
            }
        );
    }
    let mut heads: BTreeMap<String, u64> = BTreeMap::new();
    for run in runs {
        for (head, count) in &run.heads {
            *heads.entry(head.clone()).or_insert(0) += count;
        }
    }
    if eligible > 0 {
        println!("  what it was building on an eligible turn:");
        let mut rows: Vec<(&String, &u64)> = heads.iter().collect();
        rows.sort_by(|a, b| b.1.cmp(a.1));
        for (head, count) in rows {
            println!(
                "    {head:<22} {count:>6}  {:>5.1}%",
                100.0 * *count as f64 / eligible as f64
            );
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let games = number(&args, "--games", 12).max(1) as usize;
    let players = number(&args, "--players", 4).max(2) as usize;
    let seed = number(&args, "--seed", 470_000).max(0) as u64;
    let turns = number(&args, "--turns", 500).max(1) as u32;
    let jobs = match number(&args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    let size = MapSize::for_players(players);
    let (default_width, default_height) = size.dimensions(Default::default());
    let width = number(&args, "--width", default_width as i64).max(8) as i32;
    let height = number(&args, "--height", default_height as i64).max(8) as i32;
    let city_states =
        number(&args, "--city-states", size.default_city_states as i64).max(0) as usize;
    let speed = text(&args, "--speed", &civvis::game::default_speed());
    let ai_name = text(&args, "--ai", "advanced");
    let rules = Rules::embedded();
    if !rules.speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }
    let difficulty = text(&args, "--difficulty", &default_difficulty());
    if !rules.difficulties.contains_key(&difficulty) {
        eprintln!("unknown difficulty {difficulty:?}");
        std::process::exit(2);
    }

    // The same (map, seat) cells for all three arms, so the three histograms
    // and the three city counts describe the same starts.
    let cells: Vec<(usize, usize)> = (0..games)
        .flat_map(|map| [0usize, players - 1].into_iter().map(move |seat| (map, seat)))
        .collect();
    let options_for = |cell: (usize, usize)| {
        let mut options = GameOptions::new(
            players,
            width,
            height,
            seed + cell.0 as u64,
            turns,
            city_states,
        );
        options.difficulty = difficulty.clone();
        options.speed = speed.clone();
        options.human_seats = BTreeSet::from([cell.1]);
        options
    };

    println!(
        "profile: {players}p {width}x{height}, {city_states} city-states, \
{turns} {speed} turns, seed {seed}, {jobs} jobs, difficulty {difficulty}, {ai_name}"
    );
    println!(
        "{} cells ({games} maps x 2 seats) played under each of none, rebate, expansion",
        cells.len()
    );
    println!("this is a census of what the money bought, not an evaluation of whether it helped");

    let arm_games = cells.len() as f64;
    for grant in [Grant::None, Grant::Rebate, Grant::Expansion] {
        let runs: Vec<Census> = civvis::parallel::map_reporting(
            cells.len(),
            jobs,
            |index| play(options_for(cells[index]), cells[index].1, grant, &ai_name),
            |index, _| println!("  {} progress {}/{}", grant.name(), index + 1, cells.len()),
        );
        report(grant.name(), &runs, arm_games);
    }

    println!(
        "\nread, in this order:\n  1. payout turns per game — `rebate` and `expansion` must be close, or the rebate is \
not a cost-matched control but a larger gift wearing its name.\n  2. settlers trained against captures — a grant that raises the city count without \
raising settlers trained did not buy expansion, it bought an army.\n  3. the win column is NOT a win rate and this sample cannot resolve one."
    );
}
