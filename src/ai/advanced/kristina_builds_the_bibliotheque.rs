//! `kristina-builds-the-bibliotheque`: Sweden's Government Plaza holds the
//! Queen's Bibliotheque.
//!
//! **The rule.** `DLC/Expansion2/Data/Expansion2_Buildings_Major.xml`:
//! `BUILDING_QUEENS_BIBLIOTHEQUE` (Kristina's
//! `TRAIT_LEADER_BUILDING_QUEENS_BIBLIOTHEQUE`; Sweden has no other leader)
//! stands in `DISTRICT_GOVERNMENT`, needs a Tier2 government and one of
//! `BUILDING_GOV_TALL`, `BUILDING_GOV_WIDE` or `BUILDING_GOV_CONQUEST`, and
//! excludes `BUILDING_GOV_CITYSTATES`, `BUILDING_GOV_SPIES` and
//! `BUILDING_GOV_FAITH`. It holds two Writing, two Art and two Music slots and
//! earns 2 Great Writer, Artist and Musician points a turn. Under Minerva of
//! the North (`TRAIT_LEADER_KRISTINA_AUTO_THEME`) a building with three or
//! more slots themes itself when full.
//!
//! **Why it was never built.** `BasicAi::plaza_building_item_for` offers only
//! the Warlord's Throne then the Grand Master's Chapel, or the Ancestral Hall
//! then the Intelligence Agency; the Chapel and the Agency exclude the
//! Bibliotheque, so once either stood it could never be built. And the
//! empire's first Plaza waits for an open Prophet race and only ever goes in
//! the capital. Kristina Deity `civvis-20261110T140731Z`: the Plaza was
//! buildable in Stockholm t40-85 while the Prophet race held it back (Holy
//! Sites started t88-90); from t90 Stockholm had no district slot left, and no
//! Plaza stood by t150. A Great Musician idled t111-150 and an Artist t124-150
//! for want of a slot.
//!
//! Replayed on that board (t40-57, the capital idle), the culture seat's own
//! claims (Amphitheater, Walls, Settlers, Builders) took every idle queue
//! first, so a Plaza that waits for an idle queue never comes.
//!
//! **The gene.** On Sweden's seat the Plaza's tier buildings are the Ancestral
//! Hall (no drawback; a free Builder for every new city) and then the
//! Bibliotheque. The first Plaza does not wait for the Prophet race, and when
//! the capital has no site for it, any idle original city of ours that has one
//! takes it. And once a turn, as `plaza-serves-the-scientist-2/3` do for a
//! Science seat, the Plaza claims the head of a queue: the Plaza city's next
//! tier building first, else (once the capital's Theater Square stands, so the
//! culture seat's Theater comes first) the Plaza district in the capital, else
//! in the most productive city with a site. It displaces only routine work:
//! never a defensive building, a soldier, a Settler, a Spy, a Wonder, a
//! project, or the Theater chain itself (the Theater Square, Amphitheater, Art
//! or Archaeological Museum, Broadcast Center); never in the plan's threatened
//! city or one attacked in the last four turns. Every other seat, and Sweden
//! with the gene off, is unchanged.

use super::{AdvancedAi, StrategicPlan};
use crate::ai::BasicAi;
use crate::game::{Action, Game, Item};
use crate::think;

/// Turns since a city was last attacked under which it keeps its queue (the
/// Science Plaza claim's memory).
const KRISTINA_PLAZA_ATTACK_MEMORY: u32 = 4;

/// Sweden's Plaza tiers, in order. A Plaza already holding the Audience
/// Chamber or the Warlord's Throne cannot take the Hall and goes straight to
/// the Bibliotheque.
const SWEDEN_PLAZA_TIERS: [&str; 2] = ["ancestral_hall", "queens_bibliotheque"];

impl BasicAi {
    /// Whether the gene is on and `pid` plays Sweden.
    pub(crate) fn kristina_builds_here(&self, g: &Game, pid: usize) -> bool {
        self.kristina_builds_the_bibliotheque
            && g.players
                .get(pid)
                .is_some_and(|player| player.civ == "Sweden")
    }

    /// The standing Plaza's next tier building in `cid`: Sweden's tiers under
    /// the gene, else [`BasicAi::plaza_building_item_for`].
    pub(crate) fn plaza_tier_item(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if !self.kristina_builds_here(g, pid) {
            // `score-seat-expands-hard`: the Hall, then the Chapel.
            if self.plaza_hall_first {
                return Self::plaza_hall_first_item(g, pid, cid);
            }
            return Self::plaza_building_item_for(g, pid, cid, self.plaza_serves_science);
        }
        if !g.city_has_district_family(&g.cities[&cid], crate::name!("government_plaza")) {
            return None;
        }
        SWEDEN_PLAZA_TIERS
            .into_iter()
            .find_map(|building| Self::civ_building(g, pid, cid, building))
    }

    /// Whether an open Prophet race holds the first Plaza back. Never on
    /// Sweden's seat under the gene.
    pub(crate) fn plaza_waits_for_the_prophet(&self, g: &Game, pid: usize) -> bool {
        !self.kristina_builds_here(g, pid) && self.plaza_prophet_race_open(g, pid)
    }

