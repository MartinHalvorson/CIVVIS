//! Black swans (`game/black_swan.rs`): what each of the five does to the
//! world it strikes, what of two of them goes on turn after turn, that a
//! seed strikes two copies of a world alike, and that a world nobody struck
//! one on is the world it was.
use super::black_swan::{
    COLLAPSE_ANARCHY, COLLAPSE_LOYALTY, LEAK_SITES, LEAK_TURNS, MICROSTATE_ARMED_TURNS,
    MICROSTATE_DEFENDERS, WAR_VOLLEYS,
};
use super::*;

/// A world of `players` civilizations with a capital each and no random
/// disaster of any class, so a test sees only the swan it struck.
fn world(players: usize, seed: u64) -> Game {
    let mut game = Game::new_full(players, 60, 40, seed, 500, 0, false);
    game.disaster_intensity = 0;
    for pid in 0..players {
        let position = game
            .player_unit_ids(pid)
            .into_iter()
            .find_map(|uid| {
                let unit = &game.units[&uid];
                (unit.kind == "settler").then_some(unit.pos)
            })
            .unwrap();
        game.found_city_for(pid, position, None);
    }
    game
}

/// One more city for `pid`: on the open land nearest its capital that is at
/// least five tiles from every city there is.
fn settle(game: &mut Game, pid: usize) -> u32 {
    let capital = game.cities[&game.player_city_ids(pid)[0]].pos;
    let taken: Vec<Pos> = game.cities.values().map(|city| city.pos).collect();
    let position = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && tile.owner_city.is_none()
                && taken.iter().all(|city| game.wdist(*city, **position) >= 5)
        })
        .map(|(position, _)| *position)
        .min_by_key(|position| (game.wdist(capital, *position), *position))
        .expect("the world has open land left");
    game.found_city_for(pid, position, None)
}

/// Play whole world turns, every seat ending its turn as it stands.
fn advance(game: &mut Game, turns: u32) {
    let target = game.turn + turns;
    while game.turn < target {
        let current = game.current;
        game.apply(current, &Action::EndTurn).unwrap();
    }
}

fn free_cities(game: &Game) -> usize {
    game.players
        .iter()
        .find(|player| player.is_free_city)
        .expect("every world seats the Free Cities")
        .id
}

#[test]
fn a_pandemic_takes_half_of_every_city_in_the_world() {
    let mut game = world(3, 7_001);
    let cities: Vec<u32> = (0..3).map(|pid| game.player_city_ids(pid)[0]).collect();
    for (cid, pop) in cities.iter().zip([7, 4, 1]) {
        game.cities.get_mut(cid).unwrap().pop = pop;
    }
    let dice = format!("{:?}", game.rng);

    let swan = game.black_swan(Some(BlackSwanKind::Pandemic), 11).unwrap();

    let after: Vec<i32> = cities.iter().map(|cid| game.cities[cid].pop).collect();
    assert_eq!(
        after,
        [4, 2, 1],
        "half of each, and a city of one keeps its one"
    );
    assert_eq!(swan.kind, BlackSwanKind::Pandemic);
    assert_eq!(swan.turn, game.turn);
    assert!(
        swan.text.contains("2 cities") && swan.text.contains("5 of the world's 12"),
        "the account carries its numbers: {}",
        swan.text
    );
    assert!(!swan.ongoing(game.turn), "a pandemic is over as it lands");
    assert_eq!(game.black_swans, vec![swan]);
    // What follows is not the game that was going to be played, and a world
    // that differs only by a swan is told from the one it was by its dice.
    assert_ne!(format!("{:?}", game.rng), dice);
    // Every civilization is told, in the swan's own words.
    for pid in 0..3 {
        let told = game
            .events_for(pid)
            .into_iter()
            .rev()
            .find(|event| event.important)
            .expect("a black swan is not news to scroll past");
        assert!(
            told.text.starts_with("Black swan: Pandemic."),
            "{}",
            told.text
        );
    }
}

