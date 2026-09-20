use super::tests::{strategic_wonder_fixture, wonder_plan};
use super::*;

fn wonder_board(wonder: &str) -> (Game, u32, Item) {
    let (mut g, cid) = strategic_wonder_fixture(936032, wonder);
    g.players[0].civ = "Rome".to_string();
    let city = g.cities.get_mut(&cid).unwrap();
    city.pop = 25;
    city.buildings.push(crate::name!("walls"));
    // A productive host city keeps this fixture about the lane gate, not
    // whether a late wonder fits the remaining-turn budget.
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            production: 50.0,
            ..Default::default()
        },
    );
    let item = Item::Wonder {
        wonder: Name::new(wonder),
        pos: g.wonder_sites(cid, wonder)[0],
    };
    (g, cid, item)
}

fn value(ai: &AdvancedAi, g: &Game, cid: u32, item: &Item) -> f64 {
    ai.production_value(
        g,
        0,
        cid,
        item,
        &wonder_plan(GrandStrategy::Conquest, g.turn),
        &ai.counts(g, 0),
    )
}

#[test]
fn domination_does_not_start_eiffel_tower_for_the_host_score_bonus() {
    let (g, cid, item) = wonder_board("eiffel_tower");
    let mut ordinary = AdvancedAi::new();
    ordinary.enable_live_wonder_race();
    assert!(value(&ordinary, &g, cid, &item) > 0.0);
    let mut domination = AdvancedAi::targeting(VictoryTarget::Domination);
    domination.enable_live_wonder_race();
    assert!(value(&domination, &g, cid, &item) <= -10_000.0);
}

#[test]
fn bargain_and_tally_variants_do_not_reopen_the_domination_score_race() {
    let (mut g, cid, item) = wonder_board("great_bath");
    let cost = g.item_remaining_cost_for_city(0, cid, &item);
    let production = g.city_yields(cid).production.max(1.0);
    g.cities.get_mut(&cid).unwrap().production = (cost - 3.0 * production).max(0.0);
    for bargain in [false, true] {
        let mut ordinary = AdvancedAi::new();
        if bargain {
            ordinary.enable_live_wonder_race();
            ordinary.enable_cheapest_wonder_first();
        } else {
            ordinary.enable_wonder_score_tally();
        }
        assert!(
            value(&ordinary, &g, cid, &item) > 0.0,
            "fixture bargain={bargain}"
        );
        ordinary.victory_target = Some(VictoryTarget::Domination);
        assert!(
            value(&ordinary, &g, cid, &item) <= -10_000.0,
            "bargain={bargain}"
        );
    }
}

#[test]
fn a_domination_target_preserves_the_value_of_its_existing_wonder_race() {
    let (mut g, cid, item) = wonder_board("eiffel_tower");
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: item.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&cid).unwrap().production = g.item_cost_for_city(0, cid, &item) / 2.0;
    let mut ordinary = AdvancedAi::new();
    ordinary.enable_live_wonder_race();
    let before = value(&ordinary, &g, cid, &item);
    assert!(before > 0.0);
    ordinary.victory_target = Some(VictoryTarget::Domination);
    assert_eq!(value(&ordinary, &g, cid, &item), before);
}

#[test]
fn score_and_culture_targets_keep_their_wonder_lanes() {
    let (g, cid, item) = wonder_board("eiffel_tower");
    for target in [VictoryTarget::Score, VictoryTarget::Culture] {
        let mut ai = AdvancedAi::targeting(target);
        ai.enable_live_wonder_race();
        ai.enable_strategic_wonders();
        let plan = wonder_plan(target.strategy(), g.turn);
        assert!(ai.production_value(&g, 0, cid, &item, &plan, &ai.counts(&g, 0)) > 0.0);
    }
}

#[test]
fn a_previously_invested_wonder_keeps_its_race_value_after_another_build_interrupts_it() {
    let (mut g, cid, item) = wonder_board("eiffel_tower");
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: item.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&cid).unwrap().production = g.item_cost_for_city(0, cid, &item) / 2.0;
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: Item::Unit {
                unit: crate::name!("builder"),
            },
        },
    )
    .unwrap();
    assert!(g.item_invested_production(cid, &item) > 0.0);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
    let mut ai = AdvancedAi::new();
    ai.enable_live_wonder_race();
    let before = value(&ai, &g, cid, &item);
    assert!(before > 0.0);
    ai.victory_target = Some(VictoryTarget::Domination);
    assert_eq!(value(&ai, &g, cid, &item), before);
}
