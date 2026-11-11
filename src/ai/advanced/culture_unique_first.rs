//! `culture-road-takes-the-unique-first`: a Culture seat takes the civic that
//! unlocks its own civilization's largest per-city Culture source ahead of
//! every other road civic, and has the Builders for it standing when it lands.
//!
//! **Why (live Deity Kristina, 2026-11-10).** Sweden's Open-Air Museum
//! (Nationalism; +2 Culture and Tourism for each terrain class holding a
//! Swedish city, one per city) is the seat's largest Culture source, and Cold
//! War landed about 27 turns after Nationalism in every game:
//!
//! | game | Nationalism | road civics outside its closure taken first | Culture t100 / t110 / t120 | Cold War |
//! |---|---:|---|---|---:|
//! | 173750Z | t101 | Mercantilism (360) | 93 / 199 / 259 | t128 |
//! | 164254Z | t112 | Mercantilism (360) | 57 / 83 / 101 | t146 |
//! | 183027Z | t116 | Mercantilism, Colonialism, Natural History, Civil Engineering (1,790) | 72 / 102 / 152 | t141 |
//! | 181911Z | t124 | the same four (1,790) | 62 / 84 / 85 | t149 |
//! | 192354Z | t126 | the same four (1,790) | 49 / 65 / 88 | t151 |
//! | 193814Z | t130 | the same four (1,790) | 41 / 69 / 71 | t158 |
//! | 185626Z | t135 | the same four (1,790) | 49 / 56 / 60 | t159 |
//!
//! Once `cold-war-before-conservation` points the beeline at Cold War, the most
//! valuable step is Mercantilism, then Colonialism, Natural History and Civil
//! Engineering, none of them a prerequisite of Nationalism; the museum unlock
//! goal pulls Natural History forward too. And the Museums trickle in: 0-5
//! stood five turns after Nationalism and 0-8 ten turns after, on 6-9 cities,
//! because Builder charges on hand that turn were 0-13 against 6-9 cities.
//!
//! **What the gene does**, for an active Culture target only, with the source
//! read from the rules rather than named. The *source* is the unique
//! improvement or building of the seat's civilization that carries Culture
//! (a Culture yield or a Culture effect), unlocked by a civic on the road to
//! Cold War; among several, the largest per-city Culture. A civilization with
//! none, or whose source is unlocked by a technology or off the road, plays
//! exactly as before.
//!
//! 1. **Order.** Once the government holds six policy slots (the tier-2 rung
//!    `cold-war-before-conservation` also waits for), until the unlock civic
//!    is known, the civic slot's forced goal is that civic, above the museum
//!    unlock and the Cold War goal. The Inspiration ordering of
//!    `culture-road-earns-its-inspirations` still applies to its steps.
//! 2. **Builders.** For a one-per-city improvement, from
//!    [`UNIQUE_BUILDER_LEAD`] standard turns before the unlock civic completes
//!    until every city has one, a city claims a Builder while the empire's
//!    charges (on hand and queued) fall short of the cities still without one;
//!    one Builder a turn, in the fastest city whose queue head may wait.
//!
//! Off, nothing changes.

use super::{AdvancedAi, StrategicPlan, VictoryTarget};
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::reasoning::plain;
use crate::think;

/// Policy slots the government must hold before the unlock goes first: the
/// tier-2 rung. A four-slot government still climbs to it.
pub(super) const UNIQUE_FIRST_SLOTS: i64 = 6;

/// How long before the unlock civic completes, in standard turns, the
/// Builders are queued: a Builder takes two to four Online turns to build.
pub(super) const UNIQUE_BUILDER_LEAD: u32 = 10;

/// The civic whose prerequisite closure is the road.
const ROAD_GOAL: &str = "cold_war";

/// What the seat's source is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UniqueSource {
    /// A Builder improvement; `one_per_city` when the rules cap it so.
    Improvement {
        name: Name,
        one_per_city: bool,
    },
    Building(Name),
}

