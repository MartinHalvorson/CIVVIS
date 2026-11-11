use super::*;

/// One major: our capital and a second city, every unit removed, turn 30, and
/// 50 Production banked on each city's Monument so it finishes inside the cap.
/// Returns the game, the two cities and the Monument item.
fn two_cities() -> (Game, Vec<u32>, Item) {
    let mut game = Game::new_full(1, 32, 20, 936_431, 200, 0, false);
    // Rome's Trajan's Column founds every city with a Monument.
    game.players[0].civ = "Gran Colombia".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=8).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(0, *sites.first().expect("fixture: a second site"), None);
    let cities = game.player_city_ids(0);
    assert_eq!(cities.len(), 2, "fixture: two cities");
    game.turn = 30;
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for cid in &cities {
        let city = game.cities.get_mut(cid).unwrap();
        city.queue.clear();
        city.production_progress
            .insert(Game::item_progress_key(&monument), 50.0);
        assert!(
            game.can_produce(0, *cid, &monument),
            "fixture: every city can build its Monument"
        );
    }
    (game, cities, monument)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_culture_seat_monuments();
    }
    ai
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(name),
    }
}

fn claim(ai: &AdvancedAi, game: &mut Game) {
    let plan = ai.assess(game, 0);
    ai.claim_culture_monuments(game, 0, &plan);
}

/// Every city of a Culture seat training a Scout puts its Monument first and
/// the claim holds; off, or for a Domination seat, nothing moves; a Settler
/// keeps its queue.
#[test]
fn a_culture_seat_claims_every_citys_monument() {
    let (game, cities, monument) = two_cities();
    let mut busy = game.clone();
    for cid in &cities {
        busy.cities.get_mut(cid).unwrap().queue = vec![unit("scout")];
    }

    for (ai, why) in [
        (seat(VictoryTarget::Culture, false), "off"),
        (seat(VictoryTarget::Domination, true), "a Domination seat"),
    ] {
        let mut kept = busy.clone();
        claim(&ai, &mut kept);
        for cid in &cities {
            assert_eq!(
                kept.cities[cid].queue.first(),
                Some(&unit("scout")),
                "{why}: no claim"
            );
        }
    }

    let ai = seat(VictoryTarget::Culture, true);
    let mut on = busy.clone();
    claim(&ai, &mut on);
    let plan = ai.assess(&on, 0);
    for cid in &cities {
        assert_eq!(
            on.cities[cid].queue.first(),
            Some(&monument),
            "the Monument goes first"
        );
        assert!(ai.culture_monument_holds(&on, 0, *cid, &plan, &monument));
    }

    let mut pump = busy.clone();
    pump.cities.get_mut(&cities[0]).unwrap().queue = vec![unit("settler")];
    claim(&ai, &mut pump);
    assert_eq!(
        pump.cities[&cities[0]].queue.first(),
        Some(&unit("settler")),
        "a Settler keeps its queue"
    );
}

/// A city with its Monument built claims nothing more.
#[test]
fn a_built_monument_is_not_claimed_again() {
    let (mut game, cities, monument) = two_cities();
    let Item::Building { building } = monument else {
        unreachable!()
    };
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(building);
    let ai = seat(VictoryTarget::Culture, true);
    let plan = ai.assess(&game, 0);
    assert!(ai
        .culture_monument_item(&game, 0, cities[0], &plan)
        .is_none());
    assert!(ai
        .culture_monument_item(&game, 0, cities[1], &plan)
        .is_some());
}

fn seat_v2() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_culture_seat_monuments_2();
    ai
}

#[test]
fn culture_seat_monuments_2_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers("culture-seat-monuments-2", |ai| {
        ai.culture_seat_monuments_2
    });
}

