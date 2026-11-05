//! `threatened-lane-lends-the-army`: on every victory lane, the delegated
//! production governor gets the war army target while a home city is
//! threatened.
//!
//! **The finding (live Deity Korea science, 2026-11-05).** The delegated
//! governor (`BasicAi::pick_item`) stops training units at `mil_per_city`
//! times the city count, 1.0 per city on the genome.
//! `domination_war_military_per_city` lends it the strategic target: two land
//! bodies per city, raised by `enemy_weighted_army_target` while a rival at
//! war outweighs us. That lend was written for the Gran Colombia domination
//! lane, and it returns `None` for any other victory target. The live seat now
//! plays Science, so a Science seat fighting for its life keeps the 1.0
//! floor.
//!
//! A replay of `civvis-20261105T033244Z` t168 reads: "Grand strategy:
//! recovery | at war and losing ground at home … power 577 against the
//! strongest rival's 3377". Then "Jinju starts the theater square festival
//! project … the empire holds 8 military for 6 cities against a target of 1.0
//! each", with a Commercial Hub, a Market and a Stock Exchange repair in the
//! other queues. The Aztecs took five cities between t176 and t202 and
//! eliminated the seat at t203. Since 03:17Z, played-out Deity games have
//! lost a mean of about 3.6 cities each.
//!
//! **The gene.** For a victory target other than Domination, the lend applies
//! while a living major is at war with us and a home city is threatened:
//! `home_city_threatened`, meaning the plan names a threatened city, the plan
//! is Recovery, or a city of ours was attacked within four turns. A war fought
//! far from home lends nothing, so the lane keeps its economy. Domination is
//! unchanged. With the gene off, nothing changes.

use super::*;

impl AdvancedAi {
    /// `threatened-lane-lends-the-army`: whether a seat whose victory target
    /// is not Domination gets the war army lend this call. True only with the
    /// gene on, a living major at war with `pid`, and a threatened home city.
    pub(super) fn threatened_lane_lends_the_army_applies(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        at_war: bool,
    ) -> bool {
        if !self.threatened_lane_lends_the_army || !at_war {
            return false;
        }
        if !Self::home_city_threatened(g, pid, plan) {
            return false;
        }
        think!(self.journal(), Military, Detail,
               "The war lends the governor its army target";
               "threatened-lane-lends-the-army: at war with a major and a home city is \
                threatened ({} plan); the {} lane's governor builds to the war target, \
                not the genome's {:.1} a city",
               plan.strategy.as_str(),
               self.active_victory_target(g).map_or("unassigned", |target| target.as_str()),
               self.base.w.mil_per_city);
        true
    }
}

#[cfg(test)]
mod tests;
