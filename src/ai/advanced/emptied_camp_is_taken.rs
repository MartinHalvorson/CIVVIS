//! `emptied-camp-is-taken` (opt-in): a barbarian camp with nothing standing on
//! it is walked onto this turn by a soldier that can reach it and live there,
//! before the battle plan's positions hold that soldier back.
//!
//! ## Why (live Tournament 6 and Lv 8 Deity games, 2026-11-21/22)
//!
//! Over 54 recorded games the seat destroyed a median of **zero** camps by
//! turn 100, although a camp stood on its board in most of them. Civ VI pays
//! for each one: 50 Gold, and an era moment worth 2, or 3 within six tiles of
//! one of our cities (`MOMENT_BARBARIAN_CAMP_DESTROYED[_NEAR_YOUR_CITY]` in
//! `Expansion1_Moments.xml`, repeatable, Ancient to Medieval). The first era
//! ended in a Dark Age in 23 of those 54 games at 2-7 points of the 8 needed,
//! so one camp near home would have turned most of them Normal. The two games
//! that destroyed two or three camps both had a Golden first age and six or
//! seven cities at turn 60.
//!
//! What held the camps up, in Gaul `civvis-t6-20261122T043340Z` (camp two
//! tiles from Noviodunum, t7-54): at t36 our Archer killed the Spearman on
//! the camp and left it empty, with two full-strength Gaesatae two tiles away.
//! Neither stepped in, and at t37 a Horseman walked onto it again. The
//! ClearCamp task force took its posture (`objective_board.rs`, always
//! Advance) and the `battle-planner-2` positions plan placed its members:
//! "a land force of 4 takes 1 slot(s), 1 unit(s) placed ... pace 2: no unit
//! ends nearer the target than 2". The pace floor keeps a force's leaders
//! level with its slowest member on the approach, so they stop short of the
//! camp, and the units the plan has ordered never reach the ladder, whose
//! barbarian responder does walk onto an adjacent empty camp
//! (`an_adjacent_empty_camp_is_cleared_instead_of_being_held_or_explored_past`).
//! Mali `civvis-20261121T044558Z` had a camp next to its capital from t9 to
//! t65: sixteen opening delays for barbarian pressure, a Settler idle from
//! t20 to t55, two cities at t60.
//!
//! ## What the gene does
//!
//! Inside `plan_battle`, after the kill plan's blows have landed (so a camp
//! they emptied counts) and before the heal rotation and the positions plan:
//! for every camp on our board whose tile we see this frame and on which no
//! unit of anyone else stands, the soldier of ours that
//!
//! - is a land military unit with moves left, not yet ordered by the plan,
//!   not in an active siege, and not the garrison of a city of ours;
//! - can reach the camp tile this turn (`Game::reachable`);
//! - would outlive what the danger field says can hit it there
//!   (`DangerField::upper_danger` below its hit points),
//!
//! walks onto it: the least danger first, then the nearest, then the lowest
//! id. The move is an ordinary `MoveTo`, so the engine clears the camp and
//! pays for it as for any other entry, and the unit is marked ordered so the
//! positions plan and the ladder leave it on the camp.
//!
//! Nothing changes with the gene off, without the battle planner, or while
//! no camp is empty: occupied camps are still the kill plan's and the
//! ladder's.

use super::battle_planner::DangerField;
use super::*;

impl AdvancedAi {
    /// `emptied-camp-is-taken`: walk a soldier onto every empty camp it can
    /// reach and survive on this turn. Returns how many camps it stepped on.
    pub(super) fn take_emptied_camps(
        &mut self,
        g: &mut Game,
        pid: usize,
        field: &mut DangerField,
    ) -> u32 {
        if !self.emptied_camp_is_taken || g.barb_camps.is_empty() {
            return 0;
        }
        let visible = self.battlefront_visibility(g, pid);
        let camps: Vec<Pos> = g.barb_camps.keys().copied().collect();
        let mut taken = 0u32;
        for camp in camps {
            if !g.barb_camps.contains_key(&camp) || !g.sees(&visible, camp) {
                continue;
            }
            if g.unit_ids_at(camp)
                .iter()
                .any(|other| g.units.get(other).is_some_and(|unit| unit.owner != pid))
            {
                continue;
            }
            let Some(uid) = self.emptied_camp_taker(g, pid, field, camp) else {
                continue;
            };
            let from = g.units[&uid].pos;
            if !self.base.path_walk_to(g, pid, uid, camp) {
                continue;
            }
            self.battle_planner_ordered.insert(uid);
            taken += 1;
            let cleared = !g.barb_camps.contains_key(&camp);
            think!(self.journal(), Military, Decision,
                "A soldier walks onto the empty camp at {:?}", camp;
                "emptied-camp-is-taken: nothing stands on it, the {} at {:?} reaches it this \
                 turn and outlives every blow that can land there; a camp pays 50 Gold and up \
                 to 3 Era Score{}",
                g.units.get(&uid).map_or("unit", |unit| unit.kind.as_str()), from,
                if cleared { "" } else { " (the host still has to confirm the entry)" };
                camp);
        }
        taken
    }

    /// The soldier that takes `camp`: least danger, then nearest, then id.
    fn emptied_camp_taker(
        &self,
        g: &Game,
        pid: usize,
        field: &mut DangerField,
        camp: Pos,
    ) -> Option<u32> {
        let mut best: Option<(i64, i32, u32)> = None;
        for (uid, unit) in &g.units {
            if unit.owner != pid
                || unit.moves_left <= 0.0
                || self.battle_planner_ordered.contains(uid)
                || g.is_embarked(unit)
            {
                continue;
            }
            let spec = &g.rules.units[unit.kind];
            if spec.class != "military" || spec.domain.as_deref().is_some_and(|d| d != "land") {
                continue;
            }
            // The garrison keeps its city: Civ VI stacks one soldier on a
            // City Center, so the unit there is the one holding it.
            if g.city_at(unit.pos)
                .is_some_and(|cid| g.cities.get(&cid).is_some_and(|city| city.owner == pid))
            {
                continue;
            }
            if self.active_siege_member(g, pid, *uid) {
                continue;
            }
            let distance = g.wdist(unit.pos, camp);
            if distance == 0 || distance > 6 {
                continue;
            }
            if !g.reachable(*uid).contains(&camp) {
                continue;
            }
            let danger = field.upper_danger(camp, *uid);
            if danger >= f64::from(unit.hp) {
                continue;
            }
            let key = ((danger * 10.0).round() as i64, distance, *uid);
            if best.is_none_or(|current| key < current) {
                best = Some(key);
            }
        }
        best.map(|(_, _, uid)| uid)
    }
}

#[cfg(test)]
mod tests;
