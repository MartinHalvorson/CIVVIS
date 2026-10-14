//! `siege-tier-before-the-beeline`: while the Conquest campaign's target
//! stands behind walls of the Medieval tier or better (300+) and
//! `domination-siege-research-2` names the next fed siege design's technology,
//! the research slot takes that technology ahead of the air surge's beeline
//! when the whole path costs at most [`SIEGE_TIER_MAX_RESEARCH_STANDARD`]
//! standard turns of our science and the Bomber is still more than
//! [`SIEGE_TIER_YIELDS_WITHIN_TECHS`] technologies away. It ranks behind the
//! openings, the defensive walls, the conversion upgrade, the appointed war's
//! breakthrough, the Prophet-race Astrology, the decisive window's breaker and
//! `engineering-for-capped-fresh-water`; the beeline resumes the turn the
//! technology lands.
//!
//! **Why (2026-10-11 census, 54 live Immortal and 118 Emperor GC runs).** At
//! Immortal the visible rivals' median wall pool reaches 300 at t107 and 400
//! at t111 (a rival holds Siege Tactics by a median t74), but our Metal
//! Casting lands at a median t131 — 23 turns after the walls reach 400 — and
//! 21 of 54 runs never research it (16 reached t130 without it). At t125 our
//! best siege unit is a Trebuchet in 27 of 49 runs, a Catapult in 11 and
//! nothing in 7; the first Bombard comes at a median t138. Over t60-150, 444
//! research picks walked "the cheapest step toward advanced flight" against
//! 28 for a siege technology, yet Advanced Flight arrived by t150 in only 17
//! of 41 Immortal runs that reached t150 (Emperor 56 of 101). The armed
//! `domination-siege-research-2` goal sits below the air surge, the
//! production technology goal and wartime modernization in the forced-goal
//! chain, so it almost never owned the slot.
//!
//! The rule only promotes that goal: it buys nothing the siege goal would not
//! name (a fed design stronger than our best unlocked gun), and a path dearer
//! than the cap, or a Bomber two technologies away, leaves the beeline alone.

use super::*;
use std::collections::BTreeSet;

/// The wall pool the rule answers: the Medieval tier and above, the same line
/// as `domination-siege-research-2`'s upgrade.
pub(crate) const SIEGE_TIER_WALL_HP: i32 = 300;

/// The dearest path the rule will buy ahead of the beeline, in standard turns
/// of our current science (8 live turns at the seat's Online speed).
pub(crate) const SIEGE_TIER_MAX_RESEARCH_STANDARD: u32 = 16;

/// A Bomber this close keeps the slot: the beeline is about to deliver the
/// wing that opens walls of any tier.
pub(crate) const SIEGE_TIER_YIELDS_WITHIN_TECHS: usize = 2;

/// The rule's pick: the siege technology, the walls it answers, the turns of
/// research the path costs and how far the Bomber still is.
#[derive(Clone, Copy, Debug)]
pub(crate) struct SiegeTierPick {
    pub(crate) tech: Name,
    pub(crate) walls: i32,
    pub(crate) turns: f64,
    pub(crate) bomber_techs: usize,
}

impl AdvancedAi {
    /// The siege technology to research ahead of the air surge's beeline, or
    /// `None`: off, no siege goal from `domination-siege-research-2`, the
    /// target's walls below [`SIEGE_TIER_WALL_HP`], the Bomber within
    /// [`SIEGE_TIER_YIELDS_WITHIN_TECHS`] technologies, or a path dearer than
    /// [`SIEGE_TIER_MAX_RESEARCH_STANDARD`] standard turns.
    pub(super) fn siege_tier_before_the_beeline_goal(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
        siege_goal: Option<Name>,
    ) -> Option<SiegeTierPick> {
        if !self.siege_tier_before_the_beeline {
            return None;
        }
        let tech = siege_goal?;
        if g.players[pid].techs.contains(&tech) {
            return None;
        }
        let city = g.cities.get(&plan.target_city?)?;
        let walls = city.wall_hp.max(g.city_max_wall_hp(city));
        if walls < SIEGE_TIER_WALL_HP {
            return None;
        }
        let bomber_techs = Self::air_surge_missing_techs(g, pid);
        if bomber_techs <= SIEGE_TIER_YIELDS_WITHIN_TECHS {
            return None;
        }
        let missing: BTreeSet<Name> = g
            .rules
            .tech_ancestors
            .get(tech.as_str())
            .into_iter()
            .flatten()
            .map(|ancestor| Name::new(ancestor))
            .chain(std::iter::once(tech))
            .filter(|needed| !g.players[pid].techs.contains(needed))
            .collect();
        let cost = Self::engineering_path_cost(g, pid, &missing);
        let turns = cost / Self::empire_science(g, pid).max(1.0);
        if turns > g.standard_duration(SIEGE_TIER_MAX_RESEARCH_STANDARD) as f64 {
            return None;
        }
        Some(SiegeTierPick {
            tech,
            walls,
            turns,
            bomber_techs,
        })
    }

    /// Journal the rule's pick (only when it owns the slot) and return the
    /// technology for `goal_pick` to walk.
    pub(super) fn siege_tier_pick(&self, pick: &SiegeTierPick) -> &'static str {
        think!(self.journal(), Research, Decision, "The siege tier before the beeline";
               "siege-tier-before-the-beeline: {} unlocks a stronger gun against {}-HP walls in about {:.1} turns of research, while the Bomber is {} technologies away",
               plain(pick.tech.as_str()), pick.walls, pick.turns, pick.bomber_techs);
        pick.tech.as_str()
    }
}

#[cfg(test)]
mod tests;
