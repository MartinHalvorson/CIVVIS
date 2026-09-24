//! `lane-delegates-production`: until the development half ends, an assigned
//! lane's cities take the unassigned seat's production dispatch.
//!
//! ★★★★ THE LANE CHANGES WHO BUILDS, NOT ONLY WHAT IS WANTED. The turn
//! driver hands an assigned lane's every city to the strategic scorer
//! (`advanced_production`). An unassigned seat runs that scorer only in
//! Recovery, under `expansion_dispatch`, or for an appointed war, and then
//! gives every remaining queue to `advanced_support_production` and the
//! baseline city governor (`delegated_cities`), which also spends its Gold.
//! Both seats spend the first half in the same posture — Expansion 91% of
//! turns 1–125 for a Science seat and 86% for an unassigned one on the ladder
//! proxy's King shape, both aiming at ten cities — so the difference is the
//! manager, and the census reads it (turn 100, 32 paired King games):
//!
//! | | Science lane | unassigned |
//! |---|---:|---:|
//! | Settlers / military, share of Production | 18% / 14% | 11% / 10% |
//! | Granary | 3.4 | 4.2 |
//! | Market / Commercial Hub | 0.6 / 1.3 | 1.4 / 2.3 |
//! | trade route capacity | 1.8 | 3.0 |
//! | Industrial Zone | 1.5 | 0.0 |
//! | score share at the end | 14.9% | 17.7% |
//!
//! The lane target alone cost 2.3–3.0 points of score share (z −2.8 without
//! the live seat's forced list, −4.1 with it); the forced list itself was
//! neutral on both seats. With this gene the Science seat's first half takes
//! the unassigned dispatch and it gained **+1.39 pp (z +2.86)** on the same
//! seeds: Settlers 10% and military 9% of Production, Granary 5.1, Market
//! 2.5, Commercial Hub 3.2, trade capacity 4.4.
//!
//! The lane keeps everything that is the lane's: the plan still follows it,
//! its reservations and victory purchases still run ahead of the routine
//! governors, and from the specialization clock on (`phase_specialization_
//! active`, the halfway mark) its cities return to the strategic scorer that
//! prices the victory's own districts and projects. An appointed war and
//! Recovery keep the scorer throughout, exactly as they do for an unassigned
//! seat. With the gene off the dispatch is unchanged, byte for byte.
use super::{AdvancedAi, VictoryTarget};

impl AdvancedAi {
    /// Whether this turn an assigned lane's cities take the unassigned seat's
    /// production dispatch: the gene is on, a lane is assigned, and the
    /// development half has not ended.
    pub(super) fn lane_delegates_now(
        &self,
        active_victory_target: Option<VictoryTarget>,
        specialization_active: bool,
    ) -> bool {
        self.lane_delegates_production && active_victory_target.is_some() && !specialization_active
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{run_game, Ai};
    use crate::game::{Game, GameOptions};

    #[test]
    fn only_an_assigned_lane_delegates_and_only_before_specialization() {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        assert!(
            !ai.lane_delegates_now(Some(VictoryTarget::Science), false),
            "off by default"
        );
        ai.enable_lane_delegates_production();
        assert!(ai.lane_delegates_now(Some(VictoryTarget::Science), false));
        assert!(
            !ai.lane_delegates_now(Some(VictoryTarget::Science), true),
            "the specialized half keeps the lane's own scorer"
        );
        assert!(
            !ai.lane_delegates_now(None, false),
            "an unassigned seat already takes this dispatch"
        );
    }

    fn played(target: Option<VictoryTarget>, gene: bool) -> String {
        let mut game = Game::new_with(GameOptions::new(3, 32, 20, 92_409, 40, 1));
        let mut ais: Vec<AdvancedAi> = game
            .players
            .iter()
            .map(|_| {
                let mut ai = match target {
                    Some(target) => AdvancedAi::targeting(target),
                    None => AdvancedAi::new(),
                };
                if gene {
                    ai.enable_lane_delegates_production();
                }
                ai
            })
            .collect();
        assert!(!ais[0].uses_player_observation());
        run_game(&mut game, &mut ais);
        serde_json::to_string(&game.log).expect("the action log serializes")
    }

    /// Pinned by the whole decision stream: the gene changes what an
    /// assigned lane builds and nothing an unassigned seat does.
    #[test]
    fn the_gene_moves_an_assigned_lane_and_leaves_an_unassigned_seat_alone() {
        assert_ne!(
            played(Some(VictoryTarget::Science), true),
            played(Some(VictoryTarget::Science), false),
            "an assigned lane's first-half queues change hands"
        );
        assert_eq!(
            played(None, true),
            played(None, false),
            "an unassigned seat already takes the dispatch"
        );
    }
}
