use super::*;

fn fixture(mirrored: bool) -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(4, 40, 24, 936029, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let home = g.found_city_for(0, (12, 12), None);
    let target = g.found_city_for(1, (18, 12), None);
    let next = g.found_city_for(1, (16, 17), None);
    g.found_city_for(2, (12, 19), None);
    g.found_city_for(3, (30, 4), None);
    for other in 1..4 {
        g.record_contact(0, other);
    }
    g.players[0].techs.extend(
        g.rules.tech_ancestors[AIR_SURGE_GOAL_TECH]
            .iter()
            .map(|tech| Name::new(tech)),
    );
    g.players[0].gold = 10_000.0;
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 400.0);
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 150;
    for pos in [(12, 13), (13, 12)] {
        g.spawn_test_unit("cuirassier", 0, pos);
    }
    if mirrored {
        std::sync::Arc::make_mut(&mut g.host_observed).insert(g.cities[&home].pos);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    ai.air_surge_plan = Some(AirSurge {
        target_player: 1,
        objective_city: target,
        objective_pos: g.cities[&target].pos,
        body_unit: crate::name!("knight"),
        body_is_cavalry: true,
        opened_at_war: true,
        phase: AirSurgePhase::Exploit,
        appointed_turn: 140,
        tech_turn: None,
        declared_turn: None,
        last_reviewed_turn: 150,
        recovery_assessments: 0,
    });
    (g, ai, target, next)
}

#[test]
fn a_projected_host_capture_does_not_retarget_before_host_confirmation() {
    let (observed, mut ai, target, next) = fixture(true);
    let mut projected = observed.clone();
    projected.cities.get_mut(&target).unwrap().owner = 0;

    ai.maintain_air_surge_after_units(&projected, 0);
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, target);

    ai.maintain_air_surge(&observed, 0);
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, target);

    ai.maintain_air_surge(&projected, 0);
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, next);
}

#[test]
fn a_native_capture_retargets_immediately_after_the_unit_pass() {
    let (mut native, mut ai, target, next) = fixture(false);
    native.cities.get_mut(&target).unwrap().owner = 0;
    ai.maintain_air_surge_after_units(&native, 0);
    assert_eq!(ai.air_surge_plan.as_ref().unwrap().objective_city, next);
}
