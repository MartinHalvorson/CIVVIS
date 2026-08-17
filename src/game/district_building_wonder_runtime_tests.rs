use super::*;

fn one_city(seed: u64) -> (Game, u32, Pos) {
    let mut game = Game::new_full(1, 24, 16, seed, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let district = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos)
        .unwrap();
    (game, city, district)
}

fn install_district(game: &mut Game, city: u32, position: Pos, district: &str) {
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.district = Some(Name::new(district));
    tile.pillaged = false;
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(Name::new(district), position);
}

#[test]
fn military_infrastructure_builds_discounted_formations_with_real_strength() {
    let (mut game, city, position) = one_city(774_401);
    game.players[0].civ = "Zulu".to_string();
    install_district(&mut game, city, position, "ikanda");
    game.players[0].civics.insert(crate::name!("mercenaries"));
    game.players[0].civics.insert(crate::name!("nationalism"));

    let corps = Item::Formation {
        unit: crate::name!("warrior"),
        formation: 1,
    };
    let army = Item::Formation {
        unit: crate::name!("warrior"),
        formation: 2,
    };
    assert!(game.can_produce(0, city, &corps));
    assert!(game.can_produce(0, city, &army));
    // UNIT_CORPS_COST_MODIFIER 1.5 and UNIT_ARMY_COST_MODIFIER 2.0 against
    // a Warrior's 40, so 60 and 80 -- not the 90 a compounded Corps rate
    // would charge.
    assert_eq!(game.item_cost(&corps), 60.0);
    assert_eq!(game.item_cost(&army), 80.0);
    // Ikanda alone authorizes direct formations and its 25% discount is
    // an effective +33⅓% Production multiplier.
    assert!((game.item_prod_mult(0, city, Some(&corps)) - 1.3333333333333333).abs() < 1e-9);

    let before: BTreeSet<u32> = game.player_unit_ids(0).into_iter().collect();
    assert!(game.complete_item(0, city, &army));
    let trained = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| !before.contains(unit))
        .unwrap();
    assert_eq!(game.units[&trained].formation, 2);
    assert_eq!(game.units[&trained].production_cost, 80.0);
    assert_eq!(game.unit_formation_bonus(&game.units[&trained]), 22.0);
}

#[test]
fn land_combat_purchase_requires_an_unreserved_city_center_combat_layer() {
    let (mut game, city, _) = one_city(774_405);
    let center = game.cities[&city].pos;
    let open_land = game
        .nbrs(center)
        .into_iter()
        .find(|position| {
            game.map.get(*position).is_some_and(|tile| {
                game.rules.is_passable(tile) && !game.rules.is_water(tile)
            })
        })
        .unwrap();
    for unit in game.units_at(center) {
        game.relocate(unit, open_land);
    }

    // Live Firaxis evidence: Rome offered a Scout purchase with an empty
    // center while a Warrior was completing, but refused it with the
    // explicit same-class placement reason. A Settler queue did not
    // reserve the combat layer and the next military purchase succeeded.
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    assert_eq!(game.unit_purchase_cost(0, city, "scout", "gold"), None);
    game.cities.get_mut(&city).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    assert_eq!(
        game.unit_purchase_cost(0, city, "scout", "gold"),
        Some(120.0)
    );

    game.cities.get_mut(&city).unwrap().queue.clear();
    let blocker = game.spawn_unit("warrior", 0, center);
    assert_eq!(game.unit_purchase_cost(0, city, "scout", "gold"), None);
    game.relocate(blocker, open_land);
    assert_eq!(
        game.unit_purchase_cost(0, city, "scout", "gold"),
        Some(120.0)
    );
}

#[test]
fn formations_can_be_bought_directly_for_full_constituent_cost() {
    let (mut game, city, _) = one_city(774_406);
    vacate_land_combat_purchase_slot(&mut game, 0, city);
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("military_academy"));
    game.players[0].civics.insert(crate::name!("nationalism"));
    game.players[0].civics.insert(crate::name!("mobilization"));

    assert_eq!(
        game.unit_purchase_cost_for_formation(0, city, "warrior", 1, "gold"),
        Some(320.0)
    );
    assert_eq!(
        game.unit_purchase_cost_for_formation(0, city, "warrior", 2, "gold"),
        Some(480.0)
    );
    game.players[0].gold = 320.0;
    let action = Action::Buy {
        city,
        unit: crate::name!("warrior"),
        formation: 1,
        currency: "gold".to_string(),
    };
    assert!(game.legal_actions(0).contains(&action));
    let before: BTreeSet<u32> = game.player_unit_ids(0).into_iter().collect();
    game.apply(0, &action).unwrap();
    let bought = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| !before.contains(unit))
        .unwrap();
    assert_eq!(game.units[&bought].formation, 1);
    assert_eq!(game.units[&bought].production_cost, 60.0);
    assert!(game.players[0].gold.abs() < 1e-9);

    // Firaxis purchases into the City Center's combat layer, so clear the
    // first formation before quoting a second one from this city.
    let center = game.cities[&city].pos;
    let open_land = game
        .nbrs(center)
        .into_iter()
        .find(|position| {
            game.map.get(*position).is_some_and(|tile| {
                game.rules.is_passable(tile) && !game.rules.is_water(tile)
            }) && game.units_at(*position).is_empty()
        })
        .unwrap();
    game.relocate(bought, open_land);

    game.players[0].government = Some("theocracy".to_string());
    assert_eq!(
        game.unit_purchase_cost_for_formation(0, city, "warrior", 2, "faith"),
        Some(204.0)
    );
    game.players[0].faith = 204.0;
    let army = Action::Buy {
        city,
        unit: crate::name!("warrior"),
        formation: 2,
        currency: "faith".to_string(),
    };
    assert!(game.legal_actions(0).contains(&army));
    game.apply(0, &army).unwrap();
    assert!(game.players[0].faith.abs() < 1e-9);
    assert!(game
        .units
        .values()
        .any(|unit| unit.owner == 0 && unit.kind == "warrior" && unit.formation == 2));

    let legacy: Action = serde_json::from_value(serde_json::json!({
        "type": "buy", "city": city, "unit": "warrior", "currency": "gold"
    }))
    .unwrap();
    assert!(matches!(legacy, Action::Buy { formation: 0, .. }));

    game.players[0].is_minor = true;
    assert!(!game.can_produce(
        0,
        city,
        &Item::Formation {
            unit: crate::name!("warrior"),
            formation: 1,
        },
    ));
}

#[test]
fn carriers_train_as_formations_but_never_merge_in_the_field() {
    let (mut game, city, position) = one_city(774_407);
    let coast = game.nbrs(game.cities[&city].pos)[0];
    game.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    install_district(&mut game, city, position, "harbor");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("seaport"));
    game.players[0].techs.insert(crate::name!("combined_arms"));
    game.players[0].civics.insert(crate::name!("nationalism"));
    let fleet = Item::Formation {
        unit: crate::name!("aircraft_carrier"),
        formation: 1,
    };
    assert!(game.rules.units["aircraft_carrier"].can_formations);
    assert!(!game.rules.units["aircraft_carrier"].can_combine);
    assert!(game.can_produce(0, city, &fleet));

    let first = game.spawn_unit("aircraft_carrier", 0, coast);
    let second_coast = game
        .nbrs(coast)
        .into_iter()
        .find(|neighbor| game.map.get(*neighbor).is_some())
        .unwrap();
    game.map.tiles.get_mut(&second_coast).unwrap().terrain = crate::name!("coast");
    let second = game.spawn_unit("aircraft_carrier", 0, second_coast);
    assert_eq!(game.can_combine_units(0, first, second), None);
}

