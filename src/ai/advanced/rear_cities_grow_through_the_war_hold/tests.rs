use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::ai::industrial_zones::tests::producer_empire;

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

/// A routine head both claims may displace: a non-wonder building outside
/// their defence, faith, housing, science and culture lists, far from done.
fn bank() -> Item {
    building("bank")
}

fn head(game: &Game, cid: u32) -> Option<Item> {
    game.cities[&cid].queue.first().cloned()
}

/// Recovery holds the claims while the war lasts; no city is named.
fn recovery() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 0,
        rush: false,
    }
}

/// The hold, the Granary and Campus building claims, and this gene when
/// `rear`.
fn seat(rear: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_threatened_war_holds_the_claims();
    ai.enable_granary_claims_the_bound_queue();
    ai.enable_campus_building_claims_the_queue();
    if rear {
        ai.enable_rear_cities_grow_through_the_war_hold();
    }
    ai
}

/// Two majors at war, a capital each, every unit removed, turn 60. Our
/// capital is the front: a hostile Warrior stands two tiles from it. Our
/// second city is the rear, 9-12 tiles from the capital, so the Warrior is
/// more than six tiles from it. Both cities are at their housing with Pottery
/// known, 60 of the Granary's 65 Production banked and a Bank at the head of the
/// queue. Returns the game, the front city and the rear city.
fn front_and_rear() -> (Game, u32, u32) {
    let mut game = Game::new_full(2, 44, 26, 931_207, 250, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("fixture: every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
    }
    for pid in 0..2 {
        for unit in game.player_unit_ids(pid) {
            game.remove_unit(unit);
        }
    }
    let front = game.player_city_ids(0)[0];
    let home = game.cities[&front].pos;
    let site = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && tile.owner_city.is_none()
                && (9..=12).contains(&game.wdist(home, tile.pos))
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(city.pos, tile.pos) >= 5)
        })
        .map(|tile| tile.pos)
        .min()
        .expect("fixture: a rear site");
    let rear = game.place_city(0, site, None);
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    game.players[0].techs.insert(crate::name!("pottery"));
    game.turn = 60;
    game.current = 0;
    let granary = building("granary");
    for cid in [front, rear] {
        let housing = game.city_housing(&game.cities[&cid]);
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = (housing.ceil() as i32).max(2);
        city.queue = vec![bank()];
        city.production_progress
            .insert(Game::item_progress_key(&granary), 60.0);
        assert!(
            game.can_produce(0, cid, &granary),
            "fixture: the Granary is buildable"
        );
    }
    let near = game
        .map
        .tiles
        .values()
        .filter(|tile| {
            !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && game.city_at(tile.pos).is_none()
                && game.wdist(home, tile.pos) == 2
        })
        .map(|tile| tile.pos)
        .min()
        .expect("fixture: a land tile two out");
    game.spawn_test_unit("warrior", 1, near);
    (game, front, rear)
}

#[test]
fn rear_cities_grow_through_the_war_hold_is_opt_in() {
    opt_in_off_in_both_controllers("rear-cities-grow-through-the-war-hold", |ai| {
        ai.rear_cities_grow_through_the_war_hold
    });
}

/// Under the hold, the housing-bound rear city claims its Granary while the
/// front city, which a hostile presses, keeps its queue. On its own rules the
/// front city's Granary is due, so the rear filter is what holds it.
#[test]
fn under_the_hold_the_rear_city_claims_its_granary_and_the_front_does_not() {
    let (mut game, front, rear) = front_and_rear();
    let plan = recovery();
    let mut ai = seat(true);
    assert!(
        ai.threatened_war_holds_the_claims_holds(&game, 0, &plan),
        "fixture: the war holds the claims"
    );
    assert!(
        ai.bound_granary_item(&game, 0, front, &plan).is_some(),
        "fixture: on the claim's own rules the front city's Granary is due"
    );
    assert_eq!(
        ai.rear_cities_under_the_war_hold(&game, 0, &plan),
        BTreeSet::from([rear]),
        "the pressed city is the front, the far one the rear"
    );
    ai.run_idle_queue_claims(&mut game, 0, &plan);
    assert_eq!(head(&game, rear), Some(building("granary")));
    assert_eq!(head(&game, front), Some(bank()), "the front keeps the hold");
    assert!(
        ai.claim_open_behind_the_front(front),
        "the pass closes its rear set"
    );
}

/// With the gene off the hold is what it was: neither city claims.
#[test]
fn off_the_hold_keeps_every_city() {
    let (mut game, front, rear) = front_and_rear();
    let plan = recovery();
    let mut ai = seat(false);
    assert!(ai.threatened_war_holds_the_claims_holds(&game, 0, &plan));
    ai.run_idle_queue_claims(&mut game, 0, &plan);
    assert_eq!(head(&game, rear), Some(bank()));
    assert_eq!(head(&game, front), Some(bank()));
}

/// A city attacked two turns ago is the front, as is the plan's threatened
/// city; one attacked five turns ago is the rear again.
#[test]
fn a_city_attacked_two_turns_ago_counts_as_front() {
    let (mut game, front, rear) = front_and_rear();
    let plan = recovery();
    let mut ai = seat(true);
    game.cities.get_mut(&rear).unwrap().last_attacked = game.turn - 2;
    assert!(ai
        .rear_cities_under_the_war_hold(&game, 0, &plan)
        .is_empty());
    let mut claimed = game.clone();
    ai.run_idle_queue_claims(&mut claimed, 0, &plan);
    assert_eq!(head(&claimed, rear), Some(bank()));
    assert_eq!(head(&claimed, front), Some(bank()));

    game.cities.get_mut(&rear).unwrap().last_attacked = game.turn - 5;
    assert_eq!(
        ai.rear_cities_under_the_war_hold(&game, 0, &plan),
        BTreeSet::from([rear])
    );
    let mut named = recovery();
    named.threatened_city = Some(rear);
    assert!(ai
        .rear_cities_under_the_war_hold(&game, 0, &named)
        .is_empty());
}

/// Six Campus cities with Education at war, the quickest named threatened:
/// under the hold the quickest rear city claims its University and the front
/// city does not; with the gene off nothing is claimed.
#[test]
fn under_the_hold_a_rear_city_claims_its_campus_building_and_the_front_does_not() {
    let (mut game, ranked) = producer_empire("REARCAMPUS", 94_201, 2);
    for tech in ["education", "mathematics"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for cid in &ranked {
        assert_eq!(
            BasicAi::campus_building_item(&game, 0, *cid),
            Some(building("university")),
            "fixture: the University is buildable"
        );
        game.cities.get_mut(cid).unwrap().queue = vec![bank()];
    }
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    game.current = 0;
    let mut plan = recovery();
    plan.threatened_city = Some(ranked[0]);
    let universities = |game: &Game| -> Vec<u32> {
        ranked
            .iter()
            .copied()
            .filter(|cid| head(game, *cid) == Some(building("university")))
            .collect()
    };

    let mut off = game.clone();
    let mut ai = seat(false);
    assert!(ai.threatened_war_holds_the_claims_holds(&off, 0, &plan));
    ai.run_idle_queue_claims(&mut off, 0, &plan);
    assert!(
        universities(&off).is_empty(),
        "off: the hold claims nothing"
    );

    let mut on = game.clone();
    let mut ai = seat(true);
    ai.run_idle_queue_claims(&mut on, 0, &plan);
    assert_eq!(
        universities(&on),
        vec![ranked[1]],
        "the quickest rear city, one claim a turn"
    );
    assert_eq!(
        head(&on, ranked[0]),
        Some(bank()),
        "the front keeps the hold"
    );
}
