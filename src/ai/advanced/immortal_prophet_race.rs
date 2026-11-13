//! `immortal-skips-the-prophet-race`: at Immortal and Deity, a seat not aiming
//! at a Religious victory stays out of the Great Prophet race — no Astrology
//! beeline, no race Holy Site or Shrine, no Prophet patronage — unless a
//! Prophet is already pending.
//!
//! **Why (live Immortal Pericles, 2026-10-30).** The race is lost before our
//! points arrive, and the capital pays for it in the turns that decide the
//! production share the ladder retires on:
//! - civvis-20261030T134907Z: Holy Site at t26, the state's
//!   `great_person_exhausted` listing the Prophet by t58 with our points at 93
//!   (3.4 a turn). 125645Z and 131228Z: Holy Sites at t32-38, points 56-68 at
//!   t60.
//! - civvis-20261030T220042Z: the capital, at 6-9 Production, spent t34-t55
//!   on Walls, the race's Holy Site and its Shrine; the Prophet was exhausted
//!   at t60 with our points at 36, and the capital's Monument claim churned
//!   53 times against the race's site claim without finishing. The empire
//!   stood at four cities and 17 Culture against 116 at t80.
//! - Of 27 Immortal Culture seats, 16 retired under the production rule
//!   (under a quarter of the leader's Production by t80-91).
//!
//! **What the gene does.** `prophet_race_open_for` reads closed at Immortal
//! and Deity for a seat whose victory target is neither Religion nor, at
//! Immortal, Diplomacy (whose Mahabodhi points and founder defence need a
//! faith; at Deity the Diplomatic seat is closed out too, see below), before
//! any other test, unless a Prophet is pending (a Prophet earned or bought still
//! founds). Every part of the race package admits through that gate or
//! `prophet_race_enterable_for`, which calls it. Off, or below Immortal,
//! nothing changes.
//!
//! **At Deity the Diplomatic seat sits it out too (live Deity Rome, 6-player
//! Continents, Online, 2026-11-13).** In all 73 games of the day that reached
//! t100, on every victory target, the state's `great_person_exhausted` listed
//! the Prophet by t46: a median t39, quartiles t34-40, the earliest t21, and
//! 72 of the 73 by t40. The second religion stood at a median t29 and the
//! third at t36. None of the 73 founded a religion. The two
//! Diplomatic-target games both entered the race from the capital:
//! `175230Z` researched Astrology at t30 "which diplomacy needs", placed the
//! Holy Site at t28-35 and held 7.7 Prophet points of the 30 a Prophet costs
//! when the class ran out at t40 (1.1 a turn; 52 by t80, for nothing);
//! `182344Z` held 2.3 at t40, built its Shrine at t42 and reached 133 by t80.
//! Their capitals placed a Campus at t59 and t77 against a median t41 for the
//! day's other capitals. So the Mahabodhi points and the founder's defence
//! this exemption was kept for are not on offer at Deity, and the capital
//! pays for the try exactly as the Culture seat's did at Immortal. At
//! Immortal the Diplomatic seat keeps the race: nothing has measured it there.

use super::*;

impl AdvancedAi {
    /// Whether this seat sits out the Prophet race at its difficulty: the gene
    /// is on, the majors play Immortal or Deity, and the seat is not aiming
    /// at a Religious victory. At Immortal a Diplomatic seat keeps the race:
    /// the Mahabodhi Temple's two points and the founder's Inquisition
    /// defence (`inquisition-defends-the-diplomat`) both need a religion of
    /// our own, and this gene's probe measured Culture seats only. At Deity
    /// it sits the race out as well: the last Prophet is gone by t46 there
    /// (see the module).
    pub(super) fn high_tier_skips_the_prophet_race(&self, g: &Game) -> bool {
        if !self.immortal_skips_the_prophet_race {
            return false;
        }
        let target = self.active_victory_target(g);
        match g.difficulty.as_str() {
            "deity" => target != Some(VictoryTarget::Religion),
            "immortal" => !matches!(
                target,
                Some(VictoryTarget::Religion | VictoryTarget::Diplomacy)
            ),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests;