#[test]
fn direct_formations_commit_double_or_triple_strategic_resources() {
    let (mut game, city, position) = one_city(774_405);
    game.players[0].civ = "Egypt".to_string();
    install_district(&mut game, city, position, "encampment");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("military_academy"));
    game.players[0].techs.insert(crate::name!("iron_working"));
    game.players[0].civics.insert(crate::name!("nationalism"));
    game.players[0].civics.insert(crate::name!("mobilization"));

    let corps = Item::Formation {
        unit: crate::name!("swordsman"),
        formation: 1,
    };
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 39.0);
    assert!(!game.can_produce(0, city, &corps));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 40.0);
    game.do_produce(0, city, &corps).unwrap();
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 0.0);
    assert!(game.unit_resource_is_committed(city, &corps));

    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    game.do_produce(0, city, &warrior).unwrap();
    assert!(game.can_produce(0, city, &corps));
    game.do_produce(0, city, &corps).unwrap();
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 0.0);
    assert!(game.complete_item(0, city, &corps));
    assert!(!game.unit_resource_is_committed(city, &corps));

    let army = Item::Formation {
        unit: crate::name!("swordsman"),
        formation: 2,
    };
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 59.0);
    assert!(!game.can_produce(0, city, &army));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 60.0);
    game.do_produce(0, city, &army).unwrap();
    assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 0.0);
    assert!(game.unit_resource_is_committed(city, &army));
}

#[test]
fn royal_society_consumes_all_builder_charges_once_per_city_turn() {
    let (mut game, city, position) = one_city(774_402);
    install_district(&mut game, city, position, "spaceport");
    install_test_district(&mut game, city, "government_plaza");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("royal_society"));
    let project = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };
    game.cities.get_mut(&city).unwrap().queue = vec![project.clone()];
    let builder = game.spawn_unit("builder", 0, position);
    let charges = game.units[&builder].charges;
    assert!(game.can_contribute_project(0, builder, city));
    game.do_contribute_project(0, builder, city).unwrap();
    assert!(!game.units.contains_key(&builder));
    assert_eq!(
        game.cities[&city].production,
        game.item_cost(&project) * 0.02 * charges as f64
    );

    let second = game.spawn_unit("builder", 0, position);
    assert!(!game.can_contribute_project(0, second, city));
    game.turn += 1;
    assert!(game.can_contribute_project(0, second, city));
}

#[test]
fn stonehenge_is_a_holy_site_for_founding_and_alhambra_is_a_fort() {
    let (mut game, city, position) = one_city(774_403);
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("stonehenge"), position);
    game.map.tiles.get_mut(&position).unwrap().wonder = Some(crate::name!("stonehenge"));
    game.players[0].prophet_pending = true;
    let follower = game.rules.beliefs.follower.keys().next().unwrap().clone();
    let founder = game.rules.beliefs.founder.keys().next().unwrap().clone();
    game.do_found_religion(0, &follower, &founder).unwrap();
    assert_eq!(game.players[0].holy_city, Some(city));

    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.wonder = Some(crate::name!("alhambra"));
    tile.hills = false;
    tile.feature = None;
    assert_eq!(game.tile_defense_bonus(position), 4.0);
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.wonder = None;
    tile.improvement = Some(crate::name!("fort"));
    assert_eq!(game.tile_defense_bonus(position), 4.0);
    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert_eq!(game.tile_defense_bonus(position), 0.0);

    let warrior = game.spawn_unit("warrior", 0, game.cities[&city].pos);
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.terrain = crate::name!("mountain");
    tile.improvement = Some(crate::name!("mountain_tunnel"));
    tile.pillaged = false;
    assert!(game.unit_can_traverse(warrior, position));
    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert!(!game.unit_can_traverse(warrior, position));
}

#[test]
fn meteors_pepper_open_land_and_grant_advanced_heavy_cavalry() {
    let (mut game, city, _) = one_city(90_4441);
    // A stock lobby has the Apocalypse mode off and so never sees one.
    game.turn = game.max_turns;
    game.process_meteors();
    assert_eq!(game.meteor_strikes, 0, "meteors are an Apocalypse-mode rule");
    game.game_modes.insert("apocalypse".to_string());
    // At the turn limit every remaining strike is forced in, and the
    // budget never exceeds the shipped six per game.
    for expected in 1..=6u32 {
        game.process_meteors();
        assert_eq!(game.meteor_strikes, expected);
    }
    game.process_meteors();
    assert_eq!(game.meteor_strikes, 6);
    let sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| tile.improvement.as_deref() == Some("meteor_goody"))
        .map(|(position, _)| *position)
        .collect();
    assert_eq!(sites.len(), 6);
    for position in &sites {
        let tile = &game.map.tiles[position];
        assert!(matches!(
            tile.terrain.as_str(),
            "plains" | "grassland" | "snow" | "desert"
        ));
        assert!(tile.owner_city.is_none() && tile.district.is_none());
    }

    // The first unit in pops the meteor's own table: the most advanced
    // Heavy Cavalry the finder can field, in the nearest owned city,
    // exempt from resource upkeep.
    game.players[0].techs.insert(crate::name!("wheel"));
    game.players[0].techs.insert(crate::name!("stirrups"));
    let site = sites[0];
    let scout = game.spawn_unit("scout", 0, site);
    game.maybe_goody_hut(scout);
    assert!(game.map.tiles[&site].improvement.is_none());
    let knight = game
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "knight")
        .expect("the meteor grants the best heavy cavalry, not a chariot");
    assert!(knight.free_upkeep);
    assert!(game.wdist(knight.pos, game.cities[&city].pos) <= 2);

    // The refund modifier is real upkeep relief: an exempt fuel unit
    // stops counting against the oil ledger.
    let tank = game.spawn_unit("tank", 0, game.cities[&city].pos);
    game.process_strategic_resources(0);
    assert!(
        game.players[0]
            .strategic_resource_shortages
            .get(&Name::new("oil"))
            .copied()
            .unwrap_or(0)
            > 0
    );
    game.units.get_mut(&tank).unwrap().free_upkeep = true;
    game.process_strategic_resources(0);
    assert_eq!(
        game.players[0]
            .strategic_resource_shortages
            .get(&Name::new("oil"))
            .copied()
            .unwrap_or(0),
        0
    );
}

#[test]
fn routes_level_per_tile_and_engineers_lay_railroads() {
    let (mut game, _city, _) = one_city(88_2071);
    let (a, b) = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        .find_map(|(position, _)| {
            game.nbrs(*position).into_iter().find_map(|neighbor| {
                let ok = game.map.get(neighbor).is_some_and(|tile| {
                    !game.rules.is_water(tile) && game.rules.is_passable(tile)
                }) && !game.crosses_river(*position, neighbor)
                    && game.units_at(*position).is_empty()
                    && game.units_at(neighbor).is_empty()
                    && game.map.tiles[position].district.is_none()
                    && game.map.tiles[&neighbor].district.is_none();
                ok.then_some((*position, neighbor))
            })
        })
        .expect("test map has an adjacent riverless land pair");
    // Hills on the destination so the base step costs 2 MP and each
    // route level's discount is visible.
    for (position, hills) in [(a, false), (b, true)] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = hills;
        tile.road = 0;
    }
    let warrior = game.spawn_unit("warrior", 0, a);
    assert!((game.unit_step_cost(warrior, a, b) - 2.0).abs() < 1e-9);
    // The shipped ladder, per tile: Ancient/Medieval 1 MP, Industrial
    // 0.75, Modern 0.5, Railroad 0.25.
    for (level, expected) in [(1, 1.0), (2, 1.0), (3, 0.75), (4, 0.5), (5, 0.25)] {
        game.map.tiles.get_mut(&a).unwrap().road = level;
        game.map.tiles.get_mut(&b).unwrap().road = level;
        assert!((game.unit_step_cost(warrior, a, b) - expected).abs() < 1e-9);
    }
    // The destination tile's route sets the step's price.
    game.map.tiles.get_mut(&a).unwrap().road = 1;
    game.map.tiles.get_mut(&b).unwrap().road = 5;
    assert!((game.unit_step_cost(warrior, a, b) - 0.25).abs() < 1e-9);
    game.map.tiles.get_mut(&a).unwrap().road = 5;
    game.map.tiles.get_mut(&b).unwrap().road = 1;
    assert!((game.unit_step_cost(warrior, a, b) - 1.0).abs() < 1e-9);

    // Traders lay the best route their civilization's era allows.
    game.players[0].techs.clear();
    game.players[0].civics.clear();
    assert_eq!(game.road_level_for(0), 1);
    let classical = game
        .rules
        .techs
        .iter()
        .find(|(_, spec)| spec.era == 1)
        .map(|(name, _)| *name)
        .unwrap();
    game.players[0].techs.insert(classical);
    assert_eq!(game.road_level_for(0), 2);
    game.players[0].techs.insert(crate::name!("steam_power"));
    assert_eq!(game.road_level_for(0), 3);
    game.players[0].techs.insert(crate::name!("flight"));
    assert_eq!(game.road_level_for(0), 4);

    // Railroads: engineer-built only, Steam Power, 1 Iron + 1 Coal,
    // no build charge.
    game.map.tiles.get_mut(&b).unwrap().road = 0;
    let engineer = game.spawn_unit("military_engineer", 0, b);
    let charges = game.units[&engineer].charges;
    assert!(!game.can_build_railroad(0, engineer));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 2.0);
    game.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 1.0);
    assert!(game.can_build_railroad(0, engineer));
    game.apply(0, &Action::BuildRailroad { unit: engineer }).unwrap();
    assert_eq!(game.map.tiles[&b].road, 5);
    assert!((game.strategic_stockpile(0, crate::name!("iron")) - 1.0).abs() < 1e-9);
    assert!(game.strategic_stockpile(0, crate::name!("coal")).abs() < 1e-9);
    assert_eq!(game.units[&engineer].charges, charges);
    assert_eq!(game.units[&engineer].moves_left, 0.0);
    assert!(!game.can_build_railroad(0, engineer));
}

