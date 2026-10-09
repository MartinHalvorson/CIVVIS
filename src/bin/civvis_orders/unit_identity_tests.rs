use super::*;
use crate::IssuedOrder;
use civvis::mirror::StateUnit;

fn spear(id: i64, x: i32, xp: i64) -> StateUnit {
    StateUnit {
        id,
        kind: "UNIT_SPEARMAN".into(),
        x,
        y: 16,
        xp: Some(xp),
        level: Some(1),
        promotions: Some(Vec::new()),
        formation: Some(0),
        upgrade_to: Some("UNIT_PIKEMAN".into()),
        ..StateUnit::default()
    }
}

fn upgrade(id: i64) -> IssuedOrder {
    IssuedOrder {
        kind: "unit".into(),
        subject: Some(id),
        verb: Some("UPGRADE".into()),
        pos: None,
    }
}

fn episode() -> (PendingOrders, StateSnapshot) {
    // civvis-20261009T131304Z, t103 frame1 -> frame2. The native
    // replacement retains the Spearman's 9 XP at offset (32,16).
    let old = spear(1769488, 32, 9);
    let mut new = old.clone();
    new.id = 5242900;
    new.kind = "UNIT_PIKEMAN".into();
    new.upgrade_to = None;
    let before = StateSnapshot {
        turn: 103,
        frame: 1,
        units: vec![old],
        ..Default::default()
    };
    let after = StateSnapshot {
        turn: 103,
        frame: 2,
        units: vec![new],
        ..Default::default()
    };
    (
        PendingOrders {
            turn: 103,
            frame: 1,
            orders: vec![upgrade(1769488)],
            before,
        },
        after,
    )
}

#[test]
fn native_spearman_replacement_carries_the_same_soldiers_assignment() {
    let (pending, after) = episode();
    let previous = BTreeMap::from([(7, 1769488), (8, 3145749)]);
    let current = BTreeMap::from([(5242900, 21), (3145749, 22)]);
    let replacements = replacement_ids(&[pending], &after);
    assert_eq!(replacements, BTreeMap::from([(1769488, 5242900)]));
    assert_eq!(
        carried_units(&previous, &current, &replacements),
        BTreeMap::from([(7, 21), (8, 22)])
    );
    assert!(
        !carried_units(&previous, &current, &BTreeMap::new()).contains_key(&7),
        "the unchanged-ID bridge drops this live veteran"
    );
}

#[test]
fn consecutive_native_upgrades_keep_separate_assignments() {
    let (first, mut after) = episode();
    let second_old = spear(3145749, 34, 0);
    let mut second_new = second_old.clone();
    second_new.id = 5308451;
    second_new.kind = "UNIT_PIKEMAN".into();
    second_new.upgrade_to = None;
    let second = PendingOrders {
        turn: 103,
        frame: 2,
        orders: vec![upgrade(3145749)],
        before: StateSnapshot {
            turn: 103,
            frame: 2,
            units: vec![after.units[0].clone(), second_old],
            ..Default::default()
        },
    };
    after.turn = 104;
    after.frame = 0;
    after.units.insert(0, second_new);
    let replacements = replacement_ids(&[first, second], &after);
    assert_eq!(
        replacements,
        BTreeMap::from([(1769488, 5242900), (3145749, 5308451)])
    );
    let carried = carried_units(
        &BTreeMap::from([(7, 1769488), (8, 3145749)]),
        &BTreeMap::from([(5242900, 22), (5308451, 21)]),
        &replacements,
    );
    assert_eq!(carried, BTreeMap::from([(7, 22), (8, 21)]));
}

#[test]
fn missing_or_conflicting_native_evidence_cannot_transfer_an_assignment() {
    for case in 0..25 {
        let (mut pending, mut after) = episode();
        match case {
            0 => pending.orders.clear(),
            1 => pending.orders[0].verb = Some("DELETE".into()),
            2 => pending.orders[0].kind = "city".into(),
            3 => pending.before.units[0].upgrade_to = None,
            4 => pending.before.units[0].upgrade_blocked_reason = Some("No Gold".into()),
            5 => after.units[0].kind = "UNIT_MUSKETMAN".into(),
            6 => after.units[0].x += 1,
            7 => after.units[0].xp = Some(0),
            8 => after.units[0].xp = None,
            9 => after.units[0].level = Some(2),
            10 => after.units[0].promotions = None,
            11 => after.units[0].promotions = Some(vec!["PROMOTION_THRUST".into()]),
            12 => after.units[0].formation = Some(1),
            13 => after.units[0].player = 1,
            14 => after.turn = 105,
            15 => after.frame = 1,
            16 => after.seat.local_player += 1,
            17 => pending.before.units.push(after.units[0].clone()),
            18 => after.units.push(pending.before.units[0].clone()),
            19 => pending.before.units[0].xp = None,
            20 => pending.before.units[0].level = None,
            21 => pending.before.units[0].promotions = None,
            22 => pending.before.units[0].formation = None,
            23 => after.turn = 102,
            24 => after.units[0].id = -1,
            _ => unreachable!(),
        }
        assert!(
            replacement_ids(&[pending], &after).is_empty(),
            "unsafe case {case}"
        );
    }
}

#[test]
fn ambiguous_replacements_and_confirmed_deaths_drop_memory() {
    let (pending, mut after) = episode();
    let mut duplicate = after.units[0].clone();
    duplicate.id += 1;
    after.units.push(duplicate);
    assert!(replacement_ids(&[pending], &after).is_empty());
    let (pending, mut after) = episode();
    after
        .confirmed_unit_deaths
        .push(civvis::mirror::HostUnitDeath {
            player: 0,
            unit: 1769488,
            turn: 103,
            opponent: Some(1),
        });
    assert!(replacement_ids(&[pending], &after).is_empty());
}

#[test]
fn existing_survivor_and_unchanged_native_id_keep_their_own_memory() {
    let previous = BTreeMap::from([(7, 1769488), (8, 5242900)]);
    let current = BTreeMap::from([(5242900, 21)]);
    let replacements = BTreeMap::from([(1769488, 5242900)]);
    assert_eq!(
        carried_units(&previous, &current, &replacements),
        BTreeMap::from([(8, 21)])
    );
    let current = BTreeMap::from([(1769488, 20), (5242900, 21)]);
    assert_eq!(
        carried_units(&previous, &current, &replacements),
        BTreeMap::from([(7, 20), (8, 21)])
    );
}

#[test]
fn two_old_soldiers_cannot_claim_the_same_replacement() {
    let (mut pending, after) = episode();
    let mut other = pending.before.units[0].clone();
    other.id = 3145749;
    pending.before.units.push(other);
    pending.orders.push(upgrade(3145749));
    assert!(replacement_ids(&[pending], &after).is_empty());
}

#[test]
fn promotion_order_does_not_change_the_veterans_identity() {
    let (mut pending, mut after) = episode();
    pending.before.units[0].promotions =
        Some(vec!["PROMOTION_THRUST".into(), "PROMOTION_ECHELON".into()]);
    after.units[0].promotions = Some(vec!["PROMOTION_ECHELON".into(), "PROMOTION_THRUST".into()]);
    assert_eq!(
        replacement_ids(&[pending], &after),
        BTreeMap::from([(1769488, 5242900)])
    );
}
