//! `siege-at-the-walls-dies-first`: a hostile siege gun in reach of our
//! walls, and a melee soldier beside a city of ours whose walls are down and
//! whose health is half gone, is the first target of every blow we price —
//! the battle planner's kill plan, the per-unit ladder and the city's own
//! strike.
//!
//! **Why (live Deity Online Kristina, 2026-11-10).** Five of sixteen Sweden
//! culture seats were eliminated, and nine of the sixteen lost Stockholm, the
//! capital, between turns 34 and 103. In eight of the eighteen cities lost
//! at war before turn 130 a Catapult, Trebuchet or Domrey did most of the
//! damage to the walls over the eight turns before the fall, and we let it
//! stand:
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
//! Across the five eliminated games, on 77 turns an enemy siege gun stood
//! within two tiles of a city of ours, and on 49 of them we did not hit one.
//! The kill plan prices a kill at `unit_value` and a wound at
//! `damage_value`, with a flat +65 / +18 for a siege unit: a full-health
//! Catapult takes three or four blows, so finishing any wounded soldier
//! outscores a blow on it, and nothing on that scale reads what the gun will
//! do to the walls next turn.
//!
//! **What the gene does.** A *wall threat* is a hostile land unit we are at
//! war with that is either a siege gun (`UnitSpec::siege`) within its
//! attack range plus [`WALL_GUN_MARGIN`] of one of our City Centers, or a
//! melee-capable soldier beside a City Center of ours whose walls are down
//! (none, or broken) and whose health is at or under [`OPEN_CITY_HP`]. Every
//! blow on a wall threat earns [`WALL_THREAT_KILL_PREMIUM`] when it kills and
//! [`WALL_THREAT_DAMAGE_PREMIUM`] a point of damage when it does not (a
//! wounded gun is a gun the Deity AI walks back to heal):
//! - the kill plan (`battle_planner.rs`) carries the premium on the target's
//!   kill value and wound value, and keeps wall threats first when it trims
//!   the targets to `MAX_TARGETS`; the positions plan lays its shooter slots
//!   at range of the guns before the most finishable contacts, so a shooter
//!   that could not reach a gun this turn stands where it fires next turn;
//! - the per-unit ladder (`units.rs`) adds it to the blow's score, and a
//!   ranged hit or a kill on a wall threat ranks with the lethal kills;
//! - the city and Encampment strikes (`defensive_strike_value`) add it;
//! - and while any wall threat stands, outside a Conquest plan, the Objective
//!   Board raises no Siege row (`objective_board.rs`): the army that would
//!   have marched on the enemy's city stays to kill the gun. In 173141Z the
//!   board sent seven of our ten soldiers to stage on Jambi (t50-t62, "a land
//!   force of 7 will advance for Siege Jambi") while Stockholm, its walls
//!   down and 111 of 200 health gone, was defended by three.
//!
//! Off, nothing changes: `wall_threat` is false and every premium is zero.
//! Every seat and lane reads it: the rule only ever names a unit standing at
//! our own walls.
//!
//! **`city-strike-takes-the-gun`** (separate opt-in, same module). The
//! delegated city loop (`BasicAi::cities`, which every victory-planning seat
//! runs for its production) fires each walled City Center at the first tile
//! holding a hostile in `wdisk` order, before the command phase's
//! `advanced_city_strikes` prices anything, so the strike was spent before
//! the premium above could aim it. Over the live Kristina games of
//! 2026-11-10, a gun stood within two tiles of a walled city of ours and
//! the city struck something else, or nothing, on 19 of the 71 unhit
//! gun-turns of the five eliminated games and on 19 of the 26 unhit
//! gun-turns of the eight games played with `siege-at-the-walls-dies-first`
//! armed: the largest single cause once the shooters were aimed. With the
//! gene on, under victory planning, that pass and its Encampment pass
//! stand aside (`BasicAi::city_strikes_left_to_the_planner`, set each turn
//! in `take_turn_inner`), every strike is priced on an exact clone by
//! `defensive_strike_value`, and a gun at our walls carries the premium
//! there whether or not `siege-at-the-walls-dies-first` is armed. A seat
//! without victory planning, a minor and a barbarian keep the old pass:
//! their command phase never runs.
//!
//! **`every-shooter-at-the-walls`** (separate opt-in, same module). A gun
//! above was a siege-class unit only, so the Field Cannons that took Athens
//! in civvis-20261110T234227Z (t105-t117: 82% of the damage to the city in
//! its last eight turns, while the city struck Crossbowmen and Pike and
//! Shot 27 times) were never one: before t130 they were 59 of the 383 times
//! a hostile gun stood within two tiles of a city of ours. With this gene
//! the threat is read off the damage itself: every hostile land shooter
//! (ranged or bombard) within its range plus one of a City Center of ours
//! is priced at the walls-and-city damage its next blow would do there
//! (`Game::ranged_city_strengths` and the engine's centre roll, halved
//! against standing walls for a non-siege shooter, as `city_take_damage`
//! lands it), and counts as a wall threat at [`MIN_WALL_DAMAGE`] or more.
//! Every premium above scales with that damage over
//! [`REFERENCE_WALL_DAMAGE`] (a Catapult's blow on Ancient Walls), so the
//! shooter doing the most harm to the city goes first and a kill of it
//! still outranks any wound. A soldier escorting a civilian may also shoot
//! a wall threat from where it stands (`battle_planner.rs`); in the
//! eliminated games an escort stood in reach of an unhit gun on 17 turns.
//! Off, the old siege-and-taker rule reads exactly as before.

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
/// `every-shooter-at-the-walls`: the walls-and-city damage the premiums
/// above are priced at — a Catapult's blow on Ancient Walls at full health.
pub(super) const REFERENCE_WALL_DAMAGE: f64 = 40.0;
/// `every-shooter-at-the-walls`: a shooter whose next blow on our walls
/// would remove less than this is no wall threat.
pub(super) const MIN_WALL_DAMAGE: f64 = 10.0;
/// `every-shooter-at-the-walls`: the bounds of a threat's premium scale.
const SCALE_FLOOR: f64 = 0.25;
const SCALE_CEILING: f64 = 2.5;