#[test]
fn a_bridged_river_crossing_costs_its_route_and_never_returns_movement() {
    let (mut game, _city, _) = one_city(88_2073);
    let (a, b) = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        .find_map(|(position, _)| {
            game.nbrs(*position).into_iter().find_map(|neighbor| {
                let ok = game.map.get(neighbor).is_some_and(|tile| {
                    !game.rules.is_water(tile) && game.rules.is_passable(tile)
                }) && !game.crosses_river(*position, neighbor)
                    && game.units_at(*position).is_empty()
                    && game.units_at(neighbor).is_empty()
                    && game.map.tiles[position].district.is_none()
                    && game.map.tiles[&neighbor].district.is_none();
                ok.then_some((*position, neighbor))
            })
        })
        .expect("test map has an adjacent riverless land pair");
    for position in [a, b] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.road = 2; // Medieval route: SupportsBridges
    }
    assert!(game.map.set_river_edge(a, b, true));
    let warrior = game.spawn_unit("warrior", 0, a);

    // A bridged crossing is charged its route, because bridging withholds
    // the surcharge. It cannot be charged the route *minus* the surcharge:
    // the route ladder has already discarded that, so subtracting it again
    // prices the step below zero.
    for (level, expected) in [(2, 1.0), (3, 0.75), (4, 0.5), (5, 0.25)] {
        game.map.tiles.get_mut(&a).unwrap().road = level;
        game.map.tiles.get_mut(&b).unwrap().road = level;
        let cost = game.unit_step_cost(warrior, a, b);
        assert!(
            (cost - expected).abs() < 1e-9,
            "a level {level} bridge costs {cost} MP, expected {expected}"
        );
    }

    // The price is not why this matters. A step costing less than nothing
    // *returns* movement, so a unit crossing a bridge and back regains MP
    // on every crossing. `flow` and `path_to` terminate only because each
    // relaxation leaves strictly less movement than it started with, so a
    // refund unbounds them from the unit's budget and they relax the whole
    // route network -- which is the memory fault, not a mispriced tile. A
    // part-spent unit is where it shows, since the max-moves cap conceals
    // a refund taken at full movement.
    for level in [2u8, 5] {
        game.map.tiles.get_mut(&a).unwrap().road = level;
        game.map.tiles.get_mut(&b).unwrap().road = level;
        for (position, remaining) in &game.flow(warrior, a, 1.0) {
            assert!(
                *remaining <= 1.0 + 1e-9,
                "level {level}: {position:?} keeps {remaining} MP \
                 after a step that began with 1 MP"
            );
        }
    }
}

#[test]
fn golden_gate_is_a_modern_road_without_embarking_land_units() {
    let (mut game, city, _) = one_city(774_4031);
    let (bridge, left, right) = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            tile.terrain == "coast"
                && tile.owner_city.is_none()
                && game.units_at(**position).is_empty()
        })
        .find_map(|(position, _)| {
            let neighbors = game.nbrs(*position);
            let land: Vec<usize> = neighbors
                .iter()
                .enumerate()
                .filter(|(_, neighbor)| {
                    game.map.get(**neighbor).is_some_and(|tile| {
                        game.rules.is_passable(tile)
                            && !game.rules.is_water(tile)
                            && tile.owner_city.is_none()
                            && game.units_at(**neighbor).is_empty()
                    })
                })
                .map(|(index, _)| index)
                .collect();
            land.iter().find_map(|left| {
                land.iter()
                    .find(|right| (*left as i32 - **right as i32).abs() == 3)
                    .map(|right| (*position, neighbors[*left], neighbors[*right]))
            })
        })
        .expect("test map has an empty Golden Gate geometry");
    game.map.tiles.get_mut(&bridge).unwrap().wonder = Some(crate::name!("golden_gate_bridge"));
    game.map.tiles.get_mut(&bridge).unwrap().owner_city = Some(city);
    let spec = game.rules.wonders["golden_gate_bridge"].clone();
    game.apply_wonder_completion_effects(0, city, "golden_gate_bridge", bridge, &spec);
    assert_eq!(game.map.tiles[&bridge].road, 4);
    assert_eq!(game.map.tiles[&left].road, 4);
    assert_eq!(game.map.tiles[&right].road, 4);

    game.players[0].techs.remove(&Name::new("shipbuilding"));
    for technology in ["mathematics", "square_rigging", "steam_power", "combustion"] {
        game.players[0].techs.insert(Name::new(technology));
    }
    let warrior = game.spawn_unit("warrior", 0, left);
    assert!(game.unit_can_traverse(warrior, bridge));
    assert_eq!(game.unit_step_cost(warrior, left, bridge), 0.5);
    assert!(
        game.can_enter(warrior, left, bridge),
        "a bridge endpoint must admit its owning land unit"
    );
    assert!(
        game.can_move(warrior, bridge),
        "the Golden Gate tile must be a legal single-step move"
    );
    game.apply(
        0,
        &Action::Move {
            unit: warrior,
            to: bridge,
        },
    )
    .unwrap();
    assert!(!game.is_embarked(&game.units[&warrior]));
    assert_eq!(game.unit_strength(&game.units[&warrior], true), 20.0);
    assert_eq!(
        game.unit_base_max_moves(warrior),
        game.rules.units["warrior"].moves,
        "a land unit on the bridge must not receive naval or embarked movement bonuses"
    );
    assert_eq!(game.unit_step_cost(warrior, bridge, right), 0.5);
    game.apply(
        0,
        &Action::Move {
            unit: warrior,
            to: right,
        },
    )
    .unwrap();
    assert_eq!(game.units[&warrior].pos, right);
}

#[test]
fn foreign_ministry_halves_levies_and_buffs_city_state_units() {
    let (mut game, city, position) = one_city(774_404);
    install_district(&mut game, city, position, "government_plaza");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("foreign_ministry"));
    let minor = game.players.len();
    game.players.push(Player::new(minor, "Geneva", true));
    game.players[0].envoys.push((minor, 3));
    let position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    let warrior = game.spawn_unit("warrior", minor, position);
    game.players[0].gold = 100.0;

    assert_eq!(game.levy_cost(0, minor), Some(20.0));
    assert_eq!(game.unit_unembarked_strength(&game.units[&warrior]), 23.0);
    game.do_levy_military(0, minor).unwrap();
    assert_eq!(game.units[&warrior].owner, 0);
    assert_eq!(game.units[&warrior].levied_from, Some(minor));
    assert_eq!(game.players[0].gold, 80.0);
    assert_eq!(game.unit_unembarked_strength(&game.units[&warrior]), 27.0);

    game.turn += STANDARD_DEAL_TURNS;
    game.process_levies(0);
    assert_eq!(game.units[&warrior].owner, minor);
    assert_eq!(game.units[&warrior].levied_from, None);
    assert_eq!(game.unit_unembarked_strength(&game.units[&warrior]), 23.0);
}