/// Two majors at war: our capital, a second city at least eight tiles away
/// with a Scout at the head of both queues and 50 Production banked on each
/// Monument, and the rival's Warrior beside the second city, out of the
/// capital's six tiles. Returns the game, `[capital, second]` and the
/// Monument item.
fn at_war_with_a_soldier_beside_one_city() -> (Game, Vec<u32>, Item) {
    let mut game = Game::new_full(2, 40, 24, 7_953, 300, 0, false);
    game.players[0].civ = "Gran Colombia".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for pid in 0..2 {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let land = |game: &Game, pos: &Pos| !game.rules.is_water(&game.map.tiles[pos]);
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .filter(|pos| (8..=10).contains(&game.wdist(**pos, home)) && land(&game, pos))
        .copied()
        .collect();
    sites.sort();
    let second = *sites.first().expect("fixture: a second site");
    game.place_city(0, second, None);
    let cities = game.player_city_ids(0);
    let second_city = *cities.iter().find(|cid| **cid != capital).unwrap();
    let mut posts: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .filter(|pos| {
            game.wdist(**pos, second) == 1
                && game.wdist(**pos, home) > MONUMENT_THREAT_RADIUS
                && land(&game, pos)
        })
        .copied()
        .collect();
    posts.sort();
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    game.spawn_unit("warrior", 1, *posts.first().expect("fixture: a post"));
    game.turn = 30;
    game.current = 0;
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for cid in &cities {
        let city = game.cities.get_mut(cid).unwrap();
        city.queue = vec![unit("scout")];
        city.production_progress
            .insert(Game::item_progress_key(&monument), 50.0);
    }
    (game, vec![capital, second_city], monument)
}

/// At war with a major, version one claims nothing anywhere; version two
/// claims the capital's Monument and holds the second city, which has the
/// rival's soldier beside it.
#[test]
fn version_two_claims_through_the_war_outside_the_soldiers_reach() {
    let (game, cities, monument) = at_war_with_a_soldier_beside_one_city();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    };

    let mut v1 = game.clone();
    seat(VictoryTarget::Culture, true).claim_culture_monuments(&mut v1, 0, &plan);
    for cid in &cities {
        assert_eq!(v1.cities[cid].queue.first(), Some(&unit("scout")), "v1 holds at war");
    }

    let ai = seat_v2();
    let mut v2 = game.clone();
    ai.claim_culture_monuments(&mut v2, 0, &plan);
    assert_eq!(
        v2.cities[&cities[0]].queue.first(),
        Some(&monument),
        "the capital, out of the soldier's reach, claims its Monument"
    );
    assert!(ai.culture_monument_holds(&v2, 0, cities[0], &plan, &monument));
    assert_eq!(
        v2.cities[&cities[1]].queue.first(),
        Some(&unit("scout")),
        "the city with the soldier beside it keeps its queue"
    );
    assert!(ai
        .culture_monument_item(&v2, 0, cities[1], &plan)
        .is_none());
}

/// A near-done Monument goes before a Shrine that version one keeps; Walls
/// keep their place under either version.
#[test]
fn a_near_done_monument_goes_before_a_kept_head() {
    let (game, cities, monument) = two_cities();
    let shrine = Item::Building {
        building: crate::name!("shrine"),
    };
    let walls = Item::Building {
        building: crate::name!("walls"),
    };
    let mut busy = game.clone();
    busy.cities.get_mut(&cities[0]).unwrap().queue = vec![shrine.clone()];
    busy.cities.get_mut(&cities[1]).unwrap().queue = vec![walls.clone()];

    let mut v1 = busy.clone();
    claim(&seat(VictoryTarget::Culture, true), &mut v1);
    assert_eq!(v1.cities[&cities[0]].queue.first(), Some(&shrine), "v1 keeps the Shrine");

    let ai = seat_v2();
    let plan = ai.assess(&busy, 0);
    assert!(ai.culture_monument_finishes_first(&busy, 0, cities[0], &plan, &monument));
    assert!(!seat(VictoryTarget::Culture, true)
        .culture_monument_finishes_first(&busy, 0, cities[0], &plan, &monument));
    let mut v2 = busy.clone();
    claim(&ai, &mut v2);
    assert_eq!(
        v2.cities[&cities[0]].queue.first(),
        Some(&monument),
        "the near-done Monument goes first"
    );
    assert_eq!(
        v2.cities[&cities[1]].queue.first(),
        Some(&walls),
        "Walls keep their place"
    );
}

/// A Monument far from done is no near-done Monument: version two keeps
/// version one's holds for it.
#[test]
fn a_young_monument_keeps_version_ones_holds() {
    let (mut game, cities, monument) = two_cities();
    let ai = seat_v2();
    let key = Game::item_progress_key(&monument);
    let young = (1..30).rev().find(|banked| {
        game.cities
            .get_mut(&cities[0])
            .unwrap()
            .production_progress
            .insert(key.clone(), *banked as f64);
        let plan = ai.assess(&game, 0);
        ai.culture_monument_item(&game, 0, cities[0], &plan).is_some()
            && !ai.culture_monument_finishes_first(&game, 0, cities[0], &plan, &monument)
    });
    assert!(young.is_some(), "fixture: a claimable Monument far from done");
    let shrine = Item::Building {
        building: crate::name!("shrine"),
    };
    game.cities.get_mut(&cities[0]).unwrap().queue = vec![shrine.clone()];
    claim(&ai, &mut game);
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&shrine));
}
