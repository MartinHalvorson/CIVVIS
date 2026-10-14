use super::*;

fn order(subject: i64, verb: &str, pos: (i32, i32)) -> Order {
    Order {
        kind: "unit",
        subject: Some(subject),
        verb: Some(verb.to_string()),
        pos: Some(pos),
    }
}

fn shape(orders: &[Order]) -> Vec<(i64, String)> {
    orders
        .iter()
        .map(|o| (o.subject.unwrap_or(-1), o.verb.clone().unwrap_or_default()))
        .collect()
}

/// Live King civvis-20261005T091120Z, turn 20: the Warrior's blow came
/// first and killed the Horse Archer; the Slinger's shot after it was dropped.
#[test]
fn a_shot_at_a_target_moves_ahead_of_the_blow_on_it() {
    let target = (5, 5);
    let mut orders = vec![order(1, "ATTACK", target), order(2, "RANGE_ATTACK", target)];
    assert_eq!(ranged_before_melee(&mut orders), 1);
    assert_eq!(
        shape(&orders),
        vec![(2, "RANGE_ATTACK".into()), (1, "ATTACK".into())]
    );
}

#[test]
fn every_shot_at_the_target_lands_before_the_blow_in_its_own_order() {
    let target = (5, 5);
    let mut orders = vec![
        order(1, "ATTACK", target),
        order(2, "RANGE_ATTACK", target),
        order(3, "RANGE_ATTACK", target),
    ];
    assert_eq!(ranged_before_melee(&mut orders), 2);
    assert_eq!(
        shape(&orders),
        vec![
            (2, "RANGE_ATTACK".into()),
            (3, "RANGE_ATTACK".into()),
            (1, "ATTACK".into())
        ]
    );
}

#[test]
fn a_shot_keeps_its_own_units_earlier_steps_ahead_of_it() {
    let target = (5, 5);
    // Its walk already precedes the blow: the shot may move up to it.
    let mut before = vec![
        order(2, "MOVE_TO", (4, 5)),
        order(1, "ATTACK", target),
        order(2, "RANGE_ATTACK", target),
    ];
    assert_eq!(ranged_before_melee(&mut before), 1);
    assert_eq!(
        shape(&before),
        vec![
            (2, "MOVE_TO".into()),
            (2, "RANGE_ATTACK".into()),
            (1, "ATTACK".into())
        ]
    );
    // Its walk comes after the blow: the shot stays where it is.
    let mut after = vec![
        order(1, "ATTACK", target),
        order(2, "MOVE_TO", (4, 5)),
        order(2, "RANGE_ATTACK", target),
    ];
    let unchanged = shape(&after);
    assert_eq!(ranged_before_melee(&mut after), 0);
    assert_eq!(shape(&after), unchanged);
}

#[test]
fn shots_at_other_targets_and_shots_already_first_stay() {
    let mut orders = vec![
        order(2, "RANGE_ATTACK", (5, 5)),
        order(1, "ATTACK", (5, 5)),
        order(3, "RANGE_ATTACK", (6, 6)),
    ];
    let unchanged = shape(&orders);
    assert_eq!(ranged_before_melee(&mut orders), 0);
    assert_eq!(shape(&orders), unchanged);
}

#[test]
fn the_gene_is_off_unless_enabled() {
    let mut ai = civvis::ai::AdvancedAi::new();
    assert!(!ai.ranged_before_melee_enabled());
    ai.enable_ranged_before_melee();
    assert!(ai.ranged_before_melee_enabled());
}
