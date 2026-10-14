//! `civic-awaits-its-inspiration`: a civic whose inspiration is still to be
//! earned is taken after the civics that have nothing left to earn.
//!
//! **Why (157 live Emperor GC runs, 10-08/09).** An inspiration pays 40% of a
//! civic, but only while the civic is unfinished: the host credits it
//! mid-research and skips a civic already adopted. The civic chooser runs the
//! moment the previous civic completes and never reads the trigger, so the
//! opening adopts its civics before their own triggers land:
//!
//! | civic | inspired | trigger landed after adoption, within 5 / 10 / 15 turns |
//! |---|---:|---|
//! | Craftsmanship (3 improvements) | 9% | 16 / 49 / 71 of 136 |
//! | State Workforce (a specialty district) | 23% | 52 / 72 / 79 of 115 |
//! | Games and Recreation (Construction) | 9% | 15 / 40 / 62 of 130 |
//! | Recorded History (2 Campuses) | 5% | 11 / 35 / 57 of 138 |
//!
//! Our Culture ran 0.28-0.31 of the median rival's at turns 50 to 150 and our
//! civics 8 / 19 / 28 against the best rival's 13 / 25 / 36 at turns 50 / 100 /
//! 150. Nearly every civic before Monarchy is a step of a forced beeline
//! (Political Philosophy, then Divine Right): the live seat took Craftsmanship
//! at turn 8-9 with no Builder on the map, while Foreign Trade and Early Empire
//! — steps of the same beeline — waited. Reordering a goal's own steps cannot
//! delay the goal: every step is paid before it, whatever the order, and an
//! inspiration only makes the sum smaller.
//!
//! **What the gene does.**
//!
//! - **On a forced beeline:** a step whose inspiration is not in hand and
//!   whose trigger the empire can still earn (improvements, districts,
//!   buildings, units, a technology, population, wonders, city-state
//!   contacts, a pantheon) gives way to another step of the same goal that has
//!   nothing left to earn (no inspiration, one in hand, or one only a rival
//!   or the map grants), chosen the way the beeline chooses.
//! - **Off a beeline:** a pick whose trigger the empire is visibly about to
//!   complete — a Builder with charges for the improvements, a district or
//!   wonder at the head of an owned queue, the technology in research, a farm
//!   or two short with Builders out — and which would finish before it lands
//!   (but within [`INSPIRATION_WAIT_TURNS`]) gives way to the best civic worth
//!   [`INSPIRATION_WAIT_BAND`] of it that would not.
//!
//! With no alternative the pick stands. Off, the chooser is byte-identical.

use super::{AdvancedAi, GrandStrategy};
use crate::game::{Game, Item};
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;

/// The longest a civic waits for its own inspiration, in live turns. The
/// opening's civics take four to eight turns each, so a wait inside this is
/// spent on another civic the empire takes anyway.
pub(super) const INSPIRATION_WAIT_TURNS: f64 = 8.0;

/// Off a beeline, an alternative must be worth this share of the pick.
pub(super) const INSPIRATION_WAIT_BAND: f64 = 0.5;

