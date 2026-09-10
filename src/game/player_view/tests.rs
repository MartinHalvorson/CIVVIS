use super::*;

#[test]
fn every_major_gets_only_own_and_visible_units() {
    let game = Game::new_full(6, 40, 28, 91, 40, 0, false);
    for pid in 0..6 {
        let view = game.player_decision_view(pid);
        assert_eq!(view.player_unit_ids(pid), game.player_unit_ids(pid));
        for unit in view.units.values().filter(|u| u.owner != pid) {
            assert!(game.player_can_see(pid, unit.pos));
            assert!(game.unit_visible_to(unit.id, pid));
        }
        assert!(view.log.is_empty());
        for tile in view.map.tiles.values() {
            if !game.player_can_see(pid, tile.pos)
                && !game.players[pid].remembered_tiles.contains_key(&tile.pos)
            {
                assert_eq!(tile.terrain.as_str(), "unknown");
                assert!(tile.resource.is_none());
                assert!(tile.owner_city.is_none());
            }
        }
    }
}

#[test]
fn hidden_terrain_and_future_rolls_do_not_change_the_board() {
    let game = Game::new_full(2, 30, 20, 918, 40, 0, false);
    let before = game.player_decision_view(0);
    let mut changed = game.clone();
    let hidden = *game
        .map
        .tiles
        .keys()
        .find(|p| !game.player_can_see(0, **p) && !game.players[0].remembered_tiles.contains_key(p))
        .unwrap();
    let tile = changed.map.tiles.get_mut(&hidden).unwrap();
    tile.terrain = crate::name!("desert");
    tile.resource = Some(crate::name!("uranium"));
    changed.rng = Rng::new(987654);
    changed.seed = 444;
    let after = changed.player_decision_view(0);
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(&after).unwrap()
    );
}

#[test]
fn unknown_rival_research_and_treasury_are_not_exposed() {
    let game = Game::new_full(2, 30, 20, 812, 40, 0, false);
    let mut changed = game.clone();
    changed.players[0].met.clear();
    changed.players[1].research_progress = 9000.0;
    changed.players[1].gold = 12345.0;
    changed.players[1]
        .techs
        .insert(crate::name!("nuclear_fission"));
    let view = changed.player_decision_view(0);
    assert!(view.players[1].techs.is_empty());
    assert_eq!(view.players[1].research_progress, 0.0);
    assert_ne!(view.players[1].gold, 12345.0);
    assert!(!view.observed_public_empire_stats.contains_key(&1));
}

#[test]
fn global_religion_choices_survive_anonymous_rival_redaction() {
    let mut game = Game::new_full(2, 30, 20, 812, 40, 0, false);
    game.players[0].met.clear();
    game.players[1].pantheon = Some("religious_settlements".into());
    game.players[1].religion = Some("buddhism".into());
    game.players[1].religion_beliefs = vec!["tithe".into()];
    let view = game.player_decision_view(0);
    assert!(view
        .blocked_pantheons
        .contains(&crate::name!("religious_settlements")));
    assert_eq!(view.players[1].religion, game.players[1].religion);
    assert_eq!(
        view.players[1].religion_beliefs,
        game.players[1].religion_beliefs
    );
    assert!(!view.has_met(0, 1));
    assert!(view.players[1].pantheon.is_none());
}

#[test]
fn public_rivals_do_not_disclose_private_accumulators_or_camp_timers() {
    let mut game = Game::new_full(2, 30, 20, 812, 40, 0, false);
    game.players[0].met.insert(1);
    game.players[1].culture_lifetime = 12345.0;
    game.players[1].tourism_lifetime = 9876.0;
    let visible = game.units[&game.player_unit_ids(0)[0]].pos;
    game.barb_camps.insert(visible, 17);
    let view = game.player_decision_view(0);
    assert_eq!(view.players[1].culture_lifetime, 0.0);
    assert_eq!(view.players[1].tourism_lifetime, 0.0);
    assert_eq!(view.domestic_tourists(1), game.domestic_tourists(1));
    assert_eq!(view.barb_camps.get(&visible), Some(&0));
}
