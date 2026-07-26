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
    let mut files = Rules::shipped_values();
    files.insert(
        "modifiers".to_string(),
        json!({
            "selector_bundle": {
                "building_yields": {
                    "library": {"science": 7},
                    "university": {"science": 5}
                },
                "unit_purchase_discount_pct": {"warrior": 25},
                "abilities": ["public_engineering"]
            }
        }),
    );
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
        .push("stock_exchange".to_string());
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
        .extend(["library".to_string(), "madrasa".to_string()]);
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
    install_selector_modifier(&mut game);
    let full = game.unit_purchase_cost(0, city, "warrior", "gold").unwrap();
    game.attach_modifier_to_player(0, "selector_bundle").unwrap();
    let discounted = game.unit_purchase_cost(0, city, "warrior", "gold").unwrap();
    assert_eq!(discounted, full * 0.75);

    game.players[0].gold = discounted;
    let action = Action::Buy {
        city,
        unit: "warrior".to_string(),
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
    game.players[0].government = Some("theocracy".to_string());
    let base = game.item_cost_for(
        0,
        &Item::Unit {
            unit: "warrior".to_string(),
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
        unit: "warrior".to_string(),
    };
    let military = game.item_prod_mult(0, city, Some(&warrior));
    game.cities.get_mut(&city).unwrap().products = vec!["furs".to_string()];
    let builder = Item::Unit {
        unit: "builder".to_string(),
    };
    let civilian = game.item_prod_mult(0, city, Some(&builder));
    game.cities.get_mut(&city).unwrap().products = vec!["gypsum".to_string()];
    let monument = Item::Building {
        building: "monument".to_string(),
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
        tile.resource = Some("silk".to_string());
        tile.improvement = Some("industry".to_string());
        tile.pillaged = false;
    }
    game.cities.get_mut(&city).unwrap().products.clear();
    assert_eq!(
        game.city_resource_industry_effects(&game.cities[&city])
            .city_yield_pct
            .culture,
        20.0
    );
    game.map.tiles.get_mut(&position).unwrap().improvement = Some("corporation".to_string());
    assert_eq!(
        game.city_resource_industry_effects(&game.cities[&city])
            .city_yield_pct
            .culture,
        40.0
    );
}