impl AdvancedAi {
    /// Live turns until `civic`'s inspiration trigger lands, with the trigger's
    /// name, when the empire is visibly about to complete it; `None` for a
    /// civic with no boost, one in hand, and a trigger nothing in hand
    /// advances (contacts, continents, declarations, religions, growth).
    pub(super) fn civic_inspiration_eta(
        g: &Game,
        pid: usize,
        civic: &str,
    ) -> Option<(f64, String)> {
        let boost = g.rules.civics.get(civic)?.boost.as_ref()?;
        if Self::boost_in_hand(g, pid, civic, false) {
            return None;
        }
        let trigger = boost.trigger.as_str();
        let need = boost.count.max(1);
        let builders = || -> i64 {
            g.units
                .values()
                .filter(|unit| unit.owner == pid && unit.kind == "builder" && unit.charges > 0)
                .count() as i64
        };
        let improved = |want: Option<&str>| -> i64 {
            g.cities
                .values()
                .filter(|city| city.owner == pid)
                .flat_map(|city| city.owned_tiles.iter())
                .filter(|pos| {
                    g.map.get(**pos).is_some_and(|tile| {
                        !tile.pillaged
                            && tile.improvement.as_deref().is_some_and(|have| {
                                want.is_none_or(|want| have == want)
                            })
                    })
                })
                .count() as i64
        };
        let eta = match trigger {
            "improvements" => {
                let remaining = need - improved(None);
                if remaining <= 0 {
                    return None;
                }
                let crews = builders();
                if crews > 0 {
                    (remaining as f64 / crews as f64).ceil() + 1.0
                } else {
                    Self::queue_head_turns(g, pid, |item| {
                        matches!(item, Item::Unit { unit } if unit.as_str() == "builder")
                    })? + remaining as f64
                }
            }
            "improvement:farm" => {
                let remaining = need - improved(Some("farm"));
                let crews = builders();
                if remaining <= 0 || remaining > 2 || crews == 0 {
                    return None;
                }
                (remaining as f64 / crews as f64).ceil() + 1.0
            }
            "specialty_districts" => Self::queue_head_turns(g, pid, |item| {
                matches!(item, Item::District { district, .. }
                    if g.rules
                        .districts
                        .get(g.district_family(*district).as_str())
                        .is_some_and(|spec| spec.specialty))
            })?,
            "wonders" => {
                Self::queue_head_turns(g, pid, |item| matches!(item, Item::Wonder { .. }))?
            }
            _ => {
                if let Some(family) = trigger.strip_prefix("district:") {
                    let (have, _) = g.boost_progress(pid, boost);
                    if need - have != 1 {
                        return None;
                    }
                    Self::queue_head_turns(g, pid, |item| {
                        matches!(item, Item::District { district, .. }
                            if g.district_family(*district) == family)
                    })?
                } else if let Some(tech) = trigger.strip_prefix("tech:") {
                    let player = &g.players[pid];
                    if player.research.as_deref() != Some(tech) {
                        return None;
                    }
                    let left = (g.tech_cost(tech) - player.research_progress).max(0.0);
                    (left / Self::research_rate(g, pid, true)).ceil().max(1.0)
                } else {
                    return None;
                }
            }
        };
        Some((eta, trigger.to_string()))
    }

