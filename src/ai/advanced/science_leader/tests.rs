use super::*;
use crate::ai::advanced::{GrandStrategy, StrategicPlan};
use crate::game::Game;
use std::sync::Arc;

/// G187's shape at peace: player 1 the Science leader (the Zulu), player 2 a
/// nearer, weaker neighbour that out-researches nobody (Portugal). Both are
/// met, on open grassland, inside the declaration range. `leader_power` is
/// player 1's military against our 1,000.
fn leader_and_neighbour(leader_power: f64) -> (Game, AdvancedAi) {
    let mut g = Game::new_full(3, 44, 26, 187_120, 650, 0, false);
    let ids: Vec<_> = g.units.keys().copied().collect();
    for id in ids {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.found_city_for(0, crate::hex::offset_to_axial(6, 8), None);
    g.found_city_for(0, crate::hex::offset_to_axial(6, 14), None);
    g.found_city_for(2, crate::hex::offset_to_axial(14, 8), None);
    g.found_city_for(1, crate::hex::offset_to_axial(20, 16), None);
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    let power = Arc::make_mut(&mut g.observed_military_power);
    power.insert(0, 1000.0);
    power.insert(1, leader_power);
    power.insert(2, 120.0);
    // The leader's host Science figure: 150 a turn over its one city.
    Arc::make_mut(&mut g.observed_yield_adjustments)
        .entry(1)
        .or_default()
        .science = 150.0;
    g.players[0].gold = 2000.0;
    g.turn = g.standard_duration(SCIENCE_LEADER_TURN) + 5;
    g.current = 0;
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    (g, ai)
}

fn set_science(g: &mut Game, seat: usize, science: f64) {
    let derived = AdvancedAi::seat_science_per_turn(g, seat)
        - g.observed_yield_adjustments
            .get(&seat)
            .map_or(0.0, |adjustment| adjustment.science);
    Arc::make_mut(&mut g.observed_yield_adjustments)
        .entry(seat)
        .or_default()
        .science = science - derived;
}

/// The G187 fixture: without the gene the campaign aims at the nearer,
/// weaker neighbour; under it, at the Science leader we out-gun.
#[test]
fn the_science_leader_is_the_campaign_rival_under_the_gene() {
    let (g, mut ai) = leader_and_neighbour(300.0);
    assert_eq!(
        AdvancedAi::science_leader_by(&g, 0, SCIENCE_LEADER_MARGIN).map(|(leader, _, _)| leader),
        Some(1),
        "fixture: player 1 leads Science by the margin"
    );
    assert_eq!(
        ai.science_leader_at_peace(&g, 0),
        None,
        "off without the gene"
    );
    let off = ai.assess(&g, 0);
    assert_eq!(
        off.target_player,
        Some(2),
        "fixture: the elective pick is the neighbour"
    );
    ai.enable_science_leader_is_the_target();
    assert_eq!(ai.science_leader_at_peace(&g, 0), Some(1));
    let on = ai.assess(&g, 0);
    assert_eq!(
        on.target_player,
        Some(1),
        "the Science leader is the campaign's rival"
    );
    assert!(
        on.target_city
            .is_some_and(|city| g.cities[&city].owner == 1),
        "and the first objective is one of its cities: {:?}",
        on.target_city
    );
}

/// A leader beyond the declaration edge is not the target, and no war is
/// opened on it: the pick falls back to the elective choice.
#[test]
fn a_science_leader_beyond_the_edge_is_not_the_target_and_no_war_opens() {
    // 1,000 against 800 is 1.25 times, under the 1.5 edge.
    let (mut g, mut ai) = leader_and_neighbour(800.0);
    ai.enable_science_leader_is_the_target();
    assert_eq!(ai.science_leader_at_peace(&g, 0), None, "short of the edge");
    let plan = ai.assess(&g, 0);
    assert_ne!(plan.target_player, Some(1));
    for _ in 0..3 {
        let plan = ai.assess(&g, 0);
        ai.advanced_diplomacy(&mut g, 0, &plan);
        g.turn += 1;
        g.current = 0;
    }
    assert!(
        !g.is_at_war(0, 1),
        "no declaration on a leader we cannot beat"
    );
}

/// The reading's own gates: the margin (with the plan's rival kept on a
/// plain lead), the turn, peace, and a lead of ours.
#[test]
fn the_science_leader_needs_a_clear_lead_peace_and_the_turn() {
    let (mut g, mut ai) = leader_and_neighbour(300.0);
    ai.enable_science_leader_is_the_target();
    assert_eq!(ai.science_leader_at_peace(&g, 0), Some(1));

    // Inside the margin: 100 against the neighbour's 92 is no clear lead...
    set_science(&mut g, 1, 100.0);
    set_science(&mut g, 2, 92.0);
    assert_eq!(ai.science_leader_at_peace(&g, 0), None, "inside the margin");
    // ...unless it is already the plan's rival, which a plain lead keeps.
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: g.player_city_ids(1).first().copied(),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    });
    assert_eq!(
        ai.science_leader_at_peace(&g, 0),
        Some(1),
        "the incumbent keeps it"
    );
    set_science(&mut g, 2, 101.0);
    assert_eq!(
        ai.science_leader_at_peace(&g, 0),
        None,
        "not once it is passed"
    );
    ai.plan = None;
    set_science(&mut g, 1, 150.0);
    set_science(&mut g, 2, 40.0);
    assert_eq!(ai.science_leader_at_peace(&g, 0), Some(1));

    // Our own lead names no rival.
    set_science(&mut g, 0, 200.0);
    assert_eq!(ai.science_leader_at_peace(&g, 0), None, "we lead");
    set_science(&mut g, 0, 10.0);

    // Before the turn.
    let turn = g.turn;
    g.turn = g.standard_duration(SCIENCE_LEADER_TURN) - 1;
    assert_eq!(ai.science_leader_at_peace(&g, 0), None, "before the turn");
    g.turn = turn;

    // A war already running keeps its front: no extra front on the leader.
    g.at_war.insert((0, 2));
    g.at_war.insert((2, 0));
    assert_eq!(ai.science_leader_at_peace(&g, 0), None, "at war");
}

#[test]
fn science_leader_is_the_target_is_a_native_opt_in_off_in_both_controllers() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "science-leader-is-the-target",
        |ai| ai.science_leader_is_the_target,
    );
}
