//! `unchosen-war-takes-the-first-peace`: a war a major declared on us, while
//! we still hold fewer cities than the plan wants and no siege of ours
//! against it is feasible or under way, is offered white peace on every turn
//! from the first turn the host will sign one.
//!
//! **The finding (live Immortal 2026-10-26 19:10Z to 10-28 05:25Z, 142 games
//! reaching turn 82, 26-35 wars a major declared on us by turn 60).**
//! - The declaration is what retires the game. Our Production share of the
//!   leader is 0.38 the turn before (0.36 in games with no war at the same
//!   turns), then 0.25 ten turns in and 0.24 twenty turns in. 77% of these
//!   games retire by turn 95, against 37% with no major war.
//! - Nothing of ours burns: our Production still grows 30% in those twenty
//!   turns. But the empire turns to the war (military 45% of Production,
//!   buildings and districts 24%, against 20% and 45% at peace) and adds 0.3
//!   cities, while the aggressor, the Production leader in 77-83% of them at
//!   2.4 times our military, grows 77%.
//! - The host signs white peace readily once it may: of our white offers,
//!   0/74 were taken within two turns at war ages 0-7 and 31/44 (70%) at ages
//!   8-11, about half after that.
//! - We offered at a median war age of 5.5, then stopped: 35 of the 428 war
//!   turns at ages 10-30 carried an offer. The aggressor becomes the
//!   campaign ("Campaign aimed at X | already at war; no city objective yet"),
//!   so the gate's second-front and Recovery clauses skip it and the fatigue
//!   clause waits 24 turns. Those wars lasted a median 25 turns among those
//!   ended by turn 85, and 9 of 28 had not ended by then. We held 0.09
//!   foreign cities a game at turn 85.
//!
//! **Who declared.** The live mirror does not replay `Game::wars`
//! (`host_war_losses.rs`), and the ledger the simulator reconciles names the
//! lower seat as declarer, so neither says who started a war on a native
//! board. The seat keeps its own record instead: every frame,
//! [`AdvancedAi::observe_unchosen_wars`] marks a major it last saw at peace
//! that is now at war, unless this seat declared on it within
//! [`UNCHOSEN_WAR_OURS_TURNS`] (read from the frame's own action log by
//! [`AdvancedAi::note_frame_declarations`]). A war already running when the
//! record began is never counted.
//!
//! **A war we can win is kept.** "Feasible" is the war board's own reading
//! (`war_policy_siege_against`): the whole roster meets the bill of a Siege
//! row against them, or of their nearest city. Live, the aggressor stood at
//! or above our power at war age 8 in 19 of 26 of these wars and we took no
//! city from any of them; the 7 weaker aggressors gave up one city in wars
//! lasting a median 67 turns. A 12-game simulator probe without this term,
//! where the rivals are this AI and their wars are more even, read
//! −20.7 ± 13.0 pp wins (0 of 14 seats on) and −0.6 ± 1.3 pp score share.
//!
//! **The offer.** It joins the peace gate's reasons, so every veto after it
//! still holds (the launcher, the faith-war holdout, `faith-war-before-the-flip`,
//! `prey-war-is-kept`), and it never asks a city: the point is to end the war.
//! From [`UNCHOSEN_WAR_PEACE_AGE`] turns of observed war while the host's own
//! minimum has passed.
//!
//! Off, nothing changes.
//!
//! **`peace-waits-for-the-host-minimum`** (2026-11-10). No peace offer of
//! any kind goes to a major whose war on us (as recorded here) is younger
//! than the host's minimum war length, [`PEACE_HOST_MINIMUM_STANDARD`]
//! standard turns (8 at Online). The live bridge sends one peace ask per
//! rival per five turns (`PeaceRetryTurns` in `CivvisControlAgent.lua`), so
//! an "outmatched" offer at war age 3-5, which the host rarely signs, holds
//! the next ask until age 8-10. Live Deity Online culture games of 11-10,
//! 57 wars a major declared on us that drew an ask: the host signed 5 of 56
//! asks at war ages 0-7, 10 of 13 at age 8 and 3 of 19 at ages 9-10, and the
//! seat's first ask came at a median age of 4. In
//! civvis-20261110T224752Z the asks went out at 5, 10 and 15 while Bolivar
//! took Örebro at 11 and Linköping at 14; in 141854Z at 5, 10 and 15 while
//! Shaka took Stockholm at 9 and Helsingborg at 14. With the
//! gene the first ask lands at 8. The record runs under either gene.
//! Off, nothing changes.

use super::*;

/// Observed war turns before the first offer: 0/74 white offers were taken
/// at ages 0-7 and 31/44 at ages 8-11.
pub(super) const UNCHOSEN_WAR_PEACE_AGE: u32 = 8;

/// A war this seat declared within this many turns of seeing it begin is
/// ours: the host can take a frame or a turn to report a declaration.
pub(super) const UNCHOSEN_WAR_OURS_TURNS: u32 = 2;

