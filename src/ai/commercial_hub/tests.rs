use super::*;
use crate::game::{install_test_district, Action};
use crate::name::Name;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

/// The live Emperor pin's delegated-governor steps (monument-first,
/// campus-before-the-army-2, builder-before-the-army-3,
/// granary-before-the-army-2, first-granary-reserve-3, industrial-hub and
/// housing-bound-city-builds-its-granary), a lent war floor of two units a
/// city over the genome's one, and the city target met.
pub(crate) fn emperor_governor(gene: bool) -> BasicAi {
    let mut ai = BasicAi::new();
    ai.monument_first = true;
    ai.campus_before_the_army_2 = true;
    ai.builder_before_the_army_3 = true;
    ai.granary_before_the_army_2 = true;
    ai.housing_reserve = true;
    ai.industrial_hub = true;
    ai.housing_bound_city_builds_its_granary = true;
    ai.w.mil_per_city = 2.0;
    ai.lent_military_floor_base = Some(1.0);
    ai.w.city_target = 6.0;
    ai.commercial_hub_and_traders = gene;
    ai
}

/// An Emperor-shaped empire at turn 70 Online: six cities of population
/// four, each with a Monument, a Granary, a Campus and its Library, room to
/// grow and no Amenity shortfall; Currency, Foreign Trade and one route
/// slot; no Commercial Hub, no army, no route. The cities come back from the
/// most productive down.
pub(crate) fn emperor_empire(tag: &str, seed: u64) -> (Game, Vec<u32>) {
    let mut game = Game::new_full(
        1,
        44,
        26,
        crate::rng::fixture_seed(tag, seed),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    while game.player_city_ids(0).len() < 6 {
        let pos = game
            .map
            .tiles
            .values()
            .filter(|tile| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && tile.district.is_none()
                    && tile.wonder.is_none()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, tile.pos) >= 5)
                    && game
                        .cities
                        .values()
                        .any(|city| game.wdist(city.pos, tile.pos) <= 8)
            })
            .map(|tile| tile.pos)
            .min()
            .expect("room for six cities");
        game.found_city_for(0, pos, None);
    }
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    game.game_speed = GameSpeed::Online;
    game.turn = 70;
    game.difficulty = "emperor".to_string();
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    for tech in [
        "pottery",
        "writing",
        "mining",
        "currency",
        "bronze_working",
        "animal_husbandry",
        "irrigation",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for civic in ["code_of_laws", "foreign_trade"] {
        game.players[0].civics.insert(Name::new(civic));
    }
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 1);
    let cities = game.player_city_ids(0);
    for (rank, cid) in cities.iter().enumerate() {
        let city = game.cities.get_mut(cid).unwrap();
        city.pop = 4;
        for building in ["monument", "granary", "library"] {
            city.buildings.push(Name::new(building));
        }
        install_test_district(&mut game, *cid, "campus");
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                production: 40.0 - 6.0 * rank as f64,
                ..Default::default()
            },
        );
        Arc::make_mut(&mut game.observed_city_amenity_adjustments).insert(*cid, 3);
    }
    for cid in &cities {
        let housing = game.city_housing(&game.cities[cid]);
        Arc::make_mut(&mut game.observed_city_housing_adjustments).insert(*cid, 8.0 - housing);
    }
    let mut ranked = cities;
    ranked.sort_by(|a, b| {
        game.city_yields(*b)
            .production
            .total_cmp(&game.city_yields(*a).production)
            .then(a.cmp(b))
    });
    (game, ranked)
}

/// The governor's pick for `cid` in the six-city empire: no Settler, six
/// Builders, `traders` Traders, no army.
fn pick(ai: &BasicAi, game: &Game, cid: u32, traders: usize) -> Option<Item> {
    ai.pick_item(game, 0, cid, 6, 0, 6, traders, 0, 0, 0, 0)
}

fn is_hub(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "commercial_hub")
}

fn trader() -> Option<Item> {
    Some(Item::Unit {
        unit: crate::name!("trader"),
    })
}

