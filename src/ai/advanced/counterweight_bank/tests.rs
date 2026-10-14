use super::*;
use crate::game::install_test_district;
use std::sync::Arc;

/// Live Emperor G200 (civvis-20261006T061051Z) from turn 103, scaled to five
/// cities: Mongolia's Buddhism holds Mongolia, the Mapuche and four of our
/// five cities; Quito follows the Mapuche's Confucianism and has the Holy
/// Site and Shrine that sell its Missionaries; we have no religion and a
/// full Faith bank.
fn g200_turn_103() -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(4, 40, 24, 372_102, 250, 0, false);
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
    let quito = cities[4];
    for cid in &cities[..4] {
        g.cities
            .get_mut(cid)
            .unwrap()
            .pressure
            .insert("Buddhism".into(), 100.0);
    }
    g.cities
        .get_mut(&quito)
        .unwrap()
        .pressure
        .insert("Confucianism".into(), 100.0);
    install_test_district(&mut g, quito, "holy_site");
    g.cities.get_mut(&quito).unwrap().buildings = vec![crate::name!("shrine")];
    g.players[0].civics.insert(crate::name!("theology"));
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[1].religion = Some("Buddhism".into());
    g.players[2].religion = Some("Confucianism".into());
    for pid in [1, 2] {
        Arc::make_mut(&mut g.observed_majority_religion).insert(pid, "Buddhism".into());
    }
    g.players[0].faith = 565.0;
    g.current = 0;
    g.turn = 103;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(g.civ_follows_religion(0, "Buddhism"));
    assert_eq!(g.city_religion(&g.cities[&quito]), Some("Confucianism"));
    assert!(ai.counterfaith_is_safe(&g, 0, "Confucianism"));
    assert!(g
        .unit_purchase_cost(0, quito, "missionary", "faith")
        .is_some());
    (g, ai, quito)
}

fn counterweight_missionaries(g: &Game) -> usize {
    g.units
        .values()
        .filter(|unit| {
            unit.owner == 0
                && unit.kind == "missionary"
                && unit.religion.as_deref() == Some("Confucianism")
        })
        .count()
}

/// Two charged Confucian Missionaries in the field: the shipped cap.
fn two_in_the_field(g: &mut Game, _quito: u32) {
    // Away from Quito's tile, which a purchase must find free.
    let pos = (5, 10);
    for _ in 0..2 {
        let uid = g.spawn_test_unit("missionary", 0, pos);
        g.units.get_mut(&uid).unwrap().religion = Some("Confucianism".into());
        assert!(g.units[&uid].charges > 0);
    }
}

#[test]
fn the_counterweight_cap_reads_the_cities_the_majority_rests_on() {
    let (g, mut ai, _) = g200_turn_103();
    // Four of five Buddhist: two must leave it before its majority breaks.
    assert_eq!(AdvancedAi::counterweight_need(&g, 0, "Buddhism"), 2);
    assert_eq!(
        ai.counterweight_cap(&g, 0, "Buddhism", 2),
        2,
        "off: shipped"
    );
    ai.enable_counterweight_spends_the_bank();
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 3);
    assert_eq!(
        ai.counterweight_cap(&g, 0, "Buddhism", 4),
        4,
        "never fewer than shipped"
    );
    assert_eq!(
        ai.counterweight_cap(&g, 0, "Confucianism", 2),
        2,
        "no majority to break"
    );
}

#[test]
fn a_counterweight_missionary_beyond_two_is_bought_under_the_gene() {
    for gene in [false, true] {
        let (mut g, mut ai, quito) = g200_turn_103();
        if gene {
            ai.enable_counterweight_spends_the_bank();
        }
        two_in_the_field(&mut g, quito);
        ai.religious_defense(&mut g, 0, "Buddhism");
        if gene {
            assert_eq!(counterweight_missionaries(&g), 3);
            assert!(g.players[0].faith < 565.0);
        } else {
            assert_eq!(
                counterweight_missionaries(&g),
                2,
                "G200: two Missionaries were the shipped cap"
            );
            assert_eq!(g.players[0].faith, 565.0);
        }
    }
}

#[test]
fn the_other_faith_sinks_leave_the_counterweight_its_price() {
    let (mut g, mut ai, quito) = g200_turn_103();
    assert_eq!(
        ai.counterweight_faith_reserve(&g, 0),
        0.0,
        "off: nothing held"
    );
    ai.enable_counterweight_spends_the_bank();
    let price = g
        .unit_purchase_cost(0, quito, "missionary", "faith")
        .unwrap();
    // Cap three, none in the field.
    assert_eq!(ai.counterweight_faith_reserve(&g, 0), 3.0 * price);
    two_in_the_field(&mut g, quito);
    assert_eq!(ai.counterweight_faith_reserve(&g, 0), price);
    let third = g.spawn_test_unit("missionary", 0, (11, 10));
    g.units.get_mut(&third).unwrap().religion = Some("Confucianism".into());
    assert_eq!(ai.counterweight_faith_reserve(&g, 0), 0.0, "cap met");
}

#[test]
fn no_source_or_no_majority_holds_no_faith() {
    // Without a city that sells a counterweight Missionary nothing is held.
    let (mut g, mut ai, quito) = g200_turn_103();
    ai.enable_counterweight_spends_the_bank();
    g.cities.get_mut(&quito).unwrap().buildings.clear();
    assert!(g
        .unit_purchase_cost(0, quito, "missionary", "faith")
        .is_none());
    assert_eq!(ai.counterweight_faith_reserve(&g, 0), 0.0);

    // Buddhism on two of five cities holds no majority over us.
    let (mut g, mut ai, _) = g200_turn_103();
    ai.enable_counterweight_spends_the_bank();
    for cid in g.player_city_ids(0).into_iter().take(2) {
        g.cities.get_mut(&cid).unwrap().pressure.clear();
    }
    assert_eq!(AdvancedAi::counterweight_need(&g, 0, "Buddhism"), 0);
    assert_eq!(ai.counterweight_faith_reserve(&g, 0), 0.0);
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 2);
}

#[test]
fn a_founder_is_untouched() {
    let (mut g, mut ai, _) = g200_turn_103();
    ai.enable_counterweight_spends_the_bank();
    g.players[0].religion = Some("Taoism".into());
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 2);
    assert_eq!(ai.counterweight_faith_reserve(&g, 0), 0.0);
}

#[test]
fn counterweight_spends_the_bank_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "counterweight-spends-the-bank",
        |ai| ai.counterweight_spends_the_bank,
    );
}
