//! `culture-digs-one-museum`: a Culture seat makes one Museum an
//! Archaeological Museum.
//!
//! The two Museums cost and yield the same and value the same in
//! `production_value` (Great Work slots × 180 for a Culture plan), and the
//! cost tie in the base AI now breaks toward the Art Museum. So a Culture seat
//! never owns an Artifact slot: live King culture seat civvis-20261101T091422Z
//! produced 66 Art Museum orders, no Archaeological Museum and no
//! Archaeologist. Its map listed Antiquity Sites, and it lost to a rival
//! Science finish at t237, 82 visitors short of the bar. The map generator
//! buries six Antiquity Sites and two Shipwrecks per civilization, and an
//! Artifact pays Tourism from the turn it is dug to the end of the game.
//!
//! Two paths pick a Museum, and the gene reaches both: the strategic scorer
//! (`production_value`, through [`AdvancedAi::dig_museum_bonus`]) and the
//! delegated baseline governor that `lane-delegates-production-2` hands every
//! city of a lane seat for the whole game, whose cost-tie sort in `ai.rs`
//! counts an Artifact slot at half an Art slot. For that call the seat lends
//! `BasicAi::digs_one_museum`, which counts it at double instead while the
//! empire owns none.
//!
//! One Museum is enough to open that line: once it stands, the
//! `natural_history` civic goal (`culture_museum_unlock_goal`) and the
//! Archaeologist valuation (`excavation_sites` > 0) already run in strategic
//! production. Every later Museum stays an Art Museum.
//!
//! Version two keeps that Museum choice and adds the missing delegated
//! production claim. Native Immortal Culture 20261103T002240Z completed
//! Archaeological Museums on turns 153 and 175, offered an Archaeologist for
//! eight turns at 160, but never owned one and lost to Science on 182 with
//! zero Artifacts. Version one was not armed in that source game. Fresh-board
//! version-one probes at 153 and 160 still never proposed a digger: the
//! delegated governor has no Archaeologist branch. Version two reserves one
//! idle Museum queue, at major peace and outside Recovery, with an owned land
//! dig within six hexes and a quote <=20 standard turns. No existing queue is
//! displaced; an active or queued digger blocks another. The fastest eligible
//! city wins, with city id breaking ties. Routine rescoring keeps that build;
//! emergency redirects retain their priority. This is an unmeasured candidate,
//! not proof of completed digs, Mary Leakey activation, or more wins.

use super::*;

/// Enough to break the Museums' exact tie, and to move the Museum ahead of an
/// ordinary building in the same city, without outbidding a district.
pub(super) const DIG_MUSEUM_BONUS: f64 = 300.0;

impl AdvancedAi {
    /// Whether the gene acts for this seat now: on, an active Culture target,
    /// and the lane still open.
    pub(super) fn dig_museum_lane_active(&self, g: &Game) -> bool {
        (self.culture_digs_one_museum || self.culture_digs_one_museum_2)
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
    }

    /// The bonus `production_value` adds to an Archaeological Museum while the
    /// Culture seat owns none, built or queued.
    pub(super) fn dig_museum_bonus(&self, g: &Game, pid: usize, building: Name) -> f64 {
        if !self.dig_museum_lane_active(g)
            || !g.building_is_family(building, crate::name!("archaeological_museum"))
            || crate::ai::BasicAi::owns_dig_museum(g, pid)
        {
            return 0.0;
        }
        DIG_MUSEUM_BONUS
    }
}

impl AdvancedAi {
    /// Preserve an invested digger through routine rescoring. Emergency queue
    /// redirects still run afterwards and retain their existing priority.
    pub(super) fn culture_digger_holds(&self, g: &Game, item: &Item) -> bool {
        self.culture_digs_one_museum_2
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Culture)
            && matches!(item, Item::Unit { unit } if unit == "archaeologist")
    }

    /// At peace, fund one digger from an idle Museum city with an owned land
    /// dig nearby. The quote cap is twenty standard turns. Proximity is a
    /// screening condition, not a promise of a legal native walking route.
    pub(super) fn culture_digger_city(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        if !self.culture_digs_one_museum_2
            || !self.dig_museum_lane_active(g)
            || plan.strategy == GrandStrategy::Recovery
            || g.players.iter().any(|other| {
                other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && other.id != pid
                    && g.is_at_war(pid, other.id)
            })
            || g.units
                .values()
                .any(|unit| unit.owner == pid && unit.kind == "archaeologist")
            || g.cities.values().any(|city| {
                city.owner == pid
                    && city
                        .queue
                        .iter()
                        .any(|item| matches!(item, Item::Unit { unit } if unit == "archaeologist"))
            })
        {
            return None;
        }
        let sites: Vec<Pos> = g
            .excavation_sites(pid)
            .into_iter()
            .filter(|(pos, improvement)| {
                improvement == "archaeological_dig"
                    && g.map.tiles[pos]
                        .owner_city
                        .and_then(|cid| g.cities.get(&cid))
                        .is_some_and(|city| city.owner == pid)
            })
            .map(|(pos, _)| pos)
            .collect();
        if sites.is_empty() {
            return None;
        }
        let item = Item::Unit {
            unit: crate::name!("archaeologist"),
        };
        g.player_city_ids(pid)
            .into_iter()
            .filter_map(|cid| {
                let city = &g.cities[&cid];
                if !city.queue.is_empty()
                    || plan.threatened_city == Some(cid)
                    || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    || !city
                        .buildings
                        .iter()
                        .any(|b| g.building_is_family(*b, crate::name!("archaeological_museum")))
                    || !sites.iter().any(|site| g.wdist(city.pos, *site) <= 6)
                    || !g.can_produce(pid, cid, &item)
                {
                    return None;
                }
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    g.item_remaining_cost_for_city(pid, cid, &item)
                        / g.city_yields(cid).production.max(0.5)
                });
                let cap = g.standard_duration(20) as f64;
                (turns.is_finite()
                    && turns <= cap
                    && g.max_turns.saturating_sub(g.turn) as f64 > turns + cap)
                    .then_some((turns, cid))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, cid)| cid)
    }

    pub(super) fn claim_culture_digger(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(cid) = self.culture_digger_city(g, pid, plan) else {
            return;
        };
        let item = Item::Unit {
            unit: crate::name!("archaeologist"),
        };
        if g.apply(pid, &Action::Produce { city: cid, item }).is_ok() {
            think!(self.journal(), Cities, Decision,
                "{} funds one Archaeologist for its Museum", g.cities[&cid].name;
                "culture-digs-one-museum-2: an idle queue, an owned nearby dig, no digger active or queued");
        }
    }
}

#[cfg(test)]
mod tests;
