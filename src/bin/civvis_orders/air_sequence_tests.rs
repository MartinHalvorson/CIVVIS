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

#[test]
fn a_step_that_takes_a_city_is_not_folded_into_the_step_out() {
    // G52 turn 191: the Helicopter moved into Mashhad, then the battle planner
    // rotated it out to heal; folded, the host flew it past the city.
    let city = (39, 25);
    let planned = || {
        vec![
            order(7, "MOVE_TO", (40, 24)),
            order(7, "MOVE_TO", city),
            order(7, "MOVE_TO", (33, 20)),
            order(7, "FORTIFY", (33, 20)),
        ]
    };
    let tiles: std::collections::BTreeSet<(i32, i32)> = [city].into_iter().collect();
    let none = Default::default();

    let (folded, _, _) = coalesce_unit_paths_except(planned(), true, &none, &Default::default());
    assert_eq!(folded[0].pos, Some((33, 20)), "without the city the walk folds past it");

    let (orders, deferred, coalesced) = coalesce_unit_paths_except(planned(), true, &none, &tiles);
    assert_eq!((deferred, coalesced), (0, 1));
    assert_eq!(orders[0].pos, Some(city), "the walk ends on the city it takes");
    assert_eq!(orders[1].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(orders[1].pos, Some((33, 20)), "the step out rides the queue after the capture");
    assert_eq!(orders[2].verb.as_deref(), Some("FORTIFY"));

    let (old_host, deferred, _) = coalesce_unit_paths_except(planned(), false, &none, &tiles);
    assert_eq!(old_host.len(), 1, "an unsequenced host keeps only the capture this frame");
    assert_eq!(old_host[0].pos, Some(city));
    assert_eq!(deferred, 2);
}
