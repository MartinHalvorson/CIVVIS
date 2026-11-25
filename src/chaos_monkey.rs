//! Chaos Monkey: play one world under a chosen frequency of natural
//! disasters and write down what it did to the civilizations in it.
//!
//! Chaos engineering breaks a running system on purpose to learn where it is
//! fragile before a real failure teaches the same thing at full price. Here
//! the system is a world of civilizations, the breakage is Gathering Storm's
//! own disasters turned up far past the lobby's top setting
//! (`Game::disaster_frequency`), and the lesson is read off the difference
//! between the same seed played calm and played under the storm.
//!
//! [`play`] plays one game to its end with the stock agents on every seat
//! and returns one JSON record: what struck (`chronicle.disasters`), what
//! each civilization lost to it, how each stood every [`SAMPLE_EVERY`] turns
//! (people, cities, score, technologies, army, Builders, improved tiles and
//! the share of them lying pillaged), the wars that started and the cities
//! that changed hands, and how each civilization was sheltered when the game
//! ended. `civvis chaos-monkey` plays a grid of seeds and frequencies and
//! prints one record a line; `tools/chaos_monkey.py` reads them into the
//! record the Chaos Monkey page draws.
//!
//! It plays the game and measures it; it changes no rule. The dial and the
//! census it reads are the engine's (`src/game/disasters.rs`).

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use crate::ai::{run_game_observed, AdvancedAi};
use crate::game::{Game, GameOptions};

/// A civilization's standing is written down every this many turns, and on
/// the first turn and the last.
pub const SAMPLE_EVERY: u32 = 10;

/// What the observer keeps for one major civilization while the game plays.
#[derive(Default)]
struct Watch {
    /// The most people it ever had, and the turn it had them.
    peak_pop: i32,
    peak_turn: u32,
    /// The fewest people it had on any turn after that peak.
    low_pop: i32,
    /// Cities that left its hands for someone else's.
    cities_lost: u32,
    /// Cities that came into its hands from someone else's.
    cities_taken: u32,
    /// Turns it spent at war with another major civilization.
    war_turns: u32,
    /// The samples, one entry per sampled turn.
    pop: Vec<i32>,
    cities: Vec<u32>,
    score: Vec<i64>,
    techs: Vec<usize>,
    army: Vec<u32>,
    builders: Vec<u32>,
    improved: Vec<u32>,
    pillaged: Vec<u32>,
    gold: Vec<i64>,
}