#[test]
fn an_instant_nuclear_war_sets_every_civilization_on_every_other() {
    let mut game = world(3, 7_002);
    for pid in 0..3 {
        let capital = game.player_city_ids(pid)[0];
        game.cities.get_mut(&capital).unwrap().pop = 10;
        let second = settle(&mut game, pid);
        game.cities.get_mut(&second).unwrap().pop = 6;
    }
    // A friendship is no shelter: nobody declared this war.
    game.players[0].friends_until.insert(1, game.turn + 30);
    game.players[1].friends_until.insert(0, game.turn + 30);
    assert!(game.are_friends(0, 1) && !game.is_at_war(0, 1));

    let swan = game
        .black_swan(Some(BlackSwanKind::NuclearWar), 12)
        .unwrap();

    for first in 0..3 {
        for second in first + 1..3 {
            assert!(game.is_at_war(first, second), "{first} and {second}");
            assert!(!game.are_friends(first, second));
        }
    }
    // Each civilization fired on each of its two rivals, on the largest city
    // of each that nothing had landed on yet: six devices on six cities.
    let strikes = game.chronicle.nuclear_strikes.clone();
    assert_eq!(strikes.len(), 3 * WAR_VOLLEYS);
    assert_eq!(swan.sites.len(), 6, "{}", swan.text);
    for strike in strikes.iter() {
        assert_eq!(strike.turn, game.turn);
        assert_eq!(strike.platform, "black_swan");
        assert_eq!(strike.cities.len(), 1, "it landed on a city");
        assert!(game.fallout_at(strike.target));
        let struck = game.city_at(strike.target).unwrap();
        assert_ne!(game.cities[&struck].owner, strike.attacker);
    }
    let pops: Vec<i32> = (0..3)
        .flat_map(|pid| game.player_city_ids(pid))
        .map(|cid| game.cities[&cid].pop)
        .collect();
    assert_eq!(
        pops.iter().filter(|pop| **pop == 5).count(),
        3,
        "every capital of ten is a city of five: {pops:?}"
    );
    assert_eq!(pops.iter().filter(|pop| **pop == 3).count(), 3, "{pops:?}");
    assert!(
        swan.text.contains("All 3 civilizations") && swan.text.contains("6 nuclear devices"),
        "{}",
        swan.text
    );
}

#[test]
fn a_radiation_leak_spreads_a_ring_a_turn_and_then_stops() {
    let mut game = world(4, 7_003);
    let struck = game.turn;

    let swan = game
        .black_swan(Some(BlackSwanKind::RadiationLeak), 13)
        .unwrap();

    assert!(!swan.sites.is_empty() && swan.sites.len() <= LEAK_SITES);
    assert_eq!(swan.until, struck + LEAK_TURNS);
    assert!(swan.ongoing(game.turn));
    let site = swan.sites[0];
    assert!(game.city_at(site).is_some(), "a leak is from a city");
    // The ground a ring of the given radius holds, and whether all of it
    // that is on the map is contaminated.
    let ring = |game: &Game, radius: i32| -> Vec<Pos> {
        game.wdisk(site, radius)
            .into_iter()
            .filter(|position| game.wdist(site, *position) == radius)
            .collect()
    };
    let hot = |game: &Game, radius: i32| ring(game, radius).iter().all(|at| game.fallout_at(*at));
    let clean =
        |game: &Game, radius: i32| !ring(game, radius).iter().any(|at| game.fallout_at(*at));
    assert!(
        hot(&game, 0) && hot(&game, 1),
        "a tile around the site at once"
    );
    assert!(clean(&game, 2), "and no further yet");

    for turn in 1..=LEAK_TURNS {
        advance(&mut game, 1);
        let reached = 1 + turn as i32;
        assert!(hot(&game, reached), "ring {reached} on turn {turn}");
        // Unless another site's own spread has come this far.
        let alone = swan.sites[1..]
            .iter()
            .all(|other| game.wdist(site, *other) > 2 * reached + 3);
        if alone {
            assert!(clean(&game, reached + 1), "ring {} waits", reached + 1);
        }
    }
    let last = 1 + LEAK_TURNS as i32;
    let alone = swan.sites[1..]
        .iter()
        .all(|other| game.wdist(site, *other) > 2 * last + 3);
    advance(&mut game, 3);
    assert!(!game.black_swans[0].ongoing(game.turn), "the leak is spent");
    if alone {
        assert!(clean(&game, last + 1), "it spreads no further");
    }
    // Ground a leak holds pays nothing to whoever works it.
    let tile = game.map.get(site).unwrap();
    assert!(game.tile_pays_nothing(tile));
}

