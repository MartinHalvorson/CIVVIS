use super::*;
use serde_json::json;

fn game_with_city(seed: u64) -> (Game, u32) {
    let mut game = Game::new_full(2, 26, 16, seed, 100, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    (game, city)
}

fn install_selector_modifier(game: &mut Game) {
    // Merged into the imported catalog rather than replacing it: every other
    // ruleset file now attaches bundles by name, and a ruleset missing them
    // fails to build.
    let files = Rules::shipped_values_with(json!({
        "selector_bundle": {
            "building_yields": {
                "library": {"science": 7},
                "university": {"science": 5}
            },
            "unit_purchase_discount_pct": {"warrior": 25},
            "abilities": ["public_engineering"]
        }
    }));
    let mut rules = (*game.rules).clone();
    rules.modifiers = Rules::from_values(files).unwrap().modifiers;
    game.rules = Arc::new(rules);
}

fn add_product_host(game: &mut Game, city: u32) {
    install_test_district(game, city, "commercial_hub");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("stock_exchange"));
    assert_eq!(game.product_capacity(&game.cities[&city]), 3);
}

#[test]
fn arbitrary_building_yields_and_abilities_flow_through_runtime_attachments() {
    let (mut game, city) = game_with_city(86_101);
    install_selector_modifier(&mut game);
    install_test_district(&mut game, city, "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("library"), crate::name!("madrasa")]);
    let baseline = game.city_yields(city).science;

    assert!(!game.has_ability(0, "public_engineering"));
    game.attach_modifier_to_player(0, "selector_bundle").unwrap();
    assert!(game.has_ability(0, "public_engineering"));
    assert_eq!(game.city_yields(city).science - baseline, 12.0);

    game.detach_modifier_from_player(0, "selector_bundle");
    assert!(!game.has_ability(0, "public_engineering"));
    assert_eq!(game.city_yields(city).science, baseline);
}

#[test]
fn per_unit_purchase_discount_is_shared_by_quotes_actions_and_execution() {
    let (mut game, city) = game_with_city(86_102);
    vacate_land_combat_purchase_slot(&mut game, 0, city);
    install_selector_modifier(&mut game);
    let full = game.unit_purchase_cost(0, city, "warrior", "gold").unwrap();
    game.attach_modifier_to_player(0, "selector_bundle").unwrap();
    let discounted = game.unit_purchase_cost(0, city, "warrior", "gold").unwrap();
    assert_eq!(discounted, full * 0.75);

    game.players[0].gold = discounted;
    let action = Action::Buy {
        city,
        unit: crate::name!("warrior"),
        formation: 0,
        currency: "gold".to_string(),
    };
    assert!(game.legal_actions(0).contains(&action));
    game.apply(0, &action).unwrap();
    assert!(game.players[0].gold.abs() < 1e-9);
}

#[test]
fn theocracy_faith_purchase_quote_applies_its_discount_once() {
    let (mut game, city) = game_with_city(86_104);
    vacate_land_combat_purchase_slot(&mut game, 0, city);
    game.players[0].government = Some("theocracy".to_string());
    let base = game.item_cost_for(
        0,
        &Item::Unit {
            unit: crate::name!("warrior"),
        },
    );
    assert_eq!(
        game.unit_purchase_cost(0, city, "warrior", "faith"),
        Some(base * 2.0 * 0.85)
    );
}

#[test]
fn every_product_resource_has_both_a_flat_yield_and_a_real_effect() {
    let rules = Rules::embedded();
    let products: Vec<&str> = rules
        .resources
        .iter()
        .filter(|(_, resource)| resource.product_yields.total() > 0.0)
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(products.len(), 28);
    for product in products {
        let resource = &rules.resources[product];
        let effect = resource.product_effects;
        assert!(
            effect.city_yield_pct.total() > 0.0
                || effect.growth_pct > 0.0
                || effect.military_unit_production_pct > 0.0
                || effect.civilian_unit_production_pct > 0.0
                || effect.building_production_pct > 0.0,
            "{product} has no Product modifier"
        );
    }
    assert_eq!(rules.resources["wine"].product_yields.food, 3.0);
    assert_eq!(rules.resources["wine"].product_effects.city_yield_pct.culture, 20.0);
    assert_eq!(rules.resources["salt"].product_effects.growth_pct, 20.0);
    assert_eq!(rules.resources["salt"].product_effects.housing, 3.0);
    assert_eq!(rules.resources["coffee"].industry_effects.city_yield_pct.culture, 20.0);
    assert_eq!(rules.resources["coffee"].product_effects.city_yield_pct.culture, 30.0);
    assert_eq!(rules.resources["turtles"].product_yields.science, 5.0);
}

#[test]
fn products_and_corporations_apply_all_economic_effect_families() {
    let (mut game, city) = game_with_city(86_103);
    add_product_host(&mut game, city);
    let baseline = game.city_yields(city);
    game.cities.get_mut(&city).unwrap().products = vec!["wine".to_string()];
    let wine = game.city_yields(city);
    assert_eq!(wine.food - baseline.food, 3.0);
    assert!((wine.culture - baseline.culture * 1.20).abs() < 1e-9);

    game.cities.get_mut(&city).unwrap().products = vec!["salt".to_string()];
    let salt_effect = game.city_resource_industry_effects(&game.cities[&city]);
    assert_eq!(salt_effect.growth_pct, 20.0);
    assert_eq!(salt_effect.housing, 3.0);

    game.cities.get_mut(&city).unwrap().products = vec!["citrus".to_string()];
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let military = game.item_prod_mult(0, city, Some(&warrior));
    game.cities.get_mut(&city).unwrap().products = vec!["furs".to_string()];
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let civilian = game.item_prod_mult(0, city, Some(&builder));
    game.cities.get_mut(&city).unwrap().products = vec!["gypsum".to_string()];
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let building = game.item_prod_mult(0, city, Some(&monument));
    assert!(military >= 1.30);
    assert!(civilian >= 1.30);
    assert!(building >= 1.30);

    let position = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.resource = Some(crate::name!("silk"));
        tile.improvement = Some(crate::name!("industry"));
        tile.pillaged = false;
    }
    game.cities.get_mut(&city).unwrap().products.clear();
    assert_eq!(
        game.city_resource_industry_effects(&game.cities[&city])
            .city_yield_pct
            .culture,
        20.0
    );
    game.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("corporation"));
    assert_eq!(
        game.city_resource_industry_effects(&game.cities[&city])
            .city_yield_pct
            .culture,
        40.0
    );
}

