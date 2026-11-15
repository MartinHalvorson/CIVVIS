use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    if gene {
        ai.enable_score_seat_raises_wonders();
    }
    ai
}

fn plan(turn: u32) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 8,
        assessed_turn: turn,
        rush: false,
    }
}

/// Two majors at Online speed, turn `turn` of 170, every unit removed; ours
/// holds its capital and two more cities five tiles out and knows every
/// Ancient and Classical technology and civic. Each wonder the capital can
/// build stands ten Production from done, so every race is a few turns.
/// Returns the game and our capital.
fn three_cities(turn: u32) -> (Game, u32) {
    let mut g = Game::new_full(2, 40, 26, 170_412, 170, 0, false);
    g.current = 0;
    g.game_speed = crate::setup::GameSpeed::Online;
    let settlers: Vec<_> = g
        .units
        .values()
        .filter(|u| u.kind == "settler")
        .map(|u| (u.owner, u.pos))
        .collect();
    for (owner, pos) in settlers {
        g.found_city_for(owner, pos, None);
    }
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    let capital = g.player_city_ids(0)[0];
    let centre = g.cities[&capital].pos;
    let occupied: Vec<Pos> = g.cities.values().map(|city| city.pos).collect();
    let mut land: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    let mut placed: Vec<Pos> = vec![centre];
    for pos in land {
        if placed.len() == 3 {
            break;
        }
        if g.wdist(pos, centre) == 5
            && placed.iter().all(|other| g.wdist(pos, *other) >= 4)
            && occupied.iter().all(|other| g.wdist(pos, *other) >= 4)
        {
            placed.push(pos);
        }
    }
    assert_eq!(placed.len(), 3, "room for three cities");
    for pos in &placed[1..] {
        g.place_city(0, *pos, None);
    }
    let techs: Vec<Name> = g
        .rules
        .techs
        .iter()
        .filter(|(_, node)| node.era <= 1)
        .map(|(name, _)| *name)
        .collect();
    let civics: Vec<Name> = g
        .rules
        .civics
        .iter()
        .filter(|(_, node)| node.era <= 1)
        .map(|(name, _)| *name)
        .collect();
    g.players[0].techs.extend(techs);
    g.players[0].civics.extend(civics);
    g.max_turns = 170;
    g.turn = turn;
    let wonders: Vec<Item> = g
        .producible_items(0, capital)
        .into_iter()
        .filter(|item| matches!(item, Item::Wonder { .. }))
        .collect();
    for item in wonders {
        let cost = g.item_cost_for_city(0, capital, &item);
        g.cities
            .get_mut(&capital)
            .unwrap()
            .production_progress
            .insert(Game::item_progress_key(&item), (cost - 10.0).max(0.0));
    }
    (g, capital)
}

fn head(g: &Game, cid: u32) -> Option<&Item> {
    g.cities[&cid].queue.first()
}

fn counter(g: &Game, key: &str) -> u32 {
    g.players[0].counters.get(key).copied().unwrap_or(0) as u32
}

#[test]
fn score_seat_raises_wonders_is_opt_in() {
    opt_in_off_in_both_controllers("score-seat-raises-wonders", |ai| {
        ai.score_seat_raises_wonders
    });
}

/// The moment, the race chance and the lanes, as the module states them.
#[test]
fn the_moment_the_chance_and_the_lanes() {
    assert_eq!(AdvancedAi::score_wonder_moment(4, 4), 4);
    assert_eq!(AdvancedAi::score_wonder_moment(2, 4), 3);
    let this_era = AdvancedAi::score_wonder_chance(4, 4, 25.0);
    assert!((this_era - (-1.0_f64).exp()).abs() < 1e-9);
    assert!(
        this_era < SCORE_WONDER_MIN_CHANCE,
        "a 25-turn race of the era"
    );
    assert!(AdvancedAi::score_wonder_chance(3, 4, 25.0) > SCORE_WONDER_MIN_CHANCE);
    assert!(
        AdvancedAi::score_wonder_chance(2, 4, 14.0) > AdvancedAi::score_wonder_chance(3, 4, 14.0),
        "an older wonder is the safer race"
    );
    let (mut g, _) = three_cities(39);
    assert_eq!(AdvancedAi::score_wonder_lanes(&g, 3), 0, "turn 39 Online");
    g.turn = 40;
    assert_eq!(AdvancedAi::score_wonder_lanes(&g, 2), 0, "two cities");
    assert_eq!(AdvancedAi::score_wonder_lanes(&g, 3), 1);
    g.turn = 90;
    assert_eq!(AdvancedAi::score_wonder_lanes(&g, 5), 1, "five cities");
    assert_eq!(AdvancedAi::score_wonder_lanes(&g, 6), 2);
}

