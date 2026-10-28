//! `prey-is-declared-on`: a Domination seat whose military power is at
//! least 1 / [`PREY_DECLARE_RATIO`] times a major rival's declares on it
//! without waiting on a short home Defend row (`war_policy`) or a staged
//! ring (`opens` in the declaration step). The war writes the Siege row that
//! brings the army to the ring; the declaration range, another major war,
//! the edge, the treasury and the production-parity checks still apply.
//!
//! **Why (live Emperor, 2026-10-19).** The one domination win of the loop
//! (civvis-20261018T090059Z) took America, at under a third of our power, from
//! turn 139. In the games since, a weak neighbour at war's reach was held for
//! 15-30 turns: Korea at 65 against our 500-650 from turn 88 to 117
//! (civvis-20261019T054930Z), America at 19-95 against 274-523 from 89 past
//! 120 (023252Z), Poland at 74-117 against 220-373 from 95 to 110 (002331Z),
//! alternating "the Defend row at … is short N unit(s)" with "the Siege row
//! for … asks N strength and M is staged on its ring" — a ring inside a
//! border our army may not cross at peace. `overwhelming-power-declares`
//! waives the ring only at four times the rival's steady power with the army's
//! median within ten tiles. On the siege replay harness (five such boards x 4
//! seeds x 40 turns) the waiver at 0.4 took 52 cities against 46, and left the
//! twelve stuck-siege boards unchanged (126 against 126).
//!
//! Off, nothing changes.
//!
//! ## `prey-waits-for-the-march`
//!
//! Under the second gene the prey waiver opens a war only once the field
//! army's median (`field_army_median_distance`) stands within
//! [`one_war::OVERWHELMING_MARCH_TILES`] of the objective, the bar the
//! overwhelming waiver already keeps. While it is further out, the
//! declaration holds and the pre-war staging step (`campaign_staging_step`)
//! marches the army to the border. A denouncement is not held: its five
//! turns run while the army walks. Two releases keep a march that cannot
//! close from holding the war forever, for instance behind a closed border
//! deeper than ten tiles:
//!
//! - **Patience:** after [`PREY_MARCH_PATIENCE`] turns of holding on the
//!   same rival, the war is declared anyway.
//! - **Stall:** after [`PREY_MARCH_STALL`] turns in which the median came no
//!   closer, the war is declared anyway.
//!
//! Both clocks count plain turns: a march is tiles a turn, which game speed
//! does not scale.
//!
//! A war the waiver alone does not open (staged, overwhelming, counters,
//! launcher, road opening, moving on) is never held here.
//!
//! **Why (live Immortal, 2026-10-27/28).** 30 conquest declarations across
//! 147 games took 4 cities, and none of the 4 came through the prey waiver,
//! whose wars ran at a true military ratio of 3-17 times. At 20 of the 30
//! declarations the field army's median stood 12-25 tiles from the
//! objective, and those 20 took no city in wars of 4-106 turns. The target
//! raised walls before the train arrived, and the siege then held for a
//! breaker: civvis-20261028T041823Z denounced Sumeria at turn 54, marched
//! from 18-21 tiles while the denouncement ran, declared at 59 at a median
//! 16, and Lagash, unwalled at 59, raised walls at 68 and was never taken
//! in 77 turns of war. 26 of the 30 declarations came with no other war
//! running, so the pre-war march was free to run.
//!
//! ## `prey-waits-for-the-march-2`
//!
//! Version two keeps the whole of version one (it reads `v1 || v2`) and
//! tightens what passes:
//!
//! - **The march bar** is [`PREY_MARCH_TILES_2`] = 6 tiles of median, not 10.
//! - **A siege gun** of ours (a land unit with a ranged attack and the
//!   siege flag) must stand within [`PREY_MARCH_GUN_TILES`] of the
//!   objective, while any city of ours can build one. Before a gun is
//!   buildable (an Ancient-era war, before Engineering), the gun leg is not
//!   asked.
//! - **The stall release** counts a turn as progress when either the median
//!   or the nearest gun came closer.
//!
//! The hold names which leg holds it ("for the march" or "for a siege gun").
//!
//! **Why (live Immortal, 2026-10-27/28).** Our `war` orders on majors (175
//! turn and target pairs, census by -fc) captured at least one city of the
//! target within 20 turns 6 times:
//!
//! - **By field-army median to the target's nearest city:** 4/43 at 6 tiles
//!   or less, 2/46 at 7-10, 0/86 past 10.
//! - **By our nearest siege gun:** 3/32 within 4 tiles, 2/25 at 5-8, 0/52
//!   past 8, and 1/66 with no gun at all.
//! - **Joint:** army ≤10 with a gun ≤6, 4/42; army ≤10 without one, 2/47.
//!
//! The window is short: across 34 conquest declarations the target's
//! military grew 1.29 times in 10 turns and 1.76 times in 20, against our
//! 1.07 in 10, and our ratio over it fell from a median 2.47 to 1.53 in 20.
//! On the first live hold of version one (civvis-20261028T120835Z, Georgia)
//! the war opened at a median of 10. Tskhumi, unwalled, went 200 to 126, and
//! raised walls six turns in, while the bombards still stood 3-4 tiles out
//! and a trebuchet 19.

