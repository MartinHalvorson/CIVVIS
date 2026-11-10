//! `religion-denial-needs-the-edge`: a Religion seat's separate science-denial
//! declaration needs the same 1.2x steady military edge as its launcher counter.
//!
//! Native Russia civvis-20261110T024805Z-cont3, t146: the launcher counter
//! declined Vietnam at 454 power against a steady 1236, but the independent
//! `science-denial-war` route declared Formal War that frame. Vietnam already
//! reported our Buddhist majority. Four Russian cities, including the capital,
//! subsequently fell during that war at t165/169/172. Those observations do not
//! prove that withholding the declaration wins the game.
//!
//! This default-off experiment closes that one route below the existing
//! `LANE_LAUNCHER_EDGE`. It leaves spies, ongoing wars, other declarations and
//! other victory targets to their existing rules. Stronger religious seats
//! still need every original affordability, finish-horizon and pad-reach gate.

use super::*;

impl AdvancedAi {
    pub(super) fn religion_denial_has_the_edge(&self, g: &Game, pid: usize, rival: usize) -> bool {
        if !self.religion_denial_needs_the_edge
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
        {
            return true;
        }
        let ours = g.military_power(pid);
        let theirs = self.steady_rival_power(g, rival).max(1.0);
        let edge = super::launcher_war_every_lane::LANE_LAUNCHER_EDGE;
        if ours >= edge * theirs {
            return true;
        }
        think!(self.journal(), Military, Detail,
               "Religion keeps the peace with {}", g.players[rival].civ;
               "religion-denial-needs-the-edge: science denial needs {edge:.1} times their steady power; {ours:.0} against {theirs:.0}");
        false
    }
}
