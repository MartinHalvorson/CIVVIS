use super::*;
use crate::ai::advanced::early_settler_floor::EARLY_SETTLER_FLOOR_STANDARD_TURN;

/// A founded capital and a second city four to six tiles away, both at
/// population 4 with empty queues and every unit removed: the early city
/// floor's fixture.
fn two_cities() -> (Game, u32, u32) {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("the player opens with a settler");
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let sites: Vec<Pos> = game.map.tiles.keys().copied().collect();
    let mut second = None;
    for site in sites {
        if game.wdist(site, home) < 4 || game.wdist(site, home) > 6 {
            continue;
        }
        let settler = game.spawn_unit("settler", 0, site);
        if game.apply(0, &Action::FoundCity { unit: settler }).is_ok() {
            second = game
                .player_city_ids(0)
                .into_iter()
                .find(|cid| game.cities[cid].pos == site);
            break;
        }
        game.remove_unit(settler);
    }
    let second = second.expect("a legal second site");
    for cid in [capital, second] {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.queue.clear();
    }
    game.turn = 20;
    (game, capital, second)
}

fn granary() -> Item {
    Item::Building {
        building: crate::name!("granary"),
    }
}

/// Put a Granary at the head of `cid` with `turns_left` turns of the
/// city's own Production still to pay.
fn granary_with_turns_left(game: &mut Game, cid: u32, turns_left: f64) {
    let item = granary();
    let cost = game.item_cost_for_city(0, cid, &item);
    let per_turn = game.city_yields(cid).production * game.item_prod_mult(0, cid, Some(&item));
    assert!(per_turn > 0.0, "the fixture city makes Production");
    let city = game.cities.get_mut(&cid).unwrap();
    city.queue = vec![item];
    city.production = (cost - turns_left * per_turn).max(0.5);
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_nearly_built_keeps_its_queue();
    ai
}

fn settlers_queued(game: &Game) -> usize {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            matches!(
                game.cities[cid].queue.first(),
                Some(Item::Unit { unit }) if *unit == "settler"
            )
        })
        .count()
}

/// The predicate: off it never holds; on it holds a building with production
/// in it a few turns from done, and nothing else.
#[test]
fn only_an_invested_build_a_few_turns_from_done_holds() {
    let (mut game, capital, _second) = two_cities();
    let off = AdvancedAi::new();
    let on = armed();
    let limit = f64::from(game.standard_duration(NEARLY_BUILT_STANDARD_TURNS));

    granary_with_turns_left(&mut game, capital, 2.0);
    assert!(!off.nearly_built_holds(&game, 0, capital), "off: never");
    assert!(
        on.nearly_built_holds(&game, 0, capital),
        "two turns left holds"
    );

    granary_with_turns_left(&mut game, capital, limit + 3.0);
    assert!(
        !on.nearly_built_holds(&game, 0, capital),
        "far from done: the claims may take it"
    );

    // Uninvested: nothing paid yet, nothing kept.
    game.cities.get_mut(&capital).unwrap().production = 0.0;
    assert!(!on.nearly_built_holds(&game, 0, capital), "uninvested");

    // A unit head is not this gene's: Settlers, Builders and Traders keep
    // their queues already, and soldiers are what the deterrent wants.
    let city = game.cities.get_mut(&capital).unwrap();
    city.queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    city.production = 5.0;
    assert!(!on.nearly_built_holds(&game, 0, capital), "a unit head");

    // An empty queue holds nothing.
    game.cities.get_mut(&capital).unwrap().queue.clear();
    assert!(!on.nearly_built_holds(&game, 0, capital), "empty queue");
}

/// The early city floor: with a Granary two turns from done at the head of
/// both cities, off the floor's Settler displaces one of them; on, both
/// Granaries keep their queues. A Granary far from done is displaced either
/// way.
#[test]
fn the_early_settler_floor_waits_for_a_nearly_built_granary() {
    let (mut game, capital, second) = two_cities();
    assert!(game.turn < game.standard_duration(EARLY_SETTLER_FLOOR_STANDARD_TURN));
    granary_with_turns_left(&mut game, capital, 2.0);
    granary_with_turns_left(&mut game, second, 2.0);

    let mut floor_only = AdvancedAi::new();
    floor_only.enable_early_settler_floor();
    let mut off = game.clone();
    let plan = floor_only.assess(&off, 0);
    floor_only.claim_early_settler_floor(&mut off, 0, &plan);
    assert_eq!(
        settlers_queued(&off),
        1,
        "off: the floor displaces a Granary"
    );

    let mut both = armed();
    both.enable_early_settler_floor();
    let mut on = game.clone();
    let plan = both.assess(&on, 0);
    both.claim_early_settler_floor(&mut on, 0, &plan);
    assert_eq!(settlers_queued(&on), 0, "on: both Granaries finish first");
    for cid in [capital, second] {
        assert_eq!(on.cities[&cid].queue.first(), Some(&granary()));
    }

    // Far from done, the gene leaves the floor exactly as it was.
    let limit = f64::from(game.standard_duration(NEARLY_BUILT_STANDARD_TURNS));
    let mut far = game.clone();
    granary_with_turns_left(&mut far, capital, limit + 3.0);
    granary_with_turns_left(&mut far, second, limit + 3.0);
    let mut far_off = far.clone();
    let plan = floor_only.assess(&far_off, 0);
    floor_only.claim_early_settler_floor(&mut far_off, 0, &plan);
    let mut far_on = far.clone();
    let plan = both.assess(&far_on, 0);
    both.claim_early_settler_floor(&mut far_on, 0, &plan);
    assert_eq!(settlers_queued(&far_on), settlers_queued(&far_off));
    for cid in [capital, second] {
        assert_eq!(far_on.cities[&cid].queue, far_off.cities[&cid].queue);
    }
}

/// The deterrent's shared spare test keeps a nearly built head; a far one is
/// left to the deterrent's own rules, unchanged.
#[test]
fn the_deterrent_spares_a_nearly_built_head() {
    let (mut game, capital, _second) = two_cities();
    let on = armed();
    let off = AdvancedAi::new();
    granary_with_turns_left(&mut game, capital, 2.0);
    let plan = on.assess(&game, 0);
    assert!(on.deterrence_spares_the_city(&game, 0, capital, &plan));

    let limit = f64::from(game.standard_duration(NEARLY_BUILT_STANDARD_TURNS));
    granary_with_turns_left(&mut game, capital, limit + 3.0);
    let plan_on = on.assess(&game, 0);
    let plan_off = off.assess(&game, 0);
    assert_eq!(
        on.deterrence_spares_the_city(&game, 0, capital, &plan_on),
        off.deterrence_spares_the_city(&game, 0, capital, &plan_off),
        "far from done: the deterrent's own answer"
    );
}
