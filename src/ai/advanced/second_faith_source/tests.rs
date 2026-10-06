use super::*;
use crate::game::install_test_district;

/// Live Emperor G213 (civvis-20261006T084640Z) at turn 54: we founded
/// Buddhism at 42 in Bogotá (Holy Site, Shrine, Temple); Maracaibo follows it
/// and has a Holy Site without a Shrine (it never got one); Cali follows it
/// with neither; Austria's Confucianism exists. No conversion threat yet.
fn g213_turn_54() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(4, 40, 24, 372_103, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    for x in [2, 10, 18] {
        g.found_city_for(0, (x, 8), None);
    }
    let cities = g.player_city_ids(0);
    let (bogota, maracaibo) = (cities[0], cities[1]);
    for cid in &cities {
        g.cities
            .get_mut(cid)
            .unwrap()
            .pressure
            .insert("Buddhism".into(), 1_000.0);
    }
    install_test_district(&mut g, bogota, "holy_site");
    g.cities.get_mut(&bogota).unwrap().buildings =
        vec![crate::name!("shrine"), crate::name!("temple")];
    install_test_district(&mut g, maracaibo, "holy_site");
    g.players[0].religion = Some("Buddhism".into());
    g.players[0].holy_city = Some(bogota);
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].civics.insert(crate::name!("theology"));
    g.players[1].religion = Some("Confucianism".into());
    g.current = 0;
    g.turn = 54;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert_eq!(AdvancedAi::own_faith_sources(&g, 0), 1);
    assert!(ai.home_conversion_threat(&g, 0).is_none());
    (g, ai, bogota, maracaibo)
}

fn shrine() -> Item {
    Item::Building {
        building: crate::name!("shrine"),
    }
}

fn plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

#[test]
fn a_founder_starts_its_second_source_at_founding_under_the_gene() {
    let (g, mut ai, _, maracaibo) = g213_turn_54();
    assert_eq!(
        ai.adopted_faith_sanctuary_choice(&g, 0, None),
        None,
        "off: one Shrine city and no threat is the end of it"
    );
    ai.enable_founder_keeps_two_sources();
    assert!(ai.founder_wants_a_second_source(&g, 0));
    assert_eq!(
        ai.adopted_faith_sanctuary_choice(&g, 0, None),
        Some((maracaibo, shrine())),
        "the Shrine on Maracaibo's Holy Site is the soonest second source"
    );
}

#[test]
fn under_a_threat_the_holy_city_shrine_no_longer_ends_the_sanctuary() {
    let (mut g, mut ai, _, maracaibo) = g213_turn_54();
    // Confucian pressure at 80% of the top in Cali: a conversion threat.
    let cali = g.player_city_ids(0)[2];
    g.cities
        .get_mut(&cali)
        .unwrap()
        .pressure
        .insert("Confucianism".into(), 800.0);
    assert!(ai.home_conversion_threat(&g, 0).is_some());
    assert_eq!(ai.adopted_faith_sanctuary_choice(&g, 0, None), None);
    ai.enable_founder_keeps_two_sources();
    assert_eq!(
        ai.adopted_faith_sanctuary_choice(&g, 0, None),
        Some((maracaibo, shrine()))
    );
}

#[test]
fn the_sanctuary_stops_at_two_sources_and_without_a_rival_religion() {
    let (mut g, mut ai, _, maracaibo) = g213_turn_54();
    ai.enable_founder_keeps_two_sources();
    g.cities.get_mut(&maracaibo).unwrap().buildings = vec![crate::name!("shrine")];
    assert_eq!(AdvancedAi::own_faith_sources(&g, 0), 2);
    assert!(!ai.founder_wants_a_second_source(&g, 0));
    assert_eq!(ai.adopted_faith_sanctuary_choice(&g, 0, None), None);

    let (mut g, mut ai, _, _) = g213_turn_54();
    ai.enable_founder_keeps_two_sources();
    g.players[1].religion = None;
    assert!(
        !ai.founder_wants_a_second_source(&g, 0),
        "no rival religion"
    );
    assert_eq!(ai.adopted_faith_sanctuary_choice(&g, 0, None), None);
}

#[test]
fn a_seat_with_no_religion_is_untouched() {
    let (mut g, mut ai, _, _) = g213_turn_54();
    ai.enable_founder_keeps_two_sources();
    g.players[0].religion = None;
    assert_eq!(AdvancedAi::own_faith_sources(&g, 0), 0);
    assert!(!ai.founder_wants_a_second_source(&g, 0));
}

#[test]
fn the_second_source_is_queued_under_the_gene() {
    for gene in [false, true] {
        let (mut g, mut ai, _, maracaibo) = g213_turn_54();
        if gene {
            ai.enable_founder_keeps_two_sources();
        }
        let plan = plan(&g);
        ai.reserve_adopted_faith_sanctuary(&mut g, 0, &plan);
        let head = g.cities[&maracaibo].queue.first().cloned();
        if gene {
            assert_eq!(head, Some(shrine()));
        } else {
            assert_ne!(head, Some(shrine()));
        }
    }
}

#[test]
fn founder_keeps_two_sources_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers("founder-keeps-two-sources", |ai| {
        ai.founder_keeps_two_sources
    });
}