    /// Turns to the first completion among owned cities whose queue head
    /// matches `wanted`, read from the host's own count when the export
    /// carried it.
    fn queue_head_turns(g: &Game, pid: usize, wanted: impl Fn(&Item) -> bool) -> Option<f64> {
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let item = g.cities[&cid].queue.first()?;
                if !wanted(item) {
                    return None;
                }
                let turns = g.host_production_turns(cid, item).unwrap_or_else(|| {
                    (g.item_cost_for_city(pid, cid, item) - g.item_invested_production(cid, item))
                        .max(0.0)
                        / g.city_yields(cid).production.max(0.5)
                });
                Some(turns.ceil().max(1.0))
            })
            .min_by(f64::total_cmp)
    }

    /// Live turns to finish `civic` from scratch at today's Culture.
    fn civic_finish_turns(g: &Game, pid: usize, civic: &str) -> f64 {
        (g.civic_cost(civic) / Self::research_rate(g, pid, false)).ceil().max(1.0)
    }

    /// Would taking `civic` now finish it before its near inspiration lands?
    /// The landing turn and the trigger, when it would.
    pub(super) fn civic_misses_its_inspiration(
        g: &Game,
        pid: usize,
        civic: &str,
    ) -> Option<(f64, String)> {
        let (eta, trigger) = Self::civic_inspiration_eta(g, pid, civic)?;
        let finish = Self::civic_finish_turns(g, pid, civic);
        (eta > finish && eta <= INSPIRATION_WAIT_TURNS).then_some((eta, trigger))
    }

    /// Is `civic`'s inspiration still the empire's to earn: a boost not in
    /// hand whose trigger our own building, research, growth or exploration
    /// advances? A trigger only a rival or the map grants (another continent,
    /// a declaration, a religion) is not waited for.
    pub(super) fn civic_inspiration_pending(g: &Game, pid: usize, civic: &str) -> bool {
        let Some(boost) = g.rules.civics.get(civic).and_then(|spec| spec.boost.as_ref()) else {
            return false;
        };
        if Self::boost_in_hand(g, pid, civic, false) {
            return false;
        }
        let trigger = boost.trigger.as_str();
        matches!(
            trigger,
            "improvements"
                | "specialty_districts"
                | "districts"
                | "total_pop"
                | "pop"
                | "wonders"
                | "met_city_states"
                | "pantheon"
                | "camps"
                | "land_units"
                | "units"
        ) || ["improvement:", "improvement_on_resource:", "improve_resource:", "district:", "building:", "units_of:", "tech:"]
            .iter()
            .any(|prefix| trigger.starts_with(prefix))
    }

    /// The civic to take instead of `pick`, or `None` to keep it. `goal` is
    /// the forced goal `pick` stepped toward, when it came from a beeline.
    pub(super) fn civic_awaits_its_inspiration_pick(
        &self,
        g: &Game,
        pid: usize,
        available: &[Name],
        pick: &Name,
        goal: Option<&str>,
        strategy: GrandStrategy,
    ) -> Option<Name> {
        if !self.civic_awaits_its_inspiration {
            return None;
        }
        if let Some(goal) = goal {
            if !Self::civic_inspiration_pending(g, pid, pick.as_str()) {
                return None;
            }
            let steps: Vec<Name> = available
                .iter()
                .filter(|civic| *civic != pick && self.civic_leads_to(g, civic, goal))
                .filter(|civic| !Self::civic_inspiration_pending(g, pid, civic.as_str()))
                .cloned()
                .collect();
            let choice = if self.beeline_orders_by_value {
                self.beeline_step(g, pid, strategy, &steps, false)
            } else {
                steps.into_iter().min_by(|a, b| {
                    self.beeline_step_cost(g, pid, a.as_str(), false)
                        .total_cmp(&self.beeline_step_cost(g, pid, b.as_str(), false))
                        .then(a.cmp(b))
                })
            }?;
            let trigger = g.rules.civics[pick.as_str()]
                .boost
                .as_ref()
                .map_or(String::new(), |boost| boost.trigger.replace(['_', ':'], " "));
            think!(self.journal(), Research, Decision,
                "Adopting {} before {}", plain(&choice), plain(pick);
                "civic-awaits-its-inspiration: both lead to {}; {}'s inspiration ({}) is still \
                 ours to earn, {} has nothing left to earn",
                plain(goal), plain(pick), trigger, plain(&choice));
            return Some(choice);
        }
        let (eta, trigger) = Self::civic_misses_its_inspiration(g, pid, pick.as_str())?;
        let floor = self.civic_value(g, pid, pick.as_str(), strategy) * INSPIRATION_WAIT_BAND;
        let choice = available
            .iter()
            .filter(|civic| *civic != pick)
            .filter(|civic| Self::civic_misses_its_inspiration(g, pid, civic.as_str()).is_none())
            .map(|civic| (self.civic_value(g, pid, civic.as_str(), strategy), *civic))
            .filter(|(value, _)| *value >= floor)
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            .map(|(_, civic)| civic)?;
        think!(self.journal(), Research, Decision,
            "Adopting {} before {}", plain(&choice), plain(pick);
            "civic-awaits-its-inspiration: {}'s inspiration ({}) lands in about {:.0} turns, \
             after it would finish",
            plain(pick), trigger.replace('_', " "), eta);
        Some(choice)
    }
}

#[cfg(test)]
mod tests;
