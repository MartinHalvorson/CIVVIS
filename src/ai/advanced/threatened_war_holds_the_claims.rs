//! `threatened-war-holds-the-claims`: while a war with a major threatens a
//! home city, the economy's idle-queue claims stand aside so those queues
//! reach the war target.
//!
//! **The finding (live Deity Korea science, 2026-11-04/05).** In 58 Science
//! games, 664 queue decisions fell on turns the plan read "recovery | at war
//! and losing ground at home", and only 25% were military. After the
//! insolvency guard (103, answered by `war-defense-spends-the-runway`), the
//! idle-queue claims took 309, about 47%:
//!
//! | Claim | Decisions |
//! |---|---|
//! | Trader for an idle route | 73 |
//! | Pillaged-district repairs | 60 |
//! | Campus buildings | 47 |
//! | Backlog Builder | 35 |
//! | Granary | 24 |
//! | Industrial Zone | 22 |
//! | Culture-defence Theater Square | 17 |
//! | Free-slot Campus | 12 |
//! | Neighbourhood | 11 |
//! | Factory | 8 |
//!
//! The claims run ahead of both governors, so `threatened-lane-lends-the-army`
//! lends its war target to a governor that is often left with no idle queue.
//!
//! **The gene.** While a living major is at war with us and a home city is
//! threatened (`home_city_threatened`: the plan names a threatened city, the
//! plan is Recovery, or a city of ours was attacked within four turns), the
//! production pass skips these claims:
//! - housing and growth: Granary, Aqueduct, Neighbourhood;
//! - trade and builders: idle-route Trader, backlog Builder, worked-backlog
//!   Builder;
//! - industry and science: Industrial Zone, Factory, Campus building,
//!   free-slot Campus, capital Campus;
//! - culture and lane wonders: Theater claims, Monuments, Statue Harbor,
//!   Diplomatic wonder, culture-defence Theater;
//! - civic investment: Plaza, higher-level investment;
//! - pillaged-district repairs.
//!
//! The war's own claims (deterrent, walls, siege support, recon, paused and
//! near-done finishes, the launch pad) and every queued item's commitment are
//! untouched: the gene only stops new economic claims on idle queues. With
//! the gene off, nothing changes.

use super::*;

impl AdvancedAi {
    /// `threatened-war-holds-the-claims`: whether the economy's idle-queue
    /// claims stand aside this production pass. True only with the gene on, a
    /// living major at war with `pid`, and a threatened home city.
    pub(super) fn threatened_war_holds_the_claims_holds(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.threatened_war_holds_the_claims {
            return false;
        }
        let enemy = g.players.iter().find(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && g.is_at_war(pid, other.id)
        });
        let Some(enemy) = enemy else {
            return false;
        };
        if !Self::home_city_threatened(g, pid, plan) {
            return false;
        }
        think!(self.journal(), Cities, Detail,
               "The war holds the economy's claims";
               "threatened-war-holds-the-claims: at war with player {} and a home city is \
                threatened ({} plan); idle queues go to the war target, not to Granaries, \
                Traders, Campus buildings or repairs", enemy.id, plan.strategy.as_str());
        true
    }
}

#[cfg(test)]
mod tests;
