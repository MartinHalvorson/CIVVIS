//! `stranded-settler-studies-shipbuilding`: while a Settler can walk to no
//! legal city site, the research slot takes Shipbuilding (Sailing first),
//! whether or not any of our cities is on the coast.
//!
//! A land unit embarks only with Shipbuilding (`data/tree_effects.json`:
//! `"shipbuilding": {"land_unit_embark": 1}`), and the ordinary research
//! pricing reaches the naval chain only through
//! `BasicAi::water_research_goal`, which asks first whether a city of ours is
//! coastal. A landlocked capital therefore never values Shipbuilding, even
//! while its Settlers stand on the shore with nowhere left to walk.
//!
//! **Why (live Immortal, Rome Score, `civvis-20261115T232529Z`, 6p Continents
//! Online, cap 160).** Rome sat inland, walled in by a mountain range to the
//! east, Scotland to the west, the Maya to the north and two city-states to
//! the south. Puteoli fell in the t19-t28 war with Scotland and Lugdunum to
//! Maya Loyalty at t37. From then on the only free land (31 legal sites the
//! seat had seen, on the shelf beyond the range) lay behind Maya's closed
//! border (`passage-waits-past-the-opening` withholds that purchase: Maya
//! fielded 1.9-4.1x our military from t40 to t117) or across a few tiles of
//! coast. Two Settlers
//! were "stranded" at the shore from t79 to t117 (42 stranded turns, 39 in a
//! row) while research took Military Tactics, Astrology, Construction,
//! Military Engineering, Stirrups, Banking, Gunpowder, Engineering,
//! Machinery, Metal Casting and Astronomy; Sailing had been known since t39
//! and Shipbuilding, about four turns of our science, came at t118. The
//! first rival city on that shelf was founded at t136. Rome ended t160 with
//! one city, 277 points to the leader's 1,608. Of the twelve live games that
//! day with five or more stranded turns it was the only one stranded without
//! Shipbuilding; the other eleven had it at t40-89, before their first.
//!
//! The goal ranks behind the openings and the defensive walls, the stepping
//! stone and the edge unit, and ahead of every war, lane and bargain goal,
//! which resume the turn Shipbuilding lands. It reads the board fresh each
//! time the slot opens, so it is one-shot by construction: the technology,
//! once known, ends it, and so does a Settler that finds a site on foot.
//! Off, nothing changes.
use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;
use std::collections::HashSet;

/// The technology that lets a land unit embark.
pub(crate) const STRANDED_SETTLER_SEA_TECH: &str = "shipbuilding";

impl AdvancedAi {
    /// Shipbuilding as the research goal, or `None`: the gene is off, the
    /// seat already knows it, owns no Settler, knows no legal city site at
    /// all, or some Settler of ours can still walk to one (or stands on one).
    pub(super) fn stranded_settler_sea_goal(&self, g: &Game, pid: usize) -> Option<&'static str> {
        if !self.stranded_settler_studies_shipbuilding {
            return None;
        }
        let player = g.players.get(pid)?;
        if player.is_minor
            || player.is_barbarian
            || player.techs.contains(&crate::name!("shipbuilding"))
            || !g.rules.techs.contains_key(STRANDED_SETTLER_SEA_TECH)
        {
            return None;
        }
        let settlers: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| g.units.get(uid).is_some_and(|unit| unit.kind == "settler"))
            .collect();
        if settlers.is_empty() {
            return None;
        }
        let sites: HashSet<Pos> = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                !g.blocked_city_sites.contains(pos) && self.base.valid_settle_site(g, pid, *pos)
            })
            .collect();
        if sites.is_empty() {
            return None;
        }
        let walkable = settlers.iter().any(|uid| {
            let here = g.units[uid].pos;
            sites.contains(&here) || g.route_step_to_any(*uid, &sites).is_some()
        });
        (!walkable).then_some(STRANDED_SETTLER_SEA_TECH)
    }
}

#[cfg(test)]
mod tests;
