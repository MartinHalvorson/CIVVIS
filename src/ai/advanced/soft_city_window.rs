//! `first-capital-strikes-the-open-window`, beyond the first capital: a
//! neighbour's unwalled city within reach of our soldiers opens the war.
//!
//! ## Why
//!
//! The first rival city the live Deity Domination seat took in over ninety
//! games of 2026-11-11 was not the product of any opening. Gran Colombia
//! `civvis-20261111T202441Z` (four players, Pangaea, Online) opened a
//! defensive faith war on Macedon at turn 72, at 238 power against 188, and
//! at turn 74 held Alexandria Troas: a city Macedon had founded beside our
//! border a few turns earlier, with no walls and our soldiers already
//! standing near it. Every road to war in the controller prices empires or
//! a named, staged objective. None of them reads a soft city that lies
//! within two turns of soldiers we already have.
//!
//! ## What the gene does
//!
//! At peace with every major, with no early conquest opening standing, no
//! home city threatened and outside a Recovery plan, the seat looks at each
//! city of each met major it may legally declare on and declares a surprise
//! war for the best one that passes all of these:
//!
//! - the city has no walls built;
//! - at least [`SOFT_CITY_BODIES`] of our land melee soldiers at
//!   [`SOFT_CITY_BODY_HEALTH`] or more stand within [`SOFT_CITY_REACH`]
//!   tiles of it (Scouts and civilian escorts do not count);
//! - the centre, read as it will fight (its strength as last seen, or the
//!   strongest of the owner's soldiers within two tiles plus the six a
//!   palace and a wall level add to a garrison, whichever is more), is at
//!   most `FIRST_CAPITAL_MELEE_MARGIN` above our best such soldier;
//! - those soldiers' mean blows over [`SOFT_CITY_STRIKE_TURNS`] turns exceed
//!   the city's health and its healing over those turns;
//! - the owner's visible army within `FIRST_CAPITAL_DEFENDER_REACH` tiles of
//!   the city does not outweigh them;
//! - the owner's public military power is at most
//!   `FIRST_CAPITAL_POWER_RATIO` times ours;
//! - the capture would hold (`campaign_city_requirement`).
//!
//! The war that follows is the shipped one: the campaign names the open
//! city, the board writes its Siege row and the peace desk closes it.
//! Nothing here moves a unit.
//!
//! Inert with the flag off, without `first-capital-before-the-walls` and
//! `early-conquest-opening`, and on any lane but Domination.

use super::first_capital_strikes_the_open_window::{
    FIRST_CAPITAL_CITY_HEAL, FIRST_CAPITAL_DEFENDER_REACH, FIRST_CAPITAL_MELEE_MARGIN,
    FIRST_CAPITAL_POWER_RATIO,
};
use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::Game;
use crate::think;

/// How far from the city our soldiers may stand and count: two turns' walk
/// for a Warrior.
pub(crate) const SOFT_CITY_REACH: i32 = 4;

/// The melee soldiers that must stand that close.
pub(crate) const SOFT_CITY_BODIES: usize = 3;

/// The health a soldier needs to count.
pub(crate) const SOFT_CITY_BODY_HEALTH: i32 = 50;

/// The turns of blows the soldiers in reach may need to take the city, as
/// for the first capital (`FIRST_CAPITAL_STRIKE_TURNS`). Three Warriors deal
/// about 357 in three turns to a centre of 13 and 270 to one of 20, against
/// 260 of health and healing; a centre of 23 asks for a fourth.
pub(crate) const SOFT_CITY_STRIKE_TURNS: u32 = 3;

/// What a garrisoned soldier's strength gains in a capital behind one wall
/// level (`game/city.rs`: +3 each); charged to the strongest defender near
/// an ungarrisoned city, because it walks in when the war opens.
pub(crate) const SOFT_CITY_GARRISON_BONUS: f64 = 6.0;

/// A soft city in reach and what the soldiers near it make of it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SoftCity {
    /// The owner.
    pub(crate) rival: usize,
    /// The city.
    pub(crate) city: u32,
    /// Our melee soldiers within reach.
    pub(crate) bodies: usize,
    /// Their mean blows over the strike turns.
    pub(crate) blows: f64,
    /// The city's health and healing over those turns.
    pub(crate) needed: f64,
    /// The centre as it will fight.
    pub(crate) defence: f64,
}