/// A capital with a routine head takes the best race; off, on another target,
/// before turn 40, or with a Settler at its head, nothing moves.
#[test]
fn the_capital_raises_a_wonder_over_routine_work_only() {
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let (mut g, capital) = three_cities(60);
    assert!(
        g.producible_items(0, capital)
            .iter()
            .any(|item| matches!(item, Item::Wonder { .. })),
        "the capital has a wonder to raise"
    );
    g.cities.get_mut(&capital).unwrap().queue = vec![monument.clone()];
    let mut on = seat(true);
    on.claim_queue_for_the_score_wonder(&mut g, 0, &plan(60), true);
    assert!(
        matches!(head(&g, capital), Some(Item::Wonder { .. })),
        "the race takes the capital: {:?}",
        head(&g, capital)
    );
    assert_eq!(counter(&g, "score_wonder_starts"), 1);
    assert_eq!(on.score_wonder_races.len(), 1);

    for (label, mut ai, turn, queue) in [
        ("off", seat(false), 60, monument.clone()),
        ("before turn 40", seat(true), 39, monument.clone()),
        ("a Settler", seat(true), 60, settler.clone()),
    ] {
        let (mut g, capital) = three_cities(turn);
        g.cities.get_mut(&capital).unwrap().queue = vec![queue.clone()];
        ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan(turn), true);
        assert_eq!(head(&g, capital), Some(&queue), "{label}");
    }
    let (mut g, capital) = three_cities(60);
    g.cities.get_mut(&capital).unwrap().queue = vec![monument.clone()];
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_score_seat_raises_wonders();
    science.claim_queue_for_the_score_wonder(&mut g, 0, &plan(60), true);
    assert_eq!(head(&g, capital), Some(&monument), "a Science seat");
}

/// The displacement rule: Walls, a first Library, a Granary, a Repair and a
/// head two turns from done are kept; a Monument is passed.
#[test]
fn the_race_passes_only_routine_heads() {
    let (mut g, capital) = three_cities(60);
    let ai = seat(true);
    for (building, passes) in [
        ("monument", true),
        ("walls", false),
        ("library", false),
        ("granary", false),
    ] {
        let item = Item::Building {
            building: Name::new(building),
        };
        g.cities.get_mut(&capital).unwrap().queue = vec![item.clone()];
        g.cities
            .get_mut(&capital)
            .unwrap()
            .production_progress
            .remove(&Game::item_progress_key(&item));
        assert_eq!(
            ai.score_wonder_may_pass(&g, 0, capital),
            passes,
            "{building}"
        );
    }
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let cost = g.item_cost_for_city(0, capital, &monument);
    g.cities.get_mut(&capital).unwrap().queue = vec![monument.clone()];
    g.cities
        .get_mut(&capital)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&monument), cost - 0.5);
    assert!(
        !ai.score_wonder_may_pass(&g, 0, capital),
        "a Monument about to finish is left to finish"
    );
    g.cities.get_mut(&capital).unwrap().queue.clear();
    assert!(ai.score_wonder_may_pass(&g, 0, capital), "an idle queue");
}

/// A standing race: a writer's Builder is displaced again; the finished
/// wonder is counted and dropped; a wonder a rival finished is a lost race.
#[test]
fn a_race_is_kept_finished_or_lost() {
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let (mut g, capital) = three_cities(60);
    g.cities.get_mut(&capital).unwrap().queue = vec![monument.clone()];
    let mut ai = seat(true);
    ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan(60), true);
    let Some(Item::Wonder { wonder, .. }) = head(&g, capital).cloned() else {
        panic!("a race starts");
    };
    // A later writer puts a Builder at the head (through the action, which
    // banks the wonder's progress); the late pass restores the race.
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: capital,
            item: builder.clone(),
        },
    )
    .unwrap();
    assert_eq!(head(&g, capital), Some(&builder));
    ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan(60), false);
    assert!(
        matches!(head(&g, capital), Some(Item::Wonder { wonder: w, .. }) if *w == wonder),
        "the race is kept"
    );
    // Finished.
    let mut finished = g.clone();
    let mut done = ai.clone();
    let pos = finished.cities[&capital].pos;
    finished
        .cities
        .get_mut(&capital)
        .unwrap()
        .wonders
        .insert(wonder, pos);
    done.claim_queue_for_the_score_wonder(&mut finished, 0, &plan(60), false);
    assert!(done.score_wonder_races.is_empty());
    assert_eq!(counter(&finished, "score_wonder_finishes"), 1);
    // Lost: the rival holds it.
    let rival = g.player_city_ids(1)[0];
    let rival_pos = g.cities[&rival].pos;
    g.cities
        .get_mut(&rival)
        .unwrap()
        .wonders
        .insert(wonder, rival_pos);
    ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan(60), false);
    assert!(ai.score_wonder_races.is_empty());
    assert_eq!(counter(&g, "score_wonder_losses"), 1);
}