#[test]
fn levy_control_changes_unlink_escort_formations() {
    let (mut game, _city, _position) = one_city(774_4041);
    let minor = game.players.len();
    game.players.push(Player::new(minor, "Geneva", true));
    game.players[0].envoys.push((minor, 3));
    let position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    let warrior = game.spawn_unit("warrior", minor, position);
    let city_state_ram = game.spawn_unit("battering_ram", minor, position);
    game.do_link_units(minor, warrior, city_state_ram).unwrap();
    game.players[0].gold = 200.0;

    game.do_levy_military(0, minor).unwrap();
    assert_eq!(game.units[&warrior].owner, 0);
    assert_eq!(game.units[&warrior].linked_to, None);
    assert_eq!(game.units[&city_state_ram].linked_to, None);

    let suzerain_ram = game.spawn_unit("battering_ram", 0, position);
    game.do_link_units(0, warrior, suzerain_ram).unwrap();
    game.turn += STANDARD_DEAL_TURNS;
    game.process_levies(0);

    assert_eq!(game.units[&warrior].owner, minor);
    assert_eq!(game.units[&warrior].linked_to, None);
    assert_eq!(game.units[&suzerain_ram].linked_to, None);
}

#[test]
fn a_returning_levy_never_shares_a_tile_with_its_own_class() {
    let (mut game, _city, _position) = one_city(774_407);
    let minor = game.players.len();
    game.players.push(Player::new(minor, "Valletta", true));
    game.players[0].envoys.push((minor, 3));
    let position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    let levied = game.spawn_unit("warrior", minor, position);
    game.players[0].gold = 100.0;
    game.do_levy_military(0, minor).unwrap();
    assert_eq!(game.units[&levied].owner, 0);

    // While the levy serves, the city-state fields another unit of the same
    // class on the tile the levy will come home to. Every legal
    // destination is taken, so relocation cannot succeed.
    let resident = game.spawn_unit("warrior", minor, position);
    let reachable: Vec<Pos> = game.wdisk(position, 4);
    for spot in reachable {
        if spot != position && game.units_at(spot).is_empty() && game.city_at(spot).is_none() {
            game.spawn_unit("warrior", minor, spot);
        }
    }

    game.turn += STANDARD_DEAL_TURNS;
    game.process_levies(0);

    // A tile may hold one unit of a class per owner: zone of control,
    // combat and stacking all assume it. The returning levy must not be
    // parked on top of the resident.
    let here: Vec<u32> = game
        .units_at(position)
        .into_iter()
        .filter(|uid| {
            game.units[uid].owner == minor
                && game.rules.units[game.units[uid].kind].class == "military"
        })
        .collect();
    assert!(
        here.len() <= 1,
        "two military units of {} share {:?}: {:?}",
        "Valletta",
        position,
        here
    );
    assert!(game.units.contains_key(&resident), "the resident stays put");
}

#[test]
fn a_levy_outliving_its_city_state_disbands_instead_of_stranding() {
    let (mut game, _city, _position) = one_city(774_406);
    let minor = game.players.len();
    game.players.push(Player::new(minor, "Geneva", true));
    game.players[0].envoys.push((minor, 3));
    let position = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.city_at(*position).is_none())
        .unwrap();
    let warrior = game.spawn_unit("warrior", minor, position);
    game.players[0].gold = 100.0;
    game.do_levy_military(0, minor).unwrap();
    assert_eq!(game.units[&warrior].owner, 0);

    // The city-state is eliminated while its army is out on loan, so the
    // elimination that disbands its units never sees this one.
    game.players[minor].alive = false;

    game.turn += STANDARD_DEAL_TURNS;
    game.process_levies(0);
    assert!(
        !game.units.contains_key(&warrior),
        "a returning levy cannot be handed to a civilization that no longer \
         exists; nobody would ever move it off the tile again"
    );
}

#[test]
fn secret_society_choice_unlocks_its_replacement_building_and_round_trips() {
    let (mut game, city, position) = one_city(774_405);
    game.players[0].civics.insert(crate::name!("code_of_laws"));
    game.players[0].techs.insert(crate::name!("education"));
    install_district(&mut game, city, position, "campus");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("library"));

    // Secret Societies is a New Frontier mode: a stock lobby is never
    // offered one, and asking anyway is refused.
    assert!(!game
        .legal_actions(0)
        .iter()
        .any(|action| matches!(action, Action::ChooseSecretSociety { .. })));
    assert!(game
        .apply(
            0,
            &Action::ChooseSecretSociety {
                society: crate::name!("hermetic_order"),
            },
        )
        .is_err());
    game.game_modes.insert("secret_societies".to_string());

    let choices: Vec<Action> = game
        .legal_actions(0)
        .into_iter()
        .filter(|action| matches!(action, Action::ChooseSecretSociety { .. }))
        .collect();
    assert_eq!(choices.len(), 3);
    game.apply(
        0,
        &Action::ChooseSecretSociety {
            society: crate::name!("hermetic_order"),
        },
    )
    .unwrap();

    let alchemical_society = Item::Building {
        building: crate::name!("alchemical_society"),
    };
    let university = Item::Building {
        building: crate::name!("university"),
    };
    assert!(game.can_produce(0, city, &alchemical_society));
    assert!(!game.can_produce(0, city, &university));
    assert!(!game
        .legal_actions(0)
        .iter()
        .any(|action| matches!(action, Action::ChooseSecretSociety { .. })));

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(
        restored.players[0].secret_society.as_deref(),
        Some("hermetic_order")
    );
    assert!(restored.can_produce(0, city, &alchemical_society));
}

#[test]
fn dar_e_mehr_ages_from_construction_and_resets_when_repaired() {
    let (mut game, city, position) = one_city(774_4051);
    install_district(&mut game, city, position, "holy_site");
    game.world_era = 2;
    let baseline = game.city_yields(city).faith;
    let dar_e_mehr = Item::Building {
        building: crate::name!("dar_e_mehr"),
    };

    assert!(game.complete_item(0, city, &dar_e_mehr));
    assert_eq!(game.cities[&city].building_eras[&Name::new("dar_e_mehr")], 2);
    assert!((game.city_yields(city).faith - baseline - 3.0).abs() < 1e-9);

    game.world_era = 5;
    assert!((game.city_yields(city).faith - baseline - 6.0).abs() < 1e-9);
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("dar_e_mehr"));
    assert_eq!(game.city_yields(city).faith, baseline);

    assert!(game.complete_item(
        0,
        city,
        &Item::Repair {
            repair: crate::name!("dar_e_mehr"),
            pos: position,
        },
    ));
    assert_eq!(game.cities[&city].building_eras[&Name::new("dar_e_mehr")], 5);
    assert!((game.city_yields(city).faith - baseline - 3.0).abs() < 1e-9);

    game.world_era = 7;
    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.cities[&city].building_eras[&Name::new("dar_e_mehr")], 5);
    assert!((restored.city_yields(city).faith - baseline - 5.0).abs() < 1e-9);
}

