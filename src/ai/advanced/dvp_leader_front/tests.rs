use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::StrategicPlan;

/// Four majors on open grassland. Seat 0 (four Modern Armor) fights seat 1;
/// seat 3, at peace and in reach, holds two Modern Armor -- too strong to be
/// capital prey, under our military 1.5 times over -- and is the Diplomatic
/// Victory leader the tests move.
fn fronts() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(4, 40, 24, 936_034, 500, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    for (owner, pos) in [(6, 12), (14, 12), (23, 12), (16, 18)]
        .into_iter()
        .enumerate()
    {
        g.found_city_for(owner, pos, None);
        if owner != 0 {
            g.record_contact(0, owner);
        }
    }
    for _ in 0..4 {
        g.spawn_test_unit("modern_armor", 0, (8, 12));
    }
    for _ in 0..2 {
        g.spawn_test_unit("modern_armor", 3, (17, 19));
    }
    g.spawn_test_unit("warrior", 1, (14, 13));
    g.at_war.insert((0, 1));
    g.current = 0;
    g.turn = 190;
    let mut ai = AdvancedAi::new();
    ai.retarget(VictoryTarget::Domination);
    ai.enable_one_war_at_a_time();
    ai.deny_leaders = true;
    ai.deny_while_targeted = true;
    ai.battlefront_observation = true;
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: g.player_city_ids(1).first().copied(),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    });
    ai.one_war_observe(&g, 0);
    assert_eq!(ai.one_war_front(), Some(1));
    (g, ai)
}

#[test]
fn both_genes_are_opt_in() {
    opt_in_off_in_both_controllers("dvp-leader-is-the-front", |ai| ai.dvp_leader_is_the_front);
    opt_in_off_in_both_controllers("congress-guards-the-leader", |ai| {
        ai.congress_guards_the_leader_enabled()
    });
}

/// A Diplomacy reading of fifteen points is a clock the Domination army
/// answers under the gene; one point under, or another lane, is not moved.
#[test]
fn fifteen_points_is_a_domination_clock() {
    let (g, mut ai) = fronts();
    let at = |progress| VictoryFocus {
        strategy: GrandStrategy::Diplomacy,
        progress,
    };
    assert!(
        !ai.domination_counter_pressure(&g, at(DVP_FRONT_PRESSURE)),
        "off"
    );
    ai.enable_dvp_leader_is_the_front();
    assert!(ai.domination_counter_pressure(&g, at(DVP_FRONT_PRESSURE)));
    assert!(!ai.domination_counter_pressure(&g, at(DVP_FRONT_PRESSURE - 1)));
    let culture = VictoryFocus {
        strategy: GrandStrategy::Culture,
        progress: DVP_FRONT_PRESSURE,
    };
    let culture_on = ai.domination_counter_pressure(&g, culture);
    ai.disable_dvp_leader_is_the_front();
    assert_eq!(
        ai.domination_counter_pressure(&g, culture),
        culture_on,
        "the culture arm is unchanged"
    );
}

/// G402: Nubia on 15 points at peace while the army fought another rival.
/// Under the gene the leader opens the second front at once; under 15
/// points, or once it holds two thirds of our military, it does not.
#[test]
fn the_leader_on_fifteen_opens_the_second_front() {
    let (mut g, mut ai) = fronts();
    g.players[3].dvp = DVP_FRONT_POINTS;
    assert_ne!(ai.one_war_second_front(&g, 0), Some(3), "off");
    assert_eq!(ai.dvp_leader_front(&g, 0), None, "off");
    ai.enable_dvp_leader_is_the_front();
    assert_eq!(ai.dvp_leader_front(&g, 0), Some(3));
    assert_eq!(ai.one_war_second_front(&g, 0), Some(3));
    // One point short of a session from the win.
    g.players[3].dvp = DVP_FRONT_POINTS - 1;
    assert_eq!(ai.dvp_leader_front(&g, 0), None, "under fifteen");
    assert_ne!(ai.one_war_second_front(&g, 0), Some(3));
    // Inside the declaration edge.
    g.players[3].dvp = DVP_FRONT_POINTS;
    let mut row = 2;
    while g.military_power(0) >= DECLARATION_EDGE_RATIO * g.military_power(3) {
        g.spawn_test_unit("modern_armor", 3, (30, row));
        row += 1;
    }
    assert_eq!(ai.dvp_leader_front(&g, 0), None, "inside the edge");
    // A leader we already fight is the elimination gene's, not a new front.
    let (mut g, mut ai) = fronts();
    ai.enable_dvp_leader_is_the_front();
    g.players[3].dvp = DVP_FRONT_POINTS;
    g.at_war.insert((0, 3));
    assert_ne!(ai.one_war_second_front(&g, 0), Some(3), "already at war");
}

/// The counter names the leader on fifteen: at turn 199 G402's journal read
/// "Countering Netherlands | its diplomacy race reads 51%" with Nubia on 15.
#[test]
fn the_leader_on_fifteen_is_the_denial_target() {
    let (mut g, mut ai) = fronts();
    g.players[3].dvp = DVP_FRONT_POINTS;
    assert_ne!(
        ai.actionable_victory_denial(&g, 0),
        Some((3, GrandStrategy::Conquest)),
        "off"
    );
    ai.enable_dvp_leader_is_the_front();
    assert_eq!(
        ai.actionable_victory_denial(&g, 0),
        Some((3, GrandStrategy::Conquest))
    );
}
