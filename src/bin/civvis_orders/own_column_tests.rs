use super::*;

fn walk(subject: i64, pos: (i32, i32)) -> Order {
    Order {
        kind: "unit",
        subject: Some(subject),
        verb: Some("MOVE_TO".to_string()),
        pos: Some(pos),
    }
}

#[test]
fn open_walk_steps_keep_each_units_opening_walk_only() {
    let orders = vec![
        walk(7, (3, 3)),
        walk(9, (8, 8)),
        walk(7, (4, 3)),
        walk(7, (5, 3)),
        Order {
            kind: "unit",
            subject: Some(7),
            verb: Some("FORTIFY".to_string()),
            pos: None,
        },
        walk(7, (6, 3)),
    ];
    let steps = open_walk_steps(&orders);
    assert_eq!(steps.get(&7), Some(&vec![(3, 3), (4, 3), (5, 3)]));
    assert!(
        !steps.contains_key(&9),
        "a single step has no earlier hex to fall back to"
    );
}

/// G104-style column: the walk folds to its last hex, a column mate stands
/// there and does not move first, so the walk is cut back to the last free
/// hex of the same path instead of being refused whole.
#[test]
fn a_walk_ending_on_a_held_hex_is_cut_back_to_the_last_free_hex() {
    let raw = vec![walk(7, (3, 3)), walk(7, (4, 3)), walk(7, (5, 3))];
    let steps = open_walk_steps(&raw);
    let (mut orders, _, _) = coalesce_unit_paths(raw, true);
    assert_eq!(orders[0].pos, Some((5, 3)), "the walk folds to its last hex");
    let held = |_: i64, hex: (i32, i32), _: usize| hex == (5, 3);
    assert_eq!(retarget_friendly_held_walks(&mut orders, &steps, held), 1);
    assert_eq!(orders[0].pos, Some((4, 3)));
}

#[test]
fn a_walk_whose_every_hex_is_held_is_left_for_the_host() {
    let raw = vec![walk(7, (3, 3)), walk(7, (4, 3))];
    let steps = open_walk_steps(&raw);
    let (mut orders, _, _) = coalesce_unit_paths(raw, true);
    let held = |_: i64, _: (i32, i32), _: usize| true;
    assert_eq!(retarget_friendly_held_walks(&mut orders, &steps, held), 0);
    assert_eq!(orders[0].pos, Some((4, 3)));
}

#[test]
fn a_free_end_and_an_attack_are_untouched() {
    let raw = vec![walk(7, (3, 3)), walk(7, (4, 3))];
    let steps = open_walk_steps(&raw);
    let (mut orders, _, _) = coalesce_unit_paths(raw, true);
    orders.push(Order {
        kind: "unit",
        subject: Some(7),
        verb: Some("CAPTURE".to_string()),
        pos: Some((4, 3)),
    });
    let held = |_: i64, hex: (i32, i32), index: usize| index == 1 && hex == (4, 3);
    assert_eq!(retarget_friendly_held_walks(&mut orders, &steps, held), 0);
    assert_eq!(orders[0].pos, Some((4, 3)));
    assert_eq!(orders[1].pos, Some((4, 3)));
}

#[test]
fn planned_movers_split_by_what_crossed() {
    let civ6_of: std::collections::BTreeMap<u32, i64> = [(1, 101), (2, 102), (3, 103)].into();
    let planned = [
        (0, Action::Move { unit: 1, to: (0, 0) }),
        (0, Action::MoveTo { unit: 2, to: (0, 0) }),
        (0, Action::Fortify { unit: 3 }),
        (1, Action::Move { unit: 3, to: (0, 0) }),
    ];
    let orders = vec![walk(101, (1, 1))];
    let (sent, unsent) = split_planned_movers(
        &orders,
        planned.iter().map(|(seat, action)| (*seat, action)),
        &civ6_of,
    );
    assert_eq!(sent, [1].into());
    assert_eq!(unsent, [2].into(), "a fortify is no move, and a rival seat is not ours");
}
