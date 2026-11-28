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
//! **Version two (`spreaders-spread-before-they-chart-2`, 2026-11-27).**
//! With version one armed the reviewer still named the mistake in 35 of 40
//! Lv 8 games, and the gene fired in them (its "waits for the city" line is
//! in the journals). Replayed, the corps was still searching instead of
//! spreading, because three searches each take their own share and none
//! counts the others:
//! - t8-20261127T064630Z at t90: seven spreaders, five known rival cities
//!   owed. Three Missionaries on their last charge scouted the fog
//!   (`last-charge-joins-the-army`), two seekers looked for the unseen and
//!   under-charted rivals, one waited on its held march, and none spread.
//!   The seekers were the two farthest from home, which were the corps' full
//!   spreaders (three charges each). Rivals converted at t100: 0.
//! - t8-20261127T094009Z at t110: 27 spreaders, 14 known rival cities owed.
//!   Eleven charted, nearly all through the scouts' surplus clause, beside
//!   two seekers and the last-charge scouts, and the corps spread 14 times
//!   in t100-t124.
//! - Over five of the quoted games, at least one last-charge Missionary was
//!   already scouting on 401 of the 443 turns a seeker was out (969 seeker
//!   unit-turns).
//!
//! Version two keeps version one and makes the searches share one budget,
//! one in [`SCOUT_SHARE`] of the whole corps (at least one):
//! - The seekers' quota is what the budget leaves after the last-charge
//!   scouts already walking toward a goal
//!   ([`AdvancedAi::seekers_left_by_the_scouts`]), and the seekers are the
//!   spreaders with the fewest charges (a seeker keeps its charges, so the
//!   full spreaders keep the known work).
//! - The scouts' quota counts each searcher once
//!   ([`AdvancedAi::scouts_after_the_searchers`]): the surplus clause, like
//!   the under-charted share, is less this turn's seekers, and the seekers
//!   leave the list the scouts are picked from (version one subtracted them
//!   from the share only, and a seeker at the head of that list spent a
//!   slot again). A last-charge unit keeps its slot at the head as before.
//!
//! A spreader with nothing it knows left to convert still charts as before.
//! Off, nothing changes.

use super::religion_seat_charts_the_rivals::SCOUT_SHARE;
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
        if !self.spreads_before_it_charts() || sites.is_empty() {
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
        if !self.spreads_before_it_charts() {
            return 0;
        }
        self.unseen_rival_seekers(g, pid, religion, offensive).len()
    }

    /// Either version of the family is on: version two carries version one.
    fn spreads_before_it_charts(&self) -> bool {
        self.spreaders_spread_before_they_chart || self.spreaders_spread_before_they_chart_2
    }

    /// `spreaders-spread-before-they-chart-2` is on: the corps' searchers
    /// share one budget.
    pub(super) fn searchers_share_one_budget(&self) -> bool {
        self.spreaders_spread_before_they_chart_2
    }

    /// The Missionaries of `pid` that `last-charge-joins-the-army` keeps on
    /// their last charge and that are walking toward a fog goal: the corps'
    /// searchers that no quota names. Zero with version two off.
    pub(super) fn last_charge_scouts_out(&self, g: &Game, pid: usize) -> usize {
        if !self.searchers_share_one_budget() {
            return 0;
        }
        let memory = self.missionary_explore.borrow();
        g.units
            .values()
            .filter(|unit| {
                unit.owner == pid
                    && unit.kind == "missionary"
                    && memory
                        .get(&unit.id)
                        .is_some_and(|entry| entry.goal.is_some())
                    && self.keeps_its_last_charge(g, pid, unit.id)
            })
            .count()
    }

    /// The corps' one search budget: one in [`SCOUT_SHARE`] of `corps`
    /// spreaders, at least one.
    fn search_budget(corps: usize) -> usize {
        (corps / SCOUT_SHARE).max(1)
    }

    /// How many seekers the budget leaves, given `seekable` spreaders that
    /// may seek: the budget over them and the last-charge scouts out, less
    /// those scouts. `None` with version two off, which leaves the seekers'
    /// own quota.
    pub(super) fn seekers_left_by_the_scouts(
        &self,
        g: &Game,
        pid: usize,
        seekable: usize,
    ) -> Option<usize> {
        if !self.searchers_share_one_budget() {
            return None;
        }
        let scouts = self.last_charge_scouts_out(g, pid);
        Some(Self::search_budget(seekable + scouts).saturating_sub(scouts))
    }

    /// The corps' charting scouts this turn under version two: the
    /// under-charted share and the `surplus` clause of
    /// `religion-seat-charts-the-rivals`, each less this turn's seekers,
    /// taken from `corps` (sorted fewest charges first) without those
    /// seekers. A unit on its last charge keeps its place at the head of
    /// `corps` as in version one, so a last-charge scout already out spends
    /// a slot there; each searcher is counted once. `None` with version two
    /// off.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn scouts_after_the_searchers(
        &self,
        g: &Game,
        pid: usize,
        religion: &str,
        offensive: bool,
        corps: &[(i32, u32)],
        under_charted: usize,
        surplus: usize,
    ) -> Option<Vec<u32>> {
        if !self.searchers_share_one_budget() {
            return None;
        }
        let seekers = self.unseen_rival_seekers(g, pid, religion, offensive);
        let quota = under_charted
            .min(corps.len() / SCOUT_SHARE)
            .saturating_sub(seekers.len())
            .max(surplus.saturating_sub(seekers.len()));
        Some(
            corps
                .iter()
                .map(|(_, id)| *id)
                .filter(|id| !seekers.contains(id))
                .take(quota)
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests;
