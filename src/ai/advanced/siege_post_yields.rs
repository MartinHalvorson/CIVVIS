//! `siege-post-yields-to-one-blow`: a healthy siege member standing where
//! one visible hostile's blow removes it may leave its post.
//!
//! The per-unit ladder (`advanced_military_step_with_decline`) lets a healthy
//! member of an active or staging siege skip both `healing_step` and
//! `retreat_step` (`siege_post_holds`): the envelope evacuation charges
//! every enemy blow at once, and on live King game 46 it walked full-health
//! Archers off their firing posts every turn of Reduce. The cost of that
//! exemption shows in the 57 live Immortal games of 2026-11-03/04: 131 of
//! the 765 military units lost had struck that turn, 36 of them shooters
//! whose target lived — Archers, Crossbows and Bombards dealing 2-13 damage
//! to a Line Infantry or a Modern Armor from inside its reach, then removed
//! by it on the enemy's turn. Version one of `guns-stay-out-of-reach` steps
//! a siege GUN off such a tile; the shooters and melee bodies of the same
//! siege had nothing.
//!
//! Once a frame, next to `mark_gun_lethal_tiles`, this marks every healthy
//! siege member (land, with movement left) whose own tile is one blow from
//! removal: a visible hostile UNIT's blow there, at its upper roll
//! (`COMBAT_ROLL_MAX`), meets its health. City and Encampment strikes are
//! left out — a siege accepts the city's shot by design. A marked unit no
//! longer holds its post: the ordinary recovery runs for it, and
//! `retreat_step` still lets it take a blow that leaves it alive
//! (`can_survive_by_attacking`) before it withdraws. Every other siege
//! member holds as before. `Kind::OptIn`; nothing is read with the gene off.
use super::battle_planner::{DangerField, ROTATE_HP, STRUCTURE_SOURCE};
use super::AdvancedAi;
use crate::game::Game;
use std::collections::BTreeSet;

impl AdvancedAi {
    /// Our healthy siege members on a one-blow tile, recorded for this
    /// frame's per-unit ladder. Cleared first; empty with the gene off.
    pub(super) fn mark_siege_posts_under_one_blow(&mut self, g: &Game, pid: usize) {
        self.siege_posts_under_one_blow.clear();
        if !self.siege_post_yields_to_one_blow || g.is_arena() {
            return;
        }
        let posts: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| {
                let unit = &g.units[uid];
                let spec = &g.rules.units[unit.kind];
                spec.class == "military"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && unit.moves_left > 0.0
                    && unit.hp >= ROTATE_HP
                    && !g.is_embarked(unit)
                    && (self.active_siege_member(g, pid, *uid)
                        || self.staging_siege_member(g, pid, *uid))
            })
            .collect();
        if posts.is_empty() {
            return;
        }
        let marked = self.one_blow_posts(g, pid, &posts);
        for uid in &marked {
            let unit = &g.units[uid];
            crate::think!(self.journal(), Military, Detail,
                "The {} at {:?} leaves its siege post", unit.kind, unit.pos;
                "a visible hostile's blow there would remove it at {} hp; the recovery may withdraw it",
                unit.hp;
                unit.pos);
        }
        self.siege_posts_under_one_blow = marked;
    }

    /// Of `units`, those standing where a visible hostile unit's blow at its
    /// upper roll meets their health; City and Encampment strikes left out.
    pub(super) fn one_blow_posts(&self, g: &Game, pid: usize, units: &[u32]) -> BTreeSet<u32> {
        let mut field = DangerField::with_reach(g, pid, self.strike_reach);
        units
            .iter()
            .copied()
            .filter(|uid| {
                let Some(unit) = g.units.get(uid) else {
                    return false;
                };
                if !field.reaches_tile(unit.pos) {
                    return false;
                }
                let hp = f64::from(unit.hp);
                field
                    .contributions(unit.pos, *uid)
                    .iter()
                    .filter(|(source, _)| source.is_some_and(|id| id & STRUCTURE_SOURCE == 0))
                    .map(|(_, blow)| (blow * crate::ai::COMBAT_ROLL_MAX).ceil().min(100.0))
                    .fold(0.0, f64::max)
                    >= hp
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::advanced::VictoryTarget;

    /// Grassland; our archer at (16, 10) and a second at (12, 10); a Line
    /// Infantry at (18, 10) seen by a warrior of ours; at war.
    fn fixture() -> (Game, AdvancedAi, u32, u32, u32) {
        let mut g = Game::new_full(2, 40, 24, 439800, 500, 0, false);
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
        let near = g.spawn_test_unit("archer", 0, (16, 10));
        let far = g.spawn_test_unit("archer", 0, (12, 10));
        let hostile = g.spawn_test_unit("line_infantry", 1, (18, 10));
        g.spawn_test_unit("warrior", 0, (19, 10));
        assert!(g.unit_visible_to(hostile, 0));
        let ai = AdvancedAi::targeting(VictoryTarget::Domination);
        (g, ai, near, far, hostile)
    }

    #[test]
    fn the_gene_ships_off_and_is_registered() {
        crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
            "siege-post-yields-to-one-blow",
            |ai| ai.siege_post_yields_to_one_blow,
        );
    }

    /// The Archer two tiles from a Line Infantry is one blow from removal;
    /// the one six tiles back is not; a Warrior in its place threatens
    /// neither.
    #[test]
    fn an_archer_inside_a_line_infantrys_blow_is_a_one_blow_post() {
        let (g, ai, near, far, _) = fixture();
        let marked = ai.one_blow_posts(&g, 0, &[near, far]);
        assert!(marked.contains(&near), "{marked:?}");
        assert!(!marked.contains(&far), "{marked:?}");

        let (mut g, ai, near, far, hostile) = fixture();
        g.remove_unit(hostile);
        let warrior = g.spawn_test_unit("warrior", 1, (18, 10));
        assert!(g.unit_visible_to(warrior, 0));
        assert!(ai.one_blow_posts(&g, 0, &[near, far]).is_empty());
    }

    /// A walled city's strike is the siege's to accept: no post is marked
    /// for it.
    #[test]
    fn a_city_strike_marks_no_post() {
        let (mut g, ai, near, _, hostile) = fixture();
        g.remove_unit(hostile);
        let cid = g.found_city_for(1, (18, 10), None);
        g.cities.get_mut(&cid).unwrap().wall_hp = 100;
        g.units.get_mut(&near).unwrap().hp = 10;
        assert!(ai.one_blow_posts(&g, 0, &[near]).is_empty());
    }

    /// With the gene off nothing is marked, whatever the board.
    #[test]
    fn the_gene_off_marks_nothing() {
        let (g, mut ai, _, _, _) = fixture();
        ai.mark_siege_posts_under_one_blow(&g, 0);
        assert!(ai.siege_posts_under_one_blow.is_empty());
    }
}
