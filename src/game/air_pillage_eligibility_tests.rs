use super::*;

fn fixture(district: bool) -> (Game, u32, Pos) {
    let mut game = Game::new_full(3, 40, 24, 936187, 1000, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.owner_city = None;
        tile.district = None;
    }
    for player in game.players.iter_mut() {
        player.civ = "Rome".into();
        player.government = None;
        player.policies.clear();
        player.explored.extend(game.map.tiles.keys().copied());
    }
    let base = (6, 12);
    let target = (11, 12);
    game.found_city_for(0, base, None);
    let enemy_city = game.found_city_for(1, (12, 12), None);
    let tile = game.map.tiles.get_mut(&target).unwrap();
    tile.owner_city = Some(enemy_city);
    tile.pillaged = false;
    if district {
        tile.district = Some(crate::name!("campus"));
        game.cities
            .get_mut(&enemy_city)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), target);
        game.cities
            .get_mut(&enemy_city)
            .unwrap()
            .buildings
            .push(crate::name!("library"));
    } else {
        tile.improvement = Some(crate::name!("mine"));
    }
    game.at_war.insert((0, 1));
    game.current = 0;
    let bomber = game.spawn_test_unit("bomber", 0, base);
    game.spawn_test_unit("scout", 0, (10, 12));
    (game, bomber, target)
}

fn assert_occupied_target_rejected(kind: &str, owner: usize, district: bool) {
    let (mut game, bomber, target) = fixture(district);
    let occupant = game.spawn_test_unit(kind, owner, target);
    let action = Action::AirPillage {
        unit: bomber,
        target,
    };
    assert!(
        !game.air_pillageable_at(0, target),
        "{kind}/{owner}/{district}"
    );
    assert!(!game.legal_doctrine_actions(0, bomber).contains(&action));
    assert!(!game.legal_actions(0).contains(&action));
    let before = game.clone();
    assert!(game.apply(0, &action).is_err());
    assert_eq!(
        game.map.tiles[&target].pillaged,
        before.map.tiles[&target].pillaged
    );
    let city = game.map.tiles[&target].owner_city.unwrap();
    assert_eq!(
        game.cities[&city].pillaged_buildings,
        before.cities[&city].pillaged_buildings
    );
    assert_eq!(
        game.units[&bomber].attacks_left,
        before.units[&bomber].attacks_left
    );
    assert_eq!(
        game.units[&bomber].moves_left,
        before.units[&bomber].moves_left
    );
    assert_eq!(game.units[&occupant].hp, before.units[&occupant].hp);
}

#[test]
fn air_pillage_rejects_civilian_occupied_districts() {
    for kind in [
        "builder",
        "settler",
        "trader",
        "apostle",
        "rock_band",
        "missionary",
    ] {
        assert_occupied_target_rejected(kind, 1, true);
    }
}

#[test]
fn air_pillage_rejects_land_occupants_on_improvements_too() {
    for kind in ["builder", "warrior", "battering_ram", "anti_air_gun"] {
        assert_occupied_target_rejected(kind, 1, false);
    }
}

#[test]
fn air_pillage_rejects_friendly_and_neutral_land_occupants() {
    for owner in [0, 2] {
        for district in [false, true] {
            assert_occupied_target_rejected("builder", owner, district);
        }
    }
}

#[test]
fn removing_the_land_occupant_restores_air_pillage() {
    for district in [false, true] {
        let (mut game, bomber, target) = fixture(district);
        let builder = game.spawn_test_unit("builder", 1, target);
        assert!(!game.air_pillageable_at(0, target));
        game.remove_unit(builder);
        let action = Action::AirPillage {
            unit: bomber,
            target,
        };
        assert!(game.legal_doctrine_actions(0, bomber).contains(&action));
        assert!(game.legal_actions(0).contains(&action));
        game.apply(0, &action).unwrap();
        if district {
            let city = game.map.tiles[&target].owner_city.unwrap();
            assert!(game.cities[&city]
                .pillaged_buildings
                .contains(&crate::name!("library")));
        } else {
            assert!(game.map.tiles[&target].pillaged);
        }
    }
}

#[test]
fn air_pillage_land_occupant_rule_does_not_change_ground_pillage() {
    let (mut game, _, target) = fixture(false);
    let raider = game.spawn_test_unit("warrior", 0, target);
    let action = Action::Pillage { unit: raider };
    assert!(game.legal_actions(0).contains(&action));
    game.apply(0, &action).unwrap();
    assert!(game.map.tiles[&target].pillaged);
}

#[test]
fn land_garrison_can_still_be_attacked_instead_of_air_pillaged() {
    let (mut game, bomber, target) = fixture(true);
    let defender = game.spawn_test_unit("warrior", 1, target);
    let action = Action::AirStrike {
        unit: bomber,
        target,
    };
    assert!(game.legal_actions(0).contains(&action));
    game.apply(0, &action).unwrap();
    assert!(game.units.get(&defender).is_none_or(|unit| unit.hp < 100));
}

#[test]
fn air_and_sea_domains_do_not_trigger_the_land_occupant_gate() {
    for kind in ["bomber", "fighter", "galley"] {
        let (mut game, _, target) = fixture(false);
        game.spawn_test_unit(kind, 1, target);
        assert!(game.air_pillageable_at(0, target), "{kind}");
    }
}