/// The majors: the seats that can win or lose the game.
fn majors(game: &Game) -> Vec<usize> {
    game.players
        .iter()
        .filter(|player| !player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect()
}

/// People and cities per owner, in one pass over the cities.
fn census(game: &Game) -> BTreeMap<usize, (i32, u32)> {
    let mut held: BTreeMap<usize, (i32, u32)> = BTreeMap::new();
    for city in game.cities.values() {
        let entry = held.entry(city.owner).or_default();
        entry.0 += city.pop;
        entry.1 += 1;
    }
    held
}

/// One sample of every major, appended to its watch.
fn sample(game: &Game, majors: &[usize], watches: &mut BTreeMap<usize, Watch>) {
    let held = census(game);
    let mut army: BTreeMap<usize, u32> = BTreeMap::new();
    let mut builders: BTreeMap<usize, u32> = BTreeMap::new();
    for unit in game.units.values() {
        if unit.kind.as_str() == "builder" {
            *builders.entry(unit.owner).or_default() += 1;
        } else if game.rules.units[unit.kind].class == "military" {
            *army.entry(unit.owner).or_default() += 1;
        }
    }
    let mut improved: BTreeMap<usize, u32> = BTreeMap::new();
    let mut pillaged: BTreeMap<usize, u32> = BTreeMap::new();
    for city in game.cities.values() {
        for position in &city.owned_tiles {
            let Some(tile) = game.map.get(*position) else {
                continue;
            };
            if tile.improvement.is_none() && tile.district.is_none() {
                continue;
            }
            *improved.entry(city.owner).or_default() += 1;
            if tile.pillaged {
                *pillaged.entry(city.owner).or_default() += 1;
            }
        }
    }
    for pid in majors {
        let watch = watches.entry(*pid).or_default();
        let (pop, cities) = held.get(pid).copied().unwrap_or_default();
        watch.pop.push(pop);
        watch.cities.push(cities);
        watch.score.push(game.score(*pid));
        watch.techs.push(game.players[*pid].techs.len());
        watch.army.push(army.get(pid).copied().unwrap_or(0));
        watch.builders.push(builders.get(pid).copied().unwrap_or(0));
        watch.improved.push(improved.get(pid).copied().unwrap_or(0));
        watch.pillaged.push(pillaged.get(pid).copied().unwrap_or(0));
        watch.gold.push(game.players[*pid].gold.round() as i64);
    }
}

/// Play one world at one disaster frequency and describe it.
///
/// `frequency` is how many times as often disasters strike as the options'
/// own intensity setting has them: one is the game as shipped.
pub fn play(options: GameOptions, frequency: f64) -> Value {
    let seed = options.seed;
    let mut game = Game::new_with(options);
    game.set_disaster_frequency(frequency);
    let mut ais = AdvancedAi::fleet(&game);
    let majors = majors(&game);
    let major_set: BTreeSet<usize> = majors.iter().copied().collect();

    let mut watches: BTreeMap<usize, Watch> = BTreeMap::new();
    let mut sample_turns: Vec<u32> = Vec::new();
    let mut owners: BTreeMap<u32, usize> = BTreeMap::new();
    let mut at_war: BTreeSet<(usize, usize)> = BTreeSet::new();
    let mut wars_started = 0u32;
    let mut storms_peak = 0usize;
    let mut droughts_peak = 0usize;

    run_game_observed(&mut game, &mut ais, |world| {
        // Every turn: the peak and the trough after it, who is at war, and
        // which cities changed hands since the turn before.
        let held = census(world);
        for pid in &majors {
            let watch = watches.entry(*pid).or_default();
            let pop = held.get(pid).map_or(0, |entry| entry.0);
            if pop > watch.peak_pop {
                watch.peak_pop = pop;
                watch.peak_turn = world.turn;
                watch.low_pop = pop;
            } else if pop < watch.low_pop {
                watch.low_pop = pop;
            }
        }
        let now: BTreeSet<(usize, usize)> = world
            .at_war
            .iter()
            .copied()
            .filter(|(a, b)| major_set.contains(a) && major_set.contains(b))
            .map(|(a, b)| (a.min(b), a.max(b)))
            .collect();
        wars_started += now.difference(&at_war).count() as u32;
        for (a, b) in &now {
            watches.entry(*a).or_default().war_turns += 1;
            watches.entry(*b).or_default().war_turns += 1;
        }
        at_war = now;
        for city in world.cities.values() {
            if let Some(before) = owners.insert(city.id, city.owner) {
                if before != city.owner {
                    watches.entry(before).or_default().cities_lost += 1;
                    watches.entry(city.owner).or_default().cities_taken += 1;
                }
            }
        }
        storms_peak = storms_peak.max(world.storms.len());
        droughts_peak = droughts_peak.max(world.droughts.len());
        if world.turn == 1 || world.turn % SAMPLE_EVERY == 0 {
            sample_turns.push(world.turn);
            sample(world, &majors, &mut watches);
        }
    });
    // The world as it ended, whatever turn that was.
    sample_turns.push(game.reported_turn());
    sample(&game, &majors, &mut watches);

    let civs: Vec<Value> = majors
        .iter()
        .map(|pid| {
            let player = &game.players[*pid];
            let watch = watches.remove(pid).unwrap_or_default();
            let losses = game
                .chronicle
                .disasters
                .losses
                .get(pid)
                .copied()
                .unwrap_or_default();
            let cities = game.player_city_ids(*pid);
            let mut shelter = [0u32; 4];
            let mut floodplains = 0u32;
            let mut by_volcano = 0u32;
            let mut on_coast = 0u32;
            for city_id in &cities {
                let stands = game.city_disaster_shelter(*city_id);
                shelter[0] += u32::from(stands.immune);
                shelter[1] += u32::from(stands.flood);
                shelter[2] += u32::from(stands.drought);
                shelter[3] += u32::from(stands.coast);
                let city = &game.cities[city_id];
                floodplains += city
                    .owned_tiles
                    .iter()
                    .filter_map(|position| game.map.get(*position))
                    .filter(|tile| {
                        matches!(
                            tile.feature.as_deref(),
                            Some("floodplains" | "grassland_floodplains" | "plains_floodplains")
                        )
                    })
                    .count() as u32;
                let ring = game.wdisk(city.pos, 2);
                by_volcano += u32::from(ring.iter().any(|position| {
                    game.map
                        .get(*position)
                        .is_some_and(|tile| game.rules.is_volcano(tile))
                }));
                on_coast += u32::from(game.wdisk(city.pos, 1).iter().any(|position| {
                    game.map
                        .get(*position)
                        .is_some_and(|tile| game.rules.is_water(tile))
                }));
            }
            json!({
                "civ": player.civ,
                "alive": player.alive,
                "won": game.winner == Some(*pid),
                "peak_pop": watch.peak_pop,
                "peak_turn": watch.peak_turn,
                "low_pop": watch.low_pop,
                "cities_lost": watch.cities_lost,
                "cities_taken": watch.cities_taken,
                "war_turns": watch.war_turns,
                "lost": {
                    "citizens": losses.citizens,
                    "tiles": losses.tiles,
                    "units": losses.units,
                },
                "shelter": {
                    "cities": cities.len(),
                    "immune": shelter[0],
                    "flood": shelter[1],
                    "drought": shelter[2],
                    "coast": shelter[3],
                    "floodplains": floodplains,
                    "by_volcano": by_volcano,
                    "on_coast": on_coast,
                },
                "series": {
                    "pop": watch.pop,
                    "cities": watch.cities,
                    "score": watch.score,
                    "techs": watch.techs,
                    "army": watch.army,
                    "builders": watch.builders,
                    "improved": watch.improved,
                    "pillaged": watch.pillaged,
                    "gold": watch.gold,
                },
            })
        })
        .collect();

    json!({
        "seed": seed,
        "frequency": game.disaster_frequency(),
        "intensity": game.disaster_intensity(),
        "players": majors.len(),
        "max_turns": game.max_turns,
        "ended": game.reported_turn(),
        "winner": game.winner.map(|pid| game.players[pid].civ.clone()),
        "victory": game.victory_label(),
        "draw": game.is_draw(),
        "disasters": game.chronicle.disasters.events,
        "storms_peak": storms_peak,
        "droughts_peak": droughts_peak,
        "climate_phase": game.climate_phase,
        "wars": wars_started,
        "nuclear_strikes": game.chronicle.nuclear_strikes.len(),
        "sample_turns": sample_turns,
        "civs": civs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn small(seed: u64) -> GameOptions {
        GameOptions::new(2, 30, 20, seed, 40, 0)
    }

    /// The whole point of the dial: the same seed, hit harder. A world at
    /// thirty times the frequency takes many times the disasters of the
    /// shipped game, and its record says so.
    #[test]
    fn more_frequency_is_more_disasters_on_the_same_seed() {
        let struck = |frequency: f64| -> u64 {
            let record = play(small(7), frequency);
            assert_eq!(record["frequency"].as_f64(), Some(frequency));
            assert_eq!(record["civs"].as_array().map(Vec::len), Some(2));
            record["disasters"]
                .as_object()
                .unwrap()
                .values()
                .filter_map(Value::as_u64)
                .sum()
        };
        let calm = struck(1.0);
        let chaos = struck(30.0);
        assert!(
            chaos >= 30 && chaos > calm * 5,
            "thirty times the frequency struck {chaos} times against {calm} calm"
        );
    }

    /// A record carries one sample per sampled turn for every series, so the
    /// reader can line civilizations and worlds up by index.
    #[test]
    fn every_series_is_as_long_as_the_sampled_turns() {
        let record = play(small(11), 10.0);
        let turns = record["sample_turns"].as_array().unwrap().len();
        assert!(
            turns >= 5,
            "a 40-turn game samples turn 1, every tenth and the end"
        );
        for civ in record["civs"].as_array().unwrap() {
            for (name, series) in civ["series"].as_object().unwrap() {
                assert_eq!(
                    series.as_array().unwrap().len(),
                    turns,
                    "series {name} of {}",
                    civ["civ"]
                );
            }
        }
    }
}
