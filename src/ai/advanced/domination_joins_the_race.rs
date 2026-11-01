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

    /// `lane-joins-the-race`: the same parity entry for a Culture or Religion
    /// seat. A Diplomacy seat keeps its own rule (`SCIENCE_ANSWER_DIPLO_YIELD`).
    pub(super) fn lane_answers_at_parity(&self, standing: &ScienceStanding) -> bool {
        self.lane_joins_the_race
            && matches!(
                self.victory_target,
                Some(VictoryTarget::Culture | VictoryTarget::Religion)
            )
            && standing.own_techs + SCIENCE_DRIVE_TECH_SLACK >= standing.best_rival_techs
    }

    /// Whether the Domination exclusions of the space race hold this turn:
    /// a Domination seat, unless `domination-joins-the-race` has its answer
    /// driving. Identical to `victory_target == Some(Domination)` with the
    /// gene off.
    pub(super) fn space_race_excluded(&self) -> bool {
        self.victory_target == Some(VictoryTarget::Domination)
            && !(self.domination_joins_the_race
                && self.science_drive.is_some_and(|drive| drive.answering))
    }
}

#[cfg(test)]
mod tests {
    use super::super::science_victory_drive::ScienceDrive;
    use super::*;

    #[test]
    fn domination_joins_the_race_is_a_native_opt_in_off_in_both_controllers() {
        super::super::test_support::opt_in_off_in_both_controllers(
            "domination-joins-the-race",
            |ai| ai.domination_joins_the_race,
        );
    }

    fn standing(own_techs: usize, best_rival_techs: usize) -> ScienceStanding {
        ScienceStanding {
            own_science: 300.0,
            best_rival_science: 400.0,
            own_techs,
            best_rival_techs,
        }
    }

    /// 073004Z's readings: 62 to 67 at t210 stays out, 67 to 70 at t220
    /// opens; 060258Z's 65 to 77 never does. Only a Domination seat, and only
    /// with the gene.
    #[test]
    fn parity_opens_the_answer_for_a_domination_seat_only() {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        assert!(!ai.domination_answers_at_parity(&standing(67, 70)));
        ai.enable_domination_joins_the_race();
        assert!(!ai.domination_answers_at_parity(&standing(62, 67)));
        assert!(ai.domination_answers_at_parity(&standing(67, 70)));
        assert!(!ai.domination_answers_at_parity(&standing(65, 77)));

        let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
        culture.enable_domination_joins_the_race();
        assert!(!culture.domination_answers_at_parity(&standing(70, 70)));
    }

    /// `lane-joins-the-race`: Culture and Religion seats at parity, with the
    /// gene; 135422Z's 67 to 69 opens, 112906Z's 63 to 70 does not. Neither
    /// Domination nor Diplomacy reads it.
    #[test]
    fn lane_parity_opens_the_answer_for_culture_and_religion_only() {
        let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
        assert!(!culture.lane_answers_at_parity(&standing(67, 69)));
        culture.enable_lane_joins_the_race();
        assert!(culture.lane_answers_at_parity(&standing(67, 69)));
        assert!(!culture.lane_answers_at_parity(&standing(63, 70)));

        let mut religion = AdvancedAi::targeting(VictoryTarget::Religion);
        religion.enable_lane_joins_the_race();
        assert!(religion.lane_answers_at_parity(&standing(70, 72)));

        for target in [VictoryTarget::Domination, VictoryTarget::Diplomacy] {
            let mut ai = AdvancedAi::targeting(target);
            ai.enable_lane_joins_the_race();
            assert!(!ai.lane_answers_at_parity(&standing(70, 70)), "{target:?}");
        }
    }

    #[test]
    fn lane_joins_the_race_is_a_native_opt_in_off_in_both_controllers() {
        super::super::test_support::opt_in_off_in_both_controllers("lane-joins-the-race", |ai| {
            ai.lane_joins_the_race
        });
    }

    /// The exclusions hold for a Domination seat until its answer drives, and
    /// never apply to another lane.
    #[test]
    fn the_exclusions_lift_only_while_the_answer_drives() {
        let drive = |answering| ScienceDrive {
            since: 220,
            reviewed: 220,
            standing: standing(67, 70),
            launch_city: None,
            assigned: false,
            answering,
        };
        let mut stock = AdvancedAi::targeting(VictoryTarget::Domination);
        stock.science_drive = Some(drive(true));
        assert!(
            stock.space_race_excluded(),
            "the gene off keeps the exclusion"
        );

        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_domination_joins_the_race();
        assert!(ai.space_race_excluded(), "no drive, no race");
        ai.science_drive = Some(drive(false));
        assert!(ai.space_race_excluded(), "a drive that is not the answer");
        ai.science_drive = Some(drive(true));
        assert!(
            !ai.space_race_excluded(),
            "the answer drives: the race opens"
        );

        let mut science = AdvancedAi::targeting(VictoryTarget::Science);
        science.enable_domination_joins_the_race();
        assert!(!science.space_race_excluded());
    }
}
