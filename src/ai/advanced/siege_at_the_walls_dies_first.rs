//! `siege-at-the-walls-dies-first`: a hostile siege gun in reach of our
//! walls, and a melee soldier beside a city of ours whose walls are down and
//! whose health is half gone, is the first target of every blow we price —
//! the battle planner's kill plan, the per-unit ladder and the city's own
//! strike.
//!
//! **Why (live Deity Online Kristina, 2026-11-10).** Five of sixteen Sweden
//! culture seats were eliminated, and nine of the sixteen lost Stockholm, the
//! capital, between turns 34 and 103. In seven of the fourteen cities lost
//! before turn 110 a Catapult, Trebuchet or Domrey did most of the damage to
//! the walls over the eight turns before the fall, and we let it stand:
//! - 173141Z: one Catapult two tiles from Stockholm fired four turns running
//!   (64, 79 and 20 damage; the walls fell, the city's own strike went with
//!   them) while nine of ours stood within three tiles. The archers shot
//!   Warriors, Slingers and a Skirmisher; nothing touched the Catapult, and a
//!   full-health Warrior walked into the city at 20 health on turn 54.
//! - 155215Z: three Catapults two tiles from Stockholm for nine turns
//!   (t56-t64), 43 blows on our cities in the game; a defending force of nine
//!   to eleven took finishing blows on a Swordsman at 6 health and a
//!   Spearman, and the city fell at 65.
//! - 175553Z: a Catapult at 48 health two tiles from Göteborg was left for
//!   two turns while the archers killed a Warrior and a Heavy Chariot; the
//!   city fell at 60.
//!
//! Across the five eliminated games, on 59 turns an enemy siege gun stood
//! within two tiles of a city of ours, and on 41 of them we did not hit one.
//! The kill plan prices a kill at `unit_value` and a wound at
//! `damage_value`, with a flat +65 / +18 for a siege unit: a full-health
//! Catapult takes three or four blows, so a plan that finishes two wounded
//! Warriors always outscores it. Nothing on that scale reads what the gun
//! will do to the walls next turn.
//!
//! **What the gene does.** A *wall threat* is a hostile land unit we are at
//! war with that is either a siege gun (`UnitSpec::siege`) within its
//! attack range plus [`WALL_GUN_MARGIN`] of one of our City Centers, or a
//! melee-capable soldier beside a City Center of ours whose walls are down
//! (none, or broken) and whose health is at or under [`OPEN_CITY_HP`]. Every
//! blow on a wall threat earns [`WALL_THREAT_KILL_PREMIUM`] when it kills and
//! [`WALL_THREAT_DAMAGE_PREMIUM`] a point of damage when it does not:
//! - the kill plan (`battle_planner.rs`) carries the premium on the target's
//!   kill value and wound value, and keeps wall threats first when it trims
//!   the targets to `MAX_TARGETS`;
//! - the per-unit ladder (`units.rs`) adds it to the blow's score, and a
//!   ranged hit or a kill on a wall threat ranks with the lethal kills;
//! - the city and Encampment strikes (`defensive_strike_value`) add it.
//!
//! Off, nothing changes: `wall_threat` is false and every premium is zero.
//! Every seat and lane reads it: the rule only ever names a unit standing at
//! our own walls.

use super::*;

/// A siege gun this many tiles beyond its own range of one of our City
/// Centers is one move from firing on the walls.
pub(super) const WALL_GUN_MARGIN: i32 = 1;
/// A City Center of ours with its walls down and its health at or under this
/// falls to the next melee blow that empties it (cities hold 200).
pub(super) const OPEN_CITY_HP: i32 = 100;
/// What a kill of a wall threat adds, on `tactical_attack_result_in`'s scale
/// (a full-health Warrior's kill is worth about 270, a Catapult's 350): two
/// to three turns of a gun's fire on the walls, at the scale a city's
/// capture is priced.
pub(super) const WALL_THREAT_KILL_PREMIUM: f64 = 400.0;
/// What each point of damage on a wall threat adds when it survives the blow.
pub(super) const WALL_THREAT_DAMAGE_PREMIUM: f64 = 4.0;

/// Whether hostile unit `enemy` threatens the walls of a city of `pid`: see
/// the module. Read regardless of the gene; the gene gates the premium.
pub(super) fn threatens_our_walls(g: &Game, pid: usize, enemy: u32) -> bool {
    let Some(unit) = g.units.get(&enemy) else {
        return false;
    };
    if unit.owner == pid || !g.is_at_war(pid, unit.owner) {
        return false;
    }
    let spec = &g.rules.units[unit.kind];
    if spec.class != "military" || matches!(spec.domain.as_deref(), Some("air" | "sea")) {
        return false;
    }
    let cities = g.player_city_ids(pid);
    if spec.siege {
        let reach = g.unit_attack_range(enemy).max(1) + WALL_GUN_MARGIN;
        return cities
            .iter()
            .any(|cid| g.wdist(g.cities[cid].pos, unit.pos) <= reach);
    }
    spec.is_melee_capable()
        && cities.iter().any(|cid| {
            let city = &g.cities[cid];
            g.wdist(city.pos, unit.pos) == 1 && city.wall_hp <= 0 && city.hp <= OPEN_CITY_HP
        })
}

impl AdvancedAi {
    /// Whether `enemy` is a wall threat this frame. Always false with the
    /// gene off.
    pub(super) fn wall_threat(&self, g: &Game, pid: usize, enemy: u32) -> bool {
        self.siege_at_the_walls_dies_first && threatens_our_walls(g, pid, enemy)
    }

    /// The premium a blow on `enemy` earns: the kill premium if it removes
    /// the unit, else the damage premium on the hit points it took. Zero
    /// when `enemy` is no wall threat, and always zero with the gene off.
    pub(super) fn wall_threat_premium(
        &self,
        g: &Game,
        pid: usize,
        enemy: u32,
        damage: f64,
        killed: bool,
    ) -> f64 {
        if !self.wall_threat(g, pid, enemy) {
            return 0.0;
        }
        if killed {
            WALL_THREAT_KILL_PREMIUM
        } else {
            WALL_THREAT_DAMAGE_PREMIUM * damage.max(0.0)
        }
    }

    /// The premium the per-unit ladder adds to `action`, measured on an
    /// exact clone: every wall threat on the struck tile, at the damage the
    /// blow deals it or the kill. Zero for any other blow and with the gene
    /// off (the board is not cloned then).
    pub(super) fn wall_threat_blow_premium(&self, g: &Game, pid: usize, action: &Action) -> f64 {
        if !self.siege_at_the_walls_dies_first {
            return 0.0;
        }
        let target = match action {
            Action::Attack { target, .. }
            | Action::Ranged { target, .. }
            | Action::CityStrike { target, .. }
            | Action::EncampmentStrike { target, .. }
            | Action::DistrictStrike { target, .. } => *target,
            _ => return 0.0,
        };
        let threats: Vec<(u32, i32)> = g
            .unit_ids_at(target)
            .iter()
            .filter(|uid| threatens_our_walls(g, pid, **uid))
            .map(|uid| (*uid, g.units[uid].hp))
            .collect();
        if threats.is_empty() {
            return 0.0;
        }
        let mut after = g.speculative_clone();
        if after.apply(pid, action).is_err() {
            return 0.0;
        }
        threats
            .into_iter()
            .map(|(uid, hp)| match after.units.get(&uid) {
                None => WALL_THREAT_KILL_PREMIUM,
                Some(left) => WALL_THREAT_DAMAGE_PREMIUM * f64::from((hp - left.hp).max(0)),
            })
            .sum()
    }
}

#[cfg(test)]
mod tests;