/// `every-shooter-at-the-walls`: the walls-and-city hit points the next blow
/// of hostile land shooter `enemy` would remove from the City Center of `pid`
/// it hurts most among those within its range plus [`WALL_GUN_MARGIN`]: the
/// engine's centre roll on `Game::ranged_city_strengths`, at half against
/// standing walls for a shooter that is not siege (`city_take_damage`'s 50%).
/// Zero for anything else.
pub(super) fn shooter_wall_damage(g: &Game, pid: usize, enemy: u32) -> f64 {
    let Some(unit) = g.units.get(&enemy) else {
        return 0.0;
    };
    if unit.owner == pid || !g.is_at_war(pid, unit.owner) {
        return 0.0;
    }
    let spec = &g.rules.units[unit.kind];
    if spec.class != "military"
        || matches!(spec.domain.as_deref(), Some("air" | "sea"))
        || !spec.has_ranged_attack()
    {
        return 0.0;
    }
    let reach = g.unit_attack_range(enemy).max(1) + WALL_GUN_MARGIN;
    g.player_city_ids(pid)
        .into_iter()
        .filter(|cid| g.wdist(g.cities[cid].pos, unit.pos) <= reach)
        .filter_map(|cid| {
            let (attack, defense) = g.ranged_city_strengths(enemy, cid)?;
            let share = if g.cities[&cid].wall_hp > 0 && !spec.siege {
                0.5
            } else {
                1.0
            };
            Some(crate::game::expected_damage(attack, defense) * share)
        })
        .fold(0.0, f64::max)
}

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
    /// How hard `enemy` presses our walls, as the factor every premium is
    /// scaled by; zero when it is no wall threat. Without
    /// `every-shooter-at-the-walls`, one for a siege gun or a taker
    /// (`threatens_our_walls`); with it, a shooter's
    /// [`shooter_wall_damage`] over [`REFERENCE_WALL_DAMAGE`] (none under
    /// [`MIN_WALL_DAMAGE`]), and at least one for a taker.
    pub(super) fn wall_threat_scale(&self, g: &Game, pid: usize, enemy: u32) -> f64 {
        if !self.every_shooter_at_the_walls {
            return if threatens_our_walls(g, pid, enemy) {
                1.0
            } else {
                0.0
            };
        }
        let shot = shooter_wall_damage(g, pid, enemy);
        let scale = if shot >= MIN_WALL_DAMAGE {
            (shot / REFERENCE_WALL_DAMAGE).clamp(SCALE_FLOOR, SCALE_CEILING)
        } else {
            0.0
        };
        let taker = g.units.get(&enemy).is_some_and(|unit| {
            !g.rules.units[unit.kind].siege
                && !g.rules.units[unit.kind].has_ranged_attack()
                && threatens_our_walls(g, pid, enemy)
        });
        if taker {
            scale.max(1.0)
        } else {
            scale
        }
    }

    /// Whether `enemy` is a wall threat this frame. Always false with
    /// `siege-at-the-walls-dies-first` off.
    pub(super) fn wall_threat(&self, g: &Game, pid: usize, enemy: u32) -> bool {
        self.siege_at_the_walls_dies_first && self.wall_threat_scale(g, pid, enemy) > 0.0
    }

    /// The premium a city or Encampment strike on `enemy` earns: the kill
    /// premium if it removes the unit, else the damage premium on the hit
    /// points it took. Zero when `enemy` is no wall threat, and always zero
    /// with both `siege-at-the-walls-dies-first` and
    /// `city-strike-takes-the-gun` off.
    pub(super) fn wall_threat_premium(
        &self,
        g: &Game,
        pid: usize,
        enemy: u32,
        damage: f64,
        killed: bool,
    ) -> f64 {
        if !(self.siege_at_the_walls_dies_first || self.city_strike_takes_the_gun) {
            return 0.0;
        }
        let scale = self.wall_threat_scale(g, pid, enemy);
        if killed {
            WALL_THREAT_KILL_PREMIUM * scale
        } else {
            WALL_THREAT_DAMAGE_PREMIUM * scale * damage.max(0.0)
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
        let threats: Vec<(u32, i32, f64)> = g
            .unit_ids_at(target)
            .iter()
            .map(|uid| (*uid, g.units[uid].hp, self.wall_threat_scale(g, pid, *uid)))
            .filter(|(_, _, scale)| *scale > 0.0)
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
            .map(|(uid, hp, scale)| match after.units.get(&uid) {
                None => WALL_THREAT_KILL_PREMIUM * scale,
                Some(left) => WALL_THREAT_DAMAGE_PREMIUM * scale * f64::from((hp - left.hp).max(0)),
            })
            .sum()
    }
}

#[cfg(test)]
mod tests;