// ---- version 2 ----------------------------------------------------------

fn seat_v2() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Score);
    ai.enable_score_seat_raises_wonders();
    ai.enable_score_seat_raises_wonders_2();
    ai
}

fn plan_wanting(turn: u32, cities: usize) -> StrategicPlan {
    StrategicPlan {
        desired_cities: cities,
        ..plan(turn)
    }
}

/// A met rival city eight tiles from our capital; ours fields 100 military,
/// the rival `rival_power`.
fn rival_beside(g: &mut Game, capital: u32, rival_power: f64) {
    let centre = g.cities[&capital].pos;
    let occupied: Vec<Pos> = g.cities.values().map(|city| city.pos).collect();
    let mut land: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    let site = *land
        .iter()
        .find(|pos| {
            g.wdist(**pos, centre) == 8 && occupied.iter().all(|other| g.wdist(**pos, *other) >= 4)
        })
        .expect("a rival site eight tiles out");
    g.place_city(1, site, None);
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    std::sync::Arc::make_mut(&mut g.observed_military_power).insert(0, 100.0);
    std::sync::Arc::make_mut(&mut g.observed_military_power).insert(1, rival_power);
}

#[test]
fn score_seat_raises_wonders_2_is_opt_in_and_replaces_version_1() {
    opt_in_off_in_both_controllers("score-seat-raises-wonders-2", |ai| {
        ai.score_seat_raises_wonders_2
    });
    let ai = seat_v2();
    assert!(!ai.score_seat_raises_wonders && ai.score_seat_raises_wonders_2);
    assert!(ai.score_wonders_on());
}

/// ★ Expansion first: before turn 80 (Online) under the city target, or with
/// a Settler queued anywhere, the lanes hold; at the target with none queued,
/// or from turn 80, they do not. Version 1 never holds.
#[test]
fn version_2_holds_the_lanes_for_expansion_before_turn_80() {
    let v2 = seat_v2();
    let (mut g, capital) = three_cities(60);
    assert!(v2
        .score_wonder_v2_hold(&g, 0, &plan_wanting(60, 8))
        .is_some_and(|why| why.contains("3 cities of the 8")));
    assert_eq!(v2.score_wonder_v2_hold(&g, 0, &plan_wanting(60, 3)), None);
    assert_eq!(
        seat(true).score_wonder_v2_hold(&g, 0, &plan_wanting(60, 8)),
        None
    );
    let other = *g
        .player_city_ids(0)
        .iter()
        .find(|cid| **cid != capital)
        .unwrap();
    g.cities.get_mut(&other).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    assert!(v2
        .score_wonder_v2_hold(&g, 0, &plan_wanting(60, 3))
        .is_some_and(|why| why.contains("Settler")));
    g.turn = 80;
    assert_eq!(v2.score_wonder_v2_hold(&g, 0, &plan_wanting(80, 8)), None);

    // The claim itself: held at t60 under the target, the capital keeps its
    // Monument; version 1 races.
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for (mut ai, races) in [(seat_v2(), false), (seat(true), true)] {
        let (mut g, capital) = three_cities(60);
        g.cities.get_mut(&capital).unwrap().queue = vec![monument.clone()];
        ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan_wanting(60, 8), true);
        assert_eq!(
            matches!(head(&g, capital), Some(Item::Wonder { .. })),
            races,
            "version 1 races: {races}"
        );
    }
}

