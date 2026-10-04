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

/// `opening-force-keeps-its-members`: a skirmish row away from home does
/// not draft the undeclared opening's strike force. Live King
/// civvis-20261004T070716Z (game 49): four of the Kyoto opening's six bodies
/// stayed home on a ClearCamp row and Destroy rows against raiders, and the
/// opening released at turn 40.
#[test]
fn the_opening_strike_force_keeps_its_members() {
    for keeps in [false, true] {
        let mut g = flat_board(374_612, &[at(6, 10), at(30, 10), at(20, 19)], false);
        let target = g.city_at(at(30, 10)).unwrap();
        // A war with a third empire supplies the raider; the opening's target
        // is at peace.
        war(&mut g, 0, 2);
        let members: Vec<u32> = (9..=12)
            .map(|row| g.spawn_test_unit("archer", 0, at(8, row)))
            .collect();
        let mut ai = on();
        ai.early_conquest_opening = true;
        if keeps {
            ai.enable_opening_force_keeps_its_members();
        }
        ai.conquest_opening = Some(crate::ai::advanced::early_conquest::ConquestOpening {
            target: 1,
            city: target,
            opened: g.turn,
            preparing_since: None,
            grace_until: None,
            rally: at(27, 10),
            force: members.iter().copied().collect(),
            assembled: None,
            declared: None,
            kills_at_war: 0,
            losses: 0,
            taken: 0,
        });
        assert_eq!(members.iter().all(|uid| ai.conquest_force_member(*uid)), keeps);
        // A raider five tiles off, in a scout's sight.
        g.spawn_test_unit("horseman", 2, at(8, 16));
        g.spawn_test_unit("scout", 0, at(9, 15));
        let plan = conquest(&g, Some(target));
        ai.rebuild_force_groups(&g, 0, &plan);
        let taken = members
            .iter()
            .filter(|uid| matches!(row_of(&ai, **uid), Some(ObjectiveKey::Destroy(_))))
            .count();
        if keeps {
            assert_eq!(taken, 0, "the opening keeps its strike force");
        } else {
            assert!(taken > 0, "control: the Destroy row takes the opening's bodies");
        }
    }
}
