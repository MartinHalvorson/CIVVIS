use super::*;

#[test]
fn visible_rival_units_do_not_reveal_private_lifetime_or_training_records() {
    let mut game = Game::new_full(2, 30, 20, 918, 40, 0, false);
    let own = game.player_unit_ids(0)[0];
    let rival = game.player_unit_ids(1)[0];
    let pos = game.units[&own].pos;
    let unit = game.units.get_mut(&rival).unwrap();
    unit.pos = pos;
    unit.damage_dealt = 900;
    unit.production_cost = 700.0;
    unit.xp_bonus_pct = 75.0;
    unit.free_upkeep = true;
    unit.linked_to = Some(999_999);
    let view = game.player_decision_view(0);
    let observed = &view.units[&rival];
    assert_eq!(observed.damage_dealt, 0);
    assert_eq!(observed.production_cost, 0.0);
    assert_eq!(observed.xp_bonus_pct, 0.0);
    assert!(!observed.free_upkeep);
    assert!(observed.linked_to.is_none());
    assert_eq!(observed.hp, game.units[&rival].hp);
    assert_eq!(
        view.units[&own].production_cost,
        game.units[&own].production_cost
    );
}

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

fn city_with_hidden_loyalty_pressure() -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 40, 26, 91_170, 200, 0, false);
    let home = g.units[&g.player_unit_ids(0)[0]].pos;
    g.found_city_for(0, home, None);
    let frontier = g
        .map
        .tiles
        .values()
        .find(|tile| {
            (8..=10).contains(&g.wdist(home, tile.pos))
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && g.city_at(tile.pos).is_none()
        })
        .unwrap()
        .pos;
    g.found_city_for(0, frontier, None);
    let own = g.city_at(frontier).unwrap();
    let hidden = g
        .map
        .tiles
        .values()
        .find(|tile| {
            (5..=7).contains(&g.wdist(frontier, tile.pos))
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && !g.player_can_see(0, tile.pos)
                && g.city_at(tile.pos).is_none()
        })
        .unwrap()
        .pos;
    g.found_city_for(1, hidden, None);
    let rival = g.city_at(hidden).unwrap();
    g.cities.get_mut(&rival).unwrap().pop = 40;
    assert!(!g.player_can_see(0, hidden));
    (g, own, rival)
}

#[test]
fn owned_city_loyalty_rate_survives_hidden_population_redaction() {
    let (g, own, rival) = city_with_hidden_loyalty_pressure();
    let actual = g.city_loyalty_per_turn(&g.cities[&own]);
    assert!(
        actual < 0.0,
        "the city must actually be losing loyalty: {actual}"
    );
    let view = g.player_decision_view(0);
    assert!(
        !view.cities.contains_key(&rival),
        "the pressure source stays hidden"
    );
    assert_eq!(view.city_loyalty_per_turn(&view.cities[&own]), actual);
    assert!(view
        .observed_city_loyalty_per_turn
        .keys()
        .all(|id| view.cities[id].owner == 0));
    assert!(
        g.observed_city_loyalty_per_turn.is_empty(),
        "building a view does not mutate the world"
    );
}

#[test]
fn loyalty_readback_preserves_own_reports_without_exposing_foreign_reports() {
    let (mut g, own, rival) = city_with_hidden_loyalty_pressure();
    Arc::make_mut(&mut g.observed_city_loyalty_per_turn).extend([(own, -7.5), (rival, -99.0)]);
    let view = g.player_decision_view(0);
    assert_eq!(view.city_loyalty_per_turn(&view.cities[&own]), -7.5);
    assert!(!view.observed_city_loyalty_per_turn.contains_key(&rival));
    assert!(!view.cities.contains_key(&rival));
}

#[test]
fn each_observation_refreshes_the_current_owned_loyalty_rate() {
    let (mut g, own, rival) = city_with_hidden_loyalty_pressure();
    let before = g.player_decision_view(0);
    g.cities.get_mut(&rival).unwrap().pop = 1;
    let actual = g.city_loyalty_per_turn(&g.cities[&own]);
    let after = g.player_decision_view(0);
    assert_eq!(after.city_loyalty_per_turn(&after.cities[&own]), actual);
    assert_ne!(before.city_loyalty_per_turn(&before.cities[&own]), actual);
    assert!(!after.cities.contains_key(&rival));
}
