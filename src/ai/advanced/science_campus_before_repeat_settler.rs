//! `science-campus-before-repeat-settler`: a short first Campus may precede
//! a longer repeated Settler for an explicit Science aim. Separate, unpriced
//! opt-in; the existing Korean Seowon exception is unchanged.
//!
//! Native Indonesia `civvis-20261106T003458Z` at 40/1 offered a five-turn
//! Campus while the capital's Settler needed eight turns, three cities stood
//! and another Settler was walking. The journal reported the Settler floor;
//! the first capital Campus foundation appeared at 48/1. This is a production
//! opportunity, not evidence of saved turns or a stronger native outcome.
//!
//! It supplies its own repeated-Settler claim even when both ordinary
//! free-slot Campus flags are off; it does not enable their broader claims.
//! The shared free-slot Campus picker still validates its site, capacity,
//! population and safety. The new exception requires a host quote for both
//! items, a Campus of at most five native turns, a strictly longer Settler,
//! at least two founded cities and a Settler already walking. Active major
//! war, a threatened city, a rush, recent damage and nearby barbarian raiders
//! keep the existing priorities. A committed first Campus is protected from
//! both Settler claims through zero-progress replans and repricing. The
//! expansion target and the displaced Settler's paid work are unchanged.

use super::*;

impl AdvancedAi {
    pub(super) fn science_campus_pause_safe(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        self.science_campus_before_repeat_settler
            && g.victory_conditions.science
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
            && g.player_city_ids(pid).len() >= 2
            && matches!(
                plan.strategy,
                GrandStrategy::Science | GrandStrategy::Expansion
            )
            && !plan.rush
            && plan.threatened_city.is_none()
            && g.cities.get(&cid).is_some_and(|city| {
                city.owner == pid
                    && (city.last_attacked == 0 || g.turn.saturating_sub(city.last_attacked) > 4)
                    && !Self::early_settler_raider_near(g, city.pos)
            })
            && !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
    }

    pub(super) fn science_campus_precedes_repeat_settler(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        campus: &Item,
    ) -> bool {
        self.science_campus_pause_safe(g, pid, cid, plan)
            && Self::early_settler_census(g, pid).0 > 0
            && matches!(campus, Item::District { district, .. }
                if g.district_family(*district) == "campus")
            && matches!(g.cities[&cid].queue.first(), Some(Item::Unit { unit }) if *unit == "settler")
            && g.host_production_turns(cid, campus).is_some_and(|turns| {
                turns.is_finite()
                    && turns > 0.0
                    && turns <= 5.0
                    && g.host_production_turns(cid, &Self::early_settler_item())
                        .is_some_and(|remaining| remaining.is_finite() && remaining > turns)
            })
    }

    pub(super) fn science_campus_holds_repeat_settler(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        // `first-campus-before-the-settler`: a Domination seat's committed
        // first Campus keeps its queue against the same two Settler claims.
        // See `advanced/first_campus_before_the_settler.rs`.
        if self.first_campus_holds_the_settler(g, pid, cid, plan) {
            return true;
        }
        // `campus-and-library-before-the-claims`: from four cities, the first
        // Campus or Library it holds keeps its queue against both Settler
        // claims too. See `advanced/campus_and_library_before_the_claims.rs`.
        if self.campus_and_library_holds_the_settler(g, pid, cid, plan) {
            return true;
        }
        self.science_campus_pause_safe(g, pid, cid, plan)
            && g.cities[&cid].queue.first().is_some_and(|item| {
                self.free_slot_campus_holds(g, pid, cid, plan, item)
                    && g.host_production_turns(cid, item)
                        .is_some_and(|turns| turns.is_finite() && turns > 0.0)
            })
    }
}

#[cfg(test)]
mod tests;
