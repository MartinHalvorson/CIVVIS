//! `guns-stay-out-of-reach-2`: the one-blow veto on every land military unit.
use super::*;
use crate::ai::advanced::VictoryTarget;

/// A grassland field, our archer at (16, 10) and trebuchet at (16, 12), a
/// Line Infantry at (20, 10) seen by a warrior of ours behind it, at war.
fn fixture() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, crate::rng::fixture_seed("AUTO_6", 377300), 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (6, 10), None);
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let archer = g.spawn_test_unit("archer", 0, (16, 10));
    let gun = g.spawn_test_unit("trebuchet", 0, (16, 12));
    let hostile = g.spawn_test_unit("line_infantry", 1, (20, 10));
    g.spawn_test_unit("warrior", 0, (22, 10));
    assert!(g.unit_visible_to(hostile, 0));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.recorded_tactical_step = true;
    (g, ai, archer, gun, hostile)
}

#[test]
fn the_gene_ships_off_is_registered_and_turns_version_one_off() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "guns-stay-out-of-reach-2",
        |ai| ai.guns_stay_out_of_reach_2,
    );
    let mut ai = AdvancedAi::new();
    ai.enable_guns_stay_out_of_reach();
    ai.enable_guns_stay_out_of_reach_2();
    assert!(!ai.guns_stay_out_of_reach && ai.guns_stay_out_of_reach_2);
}

/// The Line Infantry steps once and strikes: two tiles out is one blow from
/// a healthy Archer's death, three is not.
#[test]
fn an_archer_does_not_step_where_one_blow_destroys_it() {
    let (mut g, mut ai, archer, _, _) = fixture();
    ai.enable_guns_stay_out_of_reach_2();
    ai.mark_gun_lethal_tiles(&g, 0);
    let lethal = &ai.base.unit_lethal_blows[&archer];
    assert!(lethal.contains_key(&(18, 10)), "{lethal:?}");
    assert!(!lethal.contains_key(&(17, 10)), "{lethal:?}");
    assert!(!lethal.contains_key(&(16, 10)), "{lethal:?}");
    assert!(ai.base.path_move(&mut g, 0, archer, (17, 10)));
    assert!(
        !ai.base.path_move(&mut g, 0, archer, (18, 10)),
        "the step into the one-blow reach is refused"
    );
    assert_eq!(g.units[&archer].pos, (17, 10));

    // Version one reads guns only: the Archer walks in.
    let (mut g, mut ai, archer, _, _) = fixture();
    ai.enable_guns_stay_out_of_reach();
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(ai.base.unit_lethal_blows.is_empty());
    assert!(ai.base.path_move(&mut g, 0, archer, (17, 10)));
    assert!(ai.base.path_move(&mut g, 0, archer, (18, 10)));
}

/// Version two carries version one: the trebuchet's tiles are marked as
/// before, in the guns' own table, and its step-out still runs.
#[test]
fn version_two_carries_the_guns() {
    let (g, mut ai, archer, gun, _) = fixture();
    ai.enable_guns_stay_out_of_reach_2();
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(ai.base.gun_lethal_tiles.contains_key(&gun));
    assert!(!ai.base.unit_lethal_blows.contains_key(&gun));
    assert!(!ai.base.gun_lethal_tiles.contains_key(&archer));
}

/// A hostile killed later in the frame holds no tile: its blow is dropped
/// once it is off the board.
#[test]
fn a_hostile_killed_this_frame_no_longer_vetoes() {
    let (mut g, mut ai, archer, _, hostile) = fixture();
    ai.enable_guns_stay_out_of_reach_2();
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(ai.base.unit_lethal_blows[&archer].contains_key(&(18, 10)));
    g.remove_unit(hostile);
    assert!(ai.base.path_move(&mut g, 0, archer, (17, 10)));
    assert!(ai.base.path_move(&mut g, 0, archer, (18, 10)));
}

