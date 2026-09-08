use super::*;

fn board() -> (Game, Pos) {
    let mut g = Game::new_full(2, 24, 18, 9185, 100, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    g.map.clear_rivers();
    for t in g.map.tiles.values_mut() {
        t.terrain = crate::name!("plains");
        t.feature = None;
        t.hills = false;
        t.resource = None;
        t.improvement = None;
        t.district = None;
        t.district_foundation = None;
        t.owner_city = None;
        t.wonder = None;
    }
    g.at_war.insert((0, 1));
    g.current = 0;
    let center = *g
        .map
        .tiles
        .keys()
        .filter(|p| g.wdisk(**p, 5).len() == 91)
        .min_by_key(|p| g.wdist(**p, (8, 8)))
        .unwrap();
    (g, center)
}

#[test]
fn movement_risk_prices_cover_and_enemy_move_then_attack() {
    let (mut g, pos) = board();
    let ours = g.spawn_test_unit("warrior", 0, pos);
    let enemy_pos = g
        .wdisk(pos, 2)
        .into_iter()
        .find(|p| g.wdist(*p, pos) == 2)
        .unwrap();
    g.spawn_test_unit("warrior", 1, enemy_pos);
    let ai = BasicAi::new();
    let risk = ai.movement_risk_frame(&g, 0, ours, pos);
    let flat = risk.score(&g, 0, ours, pos, 0.5);
    assert!(flat < 0.0, "enemy two hexes away can move and attack");
    g.map.tiles.get_mut(&pos).unwrap().hills = true;
    let hill = risk.score(&g, 0, ours, pos, 0.5);
    assert!(hill > flat, "cover must reduce expected damage");
    g.units.get_mut(&ours).unwrap().hp = 25;
    assert!(
        risk.score(&g, 0, ours, pos, 0.5) < hill - 50.0,
        "wounded units preserve their lives"
    );
}

#[test]
fn movement_risk_does_not_carry_fortification_to_another_tile() {
    let (mut g, pos) = board();
    let ours = g.spawn_test_unit("warrior", 0, pos);
    let next = g.nbrs(pos)[0];
    g.spawn_test_unit("archer", 1, g.nbrs(next)[0]);
    let ai = BasicAi::new();
    let envelopes = ai.enemy_attack_envelopes(&g, 0);
    let unfortified = BasicAi::incoming_damage(&g, 0, ours, next, &envelopes).total;
    assert!(unfortified > 0.0);
    g.units.get_mut(&ours).unwrap().fortify_turns = 2;
    assert_eq!(
        BasicAi::incoming_damage(&g, 0, ours, next, &envelopes).total,
        unfortified
    );
}

#[test]
fn movement_risk_hazards_hurt_garrisons_and_expire() {
    let (mut g, pos) = board();
    g.found_city_for(0, pos, None);
    let ours = g.spawn_test_unit("warrior", 0, pos);
    g.map.tiles.get_mut(&pos).unwrap().fallout_until = g.turn + 5;
    let ai = BasicAi::new();
    let envelopes = ai.enemy_attack_envelopes(&g, 0);
    assert!(BasicAi::anything_can_reach(&g, 0, pos, &envelopes));
    assert_eq!(
        BasicAi::incoming_damage(&g, 0, ours, pos, &envelopes).total,
        20.0
    );
    g.map.tiles.get_mut(&pos).unwrap().feature = Some(crate::name!("burning_forest"));
    assert_eq!(BasicAi::movement_hazard_damage(&g, 0, pos), 95.0);
    g.map.tiles.get_mut(&pos).unwrap().feature = None;
    g.turn += 5;
    assert_eq!(BasicAi::movement_hazard_damage(&g, 0, pos), 0.0);
}

#[test]
fn movement_risk_forecasts_a_visible_storm_without_foreseeing_random_events() {
    let (mut g, pos) = board();
    g.spawn_test_unit("warrior", 0, pos);
    let heading = 0;
    let next = g.map.step(pos, heading).unwrap();
    g.storms.push(crate::game::Storm {
        kind: "tornado".into(),
        pos,
        heading,
        severity: 1,
        ends: g.turn + 3,
    });
    assert_eq!(BasicAi::movement_hazard_damage(&g, 0, next), 30.0);
    g.storms[0].ends = g.turn + 1;
    assert_eq!(BasicAi::movement_hazard_damage(&g, 0, next), 0.0);
    g.storms.clear();
    assert_eq!(BasicAi::movement_hazard_damage(&g, 0, next), 0.0);
}

#[test]
fn movement_risk_siege_waits_for_reachable_survivable_distinct_attackers() {
    let (mut g, city_pos) = board();
    let cid = g.found_city_for(1, city_pos, None);
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let ring = g.nbrs(city_pos);
    let ours = g.spawn_test_unit("swordsman", 0, ring[0]);
    let ai = BasicAi::new();
    let alone = ai.movement_risk_frame(&g, 0, ours, city_pos);
    assert_eq!(alone.ready, 0);
    let solo_cost = alone.score(&g, 0, ours, ring[0], 0.5);
    let a = g.spawn_test_unit("swordsman", 0, ring[2]);
    let b = g.spawn_test_unit("swordsman", 0, ring[4]);
    let together = ai.movement_risk_frame(&g, 0, ours, city_pos);
    assert_eq!(together.ready, 3);
    assert_eq!(together.assault_positions.len(), 3);
    assert!(together.score(&g, 0, ours, ring[0], 0.5) > solo_cost);
    g.units.get_mut(&ours).unwrap().hp = 10;
    assert!(
        together.score(&g, 0, ours, ring[0], 0.5) < -100.0,
        "sharing never makes a lethal shot safe"
    );
    g.units.get_mut(&ours).unwrap().hp = 100;
    g.units.get_mut(&a).unwrap().hp = 20;
    g.units.get_mut(&b).unwrap().hp = 20;
    assert_eq!(ai.movement_risk_frame(&g, 0, ours, city_pos).ready, 0);
}

#[test]
fn movement_risk_route_escapes_fire_instead_of_holding_to_heal() {
    let (mut g, pos) = board();
    let ours = g.spawn_test_unit("warrior", 0, pos);
    g.map.tiles.get_mut(&pos).unwrap().feature = Some(crate::name!("burning_forest"));
    let target = g
        .wdisk(pos, 4)
        .into_iter()
        .find(|p| g.wdist(*p, pos) == 4)
        .unwrap();
    let ai = BasicAi::new();
    assert_eq!(
        ai.risk_aware_route_step(&mut g, 0, ours, target, 0),
        Some(true)
    );
    assert_ne!(g.units[&ours].pos, pos);
    assert_eq!(
        BasicAi::movement_hazard_damage(&g, 0, g.units[&ours].pos),
        0.0
    );
}

#[test]
fn movement_risk_tactical_advance_avoids_a_burning_shortcut() {
    let (mut g, pos) = board();
    let ours = g.spawn_test_unit("warrior", 0, pos);
    let target = g
        .wdisk(pos, 4)
        .into_iter()
        .find(|p| g.wdist(*p, pos) == 4)
        .unwrap();
    let shortcut = g
        .nbrs(pos)
        .into_iter()
        .find(|p| g.wdist(*p, target) < 4)
        .unwrap();
    g.map.tiles.get_mut(&shortcut).unwrap().feature = Some(crate::name!("burning_forest"));
    let ai = BasicAi::new();
    assert!(ai.tactical_step(&mut g, 0, ours, target, &[1], 1));
    assert_ne!(g.units[&ours].pos, shortcut);
    assert!(g.wdist(g.units[&ours].pos, target) <= 4);
}

#[test]
fn movement_risk_army_enters_city_range_together() {
    let (mut g, target) = board();
    let cid = g.found_city_for(1, target, None);
    g.players[0].explored.insert(target);
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let outside: Vec<_> = g
        .wdisk(target, 3)
        .into_iter()
        .filter(|p| g.wdist(*p, target) == 3)
        .collect();
    let ours = g.spawn_test_unit("swordsman", 0, outside[0]);
    let ai = BasicAi::new();
    let mut solo = g.clone();
    ai.tactical_step(&mut solo, 0, ours, target, &[1], 1);
    assert!(
        g.wdist(solo.units[&ours].pos, target) >= 3,
        "a lone attacker stages outside walls"
    );
    let a = g.spawn_test_unit("swordsman", 0, outside[2]);
    let b = g.spawn_test_unit("swordsman", 0, outside[4]);
    let frame = ai.movement_risk_frame(&g, 0, ours, target);
    assert_eq!(
        frame.ready,
        3,
        "target {:?}, city {:?}, assigned {:?}; reachable {:?}",
        target,
        frame.target_city,
        frame.assault_positions,
        [ours, a, b].map(|id| (
            g.units[&id].pos,
            g.reachable(id)
                .into_iter()
                .filter(|p| g.wdist(*p, target) <= 1)
                .collect::<Vec<_>>()
        ))
    );
    for id in [ours, a, b] {
        assert!(ai.tactical_step(&mut g, 0, id, target, &[1], 1));
        assert!(
            g.wdist(g.units[&id].pos, target) <= 2,
            "the ready army enters this turn"
        );
    }
}

#[test]
fn movement_risk_second_city_and_encampment_keep_independent_shots() {
    let (mut g, target) = board();
    let cid = g.found_city_for(1, target, None);
    g.players[0].explored.insert(target);
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let ring = g.nbrs(target);
    let ours = g.spawn_test_unit("swordsman", 0, ring[0]);
    g.spawn_test_unit("swordsman", 0, ring[2]);
    g.spawn_test_unit("swordsman", 0, ring[4]);
    let ai = BasicAi::new();
    let risk = ai.movement_risk_frame(&g, 0, ours, target);
    let single = risk.score(&g, 0, ours, ring[0], 0.5);
    let other = g
        .wdisk(ring[0], 2)
        .into_iter()
        .find(|p| g.wdist(*p, target) == 3)
        .unwrap();
    let second = g.found_city_for(1, other, None);
    g.cities.get_mut(&second).unwrap().wall_hp = 100;
    assert!(
        risk.score(&g, 0, ours, ring[0], 0.5) < single,
        "second city must add an independent shot"
    );
    // The independent encampment shot is keyed off its own walls and pillage state.
    let enc = ring[1];
    g.map.tiles.get_mut(&enc).unwrap().district = Some(crate::name!("encampment"));
    g.map.tiles.get_mut(&enc).unwrap().owner_city = Some(cid);
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("encampment"), enc);
    g.cities.get_mut(&cid).unwrap().encampment_hp = 100;
    g.cities.get_mut(&cid).unwrap().encampment_wall_hp = 100;
    let with_enc = risk.score(&g, 0, ours, ring[0], 0.5);
    g.cities.get_mut(&cid).unwrap().encampment_pillaged = true;
    assert!(
        risk.score(&g, 0, ours, ring[0], 0.5) > with_enc,
        "pillage removes only the encampment shot"
    );
}

