use super::*;
use crate::ai::advanced::GrandStrategy;
use crate::game::Item;

fn board() -> (Game, u32, StrategicPlan) {
    let mut g = Game::new_full(2, 24, 16, crate::rng::fixture_seed("CULTURERESERVE", 91_821), 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    for unit in g.player_unit_ids(1) {
        g.remove_unit(unit);
    }
    for position in g.nbrs(g.cities[&cid].pos) {
        let tile = g.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
    }
    g.players[0].civics.insert(crate::name!("drama_poetry"));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 5.0;
    let site = g.nbrs(g.cities[&cid].pos)[0];
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    let city = g.cities.get_mut(&cid).unwrap();
    city.pop = 7;
    city.queue.clear();
    city.districts.insert(crate::name!("campus"), site);
    std::sync::Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        crate::rules::Yields {
            culture: 100.0,
            ..Default::default()
        },
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, cid, plan)
}

fn queued_theater(g: &Game, cid: u32) -> bool {
    matches!(g.cities[&cid].queue.first(), Some(Item::District { district, .. })
        if g.district_family(*district) == "theater_square")
}

/// The reservation claims the idle Campus city while Culture trails, and
/// leaves it alone while the gene is off, the empire is in economic recovery,
/// the city has no Campus yet, or the city is due a Settler.
#[test]
fn a_trailing_empire_reserves_a_theater_ahead_of_the_delegated_governor() {
    let (mut g, cid, plan) = board();
    let mut ai = AdvancedAi::new();
    let mut off = g.clone();
    ai.reserve_culture_defense_theater(&mut off, 0, &plan);
    assert!(off.cities[&cid].queue.is_empty(), "off by default");

    ai.enable_culture_defense_theater();
    let mut broke = g.clone();
    broke.players[0].gold = 10.0;
    broke.players[0].gold_per_turn = -6.0;
    ai.reserve_culture_defense_theater(&mut broke, 0, &plan);
    assert!(broke.cities[&cid].queue.is_empty(), "economic recovery holds it");

    let mut due = g.clone();
    ai.reserve_culture_defense_theater(&mut due, 0, &plan);
    assert!(due.cities[&cid].queue.is_empty(), "a city due a Settler keeps it");
    // At its city target the empire sends no Settler.
    ai.base.w.city_target = 1.0;

    let mut no_campus = g.clone();
    no_campus.cities.get_mut(&cid).unwrap().districts.remove(&crate::name!("campus"));
    ai.reserve_culture_defense_theater(&mut no_campus, 0, &plan);
    assert!(no_campus.cities[&cid].queue.is_empty(), "never the first district");

    ai.reserve_culture_defense_theater(&mut g, 0, &plan);
    assert!(queued_theater(&g, cid), "queue: {:?}", g.cities[&cid].queue);
}

/// Version 2: a city at its housing whose next housing is a Granary is left
/// to the governor's housing reserve; with the Granary standing, or under
/// version 1, the reservation claims it.
#[test]
fn version_two_leaves_a_housing_bound_city_its_granary() {
    let (mut g, cid, plan) = board();
    g.players[0].techs.insert(crate::name!("pottery"));
    // The board's pop 7 already stands at or above its housing.
    assert!(g.cities[&cid].pop as f64 + 1.0 >= g.city_housing(&g.cities[&cid]));
    assert!(matches!(
        crate::ai::BasicAi::housing_reserve_item(&g, 0, cid),
        Some(Item::Building { building }) if building == "granary"
    ));
    let mut ai = AdvancedAi::new();
    ai.base.w.city_target = 1.0;

    ai.enable_culture_defense_theater();
    let mut v1 = g.clone();
    ai.reserve_culture_defense_theater(&mut v1, 0, &plan);
    assert!(queued_theater(&v1, cid), "version 1 claims it: {:?}", v1.cities[&cid].queue);

    ai.disable_culture_defense_theater();
    ai.enable_culture_defense_theater_2();
    assert!(ai.base.culture_defense_theater, "version 2 keeps the governor's step");
    let mut v2 = g.clone();
    ai.reserve_culture_defense_theater(&mut v2, 0, &plan);
    assert!(v2.cities[&cid].queue.is_empty(), "the Granary first: {:?}", v2.cities[&cid].queue);

    g.cities.get_mut(&cid).unwrap().buildings.push(crate::name!("granary"));
    ai.reserve_culture_defense_theater(&mut g, 0, &plan);
    assert!(queued_theater(&g, cid), "with its Granary it is claimed: {:?}", g.cities[&cid].queue);

    ai.disable_culture_defense_theater_2();
    assert!(!ai.base.culture_defense_theater);
}
