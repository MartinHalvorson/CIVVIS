use super::*;

fn order(unit: i64, verb: &str, pos: (i32, i32)) -> Order {
    Order {
        kind: "unit",
        subject: Some(unit),
        verb: Some(verb.to_string()),
        pos: Some(pos),
    }
}

#[test]
fn spotting_position_survives_bombing_before_cavalry_retreat() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            order(7, "MOVE_TO", (8, 8)),
            order(7, "MOVE_TO", (9, 8)),
            order(20, "AIR_ATTACK", (11, 8)),
            order(21, "AIR_ATTACK", (11, 8)),
            order(7, "MOVE_TO", (8, 8)),
        ],
        true,
    );
    assert_eq!(deferred, 0);
    assert_eq!(coalesced, 1, "only the approach may be compressed");
    assert_eq!(orders.len(), 4);
    assert_eq!(
        orders[0].pos,
        Some((9, 8)),
        "keep the position that reveals the city"
    );
    assert_eq!(orders[1].verb.as_deref(), Some("AIR_ATTACK"));
    assert_eq!(orders[2].verb.as_deref(), Some("AIR_ATTACK"));
    assert_eq!(orders[3].pos, Some((8, 8)), "retreat follows both sorties");
}

#[test]
fn cavalry_can_approach_the_bombed_city_before_capturing() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            order(7, "MOVE_TO", (9, 8)),
            order(20, "AIR_ATTACK", (11, 8)),
            order(7, "MOVE_TO", (10, 8)),
            order(7, "ATTACK", (11, 8)),
        ],
        true,
    );
    assert_eq!((orders.len(), deferred, coalesced), (4, 0, 0));
    assert_eq!(orders[0].pos, Some((9, 8)));
    assert_eq!(orders[2].pos, Some((10, 8)));
    assert_eq!(orders[3].verb.as_deref(), Some("ATTACK"));
}

#[test]
fn an_unsequenced_host_keeps_the_spot_and_defers_the_retreat() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            order(7, "MOVE_TO", (9, 8)),
            order(20, "AIR_ATTACK", (11, 8)),
            order(7, "MOVE_TO", (8, 8)),
        ],
        false,
    );
    assert_eq!((orders.len(), deferred, coalesced), (2, 1, 0));
    assert_eq!(orders[0].pos, Some((9, 8)));
}

#[test]
fn ordinary_uninterrupted_travel_still_coalesces() {
    let (orders, deferred, coalesced) = coalesce_unit_paths(
        vec![
            order(7, "MOVE_TO", (8, 8)),
            order(7, "MOVE_TO", (9, 8)),
            order(20, "REBASE", (4, 8)),
            order(7, "MOVE_TO", (10, 8)),
        ],
        true,
    );
    assert_eq!((orders.len(), deferred, coalesced), (2, 0, 2));
    assert_eq!(orders[0].pos, Some((10, 8)));
}
