use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use std::sync::Arc;

const FAITH: &str = "Orthodoxy";

/// Seat 1 founded Orthodoxy and it holds seats 2 and 3; of our three cities
/// one follows it, so our majority is still out: we are its last holdout.
/// We are at war with seat 1 and out-gunned (100 against 300), which is the
/// shipped "outmatched" peace offer.
fn holdout_at_war() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(4, 40, 24, 581144, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let following = g.found_city_for(0, (2, 8), None);
    g.found_city_for(0, (7, 8), None);
    g.found_city_for(0, (12, 8), None);
    g.found_city_for(1, (33, 8), None);
    g.found_city_for(2, (18, 8), None);
    g.found_city_for(3, (28, 8), None);
    g.players[1].religion = Some(FAITH.into());
    for pid in [1, 2, 3] {
        Arc::make_mut(&mut g.observed_majority_religion).insert(pid, FAITH.into());
    }
    let city = g.cities.get_mut(&following).unwrap();
    city.atheist_pressure = 0.0;
    city.pressure.insert(FAITH.into(), 100.0);
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.current = 0;
    g.turn = 114;
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 100.0), (1, 300.0)]);
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(AdvancedAi::we_are_the_last_holdout(&g, 0, 1));
    assert!(!g.civ_follows_religion(0, FAITH));
    (g, ai, plan, following)
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("holdout-keeps-the-faith-war", |ai| {
        ai.holdout_keeps_the_faith_war
    });
}

/// 110038Z's shape: the last holdout with a city on the founder's faith keeps
/// the war; off, nothing is kept.
#[test]
fn the_last_holdout_with_a_city_on_the_faith_keeps_the_war() {
    let (g, mut ai, _, _) = holdout_at_war();
    assert!(
        !ai.holdout_keeps_the_faith_war_with(&g, 0, 1),
        "off: unchanged"
    );
    ai.enable_holdout_keeps_the_faith_war();
    assert!(ai.holdout_keeps_the_faith_war_with(&g, 0, 1));
    assert!(
        !ai.holdout_keeps_the_faith_war_with(&g, 0, 2),
        "a follower is not the founder"
    );
}

/// A city the faith is converting (at 60% of its strongest pressure) counts;
/// one where it is a minor pressure does not.
#[test]
fn a_city_being_converted_counts_and_a_trace_does_not() {
    let (mut g, mut ai, _, following) = holdout_at_war();
    ai.enable_holdout_keeps_the_faith_war();
    let city = g.cities.get_mut(&following).unwrap();
    city.pressure.clear();
    city.pressure.insert("Islam".into(), 100.0);
    city.pressure.insert(FAITH.into(), 60.0);
    assert!(ai.holdout_keeps_the_faith_war_with(&g, 0, 1), "converting");
    let city = g.cities.get_mut(&following).unwrap();
    city.pressure.insert(FAITH.into(), 30.0);
    assert!(
        !ai.holdout_keeps_the_faith_war_with(&g, 0, 1),
        "a trace of the faith is not a conversion"
    );
}

/// No city of ours on the faith (the four spared match-point offers), not
/// the last holdout, or a city of ours falling: the war is not kept.
#[test]
fn spared_when_no_city_follows_when_others_hold_out_or_when_we_fall() {
    let (mut g, mut ai, _, following) = holdout_at_war();
    ai.enable_holdout_keeps_the_faith_war();
    let city = g.cities.get_mut(&following).unwrap();
    city.pressure.clear();
    city.atheist_pressure = 50.0;
    assert!(
        !ai.holdout_keeps_the_faith_war_with(&g, 0, 1),
        "no city on the faith"
    );

    let (mut g, mut ai, _, _) = holdout_at_war();
    ai.enable_holdout_keeps_the_faith_war();
    Arc::make_mut(&mut g.observed_majority_religion).insert(3, "Islam".into());
    assert!(!AdvancedAi::we_are_the_last_holdout(&g, 0, 1));
    assert!(
        !ai.holdout_keeps_the_faith_war_with(&g, 0, 1),
        "seat 3 still holds out"
    );

    let (mut g, mut ai, _, following) = holdout_at_war();
    ai.enable_holdout_keeps_the_faith_war();
    let turn = g.turn;
    let city = g.cities.get_mut(&following).unwrap();
    city.last_attacked = turn;
    city.hp = 80;
    assert!(
        !ai.holdout_keeps_the_faith_war_with(&g, 0, 1),
        "a falling city reopens the peace"
    );

    let (mut g, mut ai, _, _) = holdout_at_war();
    ai.enable_holdout_keeps_the_faith_war();
    g.victory_conditions.religious = false;
    assert!(
        !ai.holdout_keeps_the_faith_war_with(&g, 0, 1),
        "no Religious Victory"
    );
}

/// The choke point: the shipped "outmatched" offer is made with the gene off
/// and withheld with it on; with no city of ours on the faith it is made.
#[test]
fn the_peace_offer_is_withheld_from_the_faith_racer() {
    let (mut g, mut ai, plan, _) = holdout_at_war();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        ai.peace_offers.contains(&1),
        "off: the outmatched offer is made"
    );

    let (mut g, mut ai, plan, _) = holdout_at_war();
    ai.enable_holdout_keeps_the_faith_war();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1), "on: withheld");
    assert!(g.is_at_war(0, 1));

    let (mut g, mut ai, plan, following) = holdout_at_war();
    ai.enable_holdout_keeps_the_faith_war();
    let city = g.cities.get_mut(&following).unwrap();
    city.pressure.clear();
    city.atheist_pressure = 50.0;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        ai.peace_offers.contains(&1),
        "no city on the faith: offered"
    );
}

fn propose_peace_from_the_founder(g: &mut Game) {
    g.current = 1;
    g.apply(
        1,
        &Action::ProposeDeal {
            player: 0,
            give_gold: 0.0,
            request_gold: 0.0,
            open_borders: false,
            friendship: false,
            peace: true,
            alliance: None,
        },
    )
    .unwrap();
    g.current = 0;
    assert!(g
        .pending_deals
        .iter()
        .any(|deal| deal.peace && deal.to == 0));
}

/// Off, the out-gunned seat accepts the founder's peace.
#[test]
fn off_the_founders_peace_offer_is_accepted() {
    let (mut g, mut ai, plan, _) = holdout_at_war();
    propose_peace_from_the_founder(&mut g);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "off: the founder's peace is taken");
}

/// The founder's own peace offer is refused under the gene.
#[test]
fn the_faith_racers_peace_offer_is_refused() {
    let (mut g, mut ai, plan, _) = holdout_at_war();
    ai.enable_holdout_keeps_the_faith_war();
    propose_peace_from_the_founder(&mut g);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "the founder's peace is refused");
    assert!(
        !g.pending_deals
            .iter()
            .any(|deal| deal.peace && deal.from == 1 && deal.to == 0 && deal.expires >= g.turn),
        "and answered, not left pending"
    );
}
