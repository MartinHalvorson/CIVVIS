//! `culture-seat-allies`: a seat assigned the Culture victory allies a safe
//! neighbour.
//!
//! **Why (live Immortal Pericles, 2026-10-30).** Across ten Lv 7 Culture
//! seats, none held an alliance at turn 80, 120 or 160. Four had a declared
//! friend at some point (civvis-20261030T123717Z: Norway from t112; the
//! seat's Research Alliance proposal of t115 never reached the host).
//! - `drop_off_lane_alliances` (`src/bin/civvis_orders.rs`) and the agent's
//!   `CivvisAllianceStep` pass the planner's alliances to the host only for
//!   a seat assigned the Diplomatic lane: an alliance forbids a war on the
//!   ally, which a Domination seat's campaign planner does not price. A
//!   Culture seat is not a Domination seat, and natively it does ally, so
//!   the live seat diverged from the one the genome was screened as.
//! - The seats took Civil Service at t55-72 and their first war came at
//!   t91-175 in nine of ten (153459Z: t34). An ally does not declare on us,
//!   and early wars were the culture seats' worst losses (-43: 201608Z lost
//!   Corinth at t27 and the capital at t80 to an Egypt war begun at t54).
//! - An alliance carries passage, which is +25% tourism toward the partner
//!   (`Game::international_tourism_multiplier`, -f6's #4199), and pays one
//!   Favor a turn per level.
//!
//! **What the gene does.** On a seat whose victory target is Culture:
//! - `propose_strategic_alliance` does not wait for Scientific Theory under
//!   `research-alliance-first`: with no Research Alliance yet legal it asks
//!   for the stock Cultural kind.
//! - Never a partner: the strongest science rival, or any rival that has
//!   landed a space project (a Science racer, the target of the launcher
//!   wars), nor the rival whose domestic tourists set our culture bar (the
//!   conquest counter's main target). The culture racer was already barred
//!   (`research_alliance_barred`, `nearest_finish_culture_clock`, and the
//!   agent's `CivvisAllianceCultureRacer`).
//! - The nearest rival is preferred ([`CULTURE_ALLY_NEAR_BONUS`] less
//!   [`CULTURE_ALLY_TILE_COST`] a tile between capitals): an early war is a
//!   neighbour's.
//! - The live binary keeps the seat's alliance orders and the agent takes
//!   them (`CultureAllies`, set by `tools/civ6_play.py` when this tag is
//!   armed).
//!
//! Off, or for a seat not aiming at Culture, nothing changes.

use super::*;

/// The ranking bonus for the nearest possible partner.
pub(super) const CULTURE_ALLY_NEAR_BONUS: f64 = 400.0;

/// Ranking bonus lost per tile between our capital and the partner's.
pub(super) const CULTURE_ALLY_TILE_COST: f64 = 16.0;

impl AdvancedAi {
    /// Whether the gene runs for this seat: on, and aiming at Culture.
    pub(super) fn culture_seat_allies_on(&self) -> bool {
        self.culture_seat_allies && self.victory_target == Some(VictoryTarget::Culture)
    }

    /// Whether this seat's alliance orders reach the host: the live orders
    /// binary keeps them off the Diplomatic lane only when this is true.
    pub fn culture_seat_allies_armed(&self) -> bool {
        self.culture_seat_allies_on()
    }

    /// Whether `other` is never a Culture seat's partner under the gene: the
    /// strongest science rival, a rival that has landed a space project, or
    /// the rival with the most domestic tourists. Always false with the gene
    /// off or off the Culture target.
    pub(super) fn culture_seat_ally_barred(&self, g: &Game, pid: usize, other: usize) -> bool {
        if !self.culture_seat_allies_on() {
            return false;
        }
        let rivals: Vec<usize> = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && g.has_met(pid, rival.id)
            })
            .map(|rival| rival.id)
            .collect();
        let strongest_science = rivals.iter().copied().max_by(|left, right| {
            Self::seat_science_per_turn(g, *left)
                .total_cmp(&Self::seat_science_per_turn(g, *right))
                .then_with(|| right.cmp(left))
        });
        let bar_holder = rivals
            .iter()
            .copied()
            .max_by_key(|rival| (g.domestic_tourists(*rival), std::cmp::Reverse(*rival)));
        Some(other) == strongest_science
            || Some(other) == bar_holder
            || Self::science_denial_stages(g, other) >= 1
    }

    /// The ranking term that prefers the nearest partner: zero with the gene
    /// off, off the Culture target, or without both capitals.
    pub(super) fn culture_seat_ally_nearness(&self, g: &Game, pid: usize, other: usize) -> f64 {
        if !self.culture_seat_allies_on() {
            return 0.0;
        }
        let capital = |seat: usize| {
            g.player_city_ids(seat)
                .into_iter()
                .find(|cid| g.cities[cid].is_capital)
                .map(|cid| g.cities[&cid].pos)
        };
        match (capital(pid), capital(other)) {
            (Some(ours), Some(theirs)) => (CULTURE_ALLY_NEAR_BONUS
                - CULTURE_ALLY_TILE_COST * g.wdist(ours, theirs) as f64)
                .max(0.0),
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests;
