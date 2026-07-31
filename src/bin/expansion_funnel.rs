//! Why does this empire stop short of the number of cities it wants?
//!
//! The oracle ablation (PR #366) put the leverage in the **economy**: three
//! free military superpowers measured null against a calibration that scored
//! 62-0 for a pure resource grant. City count is the largest single multiplier
//! on an economy, and the same PR measured the agent **wanting six cities and
//! settling 4.86** — then closed with the honest note that *"expansion is
//! limited by execution rather than by ambition. Whatever binds sits in settler
//! production, site availability, or the expansion window."* Nobody has gone
//! and looked.
//!
//! `production_value` (`advanced.rs:7044`) will only pay for a settler when
//! **all five** of these hold:
//!
//! 1. `city_count + settlers_in_flight < plan.desired_cities` — room in the plan
//! 2. `settlers_in_flight < in_flight_allowed` — one at a time unless widened
//! 3. `city.pop >= 2` — the city is big enough to lose a citizen
//! 4. the expansion window is still open
//! 5. `best_settle_site(...)` returned somewhere to go
//!
//! Fail any one and the item scores `-10_000`. Pass all five and it scores
//! `920 + site*4` — which still has to **beat every other item in the queue**.
//!
//! Those are six different defects with six different repairs, and the recorded
//! evidence does not distinguish them. This samples the focal empire every turn
//! and attributes each turn it did not start a settler while short of its
//! target to the *first* condition that failed, in the order the code tests
//! them. The residual bucket — every gate passed and no settler appeared — is
//! the interesting one: it means the settler was **out-competed on value**,
//! not forbidden.
//!
//! Conditions 1-4 are reproduced here from the same formulas the agent uses, so
//! they are exact. Condition 5 is not reachable from outside the agent, so a
//! turn that clears 1-4 lands in a combined `site-or-value` bucket and the tool
//! says so rather than guessing.
//!
//! ```text
//! expansion_funnel --maps 12 --players 4 --turns 500
//! ```
//!
//! Diagnostic only: it never changes a decision unless an explicit evaluator
//! arm is named, and even then it only selects a default-off agent flag.
use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions, VictoryConditions};
use civvis::parallel;
use civvis::setup::{MapPoles, MapScript, MapTopology};

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn text(args: &[String], flag: &str, default: &str) -> String {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .cloned()
        .unwrap_or_else(|| default.to_string())
}

/// The frozen factorial arms for the adaptive-expansion mechanism census.
/// `None` preserves this binary's historical deployed-agent diagnostic mode.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpansionArm {
    Stock,
    Late,
    Dispatch,
    Complete,
}

impl ExpansionArm {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "stock" | "advanced" => Some(Self::Stock),
            "late" | "advanced_late_expansion" => Some(Self::Late),
            "dispatch" | "advanced_expansion_dispatch" => Some(Self::Dispatch),
            "complete" | "advanced_expansion_complete" => Some(Self::Complete),
            _ => None,
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::Stock => "advanced",
            Self::Late => "advanced_late_expansion",
            Self::Dispatch => "advanced_expansion_dispatch",
            Self::Complete => "advanced_expansion_complete",
        }
    }

    fn configure(self, ai: &mut AdvancedAi) {
        ai.late_expansion = matches!(self, Self::Late | Self::Complete);
        ai.expansion_dispatch = matches!(self, Self::Dispatch | Self::Complete);
    }
}