use super::*;

/// `prey-waits-for-the-march`: turns the prey declaration may hold for the
/// march before it goes ahead.
pub(super) const PREY_MARCH_PATIENCE: u32 = 8;

/// `prey-waits-for-the-march`: turns in which the field army's median came no
/// closer to the objective after which the hold is released.
pub(super) const PREY_MARCH_STALL: u32 = 3;

/// `prey-waits-for-the-march-2`: the field army's median the prey waiver asks
/// under version two.
pub(super) const PREY_MARCH_TILES_2: i32 = 6;

/// `prey-waits-for-the-march-2`: the distance from the objective within which
/// a siege gun of ours must stand under version two.
pub(super) const PREY_MARCH_GUN_TILES: i32 = 6;

/// `prey-waits-for-the-march`: the hold on one rival's declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PreyMarchHold {
    /// The rival the declaration waits on.
    pub(super) rival: usize,
    /// The first turn of the hold.
    pub(super) since: u32,
    /// The closest median the hold has seen, and the turn the march last
    /// gained ground (the median, or under version two the nearest gun).
    pub(super) best: (i32, u32),
    /// `prey-waits-for-the-march-2`: the closest our nearest siege gun has
    /// stood; `i32::MAX` with none read.
    pub(super) best_gun: i32,
    /// The last turn the hold was read.
    pub(super) seen: u32,
}

/// `prey-waits-for-the-march`: this turn's reading of a prey declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PreyMarch {
    /// The field army's median stands within the march bar.
    Within(i32),
    /// The declaration holds for the march: the median, and the turns held
    /// so far.
    Hold(i32, u32),
    /// `prey-waits-for-the-march-2`: the median is within the bar but no
    /// siege gun stands within [`PREY_MARCH_GUN_TILES`]: the nearest gun's
    /// distance (`i32::MAX` with none fielded), and the turns held so far.
    HoldForGun(i32, u32),
    /// The hold ran its [`PREY_MARCH_PATIENCE`]: the median.
    Patience(i32),
    /// The median came no closer for [`PREY_MARCH_STALL`]: the median.
    Stalled(i32),
}

/// A major rival at or under this share of our military power is prey.
pub(super) const PREY_DECLARE_RATIO: f64 = 0.4;

impl AdvancedAi {
    /// Whether `target` is prey this seat declares on now: see the module.
    /// Always false with the gene off.
    pub(super) fn prey_is_declared_on_now(&self, g: &Game, pid: usize, target: usize) -> bool {
        self.prey_is_declared_on
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.players
                .get(target)
                .is_some_and(|rival| rival.alive && !rival.is_minor && !rival.is_barbarian)
            && g.military_power(target) <= PREY_DECLARE_RATIO * g.military_power(pid)
    }

