//! `religion-lane-outlasts-the-denial`: an assigned Religion lane that has
//! founded its religion is not displaced by a Science counter to a rival's
//! science race.
//!
//! With `denial-outranks-expansion` armed, `assess` answers a rival close to
//! winning ahead of the lane ("countering a rival close to winning, ahead of
//! the lane"). Against a science race the counter it picks can be Science:
//! the empire's whole plan turns to a space race of its own. That does not
//! deny the rival's launches. Only a war on the launcher or the spies do,
//! and both keep running without the plan, the spies from the denial's own
//! readers. The shipped chain already says so for a Culture target (Native
//! Pericles 141453Z answered a launch clock with its own Spaceport); this
//! gene says it for a Religion target that holds its religion.
//!
//! The Indefinite Benchmarking Tourney's reviewer named it in 4 of 40 Lv 8
//! games on 2026-11-26 (`grand-strategy-flips-off-leading-religion-lane`):
//! - 083455Z t155, 141906Z t159, 114544Z t150, 091342Z t164: "Grand
//!   strategy: science | countering a rival close to winning, ahead of the
//!   lane", each with "its science race reads 84%";
//! - in the first three the spreaders were one majority short ("we know too
//!   few cities of 1 more to hold their majorities");
//! - our science was 82-169 a turn against 565-612, 41-60 techs against
//!   76-77; 141906Z ordered a Spaceport at t171 that never completed;
//! - every one of the four won on Religion anyway, at t167-t184, with the
//!   plan off its lane for 7-29 turns.
//!
//! A Conquest counter is untouched: a launcher war the army can win
//! (091342Z t170, 997 power against Ethiopia's 674, three projects landed)
//! is a real denial. Before founding the lane reads as before. Off, nothing
//! moves.

use super::*;

impl AdvancedAi {
    /// See the module: an assigned Religion lane holding its religion, asked
    /// to answer a rival with a Science counter. False with the gene off,
    /// for any other lane or counter, or before founding.
    pub(super) fn religion_lane_outlasts_the_denial_for(
        &self,
        g: &Game,
        pid: usize,
        target: Option<VictoryTarget>,
        counter: GrandStrategy,
    ) -> bool {
        let outlasts = self.religion_lane_outlasts_the_denial
            && target == Some(VictoryTarget::Religion)
            && counter == GrandStrategy::Science
            && g.players[pid].religion.is_some();
        if outlasts {
            let (converted, living) = self.religious_conversion_tally(g, pid);
            think!(self.journal(), Strategy, Detail,
                "Religion lane keeps the plan against a science race";
                "religion-lane-outlasts-the-denial: racing a rival's launches does not stop them; \
                 {converted} of {living} living majors follow our religion");
        }
        outlasts
    }
}

#[cfg(test)]
mod tests;
