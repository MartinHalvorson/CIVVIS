//! `denial-needs-a-road`: a Domination army counters only a rival it can
//! march to.
//!
//! Live King civvis-20261003T100536Z (game 31) aimed its denial campaign at
//! India, the culture leader, from turn 130 to its Culture loss at 179. The
//! Siege of Delhi read "0 of 8 units staged" for 45 turns while the army
//! stood 15 to 21 tiles out. Delhi was within the straight-line declaration
//! reach, but no land path reached its ring: Phoenicia, at peace with closed
//! borders, stood between (a 48-step road with every border open; diagnosed
//! by -60). Phoenicia itself, adjacent and 1.7-1.9 times outgunned, went
//! unpunished.

use super::AdvancedAi;
use crate::game::Game;
use std::collections::{BTreeSet, VecDeque};

impl AdvancedAi {
    /// Whether a land path from one of our cities reaches a tile beside one
    /// of `rival`'s cities, crossing only unowned land, our own, the
    /// rival's, any civilization's we are at war with, and territory whose
    /// borders are open to us. Water, impassable tiles and closed borders
    /// stop the march; units do not.
    pub(super) fn rival_reachable_by_land(&self, g: &Game, pid: usize, rival: usize) -> bool {
        Self::reachable_by_land_closing(g, pid, rival, None)
    }

    /// Whether our war on `other` holds the only land road to `rival`: open
    /// with the war, shut by `other`'s borders without it.
    pub(super) fn holds_the_road_to(
        &self,
        g: &Game,
        pid: usize,
        other: usize,
        rival: usize,
    ) -> bool {
        rival != other
            && g.is_at_war(pid, other)
            && Self::reachable_by_land_closing(g, pid, rival, None)
            && !Self::reachable_by_land_closing(g, pid, rival, Some(other))
    }

    /// `denial-needs-a-road`, Domination lane: whether the war on `other`
    /// holds the only land road to the front, the plan's target or the
    /// actionable denial rival, so peace with `other` would shut it. Live
    /// King civvis-20261003T164758Z (game 43) offered Egypt peace at turn
    /// 164 ("no siege against them is feasible ... the tide has run against
    /// us"). Egypt's land was the only road to the Aztecs, the denial target,
    /// and the army then mustered for Tenochtitlan for thirty turns without
    /// a path to its ring (diagnosed by -60).
    pub(super) fn war_holds_the_road(&self, g: &Game, pid: usize, other: usize) -> bool {
        if !self.denial_needs_a_road
            || self.active_victory_target(g) != Some(super::VictoryTarget::Domination)
            || !g.is_at_war(pid, other)
        {
            return false;
        }
        let mut targets: BTreeSet<usize> = BTreeSet::new();
        targets.extend(self.one_war_front());
        targets.extend(self.plan.as_ref().and_then(|plan| plan.target_player));
        targets.extend(
            self.actionable_victory_denial(g, pid)
                .map(|(rival, _)| rival),
        );
        targets
            .into_iter()
            .any(|rival| self.holds_the_road_to(g, pid, other, rival))
    }

    /// The land search behind [`Self::rival_reachable_by_land`], with
    /// `closed`'s ground read as at peace with us.
    fn reachable_by_land_closing(
        g: &Game,
        pid: usize,
        rival: usize,
        closed: Option<usize>,
    ) -> bool {
        let passable = |pos: crate::Pos| {
            let Some(tile) = g.map.get(pos) else {
                return false;
            };
            if !g.rules.is_passable(tile) || g.rules.is_water(tile) {
                return false;
            }
            match tile
                .owner_city
                .and_then(|cid| g.cities.get(&cid))
                .map(|city| city.owner)
            {
                None => true,
                Some(owner) => {
                    owner == pid
                        || owner == rival
                        || (g.is_at_war(pid, owner) && Some(owner) != closed)
                        || g.has_open_borders(pid, owner)
                }
            }
        };
        let goals: BTreeSet<crate::Pos> = g
            .player_city_ids(rival)
            .into_iter()
            .flat_map(|cid| {
                let pos = g.cities[&cid].pos;
                std::iter::once(pos).chain(g.nbrs(pos))
            })
            .collect();
        if goals.is_empty() {
            return false;
        }
        let mut seen: BTreeSet<crate::Pos> = BTreeSet::new();
        let mut queue: VecDeque<crate::Pos> = VecDeque::new();
        for cid in g.player_city_ids(pid) {
            let pos = g.cities[&cid].pos;
            if seen.insert(pos) {
                queue.push_back(pos);
            }
        }
        while let Some(pos) = queue.pop_front() {
            if goals.contains(&pos) {
                return true;
            }
            for next in g.nbrs(pos) {
                if !seen.contains(&next) && passable(next) {
                    seen.insert(next);
                    queue.push_back(next);
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AdvancedAi, VictoryTarget};
    use crate::game::Game;

    /// A rival behind a third party's closed borders is unreachable; open
    /// borders or a war on the third party opens the road.
    #[test]
    fn a_rival_behind_closed_borders_has_no_road() {
        let (mut g, ai) = strip();
        assert!(!g.has_open_borders(0, 1), "fixture: closed borders");
        assert!(!ai.rival_reachable_by_land(&g, 0, 2));
        assert!(ai.rival_reachable_by_land(&g, 0, 1), "the screen itself");
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        assert!(ai.rival_reachable_by_land(&g, 0, 2), "a war opens the road");
    }

    /// A land strip: us at the west end, a civilization with closed borders
    /// across the middle, the target at the east end.
    fn strip() -> (Game, AdvancedAi) {
        let mut g = Game::new_full(3, 40, 12, 931_036, 300, 0, false);
        for unit in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(unit);
        }
        for tile in g.map.tiles.values_mut() {
            // A strip of land, closed at both ends so the wrap cannot go
            // around the middle civilization.
            tile.terrain = if (4..=8).contains(&tile.pos.1) && (2..=36).contains(&tile.pos.0) {
                crate::name!("grassland")
            } else {
                crate::name!("ocean")
            };
            tile.feature = None;
            tile.hills = false;
        }
        let ours = g.found_city_for(0, (4, 6), None);
        let screen = g.found_city_for(1, (18, 6), None);
        g.found_city_for(2, (32, 6), None);
        // The middle civilization's land spans the whole strip.
        for tile in g.map.tiles.values_mut() {
            if (14..=22).contains(&tile.pos.0) && (4..=8).contains(&tile.pos.1) {
                tile.owner_city = Some(screen);
            }
        }
        let _ = ours;
        // Past Early Empire: the middle civilization closes its borders.
        g.players[1].borders_enforced = Some(true);
        (g, AdvancedAi::targeting(VictoryTarget::Domination))
    }

    /// See `war_holds_the_road`: the war on the middle civilization is the
    /// only road to the target behind it, and is not a war to close.
    #[test]
    fn the_war_that_holds_the_road_is_kept() {
        let (mut g, _) = strip();
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_denial_needs_a_road();
        assert!(ai.holds_the_road_to(&g, 0, 1, 2));
        ai.plan = Some(super::super::StrategicPlan {
            strategy: super::super::GrandStrategy::Conquest,
            target_player: Some(2),
            target_city: g.player_city_ids(2).first().copied(),
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        });
        assert!(ai.war_holds_the_road(&g, 0, 1));
        assert!(ai.second_front_war_kept(&g, 0, 1));
        // Open borders leave another road: the war holds nothing.
        g.players[1].borders_enforced = Some(false);
        assert!(!ai.holds_the_road_to(&g, 0, 1, 2));
        assert!(!ai.war_holds_the_road(&g, 0, 1));
    }
}
