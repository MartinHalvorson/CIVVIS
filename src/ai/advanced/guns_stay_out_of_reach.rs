//! `guns-stay-out-of-reach`: a siege gun does not end a step where one
//! visible hostile could destroy it with a single blow.
//!
//! Live King civvis-20261005T093932Z (game 117) declared on Babylon at turn
//! 135 with 31 techs against their 48 and lost eleven siege guns between
//! turns 137 and 194, most to one blow each: a trebuchet at full health
//! stepped beside a Line Infantry or a Cuirassier the seat could see at the
//! start of the turn (turns 137, 138, 156 and 172) and was gone on the
//! enemy's turn. Live replays of turns 156 and 172 pull the same guns back,
//! so the lethal step came from a mover that reads persisted siege state; the
//! veto therefore sits where every pathed move passes,
//! `BasicAi::path_step_allowed`, not in any one mover.
//!
//! At each frame's unit loop the tiles each gun can reach this turn are read
//! on the battle planner's danger field. A tile is lethal when one hostile
//! unit's blow there, at its upper roll, meets the gun's health. City and
//! Encampment strikes are left out: `guns-enter-together` deliberately walks
//! two or more guns into one city's strike to share it. A move onto a lethal
//! tile is refused unless it lowers the blow the gun stands under, and a gun
//! that starts its turn on a lethal tile steps to the nearest tile that is
//! not, so a stood-down gun never holds inside hostile reach. The danger
//! field reads ZOC through `strike-reach`, so a screen that stops the
//! hostile short of the gun clears the tile; a lone or unscreened gun is the
//! one held back.
use super::battle_planner::{DangerField, STRUCTURE_SOURCE};
use super::{AdvancedAi, ForceRole};
use crate::game::Game;
use crate::Pos;
use std::collections::HashMap;

