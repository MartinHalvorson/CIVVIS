//! `threatened-city-buys-its-defender`: the plan's threatened city, with a
//! hostile land soldier of a player at war with us (or a barbarian) within
//! [`THREATENED_PURCHASE_REACH`] tiles, buys the strongest land defender its
//! purchase menu offers for Gold above the purchase upkeep floor. It buys
//! before the city takes damage, while the menu still sells one. At most one
//! purchase per city every [`THREATENED_PURCHASE_COOLDOWN`] standard turns,
//! as the turn's city-saving purchase.
//!
//! **Why (live Emperor, civvis-20261018T045526Z).** Cali was the plan's
//! threatened city from turn 35. Its menu sold a Warrior for 80 Gold and an
//! Archer for 120 at turns 38-40, while the treasury held 151-172 Gold and
//! the city built a nine-turn Archer. `emergency_city_defense_purchase` buys
//! only for a city already damaged (`besieged_city_item`), and
//! `imminent_major_war_defense_purchase` needs `siege-preempts-the-queue`,
//! which the live seat does not arm. By turn 41, when Cali was damaged, a
//! unit stood in it and the host's menu no longer sold a land soldier. Cali
//! fell at 43, and Bogotá, Guayaquil and Popayán followed by turn 108.
//! Across the 164 retained Emperor runs of 10-16/18, 37 lost a city by turn
//! 100.
//!
//! Off, nothing runs.

use super::*;

/// How near a hostile land soldier must stand to the threatened city.
pub(super) const THREATENED_PURCHASE_REACH: i32 = 3;

/// Standard turns between two defenders bought for one city.
pub(super) const THREATENED_PURCHASE_COOLDOWN: u32 = 4;

/// `threatened-city-buys-its-defender-2`: the least share of the strongest
/// land defender the city could train that a bought defender must reach.
/// Version 1 buys the strongest body the reserve reaches. Across 119 live
/// Emperor runs (2026-10-15..19) it bought 2.6 a game for 296 Gold: 121
/// Warriors, 67 Heavy Chariots, 54 Archers, 52 Horsemen and 5 Men-at-Arms,
/// 50 of the Warriors and 58 of the Heavy Chariots after turn 50 -- 20-28
/// strength bodies that cannot trade with a city's attackers and then sit in
/// the field army as the melee a siege cannot use. A 160-game live-shaped
/// screen read v1 at -17.3 pp (z -2.55). Short of the bar the Gold is kept
/// for the next purchase or the queue.
pub(super) const DEFENDER_ERA_SHARE: f64 = 0.8;

impl AdvancedAi {
    /// See the module. True when a defender was bought. Exact no-op with the
    /// gene off.
    pub(super) fn threatened_city_defender_purchase(
        &mut self,
        g: &mut Game,
        pid: usize,
        threatened_city: Option<u32>,
    ) -> bool {
        if !self.threatened_city_buys_its_defender && !self.threatened_city_buys_its_defender_2 {
            return false;
        }
        let Some(cid) = threatened_city else {
            return false;
        };
        let Some(city) = g.cities.get(&cid).filter(|city| city.owner == pid) else {
            return false;
        };
        let city_pos = city.pos;
        let city_name = city.name.clone();
        let turn = g.turn;
        if self
            .threatened_defender_bought
            .get(&cid)
            .is_some_and(|bought| {
                turn.saturating_sub(*bought) < g.standard_duration(THREATENED_PURCHASE_COOLDOWN)
            })
        {
            return false;
        }
        let visible = g.player_visibility(pid);
        let hostile_near = g.units.values().any(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner != pid
                && (g.barb_pid == Some(unit.owner) || g.is_at_war(pid, unit.owner))
                && spec.class == "military"
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && g.wdist(unit.pos, city_pos) <= THREATENED_PURCHASE_REACH
                && visible.contains(&unit.pos)
        });
        if !hostile_near {
            return false;
        }
        let budget = g.players[pid].gold - self.purchase_upkeep_floor(g, pid, 0.0);
        // `threatened-city-buys-its-defender-2`: a body of its era or none.
        let era_floor = if self.threatened_city_buys_its_defender_2 {
            DEFENDER_ERA_SHARE * Self::best_land_defender_power(g, pid, cid)
        } else {
            0.0
        };
        let candidate = g
            .legal_purchase_actions_for_city(pid, cid)
            .0
            .into_iter()
            .filter_map(|action| {
                let Action::Buy {
                    city: buyer,
                    unit,
                    formation,
                    currency,
                } = &action
                else {
                    return None;
                };
                if *buyer != cid || *formation != 0 || currency != "gold" {
                    return None;
                }
                let spec = g.rules.units.get(unit)?;
                if spec.class != "military"
                    || spec.siege
                    || spec.promotion_class == "recon"
                    || matches!(spec.domain.as_deref(), Some("sea" | "air"))
                {
                    return None;
                }
                let price = g.unit_purchase_cost(pid, cid, unit.as_str(), "gold")?;
                if price > budget + 1e-9 {
                    return None;
                }
                let power = spec.strength.max(spec.ranged_attack_strength());
                if power + 1e-9 < era_floor {
                    return None;
                }
                Some((power, price, unit.to_string(), action.clone()))
            })
            // The strongest; the cheaper on a tie; then the name, so the pick
            // is deterministic.
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.total_cmp(&left.1))
                    .then_with(|| right.2.cmp(&left.2))
            });
        let Some((_, price, unit, action)) = candidate else {
            return false;
        };
        let gold = g.players[pid].gold;
        if g.apply(pid, &action).is_err() {
            return false;
        }
        self.threatened_defender_bought.insert(cid, turn);
        think!(self.journal(), Military, Decision,
               "{city_name} buys a {} while the threat is still at its door", plain(&unit);
               "threatened-city-buys-its-defender: a hostile land soldier within {THREATENED_PURCHASE_REACH} tiles; {price:.0} of {gold:.0} Gold, before the city's menu stops selling one");
        true
    }
}

impl AdvancedAi {
    /// The strongest land defender `cid` could train now, by the same
    /// `max(strength, ranged attack)` the purchase ranks on, whatever the
    /// treasury holds. Zero when it can train none.
    fn best_land_defender_power(g: &Game, pid: usize, cid: u32) -> f64 {
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.class == "military"
                    && !spec.siege
                    && spec.promotion_class != "recon"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            .filter(|(unit, _)| {
                g.can_produce(
                    pid,
                    cid,
                    &Item::Unit {
                        unit: (*unit).clone(),
                    },
                )
            })
            .map(|(_, spec)| spec.strength.max(spec.ranged_attack_strength()))
            .fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests;
