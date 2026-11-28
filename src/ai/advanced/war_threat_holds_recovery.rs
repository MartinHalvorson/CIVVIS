//! `war-threat-holds-recovery`: a city threatened in the last few turns of a
//! major war still holds the plan in Recovery.
//!
//! Shipped, `assess` reads two Recovery arms during a war: the power gap
//! (our army under four fifths of the strongest opponent's) and a threatened
//! city. `bounded-recovery` releases the power-gap arm once the war is
//! `RECOVERY_POSTURE_LIMIT` standard turns old, which is 12 turns on Online
//! speed. From then on only a city threatened in THIS assessment holds the
//! posture, and `threatened_city` reads the visible hostiles near our cities:
//! an attacking stack steps out of sight, or pulls back to heal, and the next
//! assessment hands the plan to the lane; it steps back in and the plan
//! returns to Recovery. The Builders, claims, muster and peace that read the
//! plan swing with it.
//!
//! The Indefinite Benchmarking Tourney's reviewer named it in 4 of 40 Lv 8
//! games on 2026-11-27 (`grand-strategy-flips-to-religion-during-losing-war`):
//! - 100321Z: war with Korea from t76; Religion and Recovery alternate from
//!   t87 to t149, three times on t92 alone, at power 210 against 956;
//! - 123004Z: 11 flips from t59 to t68 (237 against 401), back at t136-t139
//!   (272 against 963) after the capital and Nicomedia fell;
//! - 151254Z: 18 changes from t130 to t161 at 375-438 against 706-1924; each
//!   Recovery turn logs "is under threat", each Religion turn does not;
//! - 163940Z: t140-t174 at 581-729 against 2203-3393 while five cities fell.
//!
//! With the gene on, a city threatened on any turn of the current major war
//! keeps counting as threatened for [`WAR_THREAT_HOLD_TURNS`] standard turns
//! (5 on Online). The `recovery-needs-the-deficit` gate still applies to it,
//! so an army twice the opponent's still leaves Recovery. Peace forgets the
//! threat. A war whose threats stop for the window releases the plan exactly
//! as before. Off, nothing moves.

use super::*;

/// Standard turns a threatened city keeps holding Recovery after it was last
/// seen threatened. The reviewed flips came 1-3 Online turns apart; this is 5.
pub(super) const WAR_THREAT_HOLD_TURNS: u32 = 10;

/// Whether a threat last seen on `seen` still holds on `turn`, for a window
/// of `window` game turns.
pub(super) fn threat_still_held(seen: Option<u32>, turn: u32, window: u32) -> bool {
    seen.is_some_and(|seen| turn.saturating_sub(seen) < window.max(1))
}

impl AdvancedAi {
    /// See the module: stamp the turn a city was threatened during a major
    /// war, and forget it at peace. Exact no-op with the gene off.
    pub(super) fn observe_war_threat(&mut self, g: &Game, pid: usize, major_war: bool) {
        if !self.war_threat_holds_recovery || !major_war {
            self.war_threat_seen = None;
            return;
        }
        if self.threatened_city(g, pid).is_some() {
            self.war_threat_seen = Some(g.turn);
        }
    }

    /// See the module: whether the war's last threatened turn still holds
    /// Recovery. `live` is whether a city is threatened this assessment; the
    /// journal names only the turns the hold, not a live threat, carries.
    /// `false` with the gene off.
    pub(super) fn war_threat_holds(&self, g: &Game, live: bool) -> bool {
        if !self.war_threat_holds_recovery {
            return false;
        }
        let window = g.standard_duration(WAR_THREAT_HOLD_TURNS);
        let held = threat_still_held(self.war_threat_seen, g.turn, window);
        if held && !live {
            let seen = self.war_threat_seen.unwrap_or(g.turn);
            think!(self.journal(), Strategy, Detail,
                "A threat from turn {seen} still holds the defence";
                "war-threat-holds-recovery: no city reads threatened this turn, but one did \
                 {} turn(s) ago in this war; the plan stays home for {window} turns after the \
                 last threat", g.turn.saturating_sub(seen));
        }
        held
    }
}

#[cfg(test)]
mod tests;
