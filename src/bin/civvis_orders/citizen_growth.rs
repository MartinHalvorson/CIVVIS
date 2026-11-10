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
/// `food-focus-grows-the-small-city` v3: the last turn on which the capital
/// is the settler pump whatever it is training. Live Online-speed games aim
/// at a floor of six cities before turn 60.
pub(super) const CAPITAL_PUMP_UNTIL: u32 = 60;

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
/// only in one stalled at zero, unless it is training a Settler or is the
/// capital through turn [`CAPITAL_PUMP_UNTIL`]; and it releases a growing
/// city's focus when a Settler starts or while it is that capital.
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
        // `food-focus-grows-the-small-city`: a city training a Settler is the
        // settler pump, and the focus costs it about a sixth of its
        // Production. Live #4190 v1: the capital held the focus 19-50 of its
        // first 60 turns in 5 of 8 seated games, 6-32 of them on a Settler,
        // and the empire stood at 4 cities at t60 against 5.
        // v3: the capital is that pump for the whole opening. Russia Deity
        // 115325Z took the focus at t1, was refused its release 22 times
        // (the host's own allocation ran -2 Food at the housing cap, so the
        // agent restored the focus), made 5 Production a turn where the
        // released frames show 11, and ordered its first Settler at t44.
        let settler_pump = city.producing.as_deref() == Some("UNIT_SETTLER")
            || (city.capital && state.turn <= CAPITAL_PUMP_UNTIL);
        let verb = if managed
            && (headroom < 2.0
                || city.pop >= 6
                || state.turn >= 130
                || threatened
                || (widened && settler_pump && city.food_surplus > 1e-6))
        {
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
                || (widened
                    && !settler_pump
                    && (0.0..=WIDE_MAX_SURPLUS).contains(&city.food_surplus)))
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
