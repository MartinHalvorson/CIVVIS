use super::*;
use civvis::mirror::{StateCity, StateDistrict, StateGreatPerson, StateUnit};

fn district(kind: &str, x: i32, y: i32) -> StateDistrict {
    StateDistrict {
        kind: kind.to_string(),
        x,
        y,
        ..StateDistrict::default()
    }
}

fn plot(x: i32, y: i32, distance: i32, open: bool) -> StateActivationPlot {
    StateActivationPlot {
        x,
        y,
        distance,
        slot_open: Some(open),
    }
}

/// The capital's Palace at (10, 10), a Theater with an Amphitheater at
/// (11, 10), and a second city's Theater at (20, 10). One Great Person of
/// `class` stands at (10, 10).
fn board(class: &str, plots: Vec<StateActivationPlot>) -> StateSnapshot {
    StateSnapshot {
        cities: vec![
            StateCity {
                id: 1,
                x: 10,
                y: 10,
                buildings: ["BUILDING_PALACE", "BUILDING_AMPHITHEATER"]
                    .map(String::from)
                    .to_vec(),
                districts: vec![
                    district("DISTRICT_CITY_CENTER", 10, 10),
                    district("DISTRICT_THEATER", 11, 10),
                ],
                ..StateCity::default()
            },
            StateCity {
                id: 2,
                x: 21,
                y: 10,
                buildings: ["BUILDING_AMPHITHEATER"].map(String::from).to_vec(),
                districts: vec![
                    district("DISTRICT_CITY_CENTER", 21, 10),
                    district("DISTRICT_THEATER", 20, 10),
                ],
                ..StateCity::default()
            },
        ],
        units: vec![StateUnit {
            id: 7,
            kind: "UNIT_GREAT_WRITER".to_string(),
            x: 10,
            y: 10,
            great_person: Some(StateGreatPerson {
                class: Some(class.to_string()),
                can_activate: true,
                activation_plots: plots,
                ..StateGreatPerson::default()
            }),
            ..StateUnit::default()
        }],
        ..StateSnapshot::default()
    }
}

fn order(verb: &str, pos: Option<(i32, i32)>) -> Order {
    Order {
        kind: "unit",
        subject: Some(7),
        verb: Some(verb.to_string()),
        pos,
    }
}

const WRITER: &str = "GREAT_PERSON_CLASS_WRITER";

#[test]
fn a_writer_on_the_palace_walks_to_the_open_amphitheater() {
    let state = board(
        WRITER,
        vec![
            plot(10, 10, 0, true),
            plot(11, 10, 1, true),
            plot(20, 10, 10, true),
        ],
    );
    let mut orders = vec![order("ACTIVATE_GREAT_PERSON", None)];
    assert_eq!(retarget_writers(&state, &mut orders), 1);
    assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
    assert_eq!(orders[0].pos, Some((11, 10)));
}

#[test]
fn a_writer_walking_to_the_palace_takes_the_open_slot_elsewhere() {
    let state = board(WRITER, vec![plot(10, 10, 0, true), plot(20, 10, 6, true)]);
    let mut orders = vec![order("MOVE_TO", Some((10, 10)))];
    assert_eq!(retarget_writers(&state, &mut orders), 1);
    assert_eq!(orders[0].pos, Some((20, 10)));
}

#[test]
fn the_palace_stays_the_writers_when_nothing_else_is_open_or_near() {
    let full = board(WRITER, vec![plot(10, 10, 0, true), plot(11, 10, 1, false)]);
    let mut orders = vec![order("ACTIVATE_GREAT_PERSON", None)];
    assert_eq!(
        retarget_writers(&full, &mut orders),
        0,
        "the Amphitheater is full"
    );
    assert_eq!(orders[0].verb.as_deref(), Some("ACTIVATE_GREAT_PERSON"));

    let far = board(
        WRITER,
        vec![plot(10, 10, 0, true), plot(20, 10, DETOUR + 1, true)],
    );
    let mut orders = vec![order("ACTIVATE_GREAT_PERSON", None)];
    assert_eq!(retarget_writers(&far, &mut orders), 0, "beyond the detour");
}

#[test]
fn other_people_and_other_tiles_are_left_alone() {
    let artist = board(
        "GREAT_PERSON_CLASS_ARTIST",
        vec![plot(10, 10, 0, true), plot(11, 10, 1, true)],
    );
    let mut orders = vec![order("ACTIVATE_GREAT_PERSON", None)];
    assert_eq!(
        retarget_writers(&artist, &mut orders),
        0,
        "the Palace is the Artist's too"
    );

    let theater = board(WRITER, vec![plot(11, 10, 1, true), plot(20, 10, 4, true)]);
    let mut orders = vec![order("MOVE_TO", Some((11, 10)))];
    assert_eq!(
        retarget_writers(&theater, &mut orders),
        0,
        "already bound elsewhere"
    );
    assert_eq!(orders[0].pos, Some((11, 10)));
}