#[test]
fn rock_bands_are_faith_bought_and_perform_at_local_venues() {
    let (mut game, city, position) = one_city(crate::rng::fixture_seed("ROCKBAND", 774_407));
    game.players[0].civics.insert(crate::name!("cold_war"));
    game.players[0].faith = 2_000.0;
    let item = Item::Unit {
        unit: crate::name!("rock_band"),
    };
    assert!(!game.can_produce(0, city, &item));
    assert!(game.do_buy(0, city, "rock_band", "gold").is_err());
    game.do_buy(0, city, "rock_band", "faith").unwrap();
    assert_eq!(game.players[0].faith, 1_400.0);
    assert_eq!(game.rock_band_purchase_cost(0), 700.0);
    let band = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "rock_band")
        .unwrap();
    assert_eq!(game.units[&band].charges, 0);
    assert_eq!(game.rock_concert_tourism(0, band), None);
    assert!(game
        .apply(0, &Action::PerformConcert { unit: band })
        .is_err());
    let offers = game.available_promotions(band);
    assert_eq!(offers.len(), 3);
    assert_eq!(offers, game.available_promotions(band));
    game.apply(
        0,
        &Action::Promote {
            unit: band,
            promotion: offers[0],
        },
    )
    .unwrap();
    assert_eq!(game.units[&band].level, 1);
    assert_eq!(game.units[&band].xp, 0);
    game.units.get_mut(&band).unwrap().moves_left = 4.0;

    let rival = game.players.len();
    game.players
        .push(Player::new(rival, "Concert Rival", false));
    game.cities.get_mut(&city).unwrap().owner = rival;
    install_district(&mut game, city, position, "theater_square");
    game.cities.get_mut(&city).unwrap().buildings.extend(
        ["amphitheater", "art_museum", "broadcast_center"]
            .into_iter()
            .map(Name::new),
    );
    for _ in 0..4 {
        if game.units[&band].pos == position {
            break;
        }
        let next = game.route_step(band, position, 0).unwrap();
        game.apply(
            0,
            &Action::Move {
                unit: band,
                to: next,
            },
        )
        .unwrap();
    }
    assert_eq!(game.units[&band].pos, position);
    assert_eq!(game.rock_concert_tourism(0, band), Some(750.0));
    game.apply(0, &Action::PerformConcert { unit: band })
        .unwrap();
    let tier = game.players[0].counters[&format!("rock_concert_tier:{band}")];
    let (multiplier, albums, survives) = match tier {
        1 => (0.75, 0, false),
        2 => (2.0, 0, false),
        3 => (0.75, 50, true),
        4 => (2.5, 100, true),
        5 => (1.0, 150, true),
        6 => (3.0, 200, true),
        _ => unreachable!(),
    };
    assert_eq!(game.players[0].tourism_lifetime, 750.0 * multiplier);
    assert_eq!(
        game.players[0].targeted_tourism[&rival],
        750.0 * multiplier
    );
    assert_eq!(game.units.contains_key(&band), survives);
    if survives {
        assert_eq!(game.units[&band].album_sales, albums);
        assert_eq!(game.rock_concert_tourism(0, band), None);
    }
}

#[test]
fn resorts_scale_with_appeal_and_ski_resorts_are_spaced_and_unpillageable() {
    let (mut game, city, resort) = one_city(774_4061);
    game.players[0].techs.insert(crate::name!("radio"));
    let city_pos = game.cities[&city].pos;
    for neighbor in game.nbrs(resort) {
        let tile = game.map.tiles.get_mut(&neighbor).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.feature = None;
        tile.improvement = None;
    }
    let coast = game
        .nbrs(resort)
        .into_iter()
        .find(|position| *position != city_pos)
        .unwrap();
    game.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    let tile = game.map.tiles.get_mut(&resort).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.hills = false;
    tile.resource = None;
    tile.improvement = None;
    tile.district = None;
    tile.wonder = None;
    tile.pillaged = false;
    let appeal = game.tile_appeal(resort);
    assert!(appeal >= 4);
    assert!(game
        .valid_improvements(0, resort)
        .contains(&crate::name!("seaside_resort")));
    let gold_before = game
        .player_tile_yields(0, resort, &game.map.tiles[&resort])
        .gold;
    let tourism_before = game.tourism_per_turn(0);
    game.map.tiles.get_mut(&resort).unwrap().improvement = Some(crate::name!("seaside_resort"));
    assert_eq!(
        game.player_tile_yields(0, resort, &game.map.tiles[&resort])
            .gold
            - gold_before,
        appeal as f64
    );
    assert_eq!(game.tourism_per_turn(0) - tourism_before, appeal as f64);
    let tourism_map = game.tourism_by_tile(0);
    assert_eq!(tourism_map[&resort], appeal as f64);
    assert!(
        (tourism_map.values().sum::<f64>() - game.tourism_per_turn(0)).abs() < 1e-9,
        "the Tourism lens ledger must reconcile to the culture-victory total"
    );
    game.map.tiles.get_mut(&resort).unwrap().pillaged = true;
    assert_eq!(game.tourism_per_turn(0), tourism_before);

    game.players[0]
        .civics
        .insert(crate::name!("professional_sports"));
    let ski = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            *position != city_pos
                && *position != resort
                && game.wdist(*position, city_pos) <= 3
                && game
                    .nbrs(*position)
                    .iter()
                    .any(|neighbor| *neighbor != city_pos && *neighbor != resort)
        })
        .unwrap();
    let adjacent = game
        .nbrs(ski)
        .into_iter()
        .find(|position| *position != city_pos && *position != resort)
        .unwrap();
    for position in [ski, adjacent] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("mountain");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities
                .get_mut(&city)
                .unwrap()
                .owned_tiles
                .push(position);
        }
    }
    assert!(game
        .valid_improvements(0, ski)
        .contains(&crate::name!("ski_resort")));
    let amenities_before = game.city_local_amenities(&game.cities[&city]);
    game.map.tiles.get_mut(&ski).unwrap().improvement = Some(crate::name!("ski_resort"));
    assert!(!game
        .valid_improvements(0, adjacent)
        .contains(&crate::name!("ski_resort")));
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        amenities_before + 1
    );
    let pillager = game.players.len();
    game.players
        .push(Player::new(pillager, "Pillager", false));
    game.at_war.insert(pair(0, pillager));
    assert!(!game.pillageable_at(pillager, ski));
}

#[test]
fn naturalists_are_escalating_faith_purchases_that_establish_four_tile_parks() {
    let (mut game, city, _) = one_city(774_4062);
    game.players[0].civics.insert(crate::name!("conservation"));
    let city_pos = game.cities[&city].pos;
    let site = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|top| game.wdist(*top, city_pos) >= 4)
        .find_map(|top| game.national_park_diamond(top))
        .unwrap();
    for position in game.nbrs(site[0]) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.feature = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
    }
    for position in site {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.owner_city = Some(city);
        tile.terrain = crate::name!("mountain");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
        if !game.cities[&city].owned_tiles.contains(&position) {
            game.cities
                .get_mut(&city)
                .unwrap()
                .owned_tiles
                .push(position);
        }
    }
    game.map.tiles.get_mut(&site[0]).unwrap().terrain = crate::name!("plains");
    assert!(game.tile_appeal(site[0]) >= 2);
    assert!(game.national_park_sites(0).contains(&site));
    assert!(!game.can_produce(
        0,
        city,
        &Item::Unit {
            unit: crate::name!("naturalist")
        }
    ));
    assert!(game.do_buy(0, city, "naturalist", "gold").is_err());
    game.players[0].faith = 2_200.0;
    assert_eq!(game.naturalist_purchase_cost(0), 600.0);
    game.do_buy(0, city, "naturalist", "faith").unwrap();
    assert_eq!(game.players[0].faith, 1_600.0);
    assert_eq!(game.naturalist_purchase_cost(0), 700.0);
    let naturalist = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "naturalist")
        .unwrap();
    game.units.get_mut(&naturalist).unwrap().pos = site[0];
    let builder = game.spawn_unit("builder", 0, site[0]);
    assert!(game
        .apply(
            0,
            &Action::Improve {
                unit: builder,
                improvement: crate::name!("national_park"),
            },
        )
        .is_err());
    let tourism_before = game.tourism_per_turn(0);
    let amenities_before = game.city_local_amenities(&game.cities[&city]);
    let park_appeal = site
        .iter()
        .map(|position| game.tile_appeal(*position).max(0) as f64)
        .sum::<f64>();
    game.apply(
        0,
        &Action::Improve {
            unit: naturalist,
            improvement: crate::name!("national_park"),
        },
    )
    .unwrap();
    assert!(site.iter().all(|position| {
        game.map.tiles[position].improvement.as_deref() == Some("national_park")
    }));
    // The park's Tourism equals its appeal, up to the empire-wide
    // percentage modifiers (monopoly Products) the wider luxury pool can
    // seed onto this map.
    let park_tourism = game.tourism_per_turn(0) - tourism_before;
    assert!((park_tourism - park_appeal).abs() <= park_appeal * 0.01);
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        amenities_before + 2
    );
    assert!(game.district_sites(city, crate::name!("campus")).iter().all(|position| {
        game.map.tiles[position].improvement.as_deref() != Some("national_park")
    }));
    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.tourism_per_turn(0), game.tourism_per_turn(0));
    assert_eq!(
        restored.city_local_amenities(&restored.cities[&city]),
        game.city_local_amenities(&game.cities[&city])
    );
}

