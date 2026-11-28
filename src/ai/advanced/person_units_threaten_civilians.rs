//! `person-units-threaten-civilians` (hotseat): the soldiers of a rival a
//! person plays threaten our civilians whatever the war state.
//!
//! **The finding (hotseat `hotseat-20261128T031456Z`, seat p1, Greece, the
//! religion seat, 2026-11-27).** The person (Sumeria) kept a Scout beside the
//! seat's capital from turn 9 at peace (`events.jsonl` lines 205-278: the
//! Scout on (60, 26), (61, 27), (61, 28)). On turn 10 the first Settler left
//! the capital ahead of its guard ("Settler marches while its guard catches
//! up | nothing visible can reach this tile", `why.log` turn 10), on turn 11
//! it stood on (60, 29) beside the Scout, and on the person's turn a surprise
//! war took it. Every reader that asks whether a civilian can be taken
//! skipped the Scout because we were not at war with its owner:
//! `settlement_tile_risk_with_support` (step and site risk),
//! `unstacked_settler_step_is_capturable`, the escort's threat gate in
//! `stacked_escort_pace`, and the clear-road test of
//! `settler-marches-alone-when-clear` (`road_is_clear`). Against a Firaxis
//! AI that is a fair reading: it declares with a warning and the war state
//! moves before its units do. A person declares and captures on one turn.
//!
//! **The gene.** Inert with the flag off. On, a unit of a rival the host
//! marks as played by a person ([`Game::observed_person_rivals`]: the mod
//! writes `rivals[].person` from `HotseatHumanSeats`, only on a hotseat
//! install) counts in those four readers as if we were at war with it, and
//! the risk prices it at capture reach (its moves) for a civilian's step
//! and for a site alike. A board where no rival carries the mark (every
//! unattended ladder board, every single-player game, a `--self-play`
//! table) reads exactly as before.

use super::*;

impl AdvancedAi {
    /// Whether a person plays `owner` and the gene reads its soldiers as able
    /// to take a civilian of ours at any war state. False with the gene off
    /// and on every board where no rival carries the host's `person` mark.
    pub(super) fn person_threatens_civilians(&self, g: &Game, pid: usize, owner: usize) -> bool {
        self.person_units_threaten_civilians
            && owner != pid
            && g.observed_person_rivals.contains(&owner)
    }

    /// Whether `owner`'s units threaten a civilian of ours: we are at war
    /// with `owner`, or a person plays it under the gene. With the gene off,
    /// exactly `is_at_war`.
    pub(super) fn civilian_threat_owner(&self, g: &Game, pid: usize, owner: usize) -> bool {
        g.is_at_war(pid, owner) || self.person_threatens_civilians(g, pid, owner)
    }
}

#[cfg(test)]
mod tests;
