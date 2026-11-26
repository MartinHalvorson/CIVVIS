//! `spreaders-spread-before-they-chart`: a Religion seat's spreaders work the
//! cities they know before they walk off to chart.
//!
//! **Why (Indefinite Benchmarking Tourney, Lv 8 Deity, 6 majors, Continents,
//! Online, 2026-11-26).** The reviewer named "spreaders scout instead of
//! spreading" in 13 of 40 games. Counted in unit-turns (the journal re-logs a
//! decision several times a turn), the corps of civvis-t8-20261126T081944Z
//! charted or sought on 348 unit-turns of t100-t180 while Brazil's 10 known
//! cities stayed off our faith from t100 to t180 and the corps priced 14 to 42
//! known rival cities it still owed. Two paths sent units away from that work:
//! - **A held march charted.** `spreader-steps-out-of-the-apostle` holds a
//!   spreader whose every step toward its target ends in a rival Apostle's
//!   reach. The target loop then tries the next target, holds again, and
//!   falls through to `religion-seat-charts-the-rivals`, which logs "nothing
//!   we know is left to convert" and walks the unit 10-20 tiles into the fog.
//!   In 081944Z missionary 66 held toward 20 targets at t120 and then
//!   charted. Across the eight games the reviewer quoted, 239 charting
//!   unit-turns came straight after a hold (9 to 61 a game). A held spreader
//!   is a turn from its city, and `stalled-spreader-braves-the-march` sends it
//!   on after a few held turns; walked off to chart, it is neither.
//! - **Two quotas for one gap.** `religion-seat-seeks-the-unseen-rival` gives
//!   each under-charted rival a seeker, up to one in four of the corps, and
//!   `religion-seat-charts-the-rivals` still takes its own scouts for the
//!   same rivals, one in four again. Armed together they take up to half
//!   the corps for one count of rivals (081944Z t120-t150: two seekers and
//!   three to five charting units a turn of a corps of about thirteen).
//!
//! **What the gene does**, where `religion-seat-charts-the-rivals` acts:
//! - A spreader of our faith that reached the end of its target list without
//!   a step, while one of those cities is still on a route it can follow
//!   ([`AdvancedAi::held_spreader_waits_for_its_city`]), holds instead of
//!   charting. With no target, or none it can reach, it charts as before.
//! - The scouts' quota for under-charted rivals counts the seekers already
//!   looking for them ([`AdvancedAi::scouts_left_to_the_seekers`]). The
//!   surplus clause (a corps far past its known work) is untouched.
//!
//! Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// Whether `uid`, a spreader of our own faith on a seat
    /// `religion-seat-charts-the-rivals` acts for, came to the end of its
    /// targets `sites` with one of them still on a route it can follow: its
    /// march was held, so it waits for the city instead of charting. False
    /// with the gene off, for another faith, or when no site is reachable.
    pub(super) fn held_spreader_waits_for_its_city(
        &self,
        g: &Game,
        uid: u32,
        religion: &str,
        sites: &[Pos],
    ) -> bool {
        if !self.spreaders_spread_before_they_chart || sites.is_empty() {
            return false;
        }
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let pid = unit.owner;
        if g.players[pid].religion.as_deref() != Some(religion)
            || !self.religion_seat_charts_the_rivals_active(g, pid)
        {
            return false;
        }
        let Some(site) = sites
            .iter()
            .copied()
            .find(|site| g.route_step(uid, *site, 1).is_some())
        else {
            return false;
        };
        think!(self.journal(), Faith, Detail,
            "{} spreader {} waits for the city it was sent to", religion, uid;
            "spreaders-spread-before-they-chart: its march was held, not finished, and a city we still owe is on its road {} tiles away; charting would walk it away from that city",
            g.wdist(unit.pos, site);
            site);
        true
    }

    /// How many of the scouts `religion-seat-charts-the-rivals` gives the
    /// under-charted rivals are already seekers of
    /// `religion-seat-seeks-the-unseen-rival`: this turn's seekers. Zero with
    /// the gene off or the seekers' gene off.
    pub(super) fn scouts_left_to_the_seekers(
        &self,
        g: &Game,
        pid: usize,
        religion: &str,
        offensive: bool,
    ) -> usize {
        if !self.spreaders_spread_before_they_chart {
            return 0;
        }
        self.unseen_rival_seekers(g, pid, religion, offensive).len()
    }
}

#[cfg(test)]
mod tests;
