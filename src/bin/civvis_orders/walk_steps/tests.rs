use super::*;

fn trail(unit: u32, turn: u32, path: &[Pos]) -> UnitMoveTrail {
    UnitMoveTrail {
        unit,
        owner: 0,
        turn,
        path: path.to_vec(),
    }
}

#[test]
fn a_one_order_walk_reads_its_steps_off_the_trail() {
    let trails = [trail(7, 35, &[(0, 0), (1, 0), (2, 0), (3, 0)])];
    let mut reader = Reader::default();
    let walk = Action::MoveTo {
        unit: 7,
        to: (3, 0),
    };
    assert_eq!(
        reader.steps_before(&trails, 35, &walk),
        vec![(1, 0), (2, 0)]
    );
}

#[test]
fn a_single_hop_and_a_walk_cut_short_stay_one_order() {
    let trails = [trail(7, 35, &[(0, 0), (1, 0)])];
    let mut reader = Reader::default();
    let hop = Action::MoveTo {
        unit: 7,
        to: (1, 0),
    };
    assert!(reader.steps_before(&trails, 35, &hop).is_empty());
    // The engine stopped the walk short of the hex the action names.
    let mut reader = Reader::default();
    let short = Action::MoveTo {
        unit: 7,
        to: (4, 0),
    };
    assert!(reader.steps_before(&trails, 35, &short).is_empty());
    // No trail at all, another unit's trail, and last turn's trail.
    let other = [
        trail(8, 35, &[(0, 0), (1, 0), (2, 0)]),
        trail(7, 34, &[(0, 0), (1, 0), (2, 0)]),
    ];
    let mut reader = Reader::default();
    let walk = Action::MoveTo {
        unit: 7,
        to: (2, 0),
    };
    assert!(reader.steps_before(&other, 35, &walk).is_empty());
}

#[test]
fn earlier_steps_of_the_same_unit_are_not_read_twice() {
    // One hop by hand, then a walk of three: two trails, as the engine
    // records them when another unit moved in between.
    let trails = [
        trail(7, 35, &[(0, 0), (1, 0)]),
        trail(9, 35, &[(5, 5), (5, 6)]),
        trail(7, 35, &[(1, 0), (2, 0), (3, 0), (4, 0)]),
    ];
    let mut reader = Reader::default();
    let hop = Action::Move {
        unit: 7,
        to: (1, 0),
    };
    assert!(reader.steps_before(&trails, 35, &hop).is_empty());
    let walk = Action::MoveTo {
        unit: 7,
        to: (4, 0),
    };
    assert_eq!(
        reader.steps_before(&trails, 35, &walk),
        vec![(2, 0), (3, 0)]
    );
    // A second walk of the same unit starts after the first.
    let more = [
        trail(7, 35, &[(0, 0), (1, 0), (2, 0)]),
        trail(7, 35, &[(2, 0), (3, 0), (4, 0)]),
    ];
    let mut reader = Reader::default();
    let first = Action::MoveTo {
        unit: 7,
        to: (2, 0),
    };
    let second = Action::MoveTo {
        unit: 7,
        to: (4, 0),
    };
    assert_eq!(reader.steps_before(&more, 35, &first), vec![(1, 0)]);
    assert_eq!(reader.steps_before(&more, 35, &second), vec![(3, 0)]);
}
