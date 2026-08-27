use super::*;

#[test]
fn replaceable_parts_replaces_the_feudalism_farm_rule_rather_than_stacking() {
    let (mut game, city) = one_city();
    let centre = game.cities[&city].pos;
    // A farm with four farmed neighbours, on flat workable ground.
    let ring: Vec<Pos> = game.nbrs(centre).iter().copied().collect();
    let subject = ring[0];
    let neighbours: Vec<Pos> = game
        .nbrs(subject)
        .iter()
        .copied()
        .filter(|position| *position != centre && game.map.tiles.contains_key(position))
        .take(4)
        .collect();
    assert_eq!(neighbours.len(), 4);
    for position in std::iter::once(subject).chain(neighbours.iter().copied()) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.pillaged = false;
        tile.improvement = Some(crate::name!("farm"));
    }
    let food = |game: &Game| {
        game.player_tile_yields(0, subject, &game.map.tiles[&subject])
            .food
    };
    let bare = food(&game);

    // Before Feudalism a Farm gets nothing from its neighbours.
    assert_eq!(food(&game), bare);

    // Farms_MedievalAdjacency: +1 Food per TWO adjacent Farms, so four
    // neighbours are worth two Food.
    game.players[0].civics.insert(crate::name!("feudalism"));
    assert_eq!(food(&game) - bare, 2.0);

    // Farms_MechanizedAdjacency: +1 per Farm. It carries PrereqTech
    // REPLACEABLE_PARTS and the Medieval row carries the same tech as its
    // ObsoleteTech, so the total is four rather than four plus two.
    game.players[0]
        .techs
        .insert(crate::name!("replaceable_parts"));
    assert_eq!(
        food(&game) - bare,
        4.0,
        "the Feudalism rule is obsolete, not additive"
    );
}

fn one_city() -> (Game, u32) {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("MAINTENANCE", 73_102),
        120,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    (game, city)
}

#[test]
fn unit_upkeep_uses_per_type_formation_and_policy_values() {
    let (mut game, city) = one_city();
    assert_eq!(game.unit_gold_maintenance(0), 0.0);
    let archer = game.spawn_unit("archer", 0, game.cities[&city].pos);
    let swordsman = game.spawn_unit("swordsman", 0, game.cities[&city].pos);
    game.units.get_mut(&swordsman).unwrap().formation = 1;

    assert_eq!(game.rules.units["archer"].maintenance, 1.0);
    assert_eq!(game.rules.units["giant_death_robot"].maintenance, 15.0);
    assert_eq!(game.unit_gold_maintenance(0), 4.0);

    game.players[0]
        .policies
        .insert(crate::name!("conscription"));
    assert_eq!(game.unit_gold_maintenance(0), 2.0);
    game.units.get_mut(&swordsman).unwrap().formation = 2;
    assert_eq!(game.unit_gold_maintenance(0), 3.0);

    game.remove_unit(archer);
    let mut baseline = game.clone();
    game.spawn_unit("crossbowman", 0, game.cities[&city].pos);
    baseline.begin_turn(0);
    game.begin_turn(0);
    assert_eq!(baseline.players[0].gold - game.players[0].gold, 2.0);
}

#[test]
fn infrastructure_upkeep_respects_exemptions_pillaging_and_flood_level() {
    let (mut game, city) = one_city();
    let positions: Vec<Pos> = game.cities[&city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| *position != game.cities[&city].pos)
        .take(4)
        .collect();
    assert_eq!(positions.len(), 4);
    for position in game.cities[&city].owned_tiles.clone() {
        game.map.tiles.get_mut(&position).unwrap().coastal_lowland = 0;
    }
    for position in positions.iter().copied().skip(2) {
        game.map.tiles.get_mut(&position).unwrap().coastal_lowland = 1;
    }
    for (district, position) in [("campus", positions[0]), ("commercial_hub", positions[1])] {
        game.cities
            .get_mut(&city)
            .unwrap()
            .districts
            .insert(Name::new(district), position);
        game.map.tiles.get_mut(&position).unwrap().district = Some(Name::new(district));
    }
    game.cities.get_mut(&city).unwrap().buildings.extend([
        crate::name!("library"),
        crate::name!("university"),
        crate::name!("flood_barrier"),
    ]);
    game.climate_phase = 2;

    // Campus 1 + Library 1 + University 2 + Barrier (2 tiles x level 2).
    assert_eq!(game.infrastructure_gold_maintenance(0), 8.0);
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("university"));
    assert_eq!(game.infrastructure_gold_maintenance(0), 6.0);
    game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = true;
    assert_eq!(game.infrastructure_gold_maintenance(0), 4.0);

    game.map.tiles.get_mut(&positions[0]).unwrap().pillaged = false;
    game.cities
        .get_mut(&city)
        .unwrap()
        .pillaged_buildings
        .remove(&Name::new("university"));
    game.climate_phase = 4;
    assert_eq!(game.infrastructure_gold_maintenance(0), 10.0);
}

