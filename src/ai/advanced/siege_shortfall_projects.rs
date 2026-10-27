//! `siege-shortfall-claims-the-projects`: while a siege train holds because
//! its whole force is short of the bill, the city queues running a repeatable
//! district project build land soldiers instead.
//!
//! `redirect_repeatable_projects_for_force_gap` already pauses repeatable
//! economic projects for a land-force gap, but it reads the gap as fewer land
//! units than twice our cities, and only on a Conquest or Recovery plan. A
//! Domination siege can be short of its bill by a whole army while the empire
//! fields its two units a city. Retained live Immortal games to 2026-10-27:
//!
//! - 1,248 capital siege-turns in 67 games: 89% in Stage, the strength within
//!   ten tiles a median 0.09 of the bill, no unit staged on 49% of them; "the
//!   mustered train holds" was the most common journal line (861 turns).
//! - Of 61 games reaching turn 120, 8 ever held a rival's original capital
//!   and none held more than two.
//! - At war, turns 90-160, Theater Square and Campus projects took 4.8% of our
//!   Production, as much as the Bombards (4.2%); every melee unit able to take
//!   a city together took about 6.4%.
//!
//! Under the gene the siege train notes the turn a Stage train holds with the
//! whole force's strength under its bill (`siege_train`, the
//! `stage-musters-out-of-reach` readiness); for the next turn, on a
//! Domination seat, the force-gap redirect runs on any plan while that city's
//! owner is at war with us, and
//! asks [`SIEGE_SHORTFALL_LAND_PER_CITY`] land units a city. Only repeatable
//! projects are redirected, so buildings, districts, wonders and one-off
//! projects keep their queues, and the project's progress stays banked.
//!
//! Off: unchanged.

use super::*;

/// Land units a city the force-gap redirect asks while a siege is short:
/// one more than the ordinary gap's two.
pub(crate) const SIEGE_SHORTFALL_LAND_PER_CITY: usize = 3;

/// Turns a short siege reading keeps the redirect open.
const SIEGE_SHORTFALL_FRESH_TURNS: u32 = 1;

impl AdvancedAi {
    /// `siege-shortfall-claims-the-projects`: note that the siege train on
    /// `cid` held this turn with its force's `strength` under `bill`, or
    /// clear the note when it did not. No-op with the gene off.
    pub(super) fn note_siege_shortfall(&mut self, cid: u32, turn: u32, short: bool) {
        if !self.siege_shortfall_claims_the_projects {
            return;
        }
        if short {
            self.siege_force_short.insert(cid, turn);
        } else {
            self.siege_force_short.remove(&cid);
        }
    }

    /// Whether, on a Domination seat, a siege train held short of its bill
    /// within the last [`SIEGE_SHORTFALL_FRESH_TURNS`] on a city whose owner
    /// we are still at war with. Always `false` with the gene off.
    pub(super) fn siege_force_is_short(&self, g: &Game, pid: usize) -> bool {
        self.siege_shortfall_claims_the_projects
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && self.siege_force_short.iter().any(|(cid, turn)| {
                g.turn.saturating_sub(*turn) <= SIEGE_SHORTFALL_FRESH_TURNS
                    && g.cities
                        .get(cid)
                        .is_some_and(|city| city.owner != pid && g.is_at_war(pid, city.owner))
            })
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
    use crate::game::{Game, Item};

    #[test]
    fn siege_shortfall_claims_the_projects_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("siege-shortfall-claims-the-projects", |ai| {
            ai.siege_shortfall_claims_the_projects
        });
    }

    /// Two majors at war, each with a capital; seat 0's capital runs a
    /// repeatable project and seat 0 fields two land units for its one city.
    fn at_war_with_a_project() -> (Game, u32, u32) {
        let mut game = Game::new_full(2, 24, 16, 4_181, 300, 0, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .expect("every major starts with a settler");
            game.found_city_for(pid, game.units[&settler].pos, None);
            game.remove_unit(settler);
        }
        game.record_contact(0, 1);
        game.at_war.insert((0, 1));
        let ours = game.player_city_ids(0)[0];
        let theirs = game.player_city_ids(1)[0];
        let project = game
            .rules
            .projects
            .iter()
            .find(|(_, spec)| spec.repeatable && !spec.ongoing_yields.is_empty())
            .map(|(name, _)| name.clone())
            .expect("the rules carry a repeatable yield project");
        game.cities.get_mut(&ours).unwrap().queue = vec![Item::Project { project }];
        game.turn = 90;
        (game, ours, theirs)
    }

    fn science_plan(turn: u32) -> StrategicPlan {
        StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: Some(1),
            target_city: None,
            threatened_city: None,
            desired_cities: 3,
            assessed_turn: turn,
            rush: false,
        }
    }

    fn queues_a_project(game: &Game, cid: u32) -> bool {
        matches!(game.cities[&cid].queue.first(), Some(Item::Project { .. }))
    }

    #[test]
    fn a_short_siege_turns_the_project_queue_to_a_land_unit() {
        let (mut game, ours, theirs) = at_war_with_a_project();
        let mut ai = AdvancedAi::new();
        ai.victory_target = Some(VictoryTarget::Domination);
        let plan = science_plan(game.turn);

        // Off, a Science plan never reaches the force-gap redirect.
        ai.note_siege_shortfall(theirs, game.turn, true);
        let mut off = game.clone();
        ai.redirect_repeatable_projects_for_force_gap(&mut off, 0, &plan);
        assert!(
            queues_a_project(&off, ours),
            "off, the project keeps its queue"
        );

        ai.enable_siege_shortfall_claims_the_projects();
        ai.note_siege_shortfall(theirs, game.turn, true);
        assert!(ai.siege_force_is_short(&game, 0));

        // A seat on another victory lane is left to its own plan.
        ai.victory_target = Some(VictoryTarget::Science);
        assert!(!ai.siege_force_is_short(&game, 0));
        ai.victory_target = Some(VictoryTarget::Domination);

        ai.redirect_repeatable_projects_for_force_gap(&mut game, 0, &plan);
        assert!(
            !queues_a_project(&game, ours),
            "the short siege claims the project queue: {:?}",
            game.cities[&ours].queue
        );
    }

    #[test]
    fn the_shortfall_lapses_with_the_reading_and_the_war() {
        let (mut game, _, theirs) = at_war_with_a_project();
        let mut ai = AdvancedAi::new();
        ai.victory_target = Some(VictoryTarget::Domination);
        ai.enable_siege_shortfall_claims_the_projects();
        ai.note_siege_shortfall(theirs, game.turn, true);
        assert!(ai.siege_force_is_short(&game, 0));

        // A stale reading no longer counts.
        let mut later = game.clone();
        later.turn += 2;
        assert!(!ai.siege_force_is_short(&later, 0));

        // Nor does one on a city we are no longer at war with.
        game.at_war.clear();
        assert!(!ai.siege_force_is_short(&game, 0));

        // A train that holds with its bill met clears the note.
        game.at_war.insert((0, 1));
        ai.note_siege_shortfall(theirs, game.turn, false);
        assert!(!ai.siege_force_is_short(&game, 0));
    }
}
