//! TEMPORARY verification probe (adversarial review of the
//! "trade_route_capacity is never priced -> Market/Lighthouse never built"
//! claim). Delete after use.
//!
//! Reports per arm, at the terminal state and over the whole game:
//!   * Markets / Lighthouses / Banks / Shipyards actually standing
//!   * Commercial Hub / Harbor districts standing
//!   * trade capacity and active routes
//!   * city-turns whose queue head is a Market or a Lighthouse (i.e. PRODUCED)
//!
//! A market that stands without ever having been the queue head was bought
//! with gold by `BasicAi::buy_gold_infrastructure`.

use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions, Item};
use std::collections::BTreeMap;

fn arm(name: &str) -> Box<dyn Ai> {
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

fn is_market(b: &str) -> bool {
    matches!(b, "market" | "sukiennice")
}
fn is_lighthouse(b: &str) -> bool {
    b == "lighthouse"
}
fn is_bank(b: &str) -> bool {
    matches!(b, "bank" | "grand_bazaar" | "gilded_vault")
}
fn is_shipyard(b: &str) -> bool {
    b == "shipyard"
}

#[derive(Default, Clone)]
struct Tally {
    seats: f64,
    cities: f64,
    buildings: f64,
    markets: f64,
    lighthouses: f64,
    banks: f64,
    shipyards: f64,
    libraries: f64,
    monuments: f64,
    granaries: f64,
    hubs: f64,
    harbors: f64,
    capacity: f64,
    routes: f64,
    gold: f64,
    gpt: f64,
    score: f64,
    pop: f64,
    districts: f64,
    techs: f64,
    city_turns: f64,
    market_head_turns: f64,
    lighthouse_head_turns: f64,
    building_head_turns: f64,
    project_head_turns: f64,
    unit_head_turns: f64,
    district_head_turns: f64,
    empty_head_turns: f64,
    wins: f64,
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
    let b = args.get(1).cloned().unwrap_or_else(|| "advanced".to_string());
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
        "probe_market: {a} vs {b}, {pairs} pairs, {players}p {width}x{height} {speed} {turns}t cs{city_states}, seed {seed}"
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
        let mut ais: Vec<Box<dyn Ai>> = game
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

        let mut acc: Vec<Tally> = (0..players).map(|_| Tally::default()).collect();

        while game.winner.is_none() && game.turn <= game.max_turns {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if pid < players {
                for city in game.cities.values().filter(|c| c.owner == pid) {
                    acc[pid].city_turns += 1.0;
                    match city.queue.first() {
                        Some(Item::Building { building }) => {
                            acc[pid].building_head_turns += 1.0;
                            if is_market(building.as_str()) {
                                acc[pid].market_head_turns += 1.0;
                            }
                            if is_lighthouse(building.as_str()) {
                                acc[pid].lighthouse_head_turns += 1.0;
                            }
                        }
                        Some(Item::Project { .. }) => acc[pid].project_head_turns += 1.0,
                        Some(Item::Unit { .. }) => acc[pid].unit_head_turns += 1.0,
                        Some(Item::District { .. }) => acc[pid].district_head_turns += 1.0,
                        Some(_) => {}
                        None => acc[pid].empty_head_turns += 1.0,
                    }
                }
            }
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }

        let mut rows: Vec<(String, Tally)> = Vec::new();
        for pid in 0..players {
            let mut t = acc[pid].clone();
            t.seats = 1.0;
            t.wins = if game.winner == Some(pid) { 1.0 } else { 0.0 };
            t.score = game.score(pid) as f64;
            let cities: Vec<_> = game.cities.values().filter(|c| c.owner == pid).collect();
            t.cities = cities.len() as f64;
            for c in &cities {
                t.buildings += c.buildings.len() as f64;
                t.pop += c.pop as f64;
                t.districts += c.districts.len() as f64;
                for building in &c.buildings {
                    let name = building.as_str();
                    if is_market(name) {
                        t.markets += 1.0;
                    }
                    if is_lighthouse(name) {
                        t.lighthouses += 1.0;
                    }
                    if is_bank(name) {
                        t.banks += 1.0;
                    }
                    if is_shipyard(name) {
                        t.shipyards += 1.0;
                    }
                    if name == "library" {
                        t.libraries += 1.0;
                    }
                    if name == "monument" {
                        t.monuments += 1.0;
                    }
                    if name == "granary" {
                        t.granaries += 1.0;
                    }
                }
                for district in c.districts.keys() {
                    let name = district.as_str();
                    if name.contains("commercial_hub") || name == "suguba" {
                        t.hubs += 1.0;
                    }
                    if name == "harbor" || name == "royal_navy_dockyard" || name == "cothon" {
                        t.harbors += 1.0;
                    }
                }
            }
            t.capacity = game.trade_capacity(pid) as f64;
            t.routes = game.active_routes(pid) as f64;
            t.gold = game.players[pid].gold;
            t.gpt = game.players[pid].gold_per_turn;
            t.techs = game.players[pid].techs.len() as f64;
            rows.push((seats[pid].clone(), t));
        }
        rows
    });

    let mut totals: BTreeMap<String, Tally> = BTreeMap::new();
    for rows in results {
        for (name, t) in rows {
            let e = totals.entry(name).or_default();
            e.seats += t.seats;
            e.cities += t.cities;
            e.buildings += t.buildings;
            e.markets += t.markets;
            e.lighthouses += t.lighthouses;
            e.banks += t.banks;
            e.shipyards += t.shipyards;
            e.libraries += t.libraries;
            e.monuments += t.monuments;
            e.granaries += t.granaries;
            e.hubs += t.hubs;
            e.harbors += t.harbors;
            e.capacity += t.capacity;
            e.routes += t.routes;
            e.gold += t.gold;
            e.gpt += t.gpt;
            e.score += t.score;
            e.pop += t.pop;
            e.districts += t.districts;
            e.techs += t.techs;
            e.city_turns += t.city_turns;
            e.market_head_turns += t.market_head_turns;
            e.lighthouse_head_turns += t.lighthouse_head_turns;
            e.building_head_turns += t.building_head_turns;
            e.project_head_turns += t.project_head_turns;
            e.unit_head_turns += t.unit_head_turns;
            e.district_head_turns += t.district_head_turns;
            e.empty_head_turns += t.empty_head_turns;
            e.wins += t.wins;
        }
    }
    for (name, t) in &totals {
        let n = t.seats.max(1.0);
        println!(
            "\n{name}: seats {:.0} win {:.1}% score {:.1} cities {:.2} buildings {:.2} pop {:.1} \
             districts {:.2} techs {:.1} gold {:.1} gpt {:.2}",
            t.seats,
            100.0 * t.wins / n,
            t.score / n,
            t.cities / n,
            t.buildings / n,
            t.pop / n,
            t.districts / n,
            t.techs / n,
            t.gold / n,
            t.gpt / n,
        );
        println!(
            "    STANDING per seat: markets {:.2} lighthouses {:.2} banks {:.2} shipyards {:.2} \
             | libraries {:.2} monuments {:.2} granaries {:.2}",
            t.markets / n,
            t.lighthouses / n,
            t.banks / n,
            t.shipyards / n,
            t.libraries / n,
            t.monuments / n,
            t.granaries / n,
        );
        println!(
            "    per city: markets {:.3} lighthouses {:.3} hubs {:.3} harbors {:.3} | capacity {:.2} routes {:.2}",
            t.markets / t.cities.max(1.0),
            t.lighthouses / t.cities.max(1.0),
            t.hubs / t.cities.max(1.0),
            t.harbors / t.cities.max(1.0),
            t.capacity / n,
            t.routes / n,
        );
        println!(
            "    city-turns {:.0}: market-head {:.0} ({:.2}%) lighthouse-head {:.0} ({:.2}%) \
             | building {:.1}% project {:.1}% unit {:.1}% district {:.1}% empty {:.1}%",
            t.city_turns,
            t.market_head_turns,
            100.0 * t.market_head_turns / t.city_turns.max(1.0),
            t.lighthouse_head_turns,
            100.0 * t.lighthouse_head_turns / t.city_turns.max(1.0),
            100.0 * t.building_head_turns / t.city_turns.max(1.0),
            100.0 * t.project_head_turns / t.city_turns.max(1.0),
            100.0 * t.unit_head_turns / t.city_turns.max(1.0),
            100.0 * t.district_head_turns / t.city_turns.max(1.0),
            100.0 * t.empty_head_turns / t.city_turns.max(1.0),
        );
    }
}
