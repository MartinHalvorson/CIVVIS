use super::*;
use crate::game::install_test_district;

/// Live Emperor G215 (civvis-20261006T092204Z) around turn 65: we founded
/// Buddhism at 47 in Bogotá (Holy Site, Shrine, Temple from 57; Theology at
/// 52); four of five cities follow it, one follows Georgia's Orthodoxy, and
/// an Orthodox Missionary stands two tiles from Bogotá. The bank pays for a
/// cover Missionary, not for the Apostle.
fn g215_turn_65() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(4, 40, 24, 372_104, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    for x in [2, 8, 14, 20, 26] {
        g.found_city_for(0, (x, 8), None);
    }
    let cities = g.player_city_ids(0);
    let (bogota, orthodox) = (cities[0], cities[4]);
    for cid in &cities[..4] {
        g.cities
            .get_mut(cid)
            .unwrap()
            .pressure
            .insert("Buddhism".into(), 1_000.0);
    }
    g.cities
        .get_mut(&orthodox)
        .unwrap()
        .pressure
        .insert("Orthodoxy".into(), 1_000.0);
    install_test_district(&mut g, bogota, "holy_site");
    g.cities.get_mut(&bogota).unwrap().buildings =
        vec![crate::name!("shrine"), crate::name!("temple")];
    g.players[0].religion = Some("Buddhism".into());
    g.players[0].holy_city = Some(bogota);
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].civics.insert(crate::name!("theology"));
    g.players[1].religion = Some("Orthodoxy".into());
    let spreader = g.spawn_test_unit("missionary", 1, (4, 8));
    g.units.get_mut(&spreader).unwrap().religion = Some("Orthodoxy".into());
    g.current = 0;
    g.turn = 65;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let missionary = g
        .unit_purchase_cost(0, bogota, "missionary", "faith")
        .unwrap();
    let apostle = g.unit_purchase_cost(0, bogota, "apostle", "faith").unwrap();
    assert!(missionary < apostle);
    g.players[0].faith = (missionary + apostle) / 2.0;
    assert!(ai.defensive_inquisition_ready(&g, 0));
    (g, ai, bogota, orthodox)
}

fn religious_units(g: &Game) -> usize {
    g.player_unit_ids(0)
        .into_iter()
        .filter(|uid| g.rules.units[g.units[uid].kind].class == "religious")
        .count()
}

#[test]
fn the_cover_missionary_waits_for_the_apostle_under_the_gene() {
    for gene in [false, true] {
        let (mut g, mut ai, _, _) = g215_turn_65();
        if gene {
            ai.enable_founder_funds_the_inquisition();
        }
        let faith = g.players[0].faith;
        assert!(ai.prepare_defensive_inquisition(&mut g, 0));
        if gene {
            assert_eq!(religious_units(&g), 0, "the bank waits for the Apostle");
            assert_eq!(g.players[0].faith, faith);
        } else {
            assert_eq!(
                religious_units(&g),
                1,
                "G215: a cover Missionary spends the Apostle's savings"
            );
            assert!(g.players[0].faith < faith);
        }
    }
}

#[test]
fn the_apostle_is_bought_as_soon_as_it_is_affordable() {
    let (mut g, mut ai, bogota, _) = g215_turn_65();
    ai.enable_founder_funds_the_inquisition();
    g.players[0].faith = g.unit_purchase_cost(0, bogota, "apostle", "faith").unwrap();
    assert!(ai.prepare_defensive_inquisition(&mut g, 0));
    assert!(g
        .units
        .values()
        .any(|unit| unit.owner == 0 && unit.kind == "apostle"));
}

