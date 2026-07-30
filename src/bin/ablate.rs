//! Oracle ablation: measure the headroom in one subsystem at a time.
//!
//! For a subsystem S, this plays the stock agent against a copy of itself
//! that has been handed a free, cheating version of S, on mirrored maps with
//! seats swapped. The resulting paired win rate is an upper bound on
//! everything any amount of honest work on S could be worth. A grant that
//! wins nothing settles that subsystem for the price of a batch of games
//! rather than the price of a design and a pre-registered run.
//!
//! ```bash
//! cargo run --release --bin ablate -- --grant modernity --pairs 60 --players 4
//! cargo run --release --bin ablate -- --grant treasury,ground --pairs 40
//! cargo run --release --bin ablate -- --grant all --ai strategic_deep --speed online
//! ```
//!
//! `--grant none` is the control and must land at parity; if it does not, the
//! harness is reporting its own noise as headroom.
use civvis::ai::{AdvancedAi, Ai, VictoryTarget};
use civvis::elo::{builtin_ai, builtin_provenance, BUILTIN_AIS, EVAL_ONLY_AIS};
use civvis::game::{default_difficulty, Action, Game, GameOptions};
use civvis::oracle::{Grant, Oracle};
use civvis::rules::Rules;
use civvis::setup::MapSize;
use std::collections::BTreeSet;

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
    BUILTIN_AIS.contains(&name) || EVAL_ONLY_AIS.contains(&name)
}

/// Parse one grant, `all`, or a comma-separated set sharing one control.
fn parse_grants(requested: &str) -> Result<Vec<Grant>, String> {
    if requested == "all" {
        return Ok(Grant::ALL.to_vec());
    }
    let mut grants = Vec::new();
    for name in requested.split(',') {
        if name.is_empty() || name == "all" {
            return Err(format!("invalid grant list {requested:?}"));
        }
        let grant = Grant::from_id(name).ok_or_else(|| format!("unknown grant {name:?}"))?;
        if !grants.contains(&grant) {
            grants.push(grant);
        }
    }
    if grants.is_empty() {
        return Err("grant list is empty".to_string());
    }
    Ok(grants)
}

