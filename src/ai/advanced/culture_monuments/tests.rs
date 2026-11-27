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
        assert_eq!(
            v1.cities[cid].queue.first(),
            Some(&unit("scout")),
            "v1 holds at war"
        );
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
    assert!(ai.culture_monument_item(&v2, 0, cities[1], &plan).is_none());
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
    assert_eq!(
        v1.cities[&cities[0]].queue.first(),
        Some(&shrine),
        "v1 keeps the Shrine"
    );

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
        ai.culture_monument_item(&game, 0, cities[0], &plan)
            .is_some()
            && !ai.culture_monument_finishes_first(&game, 0, cities[0], &plan, &monument)
    });
    assert!(
        young.is_some(),
        "fixture: a claimable Monument far from done"
    );
    let shrine = Item::Building {
        building: crate::name!("shrine"),
    };
    game.cities.get_mut(&cities[0]).unwrap().queue = vec![shrine.clone()];
    claim(&ai, &mut game);
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&shrine));
}

// ---- `science-seat-monuments` ----------------------------------------------

fn science_seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    if gene {
        ai.enable_science_seat_monuments();
    }
    ai
}

fn building(name: &str) -> Item {
    Item::Building {
        building: crate::name::Name::new(name),
    }
}

/// The claim's first item in every city of ours, in city order.
fn heads(game: &Game, cities: &[u32]) -> Vec<Option<Item>> {
    cities
        .iter()
        .map(|cid| game.cities[cid].queue.first().cloned())
        .collect()
}

/// A third land site of ours, at least five tiles from each city in
/// `cities`, within ten of the first and, given `clear_of`, more than
/// [`MONUMENT_THREAT_RADIUS`] tiles from it. The new city's Monument has 50
/// Production banked and the queue is `head`.
fn add_city(game: &mut Game, cities: &[u32], clear_of: Option<Pos>, head: Item) -> u32 {
    let taken: Vec<Pos> = cities.iter().map(|cid| game.cities[cid].pos).collect();
    let site = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            !game.rules.is_water(tile)
                && game.wdist(**pos, taken[0]) <= 10
                && taken.iter().all(|t| game.wdist(**pos, *t) >= 5)
                && clear_of.is_none_or(|post| game.wdist(**pos, post) > MONUMENT_THREAT_RADIUS)
        })
        .map(|(pos, _)| *pos)
        .min()
        .expect("fixture: a third site");
    game.place_city(0, site, None);
    let cid = *game
        .player_city_ids(0)
        .iter()
        .find(|cid| !cities.contains(cid))
        .expect("fixture: the third city");
    let monument = building("monument");
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue = vec![head];
    city.production_progress
        .insert(Game::item_progress_key(&monument), 50.0);
    cid
}

/// [`two_cities`] and a third city, every queue idle.
fn three_cities() -> (Game, Vec<u32>, Item) {
    let (mut game, mut cities, monument) = two_cities();
    let third = add_city(&mut game, &cities, None, unit("scout"));
    game.cities.get_mut(&third).unwrap().queue.clear();
    cities.push(third);
    for cid in &cities {
        assert!(
            game.can_produce(0, *cid, &monument),
            "fixture: every city can build its Monument"
        );
    }
    (game, cities, monument)
}

#[test]
fn science_seat_monuments_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers("science-seat-monuments", |ai| {
        ai.science_seat_monuments
    });
}

/// From the third city on, every city of a Science seat training a Scout
/// puts its Monument first and the claim holds. Two cities claim nothing;
/// off, with only the Culture gene, or for a Domination seat nothing moves.
#[test]
fn a_science_seat_claims_every_citys_monument_from_its_third_city() {
    let (game, cities, monument) = three_cities();
    let mut busy = game.clone();
    for cid in &cities {
        busy.cities.get_mut(cid).unwrap().queue = vec![unit("scout")];
    }
    let mut domination = AdvancedAi::targeting(VictoryTarget::Domination);
    domination.enable_science_seat_monuments();
    for (ai, why) in [
        (science_seat(false), "off"),
        (seat(VictoryTarget::Science, true), "the Culture gene alone"),
        (domination, "a Domination seat"),
    ] {
        let mut kept = busy.clone();
        claim(&ai, &mut kept);
        assert_eq!(
            heads(&kept, &cities),
            vec![Some(unit("scout")); 3],
            "{why}: no claim"
        );
        assert!(!ai.culture_monuments_hold_locally(&kept), "{why}");
    }

    let ai = science_seat(true);
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
    assert!(ai.culture_monuments_hold_locally(&on));

    let (mut two, pair, _) = two_cities();
    for cid in &pair {
        two.cities.get_mut(cid).unwrap().queue = vec![unit("scout")];
    }
    let plan = ai.assess(&two, 0);
    assert!(ai.culture_monument_item(&two, 0, pair[1], &plan).is_none());
    claim(&ai, &mut two);
    assert_eq!(
        heads(&two, &pair),
        vec![Some(unit("scout")); 2],
        "two cities wait for the third"
    );
}