/// See `BasicAi::commercial_hub_and_traders`: on the Emperor shape the stock
/// governor spends every city on the lent army; under the gene the three
/// most productive cities open a Commercial Hub on their best Gold site and
/// the rest keep the stock pick. Before turn 60 Online, the stock pick.
#[test]
fn the_best_unthreatened_cities_open_a_commercial_hub_under_the_gene() {
    let (game, ranked) = emperor_empire("COMMERCIALHUB", 92_001);
    assert!(
        !emperor_governor(false).settler_due(&game, 0, ranked[0], 6, 0),
        "fixture: the city target is met"
    );
    for (rank, cid) in ranked.iter().enumerate() {
        let stock = pick(&emperor_governor(false), &game, *cid, 1);
        assert!(
            stock.is_some(),
            "rank {rank}: the stock governor builds something"
        );
        assert!(
            !is_hub(&game, &stock),
            "rank {rank}: the stock pick is no hub: {stock:?}"
        );
        let gene = pick(&emperor_governor(true), &game, *cid, 1);
        if rank < COMMERCIAL_HUB_MAX {
            let Some(Item::District { pos, .. }) = gene else {
                panic!("rank {rank}: the hub opens: {gene:?}");
            };
            assert!(is_hub(&game, &gene), "rank {rank}: {gene:?}");
            let hub = BasicAi::civ_district(&game, 0, "commercial_hub");
            let best_gold = game
                .district_sites(*cid, hub)
                .into_iter()
                .map(|site| game.district_yields(hub, site).gold)
                .fold(f64::MIN, f64::max);
            assert_eq!(
                game.district_yields(hub, pos).gold,
                best_gold,
                "rank {rank}: the best Gold site"
            );
        } else {
            assert_eq!(gene, stock, "rank {rank}: the stock pick");
        }
    }
    let mut early = game.clone();
    early.turn = 55;
    assert_eq!(
        pick(&emperor_governor(true), &early, ranked[0], 1),
        pick(&emperor_governor(false), &early, ranked[0], 1),
        "turn 55 Online: the stock pick"
    );
    // Five cities want two hubs.
    let five = |gene: bool, cid: u32| {
        let mut ai = emperor_governor(gene);
        ai.w.city_target = 5.0;
        ai.pick_item(&game, 0, cid, 5, 0, 6, 1, 0, 0, 0, 0)
    };
    assert!(
        is_hub(&game, &five(true, ranked[1])),
        "{:?}",
        five(true, ranked[1])
    );
    assert_eq!(
        five(true, ranked[2]),
        five(false, ranked[2]),
        "five cities: the third city keeps the stock pick"
    );
}

/// A standing hub takes its Market ahead of the army, and three hubs held
/// or queued close the step to the other cities.
#[test]
fn a_hub_builds_its_market_and_the_step_stops_at_three_hubs() {
    let (mut game, ranked) = emperor_empire("COMMERCIALMARKET", 92_003);
    for cid in &ranked[..3] {
        install_test_district(&mut game, *cid, "commercial_hub");
    }
    let market = Some(Item::Building {
        building: crate::name!("market"),
    });
    for cid in &ranked[..3] {
        assert_ne!(pick(&emperor_governor(false), &game, *cid, 1), market);
        assert_eq!(pick(&emperor_governor(true), &game, *cid, 1), market);
    }
    for cid in &ranked[3..] {
        assert_eq!(
            pick(&emperor_governor(true), &game, *cid, 1),
            pick(&emperor_governor(false), &game, *cid, 1),
            "three hubs stand"
        );
    }
    // A hub queued counts as held.
    let (mut queued, ranked) = emperor_empire("COMMERCIALMARKET", 92_003);
    for cid in &ranked[..3] {
        let hub = BasicAi::civ_district(&queued, 0, "commercial_hub");
        let pos = queued.district_sites(*cid, hub)[0];
        queued
            .cities
            .get_mut(cid)
            .unwrap()
            .queue
            .push(Item::District { district: hub, pos });
    }
    assert_eq!(
        pick(&emperor_governor(true), &queued, ranked[3], 1),
        pick(&emperor_governor(false), &queued, ranked[3], 1),
        "three hubs queued"
    );
}

