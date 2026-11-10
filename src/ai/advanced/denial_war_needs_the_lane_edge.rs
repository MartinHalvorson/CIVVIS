//! `denial-war-needs-the-lane-edge`: off the Domination lane, the separate
//! science-denial declaration needs the launcher counter's edge and spares a
//! friend.
//!
//! Live Deity Online Kristina (Sweden, Culture), 14 games of 2026-11-10: the
//! `science-denial-war` route declared 13 wars "to deny the space race" at
//! 0.04-0.88 of the target's military (median 0.25), mostly on a land
//! neighbour 4-7 hexes away. The same frames the armed launcher counter
//! (`launcher-war-serves-every-lane`) refused the same war, "under 1.2
//! times". All 13 cities lost in those wars went to the rival we declared
//! on, 10 of them at t140-175 (160429Z alone lost 7 to Vietnam), and the
//! target still won the Science Victory in 6 of the 7 games that finished.
//! In 5 games the war's opening denounced a declared friend on the turn its
//! friendship came up for renewal (140731Z Vietnam t134, 144009Z India
//! t155), so `friendship-keeps-the-peace` logged "we have denounced them"
//! and the declaration followed 3-5 turns later.
//!
//! With the gene on, on any victory target but Domination (which keeps its
//! own 0.8x launcher edge), a denial war every other gate admits is refused
//! unless our military power reaches `lane_launcher_edge` times the rival's
//! steady power (1.2x; 1.4x on a Diplomatic seat) and the rival is neither a
//! declared friend nor an ally, nor a friend whose friendship lapsed within
//! [`FRIENDSHIP_RENEW_WINDOW`] turns (the host still counting it as one
//! included). The war's denunciation sits past this gate, so it stops too.
//! The spy rungs and a denial war already running are untouched.
//! `religion-denial-needs-the-edge` is the same edge on Religion seats only;
//! the two can be armed together.

use super::protective_friendship::FRIENDSHIP_RENEW_WINDOW;
use super::*;

impl AdvancedAi {
    /// Whether the science-denial war on `rival` passes this gene. Always
    /// `true` with the gene off, on a Domination seat, or with no victory
    /// target. Called last among the war's gates, so a refusal, which is
    /// journalled, marks a war that would otherwise have opened.
    pub(super) fn denial_war_has_the_lane_edge(&self, g: &Game, pid: usize, rival: usize) -> bool {
        if !self.denial_war_needs_the_lane_edge {
            return true;
        }
        let Some(target) = self
            .active_victory_target(g)
            .filter(|target| *target != VictoryTarget::Domination)
        else {
            return true;
        };
        let ours = g.military_power(pid);
        let theirs = self.steady_rival_power(g, rival).max(1.0);
        let edge = self.lane_launcher_edge(g);
        let gate = if let Some(friend) = self.denial_war_spares_the_friend(g, pid, rival) {
            friend.to_string()
        } else if ours < edge * theirs {
            format!(
                "a denial war needs {edge:.1} times their steady power; {ours:.0} against {theirs:.0}"
            )
        } else {
            return true;
        };
        let lane = match target {
            VictoryTarget::Science => "Science",
            VictoryTarget::Culture => "Culture",
            VictoryTarget::Religion => "Religion",
            VictoryTarget::Diplomacy => "Diplomacy",
            VictoryTarget::Domination => "Domination",
            VictoryTarget::Score => "Score",
        };
        think!(self.journal(), Military, Decision,
               "{lane} keeps the peace with {}", g.players[rival].civ;
               "denial-war-needs-the-lane-edge: {gate}");
        false
    }

    /// Why `rival` is spared as a friend, or `None`: a declared friend, an
    /// ally, a friend the host still counts as one, or a friendship that
    /// lapsed within [`FRIENDSHIP_RENEW_WINDOW`] turns, which
    /// `friendship-keeps-the-peace` renews unless we denounce first.
    fn denial_war_spares_the_friend(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<&'static str> {
        if g.are_friends(pid, rival) {
            return Some("they are a declared friend");
        }
        if g.alliance_with(pid, rival).is_some() {
            return Some("they are an ally");
        }
        if g.players[rival]
            .observed_diplomatic_state
            .get(&pid)
            .is_some_and(|state| matches!(state.as_str(), "DECLARED_FRIEND" | "ALLIED"))
        {
            return Some("the host still counts them a friend");
        }
        let lapsed = |until: u32| until <= g.turn && g.turn <= until + FRIENDSHIP_RENEW_WINDOW;
        if g.players[pid]
            .friends_until
            .get(&rival)
            .copied()
            .is_some_and(lapsed)
            || self
                .friendship_expiry_seen
                .borrow()
                .get(&rival)
                .copied()
                .is_some_and(lapsed)
        {
            return Some("our friendship lapsed within 2 turns and is renewed, not denounced");
        }
        None
    }
}

#[cfg(test)]
mod tests;
