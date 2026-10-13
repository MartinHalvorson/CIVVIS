use super::{defer_great_person_plot_conflicts, Order};
use civvis::mirror::{StateGreatPerson, StateSnapshot, StateUnit};

fn person() -> StateUnit {
    StateUnit {
        id: 5_636_111,
        kind: "UNIT_COMANDANTE_GENERAL".into(),
        x: 25,
        y: 10,
        great_person: Some(StateGreatPerson {
            charges: 1,
            can_activate: false,
            ..StateGreatPerson::default()
        }),
        ..StateUnit::default()
    }
}

fn mover(id: i64, kind: &str, combat: f64, ranged: f64) -> StateUnit {
    StateUnit {
        id,
        kind: kind.into(),
        x: 23,
        y: 10,
        combat,
        ranged,
        ..StateUnit::default()
    }
}

fn order(id: i64, verb: &str, pos: Option<(i32, i32)>) -> Order {
    Order {
        kind: "unit",
        subject: Some(id),
        verb: Some(verb.into()),
        pos,
    }
}

fn recorded_approach(id: i64, kind: &str, combat: f64, ranged: f64, target: (i32, i32)) {
    // civvis-20261009T143233Z, turns 148/frame 2, 152/frame 0 and 153/frame 0.
    // These are the recorded bridge pairs and the actor/person fields read by
    // this filter, not a fresh-controller reproduction of the entire planner.
    let state = StateSnapshot {
        units: vec![person(), mover(id, kind, combat, ranged)],
        ..StateSnapshot::default()
    };
    let (kept, deferred) = defer_great_person_plot_conflicts(
        vec![
            order(id, "MOVE_TO", Some((25, 10))),
            order(id, "RANGE_ATTACK", Some(target)),
        ],
        &state,
    );
    assert_eq!(
        deferred, 0,
        "the idle civilian must not swallow a military approach"
    );
    assert_eq!(kept.len(), 2);
    assert_eq!(kept[0].subject, Some(id));
    assert_eq!(kept[0].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(kept[0].pos, Some((25, 10)));
    assert_eq!(kept[1].subject, Some(id));
    assert_eq!(kept[1].verb.as_deref(), Some("RANGE_ATTACK"));
    assert_eq!(kept[1].pos, Some(target));
}

#[test]
fn recorded_catapult_keeps_its_approach_before_the_shot() {
    recorded_approach(3_932_180, "UNIT_CATAPULT", 25.0, 0.0, (27, 10));
}

#[test]
fn recorded_archer_keeps_its_approach_before_the_shot() {
    recorded_approach(2_752_515, "UNIT_ARCHER", 15.0, 25.0, (26, 12));
}

#[test]
fn recorded_field_cannon_keeps_its_approach_before_the_shot() {
    recorded_approach(8_716_298, "UNIT_FIELD_CANNON", 50.0, 60.0, (27, 10));
}

#[test]
fn military_may_enter_an_idle_admirals_tile() {
    let mut admiral = person();
    admiral.kind = "UNIT_GREAT_ADMIRAL".into();
    let state = StateSnapshot {
        units: vec![admiral, mover(1, "UNIT_GALLEY", 30.0, 0.0)],
        ..StateSnapshot::default()
    };
    let (kept, deferred) =
        defer_great_person_plot_conflicts(vec![order(1, "MOVE_TO", Some((25, 10)))], &state);
    assert_eq!(deferred, 0);
    assert_eq!(kept.len(), 1);
}

#[test]
fn activation_protects_the_tile_even_when_the_move_precedes_it() {
    let state = StateSnapshot {
        units: vec![person(), mover(1, "UNIT_GALLEY", 30.0, 0.0)],
        ..StateSnapshot::default()
    };
    for activation_first in [true, false] {
        let mut orders = vec![order(1, "MOVE_TO", Some((25, 10)))];
        let activation = order(person().id, "ACTIVATE_GREAT_PERSON", None);
        if activation_first {
            orders.insert(0, activation);
        } else {
            orders.push(activation);
        }
        let (kept, deferred) = defer_great_person_plot_conflicts(orders, &state);
        assert_eq!(deferred, 1);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].verb.as_deref(), Some("ACTIVATE_GREAT_PERSON"));
    }
}

#[test]
fn civilians_support_and_unobserved_movers_keep_the_reservation() {
    let state = StateSnapshot {
        units: vec![
            person(),
            mover(1, "UNIT_SETTLER", 0.0, 0.0),
            mover(2, "UNIT_BATTERING_RAM", 0.0, 0.0),
        ],
        ..StateSnapshot::default()
    };
    let (kept, deferred) = defer_great_person_plot_conflicts(
        [1, 2, 3]
            .into_iter()
            .map(|id| order(id, "MOVE_TO", Some((25, 10))))
            .collect(),
        &state,
    );
    assert_eq!(deferred, 3);
    assert!(kept.is_empty());
}

#[test]
fn one_activating_person_protects_a_shared_tile() {
    let mut other = person();
    other.id += 1;
    let state = StateSnapshot {
        units: vec![person(), other.clone(), mover(1, "UNIT_WARRIOR", 20.0, 0.0)],
        ..StateSnapshot::default()
    };
    let (kept, deferred) = defer_great_person_plot_conflicts(
        vec![
            order(1, "MOVE_TO", Some((25, 10))),
            order(other.id, "ACTIVATE_GREAT_PERSON", None),
        ],
        &state,
    );
    assert_eq!(deferred, 1);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].subject, Some(other.id));
}

#[test]
fn an_available_but_unscheduled_activation_does_not_reserve_the_military_slot() {
    let mut idle = person();
    idle.great_person.as_mut().unwrap().can_activate = true;
    let state = StateSnapshot {
        units: vec![idle, mover(1, "UNIT_WARRIOR", 20.0, 0.0)],
        ..StateSnapshot::default()
    };
    let (kept, deferred) =
        defer_great_person_plot_conflicts(vec![order(1, "MOVE_TO", Some((25, 10)))], &state);
    assert_eq!(deferred, 0);
    assert_eq!(kept.len(), 1);
}

#[test]
fn trade_routes_own_moves_and_other_destinations_are_preserved() {
    let state = StateSnapshot {
        units: vec![person()],
        ..StateSnapshot::default()
    };
    let (kept, deferred) = defer_great_person_plot_conflicts(
        vec![
            order(1, "TRADE_ROUTE", Some((25, 10))),
            order(person().id, "MOVE_TO", Some((25, 10))),
            order(2, "MOVE_TO", Some((26, 10))),
        ],
        &state,
    );
    assert_eq!(deferred, 0);
    assert_eq!(kept.len(), 3);
}
