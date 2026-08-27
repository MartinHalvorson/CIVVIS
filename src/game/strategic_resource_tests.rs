use super::*;

fn strategic_game() -> Game {
    let mut game = Game::new_full(2, 24, 16, 912_447, 120, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.players[pid].gold = 500.0;
        let techs = game.rules.techs.keys().cloned().collect();
        game.players[pid].techs = techs;
        for city in game.player_city_ids(pid) {
            for position in game.cities[&city].owned_tiles.clone() {
                let tile = game.map.tiles.get_mut(&position).unwrap();
                tile.resource = None;
                tile.improvement = None;
                tile.pillaged = false;
            }
        }
    }
    game.record_contact(0, 1);
    game
}

fn resource_tile(game: &Game, pid: usize) -> Pos {
    game.player_city_ids(pid)
        .into_iter()
        .flat_map(|city| game.cities[&city].owned_tiles.clone())
        .find(|position| game.city_at(*position).is_none())
        .unwrap()
}

fn set_resource_tile(
    game: &mut Game,
    position: Pos,
    resource: &str,
    improvement: &str,
    pillaged: bool,
) {
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.terrain = crate::name!("coast");
    tile.feature = None;
    tile.hills = false;
    tile.resource = Some(Name::new(resource));
    tile.improvement = Some(Name::new(improvement));
    tile.pillaged = pillaged;
}

#[test]
fn improved_sources_use_gathering_storm_rates_and_buildings_raise_capacity() {
    let mut game = strategic_game();
    let tile = resource_tile(&game, 0);
    for (resource, improvement, expected) in [
        ("horses", "pasture", 2.0),
        ("iron", "mine", 2.0),
        ("niter", "mine", 2.0),
        ("coal", "mine", 3.0),
        ("oil", "oil_well", 3.0),
        ("aluminum", "mine", 2.0),
        ("uranium", "mine", 3.0),
    ] {
        let owned = game.map.tiles.get_mut(&tile).unwrap();
        owned.resource = Some(Name::new(resource));
        owned.improvement = Some(Name::new(improvement));
        assert_eq!(game.strategic_resource_rate(0, resource), expected);
    }

    let owned = game.map.tiles.get_mut(&tile).unwrap();
    owned.resource = Some(crate::name!("iron"));
    owned.improvement = Some(crate::name!("mine"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 49.0);
    game.process_strategic_resources(0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 50.0);
    assert_eq!(game.strategic_stockpile_capacity(0), 50.0);

    let city = game.player_city_ids(0)[0];
    install_test_district(&mut game, city, "encampment");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("barracks"));
    assert_eq!(game.strategic_stockpile_capacity(0), 60.0);
    game.process_strategic_resources(0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 52.0);

    game.players[0]
        .policies
        .insert(crate::name!("equestrian_orders"));
    assert_eq!(game.strategic_resource_rate(0, "iron"), 3.0);
}

#[test]
fn alternate_water_resources_connect_and_repair_without_builder_charges() {
    let mut game = strategic_game();
    let position = resource_tile(&game, 0);
    let city = game.map.tiles[&position].owner_city.unwrap();

    set_resource_tile(&mut game, position, "oil", "offshore_oil_rig", false);
    assert!(game.rules.improvements["offshore_oil_rig"]
        .resources
        .contains(&crate::name!("oil")));
    assert_eq!(game.connected_resource_count(0, "oil"), 1);
    assert_eq!(game.connected_resource_census(0).get("oil"), Some(&1));
    assert_eq!(
        game.city_connected_strategic_resources(&game.cities[&city]),
        1
    );
    assert_eq!(game.strategic_resource_rate(0, "oil"), 3.0);

    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert_eq!(game.connected_resource_count(0, "oil"), 0);
    assert_eq!(game.connected_resource_census(0).get("oil"), None);
    assert_eq!(
        game.city_connected_strategic_resources(&game.cities[&city]),
        0
    );
    assert_eq!(game.strategic_resource_rate(0, "oil"), 0.0);

    let builder = game.spawn_test_unit("builder", 0, position);
    let charges = game.units[&builder].charges;
    game.apply(0, &Action::RepairImprovement { unit: builder })
        .unwrap();
    assert_eq!(game.units[&builder].charges, charges);
    assert_eq!(game.connected_resource_count(0, "oil"), 1);
    assert_eq!(game.connected_resource_census(0).get("oil"), Some(&1));
    assert_eq!(game.strategic_resource_rate(0, "oil"), 3.0);

    set_resource_tile(&mut game, position, "amber", "fishing_boats", false);
    assert!(game.rules.improvements["fishing_boats"]
        .resources
        .contains(&crate::name!("amber")));
    assert_eq!(game.connected_resource_count(0, "amber"), 1);
    assert_eq!(game.connected_resource_census(0).get("amber"), Some(&1));
    assert_eq!(game.empire_luxuries(0), 1);

    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    game.units.get_mut(&builder).unwrap().moves_left = 2.0;
    assert_eq!(game.connected_resource_count(0, "amber"), 0);
    assert_eq!(game.empire_luxuries(0), 0);
    game.apply(0, &Action::RepairImprovement { unit: builder })
        .unwrap();
    assert_eq!(game.units[&builder].charges, charges);
    assert_eq!(game.connected_resource_count(0, "amber"), 1);
    assert_eq!(game.connected_resource_census(0).get("amber"), Some(&1));
    assert_eq!(game.empire_luxuries(0), 1);
}

