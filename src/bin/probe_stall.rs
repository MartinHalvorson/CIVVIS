//! TEMPORARY verification probe (adversarial review of the
//! "pillaged district freezes its own project forever" claim).
//! Delete after use.
//!
//! Measures, per arm, on the deployment profile:
//!   * city-turns matching `Game::advance_city`'s `district_project_stalled`
//!     predicate exactly (queue head is an `Item::Project` whose district
//!     family has no unpillaged member)
//!   * Production discarded on those city-turns (the whole cost of the claim)
//!   * distinct cities that ever stall, and how many are still stalled at the end
//!   * owned district tiles ever pillaged, and pillaged at the end

use civvis::game::{Action, City, Game, GameOptions, Item};
use civvis::name::Name;
use std::collections::{BTreeMap, BTreeSet};

fn arm(name: &str) -> Box<dyn civvis::ai::Ai> {
    use civvis::ai::AdvancedAi;
    match name {
        "advanced" => Box::new(AdvancedAi::new()),
        "every_lane" => {
            let mut ai = AdvancedAi::new();
            ai.enable_governor_every_lane();
            Box::new(ai)
        }
        other => panic!("unknown probe arm {other}"),
    }
}

fn family(game: &Game, district: Name) -> Name {
    let mut current = district;
    for _ in 0..game.rules.districts.len() {
        let Some(parent) = game
            .rules
            .districts
            .get_interned(current)
            .and_then(|spec| spec.replaces)
        else {
            break;
        };
        current = parent;
    }
    current
}

fn has_active_family(game: &Game, city: &City, want: Name) -> bool {
    if want.as_str() == "city_center" {
        return true;
    }
    let want_family = family(game, want);
    city.districts.iter().any(|(district, pos)| {
        family(game, *district) == want_family
            && !game.map.tiles[pos].pillaged
            && !(family(game, *district).as_str() == "encampment" && city.encampment_pillaged)
    })
}

/// Mirror of `Game::advance_city`'s `district_project_stalled`.
fn stalled(game: &Game, city: &City) -> bool {
    let Some(Item::Project { project }) = city.queue.first() else {
        return false;
    };
    let Some(spec) = game.rules.projects.get(project.as_str()) else {
        return false;
    };
    let Some(district) = spec.district else {
        return false; // no district requirement: never stalls
    };
    !std::iter::once(district)
        .chain(spec.alternate_districts.iter().copied())
        .any(|f| has_active_family(game, city, f))
}

#[derive(Default, Clone)]
struct Tally {
    seats: f64,
    city_turns: f64,
    stall_city_turns: f64,
    discarded_production: f64,
    total_production_city_turns: f64,
    cities_ever_stalled: f64,
    cities_stalled_at_end: f64,
    district_tiles_ever_pillaged: f64,
    district_tiles_pillaged_at_end: f64,
    by_project: BTreeMap<String, f64>,
    score: f64,
    cities: f64,
    buildings: f64,
    districts: f64,
}

