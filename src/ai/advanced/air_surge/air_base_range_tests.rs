use super::*;

fn fixture(base: &str) -> (Game, Pos, Pos, [u32; 2]) {
    let mut game = Game::new_full(2, 48, 24, 4062, 650, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.clear_mirror_cities();
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let home = game.found_city_for(0, (12, 10), None);
    game.found_city_for(0, (6, 16), None);
    let target = (24, 10);
    let enemy = game.found_city_for(1, target, None);
    let forward = (14, 10);
    match base {
        "aerodrome" | "foreign_aerodrome" => {
            let owner = if base == "aerodrome" { home } else { enemy };
            let tile = game.map.tiles.get_mut(&forward).unwrap();
            tile.owner_city = Some(owner);
            tile.district = Some(crate::name!("aerodrome"));
            tile.pillaged = false;
            game.cities
                .get_mut(&owner)
                .unwrap()
                .districts
                .insert(crate::name!("aerodrome"), forward);
        }
        "airstrip" => {
            let tile = game.map.tiles.get_mut(&forward).unwrap();
            tile.owner_city = Some(home);
            tile.improvement = Some(crate::name!("airstrip"));
            tile.pillaged = false;
        }
        "carrier" | "foreign_carrier" => {
            game.map.tiles.get_mut(&forward).unwrap().terrain = crate::name!("coast");
            game.spawn_test_unit(
                "aircraft_carrier",
                usize::from(base == "foreign_carrier"),
                forward,
            );
        }
        "none" => {}
        _ => panic!("unknown base fixture"),
    }
    game.current = 0;
    game.turn = 170;
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    let wing = [
        game.spawn_test_unit("bomber", 0, (12, 10)),
        game.spawn_test_unit("bomber", 0, (6, 16)),
    ];
    assert_eq!(game.wdist((12, 10), target), 12);
    assert_eq!(game.wdist(forward, target), 10);
    assert_eq!(game.unit_attack_range(wing[0]), 10);
    (game, target, forward, wing)
}

fn assert_engine_can_base_the_wing(game: &mut Game, base: Pos, wing: [u32; 2]) {
    for unit in wing {
        let action = Action::AirRebase { unit, to: base };
        assert!(game.legal_doctrine_actions(0, unit).contains(&action));
        game.apply(0, &action).unwrap();
        assert_eq!(game.units[&unit].pos, base);
    }
}

#[test]
fn an_active_forward_aerodrome_reaches_a_city_its_center_cannot() {
    let (mut game, target, base, wing) = fixture("aerodrome");
    assert_engine_can_base_the_wing(&mut game, base, wing);
    assert!(
        AdvancedAi::air_surge_in_range(&game, 0, target),
        "two Bombers legally based on the Aerodrome can reach the objective"
    );
}

#[test]
fn an_owned_forward_airstrip_reaches_the_objective() {
    let (mut game, target, base, wing) = fixture("airstrip");
    assert_engine_can_base_the_wing(&mut game, base, wing);
    assert!(AdvancedAi::air_surge_in_range(&game, 0, target));
}

#[test]
fn an_owned_carrier_is_a_campaign_base_at_its_actual_position() {
    let (mut game, target, base, wing) = fixture("carrier");
    assert_engine_can_base_the_wing(&mut game, base, wing);
    assert!(AdvancedAi::air_surge_in_range(&game, 0, target));
}

#[test]
fn pillaged_fields_do_not_supply_range() {
    for field in ["aerodrome", "airstrip"] {
        let (mut game, target, base, wing) = fixture(field);
        game.map.tiles.get_mut(&base).unwrap().pillaged = true;
        assert!(!game
            .legal_doctrine_actions(0, wing[0])
            .contains(&Action::AirRebase {
                unit: wing[0],
                to: base,
            }));
        assert!(!AdvancedAi::air_surge_in_range(&game, 0, target));
    }
}

#[test]
fn foreign_airfields_and_carriers_do_not_supply_range() {
    for field in ["foreign_aerodrome", "foreign_carrier"] {
        let (game, target, base, wing) = fixture(field);
        assert!(!game
            .legal_doctrine_actions(0, wing[0])
            .contains(&Action::AirRebase {
                unit: wing[0],
                to: base,
            }));
        assert!(!AdvancedAi::air_surge_in_range(&game, 0, target));
    }
}

#[test]
fn existing_city_center_range_remains_available() {
    let (game, _, _, _) = fixture("none");
    assert!(AdvancedAi::air_surge_in_range(&game, 0, (20, 10)));
}

#[test]
fn one_jet_bomber_cannot_lend_range_to_an_older_partner() {
    let (mut game, target, _, wing) = fixture("none");
    game.units.get_mut(&wing[0]).unwrap().kind = crate::name!("jet_bomber");
    assert!(!AdvancedAi::air_surge_in_range(&game, 0, target));
    game.units.get_mut(&wing[1]).unwrap().kind = crate::name!("jet_bomber");
    assert!(AdvancedAi::air_surge_in_range(&game, 0, target));
}