#[test]
fn shared_resource_connection_contract_preserves_defaults_and_rejects_mismatches() {
    let mut game = strategic_game();
    let position = resource_tile(&game, 0);
    let city = game.map.tiles[&position].owner_city.unwrap();

    // Every explicit improvement-resource row and every stock default is
    // accepted by the same predicate the accounting endpoints now use.
    for (improvement, spec) in &game.rules.improvements {
        for resource in &spec.resources {
            assert!(
                game.improvement_connects_resource(*improvement, *resource),
                "{} must connect its listed {} resource",
                improvement,
                resource
            );
        }
    }
    for (resource, spec) in &game.rules.resources {
        if !spec.improvement.is_empty() {
            assert!(
                game.improvement_connects_resource(Name::new(&spec.improvement), *resource),
                "{} must retain its stock {} connection",
                resource,
                spec.improvement
            );
        }
    }
    assert!(game.improvement_connects_resource(crate::name!("industry"), crate::name!("amber")));
    assert!(game.improvement_connects_resource(crate::name!("corporation"), crate::name!("amber")));
    assert!(!game.improvement_connects_resource(crate::name!("farm"), crate::name!("amber")));

    set_resource_tile(&mut game, position, "amber", "mine", false);
    assert_eq!(game.connected_resource_count(0, "amber"), 1);
    assert_eq!(game.connected_resource_census(0).get("amber"), Some(&1));
    set_resource_tile(&mut game, position, "amber", "farm", false);
    assert_eq!(game.connected_resource_count(0, "amber"), 0);
    assert_eq!(game.connected_resource_census(0).get("amber"), None);

    // The Grand Bazaar's per-city Luxury effect follows the same live
    // connection, including alternate improvements and pillage state.
    install_test_district(&mut game, city, "commercial_hub");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("market"), crate::name!("grand_bazaar")]);
    let mismatched = game.city_local_amenities_uncached(&game.cities[&city]);
    set_resource_tile(&mut game, position, "amber", "fishing_boats", false);
    assert_eq!(
        game.city_local_amenities_uncached(&game.cities[&city]),
        mismatched + 1
    );
    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert_eq!(
        game.city_local_amenities_uncached(&game.cities[&city]),
        mismatched
    );

    // The same building's strategic accumulation recognizes the water
    // Oil source, rather than silently paying only land Oil Wells.
    set_resource_tile(&mut game, position, "oil", "offshore_oil_rig", false);
    assert_eq!(game.strategic_resource_rate(0, "oil"), 4.0);
    game.players[0]
        .policies
        .insert(crate::name!("resource_management"));
    assert_eq!(game.strategic_resource_rate(0, "oil"), 5.0);
    game.players[0].government = Some("corporate_libertarianism".to_string());
    assert_eq!(game.strategic_resource_rate(0, "oil"), 6.0);
}

#[test]
fn china_trains_both_the_crouching_tiger_and_the_crossbowman() {
    // UnitReplaces carries no row for UNIT_CHINESE_CROUCHING_TIGER, so it
    // is an ADDITIONAL unique rather than a replacement -- the same shape
    // as Sumeria's War Cart, Egypt's Maryannu and Scythia's Saka Horse
    // Archer, all of which CIVVIS already had right. The two units are a
    // real choice: the Tiger hits for 50 at range 1 and costs 140, the
    // Crossbowman for 40 at range 2 and costs 180.
    let mut game = strategic_game();
    game.players[0].civ = "China".to_string();
    // Both units go obsolete at Advanced Ballistics, and the helper grants
    // every technology, so research back down to just Machinery.
    game.players[0].techs.clear();
    game.players[0].techs.insert(crate::name!("machinery"));
    let city = game.player_city_ids(0)[0];
    let unit = |name: &str| Item::Unit {
        unit: Name::new(name),
    };
    assert!(game.can_produce(0, city, &unit("crouching_tiger")));
    assert!(
        game.can_produce(0, city, &unit("crossbowman")),
        "the Crouching Tiger does not take the Crossbowman away"
    );

    // A civilization that really does replace its base unit still cannot
    // train it, so the suppression rule itself is intact.
    game.players[0].civ = "Rome".to_string();
    game.players[0].techs.clear();
    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    assert!(game.can_produce(0, city, &unit("legion")));
    assert!(!game.can_produce(0, city, &unit("swordsman")));

    // The two ranged units differ where the shipped columns say they do.
    let tiger = &game.rules.units["crouching_tiger"];
    let crossbow = &game.rules.units["crossbowman"];
    assert_eq!(
        (tiger.ranged_strength, tiger.range, tiger.cost),
        (50.0, 1, 140.0)
    );
    assert_eq!(
        (crossbow.ranged_strength, crossbow.range, crossbow.cost),
        (40.0, 2, 180.0)
    );
}

