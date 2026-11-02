//! `old-techs-before-the-beeline`: the research slot goes back for old
//! technologies before it walks the next step of a long beeline. Two arms,
//! both judged against the world era (the host's `world_era`, so live and
//! native read the same clock):
//!
//! 1. **Army upgrades.** A land combat unit we own whose next upgrade is
//!    locked behind a technology from an era the world has already left
//!    (`tech era + ARMY_UPGRADE_ERAS_BEHIND <= world era`) makes that
//!    technology the goal. The one locking the most such units wins (ties:
//!    the cheaper path). Recon units are left to the explorers.
//! 2. **Old-era backlog.** Any legal technology two or more eras behind the
//!    world (`tech era + BACKLOG_ERAS_BEHIND <= world era`) is researched,
//!    cheapest first, until none is left. That is "claiming the previous
//!    eras back after some time": a skipped tech gets one whole world era of
//!    grace, then comes due.
//!
//! The rule sits in the forced-goal chain directly ahead of the air surge's
//! beeline (behind the walls, openings, conversion, appointed war, faith,
//! decisive-window and siege-tier arms), so it also ranks ahead of every lane
//! beeline below the air surge. It yields when the Bomber's own technology is
//! the next legal step (`air_surge_missing_techs <= 1`) and while a committed
//! launch chain owns research: `first-launch-keeps-research`'s first launch
//! after Rocketry, or the endgame after the Moon Landing.
//!
//! **Why (operator, 2026-11-02, live Lv 6 Emperor civvis-20261102T163456Z,
//! Inca).** The air-surge beeline admits every legal step toward Advanced
//! Flight past the rolling three-era window, and it ranks above the wartime
//! modernization, so research reached Flight t127, Radio t132 and Advanced
//! Flight t138 while Iron Working waited until t116 and Machinery until t139
//! (Ballistics t146, Rifling t151). The world entered the Medieval era at t61,
//! the Renaissance at t82, the Industrial at t102 and the Modern at t125:
//! under this rule Iron Working (Classical) comes due at t61, Machinery
//! (Medieval) at t82, and every Medieval technology at t102. With no Swordsman or
//! Crossbowman unlocked, the seat bought 12 Warriors with Gold (t81-114) and
//! 12 Archers with Faith (t121-138) as its capture bodies and ranged line for
//! the siege of Hattin, which it never took; Archers and a Warrior were still
//! in the field at t175, in the Information era. Operator: "We probably
//! shouldn't be teching so far ahead to bombers that we can't upgrade archers
//! or warriors. We should also be claiming all techs from previous eras back
//! after some time."

use super::*;
use std::collections::BTreeSet;

/// An owned unit's locked upgrade comes due once the world is this many eras
/// past the unlocking technology's era (one: Iron Working, a Classical
/// technology, from the Medieval era on; Machinery from the Renaissance on).
pub(crate) const ARMY_UPGRADE_ERAS_BEHIND: usize = 1;

/// Every technology this many eras behind the world is backlog to clear (two:
/// the Ancient era from the Medieval on, the Medieval from the Industrial on).
pub(crate) const BACKLOG_ERAS_BEHIND: usize = 2;

/// A Bomber this close keeps the slot: its technology is the next legal step.
pub(crate) const OLD_TECHS_YIELD_WITHIN_BOMBER_TECHS: usize = 1;

/// Which arm named the technology.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OldTechReason {
    /// `units` of ours wait on the technology to upgrade into `successor`.
    ArmyUpgrade {
        held: Name,
        successor: Name,
        units: usize,
    },
    /// A technology `eras` eras behind the world.
    Backlog { eras: usize },
}

/// The rule's pick: the goal technology and why.
#[derive(Clone, Copy, Debug)]
pub(crate) struct OldTechPick {
    pub(crate) tech: Name,
    pub(crate) reason: OldTechReason,
}

impl AdvancedAi {
    /// The old technology to research ahead of the beeline, or `None`: off,
    /// the Bomber's technology is the next step, a committed launch chain owns
    /// the endgame, or nothing is overdue.
    pub(super) fn old_techs_before_the_beeline_goal(
        &self,
        g: &Game,
        pid: usize,
        endgame_committed: bool,
    ) -> Option<OldTechPick> {
        if !self.old_techs_before_the_beeline || endgame_committed {
            return None;
        }
        if self.air_surge_research_goal(g, pid).is_some()
            && Self::air_surge_missing_techs(g, pid) <= OLD_TECHS_YIELD_WITHIN_BOMBER_TECHS
        {
            return None;
        }
        Self::overdue_army_upgrade_tech(g, pid).or_else(|| Self::overdue_backlog_tech(g, pid))
    }

