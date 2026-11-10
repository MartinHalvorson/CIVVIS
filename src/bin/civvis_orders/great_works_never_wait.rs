//! `great-works-never-wait` on the live bridge: a Writer keeps out of the
//! Palace while another slot is open. The gene's reasoning lives in
//! `src/ai/advanced/great_works_never_wait.rs`.
//!
//! The Palace's one slot takes a work of any kind, and before Radio it is
//! the only Music slot a city can build; an Amphitheater's two slots take
//! Writing only. So a Writer that would activate on the Palace's plot, or
//! walks to it, goes instead to the nearest host activation plot with a
//! known-open slot that is not a city centre, within [`DETOUR`] tiles. Only
//! the host's own activation plots and their `slot_open` are read; with no
//! such plot the stock order stands.

use civvis::mirror::{StateActivationPlot, StateSnapshot};

use super::Order;

/// The farthest a Writer walks to keep the Palace slot free.
pub(super) const DETOUR: i32 = 6;

/// Whether `(x, y)` is the centre of our city that holds the Palace.
fn palace_plot(state: &StateSnapshot, x: i32, y: i32) -> bool {
    state.cities.iter().any(|city| {
        city.buildings
            .iter()
            .any(|building| building == "BUILDING_PALACE")
            && ((city.x, city.y) == (x, y)
                || city
                    .districts
                    .iter()
                    .any(|d| d.kind == "DISTRICT_CITY_CENTER" && (d.x, d.y) == (x, y)))
    })
}

/// Rewrites the Writers' Palace-bound orders in `orders`; returns how many
/// changed.
pub(super) fn retarget_writers(state: &StateSnapshot, orders: &mut [Order]) -> usize {
    let mut changed = 0;
    for unit in &state.units {
        let Some(person) = &unit.great_person else {
            continue;
        };
        if person.class.as_deref() != Some("GREAT_PERSON_CLASS_WRITER") {
            continue;
        }
        let Some(order) = orders
            .iter_mut()
            .find(|order| order.kind == "unit" && order.subject == Some(unit.id))
        else {
            continue;
        };
        let bound_for_the_palace = match order.verb.as_deref() {
            Some("ACTIVATE_GREAT_PERSON") => palace_plot(state, unit.x, unit.y),
            Some("MOVE_TO") => order.pos.is_some_and(|(x, y)| palace_plot(state, x, y)),
            _ => false,
        };
        if !bound_for_the_palace {
            continue;
        }
        let elsewhere = person
            .activation_plots
            .iter()
            .filter(|plot: &&StateActivationPlot| {
                plot.slot_open == Some(true)
                    && (1..=DETOUR).contains(&plot.distance)
                    && !palace_plot(state, plot.x, plot.y)
            })
            .min_by_key(|plot| (plot.distance, plot.y, plot.x));
        let Some(target) = elsewhere else {
            continue;
        };
        *order = Order {
            kind: "unit",
            subject: Some(unit.id),
            verb: Some("MOVE_TO".to_string()),
            pos: Some((target.x, target.y)),
        };
        changed += 1;
    }
    changed
}

#[cfg(test)]
#[path = "great_works_never_wait/tests.rs"]
mod tests;
