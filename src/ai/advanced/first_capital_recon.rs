//! `land-recon-before-the-first-capital`: give the missing land recon arm an
//! idle queue before economy claims while an early Domination opening lacks
//! a reachable capital. The existing arm still decides how many scouts are
//! missing, counts queued scouts and chooses the fastest safe idle city.
//!
//! Native Macedon runs 20261111T031847Z, 033227Z and 034202Z saw their first
//! rival major original capital at turns 97, never by 150, and 135. None
//! supplied the opening a capital before its Online turn-63 naming deadline.
//! The first run owned one Scout through turns 20, 40 and 60 even though the
//! existing recon arm asks for two once the second city exists. Its late claim
//! follows Campus, housing, industry and other economy reservations.
//!
//! This moves one existing claim earlier and keeps it through ordinary
//! production rescoring; otherwise a fresh Scout can become a Campus in the
//! same turn. It does not add a new scout quota,
//! replace a working queue or read an unobserved enemy city. The opening book,
//! first two cities, defense, war economy and a reachable capital keep priority.

use super::first_capital_before_the_walls::FIRST_CAPITAL_NAMING_DEADLINE;
use super::*;

impl AdvancedAi {
    fn first_capital_land_recon_needed(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> bool {
        self.land_recon_before_the_first_capital
            && self.first_capital_active()
            && self.base.recon_replacement
            && self.base.book_pos >= 4
            && g.player_city_ids(pid).len() >= 2
            && plan.threatened_city.is_none()
            && !g.players.iter().any(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && g.is_at_war(pid, rival.id)
            })
            && self.first_capital_target(g, pid).is_none()
    }

    pub(super) fn claim_first_capital_land_recon(
        &self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.first_capital_land_recon_needed(g, pid, plan)
            || g.turn > g.standard_duration(FIRST_CAPITAL_NAMING_DEADLINE)
            || self.counts(g, pid).scouts >= early_conquest::CONQUEST_SEARCH_SCOUTS
        {
            return;
        }
        self.reserve_idle_land_recon(g, pid, plan);
    }

    pub(super) fn first_capital_land_recon_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.first_capital_land_recon_needed(g, pid, plan)
            || !matches!(item, Item::Unit { unit } if g.rules.units.get(unit.as_str()).is_some_and(|spec|
                spec.promotion_class == "recon" && spec.domain.as_deref().unwrap_or("land") == "land"))
            || !Self::production_commitment_is_legal(g, pid, cid, item)
            || self.counts_without_city_queue(g, pid, cid).scouts
                >= early_conquest::CONQUEST_SEARCH_SCOUTS
            || (g.turn > g.standard_duration(FIRST_CAPITAL_NAMING_DEADLINE)
                && g.item_invested_production(cid, item) <= 0.0)
        {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests;
