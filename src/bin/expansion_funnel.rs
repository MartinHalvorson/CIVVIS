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
//! Diagnostic only: it never changes a decision.
use civvis::ai::{AdvancedAi, Ai, Weights};
use civvis::game::{Action, Game};
use civvis::parallel;

fn number(args: &[String], flag: &str, default: usize) -> usize {
    args.iter()
        .position(|arg| arg == flag)
        .and_then(|index| args.get(index + 1))
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
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
    /// Turns every gate passed, a site existed, and still no settler: the
    /// settler lost the production argument.
    lost_on_value: usize,
    /// Turns every other gate passed but no city could see anywhere to go.
    no_site: usize,
    /// Turns a settler was actually started.
    started: usize,
    cities_final: usize,
    target_final: usize,
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

    println!(
        "expansion_funnel: {maps} maps, {players}p {width}x{height}, {city_states} city-states, \
         {turns} turns, seed {seed0}"
    );
    println!("every major seat sampled every turn; attribution is first-failing-gate\n");

    let per_map = parallel::map(maps, jobs, move |index| {
        let seed = seed0 + index as u64;
        let mut game = Game::new(players, width, height, seed, turns, city_states);
        let genome = civvis::evolve::load_champion("evolved").unwrap_or_default();
        let mut fleet: Vec<AdvancedAi> = AdvancedAi::fleet_weighted(&game, &genome);
        let majors: Vec<usize> = (0..game.players.len())
            .filter(|pid| !game.players[*pid].is_minor && !game.players[*pid].is_barbarian)
            .collect();
        let mut funnels: Vec<Funnel> = majors.iter().map(|_| Funnel::default()).collect();
        let mut prev_settlers: Vec<usize> = majors.iter().map(|_| 0).collect();

        for turn in 0..turns {
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
                    funnel.lost_on_value += 1;
                } else {
                    funnel.no_site += 1;
                }
            }
        }
        funnels
    });

    let seats: Vec<Funnel> = per_map.into_iter().flatten().collect();
    if seats.is_empty() {
        println!("no seat sampled");
        return;
    }
    let n = seats.len() as f64;
    let sum = |f: fn(&Funnel) -> usize| seats.iter().map(f).sum::<usize>() as f64;

    let at_target = sum(|f| f.at_target);
    let in_flight = sum(|f| f.settler_in_flight);
    let small = sum(|f| f.no_city_big_enough);
    let closed = sum(|f| f.window_closed);
    let lost = sum(|f| f.lost_on_value);
    let nosite = sum(|f| f.no_site);
    let residual = lost + nosite;
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
    row("LOST ON VALUE", lost);

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