impl AdvancedAi {
    /// On, an active Culture target, and the lane still open.
    pub(super) fn unique_first_active(&self, g: &Game) -> bool {
        self.culture_road_takes_the_unique_first
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// The seat's largest per-city Culture source among its civilization's
    /// unique improvements and buildings, with the road civic that unlocks
    /// it. `None` for a civilization with none on the road.
    pub(super) fn unique_culture_source(g: &Game, pid: usize) -> Option<(UniqueSource, Name)> {
        let culture = |yields: f64, effects: &std::collections::BTreeMap<String, f64>| {
            yields
                + effects
                    .iter()
                    .filter(|(key, _)| key.contains("culture"))
                    .map(|(_, value)| value.max(0.0))
                    .sum::<f64>()
        };
        let on_road = |civic: Name| {
            civic == ROAD_GOAL
                || g.rules
                    .civic_ancestors
                    .get(ROAD_GOAL)
                    .is_some_and(|ancestors| ancestors.contains(civic.as_str()))
        };
        let improvements = g.rules.improvements.iter().filter_map(|(name, spec)| {
            let civic = spec.civic?;
            (spec
                .unique_to
                .as_deref()
                .is_some_and(|owner| g.owns_civ_unique(pid, owner))
                && spec.builder_buildable)
                .then(|| {
                    (
                        culture(spec.yields.culture, &spec.effects),
                        UniqueSource::Improvement {
                            name: *name,
                            one_per_city: spec.one_per_city,
                        },
                        civic,
                    )
                })
        });
        let buildings = g.rules.buildings.iter().filter_map(|(name, spec)| {
            let civic = spec.civic?;
            spec.unique_to
                .as_deref()
                .is_some_and(|owner| g.owns_civ_unique(pid, owner))
                .then(|| {
                    (
                        culture(spec.yields.culture, &spec.effects),
                        UniqueSource::Building(*name),
                        civic,
                    )
                })
        });
        improvements
            .chain(buildings)
            .filter(|(value, _, civic)| *value > 0.0 && on_road(*civic))
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, source, civic)| (source, civic))
    }

    /// The civic slot's forced goal on the seat: see the module. `None` with
    /// the gene off, before the tier-2 rung, or once the civic is known.
    pub(super) fn unique_first_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if !self.unique_first_active(g) {
            return None;
        }
        let (_, civic) = Self::unique_culture_source(g, pid)?;
        if g.players[pid].civics.contains(&civic) {
            return None;
        }
        let slots = g.gov_slots(pid);
        let capacity = slots.military + slots.economic + slots.diplomatic + slots.wildcard;
        (capacity >= UNIQUE_FIRST_SLOTS).then_some(civic.as_str())
    }

    /// See the module: one Builder a turn while charges fall short of the
    /// cities still without the one-per-city source, from shortly before its
    /// civic lands. Exact no-op with the gene off or for another target.
    pub(super) fn claim_unique_source_builders(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.unique_first_active(g) {
            return;
        }
        let Some((
            UniqueSource::Improvement {
                name: improvement,
                one_per_city: true,
            },
            civic,
        )) = Self::unique_culture_source(g, pid)
        else {
            return;
        };
        let player = &g.players[pid];
        let known = player.civics.contains(&civic);
        if !known {
            if player.civic.as_deref() != Some(civic.as_str()) {
                return;
            }
            let left = (g.civic_cost(civic.as_str()) - player.civic_progress).max(0.0)
                / Self::research_rate(g, pid, false);
            if left > g.standard_duration(UNIQUE_BUILDER_LEAD) as f64 {
                return;
            }
        }
        let builder = crate::name!("builder");
        let Some(per_builder) = g
            .rules
            .units
            .get(builder.as_str())
            .map(|spec| spec.charges.max(1))
        else {
            return;
        };
        let missing = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                !g.cities[cid].owned_tiles.iter().any(|pos| {
                    g.map.get(*pos).is_some_and(|tile| {
                        tile.improvement.as_deref() == Some(improvement.as_str())
                    })
                })
            })
            .count() as i64;
        let on_hand: i64 = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == builder)
            .map(|unit| i64::from(unit.charges.max(0)))
            .sum();
        let item = Item::Unit { unit: builder };
        let queued = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.cities[cid].queue.contains(&item))
            .count() as i64;
        if on_hand + queued * i64::from(per_builder) >= missing {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let choice = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let city = &g.cities[&cid];
                if plan.threatened_city == Some(cid)
                    || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    || city.queue.contains(&item)
                    || !g.can_produce(pid, cid, &item)
                {
                    return None;
                }
                if let Some(head) = city.queue.first() {
                    if matches!(head, Item::Unit { unit } if *unit == builder)
                        || !Self::culture_theater_may_defer(g, pid, cid, head, at_major_war)
                    {
                        return None;
                    }
                }
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    (g.item_cost_for_city(pid, cid, &item) - g.item_invested_production(cid, &item))
                        .max(0.0)
                        / g.city_yields(cid).production.max(0.5)
                });
                Some((turns, cid))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        let Some((turns, cid)) = choice else {
            return;
        };
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            think!(self.journal(), Cities, Decision,
                "{} trains a Builder for the {}", g.cities[&cid].name, plain(&improvement);
                "culture-road-takes-the-unique-first: {} charges on hand or queued against {} \
                 cities without one, {} {}; {:.0} turns",
                on_hand + queued * i64::from(per_builder), missing, plain(&civic),
                if known { "known" } else { "lands soon" }, turns);
        }
    }
}

#[cfg(test)]
mod tests;
