//! `dvp-leader-is-the-front`: a Diplomatic Victory leader within a session
//! of the win is a clock a Domination army answers, and opens a second front
//! beside the war it is already fighting.
//!
//! Live Emperor civvis-20261008T132311Z (game 402) passed the production
//! gate at rank 1, held 14 cities and 1,809 military and took Amsterdam at
//! turn 205, then lost to Nubia's Diplomatic Victory at 207. Nubia stood at
//! 15 points from the turn-181 session and at peace with us, while the army
//! fought the Netherlands and Scythia. Nothing could name it:
//!
//! - `domination_counter_pressure` answers Culture, Science and Religion
//!   clocks; a Diplomacy reading maps to the in-lane Diplomacy counter, which
//!   moves no army, so `domination_military_counter` never listed it and the
//!   journal read "Countering Netherlands | its diplomacy race reads 51%" at
//!   turn 199 with Nubia on 15 points (a reading of 75 or more).
//! - `diplomatic_contender_at_peace` needs no other war running.
//! - `diplomatic_contender_leader` opens a second front only at 16 points and
//!   four times the rival's military (`ONE_WAR_CRUSHED_RATIO`); our 1,108 to
//!   1,776 stood at 1.4-2.5 times Nubia's 572-777 over turns 183-201.
//!
//! With the gene, a Diplomacy reading of [`DVP_FRONT_PRESSURE`] (15 points)
//! is a Domination counter clock, and the Diplomatic Victory leader at
//! [`DVP_FRONT_POINTS`] opens the second front at once when we hold the
//! declaration edge ([`super::one_war::DECLARATION_EDGE_RATIO`]) over its
//! steady military. Only elimination takes those points off the board, so
//! `diplomatic_contender_to_eliminate` then holds the front on it.

use super::one_war::{DECLARATION_EDGE_RATIO, DIPLOMATIC_CONTENDER_DVP};
use super::*;

/// The Diplomatic Victory points at which the leader opens a second front:
/// one session from the win (a session awards +2 to the A target and +1 for
/// each resolution voted with the winner; Nubia took +4 at turn 202).
pub(crate) const DVP_FRONT_POINTS: i64 = DIPLOMATIC_CONTENDER_DVP;

/// The Diplomacy pressure (`dvp * 5 + suzerainties * 6`) a Domination army
/// answers: fifteen points, or fewer behind the Favor engine of the
/// city-states it holds.
pub(crate) const DVP_FRONT_PRESSURE: i32 = 5 * DVP_FRONT_POINTS as i32;

impl AdvancedAi {
    /// `dvp-leader-is-the-front`: whether a Diplomacy `pressure` reading is a
    /// clock the Domination army answers (see `domination_counter_pressure`).
    pub(super) fn dvp_pressure_is_a_clock(&self, pressure: VictoryFocus) -> bool {
        self.dvp_leader_is_the_front
            && pressure.strategy == GrandStrategy::Diplomacy
            && pressure.progress >= DVP_FRONT_PRESSURE
    }

    /// `dvp-leader-is-the-front`: the Diplomatic Victory leader at
    /// [`DVP_FRONT_POINTS`] or more, under our military
    /// [`DECLARATION_EDGE_RATIO`] times over its steady reading. `None` with
    /// the gene off or outside a Domination plan.
    pub(crate) fn dvp_leader_front(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.dvp_leader_is_the_front
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return None;
        }
        let ours = g.military_power(pid);
        g.players
            .iter()
            .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
            .max_by_key(|p| (p.dvp, std::cmp::Reverse(p.id)))
            .filter(|leader| leader.dvp >= DVP_FRONT_POINTS)
            .filter(|leader| {
                ours >= DECLARATION_EDGE_RATIO * self.steady_rival_power(g, leader.id).max(1.0)
            })
            .map(|leader| leader.id)
    }

    /// `congress-guards-the-leader`: whether the bridge leases the Congress
    /// ballot its guard on the Diplomatic Victory leader (a
    /// `combat_policy` `CONGRESS_GUARDS_THE_LEADER` row each batch, read by
    /// `CivvisCongressRedirect`).
    pub fn congress_guards_the_leader_enabled(&self) -> bool {
        self.congress_guards_the_leader
    }
}

#[cfg(test)]
mod tests;