impl AdvancedAi {
    /// The best soft city in reach, or `None`. See the module text for the
    /// conditions. `None` with the gene off.
    pub(crate) fn soft_city_in_reach(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<SoftCity> {
        if !self.first_capital_strikes_active()
            || self.conquest_opening.is_some()
            || plan.threatened_city.is_some()
            || plan.strategy == GrandStrategy::Recovery
            || !self.one_war_enemies(g, pid).is_empty()
        {
            return None;
        }
        let guards = self.all_reserved_civilian_guards();
        let soldiers: Vec<&crate::game::Unit> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| !guards.contains(uid))
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| {
                unit.hp >= SOFT_CITY_BODY_HEALTH
                    && !g.is_embarked(unit)
                    && Self::conquest_melee_body(&g.rules.units[unit.kind])
            })
            .collect();
        if soldiers.len() < SOFT_CITY_BODIES {
            return None;
        }
        let ours = g.military_power(pid);
        let visible = self.battlefront_visibility(g, pid);
        let turns = f64::from(SOFT_CITY_STRIKE_TURNS);
        let mut best: Option<SoftCity> = None;
        for rival in g.players.iter().filter(|rival| {
            rival.id != pid && rival.alive && !rival.is_minor && !rival.is_barbarian
        }) {
            if !g.has_met(pid, rival.id)
                || g.is_at_war(pid, rival.id)
                || !self.campaign_target_legal(g, pid, rival.id)
                || g.military_power(rival.id) > FIRST_CAPITAL_POWER_RATIO * ours
            {
                continue;
            }
            let Some(appraisal) = self.appraise_neighbour(g, pid, rival.id) else {
                continue;
            };
            for cid in g.player_city_ids(rival.id) {
                let Some(city) = g.cities.get(&cid) else {
                    continue;
                };
                if g.city_max_wall_hp(city) > 0 || city.wall_hp > 0 {
                    continue;
                }
                let near: Vec<&crate::game::Unit> = soldiers
                    .iter()
                    .copied()
                    .filter(|unit| g.wdist(unit.pos, city.pos) <= SOFT_CITY_REACH)
                    .collect();
                if near.len() < SOFT_CITY_BODIES {
                    continue;
                }
                // The owner's soldiers we can see beside the city: the
                // strongest one garrisons it, and all of them answer.
                let defenders: Vec<&crate::game::Unit> = g
                    .units
                    .values()
                    .filter(|unit| unit.owner == rival.id)
                    .filter(|unit| g.rules.units[unit.kind].class == "military")
                    .filter(|unit| g.wdist(unit.pos, city.pos) <= FIRST_CAPITAL_DEFENDER_REACH)
                    .filter(|unit| g.sees(&visible, unit.pos) && g.unit_visible_to(unit.id, pid))
                    .collect();
                let garrison = defenders
                    .iter()
                    .filter(|unit| g.wdist(unit.pos, city.pos) <= 2)
                    .map(|unit| g.unit_strength(unit, true) + SOFT_CITY_GARRISON_BONUS)
                    .fold(0.0, f64::max);
                let defence = g.city_strength(cid).max(garrison);
                let best_melee = near
                    .iter()
                    .map(|unit| g.unit_strength(unit, false))
                    .fold(f64::MIN, f64::max);
                if defence > best_melee + FIRST_CAPITAL_MELEE_MARGIN {
                    continue;
                }
                let theirs: f64 = defenders
                    .iter()
                    .map(|unit| {
                        crate::game::effective_strength(g.unit_strength(unit, true), unit.hp)
                    })
                    .sum();
                let near_ids: Vec<u32> = near.iter().map(|unit| unit.id).collect();
                if theirs > Self::campaign_strength_of(g, &near_ids) {
                    continue;
                }
                let blows: f64 = near
                    .iter()
                    .map(|unit| {
                        let attack =
                            crate::game::effective_strength(g.unit_strength(unit, false), unit.hp);
                        crate::game::expected_damage(attack, defence)
                    })
                    .sum::<f64>()
                    * turns;
                let needed = f64::from(city.hp.max(0)) + f64::from(FIRST_CAPITAL_CITY_HEAL) * turns;
                if blows <= needed {
                    continue;
                }
                let average = Self::campaign_strength_of(g, &near_ids) / near.len() as f64;
                if !self
                    .campaign_city_requirement(g, pid, cid, &appraisal, average)
                    .holdable
                {
                    continue;
                }
                let found = SoftCity {
                    rival: rival.id,
                    city: cid,
                    bodies: near.len(),
                    blows,
                    needed,
                    defence,
                };
                let better = best.as_ref().is_none_or(|old| {
                    found.blows - found.needed > old.blows - old.needed + 1e-9
                        || ((found.blows - found.needed - (old.blows - old.needed)).abs() <= 1e-9
                            && found.city < old.city)
                });
                if better {
                    best = Some(found);
                }
            }
        }
        best
    }

    /// The turn's one declaration: a surprise war for the soft city in
    /// reach. True when the war opened. `false` with the gene off.
    pub(super) fn soft_city_window_opening(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some(soft) = self.soft_city_in_reach(g, pid, plan) else {
            return false;
        };
        let Some(opening) = self.faith_war_opening(g, pid, soft.rival) else {
            return false;
        };
        let (name, pos) = match g.cities.get(&soft.city) {
            Some(city) => (city.name.clone(), city.pos),
            None => return false,
        };
        let civ = g.players[soft.rival].civ.clone();
        let (ours, theirs) = (g.military_power(pid), g.military_power(soft.rival));
        if g.apply(pid, &opening).is_err() {
            return false;
        }
        think!(self.journal(), Military, Strategy,
               "Declaring war on {} for {}", civ, name;
               "first-capital-strikes-the-open-window: an unwalled city within {} tiles of {} of \
                our melee soldiers; centre {:.0}, about {:.0} in blows over {} turns against \
                {:.0} of health and healing; their power {:.0} against our {:.0}",
               SOFT_CITY_REACH, soft.bodies, soft.defence, soft.blows, SOFT_CITY_STRIKE_TURNS,
               soft.needed, theirs, ours;
               pos);
        true
    }
}

#[cfg(test)]
mod tests;