#[test]
fn a_leader_collapse_fells_the_civilization_in_the_lead() {
    let mut game = world(3, 7_004);
    // The second civilization is in the lead: three cities, and the people.
    for _ in 0..2 {
        let more = settle(&mut game, 1);
        game.cities.get_mut(&more).unwrap().pop = 8;
    }
    let leader = (0..3).max_by_key(|pid| game.score(*pid)).unwrap();
    assert_eq!(leader, 1);
    game.install_government(1, "chiefdom");
    let government = game.players[1].government.clone();
    assert_eq!(government.as_deref(), Some("chiefdom"));

    let swan = game
        .black_swan(Some(BlackSwanKind::LeaderCollapse), 14)
        .unwrap();

    assert_eq!(swan.player, Some(1));
    assert!(game.in_anarchy(1));
    assert_eq!(game.players[1].anarchy_turns, COLLAPSE_ANARCHY);
    assert_eq!(game.players[1].government, None);
    assert_eq!(game.players[1].pending_government, government);
    let cities = game.player_city_ids(1);
    assert_eq!(cities.len(), 3);
    for cid in &cities {
        let city = &game.cities[cid];
        let most = if city.is_capital {
            COLLAPSE_LOYALTY * 2.0
        } else {
            COLLAPSE_LOYALTY
        };
        assert!(city.loyalty <= most, "{} keeps {}", city.name, city.loyalty);
    }
    // Nobody else's leader fell.
    assert!(!game.in_anarchy(0) && !game.in_anarchy(2));
    assert_eq!(game.cities[&game.player_city_ids(0)[0]].loyalty, 100.0);
    assert!(swan.text.contains("3 cities"), "{}", swan.text);
    // The government that was seated comes back when the Anarchy is served.
    advance(&mut game, COLLAPSE_ANARCHY + 1);
    assert!(!game.in_anarchy(1));
    assert_eq!(game.players[1].government, government);
}

#[test]
fn microstates_break_a_civilization_into_armed_free_cities() {
    let mut game = world(2, 7_005);
    // Only the first civilization has cities to lose, so it is the one.
    let capital = game.player_city_ids(0)[0];
    let breakaway: Vec<u32> = (0..2).map(|_| settle(&mut game, 0)).collect();
    for cid in &breakaway {
        game.cities.get_mut(cid).unwrap().pop = 5;
    }
    game.cities.get_mut(&capital).unwrap().pop = 9;
    let free = free_cities(&game);

    let swan = game
        .black_swan(Some(BlackSwanKind::Microstates), 15)
        .unwrap();

    assert_eq!(swan.player, Some(0));
    assert_eq!(
        game.player_city_ids(0),
        vec![capital],
        "it keeps its capital"
    );
    for cid in &breakaway {
        assert_eq!(game.cities[cid].owner, free);
        assert_eq!(game.cities[cid].loyalty, 100.0);
        assert_eq!(swan.armed.get(cid), Some(&1), "one device each");
    }
    // Hostile: the Free Cities are at war with everybody, without a word.
    assert!(game.is_at_war(free, 0) && game.is_at_war(free, 1));
    // Powerful: each stood up its defenders.
    let army = game
        .player_unit_ids(free)
        .into_iter()
        .filter(|uid| game.rules.units[game.units[uid].kind].promotion_class == "melee")
        .count();
    assert_eq!(army, MICROSTATE_DEFENDERS * breakaway.len());
    assert_eq!(swan.until, game.turn + MICROSTATE_ARMED_TURNS);
    assert!(swan.ongoing(game.turn));

    // Nuclear armed: a microstate fires its device on the largest city of a
    // civilization in range, and then has none. The capital it broke away
    // from is the nearest there is.
    let before = game.cities[&capital].pop;
    let mut fired = None;
    for _ in 0..MICROSTATE_ARMED_TURNS {
        advance(&mut game, 1);
        if let Some(strike) = game.chronicle.nuclear_strikes.last() {
            fired = Some(strike.clone());
            break;
        }
    }
    let strike = fired.expect("an armed microstate fires within its forty turns");
    assert_eq!(strike.attacker, free);
    assert!(!strike.thermonuclear);
    assert_eq!(strike.target, game.cities[&capital].pos);
    assert!(game.cities[&capital].pop < before);
    let held: u32 = game.black_swans[0]
        .armed
        .values()
        .map(|devices| u32::from(*devices))
        .sum();
    assert_eq!(held, 1, "one of the two devices is spent");

    // A microstate that is taken has no arsenal: its device goes with it.
    let armed: u32 = *game.black_swans[0]
        .armed
        .iter()
        .find(|(_, devices)| **devices > 0)
        .unwrap()
        .0;
    game.transfer_city(armed, 1, false);
    advance(&mut game, 1);
    assert!(!game.black_swans[0].armed.contains_key(&armed));
    assert!(!game.black_swans[0].ongoing(game.turn));
    let strikes = game.chronicle.nuclear_strikes.len();
    advance(&mut game, 12);
    assert_eq!(game.chronicle.nuclear_strikes.len(), strikes);
}