#[test]
fn rock_band_offers_three_promotions_unless_hallyu_is_active() {
    let (mut game, city, _) = one_city(774_4061);
    let band = game.spawn_test_unit("rock_band", 0, game.cities[&city].pos);
    let default_offers = game.available_promotions(band);
    assert_eq!(default_offers.len(), 3);
    assert_eq!(default_offers, game.available_promotions(band));

    game.players[0]
        .policies
        .insert(crate::name!("future_victory_culture"));
    let hallyu_offers = game.available_promotions(band);
    assert_eq!(hallyu_offers.len(), 12);
    assert!(hallyu_offers.contains(&crate::name!("religious_rock")));
    game.apply(
        0,
        &Action::Promote {
            unit: band,
            promotion: crate::name!("religious_rock"),
        },
    )
    .unwrap();
    assert_eq!(game.units[&band].level, 1);
    assert_eq!(game.units[&band].xp, 0);
    assert!(!game.promotion_pending(band));
}

#[test]
fn rock_band_probabilities_match_the_six_stock_performance_tables() {
    for level in 1..=6 {
        let weights = Game::rock_performance_weights(level);
        assert!((weights.iter().sum::<f64>() - 100.0).abs() < 0.101);
    }
    assert_eq!(Game::rock_performance_tier_for_roll(1, 0.0), 1);
    assert_eq!(Game::rock_performance_tier_for_roll(1, 18.39), 1);
    assert_eq!(Game::rock_performance_tier_for_roll(1, 18.4), 2);
    assert_eq!(Game::rock_performance_tier_for_roll(1, 44.89), 2);
    assert_eq!(Game::rock_performance_tier_for_roll(1, 44.9), 3);
    assert_eq!(Game::rock_performance_tier_for_roll(6, 0.49), 1);
    assert_eq!(Game::rock_performance_tier_for_roll(6, 0.5), 2);
    assert_eq!(Game::rock_performance_tier_for_roll(6, 99.99), 6);
}

#[test]
fn targeted_tourism_only_pressures_its_intended_rival_and_survives_saves() {
    let mut game = Game::new_full(3, 24, 16, 774_4062, 120, 0, false);
    game.players[0].tourism_lifetime = 2_000.0;
    game.players[0].targeted_tourism.insert(1, 1_000.0);
    assert_eq!(game.foreign_tourists(0), 4);

    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert_eq!(restored.players[0].targeted_tourism[&1], 1_000.0);
    assert_eq!(restored.foreign_tourists(0), 4);
}

#[test]
fn rock_band_promotions_unlock_venues_and_raise_matching_venue_levels() {
    let (mut game, city, theater) = one_city(774_4063);
    let rival = game.players.len();
    game.players
        .push(Player::new(rival, "Venue Rival", false));
    game.cities.get_mut(&city).unwrap().owner = rival;
    install_district(&mut game, city, theater, "theater_square");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("broadcast_center"));

    let theater_band = game.spawn_test_unit("rock_band", 0, theater);
    game.units
        .get_mut(&theater_band)
        .unwrap()
        .promotions
        .insert(crate::name!("roadies"));
    let ordinary = game
        .rock_concert_ai_value(0, theater_band, theater)
        .unwrap();
    game.units
        .get_mut(&theater_band)
        .unwrap()
        .promotions
        .insert(crate::name!("glam_rock"));
    assert!(
        game.rock_concert_ai_value(0, theater_band, theater)
            .unwrap()
            > ordinary
    );

    let resort = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != theater && *position != game.cities[&city].pos)
        .unwrap();
    {
        let tile = game.map.tiles.get_mut(&resort).unwrap();
        tile.district = None;
        tile.improvement = Some(crate::name!("seaside_resort"));
        tile.pillaged = false;
    }
    let surf_band = game.spawn_test_unit("rock_band", 0, resort);
    game.units
        .get_mut(&surf_band)
        .unwrap()
        .promotions
        .insert(crate::name!("roadies"));
    assert_eq!(game.rock_concert_tourism(0, surf_band), None);
    game.units
        .get_mut(&surf_band)
        .unwrap()
        .promotions
        .insert(crate::name!("surf_band"));
    assert_eq!(game.rock_concert_tourism(0, surf_band), Some(500.0));
}

#[test]
fn concert_promotions_apply_gold_loyalty_religion_and_nearby_tourism() {
    let (mut game, host_city, venue) = one_city(774_4064);
    let host_rival = game.players.len();
    game.players
        .push(Player::new(host_rival, "Host Rival", false));
    let nearby_rival = game.players.len();
    game.players
        .push(Player::new(nearby_rival, "Nearby Rival", false));
    game.cities.get_mut(&host_city).unwrap().owner = host_rival;
    install_district(&mut game, host_city, venue, "theater_square");
    game.cities
        .get_mut(&host_city)
        .unwrap()
        .buildings
        .push(crate::name!("broadcast_center"));

    let nearby_position = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| game.map.tiles[position].owner_city.is_none())
        .filter(|position| game.rules.is_passable(&game.map.tiles[position]))
        .find(|position| game.wdist(venue, *position) <= 10)
        .unwrap();
    game.found_city_for(
        nearby_rival,
        nearby_position,
        Some("Nearby City".to_string()),
    );

    let religion = "touring_faith".to_string();
    game.players[0].religion = Some(religion.clone());
    game.cities
        .get_mut(&host_city)
        .unwrap()
        .pressure
        .insert("host_faith".to_string(), 1_000.0);
    let band = game.spawn_test_unit("rock_band", 0, venue);
    game.units.get_mut(&band).unwrap().promotions.extend(
        [
            "glam_rock",
            "goes_to_11",
            "indie",
            "pop_star",
            "religious_rock",
        ]
        .into_iter()
        .map(Name::new),
    );

    game.apply(0, &Action::PerformConcert { unit: band })
        .unwrap();
    let host_tourism = game.players[0].targeted_tourism[&host_rival];
    assert!(host_tourism > 0.0);
    assert_eq!(
        game.players[0].targeted_tourism[&nearby_rival],
        host_tourism * 0.5
    );
    assert_eq!(game.players[0].gold, host_tourism * 0.25);
    assert_eq!(game.cities[&host_city].loyalty, 60.0);
    assert_eq!(
        game.city_religion(&game.cities[&host_city]),
        Some(religion.as_str())
    );
}

