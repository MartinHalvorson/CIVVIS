use super::*;

fn fixture() -> (Game, u32, Pos) {
    let mut game = Game::new_full(2, 24, 16, 4064, 500, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    let target = (8, 6);
    let tile = game.map.tiles.get_mut(&target).unwrap();
    tile.terrain = crate::name!("grassland");
    tile.feature = None;
    tile.hills = false;
    tile.resource = None;
    tile.improvement = None;
    tile.owner_city = None;
    tile.district = None;
    tile.district_foundation = None;
    tile.wonder = None;
    tile.flooded = false;
    game.players[0].techs.insert(crate::name!("flight"));
    let engineer = game.spawn_test_unit("military_engineer", 0, target);
    assert!(game.units[&engineer].charges > 0);
    assert!(game.units[&engineer].moves_left > 0.0);
    assert!(game.rules.units["military_engineer"]
        .builds
        .contains(&crate::name!("airstrip")));
    assert!(game.map.tiles[&target].owner_city.is_none());
    (game, engineer, target)
}

#[test]
fn an_engineer_can_build_an_airstrip_on_unowned_flat_ground() {
    let (mut game, engineer, target) = fixture();
    assert!(
        game.valid_improvements(0, target)
            .contains(&crate::name!("airstrip")),
        "Firaxis Improvements.xml permits Airstrips outside territory"
    );
    let action = Action::Improve {
        unit: engineer,
        improvement: crate::name!("airstrip"),
    };
    assert!(game.legal_actions(0).contains(&action));
    game.apply(0, &action).unwrap();
    assert_eq!(
        game.map.tiles[&target].improvement.as_deref(),
        Some("airstrip")
    );
    assert!(game.map.tiles[&target].owner_city.is_none());
    assert_eq!(game.air_capacity_at(0, target), 3);
}

#[test]
fn neutral_permission_does_not_open_ordinary_builder_work_or_a_missile_silo() {
    let (mut game, _, target) = fixture();
    game.players[0].techs.insert(crate::name!("rocketry"));
    let offered = game.valid_improvements(0, target);
    assert!(offered.contains(&crate::name!("airstrip")));
    assert!(!offered.contains(&crate::name!("farm")));
    assert!(!offered.contains(&crate::name!("missile_silo")));
}

#[test]
fn outside_permission_retains_terrain_unlock_and_observation_limits() {
    let (game, _, target) = fixture();
    let mut hill = game.clone();
    hill.map.tiles.get_mut(&target).unwrap().hills = true;
    let mut forest = game.clone();
    forest.map.tiles.get_mut(&target).unwrap().feature = Some(crate::name!("forest"));
    let mut locked = game.clone();
    locked.players[0].techs.remove(&crate::name!("flight"));
    let mut unknown_owner = game.clone();
    Arc::make_mut(&mut unknown_owner.blocked_city_sites).insert(target);
    let mut stale_owner = game.clone();
    stale_owner.map.tiles.get_mut(&target).unwrap().owner_city = Some(u32::MAX);
    for blocked in [&hill, &forest, &locked, &unknown_owner, &stale_owner] {
        assert!(!blocked
            .valid_improvements(0, target)
            .contains(&crate::name!("airstrip")));
    }
}

#[test]
fn a_builder_or_a_spent_engineer_cannot_claim_neutral_airstrip_permission() {
    let (game, engineer, target) = fixture();
    for mode in 0..3 {
        let mut blocked = game.clone();
        let uid = if mode == 0 {
            blocked.remove_unit(engineer);
            blocked.spawn_test_unit("builder", 0, target)
        } else {
            if mode == 1 {
                blocked.units.get_mut(&engineer).unwrap().charges = 0;
            } else {
                blocked.units.get_mut(&engineer).unwrap().moves_left = 0.0;
            }
            engineer
        };
        let action = Action::Improve {
            unit: uid,
            improvement: crate::name!("airstrip"),
        };
        assert!(!blocked.legal_actions(0).contains(&action));
        assert!(blocked.apply(0, &action).is_err());
        assert!(blocked.map.tiles[&target].improvement.is_none());
    }
}

#[test]
fn only_the_neutral_airstrips_known_owner_gets_its_active_slots() {
    let (mut game, _, target) = fixture();
    let tile = game.map.tiles.get_mut(&target).unwrap();
    tile.improvement = Some(crate::name!("airstrip"));
    tile.improvement_owner = Some(1);
    assert_eq!(game.air_capacity_at(0, target), 0);
    assert_eq!(game.air_capacity_at(1, target), 3);
    game.map.tiles.get_mut(&target).unwrap().pillaged = true;
    assert_eq!(game.air_capacity_at(1, target), 0);
    game.map.tiles.get_mut(&target).unwrap().pillaged = false;
    game.map.tiles.get_mut(&target).unwrap().improvement_owner = None;
    assert_eq!(game.air_capacity_at(0, target), 0);
    assert_eq!(game.air_capacity_at(1, target), 0);
    game.map.tiles.get_mut(&target).unwrap().improvement_owner = Some(1);
    game.map.tiles.get_mut(&target).unwrap().improvement = None;
    assert_eq!(game.air_capacity_at(1, target), 0);
}

#[test]
fn neutral_airstrip_ownership_survives_a_save_and_legacy_tiles_have_no_owner() {
    let (mut game, _, target) = fixture();
    game.map.tiles.get_mut(&target).unwrap().improvement = Some(crate::name!("airstrip"));
    game.map.tiles.get_mut(&target).unwrap().improvement_owner = Some(0);
    let bytes = serde_json::to_vec(&game).unwrap();
    let loaded: Game = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(loaded.air_capacity_at(0, target), 3);
    assert!(loaded.map.tiles[&target].owner_city.is_none());
    let mut old = serde_json::to_value(&game.map.tiles[&target]).unwrap();
    old.as_object_mut().unwrap().remove("improvement_owner");
    let legacy: crate::world::Tile = serde_json::from_value(old).unwrap();
    assert_eq!(legacy.improvement_owner, None);
}

#[test]
fn an_unseen_improvement_owner_does_not_cross_the_player_view_boundary() {
    let (mut game, engineer, target) = fixture();
    game.remove_unit(engineer);
    for player in game.players.iter_mut() {
        player.explored.clear();
        player.remembered_tiles.forget_all();
    }
    game.map.tiles.get_mut(&target).unwrap().improvement = Some(crate::name!("airstrip"));
    game.map.tiles.get_mut(&target).unwrap().improvement_owner = Some(1);
    let view = game.player_decision_view(0);
    assert_eq!(view.map.tiles[&target].terrain.as_str(), "unknown");
    assert_eq!(view.map.tiles[&target].improvement_owner, None);
}

#[test]
fn a_constructed_neutral_airstrip_accepts_three_rebases_and_refuses_the_fourth() {
    let (mut game, engineer, target) = fixture();
    let origins = [(2, 2), (2, 6), (2, 10), (6, 2)];
    for position in origins.into_iter().chain([(16, 8)]) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.improvement = None;
    }
    for origin in origins {
        game.found_city_for(0, origin, None);
    }
    game.found_city_for(1, (16, 8), None);
    assert!(game.map.tiles[&target].owner_city.is_none());
    game.apply(
        0,
        &Action::Improve {
            unit: engineer,
            improvement: crate::name!("airstrip"),
        },
    )
    .unwrap();
    for (index, origin) in origins.into_iter().enumerate() {
        let aircraft = game.spawn_test_unit("bomber", 0, origin);
        let action = Action::AirRebase {
            unit: aircraft,
            to: target,
        };
        if index < 3 {
            assert!(game.legal_doctrine_actions(0, aircraft).contains(&action));
            game.apply(0, &action).unwrap();
            assert_eq!(game.units[&aircraft].pos, target);
        } else {
            assert!(!game.legal_doctrine_actions(0, aircraft).contains(&action));
            assert!(game.apply(0, &action).is_err());
            assert_eq!(game.units[&aircraft].pos, origin);
        }
    }
}

#[test]
fn an_owned_legacy_airstrip_keeps_its_slots_without_independent_owner_metadata() {
    let (mut game, _, target) = fixture();
    game.map.tiles.get_mut(&(7, 6)).unwrap().terrain = crate::name!("grassland");
    game.map.tiles.get_mut(&(7, 6)).unwrap().feature = None;
    let city = game.found_city_for(0, (7, 6), None);
    let tile = game.map.tiles.get_mut(&target).unwrap();
    tile.owner_city = Some(city);
    tile.improvement = Some(crate::name!("airstrip"));
    tile.improvement_owner = None;
    assert_eq!(game.air_capacity_at(0, target), 3);
    game.map.tiles.get_mut(&target).unwrap().improvement_owner = Some(1);
    assert_eq!(
        game.air_capacity_at(0, target),
        3,
        "territorial transfer keeps the existing base rule"
    );
    assert_eq!(game.air_capacity_at(1, target), 0);
    game.map.tiles.get_mut(&target).unwrap().owner_city = Some(u32::MAX);
    assert_eq!(
        game.air_capacity_at(1, target),
        0,
        "a stale city handle is not neutral land"
    );
}
