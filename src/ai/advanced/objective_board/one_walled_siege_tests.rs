//! `one-walled-siege-at-a-time`. See `one_walled_siege`.

use super::super::city_campaign::CampaignPlan;
use super::super::siege_train::STAGING_FAR;
use super::one_walled_siege::SCREEN;
use super::tests::{at, conquest, flat_board, war};
use super::*;

/// Two walled cities of the rival, A in the north and B in the south, both
/// on the campaign; nobody of ours on the board yet.
fn two_walls(seed: u64) -> (Game, u32, u32) {
    let mut g = flat_board(seed, &[at(6, 10), at(30, 4)], false);
    let a = g.city_at(at(30, 4)).unwrap();
    let b = g.found_city_for(1, at(29, 17), None);
    for cid in [a, b] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.buildings = vec![crate::name!("walls"), crate::name!("medieval_walls")];
        city.wall_hp = 200;
    }
    war(&mut g, 0, 1);
    (g, a, b)
}

/// The live arms around the gene: a Domination seat whose walled rows ask
/// for guns and keep them, on a campaign over both cities.
fn seat(cities: [u32; 2], one: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_objective_board();
    ai.battlefront_observation = false;
    ai.enable_siege_needs_a_breaker();
    ai.enable_breakers_stay_with_the_siege();
    ai.enable_city_campaign_2();
    ai.campaign = Some(CampaignPlan {
        target: 1,
        cities: cities.to_vec(),
        requirement: 150.0,
        bodies: 5,
        planned: 40,
        declared: Some(50),
        taken: 0,
    });
    if one {
        ai.enable_one_walled_siege_at_a_time();
    }
    ai
}

/// The land force serving `key`, empty when none does.
fn land(ai: &AdvancedAi, key: ObjectiveKey) -> Vec<u32> {
    ai.objective_board()
        .forces
        .iter()
        .filter(|force| force.objective_key == key && force.domain == ForceDomain::Land)
        .flat_map(|force| force.units.iter().copied())
        .collect()
}

/// Test seam: `uid` stood in `key`'s land force on the last board.
fn put(ai: &mut AdvancedAi, uid: u32, key: ObjectiveKey) {
    let forces = &mut ai.objective_board_state.forces;
    for force in forces.iter_mut() {
        force.units.retain(|other| *other != uid);
    }
    if let Some(force) = forces
        .iter_mut()
        .find(|force| force.objective_key == key && force.domain == ForceDomain::Land)
    {
        force.units.push(uid);
        return;
    }
    let id = forces.iter().map(|force| force.id).max().unwrap_or(0) + 1;
    forces.push(TaskForce {
        id,
        objective_key: key,
        domain: ForceDomain::Land,
        units: vec![uid],
        rally: (0, 0),
        doctrine_state: ForcePosture::Muster,
        aimed_at: (0, 0),
        formed: 0,
    });
}

/// The plan's target moves from B to A after the guns joined B. Without the
/// gene the new top row asks for its own guns and the train splits; with it
/// the guns stay with B and A keeps a screen.
#[test]
fn the_guns_serve_one_walled_siege_through_a_retarget() {
    let (mut g, a, b) = two_walls(631_001);
    let guns: Vec<u32> = [at(7, 10), at(7, 11), at(8, 10)]
        .iter()
        .map(|pos| g.spawn_test_unit("catapult", 0, *pos))
        .collect();
    for index in 0..8 {
        g.spawn_test_unit("swordsman", 0, at(9 + index % 4, 9 + index / 4));
    }
    let guns_in = |ai: &AdvancedAi, key| {
        let force = land(ai, key);
        guns.iter().filter(|gun| force.contains(gun)).count()
    };
    let run = |one: bool| {
        let mut g = g.clone();
        let mut ai = seat([a, b], one);
        ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(b)));
        g.turn += 1;
        ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(a)));
        ai
    };

    let control = run(false);
    assert!(
        guns_in(&control, ObjectiveKey::Siege(a)) > 0 && guns_in(&control, ObjectiveKey::Siege(b)) > 0,
        "control: the train splits, A {:?} B {:?}",
        land(&control, ObjectiveKey::Siege(a)),
        land(&control, ObjectiveKey::Siege(b))
    );

    let ai = run(true);
    assert_eq!(guns_in(&ai, ObjectiveKey::Siege(a)), 0, "A recruits no gun");
    assert!(
        guns_in(&ai, ObjectiveKey::Siege(b)) >= 2,
        "B keeps its train: {:?}",
        land(&ai, ObjectiveKey::Siege(b))
    );
    let screen = land(&ai, ObjectiveKey::Siege(a));
    assert!(screen.len() <= SCREEN.bodies, "A holds a screen: {screen:?}");
    assert_eq!(
        ai.objective_board_state.one_siege.map(|record| record.at),
        Some(g.cities[&b].pos)
    );
}

