//! `campaign-second-front-waits-for-the-staging`: the campaign's second front
//! (`one_war_second_front`) opens beside the running front only once that
//! front's siege has more than [`SECOND_FRONT_STAGED_UNITS`] land units within
//! the staging band of one of its cities, or we took a city in the last
//! [`SECOND_FRONT_CAPTURE_TURNS`] standard turns, or the new target's clock is
//! urgent.
//!
//! On the 25 Emperor gate passers of 2026-10-14 07:25Z to 10-15 09:40Z (the
//! census by -34's war-count fork), at 1.5 times the strongest enemy's power
//! or better, turns with one war of our own opening converted 25.8 captures
//! per 1,000 (17 in 659 turns, 22 games) and turns with two 5.6 (1 in 180, 9
//! games). The campaign's own second front opened 7 of the 14 second
//! offensive wars, and in those games the first front's siege stood at 0-6
//! units staged.
//!
//! The road blockers (`blocker-becomes-the-target`,
//! `closed-border-blocker-declares`, `racer-blocker-declares`), the launched
//! racer, the faith war and the interceptions are untouched. A rival already
//! named the second front at the last observation keeps the name (the
//! opening and holding ratios already work that way), so the plan does not
//! flicker with the staged count.
//!
//! Off: unchanged.

use super::*;

/// `campaign-second-front-waits-for-the-staging`: the second front opens once
/// the first front's siege has more than this many land units within
/// `siege_train::STAGING_FAR` of one of its cities.
pub(crate) const SECOND_FRONT_STAGED_UNITS: usize = 4;

/// `campaign-second-front-waits-for-the-staging`: a city taken within this
/// many standard turns opens the second front whatever the staging.
pub(crate) const SECOND_FRONT_CAPTURE_TURNS: u32 = 15;

/// `campaign-second-front-waits-for-the-staging`: standard turns between two
/// "waits for the staging" lines about one rival.
const SECOND_FRONT_STAGING_NOTE_TURNS: u32 = 5;

impl AdvancedAi {
    /// `campaign-second-front-waits-for-the-staging`: the most of our land
    /// units standing within `siege_train::STAGING_FAR` of one of `front`'s
    /// cities, the siege train's "N of M units staged" count read without
    /// its force rows.
    pub(crate) fn front_units_staged(&self, g: &Game, pid: usize, front: usize) -> usize {
        let soldiers = g
            .player_unit_ids(pid)
            .into_iter()
            .filter_map(|uid| g.units.get(&uid))
            .filter(|unit| {
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && !g.is_embarked(unit)
                    && (spec.has_ranged_attack() || spec.is_melee_capable())
            })
            .map(|unit| unit.pos)
            .collect::<Vec<_>>();
        g.player_city_ids(front)
            .into_iter()
            .map(|cid| {
                let pos = g.cities[&cid].pos;
                soldiers
                    .iter()
                    .filter(|at| g.wdist(**at, pos) <= siege_train::STAGING_FAR)
                    .count()
            })
            .max()
            .unwrap_or(0)
    }

    /// `campaign-second-front-waits-for-the-staging`: whether we took a city
    /// within [`SECOND_FRONT_CAPTURE_TURNS`] standard turns.
    fn second_front_capture_recent(&self, g: &Game) -> bool {
        let window = g.standard_duration(SECOND_FRONT_CAPTURE_TURNS);
        self.second_front_captures
            .values()
            .any(|turn| g.turn.saturating_sub(*turn) <= window)
    }

    /// `campaign-second-front-waits-for-the-staging`: the reason the second
    /// front on `rival` waits, or `None` when it may open: the gene off,
    /// `rival` already at war, an urgent clock, the launched racer, a road
    /// blocker, the rival named at the last observation, no front, a front
    /// with more than [`SECOND_FRONT_STAGED_UNITS`] units staged, or a city
    /// taken in the window.
    pub(crate) fn campaign_second_front_wait(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<String> {
        if !self.campaign_second_front_waits_for_the_staging
            || g.is_at_war(pid, rival)
            || self.one_war_second == Some(rival)
            || self.urgent_victory_threat(g, rival)
            || self
                .launched_racer_finish(g, pid)
                .is_some_and(|(racer, _)| racer == rival)
            || self.road_blocker_front(g, pid) == Some(rival)
        {
            return None;
        }
        let front = self.one_war.as_ref()?.target;
        if front == rival || !g.is_at_war(pid, front) {
            return None;
        }
        let staged = self.front_units_staged(g, pid, front);
        if staged > SECOND_FRONT_STAGED_UNITS || self.second_front_capture_recent(g) {
            return None;
        }
        Some(format!(
            "the front on {} has {staged} unit(s) staged (needs {}) and no city taken in {} turns",
            g.players[front].civ,
            SECOND_FRONT_STAGED_UNITS + 1,
            SECOND_FRONT_CAPTURE_TURNS
        ))
    }

    /// The per-turn observation: the cities we took (for the capture window)
    /// and the "waits for the staging" line. Nothing with the gene off.
    pub(crate) fn observe_second_front_staging(&mut self, g: &Game, pid: usize) {
        if !self.campaign_second_front_waits_for_the_staging {
            return;
        }
        let ours = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| g.cities[cid].original_owner != pid)
            .collect::<BTreeSet<_>>();
        self.second_front_captures.retain(|cid, _| ours.contains(cid));
        for cid in ours {
            self.second_front_captures.entry(cid).or_insert(g.turn);
        }
        let Some(rival) = self.one_war_second_front_unstaged(g, pid) else {
            return;
        };
        let Some(reason) = self.campaign_second_front_wait(g, pid, rival) else {
            return;
        };
        let every = g.standard_duration(SECOND_FRONT_STAGING_NOTE_TURNS);
        let due = self
            .second_front_staging_noted
            .get(&rival)
            .is_none_or(|turn| g.turn.saturating_sub(*turn) >= every);
        if due {
            self.second_front_staging_noted.insert(rival, g.turn);
        }
        if due && self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Military, Detail,
                   "Second front waits for the staging: {}", g.players[rival].civ;
                   "campaign-second-front-waits-for-the-staging: {reason}");
        }
    }
}

#[cfg(test)]
mod tests;