#[test]
fn city_turn_upkeep_reduction_matches_post_processing_scan() {
    let (mut game, city) = one_city();
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .extend([crate::name!("monument"), crate::name!("granary")]);

    // Run the same ordered city phase the turn reducer uses, retaining
    // only the derived answers. The old aggregate is an independent
    // post-phase scan and therefore catches a missing local completion,
    // pillage, or flood-barrier contribution in the reducer ledger.
    let mut reduced = game.clone();
    let mut ledger = BTreeMap::new();
    for cid in reduced.player_city_ids(0) {
        let (_, upkeep, updates) = reduced.process_city_with_upkeep(0, cid);
        ledger.insert(cid, upkeep);
        for (changed_city, changed_upkeep) in updates {
            if ledger.contains_key(&changed_city) {
                ledger.insert(changed_city, changed_upkeep);
            }
        }
    }

    assert_eq!(
        ledger.values().copied().sum::<f64>(),
        reduced.infrastructure_gold_maintenance(0)
    );
}

#[test]
fn nuclear_devices_charge_fourteen_and_sixteen_gold_before_policy_discount() {
    let (mut game, _) = one_city();
    game.players[0]
        .counters
        .insert("project_effect:nuclear_devices".to_string(), 2);
    game.players[0]
        .counters
        .insert("project_effect:thermonuclear_devices".to_string(), 1);
    assert_eq!(game.nuclear_gold_maintenance(0), 44.0);

    game.players[0]
        .policies
        .insert(crate::name!("second_strike_capability"));
    assert_eq!(game.nuclear_gold_maintenance(0), 22.0);
}

#[test]
fn bankruptcy_thresholds_apply_amenities_disbanding_and_recovery() {
    let (mut game, city) = one_city();
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    let archer = game.spawn_unit("archer", 0, game.cities[&city].pos);
    let swordsman = game.spawn_unit("swordsman", 0, game.cities[&city].pos);
    let crossbowman = game.spawn_unit("crossbowman", 0, game.cities[&city].pos);
    let amenity_before = game.city_amenity_surplus(&game.cities[&city]);
    let mut twenty_deficit = game.clone();

    // The Amenity line is 0 and the disband line is -10, so a shallow
    // deficit costs an Amenity everywhere and no unit at all.
    game.settle_gold_budget(0, -9.0);
    assert_eq!(game.players[0].gold, 0.0);
    assert_eq!(game.players[0].gold_per_turn, -9.0);
    assert_eq!(game.players[0].bankruptcy_amenity_penalty, 1);
    assert_eq!(
        game.city_amenity_surplus(&game.cities[&city]),
        amenity_before - 1
    );
    assert_eq!(game.player_unit_ids(0).len(), 3);

    game.settle_gold_budget(0, -10.0);
    assert_eq!(game.players[0].bankruptcy_amenity_penalty, 2);
    assert_eq!(
        game.city_amenity_surplus(&game.cities[&city]),
        amenity_before - 2
    );
    assert!(game.units.contains_key(&archer));
    assert!(game.units.contains_key(&swordsman));
    assert!(
        !game.units.contains_key(&crossbowman),
        "the highest-maintenance unit is deterministically disbanded first"
    );

    twenty_deficit.settle_gold_budget(0, -20.0);
    assert_eq!(twenty_deficit.players[0].bankruptcy_amenity_penalty, 3);
    assert_eq!(twenty_deficit.player_unit_ids(0), vec![archer]);

    let encoded = serde_json::to_value(&game).unwrap();
    let mut legacy = encoded.clone();
    legacy["players"][0]
        .as_object_mut()
        .unwrap()
        .remove("gold_per_turn");
    legacy["players"][0]
        .as_object_mut()
        .unwrap()
        .remove("bankruptcy_amenity_penalty");
    let legacy: Game = serde_json::from_value(legacy).unwrap();
    assert_eq!(legacy.players[0].gold_per_turn, 0.0);
    assert_eq!(legacy.players[0].bankruptcy_amenity_penalty, 0);

    let mut restored: Game = serde_json::from_value(encoded).unwrap();
    assert_eq!(restored.players[0].gold_per_turn, -10.0);
    assert_eq!(restored.players[0].bankruptcy_amenity_penalty, 2);
    restored.settle_gold_budget(0, 4.0);
    assert_eq!(restored.players[0].gold, 4.0);
    assert_eq!(restored.players[0].gold_per_turn, 4.0);
    assert_eq!(restored.players[0].bankruptcy_amenity_penalty, 0);
    assert_eq!(
        restored.city_amenity_surplus(&restored.cities[&city]),
        amenity_before
    );
}

