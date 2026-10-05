//! `runaway-expander-counter`: the rival that is simply outgrowing everyone
//! reads as a Domination counter clock before any victory lane does.
//!
//! Live King civvis-20261003T131343Z (game 37) lost on Technology at turn
//! 238 to a rival holding fourteen or fifteen cities and 171 to 203
//! population from turn 170, against our eleven and 82 to 94; its science
//! per citizen rose from 1.1 to 2.0 and the campaign never touched it. In
//! civvis-20261003T164758Z (game 43) the Aztecs snowballed to eighteen
//! cities and 402 science. A lane clock reads such a rival only once its
//! launches or tourists are on the board, about sixty turns too late
//! (proposed by -ff).

use super::victory_heuristics::DOMINATION_SCIENCE_COUNTER;
use super::{AdvancedAi, GrandStrategy, VictoryFocus, VictoryTarget};
use crate::game::Game;

/// Standard turns before a size lead reads as a runaway: early empires
/// grow by founding, and the counter is a mid-game reading.
pub(super) const RUNAWAY_TURN: u32 = 120;
/// The rival's cities against ours.
pub(super) const RUNAWAY_CITY_RATIO: f64 = 1.3;
/// The rival's population against ours.
pub(super) const RUNAWAY_POP_RATIO: f64 = 1.8;

impl AdvancedAi {
    /// The counter clock `rival` reads as a runaway expander: Science at the
    /// Domination counter's bar, so it is answered with war and outranked
    /// by every clock of a real lane already past that bar. Urgency and the
    /// one-war gates read the rival's own pressure, so this only chooses
    /// which rival the campaign aims at when nothing nearer to winning does.
    pub(super) fn runaway_expander_clock(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> Option<VictoryFocus> {
        if !self.runaway_expander_counter
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || g.turn < g.standard_duration(RUNAWAY_TURN)
            || rival == pid
        {
            return None;
        }
        let player = g.players.get(rival)?;
        if !player.alive || player.is_minor || player.is_barbarian {
            return None;
        }
        let (cities, pop) = Self::public_size(g, rival);
        let (our_cities, our_pop) = Self::public_size(g, pid);
        (cities as f64 >= RUNAWAY_CITY_RATIO * our_cities.max(1) as f64
            && pop as f64 >= RUNAWAY_POP_RATIO * our_pop.max(1) as f64)
            .then_some(VictoryFocus {
                strategy: GrandStrategy::Science,
                progress: DOMINATION_SCIENCE_COUNTER,
            })
    }

    /// Cities and population: the host's public record where the mirror has
    /// it (it counts cities in the fog), else the cities on the board.
    fn public_size(g: &Game, pid: usize) -> (usize, i64) {
        let stats = g.observed_public_empire_stats.get(&pid);
        let board = g.player_city_ids(pid);
        let cities = stats.and_then(|s| s.city_count).unwrap_or(board.len());
        let pop = stats
            .and_then(|s| s.population)
            .map(i64::from)
            .unwrap_or_else(|| {
                board
                    .iter()
                    .map(|cid| i64::from(g.cities[cid].pop.max(0)))
                    .sum()
            });
        (cities, pop)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AdvancedAi, GrandStrategy, VictoryTarget};
    use crate::game::Game;

    fn board() -> Game {
        let mut g = Game::new_full(3, 40, 24, 51_120, 400, 0, false);
        for unit in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(unit);
        }
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
        g.found_city_for(0, (6, 8), None);
        g.found_city_for(0, (6, 14), None);
        for (y, pop) in [(4, 12), (8, 12), (12, 12), (16, 12)] {
            let cid = g.found_city_for(1, (22, y), None);
            g.cities.get_mut(&cid).unwrap().pop = pop;
        }
        g.found_city_for(2, (34, 10), None);
        for cid in g.player_city_ids(0) {
            g.cities.get_mut(&cid).unwrap().pop = 6;
        }
        for pid in 0..3 {
            for other in 0..3 {
                if pid != other {
                    g.record_contact(pid, other);
                }
            }
        }
        g.current = 0;
        g.turn = g.standard_duration(super::RUNAWAY_TURN);
        g
    }

    #[test]
    fn the_rival_outgrowing_everyone_is_a_counter_target() {
        let g = board();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.deny_leaders = true;
        ai.battlefront_observation = true;
        assert!(
            ai.runaway_expander_clock(&g, 0, 1).is_none(),
            "off by default"
        );
        ai.enable_runaway_expander_counter();
        assert!(
            ai.runaway_expander_clock(&g, 0, 1).is_some(),
            "four cities, 48 pop vs two, 12"
        );
        assert!(
            ai.runaway_expander_clock(&g, 0, 2).is_none(),
            "one city is no runaway"
        );
        assert_eq!(
            ai.actionable_victory_denial(&g, 0),
            Some((1, GrandStrategy::Conquest))
        );
        // Before the mid-game it reads nothing.
        let mut early = board();
        early.turn = early.standard_duration(super::RUNAWAY_TURN) - 1;
        assert!(ai.runaway_expander_clock(&early, 0, 1).is_none());
    }
}