/// ★ Safety first: at war, or beside a neighbour with 1.5 times our military,
/// the lanes hold; at 1.4 times they do not.
#[test]
fn version_2_holds_the_lanes_at_war_and_beside_a_stronger_neighbour() {
    let v2 = seat_v2();
    let (mut g, capital) = three_cities(90);
    rival_beside(&mut g, capital, 150.0);
    assert!(v2
        .score_wonder_v2_hold(&g, 0, &plan_wanting(90, 3))
        .is_some_and(|why| why.contains("military")));
    std::sync::Arc::make_mut(&mut g.observed_military_power).insert(1, 140.0);
    assert_eq!(v2.score_wonder_v2_hold(&g, 0, &plan_wanting(90, 3)), None);
    g.at_war.insert((0, 1));
    assert!(v2
        .score_wonder_v2_hold(&g, 0, &plan_wanting(90, 3))
        .is_some_and(|why| why.contains("at war")));
}

/// ★ Never in the city that trains the Settlers, and a held race is not put
/// back at its head.
#[test]
fn version_2_spares_the_settler_city_and_pauses_a_held_race() {
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    // Turn 90, target met: the capital, a Settler queued behind its Monument,
    // is skipped by version 2 and raced by version 1.
    for (mut ai, races) in [(seat_v2(), false), (seat(true), true)] {
        let (mut g, capital) = three_cities(90);
        g.cities.get_mut(&capital).unwrap().queue = vec![monument.clone(), settler.clone()];
        ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan_wanting(90, 3), true);
        assert_eq!(
            matches!(head(&g, capital), Some(Item::Wonder { .. })),
            races,
            "version 1 races: {races}"
        );
    }
    // A race started with the lanes open; then a war: a Builder put at the
    // head by a later writer stays there.
    let (mut g, capital) = three_cities(90);
    g.cities.get_mut(&capital).unwrap().queue = vec![monument.clone()];
    let mut ai = seat_v2();
    ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan_wanting(90, 3), true);
    assert!(matches!(head(&g, capital), Some(Item::Wonder { .. })));
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: capital,
            item: builder.clone(),
        },
    )
    .unwrap();
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.at_war.insert((0, 1));
    ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan_wanting(90, 3), false);
    assert_eq!(
        head(&g, capital),
        Some(&builder),
        "the race waits for the war"
    );
    assert_eq!(ai.score_wonder_races.len(), 1, "and stays on the books");
}

// ---- version 3 ----------------------------------------------------------

fn seat_v3() -> AdvancedAi {
    let mut ai = seat_v2();
    ai.enable_score_seat_raises_wonders_3();
    ai
}

/// Every wonder `cid` can build stands ten Production from done.
fn nearly_done(g: &mut Game, cid: u32) {
    let wonders: Vec<Item> = g
        .producible_items(0, cid)
        .into_iter()
        .filter(|item| matches!(item, Item::Wonder { .. }))
        .collect();
    assert!(!wonders.is_empty(), "the city can build a wonder");
    for item in wonders {
        let cost = g.item_cost_for_city(0, cid, &item);
        g.cities
            .get_mut(&cid)
            .unwrap()
            .production_progress
            .insert(Game::item_progress_key(&item), (cost - 10.0).max(0.0));
    }
}

/// The nearest city of player 1 to `cid`, in tiles.
fn to_the_rival(g: &Game, cid: u32) -> i32 {
    let pos = g.cities[&cid].pos;
    g.cities
        .values()
        .filter(|city| city.owner == 1)
        .map(|city| g.wdist(pos, city.pos))
        .min()
        .unwrap_or(i32::MAX)
}