fn num(args: &[String], flag: &str, default: i64) -> i64 {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let a = args
        .first()
        .cloned()
        .unwrap_or_else(|| "every_lane".to_string());
    let b = args
        .get(1)
        .cloned()
        .unwrap_or_else(|| "advanced".to_string());
    let pairs = num(&args, "--pairs", 4).max(1) as usize;
    let jobs = num(&args, "--jobs", 4).max(1) as usize;
    let seed = num(&args, "--seed", 27_000_000) as u64;
    let players = num(&args, "--players", 6).max(2) as usize;
    let width = num(&args, "--width", 74) as i32;
    let height = num(&args, "--height", 46) as i32;
    let turns = num(&args, "--turns", 250) as u32;
    let city_states = num(&args, "--city-states", 9).max(0) as usize;
    let speed = args
        .iter()
        .position(|x| x == "--speed")
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| "online".to_string());

    println!(
        "stall probe: {a} vs {b}, {pairs} pairs, {players}p {width}x{height} {speed} {turns}t cs{city_states}, seed {seed}"
    );

    let results = civvis::parallel::map(pairs * 2, jobs, |index| {
        let local_pair = index / 2;
        let swap = index % 2;
        let game_seed = seed + local_pair as u64;
        let seats: Vec<String> = (0..players)
            .map(|pid| {
                if (pid + swap) % 2 == 0 {
                    a.clone()
                } else {
                    b.clone()
                }
            })
            .collect();
        let challenger_seats = seats
            .iter()
            .enumerate()
            .filter(|(_, name)| **name == a)
            .map(|(pid, _)| pid)
            .collect();
        let mut game = Game::new_with(GameOptions {
            human_seats: challenger_seats,
            speed: speed.clone(),
            ..GameOptions::new(players, width, height, game_seed, turns, city_states)
        });
        game.victory_conditions = civvis::game::VictoryConditions::parse(
            "science,culture,religious,diplomatic,domination,score",
        )
        .expect("victory list");
        let mut ais: Vec<Box<dyn civvis::ai::Ai>> = game
            .players
            .iter()
            .map(|p| {
                if p.id < players {
                    arm(&seats[p.id])
                } else {
                    civvis::elo::builtin_ai("basic", game_seed + p.id as u64)
                }
            })
            .collect();

        let mut city_turns = vec![0.0f64; players];
        let mut stall_turns = vec![0.0f64; players];
        let mut discarded = vec![0.0f64; players];
        let mut total_prod = vec![0.0f64; players];
        let mut ever_stalled: Vec<BTreeSet<u32>> = (0..players).map(|_| BTreeSet::new()).collect();
        let mut ever_pillaged: Vec<BTreeSet<(i32, i32)>> =
            (0..players).map(|_| BTreeSet::new()).collect();
        let mut by_project: Vec<BTreeMap<String, f64>> =
            (0..players).map(|_| BTreeMap::new()).collect();

        while game.winner.is_none() && game.turn <= game.max_turns {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if pid < players {
                let ids: Vec<u32> = game
                    .cities
                    .values()
                    .filter(|c| c.owner == pid)
                    .map(|c| c.id)
                    .collect();
                for cid in ids {
                    city_turns[pid] += 1.0;
                    let produced = game.city_yields(cid).production;
                    total_prod[pid] += produced;
                    let city = &game.cities[&cid];
                    if stalled(&game, city) {
                        stall_turns[pid] += 1.0;
                        discarded[pid] += produced;
                        ever_stalled[pid].insert(cid);
                        if let Some(Item::Project { project }) = city.queue.first() {
                            *by_project[pid].entry(project.to_string()).or_insert(0.0) += 1.0;
                        }
                    }
                    for (_, pos) in game.cities[&cid].districts.iter() {
                        if game.map.tiles[pos].pillaged {
                            ever_pillaged[pid].insert((pos.0, pos.1));
                        }
                    }
                }
            }
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }

        let mut rows: Vec<(String, Tally)> = Vec::new();
        for pid in 0..players {
            let cities: Vec<&City> = game.cities.values().filter(|c| c.owner == pid).collect();
            let t = Tally {
                seats: 1.0,
                city_turns: city_turns[pid],
                stall_city_turns: stall_turns[pid],
                discarded_production: discarded[pid],
                total_production_city_turns: total_prod[pid],
                cities_ever_stalled: ever_stalled[pid].len() as f64,
                cities_stalled_at_end: cities
                    .iter()
                    .filter(|c| stalled(&game, c))
                    .count() as f64,
                district_tiles_ever_pillaged: ever_pillaged[pid].len() as f64,
                district_tiles_pillaged_at_end: cities
                    .iter()
                    .flat_map(|c| c.districts.iter())
                    .filter(|(_, pos)| game.map.tiles[pos].pillaged)
                    .count() as f64,
                by_project: by_project[pid].clone(),
                score: game.score(pid) as f64,
                cities: cities.len() as f64,
                buildings: cities.iter().map(|c| c.buildings.len() as f64).sum(),
                districts: cities.iter().map(|c| c.districts.len() as f64).sum(),
            };
            rows.push((seats[pid].clone(), t));
        }
        rows
    });

    let mut totals: BTreeMap<String, Tally> = BTreeMap::new();
    for rows in results {
        for (name, t) in rows {
            let e = totals.entry(name).or_default();
            e.seats += t.seats;
            e.city_turns += t.city_turns;
            e.stall_city_turns += t.stall_city_turns;
            e.discarded_production += t.discarded_production;
            e.total_production_city_turns += t.total_production_city_turns;
            e.cities_ever_stalled += t.cities_ever_stalled;
            e.cities_stalled_at_end += t.cities_stalled_at_end;
            e.district_tiles_ever_pillaged += t.district_tiles_ever_pillaged;
            e.district_tiles_pillaged_at_end += t.district_tiles_pillaged_at_end;
            for (k, v) in t.by_project {
                *e.by_project.entry(k).or_insert(0.0) += v;
            }
            e.score += t.score;
            e.cities += t.cities;
            e.buildings += t.buildings;
            e.districts += t.districts;
        }
    }
    for (name, t) in &totals {
        let n = t.seats.max(1.0);
        println!(
            "{name}: seats {:.0} score {:.1} cities {:.2} buildings {:.1} districts {:.1}",
            t.seats,
            t.score / n,
            t.cities / n,
            t.buildings / n,
            t.districts / n
        );
        println!(
            "    city-turns {:.0} | STALLED city-turns {:.0} ({:.4}% of city-turns)",
            t.city_turns,
            t.stall_city_turns,
            100.0 * t.stall_city_turns / t.city_turns.max(1.0)
        );
        println!(
            "    production discarded by the stall {:.0} of {:.0} total ({:.4}%)",
            t.discarded_production,
            t.total_production_city_turns,
            100.0 * t.discarded_production / t.total_production_city_turns.max(1.0)
        );
        println!(
            "    cities that ever stalled {:.2}/seat | still stalled at end {:.2}/seat",
            t.cities_ever_stalled / n,
            t.cities_stalled_at_end / n
        );
        println!(
            "    owned district tiles ever pillaged {:.2}/seat | pillaged at end {:.2}/seat",
            t.district_tiles_ever_pillaged / n,
            t.district_tiles_pillaged_at_end / n
        );
        println!("    stalled city-turns by project: {:?}", t.by_project);
    }
}