/// -9c's two constraints. A gun the Reserve held while the chosen row is
/// short of guns joins the chosen row, and a lower walled row's land force
/// is cut to a melee screen, its gun returned.
#[test]
fn a_short_chosen_siege_takes_the_reserve_gun_and_a_lower_row_keeps_a_screen() {
    let (mut g, a, b) = two_walls(631_002);
    let held = g.spawn_test_unit("catapult", 0, at(7, 10));
    let spare = g.spawn_test_unit("catapult", 0, at(7, 11));
    let column: Vec<u32> = (0..5)
        .map(|index| g.spawn_test_unit("swordsman", 0, at(24 + index, 14)))
        .collect();
    let run = |one: bool| {
        let mut g = g.clone();
        let mut ai = seat([a, b], one);
        ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(a)));
        // The last board: one gun with A, one in the Reserve, the column
        // on B.
        put(&mut ai, held, ObjectiveKey::Siege(a));
        put(&mut ai, spare, ObjectiveKey::Reserve);
        for uid in &column {
            put(&mut ai, *uid, ObjectiveKey::Siege(b));
        }
        g.turn += 1;
        ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(a)));
        ai
    };

    let control = run(false);
    assert!(
        land(&control, ObjectiveKey::Siege(b)).len() > SCREEN.bodies,
        "control: B holds the column"
    );

    let ai = run(true);
    let chosen = land(&ai, ObjectiveKey::Siege(a));
    assert!(chosen.contains(&held) && chosen.contains(&spare), "{chosen:?}");
    let screen = land(&ai, ObjectiveKey::Siege(b));
    assert!(!screen.is_empty() && screen.len() <= SCREEN.bodies, "{screen:?}");
    assert!(screen.iter().all(|uid| column.contains(uid)), "{screen:?}");
}

/// A chosen siege with its guns on the staging ring keeps them while the
/// city is pushed lower, and releases them to the other walled row once it
/// stands for `STALL_READINGS` readings without a new low.
#[test]
fn a_stalled_walled_siege_releases_its_guns() {
    let (mut g, a, b) = two_walls(631_003);
    let ring = [at(29, 13), at(30, 13)];
    let guns: Vec<u32> = ring
        .iter()
        .map(|pos| g.spawn_test_unit("catapult", 0, *pos))
        .collect();
    for pos in ring {
        assert!(g.wdist(pos, g.cities[&b].pos) <= STAGING_FAR, "fixture: on B's ring");
        assert!(g.wdist(pos, g.cities[&a].pos) > STAGING_FAR, "fixture: off A's ring");
    }
    for index in 0..6 {
        g.spawn_test_unit("swordsman", 0, at(9 + index % 3, 9 + index / 3));
    }
    let run = |pushed: bool| {
        let mut g = g.clone();
        let mut ai = seat([a, b], true);
        let mut chosen = Vec::new();
        for turn in 0..=11 {
            if pushed && turn == 6 {
                g.cities.get_mut(&b).unwrap().wall_hp -= 20;
            }
            ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(a)));
            let with_a = guns.iter().all(|gun| land(&ai, ObjectiveKey::Siege(a)).contains(gun));
            let with_b = guns.iter().all(|gun| land(&ai, ObjectiveKey::Siege(b)).contains(gun));
            chosen.push(match (with_a, with_b) {
                (true, false) => 'A',
                (false, true) => 'B',
                _ => '?',
            });
            g.turn += 1;
        }
        chosen.into_iter().collect::<String>()
    };

    // Chosen at turn 0 with its guns on the ring; nine stalled readings
    // (turns 1-9) release it.
    assert_eq!(run(false), "BBBBBBBBBAAA");
    // A new low at turn 6 restarts the count.
    assert_eq!(run(true), "BBBBBBBBBBBB");
}

/// With one walled siege on the board nothing is screened, and the board
/// is the one the gene-off seat writes.
#[test]
fn one_walled_siege_alone_changes_nothing() {
    let (mut g, a, _) = two_walls(631_004);
    let b = g.city_at(at(29, 17)).unwrap();
    let open = g.cities.get_mut(&b).unwrap();
    open.buildings.clear();
    open.wall_hp = 0;
    for pos in [at(7, 10), at(7, 11), at(8, 10)] {
        g.spawn_test_unit("catapult", 0, pos);
    }
    for index in 0..8 {
        g.spawn_test_unit("swordsman", 0, at(9 + index % 4, 9 + index / 4));
    }
    let board = |one: bool| {
        let mut ai = seat([a, b], one);
        ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(a)));
        let board = ai.objective_board();
        (board.rows.clone(), board.forces.clone(), ai.requisitions())
    };
    assert_eq!(board(true), board(false));
}