// ---------------------------------------------------------------------------
// The imported catalog.
//
// `data/modifiers.json` is generated by `tools/civ6_modifiers.py --emit-catalog`
// from the shipped `Modifiers` tables, and each CIVVIS rules object that the
// game says owns a row carries a `modifiers: ["<bundle>"]` reference to it. The
// loader flattens that reference into the object's ordinary effect map, so the
// tests below are the proof that a database row reaches the consumer that acts
// on it — not that a hand-written number happens to agree with one.

/// Every wired owner family carries the amount its shipped row states.
#[test]
fn the_imported_catalog_reaches_every_owner_family() {
    let rules = Rules::embedded();
    assert!(
        rules.modifiers.len() >= 40,
        "the shipped catalog is empty; rerun --emit-catalog"
    );
    // Civics and technologies: Envoys, Diplomatic Victory Points, embarked
    // Movement and the Tourism multiplier.
    assert_eq!(rules.civics["mysticism"].effects["free_envoys"], 1.0);
    assert_eq!(rules.civics["opera_ballet"].effects["free_envoys"], 2.0);
    assert_eq!(
        rules.civics["cultural_heritage"].effects["free_envoys"],
        3.0
    );
    assert_eq!(
        rules.civics["global_warming_mitigation"].effects["diplomatic_victory_points"],
        1.0
    );
    assert_eq!(
        rules.civics["environmentalism"].effects["tourism_pct"],
        25.0
    );
    assert_eq!(
        rules.techs["seasteads"].effects["diplomatic_victory_points"],
        1.0
    );
    assert_eq!(rules.techs["steam_power"].effects["embarked_movement"], 2.0);
    // COMPUTERS_BOOST_ALL_TOURISM ships +25%, where CIVVIS carried +100%.
    assert_eq!(rules.techs["computers"].effects["tourism_pct"], 25.0);
    // Wonders, districts, buildings, governments and Great People.
    assert_eq!(
        rules.wonders["statue_of_liberty"].effects["diplomatic_victory_points"],
        4.0
    );
    assert_eq!(rules.wonders["kilwa_kisiwani"].effects["envoys"], 3.0);
    assert_eq!(rules.districts["acropolis"].effects["envoys"], 1.0);
    assert_eq!(rules.buildings["airport"].effects["air_slots"], 1.0);
    assert_eq!(rules.buildings["hangar"].effects["air_slots"], 1.0);
    assert_eq!(
        rules.governments["synthetic_technocracy"]
            .effects
            .tourism_pct,
        -10.0
    );
    assert_eq!(rules.great_people["jakob_fugger"].effects["envoys"], 2.0);
    // Unit promotions.
    assert_eq!(rules.promotions["spyglass"].effects["sight"], 1.0);
    assert_eq!(rules.promotions["long_range"].effects["range"], 2.0);
    assert_eq!(rules.promotions["wolfpack"].effects["extra_attacks"], 1.0);
    assert_eq!(
        rules.promotions["flight_deck"].effects["aircraft_slots"],
        1.0
    );
    // MOD_MOVE_AFTER_ATTACKING is attached to Sweeping Wind as well as to
    // Elite Guard and Breakthrough; CIVVIS carried it for only the latter two.
    assert_eq!(
        rules.promotions["sweeping_wind"].effects["move_after_attack"],
        1.0
    );
}

