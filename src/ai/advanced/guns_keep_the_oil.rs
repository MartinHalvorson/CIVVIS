//! `guns-keep-the-oil`: a Domination seat's Infantry, Tanks and oil-burning
//! ships spend only the Oil its siege guns do not need.
//!
//! **The finding (live Emperor Gilgamesh civvis-20261116T021437Z).**
//! - The war took six cities from turn 153 to 183, Persia's capital among
//!   them, and lost to a rival Science win at 197. "The artillery waits on
//!   oil" ran on 39 turns from 155 to 196.
//! - The empire's only Oil was Hattusa's suzerain bonus: +2 a turn on turns
//!   136-141 and 146, while we held Hattusa. 14 stood in the stock at 146.
//! - Sippar's Line Infantry turned into an Infantry when Replaceable Parts
//!   landed at 149 (1 Oil to train, 1 a turn). Two Infantry were in the field
//!   by 152, and Isin and Ur queued more at 153-154. The stock was 0 by 156.
//! - The first Artillery (Steel at 145) came out of Sippar at 158 into that
//!   empty stock. The empire never fielded more than two, and neither fired a
//!   shot. Thirteen asks to Rome for Oil (49-59 on its trade screen) closed
//!   once, 10 Oil at 179.
//! - A unit whose fuel goes unpaid does not heal on the host. The first
//!   Artillery sat at 36 hp from turn 164 to 179 and healed 20 a turn from
//!   180, when the 179 barter landed. Across the 49 live games of 11-12 to
//!   11-16 that reached turn 140, a damaged Oil unit that stood still healed
//!   on 11 of 36 turns with its fuel unpaid (4 games). Fed, it healed on 44
//!   of 51, and units that burn nothing healed on 3,773 of 5,193. The engine models only the -1 Strength per
//!   unpaid unit (`Game::process_strategic_resources`).
//! - Immortal civvis-20261115T230117Z had 2 Artillery from turns 128-130, +3
//!   Oil a turn and 12 in stock. Three Line Infantry upgrades and one
//!   Infantry at 135-138 spent the stock. From 138 to 153 six Oil units drew
//!   on +0-3 a turn with none in stock, and the guns fired 4 shots all game.
//! - Neither game burned Oil for power. Their plants burned Coal, which stood
//!   at its cap, and the one reactor conversion went to Uranium.
//!
//! **What the gene does (Domination seat, on).** Suppose the strongest land
//! siege gun our technology unlocks burns a fuel, as the Artillery burns Oil.
//! A unit that burns the same fuel and is not a land siege gun (Infantry,
//! Tank, Biplane, Destroyer) is then trained, upgraded into, or kept at the
//! head of a queue under half paid only if it leaves the guns enough. The
//! stock, less its cost, plus [`GUN_RESOURCE_FUEL_TURNS`] of income, less the
//! upkeep of every other fielded non-gun unit and its own, must still carry
//! the guns fielded and queued, and one more gun, for as many turns. The
//! surplus sale never offers that fuel either (`siege_gun_resources_held`).
//! Capture bodies that burn nothing stay open: AT Crews took three of the six
//! cities in 021437Z.
//!
//! Off, or for any other victory target, nothing changes.

use super::gun_resource::{land_gun, GUN_RESOURCE_FUEL_TURNS};
use super::*;

impl AdvancedAi {
    /// `guns-keep-the-oil`: the fuel of the siege train's next gun, with that
    /// gun's training cost and upkeep. The gun is the strongest land siege
    /// gun our technology unlocks. `None` with the gene off, off a Domination
    /// seat, or when that gun burns nothing (a Bombard seat before Steel).
    pub(super) fn siege_gun_fuel(&self, g: &Game, pid: usize) -> Option<(Name, f64, f64)> {
        if !self.guns_keep_the_oil
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let player = &g.players[pid];
        let gun = g
            .rules
            .units
            .iter()
            .filter(|&(kind, spec)| {
                land_gun(spec)
                    && spec.buildable
                    && spec.tech.is_none_or(|tech| player.techs.contains(&tech))
                    && spec
                        .civic
                        .is_none_or(|civic| player.civics.contains(&civic))
                    && spec
                        .unique_to
                        .as_deref()
                        .is_none_or(|civ| g.owns_civ_unique(pid, civ))
                    && g.player_unit_replacement(pid, *kind) == *kind
            })
            .map(|(_, spec)| spec)
            .max_by(|left, right| {
                left.ranged_attack_strength()
                    .total_cmp(&right.ranged_attack_strength())
            })?;
        let fuel = gun.requires_resource?;
        (gun.resource_maintenance > 0.0).then_some((
            fuel,
            gun.resource_cost.max(0.0),
            gun.resource_maintenance,
        ))
    }

