//! `hopeless-war-brings-the-army-home`: the reading, the hysteresis, the
//! homes and the step.
use super::*;
use crate::ai::advanced::defend_force_holds_the_city::{HoldPlan, HoldPost, HoldRole};
use crate::ai::advanced::VictoryTarget;

/// A grassland field at war with a major: our city at (6, 10), our Archer
/// at (12, 10) and Warrior at (12, 12), a Line Infantry of theirs at
/// (20, 10) seen by a third soldier of ours, a Spearman at (21, 12).
fn war() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = Game::new_full(2, 40, 24, 377_401, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.river_edges = [false; 6];
        tile.riverside = false;
    }
    g.found_city_for(0, (6, 10), None);
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let archer = g.spawn_test_unit("archer", 0, (12, 10));
    let warrior = g.spawn_test_unit("warrior", 0, (12, 12));
    let hostile = g.spawn_test_unit("line_infantry", 1, (20, 10));
    g.spawn_test_unit("spearman", 0, (21, 12));
    assert!(g.unit_visible_to(hostile, 0));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_hopeless_war_brings_the_army_home();
    (g, ai, archer, warrior, hostile)
}

fn plan_on(ai: &mut AdvancedAi, g: &mut Game, turn: u32) {
    g.turn = turn;
    ai.plan_army_home(g, 0, &BTreeSet::new());
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "hopeless-war-brings-the-army-home",
        |ai| ai.hopeless_war_brings_the_army_home,
    );
}

/// At war with a major, no siege, a Line Infantry (65) against our best
/// (an Archer's 25): hopeless. In force on the second such turn; it lifts
/// when the war ends.
#[test]
fn a_hopeless_war_comes_into_force_on_its_second_turn_and_lifts_with_the_war() {
    let (mut g, mut ai, archer, warrior, _) = war();
    let reading = ai.read_hopeless_war(&g, 0);
    assert!(reading.at_war && reading.sieges == 0 && reading.best == 0.0);
    assert!(
        reading.theirs - reading.ours >= OBSOLETE_STRENGTH_GAP,
        "{reading:?}"
    );
    assert!(reading.hopeless());
    plan_on(&mut ai, &mut g, 10);
    assert!(
        !ai.army_home_in_force(&g),
        "one hopeless turn is not enough"
    );
    plan_on(&mut ai, &mut g, 11);
    assert!(ai.army_home_in_force(&g));
    let posts = &ai.hopeless_war_home.posts;
    // The empty centre takes the shooter.
    assert_eq!(posts.get(&archer), Some(&((6, 10), HomeRole::Centre)));
    let (home, role) = posts[&warrior];
    assert_ne!(role, HomeRole::Centre);
    assert!(g.wdist(home, (20, 10)) > 2, "{home:?}");

    g.at_war.remove(&(0, 1));
    plan_on(&mut ai, &mut g, 12);
    assert!(!ai.army_home_in_force(&g));
    assert!(ai.hopeless_war_home.posts.is_empty());

    // The gene off: nothing is read.
    let (mut g, mut ai, _, _, _) = war();
    ai.disable_hopeless_war_brings_the_army_home();
    plan_on(&mut ai, &mut g, 10);
    plan_on(&mut ai, &mut g, 11);
    assert!(!ai.army_home_in_force(&g));
    assert!(ai.hopeless_war_home.posts.is_empty());
}

/// A field army a match for the enemy is not outclassed: nothing happens.
#[test]
fn an_army_that_matches_the_enemy_stays_in_the_field() {
    let (mut g, mut ai, _, _, _) = war();
    g.spawn_test_unit("line_infantry", 0, (10, 14));
    assert!(!ai.read_hopeless_war(&g, 0).hopeless());
    plan_on(&mut ai, &mut g, 10);
    plan_on(&mut ai, &mut g, 11);
    assert!(!ai.army_home_in_force(&g));
}

/// A soldier `defend-force-holds-the-city` posts is that gene's; the others
/// still go home.
#[test]
fn a_posted_defender_stays_the_holds() {
    let (mut g, mut ai, archer, warrior, _) = war();
    let city = g.city_at((6, 10)).expect("our city");
    ai.enable_defend_force_holds_the_city();
    g.turn = 11;
    let mut posts = BTreeMap::new();
    posts.insert(
        warrior,
        HoldPost {
            city,
            at: (7, 10),
            role: HoldRole::Ring,
        },
    );
    ai.defend_hold_plan = HoldPlan {
        turn: 11,
        posts,
        cities: vec![city],
    };
    plan_on(&mut ai, &mut g, 10);
    ai.defend_hold_plan.turn = 11;
    plan_on(&mut ai, &mut g, 11);
    assert!(ai.army_home_in_force(&g));
    assert!(!ai.hopeless_war_home.posts.contains_key(&warrior));
    // The held city's centre is not one of the homes.
    assert_ne!(ai.hopeless_war_home.posts[&archer].1, HomeRole::Centre);
}

/// In force, the Archer walks into the empty centre and holds it.
#[test]
fn a_soldier_walks_home_and_holds() {
    let (mut g, mut ai, archer, _, _) = war();
    plan_on(&mut ai, &mut g, 10);
    plan_on(&mut ai, &mut g, 11);
    let start = g.units[&archer].pos;
    let before = g.wdist(start, (6, 10));
    assert!(ai.army_home_step(&mut g, 0, archer).is_some());
    let after = g.wdist(g.units[&archer].pos, (6, 10));
    assert!(after < before, "{start:?} -> {:?}", g.units[&archer].pos);
}