/// One game. `oracle_seat` is the seat holding the grant; every other major
/// plays the stock agent. Returns whether the granted seat won, how many times
/// its grant fired, and how many raw envoys it handed over.
///
/// The third value exists because a firing *count* cannot express a budget. On
/// the envoy axis two grants can fire a different number of times while moving
/// the same resource, or fire equally while moving very different amounts, and
/// `Grant::Envoys` is only a control for `Grant::Suzerain` if the amounts
/// match. #584 is the standing warning: its rebate shared the expansion grant's
/// firing condition, passed a same-position test, and still handed over twelve
/// times the gift over a trajectory. A control whose budget is never printed is
/// not a control.
fn play(
    options: GameOptions,
    oracle_seat: usize,
    grant: Grant,
    ai_name: &str,
) -> (bool, u64, i64) {
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
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        if pid == oracle_seat {
            oracle.take_turn(&mut game, pid);
        } else {
            stock[pid].take_turn(&mut game, pid);
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    (
        game.winner == Some(oracle_seat),
        oracle.fired(),
        oracle.envoys_granted(),
    )
}

/// Wilson score interval, the same statistic the promotion gate uses.
fn wilson(wins: f64, n: f64) -> (f64, f64) {
    if n <= 0.0 {
        return (0.0, 1.0);
    }
    let z = 1.959_963_984_540_054_f64;
    let p = wins / n;
    let denominator = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / denominator;
    let spread = z * ((p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt()) / denominator;
    ((center - spread).max(0.0), (center + spread).min(1.0))
}

/// Exact two-sided binomial tail for `hits` of `n` at p=1/2.
///
/// Used for McNemar's test over discordant pairs: under the null that the
/// grant changes nothing, a pair that disagrees is equally likely to
/// disagree either way.
fn exact_two_sided(hits: u32, n: u32) -> f64 {
    if n == 0 {
        return 1.0;
    }
    let mut coefficient = 1.0_f64;
    let mut tail = 0.0_f64;
    let extreme = hits.min(n - hits);
    for k in 0..=n {
        if k > 0 {
            coefficient *= (n - k + 1) as f64 / k as f64;
        }
        if k <= extreme || k >= n - extreme {
            tail += coefficient;
        }
    }
    (tail / 2f64.powi(n as i32)).min(1.0)
}

/// One (map, seat) cell: the same game played with the grant and without it.
#[derive(Clone, Copy)]
struct Cell {
    map: usize,
    seat: usize,
}

fn run(grants: &[Grant], args: &[String]) {
    let pairs = number(args, "--pairs", 40).max(1) as usize;
    let players = number(args, "--players", 4).max(2) as usize;
    let seed = number(args, "--seed", 310_000).max(0) as u64;
    let turns = number(args, "--turns", 500).max(1) as u32;
    let jobs = match number(args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    // The stock map profile for this player count. A map small enough that
    // every army is already next to every city would hide a logistics grant
    // by making logistics free for both sides.
    let size = MapSize::for_players(players);
    let (default_width, default_height) = size.dimensions(Default::default());
    let width = number(args, "--width", default_width as i64).max(8) as i32;
    let height = number(args, "--height", default_height as i64).max(8) as i32;
    let city_states =
        number(args, "--city-states", size.default_city_states as i64).max(0) as usize;
    let speed = text(args, "--speed", &civvis::game::default_speed());
    let ai_name = text(args, "--ai", "advanced");
    if !known_ai(&ai_name) {
        eprintln!("unknown AI {ai_name:?}; choose a builtin or evaluator-only AI name");
        std::process::exit(2);
    }
    let rules = Rules::embedded();
    if !rules.speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }
    let provenance = builtin_provenance(&ai_name, "evolved");
    println!("agent: {}", provenance.line());
    if provenance.degraded() {
        eprintln!(
            "refusing to record {ai_name:?}: it resolves to {:?}",
            provenance.effective
        );
        std::process::exit(3);
    }
    // A grant's p-value says whether a subsystem limits the agent, not by how
    // much. The difficulty ladder supplies the missing scale: the granted seat
    // plays the human side of the handicap and its opponents the AI side, so
    // `--grant none --difficulty king` measures what a known, documented
    // advantage is worth on this exact harness. Every other grant can then be
    // read against it instead of against nothing.
    let difficulty = text(args, "--difficulty", &default_difficulty());
    if !rules.difficulties.contains_key(&difficulty) {
        eprintln!("unknown difficulty {difficulty:?}");
        std::process::exit(2);
    }
    println!(
        "profile: {players}p {width}x{height}, {city_states} city-states, \
{turns} {speed} turns, seed {seed}, {jobs} jobs, difficulty {difficulty}"
    );
    println!(
        "sampling seats 0 and {}; fixed stock civilizations; Basic city-states/barbarians",
        players - 1
    );

    // Every map is played from two different seats so a map that simply
    // favours one start cannot be read as evidence about a grant.
    let cells: Vec<Cell> = (0..pairs)
        .flat_map(|map| {
            [0usize, players - 1]
                .into_iter()
                .map(move |seat| Cell { map, seat })
        })
        .collect();

    let options_for = |cell: Cell| {
        let mut options = GameOptions::new(
            players,
            width,
            height,
            seed + cell.map as u64,
            turns,
            city_states,
        );
        options.difficulty = difficulty.clone();
        options.speed = speed.clone();
        // Only the granted seat sits on the human side of the ladder, so the
        // handicap moves with the grant rather than with the map.
        options.human_seats = BTreeSet::from([cell.seat]);
        options
    };

    // The control is played once and shared by every grant. Each grant is then
    // compared against it cell by cell — same map, same seat, same seed — so
    // the comparison is matched and map variance drops out instead of being
    // averaged over. Comparing a granted seat's raw win rate against 1/players
    // instead would have to carry all of that variance, which at these sample
    // sizes is most of the signal.
    println!(
        "playing {} control games: {ai_name}, {speed} speed, difficulty {difficulty}...",
        cells.len()
    );
    let control: Vec<bool> = civvis::parallel::map_reporting(
        cells.len(),
        jobs,
        |index| {
            play(
                options_for(cells[index]),
                cells[index].seat,
                Grant::None,
                &ai_name,
            )
            .0
        },
        |index, _| println!("  control progress {}/{}", index + 1, cells.len()),
    );
    let control_wins = control.iter().filter(|won| **won).count();
    println!(
        "control: granted seat won {control_wins}/{} = {:.1}% (parity {:.1}%)\n",
        cells.len(),
        100.0 * control_wins as f64 / cells.len() as f64,
        100.0 / players as f64
    );

    for &grant in grants {
        println!("playing {} treatment games for {}...", cells.len(), grant.name());
        let played = civvis::parallel::map_reporting(
            cells.len(),
            jobs,
            |index| {
                play(
                    options_for(cells[index]),
                    cells[index].seat,
                    grant,
                    &ai_name,
                )
            },
            |index, _| {
                println!(
                    "  {} progress {}/{}",
                    grant.name(),
                    index + 1,
                    cells.len()
                )
            },
        );
        let treated: Vec<bool> = played.iter().map(|(won, _, _)| *won).collect();
        let fired: u64 = played.iter().map(|(_, fired, _)| *fired).sum();
        let envoys_granted: i64 = played.iter().map(|(_, _, envoys)| *envoys).sum();

        let wins = treated.iter().filter(|won| **won).count();
        // McNemar: only the cells where the grant changed the outcome carry
        // information about the grant.
        let mut helped = 0u32;
        let mut hurt = 0u32;
        for (with, without) in treated.iter().zip(&control) {
            match (with, without) {
                (true, false) => helped += 1,
                (false, true) => hurt += 1,
                _ => {}
            }
        }
        let discordant = helped + hurt;
        let p = exact_two_sided(helped, discordant);
        let n = cells.len() as f64;

        println!(
            "grant {:<10} {pairs} maps x 2 seats, {players} players, {turns} {speed} turns, {ai_name}",
            grant.name()
        );
        println!(
            "  granted seat won    {wins}/{} = {:.1}%   (control {control_wins} = {:.1}%)",
            cells.len(),
            100.0 * wins as f64 / n,
            100.0 * control_wins as f64 / n
        );
        println!(
            "  matched pairs       grant won where control lost: {helped}; \
lost where control won: {hurt}; unchanged: {}",
            cells.len() as u32 - discordant
        );
        println!("  McNemar exact       p={p:.4} over {discordant} discordant cells");
        println!(
            "  grant fired         {fired} times ({:.1} per game)",
            fired as f64 / n
        );
        // Only the envoy grants move this, so stay silent otherwise rather
        // than printing a zero on eight unrelated arms.
        if envoys_granted > 0 {
            println!(
                "  raw envoys granted  {envoys_granted} ({:.1} per game)",
                envoys_granted as f64 / n
            );
        }
        if grant != Grant::None && fired == 0 {
            println!(
                "  WARNING: the grant never fired, so this measured the stock \
agent under an oracle's name and says nothing about {}",
                grant.name()
            );
        }
        let verdict = if grant == Grant::None {
            if discordant == 0 {
                "SANITY OK — the null grant reproduced the control exactly, so \
the harness is deterministic and adds nothing of its own"
            } else {
                "BROKEN — the null grant changed outcomes, so every number \
here includes harness noise and none of it can be trusted"
            }
        } else if discordant < 8 {
            "TOO FEW DISCORDANT CELLS to say anything — raise --pairs"
        } else if p >= 0.05 {
            "NO MEASURABLE HEADROOM — perfecting this subsystem is worth less \
than this run can resolve"
        } else if helped > hurt {
            "HEADROOM — this subsystem limits the agent; work on it can pay"
        } else {
            "HARMFUL — free perfection here loses, so the grant is \
mis-specified rather than the subsystem being fine"
        };
        println!("  verdict             {verdict}");
        println!();
    }
}

/// One game played by a seat committed to `target` from the first turn, or
/// adaptive when `target` is `None`. Every other major plays the stock
/// adaptive agent.
fn play_lane(options: GameOptions, seat: usize, target: Option<VictoryTarget>) -> bool {
    let mut game = Game::new_with(options);
    let mut stock = AdvancedAi::fleet(&game);
    let mut routed = match target {
        Some(target) => AdvancedAi::targeting(target),
        None => AdvancedAi::new(),
    };
    while game.winner.is_none() && game.turn <= game.max_turns {
        let pid = game.current;
        if pid == seat {
            routed.take_turn(&mut game, pid);
        } else {
            stock[pid].take_turn(&mut game, pid);
        }
        if game.winner.is_none() && game.current == pid {
            let _ = game.apply(pid, &Action::EndTurn);
        }
    }
    game.winner == Some(seat)
}

/// What perfect victory routing is worth.
///
/// The three capability grants all measured null, which leaves the other
/// half of the question open: capability is not what limits this agent, but
/// is *choosing what to play for*? This bounds that directly. Each cell is
/// played once per victory lane with the seat committed to it from turn one,
/// and once adaptively. Taking the maximum over lanes is an oracle no agent
/// could implement — it needs the result before the decision — so the gap
/// between it and the adaptive agent is the entire headroom in victory
/// routing, search and priors included.
///
/// If this is also null, then neither what this agent can do nor what it
/// chooses to do decides these games, and the search has to move off the
/// agent altogether.
fn run_best_lane(args: &[String]) {
    let pairs = number(args, "--pairs", 40).max(1) as usize;
    let players = number(args, "--players", 4).max(2) as usize;
    let seed = number(args, "--seed", 310_000).max(0) as u64;
    let turns = number(args, "--turns", 500).max(1) as u32;
    let jobs = match number(args, "--jobs", 0) {
        requested if requested > 0 => requested as usize,
        _ => civvis::parallel::default_jobs(),
    };
    let size = MapSize::for_players(players);
    let (default_width, default_height) = size.dimensions(Default::default());
    let width = number(args, "--width", default_width as i64).max(8) as i32;
    let height = number(args, "--height", default_height as i64).max(8) as i32;
    let city_states =
        number(args, "--city-states", size.default_city_states as i64).max(0) as usize;
    let speed = text(args, "--speed", &civvis::game::default_speed());
    if !Rules::embedded().speeds.contains_key(&speed) {
        eprintln!("unknown game speed {speed:?}");
        std::process::exit(2);
    }

    let cells: Vec<Cell> = (0..pairs)
        .flat_map(|map| {
            [0usize, players - 1]
                .into_iter()
                .map(move |seat| Cell { map, seat })
        })
        .collect();
    let options_for = |cell: Cell| GameOptions {
        speed: speed.clone(),
        ..GameOptions::new(
            players,
            width,
            height,
            seed + cell.map as u64,
            turns,
            city_states,
        )
    };

    // One job per (cell, lane) so the whole grid runs across every core
    // rather than one lane at a time.
    let lanes = VictoryTarget::ALL;
    let total = cells.len() * (lanes.len() + 1);
    println!(
        "playing {total} games: {} cells x ({} lanes + adaptive), {players} players, {turns} {speed} turns",
        cells.len(),
        lanes.len()
    );
    let grid = civvis::parallel::map(total, jobs, |index| {
        let cell = cells[index / (lanes.len() + 1)];
        let slot = index % (lanes.len() + 1);
        let target = (slot > 0).then(|| lanes[slot - 1]);
        play_lane(options_for(cell), cell.seat, target)
    });

    let mut adaptive_wins = 0u32;
    let mut best_wins = 0u32;
    let mut best_only = 0u32;
    let mut adaptive_only = 0u32;
    let mut per_lane = vec![0u32; lanes.len()];
    for (index, cell) in cells.iter().enumerate() {
        let _ = cell;
        let base = index * (lanes.len() + 1);
        let adaptive = grid[base];
        let mut best = false;
        for (lane, slot) in (1..=lanes.len()).enumerate() {
            if grid[base + slot] {
                best = true;
                per_lane[lane] += 1;
            }
        }
        adaptive_wins += u32::from(adaptive);
        best_wins += u32::from(best);
        match (best, adaptive) {
            (true, false) => best_only += 1,
            (false, true) => adaptive_only += 1,
            _ => {}
        }
    }
    let n = cells.len() as f64;
    let discordant = best_only + adaptive_only;
    let p = exact_two_sided(best_only, discordant);
    println!();
    println!("best-lane oracle   {} cells, {players} players, {turns} {speed} turns", cells.len());
    println!("  adaptive won        {adaptive_wins}/{} = {:.1}%   (parity {:.1}%)",
        cells.len(), 100.0 * adaptive_wins as f64 / n, 100.0 / players as f64);
    println!("  SOME lane won       {best_wins}/{} = {:.1}%",
        cells.len(), 100.0 * best_wins as f64 / n);
    println!("  matched pairs       a lane won where adaptive lost: {best_only}; \
adaptive won where no lane did: {adaptive_only}");
    println!("  McNemar exact       p={p:.4} over {discordant} discordant cells");
    for (lane, wins) in lanes.iter().zip(&per_lane) {
        println!("    committed {:<11} {wins}/{} = {:.1}%",
            format!("{lane:?}"), cells.len(), 100.0 * *wins as f64 / n);
    }
    let verdict = if discordant < 8 {
        "TOO FEW DISCORDANT CELLS to say anything — raise --pairs"
    } else if p >= 0.05 {
        "NO MEASURABLE HEADROOM IN VICTORY ROUTING — knowing the right lane \
in advance does not win these games"
    } else if best_only > adaptive_only {
        "HEADROOM IN VICTORY ROUTING — choosing the lane better can pay, and \
this is the ceiling on it"
    } else {
        "COMMITMENT IS HARMFUL — adapting beats every fixed lane, so routing \
is not a choice worth improving"
    };
    println!("  verdict             {verdict}");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if text(&args, "--mode", "grants") == "best-lane" {
        if args.iter().any(|arg| arg == "--ai") {
            eprintln!("--ai applies only to grant mode; best-lane is an AdvancedAi experiment");
            std::process::exit(2);
        }
        println!(
            "Best-lane oracle. Each cell is played once per victory lane with the seat\n\
             committed to it from turn one, and once adaptively. The maximum over lanes is\n\
             an oracle no agent could implement, so the gap to the adaptive agent is the\n\
             whole headroom in victory routing.\n"
        );
        run_best_lane(&args);
        return;
    }
    let requested = text(&args, "--grant", "all");
    let grants = match parse_grants(&requested) {
        Ok(grants) => grants,
        Err(error) => {
            eprintln!(
                "{error}; choose one or more of {:?}, or all",
                Grant::ALL.map(Grant::name)
            );
            std::process::exit(2);
        }
    };
    println!(
        "Oracle ablation. Each grant hands one seat a free, cheating version of one\n\
         subsystem and plays it against stock agents on mirrored maps. The result is an\n\
         UPPER BOUND on what honest work on that subsystem could be worth, never a\n\
         playable agent. `none` is the control and must land at parity.\n"
    );
    run(&grants, &args);
}

#[cfg(test)]
mod tests {
    use super::{known_ai, parse_grants};
    use civvis::oracle::Grant;

    #[test]
    fn comma_separated_grants_share_a_control_without_duplicates() {
        assert_eq!(
            parse_grants("treasury,ground,treasury").unwrap(),
            vec![Grant::Treasury, Grant::Ground]
        );
        assert_eq!(parse_grants("all").unwrap(), Grant::ALL);
        assert!(parse_grants("all,ground").is_err());
        assert!(parse_grants("treasury,unknown").is_err());
    }

    #[test]
    fn deployment_controllers_are_known_ai_names() {
        assert!(known_ai("advanced"));
        assert!(known_ai("strategic_deep"));
        assert!(!known_ai("strategic_typo"));
    }
}