    /// Whether `pid`'s original capital has no site for a Government Plaza
    /// (or no longer stands).
    pub(crate) fn capital_cannot_hold_the_plaza(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid)
            .into_iter()
            .find(|cid| {
                let city = &g.cities[cid];
                city.is_capital && city.original_owner == pid
            })
            .is_none_or(|capital| Self::plaza_site_item(g, pid, capital).is_none())
    }

    /// The empire's first Plaza for `cid`: [`BasicAi::plaza_item`], and on
    /// Sweden's seat under the gene any original city of ours when the capital
    /// has no site for one.
    pub(crate) fn plaza_item_for_seat(&self, g: &Game, pid: usize, cid: u32) -> Option<Item> {
        if let Some(item) = Self::plaza_item(g, pid, cid) {
            return Some(item);
        }
        if !self.kristina_builds_here(g, pid) {
            return None;
        }
        let city = &g.cities[&cid];
        if city.owner != pid
            || city.is_capital
            || city.original_owner != pid
            || !Self::plaza_absent(g, pid)
            || !Self::capital_cannot_hold_the_plaza(g, pid)
        {
            return None;
        }
        Self::plaza_site_item(g, pid, cid)
    }
}

impl AdvancedAi {
    /// Whether `item` belongs to the culture seat's Theater chain, which the
    /// Plaza never displaces.
    fn kristina_theater_chain(g: &Game, item: &Item) -> bool {
        match item {
            Item::District { district, .. } => g.district_family(*district) == "theater_square",
            Item::Building { building } => [
                crate::name!("amphitheater"),
                crate::name!("art_museum"),
                crate::name!("archaeological_museum"),
                crate::name!("broadcast_center"),
            ]
            .into_iter()
            .any(|family| g.building_is_family(*building, family)),
            _ => false,
        }
    }

    /// Whether city `cid` may give the head of its queue to the Plaza.
    fn kristina_plaza_may_claim(&self, g: &Game, plan: &StrategicPlan, cid: u32) -> bool {
        let city = &g.cities[&cid];
        let attacked = city.last_attacked > 0
            && g.turn.saturating_sub(city.last_attacked) <= KRISTINA_PLAZA_ATTACK_MEMORY;
        plan.threatened_city != Some(cid)
            && !attacked
            && city.queue.first().is_none_or(|head| {
                Self::spy_displaces(g, head) && !Self::kristina_theater_chain(g, head)
            })
    }

    /// See the module: the Plaza's next Sweden tier building, else the Plaza
    /// district, claims the head of one queue. Exact no-op with the gene off
    /// or off Sweden's seat.
    pub(super) fn claim_queue_for_kristinas_plaza(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if self.base.minor || self.base.barb || !self.base.kristina_builds_here(g, pid) {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        let tier = city_ids.iter().copied().find_map(|cid| {
            let item = self.base.plaza_tier_item(g, pid, cid)?;
            (!g.cities[&cid].queue.contains(&item)
                && self.kristina_plaza_may_claim(g, plan, cid)
                && g.can_produce(pid, cid, &item))
            .then_some((cid, item))
        });
        let claim = tier.or_else(|| {
            if !BasicAi::plaza_absent(g, pid) {
                return None;
            }
            let capital = city_ids.iter().copied().find(|cid| {
                let city = &g.cities[cid];
                city.is_capital && city.original_owner == pid
            });
            let theater_first = capital.is_some_and(|cid| {
                !g.city_has_district_family(&g.cities[&cid], crate::name!("theater_square"))
            }) && !BasicAi::capital_cannot_hold_the_plaza(g, pid);
            if theater_first {
                return None;
            }
            capital
                .filter(|cid| self.kristina_plaza_may_claim(g, plan, *cid))
                .and_then(|cid| BasicAi::plaza_site_item(g, pid, cid).map(|item| (cid, item)))
                .or_else(|| {
                    city_ids
                        .iter()
                        .copied()
                        .filter(|cid| {
                            Some(*cid) != capital && self.kristina_plaza_may_claim(g, plan, *cid)
                        })
                        .filter_map(|cid| {
                            BasicAi::plaza_site_item(g, pid, cid)
                                .map(|item| (g.city_yields(cid).production, cid, item))
                        })
                        .max_by(|a, b| a.0.total_cmp(&b.0).then(b.1.cmp(&a.1)))
                        .map(|(_, cid, item)| (cid, item))
                })
        });
        let Some((city, item)) = claim else {
            return;
        };
        let displaced = g.cities[&city]
            .queue
            .first()
            .map(Self::plain_item)
            .unwrap_or_else(|| "nothing".to_string());
        if g.apply(
            pid,
            &Action::Produce {
                city,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            *g.players[pid]
                .counters
                .entry("kristina_plaza_claims".to_string())
                .or_insert(0) += 1;
            think!(self.journal(), Economy, Decision,
                   "{} starts {} for Kristina's Plaza", g.cities[&city].name, Self::plain_item(&item);
                   "kristina-builds-the-bibliotheque: it goes ahead of {displaced}; the Queen's \
                    Bibliotheque's six Great Work slots wait on the Plaza and its Ancestral Hall");
        }
    }
}

#[cfg(test)]
mod tests;