#[test]
fn the_reserve_names_the_apostle_then_the_inquisitors() {
    let (mut g, mut ai, bogota, _) = g215_turn_65();
    assert_eq!(
        ai.inquisition_faith_reserve(&g, 0),
        0.0,
        "off: nothing held"
    );
    ai.enable_founder_funds_the_inquisition();
    let apostle = g.unit_purchase_cost(0, bogota, "apostle", "faith").unwrap();
    assert_eq!(ai.inquisition_faith_reserve(&g, 0), apostle);

    // An Apostle of ours in the field: it launches; nothing more is held.
    let ours = g.spawn_test_unit("apostle", 0, (8, 10));
    g.units.get_mut(&ours).unwrap().religion = Some("Buddhism".into());
    assert_eq!(ai.inquisition_faith_reserve(&g, 0), 0.0);

    // Launched: Inquisitors up to the shipped cap of two.
    g.remove_unit(ours);
    g.players[0].counters.insert("inquisition".into(), 1);
    let inquisitor = g
        .unit_purchase_cost(0, bogota, "inquisitor", "faith")
        .unwrap();
    assert_eq!(ai.inquisition_faith_reserve(&g, 0), inquisitor);
    for x in [9, 11] {
        let uid = g.spawn_test_unit("inquisitor", 0, (x, 10));
        g.units.get_mut(&uid).unwrap().religion = Some("Buddhism".into());
    }
    assert_eq!(ai.inquisition_faith_reserve(&g, 0), 0.0, "cap met");
}

#[test]
fn without_a_rival_faith_at_home_nothing_is_held() {
    let (mut g, mut ai, _, orthodox) = g215_turn_65();
    ai.enable_founder_funds_the_inquisition();
    g.cities.get_mut(&orthodox).unwrap().pressure.clear();
    g.cities
        .get_mut(&orthodox)
        .unwrap()
        .pressure
        .insert("Buddhism".into(), 1_000.0);
    let spreader = g
        .units
        .values()
        .find(|unit| unit.owner == 1 && unit.kind == "missionary")
        .unwrap()
        .id;
    g.remove_unit(spreader);
    assert!(!ai.rival_faith_at_home(&g, 0));
    assert_eq!(ai.inquisition_faith_reserve(&g, 0), 0.0);
}

#[test]
fn a_seat_with_no_religion_is_untouched() {
    let (mut g, mut ai, _, _) = g215_turn_65();
    ai.enable_founder_funds_the_inquisition();
    g.players[0].religion = None;
    assert!(!ai.rival_faith_at_home(&g, 0));
    assert_eq!(ai.inquisition_first_price(&g, 0), None);
}

#[test]
fn founder_funds_the_inquisition_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "founder-funds-the-inquisition",
        |ai| ai.founder_funds_the_inquisition,
    );
}

/// The host's purchase menu lists only what the bank can pay for now, and
/// the mirror takes the menu as the answer: on the live board of G215 at
/// turn 65 Bogotá's menu had a Missionary and no Apostle, so the Apostle had
/// no quote. The reserve and the saving still stand on the model's price.
#[test]
fn an_apostle_missing_from_the_host_menu_is_still_saved_for() {
    let (mut g, mut ai, bogota, _) = g215_turn_65();
    ai.enable_founder_funds_the_inquisition();
    let missionary = g
        .unit_purchase_cost(0, bogota, "missionary", "faith")
        .unwrap();
    let key = Game::production_block_key(&Item::Unit {
        unit: crate::name!("missionary"),
    });
    std::sync::Arc::make_mut(&mut g.host_purchasable).insert(
        bogota,
        std::collections::BTreeMap::from([(
            key,
            crate::game::HostPurchaseEntry {
                gold: None,
                faith: Some(missionary),
            },
        )]),
    );
    assert_eq!(g.unit_purchase_cost(0, bogota, "apostle", "faith"), None);
    let estimate = g.game_speed.scale(g.rules.units["apostle"].cost * 2.0);
    assert_eq!(ai.inquisition_faith_reserve(&g, 0), estimate);
    let faith = g.players[0].faith;
    assert!(ai.prepare_defensive_inquisition(&mut g, 0));
    assert_eq!(religious_units(&g), 0, "no cover Missionary from the menu");
    assert_eq!(g.players[0].faith, faith);
}
