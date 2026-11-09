//! `exalted-goddess-buys-the-coastal-guard` (opt-in): an Indonesian seat's
//! coastal city with a hostile soldier or ship within
//! [`COASTAL_GUARD_REACH`] tiles buys the strongest naval military unit its
//! menu sells for Faith, ranged first, before the turn's Gold defender
//! purchase. At most one ship per city every [`COASTAL_GUARD_COOLDOWN`]
//! standard turns, and never a ship [`COASTAL_GUARD_ERAS_BEHIND`] or more
//! eras behind our own technology.
//!
//! **Why (live Deity Online Indonesia, 2026-11-05).** Gitarja's Exalted
//! Goddess of the Three Worlds lets Indonesia buy naval units with Faith,
//! and the live host quotes them (Galley 60 Faith from about turn 39,
//! Quadrireme 120 from 56, Caravel 240, Privateer 280, Ironclad 380). With
//! no religion the seat banked 140-375 Faith by turn 60 and 600-950 by turn
//! 140 and made no Faith purchase in three games, while Gold bought land
//! defenders. 154638Z was eliminated at turn 47 and 175215Z at 172. A ship
//! bought with Faith leaves the Gold for the science buildings.
//!
//! Off, nothing runs.

use super::*;

/// How near a hostile soldier or ship must stand to the coastal city.
pub(super) const COASTAL_GUARD_REACH: i32 = 3;

/// Standard turns between two ships bought for one city.
pub(super) const COASTAL_GUARD_COOLDOWN: u32 = 4;

/// Eras behind the newest technology we know that bar a ship. Measured
/// against our own technology, not `Game::world_era`: the world era is a
/// clock here, one era about every 20 turns whatever anyone has researched
/// (2 by turn 51, 6 by turn 131), so it refused the only ship on offer when
/// an early war came. Live 224142Z: three coastal cities, 188 Faith at turn
/// 60, a Galley at 60 Faith and no Shipbuilding; all three fell by turn 66.
/// Three, not two, because a science beeline reaches one high-era technology
/// long before the rest of the tree.
pub(super) const COASTAL_GUARD_ERAS_BEHIND: usize = 3;

/// Faith kept back after the purchase.
pub(super) const COASTAL_GUARD_FAITH_RESERVE: f64 = 50.0;

impl AdvancedAi {
    /// See the module. True when a ship was bought. Exact no-op with the
    /// gene off or for any civilization but Indonesia.
    pub(super) fn exalted_goddess_coastal_guard(&mut self, g: &mut Game, pid: usize) -> bool {
        if !self.exalted_goddess_buys_the_coastal_guard || g.players[pid].civ != "Indonesia" {
            return false;
        }
        let turn = g.turn;
        let cooldown = g.standard_duration(COASTAL_GUARD_COOLDOWN);
        let visible = g.player_visibility(pid);
        let mut threatened: Vec<(i32, u32)> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                self.exalted_goddess_guard_bought
                    .get(cid)
                    .is_none_or(|bought| turn.saturating_sub(*bought) >= cooldown)
            })
            .filter_map(|cid| {
                let pos = g.cities[&cid].pos;
                let coastal = g.nbrs(pos).into_iter().any(|neighbor| {
                    g.map.get(neighbor).is_some_and(|tile| {
                        tile.terrain == crate::name!("coast")
                            || tile.terrain == crate::name!("lake")
                    })
                });
                if !coastal {
                    return None;
                }
                g.units
                    .values()
                    .filter(|unit| {
                        let spec = &g.rules.units[unit.kind];
                        unit.owner != pid
                            && (g.barb_pid == Some(unit.owner) || g.is_at_war(pid, unit.owner))
                            && spec.class == "military"
                            && !matches!(spec.domain.as_deref(), Some("air"))
                            && visible.contains(&unit.pos)
                    })
                    .map(|unit| g.wdist(unit.pos, pos))
                    .filter(|reach| *reach <= COASTAL_GUARD_REACH)
                    .min()
                    .map(|reach| (reach, cid))
            })
            .collect();
        threatened.sort_unstable();
        let budget = g.players[pid].faith - COASTAL_GUARD_FAITH_RESERVE;
        let tech_era = g.players[pid]
            .techs
            .iter()
            .filter_map(|tech| g.rules.techs.get(tech).map(|spec| spec.era))
            .max()
            .unwrap_or(0);
        for (reach, cid) in threatened {
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
                    if *buyer != cid || *formation != 0 || currency != "faith" {
                        return None;
                    }
                    let spec = g.rules.units.get(unit)?;
                    if spec.class != "military"
                        || !matches!(spec.domain.as_deref(), Some("sea"))
                        || g.unit_era(unit.as_str()) + COASTAL_GUARD_ERAS_BEHIND <= tech_era
                    {
                        return None;
                    }
                    let price = g.unit_purchase_cost(pid, cid, unit.as_str(), "faith")?;
                    if price > budget + 1e-9 {
                        return None;
                    }
                    let ranged = spec.ranged_attack_strength();
                    let power = spec.strength.max(ranged);
                    Some((ranged > 0.0, power, price, unit.to_string(), action.clone()))
                })
                // Ranged first, then the strongest, then the cheaper, then
                // the name, so the pick is deterministic.
                .max_by(|left, right| {
                    left.0
                        .cmp(&right.0)
                        .then_with(|| left.1.total_cmp(&right.1))
                        .then_with(|| right.2.total_cmp(&left.2))
                        .then_with(|| right.3.cmp(&left.3))
                });
            let Some((_, _, price, unit, action)) = candidate else {
                continue;
            };
            if g.apply(pid, &action).is_err() {
                continue;
            }
            self.exalted_goddess_guard_bought.insert(cid, turn);
            if self.journal().wants(crate::reasoning::Level::Decision) {
                think!(self.journal(), Military, Decision,
                       "{} buys a {} with Faith", g.cities[&cid].name, unit;
                       "exalted-goddess-buys-the-coastal-guard: a hostile unit {reach} tile(s) \
                        away; {price:.0} Faith of {:.0}", g.players[pid].faith + price);
            }
            return true;
        }
        false
    }
}

#[cfg(test)]
mod tests;
