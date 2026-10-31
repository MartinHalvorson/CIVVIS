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
//! and Deity for a seat whose victory target is neither Religion nor
//! Diplomacy (whose Mahabodhi points and founder defence need a faith), before
//! any other test, unless a Prophet is pending (a Prophet earned or bought still
//! founds). Every part of the race package admits through that gate or
//! `prophet_race_enterable_for`, which calls it. Off, or below Immortal,
//! nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether this seat sits out the Prophet race at its difficulty: the gene
    /// is on, the majors play Immortal or Deity, and the seat is aiming at
    /// neither a Religious nor a Diplomatic victory. A Diplomatic seat keeps
    /// the race: the Mahabodhi Temple's two points and the founder's
    /// Inquisition defence (`inquisition-defends-the-diplomat`) both need a
    /// religion of our own, and this gene's probe measured Culture seats only.
    pub(super) fn high_tier_skips_the_prophet_race(&self, g: &Game) -> bool {
        self.immortal_skips_the_prophet_race
            && matches!(g.difficulty.as_str(), "immortal" | "deity")
            && !matches!(
                self.active_victory_target(g),
                Some(VictoryTarget::Religion | VictoryTarget::Diplomacy)
            )
    }
}

#[cfg(test)]
mod tests;
