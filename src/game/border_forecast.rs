//! Border-growth forecast: which plot a city's culture claims next, and when.
//!
//! `Game::expand_borders` is the engine's cultural tile picker and it is
//! private on purpose — nothing outside the turn loop may claim ground.
//! What a planner deciding whether to *buy* a plot needs is the other
//! question: would culture hand this plot over for free anyway, and how
//! soon? Paying Gold for the plot the border takes next turn buys one turn
//! of ownership; paying for a plot the picker would pass over for twenty
//! turns buys twenty. This module answers both halves from the same
//! influence costs and the same culture curve the engine spends, so the
//! forecast cannot drift from the pick.

use super::Game;
use crate::Pos;

impl Game {
    /// The plots the next cultural expansion of `cid` would draw from: every
    /// unclaimed neighbour of the city's territory within the engine's
    /// five-ring reach that ties for the lowest influence cost. The engine
    /// picks uniformly among these, so a plot outside the set is not the
    /// next one whatever the dice say. Empty when the borders cannot grow.
    pub fn border_growth_front(&self, cid: u32) -> Vec<Pos> {
        let Some(city) = self.cities.get(&cid) else {
            return Vec::new();
        };
        let city_pos = city.pos;
        let mut candidates = std::collections::BTreeSet::new();
        for pos in &city.owned_tiles {
            for neighbor in self.nbrs(*pos) {
                let Some(tile) = self.map.get(neighbor) else {
                    continue;
                };
                if tile.owner_city.is_some() || self.wdist(neighbor, city_pos) > 5 {
                    continue;
                }
                candidates.insert(neighbor);
            }
        }
        let scored: Vec<(Pos, f64)> = candidates
            .into_iter()
            .map(|position| (position, self.border_influence_cost(city_pos, position)))
            .collect();
        let Some(best) = scored
            .iter()
            .map(|(_, score)| *score)
            .min_by(f64::total_cmp)
        else {
            return Vec::new();
        };
        scored
            .into_iter()
            .filter(|(_, score)| *score == best)
            .map(|(position, _)| position)
            .collect()
    }

    /// Turns until `cid`'s borders next grow on their own, at the city's
    /// current Culture: the shipped plot curve (`10 + 6 × plots^1.3`, the
    /// first seven plots free) less the culture already banked, divided by
    /// the per-turn border culture. `None` when the borders are not growing
    /// at all — no Culture, a seat that cannot annex, or a Border Control
    /// treaty in force — so a caller cannot mistake "never" for "soon".
    pub fn border_growth_turns(&self, cid: u32) -> Option<f64> {
        let city = self.cities.get(&cid)?;
        let pid = city.owner;
        if !self.annexes_tiles_with_own_yields(pid)
            || self.congress_effect_active("border_control_treaty", "B", &pid.to_string())
        {
            return None;
        }
        let border_mult = 1.0
            + (self.pantheon_effect(pid, "border_growth_pct")
                + self.governor_effect(pid, cid, "border_growth_pct"))
                / 100.0;
        let per_turn = self.city_yields(cid).culture * border_mult;
        if per_turn <= 0.0 {
            return None;
        }
        let plots = (city.owned_tiles.len() as i32 - 7).max(0) as f64;
        let need = (10.0 + 6.0 * plots.powf(1.3)).trunc();
        let remaining = (need - city.border_culture).max(0.0);
        Some((remaining / per_turn).ceil().max(1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn founded_game() -> (Game, u32) {
        let mut game = Game::new(2, 24, 16, 7_102, 200, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &crate::game::Action::FoundCity { unit: settler })
            .unwrap();
        let city = game.player_city_ids(0)[0];
        (game, city)
    }

    #[test]
    fn the_front_is_exactly_where_the_engine_expands() {
        let (mut game, city) = founded_game();
        for _ in 0..4 {
            let front = game.border_growth_front(city);
            assert!(!front.is_empty(), "a fresh capital has room to grow");
            let before = game.cities[&city].owned_tiles.len();
            game.expand_borders(city);
            let claimed = *game.cities[&city].owned_tiles.last().unwrap();
            assert_eq!(game.cities[&city].owned_tiles.len(), before + 1);
            assert!(
                front.contains(&claimed),
                "the engine claimed {claimed:?}, outside the forecast front {front:?}"
            );
        }
    }

    #[test]
    fn growth_turns_follow_the_shipped_curve_and_the_banked_culture() {
        let (mut game, city) = founded_game();
        let culture = game.city_yields(city).culture;
        assert!(culture > 0.0, "a capital makes culture");
        let owned = game.cities[&city].owned_tiles.len() as i32;
        let need = (10.0 + 6.0 * ((owned - 7).max(0) as f64).powf(1.3)).trunc();
        game.cities.get_mut(&city).unwrap().border_culture = 0.0;
        assert_eq!(
            game.border_growth_turns(city),
            Some((need / culture).ceil().max(1.0))
        );
        // Culture already banked brings the claim forward; a bank past the
        // threshold is "next turn", never zero or negative.
        game.cities.get_mut(&city).unwrap().border_culture = need + 5.0;
        assert_eq!(game.border_growth_turns(city), Some(1.0));
    }

    #[test]
    fn a_seat_whose_borders_cannot_grow_forecasts_never() {
        let (mut game, city) = founded_game();
        assert!(game.border_growth_turns(city).is_some());
        // A Border Control treaty in force: the engine skips growth and so
        // does the forecast.
        game.active_congress_effects.push(crate::game::CongressEffect {
            resolution: "border_control_treaty".to_string(),
            outcome: "B".to_string(),
            target: "0".to_string(),
            expires: game.turn + 30,
        });
        assert_eq!(game.border_growth_turns(city), None);
    }
}
