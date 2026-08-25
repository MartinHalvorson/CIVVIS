//! `growth-to-settle`: while the empire is behind the settling pace and no
//! city can yet build a Settler, the citizens work food.
//!
//! ## The constraint this is about
//!
//! A Settler costs a population point and the engine refuses to start one in a
//! city below population two (`Game::can_produce`, and the production gate's
//! own `city.pop >= 2`). The controller's note beside the Settler's lane
//! multiplier records what that costs: the Settler
//! *"is not blocked by losing a ranking, it is blocked before the ranking is
//! consulted — **'no city at pop 2' on 23.8% of seat-turns**, a growth
//! constraint no price can buy past."*
//!
//! That is the other half of the defect [`super::expansion_schedule`]
//! addresses. Opening the pipeline is worth nothing in a turn where no city
//! is allowed to use it, and the recorded games say those turns decide the
//! game: over 218 completed live runs, **every win came from an empire with
//! four to six cities at turn 60** and the median loss loses the lead for
//! good at turn 61.
//!
//! ## What the gene does
//!
//! `Game::citizen_strategy` weighs a city's tiles from local evidence and one
//! empire scalar, [`crate::game::Player::citizen_food_bias`], which the engine
//! itself never writes — its own doc says *"an agent does, which is the
//! point: citizen assignment is the one city-level decision no player, human
//! or AI, can currently express."* Nothing in this controller writes it.
//!
//! While **all three** of these hold, the gene writes
//! [`GROWTH_TO_SETTLE_BIAS`] into it, and zero the moment any stops:
//!
//! 1. the game is still inside the opening the band is measured on
//!    ([`AdvancedAi::expansion_band_turn`]);
//! 2. the empire is **behind** the pace that wins
//!    ([`AdvancedAi::expansion_pace`]), counting walkers already on their way;
//! 3. some city of ours is **below the Settler's population floor**, so growth
//!    is the thing actually missing.
//!
//! ⚠⚠ **Why it is scoped this narrowly.** The broad version of this idea has
//! already been screened and lost: `city_strategy`, which stamps a
//! `CityDirective` on every city every turn, measured **42.5% paired over 120
//! maps, Elo-equivalent −53, sign p=0.0014 against**, and its own note records
//! the mechanism — *"the treatment simply built a uniformly smaller empire"*.
//! A standing food appetite trades away the production that builds the very
//! Settlers it is meant to enable. This gene therefore holds the appetite only
//! while the empire is demonstrably behind and demonstrably unable to build,
//! and drops it the moment either stops being true — it can only ever be on
//! for part of one opening.
//!
//! Byte-identical when off: the field is never written at all.

use super::AdvancedAi;
use crate::game::Game;

/// Extra food appetite while the opening is behind and blocked, added to the
/// governor's shipped weight of 1.25. Deliberately below the 1.15 production
/// bump an in-progress Settler already applies, so a city that *can* build one
/// still builds it rather than growing instead.
pub const GROWTH_TO_SETTLE_BIAS: f64 = 0.75;

/// The population a city needs before it may start a Settler.
pub const SETTLER_POPULATION_FLOOR: i64 = 2;

impl AdvancedAi {
    /// The food appetite the opening asks for this turn: the bias while the
    /// empire is behind the pace with no city able to build, zero otherwise.
    pub(super) fn growth_to_settle_bias(&self, g: &Game, pid: usize) -> f64 {
        if !self.growth_to_settle || g.turn > Self::expansion_band_turn(g) {
            return 0.0;
        }
        let city_ids = g.player_city_ids(pid);
        let settlers = g
            .units
            .values()
            .filter(|unit| unit.owner == pid && unit.kind == "settler")
            .count();
        if city_ids.len() + settlers >= Self::expansion_pace(g) {
            return 0.0;
        }
        let below_floor = city_ids
            .iter()
            .any(|cid| (g.cities[cid].pop as i64) < SETTLER_POPULATION_FLOOR);
        if below_floor {
            GROWTH_TO_SETTLE_BIAS
        } else {
            0.0
        }
    }

