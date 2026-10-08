use super::*;
use crate::game::install_test_district;
use std::sync::Arc;

/// A founder in live Emperor 20261008T100610Z's shape near turn 55: we
/// founded Taoism in Quito (Holy Site and Shrine), Quito and Caracas follow
/// it, Bogota and Guayaquil already follow Mongolia's Catholicism, and a
/// held Catholic Missionary of ours stands beside a Taoist one.
fn founder_under_conversion() -> (Game, AdvancedAi, u32) {
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
    for (cid, faith) in [
        (cities[0], "Catholicism"),
        (cities[1], "Catholicism"),
        (cities[2], "Taoism"),
        (quito, "Taoism"),
    ] {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .pressure
            .insert(faith.into(), 100.0);
    }
    install_test_district(&mut g, quito, "holy_site");
    g.cities.get_mut(&quito).unwrap().buildings = vec![crate::name!("shrine")];
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].religion = Some("Taoism".into());
    g.players[0].holy_city = Some(quito);
    g.players[1].religion = Some("Catholicism".into());
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Catholicism".into());
    g.players[0].faith = 400.0;
    g.current = 0;
    g.turn = 55;
    for faith in ["Taoism", "Catholicism"] {
        let uid = g.spawn_test_unit("missionary", 0, (5, 10));
        g.units.get_mut(&uid).unwrap().religion = Some(faith.into());
    }
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(g
        .unit_purchase_cost(0, quito, "missionary", "faith")
        .is_some());
    (g, ai, quito)
}

fn taoist_missionaries(g: &Game) -> usize {
    g.units
        .values()
        .filter(|unit| {
            unit.owner == 0
                && unit.kind == "missionary"
                && unit.religion.as_deref() == Some("Taoism")
        })
        .count()
}

/// Two rival-held cities: the corps is one Taoist Missionary, not two, and
/// the cap is three, so the founder buys Taoist Missionaries in the Holy
/// City. Off, the held Catholic Missionary fills the shipped cap of two and
/// nothing is bought.
#[test]
fn a_founder_buys_its_own_defenders_once_a_rival_faith_holds_a_city() {
    for gene in [false, true] {
        let (mut g, mut ai, _) = founder_under_conversion();
        if gene {
            ai.enable_founder_defends_its_cities();
        }
        assert_eq!(AdvancedAi::rival_held_cities(&g, 0), 2);
        ai.religious_spending_with_reserve(&mut g, 0, false, 80.0);
        if gene {
            assert!(taoist_missionaries(&g) > 1, "a Taoist defender was bought");
            assert!(g.players[0].faith < 400.0);
        } else {
            assert_eq!(taoist_missionaries(&g), 1, "off: the cap is full");
            assert_eq!(g.players[0].faith, 400.0);
        }
    }
}

/// The corps and the cap read the gene only while a rival faith holds a
/// city of ours; a founder converting others is untouched.
#[test]
fn the_gene_is_idle_without_a_rival_held_city() {
    let (mut g, mut ai, quito) = founder_under_conversion();
    ai.enable_founder_defends_its_cities();
    assert!(ai.founder_defence_live(&g, 0));
    assert_eq!(ai.founder_corps_count(&g, 0, "missionary", "Taoism"), 1);
    assert_eq!(ai.founder_defence_missionary_cap(&g, 0, 2), 3);
    assert_eq!(ai.founder_purchase_order(&g, 0)[0], quito);

    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pressure.clear();
        city.pressure.insert("Taoism".into(), 100.0);
    }
    assert_eq!(AdvancedAi::rival_held_cities(&g, 0), 0);
    assert!(!ai.founder_defence_live(&g, 0));
    assert_eq!(ai.founder_corps_count(&g, 0, "missionary", "Taoism"), 2);
    assert_eq!(ai.founder_defence_missionary_cap(&g, 0, 2), 2);
    assert_eq!(ai.founder_purchase_order(&g, 0), g.player_city_ids(0));
}

/// An Inquisition unit more than five turns of Faith away holds nothing
/// back; one within reach still does. With the gene off the shipped saving
/// stands.
#[test]
fn an_unreachable_inquisition_does_not_hold_the_bank() {
    let (g, mut ai, _) = founder_under_conversion();
    assert!(!ai.founder_inquisition_out_of_reach(&g, 0, 10_000.0), "off");
    ai.enable_founder_defends_its_cities();
    let income: f64 = g
        .player_city_ids(0)
        .into_iter()
        .map(|cid| g.city_yields(cid).faith)
        .sum();
    let faith = g.players[0].faith;
    let near = faith + FOUNDER_SAVE_TURNS * income.max(0.0);
    assert!(!ai.founder_inquisition_out_of_reach(&g, 0, near));
    assert!(ai.founder_inquisition_out_of_reach(&g, 0, near + 1.0));
    // No city of our faith sells an Apostle (no Temple): nothing to save for.
    assert!(ai.founder_inquisition_unit_out_of_reach(&g, 0, "apostle"));
}
