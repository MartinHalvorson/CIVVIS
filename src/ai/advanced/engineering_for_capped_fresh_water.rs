//! `engineering-for-capped-fresh-water`: once two of our cities stand within
//! one of their housing, Granary built (or none to build), no Aqueduct, and
//! the Aqueduct's technology would give each a legal Aqueduct site, the
//! research slot takes that technology (Engineering) when the whole path costs
//! at most [`ENGINEERING_MAX_RESEARCH_STANDARD`] standard turns of our current
//! science, priced on the host's own research quote. It ranks behind the openings, the defensive walls, the conversion
//! upgrade, the appointed war's breakthrough, the Prophet-race Astrology and
//! the decisive window's breaker, and ahead of the air surge, the production
//! technology goal and every later lane goal, which resume the turn it lands.
//!
//! **Why (2026-10-11 census, 25 live Immortal and 45 Emperor GC runs).** At
//! Immortal we research Engineering at a median t72 (p75 t96), but in 11 of
//! 25 runs two or more capped fresh-water cities with their Granary waited on
//! it first — and at that moment the rest of the path cost a median 1.6 live
//! turns of research (max 4.5; Emperor 12 of 45 runs, median 2.65). In six of
//! those runs it then landed 36-74 turns later (t124-t143) while the
//! conquest beeline walked Military Tactics, Stirrups, Banking, Cartography
//! and Mass Production toward Industrialization and Advanced Flight: 551
//! capped fresh-water city-turns at Immortal and 1,241 at Emperor (1,115 and
//! 1,934 counting every capped city with its Granary) sat at the host's ×0.25
//! or ×0.5 housing growth for want of a technology a turn or two away.
//! `housing-research` reads only when nothing else holds the slot and
//! `housing-research-2` sits last in the chain, so neither reached it.
//!
//! The rule is one-shot by construction — the technology, once known, ends it
//! — and cheap by gate: a path dearer than the cap leaves the beeline alone.
//! `aqueduct-claims-the-capped-queue` then builds the Aqueducts.

use super::*;
use std::collections::BTreeSet;

/// Capped cities with a legal Aqueduct site the rule needs before it buys the
/// technology: one capped city is a production problem, two are the empire's.
pub(crate) const ENGINEERING_MIN_CAPPED_CITIES: usize = 2;

/// The dearest path the rule will buy, in standard turns of our current
/// science (4 live turns at the seat's Online speed). The census's
/// qualifying turns cost a median 1.6 live turns at Immortal (max 4.5) and
/// 2.65 at Emperor, on the host's own quote.
pub(crate) const ENGINEERING_MAX_RESEARCH_STANDARD: u32 = 8;

/// The rule's pick: the technology, the Aqueduct it unlocks, how many capped
/// cities gain a legal site, and the turns of research it costs.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CappedEngineering {
    pub(crate) tech: Name,
    pub(crate) aqueduct: Name,
    pub(crate) cities: usize,
    pub(crate) turns: f64,
}

impl AdvancedAi {
    /// The Aqueduct's technology to research now, or `None`: off, already
    /// known, fewer than [`ENGINEERING_MIN_CAPPED_CITIES`] capped cities with
    /// their Granary and a legal Aqueduct site once it is known, or a path
    /// dearer than [`ENGINEERING_MAX_RESEARCH_STANDARD`] standard turns.
    pub(super) fn capped_fresh_water_engineering_goal(
        &self,
        g: &Game,
        pid: usize,
    ) -> Option<CappedEngineering> {
        if !self.engineering_for_capped_fresh_water {
            return None;
        }
        let aqueduct = BasicAi::civ_district(g, pid, "aqueduct");
        let tech = g.rules.districts.get(&aqueduct)?.tech?;
        if g.players[pid].techs.contains(&tech) {
            return None;
        }
        let candidates: Vec<u32> = g
            .player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                city.pop >= aqueduct_claims_the_capped_queue::CAPPED_AQUEDUCT_MIN_POP
                    && city.pop as f64 + 1.0 - g.city_housing(city) >= 0.0
                    && BasicAi::civ_building(g, pid, *cid, "granary").is_none()
                    && !g.city_has_district_family(city, crate::name!("aqueduct"))
            })
            .collect();
        if candidates.len() < ENGINEERING_MIN_CAPPED_CITIES {
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
        let science = Self::empire_science(g, pid).max(1.0);
        let turns = cost / science;
        if turns > g.standard_duration(ENGINEERING_MAX_RESEARCH_STANDARD) as f64 {
            return None;
        }
        // One private counterfactual, only once the cheap gates bind: the
        // host's own placement and construction rules decide the site.
        let mut future = g.clone();
        future.players[pid].techs.extend(missing.iter().copied());
        // The host's menu and plot lists are a snapshot of TODAY's legality:
        // before the technology the Aqueduct is in no city's menu, so the
        // positive gate in `can_produce` would refuse every site. Drop those
        // snapshots in the private copy and let the ruleset's placement model
        // decide; a host refusal (`blocked_production`) still stands.
        future.host_buildable = Default::default();
        future.host_district_plots = Default::default();
        future.host_district_sites = Default::default();
        let legal = candidates
            .iter()
            .filter(|cid| {
                future.district_sites(**cid, aqueduct).into_iter().any(|pos| {
                    future.can_produce(
                        pid,
                        **cid,
                        &Item::District {
                            district: aqueduct,
                            pos,
                        },
                    )
                })
            })
            .count();
        if legal < ENGINEERING_MIN_CAPPED_CITIES {
            return None;
        }
        Some(CappedEngineering {
            tech,
            aqueduct,
            cities: legal,
            turns,
        })
    }

    /// Journal the rule's pick (only when it owns the slot) and return the
    /// technology for `goal_pick` to walk.
    pub(super) fn capped_engineering_pick(&self, pick: &CappedEngineering) -> &'static str {
        think!(self.journal(), Research, Decision, "Engineering for the capped fresh-water cities";
               "engineering-for-capped-fresh-water: {} cities within one of their housing have their Granary and a legal {} site once {} is known, about {:.1} turns of research",
               pick.cities, plain(pick.aqueduct.as_str()), plain(pick.tech.as_str()), pick.turns);
        pick.tech.as_str()
    }

    /// The science still owed on `missing`: the host's own quote where the
    /// mirror carries it (cost less every beaker banked, partial research and
    /// earned boosts included, even on a node not being researched), else
    /// the ruleset price less the active node's progress; less the shared
    /// overflow bank.
    fn engineering_path_cost(g: &Game, pid: usize, missing: &BTreeSet<Name>) -> f64 {
        let player = &g.players[pid];
        let owed: f64 = missing
            .iter()
            .map(|tech| {
                g.host_remaining_research_cost(pid, *tech)
                    .unwrap_or_else(|| {
                        let progress = if player.research.as_deref() == Some(tech.as_str()) {
                            player.research_progress
                        } else {
                            0.0
                        };
                        (g.tech_cost(tech.as_str()) - progress).max(0.0)
                    })
            })
            .sum();
        (owed - player.research_overflow).max(0.0)
    }
}

#[cfg(test)]
mod tests;
