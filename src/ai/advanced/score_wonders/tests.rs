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

/// A standing race: a writer's Monument is displaced again; the finished
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
    // A later writer puts a Monument back; the late pass restores the race.
    g.cities.get_mut(&capital).unwrap().queue = vec![monument.clone()];
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
