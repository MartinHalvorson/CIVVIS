use super::*;
use crate::game::install_test_district;
use std::sync::Arc;

/// A founder in live Emperor civvis-20261010T183646Z's shape near turn 57:
/// we founded Taoism in Quito (Holy Site and Shrine), every city of ours still
/// follows it, Mongolia's Catholicism is the faith the host names as Quito's
/// next majority, and no spreader of ours is in the field.
fn holy_city_named_by_a_rival() -> (Game, AdvancedAi, u32) {
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
    for x in [2, 8, 14, 20] {
        g.found_city_for(0, (x, 8), None);
    }
    let cities = g.player_city_ids(0);
    let quito = cities[3];
    for cid in &cities {
        g.cities
            .get_mut(cid)
            .unwrap()
            .pressure
            .insert("Taoism".into(), 100.0);
    }
    install_test_district(&mut g, quito, "holy_site");
    g.cities.get_mut(&quito).unwrap().buildings = vec![crate::name!("shrine")];
    g.cities.get_mut(&quito).unwrap().religion_next_seen = Some("Catholicism".into());
    g.players[0].techs.insert(crate::name!("astrology"));
    g.players[0].religion = Some("Taoism".into());
    g.players[0].holy_city = Some(quito);
    g.players[1].religion = Some("Catholicism".into());
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, "Catholicism".into());
    g.players[0].faith = 400.0;
    g.current = 0;
    g.turn = 57;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(g
        .unit_purchase_cost(0, quito, "missionary", "faith")
        .is_some());
    (g, ai, quito)
}

fn taoist_spreaders(g: &Game) -> Vec<u32> {
    g.units
        .values()
        .filter(|unit| {
            unit.owner == 0
                && unit.kind == "missionary"
                && unit.religion.as_deref() == Some("Taoism")
        })
        .map(|unit| unit.id)
        .collect()
}

/// No city of ours needs support by the shipped pressure test, so off nothing
/// is bought. On, the rival named as Quito's next faith buys a Taoist
/// Missionary in Quito, ahead of every other religious purchase.
#[test]
fn a_rival_named_next_buys_the_holy_city_a_missionary() {
    for gene in [false, true] {
        let (mut g, mut ai, quito) = holy_city_named_by_a_rival();
        if gene {
            ai.enable_founder_holds_the_holy_city();
        }
        ai.religious_spending_with_reserve(&mut g, 0, false, 80.0);
        let bought = taoist_spreaders(&g);
        if gene {
            assert_eq!(bought.len(), 1, "a Taoist Missionary was bought");
            assert!(
                g.wdist(g.units[&bought[0]].pos, g.cities[&quito].pos) <= 1,
                "bought in Quito"
            );
            assert!(g.players[0].faith < 400.0);
        } else {
            assert!(bought.is_empty(), "off: no defensive target, no purchase");
            assert_eq!(g.players[0].faith, 400.0);
        }
    }
}

/// A charged Taoist spreader beside Quito is the cover: nothing is bought, and
/// the spreader spends its charge in Quito instead of walking off. The next
/// turn it is free again: one spread per gap.
#[test]
fn a_spreader_near_the_holy_city_spreads_there_first() {
    let (mut g, mut ai, quito) = holy_city_named_by_a_rival();
    ai.enable_founder_holds_the_holy_city();
    let at = g.cities[&quito].pos;
    let uid = g.spawn_test_unit("missionary", 0, (at.0, at.1 + 1));
    g.units.get_mut(&uid).unwrap().religion = Some("Taoism".into());
    let charges = g.units[&uid].charges;
    assert!(charges > 0);

    ai.religious_spending_with_reserve(&mut g, 0, false, 80.0);
    assert_eq!(taoist_spreaders(&g).len(), 1, "covered: nothing bought");
    assert_eq!(g.players[0].faith, 400.0);

    assert_eq!(
        ai.holy_city_guard_step(&mut g, 0, uid, "Taoism"),
        Some(true)
    );
    assert_eq!(
        g.units[&uid].charges,
        charges - 1,
        "the charge went into Quito"
    );
    assert_eq!(
        ai.holy_city_guard_step(&mut g, 0, uid, "Taoism"),
        None,
        "the gap holds the next spread"
    );
}

/// The host's next faith flickers; the guard holds for the warning window
/// after the last sighting and lapses after it. A Holy City already lost, or
/// the gene off, is never guarded.
#[test]
fn the_warning_outlives_a_flicker_and_lapses() {
    let (mut g, mut ai, quito) = holy_city_named_by_a_rival();
    assert_eq!(ai.holy_city_at_risk(&g, 0), None, "off");
    ai.enable_founder_holds_the_holy_city();
    assert_eq!(ai.holy_city_at_risk(&g, 0), Some(quito));

    g.cities.get_mut(&quito).unwrap().religion_next_seen = None;
    g.turn += 1;
    assert_eq!(
        ai.holy_city_at_risk(&g, 0),
        Some(quito),
        "a flicker keeps it"
    );
    g.turn += g.standard_duration(HOLY_CITY_WARNING_STANDARD) + 1;
    assert_eq!(ai.holy_city_at_risk(&g, 0), None, "the window lapsed");

    let (mut g, mut ai, quito) = holy_city_named_by_a_rival();
    ai.enable_founder_holds_the_holy_city();
    let city = g.cities.get_mut(&quito).unwrap();
    city.pressure.clear();
    city.pressure.insert("Catholicism".into(), 100.0);
    assert_eq!(ai.holy_city_at_risk(&g, 0), None, "already Catholic");
}
