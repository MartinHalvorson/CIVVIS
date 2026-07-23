//! Belief state: what a fog-honest agent remembers about what it can no
//! longer see.
//!
//! `obs_tensor` shows only what is visible *right now*, so an enemy army
//! that steps behind the fog vanishes from the observation entirely. Human
//! players do not forget it — they track its last known position and reason
//! about how stale that information is. `BeliefState` is that memory:
//! updated only from tiles the player can actually see, so it never invents
//! knowledge the fog should be hiding.
//!
//! This is the first rung of AI_GAPS item 5. It supplies the memory; using
//! it to value scouting as information gain is the next step.
use std::collections::BTreeMap;

use crate::game::Game;
use crate::obs::visibility;
use crate::Pos;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sighting {
    pub pos: Pos,
    pub turn: u32,
    pub owner: usize,
    pub kind: String,
    /// Strength when last seen, not current strength: the memory ages.
    pub strength: f64,
}

#[derive(Clone, Debug, Default)]
pub struct BeliefState {
    /// Last confirmed sighting per enemy unit id.
    pub units: BTreeMap<u32, Sighting>,
    /// Last confirmed owner per foreign city id (cities do not move, but
    /// they change hands, and a stale owner is exactly the kind of mistake
    /// a fog-honest agent should be able to make).
    pub cities: BTreeMap<u32, (Pos, usize, u32)>,
    pub updated_turn: u32,
}

impl BeliefState {
    pub fn new() -> BeliefState {
        BeliefState::default()
    }

    /// Fold this turn's observations into memory. Units seen now are
    /// refreshed; a unit whose last known tile is currently visible and
    /// empty of it is forgotten, because the player can see it left.
    pub fn observe(&mut self, g: &Game, pid: usize) {
        let (vis, _) = visibility(g, pid);
        self.updated_turn = g.turn;

        let mut seen_now: Vec<u32> = Vec::new();
        for unit in g.units.values() {
            if unit.owner == pid || !vis.contains(&unit.pos) {
                continue;
            }
            if !g.unit_visible_to(unit.id, pid) {
                continue;
            }
            seen_now.push(unit.id);
            self.units.insert(
                unit.id,
                Sighting {
                    pos: unit.pos,
                    turn: g.turn,
                    owner: unit.owner,
                    kind: unit.kind.clone(),
                    strength: g.unit_strength(unit, false),
                },
            );
        }
        // Forget what we can see is gone: a remembered tile now in view and
        // no longer holding that unit is positive evidence it moved.
        self.units.retain(|id, sighting| {
            if seen_now.contains(id) {
                return true;
            }
            !vis.contains(&sighting.pos)
        });
        // Dead units stay remembered only until their last tile is seen.
        self.units
            .retain(|id, sighting| g.units.contains_key(id) || !vis.contains(&sighting.pos));

        for city in g.cities.values() {
            if city.owner == pid || !vis.contains(&city.pos) {
                continue;
            }
            self.cities.insert(city.id, (city.pos, city.owner, g.turn));
        }
    }

    /// How many turns old a sighting is.
    pub fn staleness(&self, sighting: &Sighting) -> u32 {
        self.updated_turn.saturating_sub(sighting.turn)
    }

    /// Remembered enemy strength near a tile, discounted by staleness so a
    /// twenty-turn-old sighting counts for little. `horizon` is the number
    /// of turns after which a memory is worthless.
    pub fn remembered_threat(
        &self,
        g: &Game,
        pid: usize,
        at: Pos,
        radius: i32,
        horizon: u32,
    ) -> f64 {
        self.units
            .values()
            .filter(|s| s.owner != pid && g.is_at_war(pid, s.owner))
            .filter(|s| g.wdist(s.pos, at) <= radius)
            .map(|s| {
                let age = self.staleness(s).min(horizon) as f64;
                let decay = 1.0 - age / horizon.max(1) as f64;
                s.strength * decay
            })
            .sum()
    }