impl AdvancedAi {
    /// Our land siege guns with movement left, each with the tiles it can
    /// reach this turn (its own included) where one visible hostile unit's
    /// blow, at its upper roll, would destroy it. Left in
    /// `BasicAi::gun_lethal_tiles` for `path_step_allowed`, cleared first;
    /// empty with the gene off.
    pub(super) fn mark_gun_lethal_tiles(&mut self, g: &Game, pid: usize) {
        self.base.gun_lethal_tiles.clear();
        if !self.guns_stay_out_of_reach {
            return;
        }
        let guns: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                matches!(Self::force_role(g, *uid), ForceRole::Siege)
                    && g.rules.units[unit.kind].domain.as_deref() != Some("sea")
                    && unit.moves_left > 0.0
                    && !g.is_embarked(unit)
            })
            .collect();
        if guns.is_empty() {
            return;
        }
        let mut field = DangerField::with_reach(g, pid, self.strike_reach);
        for uid in guns {
            let unit = &g.units[&uid];
            let hp = f64::from(unit.hp);
            let mut tiles = vec![unit.pos];
            tiles.extend(g.reachable(uid));
            let lethal: HashMap<Pos, f64> = tiles
                .into_iter()
                .filter_map(|tile| {
                    let blow = field
                        .contributions(tile, uid)
                        .iter()
                        .filter(|(source, _)| source.is_some_and(|id| id & STRUCTURE_SOURCE == 0))
                        .map(|(_, blow)| (blow * crate::ai::COMBAT_ROLL_MAX).ceil().min(100.0))
                        .fold(0.0, f64::max);
                    (blow >= hp).then_some((tile, blow))
                })
                .collect();
            if !lethal.is_empty() {
                self.base.gun_lethal_tiles.insert(uid, lethal);
            }
        }
    }

    /// A gun marked on its own tile steps to the nearest tile it can reach
    /// that is not lethal. The battle planner's placements are its own and
    /// read the same field; they are left alone. Nothing with the gene off.
    pub(super) fn guns_step_out_of_reach(&mut self, g: &mut Game, pid: usize) {
        let mut exposed: Vec<(u32, Pos, f64)> = self
            .base
            .gun_lethal_tiles
            .iter()
            .filter_map(|(uid, lethal)| {
                let pos = g.units.get(uid)?.pos;
                lethal.get(&pos).map(|blow| (*uid, pos, *blow))
            })
            .filter(|(uid, _, _)| !self.battle_planner_ordered.contains(uid))
            .collect();
        exposed.sort_by_key(|(uid, _, _)| *uid);
        for (uid, pos, blow) in exposed {
            let lethal = &self.base.gun_lethal_tiles[&uid];
            let Some(to) = g
                .reachable(uid)
                .into_iter()
                .filter(|tile| !lethal.contains_key(tile))
                .min_by_key(|tile| (g.wdist(pos, *tile), *tile))
            else {
                continue;
            };
            if self.base.path_walk_to(g, pid, uid, to) {
                crate::think!(self.journal(), Military, Decision,
                    "The {} steps out of a one-blow kill", g.units[&uid].kind;
                    "a visible hostile's blow reads {:.0} where it stood against {} health; {:?} lies outside every such blow",
                    blow, g.units[&uid].hp, to;
                    to);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::VictoryTarget;

    /// A grassland field, our trebuchet at (16, 10), a Babylonian Line
    /// Infantry at (20, 10) seen by a warrior of ours behind it, at war.
    fn fixture() -> (Game, AdvancedAi, u32, u32) {
        let mut g = Game::new_full(2, 40, 24, 377300, 500, 0, false);
        for uid in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(uid);
        }
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.resource = None;
            tile.hills = false;
        }
        g.found_city_for(0, (6, 10), None);
        g.current = 0;
        g.at_war.insert((0, 1));
        g.players[0].explored.extend(g.map.tiles.keys().copied());
        let gun = g.spawn_test_unit("trebuchet", 0, (16, 10));
        let hostile = g.spawn_test_unit("line_infantry", 1, (20, 10));
        g.spawn_test_unit("warrior", 0, (22, 10));
        assert!(g.unit_visible_to(hostile, 0));
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.base.recorded_tactical_step = true;
        (g, ai, gun, hostile)
    }

    /// A Line Infantry with two moves steps once and strikes: its reach is
    /// two tiles, and its blow at the upper roll destroys a healthy
    /// trebuchet.
    #[test]
    fn a_gun_does_not_step_where_one_blow_destroys_it() {
        let (mut g, mut ai, gun, _) = fixture();
        ai.enable_guns_stay_out_of_reach();
        ai.mark_gun_lethal_tiles(&g, 0);
        let lethal = &ai.base.gun_lethal_tiles[&gun];
        assert!(lethal.contains_key(&(18, 10)), "{lethal:?}");
        assert!(!lethal.contains_key(&(17, 10)), "{lethal:?}");
        assert!(!lethal.contains_key(&(16, 10)), "{lethal:?}");
        assert!(
            ai.base.path_move(&mut g, 0, gun, (17, 10)),
            "a step short of the reach is taken"
        );
        assert!(
            !ai.base.path_move(&mut g, 0, gun, (18, 10)),
            "the step into reach is refused"
        );
        assert_eq!(g.units[&gun].pos, (17, 10));

        // The gene off, nothing is marked and the same steps are taken.
        let (mut g, mut ai, gun, _) = fixture();
        ai.mark_gun_lethal_tiles(&g, 0);
        assert!(ai.base.gun_lethal_tiles.is_empty());
        assert!(ai.base.path_move(&mut g, 0, gun, (17, 10)));
        assert!(ai.base.path_move(&mut g, 0, gun, (18, 10)));
    }

    #[test]
    fn a_gun_inside_the_reach_steps_out_and_a_weaker_hostile_is_no_veto() {
        let (mut g, mut ai, gun, _) = fixture();
        ai.enable_guns_stay_out_of_reach();
        g.remove_unit(gun);
        let gun = g.spawn_test_unit("trebuchet", 0, (18, 10));
        ai.mark_gun_lethal_tiles(&g, 0);
        assert!(ai.base.gun_lethal_tiles[&gun].contains_key(&(18, 10)));
        ai.guns_step_out_of_reach(&mut g, 0);
        let pos = g.units[&gun].pos;
        assert!(g.wdist(pos, (20, 10)) > 2, "it stands at {pos:?}");

        // A Warrior's blow cannot destroy a healthy trebuchet.
        let (mut g, mut ai, gun, hostile) = fixture();
        ai.enable_guns_stay_out_of_reach();
        g.remove_unit(hostile);
        let warrior = g.spawn_test_unit("warrior", 1, (20, 10));
        assert!(g.unit_visible_to(warrior, 0));
        ai.mark_gun_lethal_tiles(&g, 0);
        assert!(ai.base.gun_lethal_tiles.is_empty());
        assert!(ai.base.path_move(&mut g, 0, gun, (17, 10)));
        assert!(ai.base.path_move(&mut g, 0, gun, (18, 10)));
    }

    /// `guns-enter-together` walks guns into a city's strike on purpose: a
    /// walled city's blow on a wounded gun is no veto here.
    #[test]
    fn a_city_strike_is_left_to_guns_enter_together() {
        let (mut g, mut ai, gun, hostile) = fixture();
        ai.enable_guns_stay_out_of_reach();
        g.remove_unit(hostile);
        let cid = g.found_city_for(1, (20, 10), None);
        g.cities.get_mut(&cid).unwrap().wall_hp = 100;
        g.units.get_mut(&gun).unwrap().hp = 10;
        ai.mark_gun_lethal_tiles(&g, 0);
        assert!(
            ai.base.gun_lethal_tiles.is_empty(),
            "{:?}",
            ai.base.gun_lethal_tiles
        );
    }
}
