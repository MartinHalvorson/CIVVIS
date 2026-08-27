//! Field craft: `flip-nearby-city-states`, one opt-in gene that adds a
//! city-state's *place* — proximity to our cities and the sitting suzerain
//! we would unseat — to the envoy score, amortised over the envoys the
//! suzerainty still needs.
//!
//! Operator goal (2026-08-24): *"very smart heuristics around unit tactics in
//! warfare … flip nearby city states, defend in our territory for maximum
//! healing rates, utilize support bonuses, zone of control and whatever else
//! you can think of."* Three unit-tactics genes shipped beside this one —
//! `shoot-and-scoot`, `zoc-screen`, `pillage-to-heal` — and were removed
//! from the code on 2026-08-25 by operator directive; their screen rows stay
//! in the ranking's *Removed from the code* table.
//!
//! `flip_nearby_city_state_bonus` is one term in `advanced_envoys`'s score.
//! Off in `AdvancedAi::new()` and `legacy()`, a `Kind::OptIn` row in
//! `genes.rs`, byte-identical when off. Fires probe under
//! `docs/gene_screens/fires/`.


use super::AdvancedAi;
use crate::game::Game;

/// A city-state within this many tiles of one of our cities is on our
/// border. Nine is the distance a Warrior walks in a handful of turns and
/// the ring inside which its borders meet ours on the screen's map.
pub(super) const FLIP_RADIUS: i32 = 9;
/// Per tile inside the radius, so the city-state on the border itself is
/// worth ninety before the flip terms — the size of one lane alignment.
const FLIP_PER_TILE: i64 = 10;
/// The sitting suzerain is at war with us: its client fights us today and
/// would fight for us tomorrow.
const FLIP_ENEMY_SUZERAIN: i64 = 200;
/// The sitting suzerain is a rival at peace with us.
const FLIP_RIVAL_SUZERAIN: i64 = 60;

impl AdvancedAi {

    // ------------------------------------------------------------------
    // shoot-and-scoot
    // ------------------------------------------------------------------

    // ------------------------------------------------------------------
    // pillage-to-heal
    // ------------------------------------------------------------------

    // ------------------------------------------------------------------
    // zoc-screen
    // ------------------------------------------------------------------

    // ------------------------------------------------------------------
    // flip-nearby-city-states
    // ------------------------------------------------------------------

    /// The gene: what a city-state's *place* is worth to the envoy scorer —
    /// proximity to our cities, and the sitting suzerain we would unseat —
    /// amortised over the envoys the suzerainty still needs. Zero for a
    /// city-state we already hold securely, so a border client past its
    /// contest does not soak up envoys.
    pub(super) fn flip_nearby_city_state_bonus(
        &self,
        g: &Game,
        pid: usize,
        minor: usize,
        needed: i64,
    ) -> i64 {
        if !self.flip_nearby_city_states {
            return 0;
        }
        let Some(seat) = g
            .player_city_ids(minor)
            .into_iter()
            .next()
            .map(|cid| g.cities[&cid].pos)
        else {
            return 0;
        };
        let Some(near) = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.wdist(g.cities[&cid].pos, seat))
            .min()
        else {
            return 0;
        };
        if near > FLIP_RADIUS {
            return 0;
        }
        let holder = g.suzerain_of(minor);
        if holder == Some(pid) {
            let mine = g.envoys_at(pid, minor);
            let rival = g
                .players
                .iter()
                .filter(|p| !p.is_minor && !p.is_barbarian && p.id != pid)
                .map(|p| g.envoys_at(p.id, minor))
                .max()
                .unwrap_or(0);
            if mine > rival + 1 {
                return 0;
            }
        }
        let proximity = i64::from(FLIP_RADIUS + 1 - near) * FLIP_PER_TILE;
        let flip = match holder {
            Some(leader) if leader != pid && g.is_at_war(pid, leader) => FLIP_ENEMY_SUZERAIN,
            Some(leader) if leader != pid => FLIP_RIVAL_SUZERAIN,
            _ => 0,
        };
        (proximity + flip) / needed.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::AdvancedAi;
    use super::*;
    use crate::game::{Action, Game};

    #[test]
    fn flip_nearby_city_states_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("flip-nearby-city-states", |ai| ai.flip_nearby_city_states);
    }

    /// The envoy term: a city-state on our border under an enemy's envoys
    /// outranks the same city-state at peace, which outranks one nobody
    /// holds, and a city-state beyond the radius is worth nothing. Off, all
    /// of it is zero.
    #[test]
    fn a_nearby_city_state_under_the_enemy_is_worth_flipping() {
        let mut game = Game::new_full(2, 40, 24, 80_108, 300, 2, false);
        for pid in 0..2 {
            let settler = game
                .player_unit_ids(pid)
                .into_iter()
                .find(|unit| game.units[unit].kind == "settler")
                .unwrap();
            game.found_city_for(pid, game.units[&settler].pos, None);
        }
        let minors: Vec<usize> = game
            .players
            .iter()
            .filter(|p| p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        assert_eq!(minors.len(), 2);
        let home = game.cities[&game.player_city_ids(0)[0]].pos;
        // Seat the two city-states by hand: one four tiles from our
        // capital, one far beyond the radius.
        for (minor, distance) in minors.iter().zip([4, FLIP_RADIUS + 6]) {
            for cid in game.player_city_ids(*minor) {
                game.cities.remove(&cid);
            }
            for unit in game.player_unit_ids(*minor) {
                game.remove_unit(unit);
            }
            let seat = game
                .map
                .tiles
                .keys()
                .copied()
                .filter(|position| {
                    game.wdist(*position, home) == distance
                        && game.rules.is_passable(&game.map.tiles[position])
                        && !game.rules.is_water(&game.map.tiles[position])
                        && game.map.tiles[position].owner_city.is_none()
                        && game.city_at(*position).is_none()
                })
                .min()
                .expect("open ground at the distance");
            game.found_city_for(*minor, seat, None);
        }
        let (near, far) = (minors[0], minors[1]);
        let mut stock = AdvancedAi::new();
        assert_eq!(stock.flip_nearby_city_state_bonus(&game, 0, near, 2), 0);
        let mut flipping = AdvancedAi::new();
        flipping.enable_flip_nearby_city_states();
        let unheld = flipping.flip_nearby_city_state_bonus(&game, 0, near, 2);
        assert!(unheld > 0, "a border city-state is worth envoys: {unheld}");
        assert_eq!(flipping.flip_nearby_city_state_bonus(&game, 0, far, 2), 0);

        // The rival takes the suzerainty at peace, then at war.
        game.players[1].envoys_free = 3;
        game.players[1].met.insert(near);
        game.players[near].met.insert(1);
        for _ in 0..3 {
            game.current = 1;
            game.apply(1, &Action::SendEnvoy { player: near })
                .expect("the rival places an envoy");
        }
        game.current = 0;
        assert_eq!(game.suzerain_of(near), Some(1));
        let rival_held = flipping.flip_nearby_city_state_bonus(&game, 0, near, 2);
        assert!(rival_held > unheld, "{rival_held} > {unheld}");
        game.at_war.insert((0, 1));
        let enemy_held = flipping.flip_nearby_city_state_bonus(&game, 0, near, 2);
        assert!(enemy_held > rival_held, "{enemy_held} > {rival_held}");
        // Amortised: the same flip four envoys away is worth half as much
        // per envoy as two away.
        assert_eq!(
            flipping.flip_nearby_city_state_bonus(&game, 0, near, 4),
            enemy_held * 2 / 4
        );
        stock.disable_flip_nearby_city_states();
    }
}