    /// Arm 1: the technology that unlocks the next upgrade of the most land
    /// combat units we own, among those from an era the world has left.
    pub(crate) fn overdue_army_upgrade_tech(g: &Game, pid: usize) -> Option<OldTechPick> {
        let world_era = g.world_era;
        let known = &g.players[pid].techs;
        // tech -> (units waiting, one held kind, its successor)
        let mut waiting: BTreeMap<Name, (usize, Name, Name)> = BTreeMap::new();
        for unit in g.units.values().filter(|unit| unit.owner == pid) {
            let Some(held) = g.rules.units.get(&unit.kind) else {
                continue;
            };
            if held.class != "military"
                || held.promotion_class == "recon"
                || matches!(held.domain.as_deref(), Some("sea" | "air"))
            {
                continue;
            }
            let Some(base) = held.upgrade_to else {
                continue;
            };
            let successor = g.player_unit_replacement(pid, base);
            let Some(spec) = g.rules.units.get(&successor) else {
                continue;
            };
            let Some(tech) = spec.tech else { continue };
            if !spec.buildable || spec.civic.is_some() || known.contains(&tech) {
                continue;
            }
            let Some(era) = g.rules.techs.get(&tech).map(|spec| spec.era) else {
                continue;
            };
            if era + ARMY_UPGRADE_ERAS_BEHIND > world_era {
                continue;
            }
            let gain = spec.strength.max(spec.ranged_attack_strength())
                - held.strength.max(held.ranged_attack_strength());
            if gain <= 0.0 {
                continue;
            }
            let entry = waiting.entry(tech).or_insert((0, unit.kind, successor));
            entry.0 += 1;
        }
        waiting
            .into_iter()
            .max_by(|(a, (an, ..)), (b, (bn, ..))| {
                an.cmp(bn).then_with(|| {
                    Self::old_tech_path_cost(g, pid, *b)
                        .total_cmp(&Self::old_tech_path_cost(g, pid, *a))
                        .then_with(|| b.cmp(a))
                })
            })
            .map(|(tech, (units, held, successor))| OldTechPick {
                tech,
                reason: OldTechReason::ArmyUpgrade {
                    held,
                    successor,
                    units,
                },
            })
    }

    /// Arm 2: the cheapest legal technology two or more eras behind the world.
    pub(crate) fn overdue_backlog_tech(g: &Game, pid: usize) -> Option<OldTechPick> {
        let world_era = g.world_era;
        g.available_techs(pid)
            .into_iter()
            .filter_map(|tech| {
                let spec = g.rules.techs.get(&tech)?;
                (!spec.repeatable && spec.era + BACKLOG_ERAS_BEHIND <= world_era).then_some((
                    tech,
                    spec.cost,
                    world_era - spec.era,
                ))
            })
            .min_by(|(a, ac, _), (b, bc, _)| ac.total_cmp(bc).then_with(|| a.cmp(b)))
            .map(|(tech, _, eras)| OldTechPick {
                tech,
                reason: OldTechReason::Backlog { eras },
            })
    }

    /// The research still owed for `tech` and its unknown ancestors.
    fn old_tech_path_cost(g: &Game, pid: usize, tech: Name) -> f64 {
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
        Self::engineering_path_cost(g, pid, &missing)
    }

    /// Journal the rule's pick (only when it owns the slot) and return the
    /// technology for `goal_pick` to walk.
    pub(super) fn old_tech_pick(&self, g: &Game, pick: &OldTechPick) -> &'static str {
        let era_name = |tech: Name| {
            g.rules
                .techs
                .get(&tech)
                .and_then(|spec| crate::rules::ERA_NAMES.get(spec.era))
                .copied()
                .unwrap_or("?")
        };
        match pick.reason {
            OldTechReason::ArmyUpgrade {
                held,
                successor,
                units,
            } => {
                think!(self.journal(), Research, Decision, "The army's upgrade before the beeline";
                       "old-techs-before-the-beeline: {} {} unit(s) wait on {} ({} era) to upgrade into {}, and the world is in the {} era",
                       units, plain(held.as_str()), plain(pick.tech.as_str()), era_name(pick.tech),
                       plain(successor.as_str()),
                       crate::rules::ERA_NAMES.get(g.world_era).copied().unwrap_or("?"));
            }
            OldTechReason::Backlog { eras } => {
                think!(self.journal(), Research, Decision, "An old era's backlog before the beeline";
                       "old-techs-before-the-beeline: {} is a {} technology, {} eras behind the world; clearing the backlog before the next beeline step",
                       plain(pick.tech.as_str()), era_name(pick.tech), eras);
            }
        }
        pick.tech.as_str()
    }
}

#[cfg(test)]
mod tests;
