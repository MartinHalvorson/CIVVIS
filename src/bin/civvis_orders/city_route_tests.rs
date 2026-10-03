use super::*;
use civvis::mirror::{StateCity, StateDistrict, StateMinor, StateRival, StateSnapshot, StateUnit};

fn wounded_unit() -> StateUnit {
    StateUnit {
        id: 7,
        x: 13,
        y: 23,
        hp: 75.0,
        combat: 35.0,
        ..Default::default()
    }
}

fn steps() -> Vec<Order> {
    [(14, 22), (13, 21)]
        .into_iter()
        .map(|pos| Order {
            kind: "unit",
            subject: Some(7),
            verb: Some("MOVE_TO".into()),
            pos: Some(pos),
        })
        .chain(std::iter::once(Order {
            kind: "unit",
            subject: Some(7),
            verb: Some("FORTIFY".into()),
            pos: None,
        }))
        .collect()
}

fn city_board() -> StateSnapshot {
    // Native 20261001T050754Z, turn 78: a 75 HP Swordsman beside
    // Västerås. There is no enemy military unit within the old guard's ring.
    StateSnapshot {
        units: vec![wounded_unit()],
        rivals: vec![StateRival {
            at_war: true,
            cities: vec![StateCity {
                x: 14,
                y: 20,
                ranged_strength: Some(25.0),
                max_wall_damage: 100.0,
                wall_damage: 56.0,
                ..Default::default()
            }],
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn a_wounded_swordsman_keeps_its_steps_beside_an_enemy_city() {
    let state = city_board();
    let (orders, deferred, coalesced) =
        coalesce_unit_paths_except(steps(), true, &wounded_local_routes(&state));
    assert_eq!(
        coalesced, 0,
        "the city must not erase the planned first step"
    );
    assert_eq!(deferred, 0);
    assert_eq!(orders[0].pos, Some((14, 22)));
    assert_eq!(orders[1].pos, Some((13, 21)));
    assert_eq!(orders[2].verb.as_deref(), Some("FORTIFY"));
}

#[test]
fn a_four_hp_skirmisher_retreats_from_known_cities_without_a_visible_army() {
    let mut state = city_board();
    state.units[0].x = 46;
    state.units[0].y = 17;
    state.units[0].hp = 4.0;
    state.units[0].combat = 20.0;
    state.rivals[0].cities = vec![StateCity {
        x: 46,
        y: 18,
        ranged_strength: Some(40.0),
        max_wall_damage: 400.0,
        ..Default::default()
    }];
    assert!(wounded_local_routes(&state).contains(&7));
    // The known city remains a route obstacle when its strength is fogged.
    state.rivals[0].cities[0].ranged_strength = None;
    assert!(wounded_local_routes(&state).contains(&7));
}

#[test]
fn hostile_minor_cities_and_units_preserve_wounded_routes() {
    let mut state = city_board();
    let rival = state.rivals.pop().unwrap();
    state.minors.push(StateMinor {
        at_war: true,
        cities: rival.cities,
        ..Default::default()
    });
    assert!(wounded_local_routes(&state).contains(&7));
    state.minors[0].cities.clear();
    state.minors[0].units.push(StateUnit {
        x: 14,
        y: 23,
        ranged: 40.0,
        ..Default::default()
    });
    assert!(wounded_local_routes(&state).contains(&7));
}

#[test]
fn an_enemy_encampment_can_be_near_when_its_city_is_far() {
    let mut state = city_board();
    state.rivals[0].cities[0].x = 19;
    state.rivals[0].cities[0].y = 23;
    state.rivals[0].cities[0].districts.push(StateDistrict {
        kind: "DISTRICT_ENCAMPMENT".into(),
        x: 16,
        y: 23,
        complete: true,
        ..Default::default()
    });
    assert!(wounded_local_routes(&state).contains(&7));
    state.rivals[0].cities[0].districts[0].pillaged = true;
    assert!(wounded_local_routes(&state).is_empty());
    state.rivals[0].cities[0].districts[0].pillaged = false;
    state.rivals[0].cities[0].districts[0].complete = false;
    assert!(wounded_local_routes(&state).is_empty());
}

#[test]
fn ordinary_travel_still_coalesces_and_civilians_do_not_enter_the_guard() {
    for change in [0, 1, 2, 3] {
        let mut state = city_board();
        match change {
            0 => state.units[0].hp = 100.0,
            1 => state.units[0].combat = 0.0,
            2 => state.rivals[0].at_war = false,
            _ => state.rivals[0].cities[0].x = 40,
        }
        let (orders, _, coalesced) =
            coalesce_unit_paths_except(steps(), true, &wounded_local_routes(&state));
        assert_eq!(coalesced, 1);
        assert_eq!(orders[0].pos, Some((13, 21)));
    }
}

#[test]
fn an_older_host_receives_the_first_safe_step_and_defers_the_followup() {
    let state = city_board();
    let (orders, deferred, coalesced) =
        coalesce_unit_paths_except(steps(), false, &wounded_local_routes(&state));
    assert_eq!(orders.len(), 1);
    assert_eq!(orders[0].pos, Some((14, 22)));
    assert_eq!(deferred, 2);
    assert_eq!(coalesced, 0);
}
