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

/// `breakers-stay-with-the-siege`: a gun beside a pressured home city does
/// not join its Defend row while a walled Siege row asks for guns, and a gun
/// last turn's board put there comes back. Live King civvis-20261005T024614Z
/// (game 94): fielded Catapults went to the Guayaquil anvil while Lisbon's
/// siege held for a wall-breaker.
#[test]
fn a_gun_stays_with_the_walled_siege() {
    let board = || {
        let mut g = flat_board(374_613, &[at(6, 10), at(30, 10)], false);
        let target = g.city_at(at(30, 10)).unwrap();
        let walled = g.cities.get_mut(&target).unwrap();
        walled.buildings = vec![crate::name!("walls"), crate::name!("medieval_walls")];
        walled.wall_hp = 200;
        war(&mut g, 0, 1);
        let home = g.city_at(at(6, 10)).unwrap();
        // Three raiders at home, two of ours beside it, a gun beside them and a
        // column on the road to the target.
        for pos in [at(8, 10), at(7, 11), at(8, 11)] {
            g.spawn_test_unit("swordsman", 1, pos);
        }
        for pos in [at(5, 10), at(5, 11)] {
            g.spawn_test_unit("warrior", 0, pos);
        }
        let gun = g.spawn_test_unit("catapult", 0, at(6, 11));
        for row in 8..=13 {
            g.spawn_test_unit("swordsman", 0, at(16, row));
        }
        (g, target, home, gun)
    };
    let ai_for = |stay: bool| {
        let mut ai = on();
        ai.retarget(VictoryTarget::Domination);
        ai.enable_siege_needs_a_breaker();
        if stay {
            ai.enable_breakers_stay_with_the_siege();
        }
        ai
    };

    let (mut g, target, home, gun) = board();
    let plan = conquest(&g, Some(target));
    let mut control = ai_for(false);
    control.rebuild_force_groups(&g, 0, &plan);
    assert!(
        AdvancedAi::city_pressure(&g, 0, home) >= BASTION_PRESSURE,
        "fixture: home is pressured"
    );
    assert_eq!(
        row_of(&control, gun),
        Some(ObjectiveKey::Defend(home)),
        "control: the Defend row takes the gun"
    );

    let mut ai = ai_for(true);
    ai.rebuild_force_groups(&g, 0, &plan);
    assert!(
        matches!(
            row_of(&ai, gun),
            Some(ObjectiveKey::Siege(_) | ObjectiveKey::Reserve) | None
        ),
        "{:?}",
        row_of(&ai, gun)
    );
    assert!(ai
        .objective_board()
        .forces
        .iter()
        .any(|force| force.objective_key == ObjectiveKey::Defend(home) && !force.units.is_empty()));

    // A gun the board sent to the anvil last turn comes back.
    control.enable_breakers_stay_with_the_siege();
    g.turn += 1;
    control.rebuild_force_groups(&g, 0, &plan);
    assert_ne!(row_of(&control, gun), Some(ObjectiveKey::Defend(home)));

    // No walled siege on the board: the gun defends.
    let (mut g, target, home, gun) = board();
    let open = g.cities.get_mut(&target).unwrap();
    open.buildings.clear();
    open.wall_hp = 0;
    let mut ai = ai_for(true);
    ai.rebuild_force_groups(&g, 0, &conquest(&g, Some(target)));
    assert_eq!(row_of(&ai, gun), Some(ObjectiveKey::Defend(home)));
}