    /// The `fuel` a turn our land siege guns burn, fielded and queued, and
    /// the `fuel` a turn every other unit we field burns, as the host charges
    /// it (a unit on free upkeep burns none).
    fn fuel_upkeep_split(g: &Game, pid: usize, fuel: Name) -> (f64, f64) {
        let mut guns = 0.0;
        let mut others = 0.0;
        for unit in g
            .units
            .values()
            .filter(|unit| unit.owner == pid && !unit.free_upkeep)
        {
            let spec = &g.rules.units[unit.kind];
            if spec.requires_resource == Some(fuel) {
                if land_gun(spec) {
                    guns += spec.resource_maintenance.max(0.0);
                } else {
                    others += spec.resource_maintenance.max(0.0);
                }
            }
        }
        for item in g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .flat_map(|city| city.queue.iter())
        {
            if let Item::Unit { unit } | Item::Formation { unit, .. } = item {
                if let Some(spec) = g.rules.units.get_interned(*unit) {
                    if land_gun(spec) && spec.requires_resource == Some(fuel) {
                        guns += spec.resource_maintenance.max(0.0);
                    }
                }
            }
        }
        (guns, others)
    }

    /// The fuel left over once a consumer costing `cost` now and `upkeep` a
    /// turn has drawn on it, after the guns' [`GUN_RESOURCE_FUEL_TURNS`]
    /// turns. Negative when the guns would go short. With it, the numbers the
    /// journal prints: stock, income, the others' upkeep, the guns' need.
    fn guns_fuel_margin(
        g: &Game,
        pid: usize,
        (fuel, gun_cost, gun_upkeep): (Name, f64, f64),
        cost: f64,
        upkeep: f64,
    ) -> (f64, [f64; 4]) {
        let (guns, others) = Self::fuel_upkeep_split(g, pid, fuel);
        let stock = g.strategic_stockpile(pid, fuel);
        let income = g.strategic_resource_rate(pid, fuel.as_str());
        let turns = GUN_RESOURCE_FUEL_TURNS;
        let carried = stock - cost + turns * (income - others - upkeep);
        let needed = gun_cost + turns * (guns + gun_upkeep);
        (carried - needed, [stock, income, others, needed])
    }

    /// `guns-keep-the-oil`: whether a unit of `unit`, costing `cost` of its
    /// resource now and `upkeep` a turn, may draw on the siege guns' fuel.
    /// Always true with the gene off, for a land siege gun, and for a unit
    /// that burns something else.
    pub(super) fn guns_keep_the_oil_allows(
        &self,
        g: &Game,
        pid: usize,
        unit: Name,
        cost: f64,
        upkeep: f64,
    ) -> bool {
        if !self.guns_keep_the_oil {
            return true;
        }
        let Some(spec) = g.rules.units.get_interned(unit) else {
            return true;
        };
        if land_gun(spec) || (cost <= 0.0 && upkeep <= 0.0) {
            return true;
        }
        let Some(gun) = self.siege_gun_fuel(g, pid) else {
            return true;
        };
        spec.requires_resource != Some(gun.0)
            || Self::guns_fuel_margin(g, pid, gun, cost, upkeep).0 >= -1e-9
    }

    /// `guns-keep-the-oil`: [`Self::guns_keep_the_oil_allows`] for a
    /// production item in `cid`. The host takes a unit's resource when its
    /// production starts, so the head of the city's queue has paid it, and
    /// one paid at least half its hammers finishes
    /// (`upkeep_item_mostly_paid`).
    pub(super) fn guns_keep_the_oil_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> bool {
        if !self.guns_keep_the_oil {
            return true;
        }
        let (unit, units) = match item {
            Item::Unit { unit } => (*unit, 1.0),
            Item::Formation { unit, formation } => (*unit, if *formation >= 2 { 3.0 } else { 2.0 }),
            _ => return true,
        };
        let Some(spec) = g.rules.units.get_interned(unit) else {
            return true;
        };
        let queued = g
            .cities
            .get(&cid)
            .is_some_and(|city| city.queue.first() == Some(item));
        if queued && Self::upkeep_item_mostly_paid(g, pid, cid, item) {
            return true;
        }
        let cost = if queued {
            0.0
        } else {
            spec.resource_cost.max(0.0) * units
        };
        self.guns_keep_the_oil_allows(g, pid, unit, cost, spec.resource_maintenance.max(0.0))
    }

    /// Journal, once a governor pass, that the guns' fuel is held: a unit
    /// costing 1 and burning 1 a turn would already wait. `production_value`
    /// prices items out silently, so this line shows the gene acting.
    pub(super) fn note_guns_keep_the_oil(&self, g: &Game, pid: usize) {
        if !self.guns_keep_the_oil || !self.journal().wants(crate::reasoning::Level::Decision) {
            return;
        }
        let Some(gun) = self.siege_gun_fuel(g, pid) else {
            return;
        };
        let (margin, [stock, income, others, needed]) =
            Self::guns_fuel_margin(g, pid, gun, 1.0, 1.0);
        if margin >= -1e-9 {
            return;
        }
        let fuel = gun.0.as_str();
        think!(self.journal(), Economy, Decision,
            "The siege guns keep the {}", fuel;
            "guns-keep-the-oil: {stock:.0} {fuel} and {income:+.0} a turn, less {others:.0} a turn \
             for the units already burning it, fall short of the {needed:.0} the guns fielded and \
             queued and one more need for {:.0} turns; a unit that burns {fuel} and is no gun \
             waits", GUN_RESOURCE_FUEL_TURNS);
    }
}

#[cfg(test)]
mod tests;