#[test]
fn movement_risk_finishes_breached_walls_without_waiting_for_three_units() {
    let (mut g, target) = board();
    let cid = g.found_city_for(1, target, None);
    g.players[0].explored.insert(target);
    g.cities.get_mut(&cid).unwrap().wall_hp = 10;
    let outside = g
        .wdisk(target, 3)
        .into_iter()
        .find(|p| g.wdist(*p, target) == 3)
        .unwrap();
    let ours = g.spawn_test_unit("swordsman", 0, outside);
    let ai = BasicAi::new();
    assert_eq!(ai.movement_risk_frame(&g, 0, ours, target).ready, 1);
    assert!(ai.tactical_step(&mut g, 0, ours, target, &[1], 1));
    assert!(g.wdist(g.units[&ours].pos, target) <= 2);
}

#[test]
fn movement_risk_does_not_recruit_another_groups_units_for_a_siege() {
    let (mut g, target) = board();
    let cid = g.found_city_for(1, target, None);
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    let ring = g.nbrs(target);
    let ours = g.spawn_test_unit("swordsman", 0, ring[0]);
    g.spawn_test_unit("swordsman", 0, ring[2]);
    g.spawn_test_unit("swordsman", 0, ring[4]);
    let ai = BasicAi::new();
    assert_eq!(
        ai.movement_risk_frame_for_group(&g, 0, ours, target, Some(&[ours]))
            .ready,
        0
    );
}
