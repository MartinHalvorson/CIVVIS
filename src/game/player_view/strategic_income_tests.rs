use super::*;

fn income_fixture() -> (Game, usize, Pos) {
    let mut game = Game::new_full(2, 40, 26, 379_301, 500, 1, false);
    let minor = game
        .players
        .iter()
        .find(|p| p.is_minor && !p.is_barbarian)
        .unwrap()
        .id;
    let city = game.player_city_ids(minor)[0];
    let center = game.cities[&city].pos;
    let resource = *game.cities[&city]
        .owned_tiles
        .iter()
        .find(|p| **p != center)
        .unwrap();
    for tile in game.map.tiles.values_mut() {
        tile.resource = None;
    }
    let tile = game.map.tiles.get_mut(&resource).unwrap();
    tile.terrain = Name::new("plains");
    tile.feature = None;
    tile.resource = Some(Name::new("aluminum"));
    tile.improvement = Some(Name::new("mine"));
    tile.pillaged = false;
    game.players[0].techs = game.rules.techs.keys().copied().collect();
    game.players[0].envoys = vec![(minor, 3)];
    game.record_contact(0, minor);
    assert_eq!(game.suzerain_of(minor), Some(0));
    assert_eq!(game.strategic_resource_rate(0, "aluminum"), 2.0);
    (game, minor, resource)
}

#[test]
fn noncenter_city_state_mine_income_survives_public_city_redaction() {
    let (game, minor, _) = income_fixture();
    let view = game.player_decision_view(0);
    assert_eq!(view.suzerain_of(minor), Some(0));
    assert_eq!(view.strategic_resource_rate(0, "aluminum"), 2.0);
}

#[test]
fn every_strategic_resource_preserves_its_public_rate() {
    let (mut game, _, position) = income_fixture();
    for (resource, improvement, expected) in [
        ("horses", "pasture", 2.0),
        ("iron", "mine", 2.0),
        ("niter", "mine", 2.0),
        ("coal", "mine", 3.0),
        ("oil", "oil_well", 3.0),
        ("aluminum", "mine", 2.0),
        ("uranium", "mine", 3.0),
    ] {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.resource = Some(Name::new(resource));
        tile.improvement = Some(Name::new(improvement));
        assert_eq!(game.strategic_resource_rate(0, resource), expected);
        assert_eq!(
            game.player_decision_view(0)
                .strategic_resource_rate(0, resource),
            expected,
            "{resource}"
        );
    }
}

#[test]
fn repeated_observation_never_adds_the_readback_twice() {
    let (game, _, _) = income_fixture();
    let mut view = game.player_decision_view(0);
    for _ in 0..3 {
        assert_eq!(view.strategic_resource_rate(0, "aluminum"), 2.0);
        view = view.player_decision_view(0);
    }
}

#[test]
fn existing_host_adjustments_are_reconciled_even_when_clamped() {
    let (mut game, _, _) = income_fixture();
    for adjustment in [5.0, -1.0, -5.0] {
        Arc::make_mut(&mut game.observed_strategic_income_adjustments)
            .entry(0)
            .or_default()
            .insert(Name::new("aluminum"), adjustment);
        let observed = game.strategic_resource_rate(0, "aluminum");
        assert_eq!(
            game.player_decision_view(0)
                .strategic_resource_rate(0, "aluminum"),
            observed
        );
    }
}

#[test]
fn income_readback_does_not_mutate_world_or_copy_foreign_private_corrections() {
    let (mut game, minor, _) = income_fixture();
    Arc::make_mut(&mut game.observed_strategic_income_adjustments)
        .entry(1)
        .or_default()
        .insert(Name::new("aluminum"), 77.0);
    let before = serde_json::to_value(&game).unwrap();
    let view = game.player_decision_view(0);
    assert_eq!(serde_json::to_value(&game).unwrap(), before);
    assert!(!view.observed_strategic_income_adjustments.contains_key(&1));
    let city = game.player_city_ids(minor)[0];
    assert_eq!(view.cities[&city].owned_tiles, vec![game.cities[&city].pos]);
    assert_eq!(view.strategic_resource_rate(0, "aluminum"), 2.0);
}

#[test]
fn fresh_readback_loses_pillaged_or_no_longer_allied_income() {
    let (mut game, _, position) = income_fixture();
    let before = game.player_decision_view(0);
    assert_eq!(before.strategic_resource_rate(0, "aluminum"), 2.0);
    game.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert_eq!(
        game.player_decision_view(0)
            .strategic_resource_rate(0, "aluminum"),
        0.0
    );
    game.map.tiles.get_mut(&position).unwrap().pillaged = false;
    game.players[0].envoys.clear();
    assert_eq!(
        game.player_decision_view(0)
            .strategic_resource_rate(0, "aluminum"),
        0.0
    );
}

#[test]
fn own_improvement_counterfactual_changes_income_on_the_observed_board() {
    let (mut game, _, _) = income_fixture();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|id| game.units[id].kind == "settler")
        .unwrap();
    let city = game.found_city_for(0, game.units[&settler].pos, None);
    let position = *game.cities[&city]
        .owned_tiles
        .iter()
        .find(|p| **p != game.cities[&city].pos)
        .unwrap();
    let tile = game.map.tiles.get_mut(&position).unwrap();
    tile.resource = Some(Name::new("aluminum"));
    tile.improvement = None;
    tile.pillaged = false;
    let mut view = game.player_decision_view(0);
    assert_eq!(view.strategic_resource_rate(0, "aluminum"), 2.0);
    view.map.tiles.get_mut(&position).unwrap().improvement = Some(Name::new("mine"));
    assert_eq!(view.strategic_resource_rate(0, "aluminum"), 4.0);
    view.map.tiles.get_mut(&position).unwrap().pillaged = true;
    assert_eq!(view.strategic_resource_rate(0, "aluminum"), 2.0);
}

#[test]
fn unrevealed_strategic_resources_do_not_gain_an_income_disclosure() {
    let (mut game, _, _) = income_fixture();
    game.players[0].techs.clear();
    assert_eq!(game.strategic_resource_rate(0, "aluminum"), 0.0);
    assert_eq!(
        game.player_decision_view(0)
            .strategic_resource_rate(0, "aluminum"),
        0.0
    );
}
