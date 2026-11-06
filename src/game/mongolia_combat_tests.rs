use super::*;

fn fixture() -> Game {
    let mut game = Game::new_full(2, 20, 14, 591_106, 250, 0, false);
    for id in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(id);
    }
    game.map.clear_rivers();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
        tile.owner_city = None;
    }
    for player in game.players.iter_mut() {
        player.civ = "Rome".into();
        player.government = None;
        player.policies.clear();
        player.techs.clear();
        player.civics.clear();
        player.religion = None;
    }
    game.players[0].civ = "Mongolia".into();
    game.handicap_exempt.extend([0, 1]);
    game
}

#[test]
fn mongolia_combat_visibility_uses_only_its_positive_advantage() {
    let mut game = fixture();
    let ours = game.spawn_test_unit("warrior", 0, (4, 4));
    let theirs = game.spawn_test_unit("warrior", 1, (5, 4));
    for (ours_level, theirs_level, ours_bonus, theirs_bonus) in [
        (0.0, 0.0, 0.0, 0.0),
        (1.0, 0.0, 6.0, 0.0),
        (2.0, 0.0, 12.0, 0.0),
        (4.0, 0.0, 24.0, 0.0),
        (3.0, 1.0, 12.0, 0.0),
        (2.0, 2.0, 0.0, 0.0),
        (0.0, 2.0, 0.0, 6.0),
    ] {
        game.players[0].observed_visibility.insert(1, ours_level);
        game.players[1].observed_visibility.insert(0, theirs_level);
        assert_eq!(
            game.matchup_bonus(ours, &game.units[&theirs], true),
            ours_bonus
        );
        assert_eq!(
            game.matchup_bonus(theirs, &game.units[&ours], false),
            theirs_bonus
        );
    }
}

#[test]
fn mongolia_combat_genghis_bonus_is_cavalry_specific_on_both_strengths() {
    let mut game = fixture();
    for (kind, expected) in [
        ("warrior", 0.0),
        ("archer", 0.0),
        ("horseman", 3.0),
        ("knight", 3.0),
        ("helicopter", 3.0),
        ("keshig", 3.0),
        ("maryannu_chariot_archer", 0.0),
        ("galley", 0.0),
        ("bomber", 0.0),
    ] {
        let id = game.spawn_test_unit(kind, 0, (4, 4));
        let mut reference = game.clone();
        reference.players[0].civ = "Rome".into();
        assert_eq!(
            game.unit_strength(&game.units[&id], false)
                - reference.unit_strength(&reference.units[&id], false),
            expected,
            "{kind} attack"
        );
        assert_eq!(
            game.unit_strength(&game.units[&id], true)
                - reference.unit_strength(&reference.units[&id], true),
            expected,
            "{kind} defense"
        );
        if game.rules.units[kind].ranged_strength > 0.0 {
            assert_eq!(
                game.unit_ranged_strength(&game.units[&id])
                    - reference.unit_ranged_strength(&reference.units[&id]),
                expected,
                "{kind} ranged"
            );
        }
        game.remove_unit(id);
    }
}

#[test]
fn mongolia_combat_kublai_keeps_ortoo_without_inheriting_genghis() {
    let mut game = fixture();
    let ours = game.spawn_test_unit("keshig", 0, (4, 4));
    let theirs = game.spawn_test_unit("warrior", 1, (5, 4));
    let default_melee = game.unit_strength(&game.units[&ours], false);
    let default_ranged = game.unit_ranged_strength(&game.units[&ours]);
    for leader in ["LEADER_KUBLAI_KHAN_MONGOLIA", "LEADER_UNKNOWN_ALTERNATE"] {
        Arc::make_mut(&mut game.observed_leader_types).insert(0, leader.into());
        game.players[0].observed_visibility.insert(1, 2.0);
        game.players[1].observed_visibility.insert(0, 0.0);
        assert_eq!(
            game.unit_strength(&game.units[&ours], false),
            default_melee - 3.0
        );
        assert_eq!(
            game.unit_ranged_strength(&game.units[&ours]),
            default_ranged - 3.0
        );
        assert_eq!(game.matchup_bonus(ours, &game.units[&theirs], true), 12.0);
    }
    Arc::make_mut(&mut game.observed_leader_types).insert(0, "LEADER_GENGHIS_KHAN".into());
    assert_eq!(
        game.unit_ranged_strength(&game.units[&ours]),
        default_ranged
    );
}

#[test]
fn mongolia_combat_visibility_reaches_melee_and_both_sides_of_a_shot() {
    let mut game = fixture();
    let ours = game.spawn_test_unit("archer", 0, (4, 4));
    let theirs = game.spawn_test_unit("archer", 1, (5, 4));
    game.players[0].observed_visibility.insert(1, 0.0);
    game.players[1].observed_visibility.insert(0, 0.0);
    let melee = game.melee_exchange_strengths(ours, theirs).unwrap();
    let shot = game.ranged_strike_strengths(ours, theirs, (5, 4)).unwrap();
    let incoming = game.ranged_strike_strengths(theirs, ours, (4, 4)).unwrap();
    game.players[0].observed_visibility.insert(1, 2.0);
    let upgraded = game.melee_exchange_strengths(ours, theirs).unwrap();
    assert_eq!(upgraded, (melee.0 + 12.0, melee.1));
    assert_eq!(
        game.ranged_strike_strengths(ours, theirs, (5, 4)).unwrap(),
        (shot.0 + 12.0, shot.1)
    );
    assert_eq!(
        game.ranged_strike_strengths(theirs, ours, (4, 4)).unwrap(),
        (incoming.0, incoming.1 + 12.0)
    );
}

#[test]
fn mongolia_combat_minor_and_barbarian_seats_do_not_inherit_civilization_traits() {
    let mut game = fixture();
    let ours = game.spawn_test_unit("horseman", 0, (4, 4));
    let theirs = game.spawn_test_unit("warrior", 1, (5, 4));
    game.players[0].observed_visibility.insert(1, 2.0);
    game.players[1].observed_visibility.insert(0, 0.0);
    for barbarian in [false, true] {
        game.players[0].is_minor = !barbarian;
        game.players[0].is_barbarian = barbarian;
        assert_eq!(game.mongolian_cavalry_strength(&game.units[&ours]), 0.0);
        assert_eq!(game.matchup_bonus(ours, &game.units[&theirs], true), 6.0);
    }
}
