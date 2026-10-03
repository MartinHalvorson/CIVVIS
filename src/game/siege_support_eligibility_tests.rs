use super::*;

fn fixture() -> Game {
    let mut game = Game::new_full(2, 40, 24, 936195, 1000, 0, false);
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
    game.found_city_for(0, (6, 12), None);
    game.at_war.insert((0, 1));
    game.current = 0;
    game
}

#[test]
fn siege_support_does_not_extend_aircraft_operational_range() {
    for aircraft in ["bomber", "jet_bomber", "fighter", "jet_fighter"] {
        for support in ["observation_balloon", "drone"] {
            let mut game = fixture();
            let unit = game.spawn_test_unit(aircraft, 0, (6, 12));
            let printed_range = game.unit_attack_range(unit);
            game.spawn_test_unit(support, 0, (7, 12));
            assert_eq!(
                game.unit_attack_range(unit),
                printed_range,
                "{aircraft}/{support}"
            );
        }
    }
}

#[test]
fn ground_siege_keeps_one_friendly_support_range_bonus() {
    for kind in [
        "catapult",
        "trebuchet",
        "bombard",
        "artillery",
        "rocket_artillery",
    ] {
        let mut game = fixture();
        let unit = game.spawn_test_unit(kind, 0, (6, 12));
        let base = game.unit_attack_range(unit);
        let balloon = game.spawn_test_unit("observation_balloon", 0, (7, 12));
        let drone = game.spawn_test_unit("drone", 0, (6, 12));
        assert_eq!(
            game.unit_attack_range(unit),
            base + 1,
            "{kind}: bonuses do not stack"
        );
        game.remove_unit(balloon);
        game.remove_unit(drone);
        game.spawn_test_unit("drone", 1, (7, 12));
        assert_eq!(
            game.unit_attack_range(unit),
            base,
            "{kind}: foreign support is ineligible"
        );
    }
}

#[test]
fn only_artillery_and_rocket_artillery_receive_drone_bombard_strength() {
    // Gathering Storm's CLASS_TARGETTING_ASSIST tags, not all units whose
    // attacks use a Bombard stat (notably Catapults and Bombers).
    for kind in [
        "artillery",
        "rocket_artillery",
        "catapult",
        "trebuchet",
        "bombard",
        "bomber",
        "jet_bomber",
    ] {
        let mut game = fixture();
        let unit = game.spawn_test_unit(kind, 0, (6, 12));
        let base = game.unit_bombard_strength(&game.units[&unit]);
        let drone = game.spawn_test_unit("drone", 0, (7, 12));
        let extra = if matches!(kind, "artillery" | "rocket_artillery") {
            5.0
        } else {
            0.0
        };
        assert_eq!(
            game.unit_bombard_strength(&game.units[&unit]),
            base + extra,
            "{kind}"
        );
        game.remove_unit(drone);
        assert_eq!(
            game.unit_bombard_strength(&game.units[&unit]),
            base,
            "{kind}: support left"
        );
    }
}

#[test]
fn aircraft_range_promotions_still_apply_without_ground_support_bonus() {
    let mut game = fixture();
    let bomber = game.spawn_test_unit("bomber", 0, (6, 12));
    game.units
        .get_mut(&bomber)
        .unwrap()
        .promotions
        .insert(crate::name!("long_range"));
    assert_eq!(game.unit_attack_range(bomber), 12);
    game.spawn_test_unit("drone", 0, (7, 12));
    assert_eq!(game.unit_attack_range(bomber), 12);
}

#[test]
fn balloon_cannot_make_the_native_distance_eleven_bomber_strike_legal() {
    let mut game = fixture();
    let bomber = game.spawn_test_unit("bomber", 0, (6, 12));
    game.spawn_test_unit("observation_balloon", 0, (7, 12));
    let target = (17, 12);
    let defender = game.spawn_test_unit("warrior", 1, target);
    game.spawn_test_unit("scout", 0, (16, 12));
    assert_eq!(game.wdist(game.units[&bomber].pos, target), 11);
    let action = Action::AirStrike {
        unit: bomber,
        target,
    };
    assert!(!game.legal_actions(0).contains(&action));
    let before = (game.units[&bomber].attacks_left, game.units[&defender].hp);
    assert!(game.apply(0, &action).is_err());
    assert_eq!(
        (game.units[&bomber].attacks_left, game.units[&defender].hp),
        before
    );
}
