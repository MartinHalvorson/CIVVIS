//! `kristina-fills-to-theme` on the live bridge: a Sweden Writer, Artist or
//! Musician places its work where it fills a building Kristina themes. The
//! gene's reasoning lives in `src/ai/advanced/kristina_fills_to_theme.rs`.
//!
//! Two changes to the stock Great Person orders, both on the host's own
//! activation plots and their reported `slot_open`:
//! - a person walking to a plot prefers, after a known-open slot, the plot
//!   whose 3+-slot building is closest to full, then distance as before;
//! - a person able to activate where it stands, into a building that can never
//!   theme (the Palace, an Amphitheater), walks instead to a known-open plot
//!   that can, within [`DETOUR`] tiles.

use civvis::ai::AdvancedAi;
use civvis::mirror::{StateActivationPlot, StateSnapshot};

use super::Order;

/// The farthest a person that could activate now walks to theme instead.
pub(super) const DETOUR: i32 = 4;

/// Rewrites the cultural people's orders in `orders`; returns how many changed.
pub(super) fn retarget(state: &StateSnapshot, orders: &mut [Order]) -> usize {
    let mut changed = 0;
    for unit in &state.units {
        let Some(person) = &unit.great_person else {
            continue;
        };
        let Some(class) = person.class.as_deref() else {
            continue;
        };
        if !matches!(
            class,
            "GREAT_PERSON_CLASS_WRITER"
                | "GREAT_PERSON_CLASS_ARTIST"
                | "GREAT_PERSON_CLASS_MUSICIAN"
        ) {
            continue;
        }
        let Some(order) = orders
            .iter_mut()
            .find(|order| order.kind == "unit" && order.subject == Some(unit.id))
        else {
            continue;
        };
        let rank = |plot: &StateActivationPlot| {
            AdvancedAi::kristina_theme_rank(state, class, plot.x, plot.y)
        };
        let open = person
            .activation_plots
            .iter()
            .filter(|plot| plot.slot_open != Some(false));
        let target = match order.verb.as_deref() {
            Some("MOVE_TO") => open
                .min_by_key(|plot| {
                    (
                        plot.slot_open != Some(true),
                        rank(plot),
                        plot.distance,
                        plot.y,
                        plot.x,
                    )
                })
                .map(|plot| (plot.x, plot.y)),
            Some("ACTIVATE_GREAT_PERSON") => {
                let here_themes = person
                    .activation_plots
                    .iter()
                    .find(|plot| plot.distance == 0)
                    .is_none_or(|plot| rank(plot) < AdvancedAi::KRISTINA_NO_THEME);
                if here_themes {
                    None
                } else {
                    open.filter(|plot| {
                        plot.slot_open == Some(true)
                            && (1..=DETOUR).contains(&plot.distance)
                            && rank(plot) < AdvancedAi::KRISTINA_NO_THEME
                    })
                    .min_by_key(|plot| (rank(plot), plot.distance, plot.y, plot.x))
                    .map(|plot| (plot.x, plot.y))
                }
            }
            _ => None,
        };
        let Some(target) = target else {
            continue;
        };
        if order.verb.as_deref() == Some("MOVE_TO") && order.pos == Some(target) {
            continue;
        }
        *order = Order {
            kind: "unit",
            subject: Some(unit.id),
            verb: Some("MOVE_TO".to_string()),
            pos: Some(target),
        };
        changed += 1;
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;
    use civvis::mirror::{StateCity, StateDistrict, StateGreatPerson, StateGreatWork, StateUnit};

    fn work(building: &str, object: &str, slot: i32) -> StateGreatWork {
        StateGreatWork {
            kind: format!("GREATWORK_{building}_{slot}"),
            object: object.to_string(),
            building: building.to_string(),
            slot,
            ..StateGreatWork::default()
        }
    }

    fn district(kind: &str, x: i32, y: i32) -> StateDistrict {
        StateDistrict {
            kind: kind.to_string(),
            x,
            y,
            ..StateDistrict::default()
        }
    }

    fn plot(x: i32, y: i32, distance: i32) -> StateActivationPlot {
        StateActivationPlot {
            x,
            y,
            distance,
            slot_open: Some(true),
        }
    }

    /// Stockholm: Palace (empty) at (10, 10), a Theater at (11, 10) whose Art
    /// Museum holds two works, an Amphitheater beside it. Uppsala: a Theater
    /// at (20, 10) with an empty Art Museum.
    fn board(person: StateGreatPerson) -> StateSnapshot {
        StateSnapshot {
            cities: vec![
                StateCity {
                    id: 1,
                    buildings: [
                        "BUILDING_PALACE",
                        "BUILDING_AMPHITHEATER",
                        "BUILDING_MUSEUM_ART",
                    ]
                    .map(String::from)
                    .to_vec(),
                    districts: vec![
                        district("DISTRICT_CITY_CENTER", 10, 10),
                        district("DISTRICT_THEATER", 11, 10),
                    ],
                    great_works: Some(vec![
                        work("BUILDING_MUSEUM_ART", "GREATWORKOBJECT_PORTRAIT", 0),
                        work("BUILDING_MUSEUM_ART", "GREATWORKOBJECT_SCULPTURE", 1),
                    ]),
                    ..StateCity::default()
                },
                StateCity {
                    id: 2,
                    buildings: ["BUILDING_MUSEUM_ART"].map(String::from).to_vec(),
                    districts: vec![district("DISTRICT_THEATER", 20, 10)],
                    great_works: Some(Vec::new()),
                    ..StateCity::default()
                },
            ],
            units: vec![StateUnit {
                id: 7,
                kind: "UNIT_GREAT_ARTIST".to_string(),
                great_person: Some(person),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        }
    }

    fn artist(can_activate: bool, plots: Vec<StateActivationPlot>) -> StateGreatPerson {
        StateGreatPerson {
            class: Some("GREAT_PERSON_CLASS_ARTIST".to_string()),
            can_activate,
            activation_plots: plots,
            ..StateGreatPerson::default()
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

    #[test]
    fn the_rank_reads_how_close_a_theming_building_is_to_full() {
        let state = board(artist(false, Vec::new()));
        let class = "GREAT_PERSON_CLASS_ARTIST";
        assert_eq!(AdvancedAi::kristina_theme_rank(&state, class, 11, 10), 0);
        assert_eq!(AdvancedAi::kristina_theme_rank(&state, class, 20, 10), 2);
        assert_eq!(
            AdvancedAi::kristina_theme_rank(&state, class, 10, 10),
            AdvancedAi::KRISTINA_NO_THEME,
            "the Palace never themes"
        );
        assert_eq!(
            AdvancedAi::kristina_theme_rank(&state, "GREAT_PERSON_CLASS_WRITER", 11, 10),
            AdvancedAi::KRISTINA_NO_THEME,
            "a Writer's Amphitheater has two slots"
        );
    }

    #[test]
    fn a_walking_artist_completes_the_nearly_full_museum() {
        let state = board(artist(false, vec![plot(20, 10, 2), plot(11, 10, 6)]));
        let mut orders = vec![order("MOVE_TO", Some((20, 10)))];
        assert_eq!(retarget(&state, &mut orders), 1);
        assert_eq!(orders[0].pos, Some((11, 10)));
        // Already headed there: nothing changes.
        assert_eq!(retarget(&state, &mut orders), 0);
    }

    #[test]
    fn an_artist_at_the_palace_walks_to_a_museum_nearby() {
        let state = board(artist(true, vec![plot(10, 10, 0), plot(11, 10, 1)]));
        let mut orders = vec![order("ACTIVATE_GREAT_PERSON", None)];
        assert_eq!(retarget(&state, &mut orders), 1);
        assert_eq!(orders[0].verb.as_deref(), Some("MOVE_TO"));
        assert_eq!(orders[0].pos, Some((11, 10)));
        // On the museum's own plot it activates.
        let state = board(artist(true, vec![plot(11, 10, 0), plot(20, 10, 3)]));
        let mut orders = vec![order("ACTIVATE_GREAT_PERSON", None)];
        assert_eq!(retarget(&state, &mut orders), 0);
        // A museum beyond the detour is not worth the walk.
        let state = board(artist(
            true,
            vec![plot(10, 10, 0), plot(20, 10, DETOUR + 1)],
        ));
        let mut orders = vec![order("ACTIVATE_GREAT_PERSON", None)];
        assert_eq!(retarget(&state, &mut orders), 0);
    }
}