#[test]
fn national_parks_use_total_appeal_and_supply_the_five_correct_cities() {
    let (mut game, host_city, _) = one_city(774_4066);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = Some(crate::name!("forest"));
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.hills = false;
        tile.pillaged = false;
    }
    let park = game
        .map
        .tiles
        .keys()
        .copied()
        .find_map(|top| {
            game.national_park_diamond(top).filter(|positions| {
                positions
                    .iter()
                    .all(|position| *position != game.cities[&host_city].pos)
            })
        })
        .unwrap();
    for position in park {
        game.map.tiles.get_mut(&position).unwrap().owner_city = Some(host_city);
        if !game.cities[&host_city].owned_tiles.contains(&position) {
            game.cities
                .get_mut(&host_city)
                .unwrap()
                .owned_tiles
                .push(position);
        }
    }

    let extra_positions: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| game.city_at(*position).is_none())
        .filter(|position| {
            park.iter()
                .all(|park_position| game.wdist(*position, *park_position) > 1)
        })
        .take(5)
        .collect();
    assert_eq!(extra_positions.len(), 5);
    for (index, position) in extra_positions.into_iter().enumerate() {
        game.found_city_for(0, position, Some(format!("Park Neighbor {index}")));
    }

    let city_ids = game.player_city_ids(0);
    // Keep every city in the same Amenities yield band so this test
    // isolates the park's direct Tourism from its separate Amenity grant.
    for city_id in &city_ids {
        game.cities.get_mut(city_id).unwrap().pop = 30;
    }
    let amenities_before: BTreeMap<u32, i64> = city_ids
        .iter()
        .map(|city_id| (*city_id, game.city_local_amenities(&game.cities[city_id])))
        .collect();
    let tourism_before = game.tourism_per_turn(0);
    let culture_before = game
        .player_city_ids(0)
        .into_iter()
        .map(|city_id| game.city_yields(city_id).culture)
        .sum::<f64>();
    let park_appeal: f64 = park
        .iter()
        .map(|position| game.tile_appeal(*position).max(0) as f64)
        .sum();
    for position in park {
        game.map.tiles.get_mut(&position).unwrap().improvement =
            Some(crate::name!("national_park"));
    }
    let culture_after = game
        .player_city_ids(0)
        .into_iter()
        .map(|city_id| game.city_yields(city_id).culture)
        .sum::<f64>();
    let expected_tourism = park_appeal + 0.15 * (culture_after - culture_before);
    assert!((game.tourism_per_turn(0) - tourism_before - expected_tourism).abs() < 1e-9);
    assert_eq!(
        game.city_local_amenities(&game.cities[&host_city]) - amenities_before[&host_city],
        2
    );

    let mut nearest: Vec<(i32, u32)> = city_ids
        .iter()
        .copied()
        .filter(|city_id| *city_id != host_city)
        .map(|city_id| {
            (
                park.iter()
                    .map(|position| game.wdist(game.cities[&city_id].pos, *position))
                    .min()
                    .unwrap(),
                city_id,
            )
        })
        .collect();
    nearest.sort();
    for (index, (_, city_id)) in nearest.into_iter().enumerate() {
        assert_eq!(
            game.city_local_amenities(&game.cities[&city_id]) - amenities_before[&city_id],
            i64::from(index < 4)
        );
    }

    let mut bridge_without_park = game.clone();
    for position in park {
        bridge_without_park
            .map
            .tiles
            .get_mut(&position)
            .unwrap()
            .improvement = None;
    }
    let mut bridge_with_park = game.clone();
    for variant in [&mut bridge_without_park, &mut bridge_with_park] {
        let city_pos = variant.cities[&host_city].pos;
        variant
            .cities
            .get_mut(&host_city)
            .unwrap()
            .wonders
            .insert(crate::name!("golden_gate_bridge"), city_pos);
    }
    let bridge_appeal: f64 = park
        .iter()
        .map(|position| bridge_with_park.tile_appeal(*position).max(0) as f64)
        .sum();
    assert_eq!(
        bridge_with_park.tourism_per_turn(0) - bridge_without_park.tourism_per_turn(0),
        bridge_appeal * 2.0
    );
}

#[test]
fn ski_resorts_are_unworkable_but_national_park_tiles_remain_workable() {
    let (mut game, city, ski) = one_city(774_4067);
    let park = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|position| *position != game.cities[&city].pos && *position != ski)
        .unwrap();
    game.map.tiles.get_mut(&ski).unwrap().improvement = Some(crate::name!("ski_resort"));
    game.map.tiles.get_mut(&park).unwrap().improvement = Some(crate::name!("national_park"));
    game.cities.get_mut(&city).unwrap().pop = 100;

    let plan = game.city_citizen_plan(city);
    assert!(!plan.worked_tiles.contains(&ski));
    assert!(plan.worked_tiles.contains(&park));
}

#[test]
fn appeal_improvements_drive_gold_tourism_and_cristo_from_rules_data() {
    let (mut game, city, position) = one_city(774_4065);
    {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.hills = false;
        tile.pillaged = false;
    }
    for neighbor in game.nbrs(position) {
        let tile = game.map.tiles.get_mut(&neighbor).unwrap();
        tile.terrain = crate::name!("coast");
        tile.feature = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.pillaged = false;
    }
    let appeal = game.tile_appeal(position).max(0) as f64;
    assert!(appeal >= 4.0);
    let bare_yields = game.player_tile_yields(0, position, &game.map.tiles[&position]);
    let tourism_without_resort = game.tourism_per_turn(0);

    game.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("seaside_resort"));
    let resort_yields = game.player_tile_yields(0, position, &game.map.tiles[&position]);
    assert_eq!(resort_yields.gold - bare_yields.gold, appeal);
    assert_eq!(game.tourism_per_turn(0) - tourism_without_resort, appeal);

    let mut cristo_without_resort = game.clone();
    cristo_without_resort
        .map
        .tiles
        .get_mut(&position)
        .unwrap()
        .improvement = None;
    cristo_without_resort
        .cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("cristo_redentor"), game.cities[&city].pos);
    let mut cristo_with_resort = cristo_without_resort.clone();
    cristo_with_resort
        .map
        .tiles
        .get_mut(&position)
        .unwrap()
        .improvement = Some(crate::name!("seaside_resort"));
    assert_eq!(
        cristo_with_resort.tourism_per_turn(0) - cristo_without_resort.tourism_per_turn(0),
        appeal * 2.0
    );
}

#[test]
fn flood_defenses_mitigate_damage_and_great_bath_adds_permanent_faith() {
    let (mut game, city, position) = one_city(774_407);
    {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.feature = Some(crate::name!("grassland_floodplains"));
        tile.improvement = Some(crate::name!("farm"));
        tile.pillaged = false;
    }
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("great_bath"), position);

    let score_before = game.players[0].era_score;
    game.resolve_flood(&[position]);
    assert!(!game.map.tiles[&position].pillaged);
    assert_eq!(game.map.tiles[&position].disaster_faith, 1.0);
    game.resolve_flood(&[position]);
    assert_eq!(game.map.tiles[&position].disaster_faith, 2.0);
    assert_eq!(game.players[0].era_score, score_before + 2);

    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .remove(&Name::new("great_bath"));
    install_district(&mut game, city, position, "holy_site");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("dar_e_mehr"));
    game.resolve_flood(&[position]);
    assert!(game.map.tiles[&position].pillaged);
    assert!(!game.cities[&city].pillaged_buildings.contains(&Name::new("dar_e_mehr")));
}