/// `peace-waits-for-the-host-minimum`: the host's minimum war length in
/// standard turns (`DIPLOMACY_WAR_MIN_TURNS`), 8 at Online.
pub(super) const PEACE_HOST_MINIMUM_STANDARD: u32 = 10;

impl AdvancedAi {
    /// Record which majors began a war on us since the last frame. Read
    /// before the frame plans; see the module. A no-op with the gene off.
    pub(super) fn observe_unchosen_wars(&mut self, g: &Game, pid: usize) {
        if !self.unchosen_war_takes_the_first_peace && !self.peace_waits_for_the_host_minimum {
            return;
        }
        let turn = g.turn;
        self.unchosen_war_declared
            .retain(|_, declared| turn.saturating_sub(*declared) <= UNCHOSEN_WAR_OURS_TURNS);
        for other in g.players.iter().filter(|other| {
            other.id != pid && other.alive && !other.is_minor && !other.is_barbarian
        }) {
            let at_war = g.is_at_war(pid, other.id);
            let was_at_war = self.unchosen_war_seen.insert(other.id, at_war);
            if !at_war {
                self.unchosen_war_since.remove(&other.id);
            } else if was_at_war == Some(false)
                && !self.unchosen_war_declared.contains_key(&other.id)
            {
                self.unchosen_war_since.insert(other.id, turn);
            }
        }
    }

    /// Remember the majors this frame declared war on, from the frame's own
    /// action log after `start`. A no-op with the gene off.
    pub(super) fn note_frame_declarations(&mut self, g: &Game, pid: usize, start: usize) {
        if !self.unchosen_war_takes_the_first_peace && !self.peace_waits_for_the_host_minimum {
            return;
        }
        let declared: Vec<usize> =
            g.log
                .since(start)
                .filter(|(seat, _)| *seat == pid)
                .filter_map(|(_, action)| match action {
                    Action::DeclareWar { player }
                    | Action::DeclareWarWithCasusBelli { player, .. } => Some(*player),
                    _ => None,
                })
                .collect();
        for player in declared {
            self.unchosen_war_declared.insert(player, g.turn);
            self.unchosen_war_since.remove(&player);
        }
    }

    /// The turn `rival` declared the war the peace gate should offer to end,
    /// when it qualifies: see the module. Always `None` with the gene off.
    pub(super) fn unchosen_war_peace_since(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        if !self.unchosen_war_takes_the_first_peace || !g.is_at_war(pid, rival) {
            return None;
        }
        let since = self.unchosen_war_since.get(&rival).copied();
        // Report the first binding predicate, in the same order as the gate.
        // A siege journal describes a different force from the whole-roster
        // feasibility test; it cannot establish why this offer was withheld.
        let blocked = match since {
            None => Some("no observed unchosen declaration"),
            Some(since) if g.turn.saturating_sub(since) < UNCHOSEN_WAR_PEACE_AGE => {
                Some("observed war is younger than eight turns")
            }
            _ if g.peace_available_at(pid, rival).is_some() => {
                Some("host minimum war duration has not passed")
            }
            _ if g.player_city_ids(pid).len() >= plan.desired_cities => {
                Some("the empire holds the planned city count")
            }
            _ if self.war_policy_siege_against(g, pid, rival).0 => {
                Some("whole-roster siege feasibility")
            }
            _ if self.domination_siege_is_progressing(g, pid, rival, plan) => {
                Some("a Domination siege is progressing")
            }
            _ if self.domination_siege_train_mobilizing(g, pid, rival, plan) => {
                Some("a Domination siege train is mobilizing")
            }
            _ => None,
        };
        if self.journal().wants(crate::reasoning::Level::Detail) {
            think!(self.journal(), Diplomacy, Detail,
                   "Early peace gate for {}", g.players[rival].civ;
                   "{}; observed declaration {:?}, turn {}, cities {}/{}",
                   blocked.unwrap_or("eligible; later diplomacy vetoes still apply"),
                   since, g.turn, g.player_city_ids(pid).len(), plan.desired_cities);
        }
        if blocked.is_none() {
            since
        } else {
            None
        }
    }

    /// `peace-waits-for-the-host-minimum`: the observed age of the war
    /// `rival` declared on us while it is younger than the host's minimum, so
    /// no offer goes out this turn. `None` with the gene off, at peace, for a
    /// war we declared, and for one already running when the record began.
    pub(super) fn young_war_peace_waits(&self, g: &Game, pid: usize, rival: usize) -> Option<u32> {
        if !self.peace_waits_for_the_host_minimum || !g.is_at_war(pid, rival) {
            return None;
        }
        let age = g.turn.saturating_sub(*self.unchosen_war_since.get(&rival)?);
        (age < g.standard_duration(PEACE_HOST_MINIMUM_STANDARD)).then_some(age)
    }
}

#[cfg(test)]
mod tests;