/// Science (a Library, a Campus), a Settler and Walls keep their queue on a
/// Science seat; a Culture seat's claim passes the Library and the Campus.
#[test]
fn science_settlers_and_walls_keep_a_science_seats_queue() {
    let (game, cities, monument) = three_cities();
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: game.cities[&cities[2]].pos,
    };
    let mut busy = game.clone();
    for (cid, head) in cities
        .iter()
        .zip([building("library"), unit("settler"), campus.clone()])
    {
        busy.cities.get_mut(cid).unwrap().queue = vec![head];
    }

    let mut science = busy.clone();
    claim(&science_seat(true), &mut science);
    assert_eq!(
        heads(&science, &cities),
        vec![
            Some(building("library")),
            Some(unit("settler")),
            Some(campus.clone())
        ],
        "science and the Settler keep their queue"
    );

    let mut culture = busy.clone();
    claim(&seat(VictoryTarget::Culture, true), &mut culture);
    assert_eq!(
        heads(&culture, &cities),
        vec![
            Some(monument.clone()),
            Some(unit("settler")),
            Some(monument.clone())
        ],
        "the Culture seat passes science"
    );

    let mut walls = game.clone();
    walls.cities.get_mut(&cities[0]).unwrap().queue = vec![building("walls")];
    claim(&science_seat(true), &mut walls);
    assert_eq!(
        walls.cities[&cities[0]].queue.first(),
        Some(&building("walls"))
    );
}

/// Version two's near-done pass stays the Culture seat's: a Science seat
/// with both genes keeps its Library ahead of a near-done Monument.
#[test]
fn the_near_done_pass_stays_on_the_culture_seat() {
    let (mut game, cities, monument) = three_cities();
    game.cities.get_mut(&cities[0]).unwrap().queue = vec![building("library")];
    let mut ai = science_seat(true);
    ai.enable_culture_seat_monuments_2();
    let plan = ai.assess(&game, 0);
    assert!(ai
        .culture_monument_item(&game, 0, cities[0], &plan)
        .is_some());
    assert!(!ai.culture_monument_finishes_first(&game, 0, cities[0], &plan, &monument));
    claim(&ai, &mut game);
    assert_eq!(
        game.cities[&cities[0]].queue.first(),
        Some(&building("library"))
    );
}

/// A Culture seat plays as before with the Science gene on: the same heads
/// after the claim on an open board and at war, and no claim through the war
/// for version one.
#[test]
fn a_culture_seat_plays_as_before_with_the_science_gene_on() {
    let (game, cities, _) = three_cities();
    let mut busy = game.clone();
    for (cid, head) in cities
        .iter()
        .zip([unit("scout"), building("library"), unit("settler")])
    {
        busy.cities.get_mut(cid).unwrap().queue = vec![head];
    }
    let (war, war_cities, _) = at_war_with_a_soldier_beside_one_city();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    };
    for v2 in [false, true] {
        let mut culture = seat(VictoryTarget::Culture, true);
        if v2 {
            culture.enable_culture_seat_monuments_2();
        }
        let mut both = culture.clone();
        both.enable_science_seat_monuments();
        assert_eq!(
            culture.culture_monuments_hold_locally(&busy),
            both.culture_monuments_hold_locally(&busy)
        );

        let (mut before, mut after) = (busy.clone(), busy.clone());
        claim(&culture, &mut before);
        claim(&both, &mut after);
        assert_eq!(heads(&before, &cities), heads(&after, &cities), "v2 {v2}");

        let (mut before, mut after) = (war.clone(), war.clone());
        culture.claim_culture_monuments(&mut before, 0, &plan);
        both.claim_culture_monuments(&mut after, 0, &plan);
        assert_eq!(
            heads(&before, &war_cities),
            heads(&after, &war_cities),
            "v2 {v2} at war"
        );
        if !v2 {
            assert_eq!(heads(&after, &war_cities), vec![Some(unit("scout")); 2]);
        }
    }

    let mut idle = seat(VictoryTarget::Culture, false);
    idle.enable_science_seat_monuments();
    let mut kept = busy.clone();
    claim(&idle, &mut kept);
    assert_eq!(heads(&kept, &cities), heads(&busy, &cities));
}

/// At war with a major, a Science seat claims in its cities out of the
/// rival's reach and holds the one with the rival's soldier beside it.
#[test]
fn a_science_seat_claims_through_the_war_outside_the_soldiers_reach() {
    let (mut game, mut cities, monument) = at_war_with_a_soldier_beside_one_city();
    let post = game
        .player_unit_ids(1)
        .into_iter()
        .map(|uid| game.units[&uid].pos)
        .next()
        .expect("fixture: the rival's soldier");
    let third = add_city(&mut game, &cities, Some(post), unit("scout"));
    cities.push(third);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    };

    let mut off = game.clone();
    science_seat(false).claim_culture_monuments(&mut off, 0, &plan);
    assert_eq!(heads(&off, &cities), vec![Some(unit("scout")); 3]);

    let ai = science_seat(true);
    let mut on = game.clone();
    ai.claim_culture_monuments(&mut on, 0, &plan);
    assert_eq!(
        heads(&on, &cities),
        vec![
            Some(monument.clone()),
            Some(unit("scout")),
            Some(monument.clone())
        ],
        "the cities out of reach claim; the one beside the soldier holds"
    );
    assert!(ai.culture_monument_holds(&on, 0, cities[0], &plan, &monument));
    assert!(ai.culture_monument_item(&on, 0, cities[1], &plan).is_none());

    let mut threatened = game.clone();
    let plan = StrategicPlan {
        threatened_city: Some(cities[2]),
        ..plan
    };
    ai.claim_culture_monuments(&mut threatened, 0, &plan);
    assert_eq!(
        threatened.cities[&cities[2]].queue.first(),
        Some(&unit("scout")),
        "the plan's threatened city keeps its queue"
    );
}
