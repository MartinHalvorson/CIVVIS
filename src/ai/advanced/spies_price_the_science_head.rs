//! `spies-price-the-science-head` (opt-in): choose the eligible producer with
//! the earliest native Spy completion. On equal quotes, spare an existing
//! Campus-family district or building before comparing raw Production and id.
//! Both the Eureka-steal and denial training claims use the same ranking.
//!
//! Native Indonesia012732Z showed why blindly protecting a short Campus
//! would be wrong: the Spy interruption was real, but successful Eureka steals
//! can outproduce that Campus. This experiment instead preserves the earliest
//! quoted Spy completion and prices the science commitment only on ties.
//! It does not cancel a Spy already training or change mission selection.
//!
//! Only an assigned Science seat uses the new ranking, and only when every
//! eligible producer has a finite positive host quote. An incomplete menu or
//! a simulator-only board keeps the original Production/id ordering. All
//! eligibility, local safety, capacity, pause and emergency controls stay in
//! their callers. A native quote is a current forecast, not a guarantee of
//! future production, unchanged delivery time or victory strength.

use super::*;

impl AdvancedAi {
    pub(super) fn rank_spy_producers(&self, g: &Game, candidates: &mut [(f64, u32)]) {
        let spy = Item::Unit {
            unit: crate::name!("spy"),
        };
        let quoted = self.spies_price_the_science_head
            && self.active_victory_target(g) == Some(VictoryTarget::Science)
            && candidates.iter().all(|(_, cid)| {
                g.host_production_turns(*cid, &spy)
                    .is_some_and(|turns| turns.is_finite() && turns > 0.0)
            });
        let head_is_science = |cid: u32| {
            g.cities[&cid]
                .queue
                .first()
                .is_some_and(|head| Self::science_head(g, head))
        };
        candidates.sort_by(|(left, left_city), (right, right_city)| {
            let native_order = if quoted {
                // `quoted` validated the complete menu before this sort.
                g.host_production_turns(*left_city, &spy)
                    .unwrap()
                    .total_cmp(&g.host_production_turns(*right_city, &spy).unwrap())
                    .then_with(|| head_is_science(*left_city).cmp(&head_is_science(*right_city)))
            } else {
                std::cmp::Ordering::Equal
            };
            native_order
                .then_with(|| right.total_cmp(left))
                .then(left_city.cmp(right_city))
        });
    }
}

#[cfg(test)]
mod tests;