/// [`three_cities`] at turn 90 with a met rival fielding `rival_power` to our
/// 100 and a city of its beside our capital (under ten tiles) and our
/// `front` city, while our `rear` city stands ten or more from every rival
/// city. Every city works a Monument; the rear city's wonders, like the
/// capital's, stand ten Production from done. Returns the game, the
/// capital, the rear and the front city.
fn front_and_rear(rival_power: f64) -> (Game, u32, u32, u32) {
    let (mut g, capital) = three_cities(90);
    let others: Vec<u32> = g
        .player_city_ids(0)
        .into_iter()
        .filter(|cid| *cid != capital)
        .collect();
    let occupied: Vec<Pos> = g.cities.values().map(|city| city.pos).collect();
    let mut land: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    let centre = g.cities[&capital].pos;
    let rival_home = to_the_rival;
    let mut found = None;
    'search: for (rear, front) in [(others[0], others[1]), (others[1], others[0])] {
        for pos in &land {
            let (rear_pos, front_pos) = (g.cities[&rear].pos, g.cities[&front].pos);
            if (5..=9).contains(&g.wdist(*pos, centre))
                && g.wdist(*pos, front_pos) < SCORE_WONDER_V3_REAR_TILES
                && g.wdist(*pos, rear_pos) >= SCORE_WONDER_V3_REAR_TILES
                && rival_home(&g, rear) >= SCORE_WONDER_V3_REAR_TILES
                && occupied.iter().all(|other| g.wdist(*pos, *other) >= 4)
            {
                found = Some((*pos, rear, front));
                break 'search;
            }
        }
    }
    let (site, rear, front) = found.expect("a rival site beside the capital and the front");
    g.place_city(1, site, None);
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    std::sync::Arc::make_mut(&mut g.observed_military_power).insert(0, 100.0);
    std::sync::Arc::make_mut(&mut g.observed_military_power).insert(1, rival_power);
    nearly_done(&mut g, rear);
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for cid in [capital, rear, front] {
        g.cities.get_mut(&cid).unwrap().queue = vec![monument.clone()];
    }
    assert!(to_the_rival(&g, capital) < SCORE_WONDER_V3_REAR_TILES);
    assert!(to_the_rival(&g, front) < SCORE_WONDER_V3_REAR_TILES);
    assert!(to_the_rival(&g, rear) >= SCORE_WONDER_V3_REAR_TILES);
    (g, capital, rear, front)
}

fn races(g: &Game, cid: u32) -> bool {
    matches!(head(g, cid), Some(Item::Wonder { .. }))
}

#[test]
fn score_seat_raises_wonders_3_is_opt_in_and_replaces_versions_1_and_2() {
    opt_in_off_in_both_controllers("score-seat-raises-wonders-3", |ai| {
        ai.score_seat_raises_wonders_3
    });
    let ai = seat_v3();
    assert!(
        !ai.score_seat_raises_wonders
            && !ai.score_seat_raises_wonders_2
            && ai.score_seat_raises_wonders_3
    );
    assert!(ai.score_wonders_on());
    // It carries version 2's holds.
    let (g, _) = three_cities(60);
    assert!(ai
        .score_wonder_v2_hold(&g, 0, &plan_wanting(60, 8))
        .is_some_and(|why| why.starts_with("expansion first")));
}

/// ★ Beside a neighbour with 1.5 times our military, version 3 races in the
/// rear city and nowhere else; version 2 races nowhere (unchanged), version
/// 1 in the capital (unchanged). At war the rear lane stays open.
#[test]
fn version_3_races_in_the_rear_city_through_a_threat_hold() {
    let plan = plan_wanting(90, 3);
    let (mut g, capital, rear, front) = front_and_rear(150.0);
    let mut v3 = seat_v3();
    assert!(v3
        .score_wonder_v2_hold(&g, 0, &plan)
        .is_some_and(|why| why.starts_with("safety first")));
    let picked = v3.score_wonder_v3_rear(&g, 0, &plan).expect("a rear city");
    assert_eq!((picked.city, picked.threat), (rear, 1));
    assert!(picked.tiles >= SCORE_WONDER_V3_REAR_TILES);
    v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, true);
    assert!(races(&g, rear), "the rear city races");
    assert!(
        !races(&g, capital),
        "the capital beside the threat does not"
    );
    assert!(!races(&g, front), "nor the frontier city");
    assert_eq!(counter(&g, "score_wonder_rear_starts"), 1);
    assert_eq!(counter(&g, "score_wonder_rear_turns"), 1);
    assert!(v3
        .score_wonder_rear_note
        .as_deref()
        .is_some_and(|note| note.contains("keeps the rear lane") && note.contains("tiles")));

    for (mut ai, capital_races) in [(seat_v2(), false), (seat(true), true)] {
        let (mut g, capital, rear, front) = front_and_rear(150.0);
        ai.claim_queue_for_the_score_wonder(&mut g, 0, &plan, true);
        assert_eq!(
            races(&g, capital),
            capital_races,
            "version 1 races: {capital_races}"
        );
        assert!(!races(&g, rear) && !races(&g, front));
        assert_eq!(counter(&g, "score_wonder_rear_turns"), 0);
    }

    // At war (even with the military level) the rear lane stays open.
    let (mut g, capital, rear, _) = front_and_rear(100.0);
    g.at_war.insert((0, 1));
    let mut v3 = seat_v3();
    v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, true);
    assert!(races(&g, rear) && !races(&g, capital));

    // No hold: version 3 plays version 2's lanes (the capital first).
    let (mut g, capital, rear, _) = front_and_rear(100.0);
    let mut v3 = seat_v3();
    v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, true);
    assert!(races(&g, capital) && !races(&g, rear));
    assert_eq!(counter(&g, "score_wonder_rear_turns"), 0);
}