#[test]
fn one_seed_strikes_two_copies_of_a_world_alike() {
    let mut game = world(4, 7_006);
    for pid in 0..4 {
        settle(&mut game, pid);
    }
    for seed in 0..12u64 {
        let (mut one, mut other) = (game.clone(), game.clone());
        let struck = one.black_swan(None, seed).unwrap();
        assert_eq!(other.black_swan(None, seed).unwrap(), struck);
        assert_eq!(
            serde_json::to_string(&one).unwrap(),
            serde_json::to_string(&other).unwrap(),
            "seed {seed}, {:?}",
            struck.kind
        );
    }
    // And the seed is what draws which swan it is: over a dozen seeds this
    // world sees more than one kind of them.
    let mut kinds: Vec<&str> = (0..12u64)
        .map(|seed| game.clone().black_swan(None, seed).unwrap().kind.id())
        .collect();
    kinds.sort_unstable();
    kinds.dedup();
    assert!(kinds.len() > 1, "{kinds:?}");
}

#[test]
fn a_swan_strikes_only_what_the_world_has() {
    // Capitals only: there is no second city to break away.
    let mut game = world(2, 7_007);
    let refused = game.black_swan(Some(BlackSwanKind::Microstates), 1);
    assert_eq!(
        refused,
        Err("no civilization has a second city to break away".to_string())
    );
    assert!(game.black_swans.is_empty(), "a refused swan struck nothing");
    // One drawn by the seed is never one this world has nothing for.
    for seed in 0..24u64 {
        let struck = game.clone().black_swan(None, seed).unwrap();
        assert_ne!(struck.kind, BlackSwanKind::Microstates, "seed {seed}");
    }
    // Every kind is asked for by the name it gives.
    for kind in BlackSwanKind::ALL {
        assert_eq!(BlackSwanKind::parse(kind.id()), Some(kind));
        assert!(!kind.name().is_empty() && !kind.about().is_empty());
    }
    assert_eq!(BlackSwanKind::parse("asteroid"), None);
}

#[test]
fn a_struck_swan_goes_on_from_a_save() {
    let mut game = world(4, 7_008);
    game.black_swan(Some(BlackSwanKind::RadiationLeak), 17)
        .unwrap();
    advance(&mut game, 1);

    let saved = serde_json::to_string(&game).unwrap();
    let mut loaded: Game = serde_json::from_str(&saved).unwrap();

    assert_eq!(loaded.black_swans, game.black_swans);
    assert!(loaded.black_swans[0].ongoing(loaded.turn));
    // The loaded world leaks on exactly as the one it was saved from does.
    advance(&mut game, 2);
    advance(&mut loaded, 2);
    let fallout = |game: &Game| -> Vec<(Pos, u32)> {
        game.map
            .tiles
            .iter()
            .filter(|(_, tile)| tile.fallout_until > 0)
            .map(|(position, tile)| (*position, tile.fallout_until))
            .collect()
    };
    assert!(!fallout(&game).is_empty());
    assert_eq!(fallout(&loaded), fallout(&game));
}

#[test]
fn a_world_no_swan_struck_is_the_world_it_was() {
    let mut game = world(2, 7_009);
    // Its save does not carry the field at all, so every save written before
    // there were black swans is still this game's save, byte for byte.
    let saved = serde_json::to_string(&game).unwrap();
    assert!(!saved.contains("black_swans"));
    // And the turn's pass over the swans neither writes the world nor draws
    // from its dice.
    game.process_black_swans();
    assert_eq!(serde_json::to_string(&game).unwrap(), saved);
    // A swan that is over is as quiet as none.
    let mut struck = game.clone();
    let capital = struck.player_city_ids(0)[0];
    struck.cities.get_mut(&capital).unwrap().pop = 4;
    struck.black_swan(Some(BlackSwanKind::Pandemic), 3).unwrap();
    let after = serde_json::to_string(&struck).unwrap();
    struck.turn += 1;
    struck.process_black_swans();
    struck.turn -= 1;
    assert_eq!(serde_json::to_string(&struck).unwrap(), after);
}
