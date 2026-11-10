//! `domination-charts-every-capital`: a Domination seat keeps charting until
//! every rival's original capital is on its map, and takes Cartography
//! sooner.
//!
//! **Why (live Deity, Online, Small Continents, 6 majors; 80 runs past turn
//! 120 of 2026-11-09/10).** Three or more of the five rival capitals stood off
//! our landmass, or unproven on it, in 94% of games. Same-land capitals were
//! first seen at a median turn 100, overseas ones at turn 146 (IQR 128-160),
//! and all five were seen in 11 of 80 games. Rivals took 8 of 210 rival
//! capitals from each other, so a Domination win takes every one itself,
//! and `amphibious-siege-train-2` and `naval-crossing-keeps-going` need a
//! capital to sail for. The Religion seat had the same gap and
//! `religion-seat-charts-the-rivals` measured it: Cartography at turns
//! 79-113, the hull a build behind it, the hull losing its queue head to six
//! other claims, and the one ship drafted into Destroy missions and Settler
//! escorts.
//!
//! **What the gene does**, for a seat on the Domination lane from
//! [`CHARTING_FROM_STANDARD`] standard turns (turn 40 Online) while a living
//! major is unmet or a met major's original capital has never been seen
//! ([`AdvancedAi::uncharted_rival_capitals`]):
//! - **Gap.** The contact gap stays open (`BasicAi::contact_gap_open`), so
//!   `contact-fleet`'s hull hold, `sea-scout-claims-the-queue`'s claim and
//!   the ocean-crossing landfall routing go on after every major is met and
//!   located and every city-state is met.
//! - **The Religion seat's charting parts**, for this seat as well
//!   ([`AdvancedAi::seat_charts_the_water`]): each technology on the way to
//!   Cartography is worth `CARTOGRAPHY_VALUE` more while the empire touches
//!   water; the hull may be laid down at Buttress, a technology early; the
//!   committed hull keeps its queue head against the other claims; and the
//!   first unlinked warship of ours with water to chart is left out of the
//!   objective board's pool and the Settler's naval guard.
//!
//! It lays down one hull at a time and releases no land unit. The Religion
//! seat's spreader parts stay the Religion seat's. Off, or on any other lane,
//! nothing changes.

use super::*;

/// Standard turns before the gene acts: turn 40 Online, after the opening's
/// first army and Settlers and before the turn-80 Cartography it aims at.
pub(super) const CHARTING_FROM_STANDARD: u32 = 60;

impl AdvancedAi {
    /// Whether `pid` has seen `rival`'s original capital: on the board on a
    /// tile it has explored, or in its city memory. The live board holds
    /// only cities the seat has seen and the native board every city, so the
    /// explored test makes the two agree.
    pub(super) fn rival_capital_charted(g: &Game, pid: usize, rival: usize) -> bool {
        let Some(seat) = g.players.get(pid) else {
            return false;
        };
        g.cities.values().any(|city| {
            city.is_capital && city.original_owner == rival && seat.explored.contains(&city.pos)
        }) || seat
            .remembered_cities
            .values()
            .any(|city| city.is_capital && city.original_owner == rival)
    }

    /// Living majors `pid` has not met, plus met majors whose original
    /// capital it has never seen.
    pub(super) fn uncharted_rival_capitals(g: &Game, pid: usize) -> usize {
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !rival.is_free_city
                    && !g.same_team(pid, rival.id)
                    && (!g.has_met(pid, rival.id) || !Self::rival_capital_charted(g, pid, rival.id))
            })
            .count()
    }

    /// Whether the gene acts for this seat this turn. See the module.
    pub(super) fn domination_charts_every_capital_active(&self, g: &Game, pid: usize) -> bool {
        self.domination_charts_every_capital
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && g.turn >= g.standard_duration(CHARTING_FROM_STANDARD)
            && Self::uncharted_rival_capitals(g, pid) > 0
    }

    /// Whether a lane's charting gene acts for this seat this turn and keeps
    /// the contact gap open (`BasicAi::lane_charting_gap`): this gene's seat.
    pub(super) fn lane_charts_the_water(&self, g: &Game, pid: usize) -> bool {
        self.domination_charts_every_capital_active(g, pid)
    }

    /// Whether the charting parts `religion-seat-charts-the-rivals` gives a
    /// Religion founder act for this seat: that gene's seat, or a lane
    /// charting gene's ([`Self::lane_charts_the_water`]).
    pub(super) fn seat_charts_the_water(&self, g: &Game, pid: usize) -> bool {
        self.religion_seat_charts_the_rivals_active(g, pid) || self.lane_charts_the_water(g, pid)
    }
}

#[cfg(test)]
mod tests;
