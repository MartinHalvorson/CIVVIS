//! `domination-joins-the-race`: a Domination seat at tech parity answers a
//! rival's space race with its own, beside its war.
//!
//! Every King game of 2026-11-01 ended in a Science victory, and a Domination
//! seat could not take part: the delegated governor ran with
//! `exclude_space_race`, the scorer vetoed every Spaceport, and the endgame
//! schedule refused the seat. `science-drive-answers-the-launch` already lets
//! another lane's seat answer a rival's launch, but only from an outright
//! science lead, and `science-drive-answers-from-the-pad` only once our own
//! pad has landed a project, which a Domination seat never builds. Live King
//! civvis-20261101T073004Z stood 67 techs to the leader's 70 at t220, reached
//! the whole tree at t250, and lost to Korea's expedition at t257 without a
//! pad.
//!
//! With the gene, a Domination seat whose techs are within
//! [`SCIENCE_DRIVE_TECH_SLACK`] of the field's best opens the answer (the bar
//! a driving seat already holds to), and while that answer drives, the
//! Domination exclusions lift. The war plan is not touched.
use super::science_victory_drive::{ScienceStanding, SCIENCE_DRIVE_TECH_SLACK};
use super::{AdvancedAi, VictoryTarget};

impl AdvancedAi {
    /// Whether a Domination seat's answer may open on this reading: the gene
    /// is on, the seat races Domination, and its techs are within
    /// [`SCIENCE_DRIVE_TECH_SLACK`] of the best living rival's. The caller
    /// has already established that a rival has landed a space project.
    pub(super) fn domination_answers_at_parity(&self, standing: &ScienceStanding) -> bool {
        self.domination_joins_the_race
            && self.victory_target == Some(VictoryTarget::Domination)
            && standing.own_techs + SCIENCE_DRIVE_TECH_SLACK >= standing.best_rival_techs
    }

    /// An explicit Culture target spends production on its Culture finish,
    /// including when an idle Scientist asks for a Spaceport. Domination
    /// keeps its existing exclusion unless its opted-in answer is driving.
    pub(super) fn space_race_excluded(&self) -> bool {
        self.victory_target == Some(VictoryTarget::Culture)
            || (self.victory_target == Some(VictoryTarget::Domination)
                && !(self.domination_joins_the_race
                    && self.science_drive.is_some_and(|drive| drive.answering)))
    }
}

#[cfg(test)]
mod tests;
