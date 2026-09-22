use super::*;
use std::sync::Arc;

#[test]
fn observed_majorities_count_unseen_rivals_in_religious_defense() {
    let mut game = Game::new_full(3, 42, 24, 7_626, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.victory_conditions.religious = true;
    game.players[1].religion = Some("Runaway Faith".to_string());
    let mut ai = AdvancedAi::new();
    ai.enable_religious_veto_defence();

    // A mirror can know the host's majority without seeing any rival cities.
    assert!(game.player_city_ids(2).is_empty());
    assert!(ai.religious_veto_engaged(&game, 0).is_none());
    Arc::make_mut(&mut game.observed_majority_religion).insert(2, "Runaway Faith".to_string());
    let stakes = ai.religious_veto_engaged(&game, 0).unwrap();
    assert_eq!((stakes.founder, stakes.dominated, stakes.others), (1, 1, 1));
    assert_eq!(stakes.stake, 0.5);
    assert_eq!(
        ai.religious_veto_threat(&game, 0, None).as_deref(),
        Some("Runaway Faith")
    );
    assert!(!AdvancedAi::religious_veto_spends(Some(&stakes)));

    // Our actual city count still determines whether the threat warrants spending.
    let ours = game.player_city_ids(0)[0];
    game.cities
        .get_mut(&ours)
        .unwrap()
        .pressure
        .insert("Runaway Faith".to_string(), 800.0);
    let stakes = ai.religious_veto_engaged(&game, 0).unwrap();
    assert_eq!(stakes.stake, 1.0);
    assert!(AdvancedAi::religious_veto_spends(Some(&stakes)));

    Arc::make_mut(&mut game.observed_majority_religion).insert(2, "Other Faith".to_string());
    let stakes = ai.religious_veto_stakes(&game, 0).unwrap();
    assert_eq!(stakes.dominated, 0);
    assert!(!AdvancedAi::religious_veto_spends(Some(&stakes)));

    ai.disable_religious_veto_defence();
    assert!(ai.religious_veto_stakes(&game, 0).is_none());
}

fn source_guard_fixture() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = Game::new_full(2, 32, 20, 370200, 400, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let source = g.found_city_for(0, (10, 10), None);
    let distant = g.found_city_for(0, (3, 10), None);
    g.found_city_for(1, (22, 10), None);
    g.players[0].religion = Some("Home Faith".into());
    g.players[0].holy_city = Some(source);
    g.players[0].counters.insert("inquisition".into(), 1);
    g.players[1].religion = Some("Foreign Faith".into());
    for cid in [source, distant] {
        g.cities.get_mut(&cid).unwrap().pressure.clear();
    }
    let city = g.cities.get_mut(&source).unwrap();
    city.pressure.insert("Home Faith".into(), 1000.0);
    city.buildings
        .extend([crate::name!("shrine"), crate::name!("temple")]);
    city.districts.insert(crate::name!("holy_site"), (9, 10));
    let tile = g.map.tiles.get_mut(&(9, 10)).unwrap();
    tile.owner_city = Some(source);
    tile.district = Some(crate::name!("holy_site"));
    tile.pillaged = false;
    g.cities
        .get_mut(&distant)
        .unwrap()
        .pressure
        .insert("Foreign Faith".into(), 1000.0);
    let inquisitor = g.spawn_test_unit("inquisitor", 0, (10, 10));
    let invader = g.spawn_test_unit("missionary", 1, (12, 10));
    g.units.get_mut(&inquisitor).unwrap().religion = Some("Home Faith".into());
    g.units.get_mut(&inquisitor).unwrap().charges = 3;
    g.units.get_mut(&invader).unwrap().religion = Some("Foreign Faith".into());
    g.units.get_mut(&invader).unwrap().charges = 3;
    g.turn = 172;
    g.current = 0;
    let mut ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    ai.enable_religious_veto_defence();
    (g, ai, inquisitor, source, invader)
}

#[test]
fn inquisitor_keeps_the_last_threatened_purchase_source_covered() {
    let (mut g, ai, inquisitor, source, _) = source_guard_fixture();
    assert_eq!(g.city_religion(&g.cities[&source]), Some("Home Faith"));
    let legal = g.legal_actions(0);
    ai.inquisitor_veto_step(&mut g, 0, inquisitor, &legal);
    assert_eq!(
        g.units[&inquisitor].pos, g.cities[&source].pos,
        "the sole purchase source needs cover before the distant converted city"
    );
}

#[test]
fn source_guard_releases_when_the_spreader_is_exhausted_or_another_source_exists() {
    let (mut g, ai, inquisitor, source, invader) = source_guard_fixture();
    g.units.get_mut(&invader).unwrap().charges = 0;
    assert!(ai
        .inquisitor_purchase_source_guard(&g, 0, inquisitor, "Home Faith")
        .is_none());
    g.units.get_mut(&invader).unwrap().charges = 3;
    let other = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != source)
        .unwrap();
    let c = g.cities.get_mut(&other).unwrap();
    c.pressure.clear();
    c.pressure.insert("Home Faith".into(), 1000.0);
    c.buildings.push(crate::name!("shrine"));
    c.districts.insert(crate::name!("holy_site"), (3, 9));
    let t = g.map.tiles.get_mut(&(3, 9)).unwrap();
    t.owner_city = Some(other);
    t.district = Some(crate::name!("holy_site"));
    t.pillaged = false;
    assert!(ai
        .inquisitor_purchase_source_guard(&g, 0, inquisitor, "Home Faith")
        .is_none());
}

