use super::super::*;

fn fixture(held_faith: &str) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(3, 32, 24, 367_200, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let home =
        [(4, 4), (10, 4), (4, 10), (10, 10), (16, 10)].map(|pos| g.found_city_for(0, pos, None));
    let rival = g.found_city_for(1, (22, 4), None);
    let counter = g.found_city_for(2, (22, 12), None);
    let source = home[4];
    g.players[1].religion = Some("Orthodoxy".into());
    g.players[2].religion = Some("Buddhism".into());
    for cid in home.into_iter().chain([rival, counter]) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.atheist_pressure = 0.0;
        city.pressure.clear();
        city.pressure.insert(
            if cid == source || cid == counter {
                "Buddhism"
            } else {
                "Orthodoxy"
            }
            .into(),
            1000.0,
        );
    }
    crate::game::install_test_district(&mut g, source, "holy_site");
    g.cities
        .get_mut(&source)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].faith = 1000.0;
    g.current = 0;
    g.turn = 121;
    for pos in [(4, 5), (5, 5)] {
        let uid = g.spawn_test_unit("missionary", 0, pos);
        g.units.get_mut(&uid).unwrap().religion = Some(held_faith.into());
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.religious_veto_defence = false;
    assert_eq!(ai.adopted_faith_threat(&g, 0).as_deref(), Some("Orthodoxy"));
    assert!(g
        .unit_purchase_cost(0, source, "missionary", "faith")
        .is_some());
    (g, ai, source)
}

#[test]
fn held_threat_missionaries_do_not_block_a_counterfaith_purchase() {
    let (mut g, mut ai, _) = fixture("Orthodoxy");
    for uid in g.player_unit_ids(0) {
        assert!(!ai.advanced_missionary_step(&mut g, 0, uid, false));
        assert_eq!(g.units[&uid].charges, 3);
    }
    let before = g.units.len();
    ai.religious_defense(&mut g, 0, "Orthodoxy");
    assert_eq!(g.units.len(), before + 1);
    assert!(g.units.values().any(|unit| unit.owner == 0
        && unit.kind == "missionary"
        && unit.religion.as_deref() == Some("Buddhism")));
}

#[test]
fn two_usable_counterfaith_missionaries_still_fill_the_limit() {
    let (mut g, mut ai, _) = fixture("Buddhism");
    let before = g.units.len();
    let faith = g.players[0].faith;
    ai.religious_defense(&mut g, 0, "Orthodoxy");
    assert_eq!(g.units.len(), before);
    assert_eq!(g.players[0].faith, faith);
}

#[test]
fn other_lanes_and_disabled_religious_victory_keep_their_roster_limit() {
    for disabled in [false, true] {
        let (mut g, mut ai, _) = fixture("Orthodoxy");
        if disabled {
            g.victory_conditions.religious = false;
        } else {
            ai.victory_target = Some(VictoryTarget::Science);
        }
        let before = g.units.len();
        ai.religious_defense(&mut g, 0, "Orthodoxy");
        assert_eq!(g.units.len(), before);
    }
}

#[test]
fn a_mixed_corps_buys_only_the_missing_counterfaith_defender() {
    let (mut g, mut ai, _) = fixture("Orthodoxy");
    let uid = g.spawn_test_unit("missionary", 0, (10, 5));
    g.units.get_mut(&uid).unwrap().religion = Some("Buddhism".into());
    let before = g.units.len();
    ai.religious_defense(&mut g, 0, "Orthodoxy");
    assert_eq!(g.units.len(), before + 1);
    let faith = g.players[0].faith;
    ai.religious_defense(&mut g, 0, "Orthodoxy");
    assert_eq!(g.units.len(), before + 1);
    assert_eq!(g.players[0].faith, faith);
}