#[test]
fn begin_turn_records_net_gold_and_exposes_bankruptcy_state() {
    let (mut game, city) = one_city();
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    // Flatten the city's ground to bare plains so its yields are a known
    // quantity. Rivers are part of that ground and were being left on it,
    // which left one piece of the tile still up to the generator.
    for position in game.cities[&city].owned_tiles.clone() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.river_edges = [false; 6];
    }
    let robot = game.spawn_unit("giant_death_robot", 0, game.cities[&city].pos);
    let escort = game.spawn_unit(
        "giant_death_robot",
        0,
        *game.cities[&city]
            .owned_tiles
            .iter()
            .find(|pos| **pos != game.cities[&city].pos)
            .unwrap(),
    );
    assert!((game.city_yields(city).gold - 5.0).abs() < 1e-9);
    assert_eq!(game.unit_gold_maintenance(0), 30.0);

    game.players[0].gold = 0.0;
    game.begin_turn(0);
    assert_eq!(game.players[0].gold, 0.0);
    assert_eq!(game.players[0].gold_per_turn, -25.0);
    // Three Amenities from the 0 line at -10 steps, two disbands from the
    // -10 line at the same step: both robots go.
    assert_eq!(game.players[0].bankruptcy_amenity_penalty, 3);
    assert!(!game.units.contains_key(&robot));
    assert!(!game.units.contains_key(&escort));

    let observed = crate::obs::observation(&game, 0);
    assert_eq!(observed["me"]["gold_per_turn"], serde_json::json!(-25.0));
    assert_eq!(
        observed["me"]["bankruptcy_amenity_penalty"],
        serde_json::json!(3)
    );
    assert_eq!(
        observed["players"][0]["gold_per_turn"],
        serde_json::json!(-25.0)
    );
}

#[test]
fn unit_production_cards_only_reach_the_eras_they_ship_for() {
    // Agoge is an Ancient/Classical card. Before this gate it boosted a
    // Modern Infantry as readily as a Warrior.
    let (mut game, city) = one_city();
    let bonus = |game: &Game, unit: &str| {
        game.item_prod_mult(
            0,
            city,
            Some(&Item::Unit {
                unit: Name::new(unit),
            }),
        )
    };
    let warrior_before = bonus(&game, "warrior");
    let infantry_before = bonus(&game, "infantry");
    game.players[0].policies = [crate::name!("agoge")].into_iter().collect();
    assert_eq!(bonus(&game, "warrior"), warrior_before + 0.5);
    assert_eq!(bonus(&game, "infantry"), infantry_before);

    // Military First sits at the far end of the same ladder, but it does
    // not hand off from Grande Armee -- it repeats every era below it as
    // well, so it boosts a Warrior just as readily as a Mechanized
    // Infantry. See
    // the_unit_ladders_repeat_their_predecessors_eras_instead_of_succeeding_them.
    game.players[0].policies = [crate::name!("military_first")].into_iter().collect();
    assert_eq!(bonus(&game, "warrior"), warrior_before + 0.5);
    let mechanized_before = bonus(&game, "mechanized_infantry");
    game.players[0].policies.clear();
    assert_eq!(bonus(&game, "mechanized_infantry"), mechanized_before - 0.5);
}

