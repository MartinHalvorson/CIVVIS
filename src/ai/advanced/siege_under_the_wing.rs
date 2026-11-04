//! `siege-moves-under-the-wing`: a Domination siege whose objective stands
//! behind walls our land guns cannot breach, beyond the Bomber wing's reach,
//! moves to another city of the same rival that the wing can hit.
//!
//! Post-reboot live Immortal games of October 27-28 that reached turn 160
//! with Bombers fielded spent 385 of 668 siege-turns (58%) on an objective
//! more than ten tiles from every city of ours. On 120 of those turns another
//! city of the same owner stood within that reach.
//! - `civvis-20261027T233902Z` held its siege on Pewen Mapu and Wente Mapu,
//!   14-18 tiles out, from turn 150 to 193, with "the walls hold the ask"
//!   (our best gun hit 5-8 a shot against 114-125). Ina Pire Mapu stood at
//!   the wing's reach the whole time and was taken up only at turn 199.
//! - `civvis-20261028T060959Z` besieged Quảng Trị (400 walls, our guns hit
//!   about 6) for 40 turns at 2.5-3 times Vietnam's power, 22 tiles from
//!   home. Its Bombers spent 21 strikes on a city-state four tiles out, and
//!   it lost on Diplomacy with no capture.
//!
//! A captured city is a base, so each capture moves the wing's reach forward.

use super::*;

impl AdvancedAi {
    /// The city the campaign should besiege instead of `prior_id`, or `None`
    /// to keep it. It moves only when every one of these holds:
    /// - the gene is on and the seat plays Conquest for Domination;
    /// - `prior_id` is a major's city we are at war with;
    /// - its walls and health are full, so no siege has opened it;
    /// - our land guns cannot breach it (`breakers-match-the-walls` asks no
    ///   count of guns);
    /// - at least `AIR_SURGE_LAUNCH_BOMBERS` Bombers stand, and the wing
    ///   cannot reach it.
    ///
    /// The new city is the same rival's, within the wing's reach, with a land
    /// staging route, and not stood down; the lowest campaign value wins.
    pub(super) fn siege_under_the_wing_city(
        &self,
        g: &Game,
        pid: usize,
        prior_id: u32,
        strategy: GrandStrategy,
    ) -> Option<u32> {
        if !self.siege_moves_under_the_wing
            || strategy != GrandStrategy::Conquest
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let prior = g.cities.get(&prior_id)?;
        let owner = prior.owner;
        if owner == pid
            || !g.is_at_war(pid, owner)
            || g.players[owner].is_minor
            || g.players[owner].is_barbarian
            || prior.hp < CITY_MAX_HP
            || prior.wall_hp <= 0
            || prior.wall_hp < g.city_max_wall_hp(prior)
        {
            return None;
        }
        if !matches!(
            self.breakers_matched_to_walls(g, pid, prior_id),
            Some(siege_production::BreakerMatch { ask: None, .. })
        ) {
            return None;
        }
        let wing = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && Self::air_surge_is_bomber(g, unit.kind))
            .count();
        if wing < air_surge::AIR_SURGE_LAUNCH_BOMBERS || Self::air_surge_in_range(g, pid, prior.pos)
        {
            return None;
        }
        g.cities
            .values()
            .filter(|city| {
                city.owner == owner
                    && city.id != prior_id
                    && Self::air_surge_in_range(g, pid, city.pos)
                    && !self.capture_stood_down_holds(g, city.id)
                    && !Self::should_defer_city_capture(g, pid, city.id)
                    && self.war_staging_route(g, pid, owner, city.pos).is_some()
            })
            .min_by(|left, right| {
                self.campaign_city_value(g, pid, left, strategy)
                    .total_cmp(&self.campaign_city_value(g, pid, right, strategy))
                    .then(left.id.cmp(&right.id))
            })
            .map(|city| city.id)
    }
}

#[cfg(test)]
mod tests;