    /// Write this turn's appetite. An exact no-op while the gene is off — the
    /// engine's own field is never touched, so a withheld arm is byte-identical.
    pub(super) fn maintain_growth_to_settle(&self, g: &mut Game, pid: usize) {
        if !self.growth_to_settle {
            return;
        }
        let want = self.growth_to_settle_bias(g, pid);
        if g.players[pid].citizen_food_bias != want {
            g.players[pid].citizen_food_bias = want;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{Action, Game};
    use crate::setup::GameSpeed;

    fn board() -> (Game, usize) {
        let mut g = Game::new(2, 24, 16, 71, 250, 0);
        g.game_speed = GameSpeed::Online;
        let settler = g
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.current = 0;
        g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        (g, 0)
    }

    #[test]
    fn off_by_default_and_toggles() {
        let ai = AdvancedAi::new();
        assert!(!ai.growth_to_settle, "an opt-in ships off");
        assert!(!AdvancedAi::legacy().growth_to_settle);
        let mut ai = AdvancedAi::new();
        ai.enable_growth_to_settle();
        assert!(ai.growth_to_settle);
        ai.disable_growth_to_settle();
        assert!(!ai.growth_to_settle);
    }

    #[test]
    fn a_blocked_opening_behind_the_pace_works_food() {
        let (mut g, pid) = board();
        g.turn = 40;
        let city = g.player_city_ids(pid)[0];
        g.cities.get_mut(&city).unwrap().pop = 1;
        let mut ai = AdvancedAi::new();
        ai.enable_growth_to_settle();
        // One city, no walker, at t40: the pace wants three.
        assert_eq!(ai.growth_to_settle_bias(&g, pid), GROWTH_TO_SETTLE_BIAS);
        ai.maintain_growth_to_settle(&mut g, pid);
        assert_eq!(g.players[pid].citizen_food_bias, GROWTH_TO_SETTLE_BIAS);
    }

    #[test]
    fn a_city_that_can_already_build_a_settler_is_left_alone() {
        let (mut g, pid) = board();
        g.turn = 40;
        let city = g.player_city_ids(pid)[0];
        g.cities.get_mut(&city).unwrap().pop = SETTLER_POPULATION_FLOOR as i32;
        let mut ai = AdvancedAi::new();
        ai.enable_growth_to_settle();
        assert_eq!(
            ai.growth_to_settle_bias(&g, pid),
            0.0,
            "behind the pace, but growth is not what is missing"
        );
    }

    #[test]
    fn an_opening_on_pace_is_left_alone() {
        let (mut g, pid) = board();
        g.turn = 5;
        let city = g.player_city_ids(pid)[0];
        g.cities.get_mut(&city).unwrap().pop = 1;
        let mut ai = AdvancedAi::new();
        ai.enable_growth_to_settle();
        // The pace wants one city at t5 and the empire holds one.
        assert_eq!(ai.growth_to_settle_bias(&g, pid), 0.0);
    }

    #[test]
    fn the_appetite_is_dropped_once_the_opening_is_over() {
        let (mut g, pid) = board();
        let city = g.player_city_ids(pid)[0];
        g.cities.get_mut(&city).unwrap().pop = 1;
        let mut ai = AdvancedAi::new();
        ai.enable_growth_to_settle();
        g.turn = 40;
        ai.maintain_growth_to_settle(&mut g, pid);
        assert_eq!(g.players[pid].citizen_food_bias, GROWTH_TO_SETTLE_BIAS);
        g.turn = 61;
        ai.maintain_growth_to_settle(&mut g, pid);
        assert_eq!(
            g.players[pid].citizen_food_bias, 0.0,
            "past the band the appetite is given back"
        );
    }

    #[test]
    fn the_gene_off_never_writes_the_engines_field() {
        let (mut g, pid) = board();
        g.turn = 40;
        let city = g.player_city_ids(pid)[0];
        g.cities.get_mut(&city).unwrap().pop = 1;
        g.players[pid].citizen_food_bias = 0.125;
        let ai = AdvancedAi::new();
        assert_eq!(ai.growth_to_settle_bias(&g, pid), 0.0);
        ai.maintain_growth_to_settle(&mut g, pid);
        assert_eq!(
            g.players[pid].citizen_food_bias, 0.125,
            "a withheld arm leaves the field exactly as it found it"
        );
    }
}