/// Thirteen civics award Envoys in Gathering Storm. CIVVIS carried two of them.
#[test]
fn every_shipped_envoy_civic_pays_on_completion() {
    let (mut game, _) = game_with_city(86_105);
    let awards = [
        ("mysticism", 1),
        ("military_training", 1),
        ("theology", 1),
        ("naval_tradition", 1),
        ("mercenaries", 1),
        ("colonialism", 2),
        ("opera_ballet", 2),
        ("scorched_earth", 2),
        ("natural_history", 2),
        ("conservation", 3),
        ("cultural_heritage", 3),
        ("near_future_governance", 3),
        ("global_warming_mitigation", 3),
    ];
    let mut total = 0;
    for (civic, envoys) in awards {
        let before = game.players[0].envoys_free;
        let first = game.players[0].civics.insert(Name::new(civic));
        game.apply_tree_completion(0, false, civic, first);
        assert_eq!(
            game.players[0].envoys_free - before,
            envoys,
            "{civic} awarded the wrong number of Envoys"
        );
        total += envoys;
        // Completing the same node again is not a second award.
        let repeat = game.players[0].civics.insert(Name::new(civic));
        game.apply_tree_completion(0, false, civic, repeat);
        assert_eq!(game.players[0].envoys_free - before, envoys, "{civic}");
    }
    assert_eq!(total, 25);
}

/// The five Diplomatic Victory Point rows, through the two paths that award
/// them: tree completion and wonder completion.
#[test]
fn the_diplomatic_victory_point_rows_award_their_points() {
    let (mut game, city) = game_with_city(86_106);
    let position = game.cities[&city].pos;
    for (wonder, points) in [
        ("mahabodhi_temple", 2),
        ("potala_palace", 1),
        ("statue_of_liberty", 4),
    ] {
        let spec = game.rules.wonders[wonder].clone();
        let before = game.players[0].dvp;
        game.apply_wonder_completion_effects(0, city, wonder, position, &spec);
        assert_eq!(game.players[0].dvp - before, points, "{wonder}");
    }
    for (node, technology, points) in [
        ("seasteads", true, 1),
        ("global_warming_mitigation", false, 1),
    ] {
        let before = game.players[0].dvp;
        let first = if technology {
            game.players[0].techs.insert(Name::new(node))
        } else {
            game.players[0].civics.insert(Name::new(node))
        };
        game.apply_tree_completion(0, technology, node, first);
        assert_eq!(game.players[0].dvp - before, points, "{node}");
    }
    assert_eq!(game.players[0].dvp, 9);
}