#[test]
fn aqueducts_dams_and_flood_barriers_execute_disaster_protection() {
    let (mut game, city, position) = one_city(774_408);
    let farm = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .find(|tile| *tile != position && *tile != game.cities[&city].pos)
        .unwrap();
    game.map.tiles.get_mut(&farm).unwrap().improvement = Some(crate::name!("farm"));
    let food_before = game
        .player_tile_yields(0, farm, &game.map.tiles[&farm])
        .food;
    install_district(&mut game, city, position, "aqueduct");
    let unit = game.spawn_unit("warrior", 0, farm);
    game.resolve_drought(&[farm]);
    assert!(game.map.tiles[&farm].pillaged);
    assert!(game.map.tiles[&farm].drought);
    assert_eq!(game.units[&unit].hp, 100);
    assert_eq!(
        game.player_tile_yields(0, farm, &game.map.tiles[&farm])
            .food,
        food_before,
        "Aqueducts prevent the drought Food loss, not improvement pillaging"
    );

    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .0
        .remove(&Name::new("aqueduct"));
    game.map.tiles.get_mut(&position).unwrap().district = None;
    game.map.tiles.get_mut(&farm).unwrap().pillaged = false;
    game.resolve_drought(&[farm]);
    assert!(game.map.tiles[&farm].pillaged);
    assert_eq!(
        game.player_tile_yields(0, farm, &game.map.tiles[&farm])
            .food,
        food_before - 1.0
    );
    game.clear_drought(&[farm]);
    assert!(!game.map.tiles[&farm].drought);
    assert_eq!(
        game.player_tile_yields(0, farm, &game.map.tiles[&farm])
            .food,
        food_before
    );

    let coast = game.nbrs(position)[0];
    game.map.tiles.get_mut(&coast).unwrap().terrain = crate::name!("coast");
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.terrain = crate::name!("plains");
    tile.hills = false;
    tile.feature = None;
    tile.coastal_lowland = 1;
    tile.improvement = Some(crate::name!("farm"));
    tile.pillaged = true;
    let barrier = Item::Building {
        building: crate::name!("flood_barrier"),
    };
    let lowlands = game.coastal_lowland_tiles(&game.cities[&city]).len();
    assert!(lowlands > 0);
    game.players[0].techs.insert(crate::name!("computers"));
    assert!(game.can_produce(0, city, &barrier));
    assert_eq!(
        game.item_cost_for_city(0, city, &barrier),
        80.0 * lowlands as f64
    );
    assert!(game.complete_item(0, city, &barrier));
    assert!(!game.map.tiles[&position].pillaged);
    game.resolve_coastal_flooding();
    assert!(!game.map.tiles[&position].pillaged);

    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .retain(|building| building != "flood_barrier");
    game.resolve_coastal_flooding();
    assert!(game.map.tiles[&position].pillaged);
}

/// A battlefield is decided by the fighting: the arena runs no random
/// disasters — except the one class a named battle is actually remembered
/// for, which stays exactly as historical as the rest of its briefing.
#[test]
fn a_battlefield_runs_only_the_disasters_its_battle_is_remembered_for() {
    let classes = [
        "volcanic_eruption",
        "river_flood",
        "drought",
        "hurricane",
        "tornado",
        "blizzard",
        "dust_storm",
    ];
    // A civ world is unrestricted; a generic arena — and a named battle
    // with calm weather on its record — allows nothing.
    for class in classes {
        assert!(Game::script_disaster_allowed(MapScript::Pangaea, class));
        for script in [
            MapScript::Battlefield,
            MapScript::TacticsPlanet,
            MapScript::TacticsOcean,
            MapScript::Kadesh,
            MapScript::Agincourt,
            MapScript::Waterloo,
        ] {
            assert!(
                !Game::script_disaster_allowed(script, class),
                "{script:?} should not run {class}"
            );
        }
    }
    // The remembered weather, battle by battle — and only that class.
    let remembered = [
        (MapScript::Hattin, "drought"),
        (MapScript::SpanishArmada, "hurricane"),
        (MapScript::Stalingrad, "blizzard"),
        (MapScript::Normandy, "hurricane"),
        (MapScript::DienBienPhu, "river_flood"),
        (MapScript::DesertStorm, "dust_storm"),
        (MapScript::Mosul, "dust_storm"),
    ];
    for (script, kept) in remembered {
        for class in classes {
            assert_eq!(
                Game::script_disaster_allowed(script, class),
                class == kept,
                "{script:?} {class}"
            );
        }
    }
    // Every class a scenario lists is one the shipped rules actually
    // roll, so an allowance cannot be a typo that silently never fires.
    let (game, _, _) = one_city(774_410);
    for scenario in crate::historical_scenarios::all() {
        for &class in scenario.disasters {
            assert!(
                game.rules.disasters.get(class).is_some(),
                "{} lists unknown disaster {class}",
                scenario.id
            );
        }
    }
    // And the gate reaches the rate itself: the same intensity that gives
    // a civ world its storms leaves the arena becalmed.
    assert!(game.disaster_rate("blizzard") > 0.0);
    let mut arena = Game::new_with(GameOptions {
        map_script: MapScript::Battlefield,
        ..GameOptions::new(2, 20, 14, 774_411, 60, 0)
    });
    arena.disaster_intensity = 4;
    for class in classes {
        assert_eq!(arena.disaster_rate(class), 0.0, "arena rolled {class}");
    }
}

#[test]
fn nuclear_plants_age_recommission_and_resolve_all_accident_state() {
    let (mut game, city, position) = one_city(774_409);
    install_district(&mut game, city, position, "industrial_zone");
    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("factory"), crate::name!("nuclear_power_plant")];
    game.cities.get_mut(&city).unwrap().reactor_age = 10;
    game.process_reactors(0);
    assert_eq!(game.cities[&city].reactor_age, 11);
    assert_eq!(game.reactor_accident_risk(city), 0.0005);

    let recommission = Item::Project {
        project: crate::name!("recommission_reactor"),
    };
    assert!(game.complete_item(0, city, &recommission));
    assert_eq!(game.cities[&city].reactor_age, 0);
    assert_eq!(game.reactor_accident_risk(city), 0.0);

    game.cities.get_mut(&city).unwrap().pop = 5;
    let worker = game.spawn_unit("builder", 0, position);
    let fallout_tile = game
        .nbrs(position)
        .into_iter()
        .find(|candidate| game.map.tiles[candidate].owner_city == Some(city))
        .unwrap();
    let restored_yields =
        game.player_tile_yields(0, fallout_tile, &game.map.tiles[&fallout_tile]);
    game.resolve_reactor_accident(city, 2);
    assert!(game.cities[&city]
        .pillaged_buildings
        .contains(&Name::new("nuclear_power_plant")));
    assert_eq!(game.cities[&city].pop, 4);
    assert!(game.map.tiles[&position].fallout_until >= game.turn + 20);
    assert_eq!(
        game.player_tile_yields(0, fallout_tile, &game.map.tiles[&fallout_tile]),
        Yields::default()
    );
    game.turn = game.map.tiles[&fallout_tile].fallout_until;
    assert_eq!(
        game.player_tile_yields(0, fallout_tile, &game.map.tiles[&fallout_tile]),
        restored_yields
    );
    assert_eq!(game.units[&worker].hp, 60);
    assert_eq!(game.players[0].counters["reactor_accident:2"], 1);
}

#[test]
fn power_conversion_projects_replace_exactly_one_existing_plant() {
    let (mut game, city, position) = one_city(774_410);
    install_district(&mut game, city, position, "industrial_zone");
    game.players[0].techs.extend([
        crate::name!("industrialization"),
        crate::name!("electricity"),
        crate::name!("nuclear_fission"),
    ]);
    game.cities.get_mut(&city).unwrap().buildings =
        vec![crate::name!("factory"), crate::name!("coal_power_plant")];

    let coal = Item::Project {
        project: crate::name!("convert_reactor_to_coal"),
    };
    let oil = Item::Project {
        project: crate::name!("convert_reactor_to_oil"),
    };
    let nuclear = Item::Project {
        project: crate::name!("convert_reactor_to_uranium"),
    };
    assert!(!game.can_produce(0, city, &coal));
    assert!(game.can_produce(0, city, &oil));
    assert!(game.complete_item(0, city, &oil));
    assert!(game.cities[&city]
        .buildings
        .contains(&crate::name!("oil_power_plant")));
    assert!(!game.cities[&city]
        .buildings
        .contains(&crate::name!("coal_power_plant")));
    assert!(!game.can_produce(0, city, &oil));

    game.cities.get_mut(&city).unwrap().reactor_age = 27;
    assert!(game.can_produce(0, city, &nuclear));
    assert!(game.complete_item(0, city, &nuclear));
    assert_eq!(
        game.cities[&city]
            .buildings
            .iter()
            .filter(|building| matches!(
                building.as_str(),
                "coal_power_plant" | "oil_power_plant" | "nuclear_power_plant"
            ))
            .count(),
        1
    );
    assert!(game.cities[&city]
        .buildings
        .contains(&crate::name!("nuclear_power_plant")));
    assert_eq!(
        game.cities[&city].building_eras[&Name::new("nuclear_power_plant")],
        game.world_era
    );
    assert_eq!(game.cities[&city].reactor_age, 0);
}
