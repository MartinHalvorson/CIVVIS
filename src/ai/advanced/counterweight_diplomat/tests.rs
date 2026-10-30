use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::install_test_district;
use std::sync::Arc;

/// The `counterweight-spends-the-bank` board (live Emperor G200 from turn
/// 103, five cities) on `lane`: Mongolia's Buddhism holds Mongolia, the
/// Mapuche and four of our five cities; Quito follows the Mapuche's
/// Confucianism and has the Holy Site and Shrine that sell its Missionaries.
/// We have no religion and a full Faith bank.
fn g200_turn_103(lane: VictoryTarget) -> (Game, AdvancedAi, u32) {
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
    let mut ai = AdvancedAi::targeting(lane);
    ai.enable_counterweight_spends_the_bank();
    assert!(g.civ_follows_religion(0, "Buddhism"));
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

fn two_in_the_field(g: &mut Game) {
    for _ in 0..2 {
        let uid = g.spawn_test_unit("missionary", 0, (5, 10));
        g.units.get_mut(&uid).unwrap().religion = Some("Confucianism".into());
    }
}

#[test]
fn counterweight_defends_the_diplomat_is_opt_in() {
    opt_in_off_in_both_controllers("counterweight-defends-the-diplomat", |ai| {
        ai.counterweight_defends_the_diplomat
    });
}

/// The Domination lane is answered either way; the Diplomacy lane only under
/// the gene; the Culture lane never.
#[test]
fn the_gene_opens_the_diplomacy_lane_only() {
    let (g, mut ai, _) = g200_turn_103(VictoryTarget::Diplomacy);
    assert!(!ai.counterweight_lane(&g));
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 2, "off: shipped");
    ai.enable_counterweight_defends_the_diplomat();
    assert!(ai.counterweight_lane(&g));
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 3);

    let (g, mut ai, _) = g200_turn_103(VictoryTarget::Culture);
    ai.enable_counterweight_defends_the_diplomat();
    assert!(!ai.counterweight_lane(&g));
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 2);

    let (g, ai, _) = g200_turn_103(VictoryTarget::Domination);
    assert!(ai.counterweight_lane(&g));
}

/// 171157Z: our own faith holds none of our cities. Stock, the founder path
/// has nothing it can buy and the bank stays whole; under the gene the seat
/// defends as a faithless one and buys the Confucian counterweight.
#[test]
fn a_founder_with_no_city_of_its_faith_buys_the_counterweight() {
    for gene in [false, true] {
        let (mut g, mut ai, _) = g200_turn_103(VictoryTarget::Diplomacy);
        g.players[0].religion = Some("Catholicism".into());
        if gene {
            ai.enable_counterweight_defends_the_diplomat();
        }
        assert_eq!(ai.counterweight_faithless(&g, 0), gene);
        two_in_the_field(&mut g);
        ai.religious_defense(&mut g, 0, "Buddhism");
        if gene {
            assert_eq!(counterweight_missionaries(&g), 3);
            assert!(g.players[0].faith < 565.0);
        } else {
            assert_eq!(counterweight_missionaries(&g), 2);
            assert_eq!(g.players[0].faith, 565.0, "stock: the bank stays whole");
        }
    }
}

/// A founder whose faith still holds a city of ours keeps its own defence.
#[test]
fn a_founder_with_a_city_of_its_faith_is_untouched() {
    let (mut g, mut ai, quito) = g200_turn_103(VictoryTarget::Diplomacy);
    ai.enable_counterweight_defends_the_diplomat();
    g.players[0].religion = Some("Confucianism".into());
    assert_eq!(g.city_religion(&g.cities[&quito]), Some("Confucianism"));
    assert!(!ai.counterweight_faithless(&g, 0));
    assert_eq!(ai.counterweight_cap(&g, 0, "Buddhism", 2), 2);
}

/// Off the Diplomacy lane a founder with no city of its faith stays a founder.
#[test]
fn a_culture_founder_stays_a_founder() {
    let (mut g, mut ai, _) = g200_turn_103(VictoryTarget::Culture);
    ai.enable_counterweight_defends_the_diplomat();
    g.players[0].religion = Some("Catholicism".into());
    assert!(!ai.counterweight_faithless(&g, 0));
}

/// A seat that never founded is faithless whatever the gene and lane.
#[test]
fn a_seat_that_never_founded_is_faithless_either_way() {
    for lane in [VictoryTarget::Culture, VictoryTarget::Diplomacy, VictoryTarget::Domination] {
        let (g, ai, _) = g200_turn_103(lane);
        assert!(ai.counterweight_faithless(&g, 0));
    }
}

#[test]
fn faith_war_defends_the_diplomat_is_opt_in() {
    opt_in_off_in_both_controllers("faith-war-defends-the-diplomat", |ai| {
        ai.faith_war_defends_the_diplomat
    });
}

/// `faith-war-defends-the-diplomat` opens the faith war on the Diplomacy lane
/// only; the Domination lane keeps it either way.
#[test]
fn the_faith_war_opens_on_the_diplomacy_lane_only() {
    for (lane, stock) in [
        (VictoryTarget::Diplomacy, false),
        (VictoryTarget::Culture, false),
        (VictoryTarget::Domination, true),
    ] {
        let (g, mut ai, _) = g200_turn_103(lane);
        assert_eq!(ai.faith_war_lane(&g), stock, "{lane:?} stock");
        ai.enable_faith_war_defends_the_diplomat();
        assert_eq!(
            ai.faith_war_lane(&g),
            stock || lane == VictoryTarget::Diplomacy,
            "{lane:?} under the gene"
        );
        assert!(!ai.counterweight_lane(&g) || lane == VictoryTarget::Domination, "the counterweight gene is separate");
    }
}