/// Square Rigging, Steam Power and Combustion each add their shipped Movement
/// to an embarked unit, and nothing to the same unit on land.
#[test]
fn the_embarked_movement_rows_reach_an_embarked_unit() {
    let (mut game, _) = game_with_city(86_107);
    let land = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "warrior")
        .expect("a starting warrior");
    let ashore = game.unit_max_moves(land);
    for (tech, expected) in [
        ("square_rigging", 1.0),
        ("steam_power", 3.0),
        ("combustion", 4.0),
    ] {
        game.players[0].techs.insert(Name::new(tech));
        assert_eq!(game.tree_effect(0, "embarked_movement"), expected, "{tech}");
    }
    assert_eq!(
        game.unit_max_moves(land),
        ashore,
        "a land unit ashore pays no embarked term"
    );
}

/// Sight, attack range, extra attacks, movement after an attack and carrier
/// slots each arrive from the promotion row that grants them.
#[test]
fn the_unit_promotion_rows_reach_the_unit() {
    let (mut game, _) = game_with_city(86_108);
    let unit = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "warrior")
        .expect("a starting warrior");
    let sight = game.unit_sight(unit);
    let attacks = game.unit_max_attacks(unit);
    assert_eq!(
        game.promotion_effect(&game.units[&unit], "move_after_attack"),
        0.0
    );

    game.units
        .get_mut(&unit)
        .unwrap()
        .promotions
        .insert(crate::name!("spyglass"));
    assert_eq!(game.unit_sight(unit) - sight, 1);

    game.units
        .get_mut(&unit)
        .unwrap()
        .promotions
        .insert(crate::name!("wolfpack"));
    assert_eq!(game.unit_max_attacks(unit) - attacks, 1);

    game.units
        .get_mut(&unit)
        .unwrap()
        .promotions
        .insert(crate::name!("sweeping_wind"));
    assert_eq!(game.unit_max_attacks(unit) - attacks, 2);
    assert_eq!(
        game.promotion_effect(&game.units[&unit], "move_after_attack"),
        1.0
    );

    game.units
        .get_mut(&unit)
        .unwrap()
        .promotions
        .insert(crate::name!("long_range"));
    assert_eq!(game.promotion_effect(&game.units[&unit], "range"), 2.0);

    game.units
        .get_mut(&unit)
        .unwrap()
        .promotions
        .insert(crate::name!("flight_deck"));
    game.units
        .get_mut(&unit)
        .unwrap()
        .promotions
        .insert(crate::name!("hangar_deck"));
    assert_eq!(
        game.promotion_effect(&game.units[&unit], "aircraft_slots"),
        2.0
    );
}

/// Computers multiplies every Tourism source by the quarter its row states,
/// and Synthetic Technocracy takes its tenth back.
#[test]
fn the_tourism_rows_multiply_the_empire_total() {
    let (mut game, _) = game_with_city(86_109);
    assert_eq!(game.tree_effect(0, "tourism_pct"), 0.0);
    game.players[0].techs.insert(crate::name!("computers"));
    assert_eq!(game.tree_effect(0, "tourism_pct"), 25.0);
    game.players[0]
        .civics
        .insert(crate::name!("environmentalism"));
    assert_eq!(game.tree_effect(0, "tourism_pct"), 50.0);
    game.players[0].government = Some("synthetic_technocracy".to_string());
    assert_eq!(game.gov_effects(0).tourism_pct, -10.0);
}
