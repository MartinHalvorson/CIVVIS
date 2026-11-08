use super::*;
use crate::game::HostMenuEntry;
use std::sync::Arc;

fn digger() -> Item {
    Item::Unit {
        unit: crate::name!("archaeologist"),
    }
}

fn museum_with_a_site() -> (BasicAi, Game, Vec<u32>, Pos) {
    let (mut g, cities) =
        crate::ai::industrial_zones::tests::producer_empire("CULTUREMUSEUMDIGGER", 475_200, 2);
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.players[0].civics = g.rules.civics.keys().copied().collect();
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    for tile in g.map.tiles.values_mut() {
        if matches!(
            tile.resource.as_deref(),
            Some("antiquity_site" | "shipwreck")
        ) {
            tile.resource = None;
        }
    }
    let site = g.cities[&cities[0]]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&cities[0]].pos && g.map.tiles[pos].district.is_none())
        .unwrap();
    let tile = g.map.tiles.get_mut(&site).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.resource = Some(crate::name!("antiquity_site"));
    tile.improvement = None;
    tile.district_foundation = None;
    tile.wonder = None;
    tile.flooded = false;
    tile.submerged = false;
    let mut ai = BasicAi::new();
    ai.digs_one_museum = true;
    ai.exclude_space_race = true;
    assert!(g.players[0].live_great_person_activation_needs.is_empty());
    assert!(g.can_produce(0, cities[0], &digger()));
    (ai, g, cities, site)
}

#[test]
fn the_delegated_culture_picker_hires_a_digger_without_a_waiting_great_person() {
    let (ai, g, cities, _) = museum_with_a_site();
    let city = cities[0];
    assert_eq!(ai.culture_museum_digger(&g, 0, city), Some(digger()));
    assert_eq!(
        ai.pick_item(&g, 0, city, cities.len(), 100, 100, 100, 100, 100, 100, 100),
        Some(digger())
    );
    let mut off = ai;
    off.digs_one_museum = false;
    assert_eq!(off.culture_museum_digger(&g, 0, city), None);
    assert_ne!(
        off.pick_item(&g, 0, city, cities.len(), 100, 100, 100, 100, 100, 100, 100),
        Some(digger())
    );
}

#[test]
fn culture_diggers_leave_wars_and_threatened_cities_to_defense() {
    let (mut ai, mut g, cities, _) = museum_with_a_site();
    let city = cities[0];
    ai.plan_threatened_city = Some(city);
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
    ai.plan_threatened_city = None;
    g.players[1].alive = true;
    g.at_war.insert((0, 1));
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
    g.at_war.clear();
    ai.minor = true;
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
}

#[test]
fn one_active_or_queued_digger_prevents_parallel_training() {
    let (ai, mut g, cities, _) = museum_with_a_site();
    let city = cities[0];
    g.cities.get_mut(&cities[1]).unwrap().queue.push(digger());
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
    g.cities.get_mut(&cities[1]).unwrap().queue.clear();
    let unit = g.spawn_test_unit("archaeologist", 0, g.cities[&city].pos);
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
    g.remove_unit(unit);
    assert_eq!(ai.culture_museum_digger(&g, 0, city), Some(digger()));
}

#[test]
fn the_digger_needs_an_owned_land_site_and_an_operational_empty_museum() {
    let (ai, mut g, cities, site) = museum_with_a_site();
    let city = cities[0];
    g.map.tiles.get_mut(&site).unwrap().owner_city = None;
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
    g.map.tiles.get_mut(&site).unwrap().owner_city = Some(city);
    g.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("archaeological_museum"));
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
    g.cities.get_mut(&city).unwrap().pillaged_buildings.clear();
    g.cities.get_mut(&city).unwrap().hp = 199;
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
    g.cities.get_mut(&city).unwrap().hp = 200;
    g.players[0]
        .counters
        .insert("great_work:artifact".into(), 3);
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
}

#[test]
fn the_native_training_quote_and_remaining_game_bound_the_digger() {
    let (ai, mut g, cities, _) = museum_with_a_site();
    let city = cities[0];
    let quotes = Arc::make_mut(&mut g.host_buildable);
    quotes.entry(city).or_default().insert(
        "unit:archaeologist".into(),
        HostMenuEntry {
            cost: Some(200.0),
            turns: Some(4.0),
        },
    );
    assert_eq!(g.host_production_turns(city, &digger()), Some(4.0));
    assert_eq!(ai.culture_museum_digger(&g, 0, city), Some(digger()));
    Arc::make_mut(&mut g.host_buildable)
        .get_mut(&city)
        .unwrap()
        .get_mut("unit:archaeologist")
        .unwrap()
        .turns = Some(30.0);
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
    Arc::make_mut(&mut g.host_buildable)
        .get_mut(&city)
        .unwrap()
        .get_mut("unit:archaeologist")
        .unwrap()
        .turns = Some(4.0);
    g.max_turns = g.turn + 5;
    assert_eq!(ai.culture_museum_digger(&g, 0, city), None);
}
