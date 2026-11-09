use super::*;

/// A healthy Native-strength capital with an adjacent hostile and friendly
/// troops four to six hexes away. Their aggregate strength is not immediate
/// protection from the attack which the enemy can actually execute.
fn fixture(attacker_kind: &str) -> (Game, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 478_401, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let cid = g.found_city_for(0, (8, 10), None);
    g.current = 0;
    g.turn = 90;
    g.at_war.insert((0, 1));
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(cid, 32.0);
    let enemy = g.spawn_test_unit(attacker_kind, 1, (9, 10));
    let distant: Vec<_> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|&p| (4..=6).contains(&g.wdist(p, (8, 10))))
        .collect();
    for pos in distant.into_iter().take(16) {
        if AdvancedAi::city_pressure(&g, 0, cid) < BASTION_PRESSURE {
            break;
        }
        g.spawn_test_unit("warrior", 0, pos);
    }
    let pressure = AdvancedAi::city_pressure(&g, 0, cid);
    assert!(
        (IMMINENT_ATTACK_PRESSURE..BASTION_PRESSURE).contains(&pressure),
        "fixture pressure {pressure}"
    );
    assert_eq!(g.cities[&cid].hp, 200);
    assert_eq!(g.cities[&cid].wall_hp, g.city_max_wall_hp(&g.cities[&cid]));
    (g, cid, enemy)
}

#[test]
fn imminent_pressure_floor_recalls_for_every_victory_target() {
    let (g, cid, _) = fixture("knight");
    assert!(AdvancedAi::imminent_city_attack(
        &g,
        0,
        cid,
        &g.player_vision_frame(0)
    ));
    for target in VictoryTarget::ALL {
        let mut ai = AdvancedAi::targeting(target);
        ai.battlefront_observation = true;
        assert_eq!(
            ai.threatened_city(&g, 0),
            Some(cid),
            "target {target:?}: a strong executable attack must honor the lower floor"
        );
    }
}

#[test]
fn pressure_below_the_lower_floor_does_not_recall_the_empire() {
    let (mut g, cid, _) = fixture("knight");
    let spots: Vec<_> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|&p| (4..=6).contains(&g.wdist(p, (8, 10))) && g.units.values().all(|u| u.pos != p))
        .collect();
    for pos in spots.into_iter().take(16) {
        if AdvancedAi::city_pressure(&g, 0, cid) < IMMINENT_ATTACK_PRESSURE {
            break;
        }
        g.spawn_test_unit("warrior", 0, pos);
    }
    assert!(AdvancedAi::city_pressure(&g, 0, cid) < IMMINENT_ATTACK_PRESSURE);
    assert!(AdvancedAi::imminent_city_attack(
        &g,
        0,
        cid,
        &g.player_vision_frame(0)
    ));
    let mut ai = AdvancedAi::new();
    ai.battlefront_observation = true;
    assert_eq!(ai.threatened_city(&g, 0), None);
}

#[test]
fn weak_legal_attack_does_not_use_the_lower_floor() {
    let (g, cid, enemy) = fixture("horseman");
    assert!(g.attack_reach(enemy).contains(&g.cities[&cid].pos));
    assert!(!AdvancedAi::imminent_city_attack(
        &g,
        0,
        cid,
        &g.player_vision_frame(0)
    ));
    let mut ai = AdvancedAi::new();
    ai.battlefront_observation = true;
    assert_eq!(ai.threatened_city(&g, 0), None);
}

#[test]
fn a_blocked_attack_route_is_not_an_imminent_city_attack() {
    let (mut g, cid, enemy) = fixture("knight");
    g.units.get_mut(&enemy).unwrap().pos = (10, 10);
    g.spawn_test_unit("scout", 0, (11, 10));
    for pos in g.nbrs((8, 10)) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
    }
    assert!(!g.attack_reach(enemy).contains(&g.cities[&cid].pos));
    let mut ai = AdvancedAi::new();
    ai.battlefront_observation = true;
    assert_eq!(ai.threatened_city(&g, 0), None);
}

#[test]
fn disabled_observation_and_peace_keep_the_old_threshold() {
    let (mut g, _cid, _) = fixture("knight");
    let mut ai = AdvancedAi::new();
    ai.battlefront_observation = false;
    assert_eq!(ai.threatened_city(&g, 0), None);
    ai.battlefront_observation = true;
    g.at_war.clear();
    assert_eq!(ai.threatened_city(&g, 0), None);
}