/// ★ The capital keeps the lane when it is itself the rear: at war with a
/// rival whose only city stands ten or more tiles from every one of ours.
#[test]
fn version_3_races_in_the_capital_when_it_is_the_rear() {
    let plan = plan_wanting(90, 3);
    let (mut g, capital) = three_cities(90);
    let ours = g.player_city_ids(0);
    if ours
        .iter()
        .any(|cid| to_the_rival(&g, *cid) < SCORE_WONDER_V3_REAR_TILES)
    {
        panic!("the fixture's rival capital stands under ten tiles from a city of ours");
    }
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.at_war.insert((0, 1));
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for cid in &ours {
        g.cities.get_mut(cid).unwrap().queue = vec![monument.clone()];
    }
    let mut v3 = seat_v3();
    let picked = v3.score_wonder_v3_rear(&g, 0, &plan).expect("a rear city");
    assert_eq!(picked.city, capital, "the most productive rear city");
    v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, true);
    assert!(races(&g, capital));
}

/// ★ War at the gates closes the rear lane: a threatened city, a city
/// attacked in the last four turns, or a Recovery plan.
#[test]
fn war_at_the_gates_closes_the_rear_lane() {
    let plan = plan_wanting(90, 3);
    let recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..plan.clone()
    };
    for case in ["threatened", "attacked", "recovery"] {
        let (mut g, capital, rear, front) = front_and_rear(150.0);
        let plan = match case {
            "threatened" => StrategicPlan {
                threatened_city: Some(front),
                ..plan.clone()
            },
            "recovery" => recovery.clone(),
            _ => {
                g.cities.get_mut(&capital).unwrap().last_attacked = 88;
                plan.clone()
            }
        };
        let mut v3 = seat_v3();
        assert!(v3.score_wonder_v3_rear(&g, 0, &plan).is_err(), "{case}");
        v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, true);
        assert!(!races(&g, rear), "{case}: the rear city waits");
        assert!(
            v3.score_wonder_rear_note
                .as_deref()
                .is_some_and(|note| note.starts_with("the rear lane is shut")),
            "{case}"
        );
    }
    // An attack five turns ago no longer closes it.
    let (mut g, capital, rear, _) = front_and_rear(150.0);
    g.cities.get_mut(&capital).unwrap().last_attacked = 85;
    let mut v3 = seat_v3();
    v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, true);
    assert!(races(&g, rear));
}

/// ★ A race in the rear city goes back to its head while the city
/// qualifies; once a threat city rises within ten tiles of it, a later
/// writer's head stays, as version 2 holds a race, and the race stays on the
/// books.
#[test]
fn a_rear_race_is_held_once_its_city_stops_qualifying() {
    let plan = plan_wanting(90, 3);
    let (mut g, _, rear, _) = front_and_rear(150.0);
    let mut v3 = seat_v3();
    v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, true);
    assert!(races(&g, rear));
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let displace = |g: &mut Game| {
        g.apply(
            0,
            &Action::Produce {
                city: rear,
                item: builder.clone(),
            },
        )
        .unwrap();
    };
    displace(&mut g);
    v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, false);
    assert!(races(&g, rear), "the rear lane keeps its race");

    // A rival city rises five tiles from the rear city.
    let rear_pos = g.cities[&rear].pos;
    let occupied: Vec<Pos> = g.cities.values().map(|city| city.pos).collect();
    let site = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .filter(|pos| {
            g.wdist(*pos, rear_pos) == 5 && occupied.iter().all(|other| g.wdist(*pos, *other) >= 4)
        })
        .min()
        .expect("a rival site near the rear city");
    g.place_city(1, site, None);
    displace(&mut g);
    v3.claim_queue_for_the_score_wonder(&mut g, 0, &plan, false);
    assert_eq!(head(&g, rear), Some(&builder), "the race waits");
    assert_eq!(v3.score_wonder_races.len(), 1, "and stays on the books");
    assert!(v3
        .score_wonder_rear_note
        .as_deref()
        .is_some_and(|note| note.contains("no city stands 10 tiles")));
}
