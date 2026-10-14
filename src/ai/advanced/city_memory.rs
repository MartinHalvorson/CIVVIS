//! City- and unit-keyed memory carried through a live board rebuild.
//!
//! The live seat plays `--fresh-board`: every turn the mirror builds a new
//! board, and a new board numbers its cities and units afresh. Replaying live
//! King civvis-20261003T135713Z (game 38) over turns 95-131, Ulundi's id was
//! 69, 68, 67, 71, 72, 73, 71, 72, 77, 83, ... — a different number on almost
//! every turn. The campaign, siege, conquest and objective-board memories
//! already follow their cities by position across a rebuild; the commitment
//! ledger, the capture stand-downs, the reached campaign cities and the
//! remembered city sightings did not. So the capture commitment reopened at
//! zero turns every turn and `capture-go-or-stand-down` could never fire; a
//! stand-down, had one been written, held out whichever city inherited the
//! old number; and a fogged city's last walls and hit points answered for
//! another city.

use super::AdvancedAi;
use crate::game::Game;
use std::collections::BTreeMap;

impl AdvancedAi {
    /// Follow the city-keyed memory to `next`'s ids by position, and the
    /// commitment ledger's unit owners through `units` (old id to new). See
    /// the module documentation.
    pub fn remap_city_memory(&mut self, previous: &Game, next: &Game, units: &BTreeMap<u32, u32>) {
        let city = |id: u32| {
            previous
                .cities
                .get(&id)
                .and_then(|city| next.city_at(city.pos))
        };
        self.commitments
            .remap_ids(|uid| units.get(&uid).copied(), city);
        self.capture_stood_down = std::mem::take(&mut self.capture_stood_down)
            .into_iter()
            .filter_map(|(cid, until)| city(cid).map(|cid| (cid, until)))
            .collect();
        self.campaign_cities_reached = std::mem::take(&mut self.campaign_cities_reached)
            .into_iter()
            .filter_map(city)
            .collect();
        // A sighting carries its own position, so even a city the previous
        // board no longer held is found again.
        self.belief.cities = std::mem::take(&mut self.belief.cities)
            .into_values()
            .filter_map(|sighting| next.city_at(sighting.pos).map(|cid| (cid, sighting)))
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::super::commitments::{Kind, Owner, Target};
    use super::super::{AdvancedAi, GrandStrategy, StrategicPlan};
    use crate::game::Game;
    use crate::Pos;
    use std::collections::BTreeMap;

    fn board(cities: &[(usize, Pos)], turn: u32) -> Game {
        let mut game = Game::new_full(3, 36, 22, 91_777, 1_000, 0, false);
        for unit in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(unit);
        }
        game.barb_camps.clear();
        game.barb_naval_camps.clear();
        for tile in game.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
        for (pid, pos) in cities {
            game.found_city_for(*pid, *pos, None);
        }
        game.at_war.clear();
        game.at_war.insert((0, 1));
        game.turn = turn;
        game.current = 0;
        game
    }

    /// The same three cities, founded in another order: every id moves.
    fn boards() -> (Game, Game) {
        let previous = board(&[(0, (6, 12)), (1, (14, 12)), (1, (17, 15))], 60);
        let next = board(&[(1, (17, 15)), (1, (14, 12)), (0, (6, 12))], 61);
        assert_ne!(
            previous.city_at((17, 15)),
            next.city_at((17, 15)),
            "fixture: the rebuild moves the target's id"
        );
        (previous, next)
    }

    fn sieging(game: &Game) -> AdvancedAi {
        let mut ai = AdvancedAi::new();
        ai.plan = Some(StrategicPlan {
            strategy: GrandStrategy::Conquest,
            target_player: Some(1),
            target_city: game.city_at((17, 15)),
            threatened_city: None,
            desired_cities: 6,
            assessed_turn: game.turn,
            rush: false,
        });
        ai
    }

    #[test]
    fn a_capture_commitment_survives_a_rebuild() {
        for remap in [false, true] {
            let (mut previous, mut next) = boards();
            let mut ai = sieging(&previous);
            ai.reconcile_commitments(&mut previous, 0);
            ai.remap_campaign_city_memory(&previous, &next);
            if remap {
                ai.remap_city_memory(&previous, &next, &BTreeMap::new());
            }
            ai.reconcile_commitments(&mut next, 0);
            let c = ai
                .commitments
                .open_for(Kind::Capture, Owner::Empire)
                .expect("a capture is open");
            assert_eq!(c.target, Target::City(next.city_at((17, 15)).unwrap()));
            assert_eq!(
                c.made,
                if remap { 60 } else { 61 },
                "without the remap the rebuild reads as a retarget and the decision restarts"
            );
        }
    }

    #[test]
    fn a_stand_down_and_a_sighting_follow_their_city() {
        let (previous, next) = boards();
        let mut ai = sieging(&previous);
        let target = previous.city_at((17, 15)).unwrap();
        ai.capture_stood_down.insert(target, 80);
        ai.belief.cities.insert(
            target,
            crate::belief::CitySighting {
                pos: (17, 15),
                turn: 55,
                owner: 1,
                hp: 120,
                wall_hp: 40,
                strength: 30.0,
            },
        );
        ai.remap_city_memory(&previous, &next, &BTreeMap::new());
        let moved = next.city_at((17, 15)).unwrap();
        assert_eq!(ai.capture_stood_down.get(&moved), Some(&80));
        assert_eq!(ai.capture_stood_down.len(), 1);
        assert_eq!(
            ai.remembered_city(moved).map(|s| (s.pos, s.wall_hp)),
            Some(((17, 15), 40))
        );
        assert!(
            ai.belief
                .cities
                .values()
                .all(|s| next.city_at(s.pos).is_some()),
            "no sighting keeps an id another city now holds"
        );
    }
}