    /// `prey-waits-for-the-march`: whether the prey declaration on `rival`
    /// toward `objective` goes ahead this turn, and why; see the module.
    /// `None` with the gene off. Reading it advances the hold, so it is read
    /// once a turn, on the turn the waiver alone would open the war.
    pub(super) fn prey_march_reading(
        &mut self,
        g: &Game,
        pid: usize,
        rival: usize,
        objective: Pos,
    ) -> Option<PreyMarch> {
        if !self.prey_waits_for_the_march && !self.prey_waits_for_the_march_2 {
            return None;
        }
        let median = self
            .field_army_median_distance(g, pid, objective)
            .unwrap_or(i32::MAX);
        // `prey-waits-for-the-march-2`: the gun leg, while a gun is buildable.
        let gun = (self.prey_waits_for_the_march_2 && Self::prey_gun_buildable(g, pid))
            .then(|| Self::nearest_land_gun(g, pid, objective));
        let gun_short = gun.is_some_and(|distance| distance > PREY_MARCH_GUN_TILES);
        if median <= self.prey_march_bar() && !gun_short {
            self.prey_march_hold = None;
            return Some(PreyMarch::Within(median));
        }
        let turn = g.turn;
        let stall = PREY_MARCH_STALL;
        let mut hold = self
            .prey_march_hold
            .filter(|hold| hold.rival == rival && turn.saturating_sub(hold.seen) <= stall)
            .unwrap_or(PreyMarchHold {
                rival,
                since: turn,
                best: (median, turn),
                best_gun: gun.unwrap_or(i32::MAX),
                seen: turn,
            });
        if median < hold.best.0 {
            hold.best = (median, turn);
        }
        if let Some(distance) = gun.filter(|distance| *distance < hold.best_gun) {
            hold.best_gun = distance;
            hold.best.1 = turn;
        }
        hold.seen = turn;
        self.prey_march_hold = Some(hold);
        let held = turn.saturating_sub(hold.since);
        Some(if held >= PREY_MARCH_PATIENCE {
            PreyMarch::Patience(median)
        } else if turn.saturating_sub(hold.best.1) >= stall {
            PreyMarch::Stalled(median)
        } else if median <= self.prey_march_bar() {
            PreyMarch::HoldForGun(gun.unwrap_or(i32::MAX), held)
        } else {
            PreyMarch::Hold(median, held)
        })
    }

    /// The field army's median the prey waiver asks: version two's
    /// [`PREY_MARCH_TILES_2`], else the overwhelming waiver's bar.
    pub(super) fn prey_march_bar(&self) -> i32 {
        if self.prey_waits_for_the_march_2 {
            PREY_MARCH_TILES_2
        } else {
            one_war::OVERWHELMING_MARCH_TILES
        }
    }

    /// The tag the hold's journal lines name.
    pub(super) fn prey_march_tag(&self) -> &'static str {
        if self.prey_waits_for_the_march_2 {
            "prey-waits-for-the-march-2"
        } else {
            "prey-waits-for-the-march"
        }
    }

    /// `prey-waits-for-the-march-2`: whether any city of ours can build a
    /// land siege gun now.
    fn prey_gun_buildable(g: &Game, pid: usize) -> bool {
        g.player_city_ids(pid).into_iter().any(|cid| {
            g.rules.units.iter().any(|(name, spec)| {
                prey_land_gun(spec) && g.can_produce(pid, cid, &Item::Unit { unit: *name })
            })
        })
    }

    /// `prey-waits-for-the-march-2`: the distance from `objective` of our
    /// nearest land siege gun not at sea; `i32::MAX` with none.
    fn nearest_land_gun(g: &Game, pid: usize, objective: Pos) -> i32 {
        g.player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| prey_land_gun(&g.rules.units[unit.kind]) && !g.is_embarked(unit))
            .map(|unit| g.wdist(unit.pos, objective))
            .min()
            .unwrap_or(i32::MAX)
    }
}

/// A land siege gun, as `siege_train`'s and `gun_resource`'s readings count
/// one.
fn prey_land_gun(spec: &crate::rules::UnitSpec) -> bool {
    spec.class == "military"
        && spec.siege
        && spec.has_ranged_attack()
        && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
}

#[cfg(test)]
mod tests;