#[test]
fn colosseum_and_colonial_offices_pay_the_loyalty_they_ship_with() {
    let (mut game, city) = one_city();
    let loyalty = |game: &mut Game| {
        game.cities.get_mut(&city).unwrap().loyalty = 50.0;
        game.process_loyalty(0);
        game.cities[&city].loyalty - 50.0
    };
    let baseline = loyalty(&mut game);

    // The Colosseum supplies +2 Loyalty directly and, in this fixture,
    // its +2 Amenities lifts the capital from Content to Happy for +3.
    let center = game.cities[&city].pos;
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("colosseum"), center);
    assert_eq!(loyalty(&mut game), baseline + 5.0);

    // Martial Law's shipped MARTIALLAW_GARRISONIDENTITY is +4, gated on
    // REQUIREMENT_CITY_HAS_GARRISON_UNIT. Limitanei is the +2 card.
    game.cities.get_mut(&city).unwrap().wonders.clear();
    game.spawn_unit("warrior", 0, game.cities[&city].pos);
    game.players[0].policies = [crate::name!("martial_law")].into_iter().collect();
    assert_eq!(loyalty(&mut game), baseline + 4.0);
    game.players[0].policies = [crate::name!("limitanei")].into_iter().collect();
    assert_eq!(loyalty(&mut game), baseline + 2.0);
}

#[test]
fn giant_death_robot_upgrades_hang_off_the_technologies_that_ship_them() {
    // Advanced Power Cells fits the Particle Beam Siege Cannon, Cybernetics
    // grants Enhanced Mobility, Smart Materials the armour plating and
    // Advanced AI the air defence. Nothing grants it extra healing.
    let (mut game, city) = one_city();
    let robot = game.spawn_unit("giant_death_robot", 0, game.cities[&city].pos);
    let unit = game.units[&robot].clone();

    assert_eq!(game.gdr_siege_bonus(&unit), 0.0);
    assert!(!game.gdr_full_wall_damage(&unit));
    let base_moves = game.unit_max_moves(robot);

    game.players[0].techs.insert(crate::name!("cybernetics"));
    assert_eq!(game.unit_max_moves(robot), base_moves + 3.0);
    assert_eq!(game.gdr_siege_bonus(&unit), 0.0);

    game.players[0]
        .techs
        .insert(crate::name!("advanced_power_cells"));
    assert_eq!(game.gdr_siege_bonus(&unit), 30.0);
    assert!(game.gdr_full_wall_damage(&unit));
}

#[test]
fn recurring_trade_contracts_are_part_of_each_players_budget() {
    let mut game = Game::new_full(2, 24, 16, 73_102, 120, 0, false);
    game.turn = 10;
    game.active_trade_deals.push(ActiveTradeDeal {
        id: 1,
        from: 0,
        to: 1,
        offer: DealItems {
            gold_per_turn: 7.0,
            ..DealItems::default()
        },
        request: DealItems {
            gold_per_turn: 2.0,
            ..DealItems::default()
        },
        started: 9,
        ends: 40,
    });
    assert_eq!(game.contracted_gold_per_turn(0), -5.0);
    assert_eq!(game.contracted_gold_per_turn(1), 5.0);

    game.players[0].gold = 1.0;
    game.settle_gold_budget(0, 0.0);
    game.settle_gold_budget(1, 0.0);
    assert_eq!(game.players[0].gold_per_turn, -5.0);
    assert_eq!(game.players[1].gold_per_turn, 5.0);
    assert_eq!(game.players[0].bankruptcy_amenity_penalty, 0);
}