/// The battle planner's own placements read the danger field already; a
/// unit it ordered this turn is not marked. A weaker hostile is no veto.
#[test]
fn a_planned_unit_and_a_weak_hostile_are_left_alone() {
    let (g, mut ai, archer, _, _) = fixture();
    ai.enable_guns_stay_out_of_reach_2();
    ai.battle_planner_ordered.insert(archer);
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(!ai.base.unit_lethal_blows.contains_key(&archer));

    let (mut g, mut ai, _, _, hostile) = fixture();
    g.remove_unit(hostile);
    let warrior = g.spawn_test_unit("warrior", 1, (20, 10));
    assert!(g.unit_visible_to(warrior, 0));
    let swordsman = g.spawn_test_unit("swordsman", 0, (16, 8));
    ai.enable_guns_stay_out_of_reach_2();
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(
        !ai.base.unit_lethal_blows.contains_key(&swordsman),
        "{:?}",
        ai.base.unit_lethal_blows
    );
}

/// A unit already inside a one-blow reach may step to a tile where the
/// strongest blow is lower, never to one where it is not.
#[test]
fn a_unit_inside_the_reach_may_only_step_to_a_lighter_blow() {
    let (mut g, mut ai, archer, _, _) = fixture();
    ai.enable_guns_stay_out_of_reach_2();
    g.relocate(archer, (18, 10));
    ai.mark_gun_lethal_tiles(&g, 0);
    let lethal = ai.base.unit_lethal_blows[&archer].clone();
    let here = lethal[&(18, 10)]
        .iter()
        .map(|(_, b)| *b)
        .fold(0.0, f64::max);
    // Adjacent to the Line Infantry the blow is no lighter: refused.
    let nearer = lethal.get(&(19, 10)).map_or(0.0, |blows| {
        blows.iter().map(|(_, b)| *b).fold(0.0, f64::max)
    });
    assert!(nearer >= here, "{lethal:?}");
    assert!(!ai.base.path_move(&mut g, 0, archer, (19, 10)));
    // Back out of the reach: taken.
    assert!(ai.base.path_move(&mut g, 0, archer, (17, 10)));
}

/// Version three: a member of a Defend task force is left free to close on
/// the attackers of our own city; every other unit keeps version two's veto.
#[test]
fn version_three_frees_the_defenders_and_keeps_the_veto_for_the_rest() {
    use crate::ai::advanced::objective_board::{ObjectiveKey, TaskForce};
    use crate::ai::advanced::{ForceDomain, ForcePosture};
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "guns-stay-out-of-reach-3",
        |ai| ai.guns_stay_out_of_reach_3,
    );
    let (mut g, mut ai, archer, gun, _) = fixture();
    let defender = g.spawn_test_unit("archer", 0, (16, 8));
    let city = g.city_at((6, 10)).expect("our city");
    ai.enable_guns_stay_out_of_reach_2();
    ai.enable_guns_stay_out_of_reach_3();
    assert!(!ai.guns_stay_out_of_reach && !ai.guns_stay_out_of_reach_2);
    ai.objective_board_state.forces.push(TaskForce {
        id: 1,
        objective_key: ObjectiveKey::Defend(city),
        domain: ForceDomain::Land,
        units: vec![defender],
        rally: (6, 10),
        doctrine_state: ForcePosture::Hold,
        aimed_at: (6, 10),
        formed: g.turn,
    });
    ai.mark_gun_lethal_tiles(&g, 0);
    assert!(!ai.base.unit_lethal_blows.contains_key(&defender));
    assert!(ai.base.unit_lethal_blows.contains_key(&archer));
    assert!(
        ai.base.gun_lethal_tiles.contains_key(&gun),
        "the guns are carried"
    );
    assert!(ai.base.path_move(&mut g, 0, defender, (17, 8)));
    assert!(ai.base.path_move(&mut g, 0, defender, (18, 8)));
}
