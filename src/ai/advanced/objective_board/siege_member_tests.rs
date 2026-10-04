use super::tests::{at, conquest, flat_board, on, war};
use super::*;

/// The force holding `uid`, by its row key.
fn row_of(ai: &AdvancedAi, uid: u32) -> Option<ObjectiveKey> {
    ai.objective_board()
        .forces
        .iter()
        .find(|force| force.units.contains(&uid))
        .map(|force| force.objective_key)
}

/// See `SIEGE_MEMBER_STRIKE_REACH`: a skirmish across the front does not
/// pull a member off a staging Siege force; one within reach of it still
/// may.
#[test]
fn a_staging_siege_force_keeps_its_members() {
    for keeps in [false, true] {
        let mut g = flat_board(374_611, &[at(6, 10), at(30, 10)], false);
        let target = g.city_at(at(30, 10)).unwrap();
        war(&mut g, 0, 1);
        // The two bodies the Reserve keeps nearest home, and a column out
        // on the road to the target.
        g.spawn_test_unit("warrior", 0, at(6, 11));
        g.spawn_test_unit("warrior", 0, at(7, 10));
        let members: Vec<u32> = (8..=13)
            .map(|row| g.spawn_test_unit("swordsman", 0, at(16, row)))
            .collect();
        let mut ai = on();
        // A Domination war's surplus joins the Siege row.
        ai.retarget(VictoryTarget::Domination);
        if keeps {
            ai.enable_siege_force_keeps_its_members();
        }
        let plan = conquest(&g, Some(target));
        ai.rebuild_force_groups(&g, 0, &plan);
        let members: Vec<u32> = members
            .into_iter()
            .filter(|uid| row_of(&ai, *uid) == Some(ObjectiveKey::Siege(target)))
            .collect();
        assert!(members.len() >= 3, "fixture: a Siege force forms");
        // A raider four tiles off the column's flank, in a scout's sight.
        g.spawn_test_unit("horseman", 1, at(17, 17));
        g.spawn_test_unit("scout", 0, at(18, 16));
        g.turn += 1;
        ai.rebuild_force_groups(&g, 0, &plan);
        let taken = members
            .iter()
            .filter(|uid| matches!(row_of(&ai, **uid), Some(ObjectiveKey::Destroy(_))))
            .count();
        if keeps {
            assert_eq!(taken, 0, "the siege keeps its members");
        } else {
            assert!(taken > 0, "control: the Destroy row takes members");
        }
    }
}