fn turn_list(turns: &[u32]) -> String {
    let mut sorted = turns.to_vec();
    sorted.sort_unstable();
    if sorted.is_empty() {
        "none".to_string()
    } else {
        sorted
            .into_iter()
            .map(|turn| turn.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Report the factual 2×2 mechanism data. Nothing here selects a winner,
/// changes an arm, or inspects an outcome seed; it only tells the fixed gates
/// whether the proposed dispatcher and late-window mechanisms actually fired.
fn print_mechanism_census(arm: ExpansionArm, maps: &[MapResult]) {
    let Some(first) = maps.first() else {
        return;
    };
    let seats: Vec<&Funnel> = maps
        .iter()
        .flat_map(|result| result.seats.iter())
        .collect();
    let Some(sample) = seats.first() else {
        return;
    };
    let stock_deadline = sample.stock_deadline;
    let late_deadline = sample.late_deadline;
    println!(
        "\n=== fixed adaptive-expansion mechanism census [{}] ===",
        arm.id()
    );
    println!(
        "realized map {}x{}; stock deadline {stock_deadline}; late deadline {late_deadline}; newly opened interval [{stock_deadline},{late_deadline})",
        first.actual_width, first.actual_height,
    );
    println!("by map (all fields aggregate that map's major seats):");
    for result in maps {
        let calls = result
            .seats
            .iter()
            .map(|seat| seat.dispatch_calls)
            .sum::<usize>();
        let productions = result
            .seats
            .iter()
            .map(|seat| seat.dispatch_productions)
            .sum::<usize>();
        let settlers: Vec<u32> = result
            .seats
            .iter()
            .flat_map(|seat| seat.dispatch_settler_turns.iter().copied())
            .collect();
        let late_dispatch = settlers
            .iter()
            .filter(|turn| **turn >= stock_deadline && **turn < late_deadline)
            .count();
        let advanced_late: Vec<u32> = result
            .seats
            .iter()
            .flat_map(|seat| seat.advanced_late_settler_turns.iter().copied())
            .collect();
        let founded = result
            .seats
            .iter()
            .map(|seat| seat.founded_cities_final)
            .sum::<usize>();
        println!(
            "  seed {}: seats {}, calls {calls}, successful produces {productions}, \
             dispatcher Settlers {} (late {late_dispatch}; turns [{}]), \
             all Advanced late Settlers {} (turns [{}]), founded cities {founded}",
            result.seed,
            result.seats.len(),
            settlers.len(),
            turn_list(&settlers),
            advanced_late.len(),
            turn_list(&advanced_late),
        );
    }

    let calls = seats.iter().map(|seat| seat.dispatch_calls).sum::<usize>();
    let productions = seats
        .iter()
        .map(|seat| seat.dispatch_productions)
        .sum::<usize>();
    let production_seats = seats
        .iter()
        .filter(|seat| seat.dispatch_productions > 0)
        .count();
    let settler_seats = seats
        .iter()
        .filter(|seat| !seat.dispatch_settler_turns.is_empty())
        .count();
    let dispatch_settlers: Vec<u32> = seats
        .iter()
        .flat_map(|seat| seat.dispatch_settler_turns.iter().copied())
        .collect();
    let dispatch_late_seats = seats
        .iter()
        .filter(|seat| {
            seat.dispatch_settler_turns
                .iter()
                .any(|turn| *turn >= stock_deadline && *turn < late_deadline)
        })
        .count();
    let dispatch_late = dispatch_settlers
        .iter()
        .filter(|turn| **turn >= stock_deadline && **turn < late_deadline)
        .count();
    let advanced_late_seats = seats
        .iter()
        .filter(|seat| !seat.advanced_late_settler_turns.is_empty())
        .count();
    let advanced_late: Vec<u32> = seats
        .iter()
        .flat_map(|seat| seat.advanced_late_settler_turns.iter().copied())
        .collect();
    let founded = seats
        .iter()
        .map(|seat| seat.founded_cities_final)
        .sum::<usize>();
    println!("aggregate across {} major seat-maps:", seats.len());
    println!(
        "  dispatcher calls {calls}; successful produces {productions} on {production_seats}/{} seats",
        seats.len(),
    );
    println!(
        "  dispatcher Settlers {} on {settler_seats}/{} seats; late {} on {dispatch_late_seats}/{} seats; turns [{}]",
        dispatch_settlers.len(),
        seats.len(),
        dispatch_late,
        seats.len(),
        turn_list(&dispatch_settlers),
    );
    println!(
        "  all Advanced late Settlers {} on {advanced_late_seats}/{} seats; turns [{}]",
        advanced_late.len(),
        seats.len(),
        turn_list(&advanced_late),
    );
    println!("  final founded cities excluding captures {founded}");
}

#[derive(Default, Clone)]
struct Funnel {
    /// Turns the empire was already at or over its planned city target.
    at_target: usize,
    /// Turns a settler was already walking.
    settler_in_flight: usize,
    /// Turns no city had reached pop 2.
    no_city_big_enough: usize,
    /// Turns the expansion window had closed.
    window_closed: usize,
    /// Turns every gate passed and a site existed, but every city was already
    /// mid-build. `advanced_production` skips any city whose queue is
    /// non-empty, so the settler was never scored at all.
    all_cities_busy: usize,
    /// Turns every gate passed, a site existed, and at least one city was free
    /// to choose — and still chose something else. This is the only bucket that
    /// is genuinely a valuation loss.
    lost_on_value: usize,
    /// Turns every other gate passed but no city could see anywhere to go.
    no_site: usize,
    /// Turns a settler was actually started.
    started: usize,
    cities_final: usize,
    target_final: usize,
    /// Successful actions made by the newly exposed adaptive-Expansion
    /// dispatcher. These are copied from the agent only after the game ends.
    dispatch_calls: usize,
    dispatch_productions: usize,
    dispatch_settler_turns: Vec<u32>,
    advanced_late_settler_turns: Vec<u32>,
    /// Owned cities founded by this player, excluding captured cities. The
    /// capital is a founded city too, intentionally matching the fixed gate's
    /// aggregate comparison across otherwise identical arms.
    founded_cities_final: usize,
    stock_deadline: u32,
    late_deadline: u32,
}

struct MapResult {
    seed: u64,
    actual_width: i32,
    actual_height: i32,
    seats: Vec<Funnel>,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let players = number(&args, "--players", 4);
    let maps = number(&args, "--maps", 12);
    let width = number(&args, "--width", 60) as i32;
    let height = number(&args, "--height", 38) as i32;
    let city_states = number(&args, "--city-states", 6);
    let turns = number(&args, "--turns", 500) as u32;
    let seed0 = number(&args, "--seed", 2_400_000) as u64;
    let jobs = number(&args, "--jobs", parallel::default_jobs());
    let arm = args
        .iter()
        .position(|arg| arg == "--arm")
        .and_then(|index| args.get(index + 1))
        .map(|name| {
            ExpansionArm::parse(name).unwrap_or_else(|| {
                eprintln!(
                    "unknown --arm {name:?}; choose stock, late, dispatch, or complete"
                );
                std::process::exit(2);
            })
        });
    let speed = text(&args, "--speed", "standard");
    let map_name = text(&args, "--map", MapScript::default().id());
    let map_script = MapScript::from_id(&map_name).unwrap_or_else(|| {
        eprintln!("unknown map script {map_name:?}");
        std::process::exit(2);
    });
    let shape_name = text(&args, "--shape", MapTopology::default().id());
    let map_topology = MapTopology::from_id(&shape_name).unwrap_or_else(|| {
        eprintln!("unknown map shape {shape_name:?}");
        std::process::exit(2);
    });
    let poles_name = text(&args, "--poles", MapPoles::default().id());
    let map_poles = MapPoles::from_id(&poles_name).unwrap_or_else(|| {
        eprintln!("unknown thermal distribution {poles_name:?}");
        std::process::exit(2);
    });
    let victory_names = text(
        &args,
        "--victories",
        &VictoryConditions::NAMES.join(","),
    );
    let victory_conditions = VictoryConditions::parse(&victory_names).unwrap_or_else(|why| {
        eprintln!("--victories: {why}");
        std::process::exit(2);
    });
    let randomize_civs = args.iter().any(|arg| arg == "--randomize-civs");
    // Fires-check lever: run the same census with the treatment applied. A
    // treatment that does not collapse the `LOST ON VALUE` bucket is not
    // reaching the decision, and evaluating it would measure the stock agent
    // under another name.
    let settler_price = args
        .iter()
        .position(|arg| arg == "--settler-price")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);
    let preempt_margin = args
        .iter()
        .position(|arg| arg == "--preempt-margin")
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse::<f64>().ok())
        .unwrap_or(1.0);

    println!(
        "expansion_funnel: {maps} maps, {players}p requested {width}x{height}, {city_states} city-states, \
         {turns} turns, seed {seed0}"
    );
    println!(
        "profile: speed {speed}, map {}, shape {}, poles {}, civilizations {}, victories {}",
        map_script.id(),
        map_topology.id(),
        map_poles.id(),
        if randomize_civs {
            "randomized"
        } else {
            "fixed"
        },
        VictoryConditions::NAMES
            .into_iter()
            .filter(|name| victory_conditions.is_enabled(name))
            .collect::<Vec<_>>()
            .join(","),
    );
    if let Some(arm) = arm {
        println!("mechanism arm: {}", arm.id());
    } else {
        println!("mechanism arm: none (historical deployed-agent diagnostic)");
    }
    println!("settler_price {settler_price}, preempt_margin {preempt_margin}");
    println!("every major seat sampled every turn; attribution is first-failing-gate\n");

    let per_map = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new_with(GameOptions {
            speed: speed.clone(),
            map_script,
            map_topology,
            map_poles,
            randomize_civs,
            ..GameOptions::new(players, width, height, seed, turns, city_states)
        });
        game.victory_conditions = victory_conditions;
        let actual_width = game.map.width;
        let actual_height = game.map.height;
        let mut fleet: Vec<AdvancedAi> = if arm.is_some() {
            // The fixed factorial compares the named `advanced` factory, not
            // the evolved deployed agent the legacy diagnostic samples.
            AdvancedAi::fleet(&game)
        } else {
            let genome = civvis::evolve::load_champion("evolved").unwrap_or_default();
            AdvancedAi::fleet_weighted(&game, &genome)
        };
        for agent in fleet.iter_mut() {
            agent.settler_price = settler_price;
            agent.preempt_margin = preempt_margin;
            if let Some(arm) = arm {
                arm.configure(agent);
            }
        }
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor && !game.players[*pid].is_barbarian)
            .collect();
        let mut funnels: Vec<Funnel> = majors.iter().map(|_| Funnel::default()).collect();
        let mut prev_settlers: Vec<usize> = majors.iter().map(|_| 0).collect();

        for _ in 0..turns {
            if game.winner.is_some() {
                break;
            }
            for pid in 0..game.players.len() {
                if game.winner.is_some() {
                    break;
                }
                fleet[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    let _ = game.apply(pid, &Action::EndTurn);
                }
            }

            for (slot, pid) in majors.iter().enumerate() {
                let funnel = &mut funnels[slot];
                let city_ids = game.player_city_ids(*pid);
                let cities = city_ids.len();
                let settlers = game
                    .player_unit_ids(*pid)
                    .into_iter()
                    .filter(|uid| {
                        game.units.get(uid).map(|u| u.kind == "settler").unwrap_or(false)
                    })
                    .count();

                // A settler appearing is the outcome; count it and move on.
                if settlers > prev_settlers[slot] {
                    funnel.started += 1;
                }
                prev_settlers[slot] = settlers;

                // `desired_cities`, reproduced from `assess()` exactly.
                let land = game
                    .map
                    .tiles
                    .values()
                    .filter(|t| game.rules.is_passable(t) && !game.rules.is_water(t))
                    .count();
                let map_capacity = (2 + land / 55).clamp(3, 9);
                let cadence = game.standard_duration(90).max(1) as usize;
                let desired = (3 + game.turn as usize / cadence).min(map_capacity).min(6);
                funnel.cities_final = cities;
                funnel.target_final = desired;

                if cities + settlers >= desired {
                    funnel.at_target += 1;
                } else if settlers >= 1 {
                    funnel.settler_in_flight += 1;
                } else if !city_ids
                    .iter()
                    .any(|cid| game.cities.get(cid).map(|c| c.pop >= 2).unwrap_or(false))
                {
                    funnel.no_city_big_enough += 1;
                } else if game.turn >= game.standard_duration(175) {
                    funnel.window_closed += 1;
                } else if fleet[*pid].any_settle_site(&game, *pid) {
                    let anyone_free = city_ids
                        .iter()
                        .any(|cid| game.cities.get(cid).is_some_and(|c| c.queue.is_empty()));
                    if anyone_free {
                        funnel.lost_on_value += 1;
                    } else {
                        funnel.all_cities_busy += 1;
                    }
                } else {
                    funnel.no_site += 1;
                }
            }
        }
        let stock_deadline = game.standard_duration(300).min(
            game.max_turns
                .saturating_sub(game.standard_duration(50)),
        );
        let late_deadline = game
            .max_turns
            .saturating_sub(game.standard_duration(50));
        for (slot, pid) in majors.iter().enumerate() {
            let census = fleet[*pid].expansion_census();
            let funnel = &mut funnels[slot];
            funnel.dispatch_calls = census.dispatch_calls as usize;
            funnel.dispatch_productions = census.dispatch_productions as usize;
            funnel.dispatch_settler_turns = census.dispatch_settler_turns;
            funnel.advanced_late_settler_turns = census.advanced_late_settler_turns;
            funnel.founded_cities_final = game
                .cities
                .values()
                .filter(|city| city.owner == *pid && city.original_owner == *pid)
                .count();
            funnel.stock_deadline = stock_deadline;
            funnel.late_deadline = late_deadline;
        }
        MapResult {
            seed,
            actual_width,
            actual_height,
            seats: funnels,
        }
    });

    let seats: Vec<&Funnel> = per_map
        .iter()
        .flat_map(|result| result.seats.iter())
        .collect();
    if seats.is_empty() {
        println!("no seat sampled");
        return;
    }
    let n = seats.len() as f64;
    let sum = |f: fn(&Funnel) -> usize| seats.iter().map(|seat| f(seat)).sum::<usize>() as f64;

    let at_target = sum(|f| f.at_target);
    let in_flight = sum(|f| f.settler_in_flight);
    let small = sum(|f| f.no_city_big_enough);
    let closed = sum(|f| f.window_closed);
    let busy = sum(|f| f.all_cities_busy);
    let lost = sum(|f| f.lost_on_value);
    let nosite = sum(|f| f.no_site);
    let residual = lost + nosite + busy;
    let total = at_target + in_flight + small + closed + residual;

    println!("seats sampled            {}", seats.len());
    println!("cities at end            {:.2}", sum(|f| f.cities_final) / n);
    println!("planned target at end    {:.2}", sum(|f| f.target_final) / n);
    println!("settlers started         {:.2} per seat", sum(|f| f.started) / n);
    println!("\nwhere every seat-turn went (attribution is first-failing gate):");
    let row = |label: &str, value: f64| {
        println!("  {label:<26} {:6.1}%  ({:.0} seat-turns)", value * 100.0 / total, value)
    };
    row("already at target", at_target);
    row("settler already walking", in_flight);
    row("no city at pop 2", small);
    row("expansion window closed", closed);
    row("no reachable site", nosite);
    row("every city mid-build", busy);
    row("LOST ON VALUE (free city)", lost);

    if let Some(arm) = arm {
        print_mechanism_census(arm, &per_map);
    }

    // The residual is the only bucket that names a defect rather than a rule
    // working as written, so branch on it.
    println!();
    let short = residual * 100.0 / total;
    let lost_share = lost * 100.0 / total;
    let nosite_share = nosite * 100.0 / total;
    if residual < 0.05 * total {
        println!(
            "READING: expansion is NOT execution-limited in the way PR #366 suspected. Only \
             {short:.1}% of seat-turns clear every reproducible gate without producing a \
             settler; the empire is short of cities because the gates themselves — the \
             target, the one-at-a-time rule, pop 2, and the window — account for nearly all \
             of it. Those are design choices, and changing expansion means changing one of \
             them, not finding a bug."
        );
    } else {
        if lost > nosite {
            println!(
                "READING: {short:.1}% of seat-turns want a city and start no settler, and \
                 {lost_share:.1} of those points are the settler being OUT-COMPETED — a site \
                 existed and 920 + site*4 lost the production argument anyway. Only \
                 {nosite_share:.1} points are the map running out of room. Expansion is \
                 limited by what the settler is WORTH against the rest of the queue, which \
                 is a valuation defect and a tunable one."
            );
        } else {
            println!(
                "READING: {short:.1}% of seat-turns want a city and start no settler, and \
                 {nosite_share:.1} of those points are NO REACHABLE SITE against \
                 {lost_share:.1} lost on value. Expansion is limited by the map and by what \
                 `best_settle_site` will accept, not by the settler's price. Raising the \
                 settler's value cannot help; the site filter or the settle radius is where \
                 to look."
            );
        }
    }
}