#[test]
fn unit_build_commits_material_once_even_when_production_is_paused() {
    let mut game = strategic_game();
    // Rome replaces Swordsmen with Legions; use a civilization without a
    // unique melee replacement so this scenario isolates resource payment.
    game.players[0].civ = "Egypt".to_string();
    game.players[0].techs.clear();
    game.players[0].techs.insert(crate::name!("iron_working"));
    let city = game.player_city_ids(0)[0];
    let swordsman = Item::Unit {
        unit: crate::name!("swordsman"),
    };
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 19.0);
    assert!(!game.can_produce(0, city, &swordsman));

    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 20.0);
    game.do_produce(0, city, &swordsman).unwrap();
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 0.0);
    assert!(game.unit_resource_is_committed(city, &swordsman));

    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    game.do_produce(0, city, &builder).unwrap();
    assert!(game.can_produce(0, city, &swordsman));
    game.do_produce(0, city, &swordsman).unwrap();
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 0.0);
    assert!(game.complete_item(0, city, &swordsman));
    assert!(!game.unit_resource_is_committed(city, &swordsman));
    assert!(game
        .player_unit_ids(0)
        .into_iter()
        .any(|unit| game.units[&unit].kind == "swordsman"));
}

#[test]
fn researching_the_retiring_technology_clears_the_order_and_refunds_material() {
    let mut game = strategic_game();
    game.players[0].civ = "Egypt".to_string();
    // ⚠ `apprenticeship` belongs on this list: it unlocks the Man-At-Arms, and a
    // unit leaves the menu as soon as its upgrade is available. Without removing
    // it the Swordsman is already retired and this test cannot reach the
    // MandatoryObsoleteTech it is actually about.
    for tech in ["gunpowder", "replaceable_parts", "apprenticeship"] {
        game.players[0].techs.remove(&Name::new(tech));
    }
    let city = game.player_city_ids(0)[0];
    let swordsman = Item::Unit {
        unit: crate::name!("swordsman"),
    };
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 20.0);
    game.do_produce(0, city, &swordsman).unwrap();
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 0.0);
    game.cities.get_mut(&city).unwrap().production = 30.0;

    game.players[0]
        .techs
        .insert(crate::name!("replaceable_parts"));
    game.drop_obsolete_production(0);
    assert!(game.cities[&city].queue.is_empty());
    assert_eq!(game.cities[&city].production, 0.0);
    // Iron committed to an order the ruleset just cancelled comes back.
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 20.0);
}

#[test]
fn unpaid_fuel_applies_one_shared_strength_penalty_per_unfed_unit() {
    let mut game = strategic_game();
    let city = game.player_city_ids(0)[0];
    let pos = game.cities[&city].pos;
    let infantry: Vec<u32> = (0..3)
        .map(|_| game.spawn_unit("infantry", 0, pos))
        .collect();
    game.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 1.0);

    game.process_strategic_resources(0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("oil")), 0.0);
    assert_eq!(
        game.players[0].strategic_resource_shortages[&Name::new("oil")],
        2
    );
    assert!(infantry
        .iter()
        .all(|unit| game.unit_unembarked_strength(&game.units[unit]) == 73.0));

    game.process_strategic_resources(0);
    assert_eq!(
        game.players[0].strategic_resource_shortages[&Name::new("oil")],
        3
    );
    assert!(infantry
        .iter()
        .all(|unit| game.unit_unembarked_strength(&game.units[unit]) == 72.0));
}

#[test]
fn strategic_trade_is_an_immediate_permanent_stockpile_transfer() {
    let mut game = strategic_game();
    for player in 0..game.players.len() {
        game.players[player].met.clear();
    }
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 30.0);
    let mut iron = DealItems::default();
    iron.resources.insert("iron".to_string(), 10);
    let payment = DealItems {
        gold: 240.0,
        ..DealItems::default()
    };

    assert!(game.do_trade(0, 1, &iron, &payment).is_err());
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 30.0);
    assert_eq!(game.strategic_stockpile(1, crate::name!("iron")), 0.0);
    game.record_contact(0, 1);
    game.do_trade(0, 1, &iron, &payment).unwrap();
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 20.0);
    assert_eq!(game.strategic_stockpile(1, crate::name!("iron")), 10.0);
    assert!(game.active_trade_deals.is_empty());

    game.turn += STANDARD_DEAL_TURNS + 1;
    game.process_trade_deals(0);
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 20.0);
    assert_eq!(game.strategic_stockpile(1, crate::name!("iron")), 10.0);
}