/// The plan's threatened city and a city attacked within four turns keep
/// the stock (defence) pick, and the next city takes the hub instead.
#[test]
fn a_threatened_city_keeps_its_defence_pick_under_the_gene() {
    let (game, ranked) = emperor_empire("COMMERCIALTHREAT", 92_005);
    let top = ranked[0];
    let lent = |gene: bool| {
        let mut ai = emperor_governor(gene);
        ai.plan_threatened_city = Some(top);
        ai
    };
    let stock = pick(&lent(false), &game, top, 1);
    assert!(stock.is_some());
    assert_eq!(
        pick(&lent(true), &game, top, 1),
        stock,
        "the plan's threatened city"
    );
    assert!(
        !is_hub(&game, &pick(&emperor_governor(true), &game, ranked[3], 1)),
        "fixture: the fourth city keeps the stock pick unthreatened"
    );
    assert!(
        is_hub(&game, &pick(&lent(true), &game, ranked[3], 1)),
        "the fourth city takes the threatened city's hub"
    );

    let mut attacked = game.clone();
    attacked.cities.get_mut(&top).unwrap().last_attacked = 68;
    assert_eq!(
        pick(&emperor_governor(true), &attacked, top, 1),
        pick(&emperor_governor(false), &attacked, top, 1),
        "attacked two turns ago"
    );
    attacked.cities.get_mut(&top).unwrap().last_attacked = 60;
    assert!(
        is_hub(&game, &pick(&emperor_governor(true), &attacked, top, 1)),
        "attacked ten turns ago"
    );
}

/// A housing-bound city builds its Granary first: the housing gene's step
/// stands ahead, and alone this step yields to the Granary.
#[test]
fn a_housing_bound_city_still_builds_its_granary_first() {
    let (mut game, ranked) = emperor_empire("COMMERCIALGRANARY", 92_007);
    let top = ranked[0];
    game.cities
        .get_mut(&top)
        .unwrap()
        .buildings
        .retain(|building| *building != "granary");
    let housing = game.city_housing(&game.cities[&top]);
    *Arc::make_mut(&mut game.observed_city_housing_adjustments)
        .get_mut(&top)
        .unwrap() += game.cities[&top].pop as f64 - housing;
    assert_eq!(
        game.city_housing(&game.cities[&top]),
        game.cities[&top].pop as f64,
        "fixture: housing-bound"
    );
    let granary = Some(Item::Building {
        building: crate::name!("granary"),
    });
    assert_eq!(pick(&emperor_governor(true), &game, top, 1), granary);
    let mut alone = emperor_governor(true);
    alone.housing_bound_city_builds_its_granary = false;
    alone.granary_before_the_army_2 = false;
    alone.housing_reserve = false;
    assert!(
        !is_hub(&game, &pick(&alone, &game, top, 1)),
        "without the housing steps the hub still waits for the Granary"
    );
    assert!(
        BasicAi::commercial_hub_step(&alone, &game, 0, top, 6, 1).is_none(),
        "the step yields"
    );
}

/// An open route slot takes a Trader ahead of the army in any city that can
/// start a route; a full slot leaves the stock pick.
#[test]
fn a_trader_takes_an_open_route_slot_under_the_gene() {
    let (game, ranked) = emperor_empire("COMMERCIALTRADER", 92_009);
    assert_eq!(game.trade_capacity(0), 1, "fixture: one route slot");
    assert_eq!(game.active_routes(0), 0);
    for cid in [ranked[0], ranked[5]] {
        let stock = pick(&emperor_governor(false), &game, cid, 0);
        assert_ne!(stock, trader(), "the stock pick is the army: {stock:?}");
        assert_eq!(pick(&emperor_governor(true), &game, cid, 0), trader());
    }
    assert_eq!(
        pick(&emperor_governor(true), &game, ranked[5], 1),
        pick(&emperor_governor(false), &game, ranked[5], 1),
        "the slot is taken"
    );
    let mut wider = game.clone();
    Arc::make_mut(&mut wider.observed_trade_capacity).insert(0, 3);
    assert_eq!(
        pick(&emperor_governor(true), &wider, ranked[5], 2),
        trader(),
        "a Trader per open slot"
    );
}

/// A due Settler comes first: with the city target above the empire, the
/// gene's pick is the stock pick.
#[test]
fn a_due_settler_comes_before_the_commercial_step() {
    let (game, ranked) = emperor_empire("COMMERCIALSETTLER", 92_011);
    let roomy = |gene: bool| {
        let mut ai = emperor_governor(gene);
        ai.w.city_target = 10.0;
        ai
    };
    assert!(
        roomy(false).settler_due(&game, 0, ranked[0], 6, 0),
        "fixture: a Settler is due"
    );
    assert!(
        !is_hub(&game, &pick(&roomy(true), &game, ranked[0], 1)),
        "the hub waits"
    );
    assert_eq!(
        pick(&roomy(true), &game, ranked[0], 1),
        pick(&roomy(false), &game, ranked[0], 1)
    );
}