#[test]
fn source_guard_can_restore_the_sole_converted_source_and_assigns_only_one_defender() {
    let (mut g, ai, inquisitor, source, _) = source_guard_fixture();
    let c = g.cities.get_mut(&source).unwrap();
    c.pressure.insert("Foreign Faith".into(), 2000.0);
    let source_pos = c.pos;
    assert_eq!(
        ai.inquisitor_purchase_source_guard(&g, 0, inquisitor, "Home Faith"),
        Some(source_pos)
    );
    let second = g.spawn_test_unit("inquisitor", 0, (11, 10));
    g.units.get_mut(&second).unwrap().religion = Some("Home Faith".into());
    g.units.get_mut(&second).unwrap().charges = 3;
    assert!(ai
        .inquisitor_purchase_source_guard(&g, 0, second, "Home Faith")
        .is_none());
    let legal = g.legal_actions(0);
    ai.inquisitor_veto_step(&mut g, 0, inquisitor, &legal);
    assert!(g.cities[&source].pressure["Foreign Faith"] < 2000.0);
}

#[test]
fn source_guard_covers_an_apostle_one_move_from_spread_range() {
    let (mut g, ai, inquisitor, source, old_invader) = source_guard_fixture();
    g.remove_unit(old_invader);
    let invader = g.spawn_test_unit("apostle", 1, (15, 10));
    let unit = g.units.get_mut(&invader).unwrap();
    unit.religion = Some("Foreign Faith".into());
    unit.charges = 3;
    // The rival has spent this turn's movement, but refreshes it before the
    // source can next react. Native Quito lost its guard at this distance.
    unit.moves_left = 0.0;
    assert_eq!(g.unit_max_moves(invader), 4.0);
    let source_pos = g.cities[&source].pos;
    assert_eq!(g.wdist(g.units[&invader].pos, source_pos), 5);
    let legal = g.legal_actions(0);
    ai.inquisitor_veto_step(&mut g, 0, inquisitor, &legal);
    assert_eq!(g.units[&inquisitor].pos, source_pos,
        "do not send the only source guard to a distant converted city when an Apostle can reach spread range next turn");
}

#[test]
fn source_guard_uses_the_spreaders_full_movement_bonus() {
    let (mut g, ai, inquisitor, source, old_invader) = source_guard_fixture();
    g.remove_unit(old_invader);
    let invader = g.spawn_test_unit("apostle", 1, (17, 10));
    let unit = g.units.get_mut(&invader).unwrap();
    unit.religion = Some("Foreign Faith".into());
    unit.charges = 3;
    unit.bonus_moves = 2.0;
    assert_eq!(g.unit_max_moves(invader), 6.0);
    assert_eq!(
        ai.inquisitor_purchase_source_guard(&g, 0, inquisitor, "Home Faith"),
        Some(g.cities[&source].pos)
    );
}

#[test]
fn source_guard_releases_beyond_movement_and_spread_warning() {
    let (mut g, ai, inquisitor, _, old_invader) = source_guard_fixture();
    g.remove_unit(old_invader);
    let invader = g.spawn_test_unit("apostle", 1, (16, 10));
    let unit = g.units.get_mut(&invader).unwrap();
    unit.religion = Some("Foreign Faith".into());
    unit.charges = 3;
    assert_eq!(g.unit_max_moves(invader), 4.0);
    assert!(ai
        .inquisitor_purchase_source_guard(&g, 0, inquisitor, "Home Faith")
        .is_none());
}