    /// Enemy units we remember but cannot currently see.
    pub fn unseen_sightings(&self, g: &Game, pid: usize) -> Vec<&Sighting> {
        let (vis, _) = visibility(g, pid);
        self.units
            .values()
            .filter(|s| s.owner != pid && !vis.contains(&s.pos))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::BeliefState;
    use crate::game::{Action, Game};

    fn two_player_game() -> Game {
        let mut g = Game::new_full(2, 30, 18, 4_101, 200, 0, false);
        for pid in 0..2 {
            let settler = g
                .player_unit_ids(pid)
                .into_iter()
                .find(|u| g.units[u].kind == "settler")
                .unwrap();
            g.current = pid;
            g.apply(pid, &Action::FoundCity { unit: settler }).unwrap();
        }
        g.current = 0;
        g
    }

    /// A unit seen once and then hidden stays in memory at its last known
    /// tile, and its staleness grows — that is the whole point. The sighting
    /// must happen away from our cities, or the tile stays permanently in
    /// view and we would simply watch the unit leave.
    #[test]
    fn remembers_a_unit_after_it_leaves_sight() {
        let mut g = two_player_game();
        let capital = g.player_city_ids(0)[0];
        let home = g.cities[&capital].pos;
        // A scout far from home is our only eye on the frontier.
        let frontier = g
            .map
            .tiles
            .keys()
            .copied()
            .find(|p| {
                g.wdist(*p, home) > 10
                    && g.map
                        .get(*p)
                        .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
            })
            .expect("a frontier tile");
        let scout = g.spawn_test_unit("scout", 0, frontier);
        let enemy_tile = g
            .nbrs(frontier)
            .into_iter()
            .find(|p| g.map.get(*p).is_some())
            .unwrap();
        let enemy = g.spawn_test_unit("warrior", 1, enemy_tile);

        let mut belief = BeliefState::new();
        belief.observe(&g, 0);
        assert!(belief.units.contains_key(&enemy), "the scout sees it");

        // The scout goes home and the enemy moves on: nobody can see the
        // frontier any more, so the last known tile must be remembered.
        g.units.get_mut(&scout).unwrap().pos = home;
        let far = g
            .map
            .tiles
            .keys()
            .copied()
            .find(|p| g.wdist(*p, home) > 10 && g.wdist(*p, enemy_tile) > 6)
            .expect("a distant tile");
        g.units.get_mut(&enemy).unwrap().pos = far;
        g.turn += 5;
        belief.observe(&g, 0);

        let remembered = belief.units.get(&enemy).expect("memory survives the fog");
        assert_eq!(
            remembered.pos, enemy_tile,
            "memory holds the LAST KNOWN tile"
        );
        assert_eq!(belief.staleness(remembered), 5);
        assert_eq!(belief.unseen_sightings(&g, 0).len(), 1);
    }

    /// Memory must not become omniscience: seeing the old tile empty is
    /// evidence the unit left, so the stale sighting is dropped.
    #[test]
    fn forgets_when_the_remembered_tile_is_seen_empty() {
        let mut g = two_player_game();
        let capital = g.player_city_ids(0)[0];
        let near = g.cities[&capital].pos;
        let enemy = g.spawn_test_unit("warrior", 1, near);
        let mut belief = BeliefState::new();
        belief.observe(&g, 0);
        assert!(belief.units.contains_key(&enemy));

        // It steps one tile — still inside our city's sight, so we watch it
        // go and must not keep believing it sits on the old tile.
        let neighbor = g
            .nbrs(near)
            .into_iter()
            .find(|p| g.map.get(*p).is_some())
            .unwrap();
        g.units.get_mut(&enemy).unwrap().pos = neighbor;
        belief.observe(&g, 0);
        let remembered = belief.units.get(&enemy).expect("still visible nearby");
        assert_eq!(remembered.pos, neighbor, "memory tracks the new tile");
    }

    /// Threat memory decays to nothing past the horizon.
    #[test]
    fn remembered_threat_decays_with_age() {
        let mut g = two_player_game();
        let capital = g.player_city_ids(0)[0];
        let near = g.cities[&capital].pos;
        g.spawn_test_unit("warrior", 1, near);
        g.at_war.insert((0, 1));

        let mut belief = BeliefState::new();
        belief.observe(&g, 0);
        let fresh = belief.remembered_threat(&g, 0, near, 3, 20);
        assert!(fresh > 0.0, "a just-seen enemy is a live threat");

        belief.updated_turn += 20;
        let stale = belief.remembered_threat(&g, 0, near, 3, 20);
        assert_eq!(stale, 0.0, "a memory past the horizon is worthless");
    }
}
