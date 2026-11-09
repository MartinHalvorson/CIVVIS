//! Native Ski Resorts are built from adjacent land, never by entering a mountain.
//! Expansion2_Improvements.xml:88 sets BuildOnAdjacentPlot=true; :36 forbids
//! adjacent resorts. The ordinary Builder job walker targets the improvement
//! tile itself, so the live mountain traversal constraint leaves these jobs idle.

use super::*;
use std::collections::{BTreeSet, VecDeque};

const MAX_STAGING_STEPS: usize = 6;

impl AdvancedAi {
    /// Jobs for spare live Builders: (board unit, mountain, next land step).
    /// A missing step means build from the current adjacent tile. The Native
    /// bridge supplies idle/blocked identities and translates coordinates;
    /// this policy owns the objective, sites, routes, danger and reservations.
    pub fn native_culture_ski_resort_jobs(
        &self,
        g: &Game,
        pid: usize,
        busy: &BTreeSet<u32>,
        blocked: &BTreeSet<Pos>,
    ) -> Vec<(u32, Pos, Option<Pos>)> {
        if self.active_victory_target(g) != Some(VictoryTarget::Culture)
            || !g.players[pid]
                .civics
                .contains(&crate::name!("professional_sports"))
        {
            return Vec::new();
        }
        let ski = crate::name!("ski_resort");
        let sites: Vec<_> = g
            .map
            .tiles
            .iter()
            .filter_map(|(&pos, tile)| {
                (tile.terrain == "mountain"
                    && tile.improvement.is_none()
                    && tile
                        .owner_city
                        .and_then(|cid| g.cities.get(&cid))
                        .is_some_and(|c| c.owner == pid)
                    && g.tile_appeal(pos) > 0
                    && g.valid_improvements(pid, pos).contains(&ski))
                .then_some(pos)
            })
            .collect();
        if sites.is_empty() {
            return Vec::new();
        }
        // Include the live civilian lessons (including remembered raiders),
        // and never put this optional job within a visible enemy's next move.
        let visible = self.battlefront_visibility(g, pid);
        let hostiles: Vec<_> = g
            .units
            .values()
            .filter(|u| {
                u.owner != pid
                    && g.is_at_war(pid, u.owner)
                    && g.rules.units[u.kind].class == "military"
                    && g.sees(&visible, u.pos)
                    && g.unit_visible_to(u.id, pid)
            })
            .map(|u| (u.pos, g.unit_max_moves(u.id).ceil() as i32 + 2))
            .collect();
        let mut reserved_sites = BTreeSet::new();
        let mut reserved_steps = BTreeSet::new();
        let mut jobs = Vec::new();
        for unit in g.units.values().filter(|u| {
            u.owner == pid
                && u.kind == "builder"
                && u.charges > 0
                && u.moves_left > 0.0
                && !busy.contains(&u.id)
                && g.map
                    .get(u.pos)
                    .is_some_and(|t| !g.rules.is_water(t) && t.terrain != "mountain")
        }) {
            let reach = self.barbarian_reach(g, pid, unit.pos, MAX_STAGING_STEPS as i32 + 8);
            let safe = |pos| {
                self.civilian_safe_at(g, pid, unit.id, pos, &reach)
                    && !hostiles
                        .iter()
                        .any(|&(at, radius)| g.wdist(pos, at) <= radius)
            };
            if !safe(unit.pos) {
                continue;
            }
            // Owned land only: no speculative fog, embarkation, closed border,
            // mountain traversal or occupation of another civilian's tile.
            let walkable = |pos| {
                !blocked.contains(&pos)
                    && !reserved_steps.contains(&pos)
                    && g.map.get(pos).is_some_and(|t| {
                        t.terrain != "mountain"
                            && !g.rules.is_water(t)
                            && t.owner_city
                                .and_then(|cid| g.cities.get(&cid))
                                .is_some_and(|c| c.owner == pid)
                    })
                    && g.unit_can_traverse(unit.id, pos)
                    && safe(pos)
                    && !g.units.values().any(|other| {
                        other.id != unit.id
                            && other.pos == pos
                            && g.rules.units[other.kind].class != "military"
                    })
            };
            let mut queue = VecDeque::from([(unit.pos, 0usize, None)]);
            let mut seen = BTreeSet::from([unit.pos]);
            let mut best: Option<(f64, Pos, Option<Pos>)> = None;
            while let Some((staging, steps, first)) = queue.pop_front() {
                for &site in &sites {
                    if g.wdist(staging, site) != 1
                        || reserved_sites
                            .iter()
                            .any(|&other| g.wdist(site, other) <= 1)
                    {
                        continue;
                    }
                    let value = g.tile_appeal(site) as f64 / (steps + 1) as f64;
                    if best.as_ref().is_none_or(|&(old, old_site, old_first)| {
                        value > old || (value == old && (site, first) < (old_site, old_first))
                    }) {
                        best = Some((value, site, first));
                    }
                }
                if steps == MAX_STAGING_STEPS {
                    continue;
                }
                for next in g.nbrs(staging) {
                    if seen.contains(&next)
                        || !walkable(next)
                        || (steps == 0 && !g.can_move(unit.id, next))
                    {
                        continue;
                    }
                    seen.insert(next);
                    queue.push_back((next, steps + 1, first.or(Some(next))));
                }
            }
            if let Some((_, site, first)) = best {
                reserved_sites.insert(site);
                reserved_steps.insert(first.unwrap_or(unit.pos));
                jobs.push((unit.id, site, first));
            }
        }
        jobs
    }
}

#[cfg(test)]
mod tests;
