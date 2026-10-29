//! Native governor growth recovery, using host food and allocation quotes.
//!
//! `Base/Assets/UI/Panels/CityPanel.lua:445-458` favors a yield with SET_FOCUS,
//! flags 0, PARAM_YIELD_TYPE and PARAM_DATA0=1; zero releases that preference.
//! The host still allocates citizens. A request is verified only by readback.

use civvis::{hex, mirror};

use super::{IssuedOrder, Verdict};

/// `food-focus-grows-the-small-city`: the largest city the widened request
/// reaches, and the largest Food surplus a turn at which it is made. See
/// `src/ai/advanced/food_focus_grows_the_small_city.rs`.
pub(super) const WIDE_MAX_POP: i32 = 5;
pub(super) const WIDE_MAX_SURPLUS: f64 = 2.0;

pub(super) struct Choice {
    pub city: i64,
    pub verb: &'static str,
}

fn richer_unworked_food(snapshot: &mirror::Snapshot, city: &mirror::StateCity) -> bool {
    let Some(worked) = &city.worked else {
        return false;
    };
    let Some(lowest) = worked
        .iter()
        .filter(|plot| (plot.x, plot.y) != (city.x, city.y))
        .filter_map(|plot| plot.yields.as_ref().map(|yields| yields.food))
        .filter(|food| food.is_finite() && *food >= 0.0)
        .min_by(f64::total_cmp)
    else {
        return false;
    };
    let center = hex::offset_to_axial(city.x, city.y);
    snapshot.revealed_positions().any(|pos| {
        snapshot.is_current(pos)
            && !worked.iter().any(|plot| (plot.x, plot.y) == pos)
            && hex::distance(center, hex::offset_to_axial(pos.0, pos.1)) <= 3
            && snapshot.plot(pos).is_some_and(|plot| {
                plot.oc == Some(city.id)
                    && !plot.i
                    && !plot.p
                    && plot.d.is_none()
                    && plot.host_yields().is_some_and(|yields| {
                        yields.food.is_finite() && yields.food >= lowest + 1.0
                    })
            })
    })
}

/// The focus requests for this frame. `widened` (the gene
/// `food-focus-grows-the-small-city`) also favours Food in a city of up to
/// [`WIDE_MAX_POP`] growing at no more than [`WIDE_MAX_SURPLUS`] a turn, not
/// only in one stalled at zero.
pub(super) fn choices(
    snapshot: &mirror::Snapshot,
    state: &mirror::StateSnapshot,
    widened: bool,
) -> Vec<Choice> {
    let mut choices = Vec::new();
    for city in &state.cities {
        let (Some(favored), Some(disfavored), Some(managed)) = (
            city.food_favored,
            city.food_disfavored,
            city.food_focus_managed,
        ) else {
            continue;
        };
        let Some(headroom) = city
            .housing
            .filter(|housing| housing.is_finite() && *housing >= 0.0)
            .map(|housing| housing - f64::from(city.pop))
        else {
            continue;
        };
        let threatened = city.damage > 0.0 || city.wall_damage > 0.0;
        let verb =
            if managed && (headroom < 2.0 || city.pop >= 6 || state.turn >= 130 || threatened) {
                Some("RELEASE_FOOD")
            } else if !favored
                && !disfavored
                && state.turn <= 100
                && (1..=if widened { WIDE_MAX_POP } else { 4 }).contains(&city.pop)
                && city.damage == 0.0
                && city.wall_damage == 0.0
                && headroom >= 2.0
                && city.food_surplus.is_finite()
                && (city.food_surplus.abs() <= 1e-6
                    || (widened && (0.0..=WIDE_MAX_SURPLUS).contains(&city.food_surplus)))
                && city.overall_growth_mult.is_finite()
                && city.overall_growth_mult > 0.0
                && snapshot.turn == state.turn
                && richer_unworked_food(snapshot, city)
            {
                Some("FAVOR_FOOD")
            } else {
                None
            };
        if let Some(verb) = verb {
            choices.push(Choice {
                city: city.id,
                verb,
            });
        }
    }
    choices
}

pub(super) fn verify(order: &IssuedOrder, after: &mirror::StateSnapshot) -> Verdict {
    let Some(city) = after
        .cities
        .iter()
        .find(|city| Some(city.id) == order.subject)
    else {
        return Verdict::Failed("city_gone".into());
    };
    let (Some(favored), Some(managed)) = (city.food_favored, city.food_focus_managed) else {
        return Verdict::Unverifiable;
    };
    match order.verb.as_deref() {
        Some("FAVOR_FOOD") if favored && managed => Verdict::Verified,
        Some("RELEASE_FOOD") if !favored && !managed => Verdict::Verified,
        Some("FAVOR_FOOD" | "RELEASE_FOOD") => Verdict::Failed("food_focus_unchanged".into()),
        _ => Verdict::Failed("unknown_food_focus".into()),
    }
}

#[cfg(test)]
#[path = "citizen_growth_tests.rs"]
mod tests;
