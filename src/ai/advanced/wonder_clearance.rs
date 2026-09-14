//! Keep one valuable wonder settlement on the live agenda while a small land
//! force clears its approach. This runs without the optional objective board.
//! Civilian safety, recovery, and immediate combat remain authoritative.

use super::AdvancedAi;
use crate::game::Game;
use crate::Pos;

const CLEARANCE_TURNS: u32 = 16;
const MIN_SITE_VALUE: f64 = 80.0;
const FORCE_REACH: i32 = 10;

#[derive(Clone, Debug)]
pub(super) struct Clearance {
    pub site: Pos,
    pub until: u32,
    pub units: Vec<u32>,
}

impl AdvancedAi {
    fn clearance_site_legal(&self, g: &Game, pid: usize, uid: u32, site: Pos) -> bool {
        g.units
            .get(&uid)
            .is_some_and(|u| u.owner == pid && u.kind == "settler")
            && g.players[pid].explored.contains(&site)
            && !g.blocked_city_sites.contains(&site)
            && !self.settler_site_is_dead(uid, site)
            && !self.settler_capture_scars.contains_key(&site)
            && self.early_settler_site_allowed(g, pid, uid, site)
            && !self.city_state_settlement_exclusion(g, pid).contains(&site)
            && !g.cities.values().any(|c| g.wdist(c.pos, site) < 4)
            && g.map.get(site).is_some_and(|tile| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && !g.tile_is_natural_wonder(tile)
                    && tile
                        .owner_city
                        .is_none_or(|cid| g.cities[&cid].owner == pid)
            })
    }

    pub(super) fn wonder_clearance_site(&self, g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        if !self.base.garrison_under_fire
            || g.players.iter().any(|p| {
                p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && g.is_at_war(pid, p.id)
            })
        {
            return None;
        }
        let request = self.wonder_clearance.get(&uid)?;
        (request.until > g.turn && self.clearance_site_legal(g, pid, uid, request.site))
            .then_some(request.site)
    }

    fn clearance_unit_available(&self, g: &Game, pid: usize, uid: u32, site: Pos) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return false;
        };
        let spec = &g.rules.units[unit.kind];
        if unit.owner != pid
            || spec.class != "military"
            || spec.siege
            || spec.promotion_class == "recon"
            || unit.hp < 70
            || !matches!(spec.domain.as_deref(), None | Some("land"))
            || g.is_embarked(unit)
            || unit.linked_to.is_some()
            || self.guard_is_bound_to_any_settler(uid)
            || self.builder_guard_reserved(uid)
            || g.wdist(unit.pos, site) > FORCE_REACH
        {
            return false;
        }
        let visible = g.player_vision_frame(pid);
        !g.player_city_ids(pid).iter().any(|cid| {
            g.wdist(g.cities[cid].pos, unit.pos) <= 2
                && Self::imminent_city_attack(g, pid, *cid, &visible)
        })
    }

    fn clearance_roster(&self, g: &Game, pid: usize, request: &Clearance) -> Vec<u32> {
        let mut candidates: Vec<_> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| self.clearance_unit_available(g, pid, *uid, request.site))
            .collect();
        candidates.sort_by_key(|uid| {
            (
                !request.units.contains(uid),
                g.wdist(g.units[uid].pos, request.site),
                *uid,
            )
        });
        let mut ranged = 0;
        let mut melee = 0;
        candidates
            .into_iter()
            .filter(|uid| {
                let spec = &g.rules.units[g.units[uid].kind];
                if Self::early_archers_shooter(spec) && ranged < 2 {
                    ranged += 1;
                    true
                } else if !spec.has_ranged_attack() && spec.is_melee_capable() && melee < 1 {
                    melee += 1;
                    true
                } else {
                    false
                }
            })
            .collect()
    }

    pub(super) fn refresh_wonder_clearance(&mut self, g: &Game, pid: usize) {
        // Retain expired entries for one additional window: a still-blocked
        // site cannot restart its wait indefinitely on every planning frame.
        self.wonder_clearance.retain(|uid, r| {
            g.units
                .get(uid)
                .is_some_and(|u| u.owner == pid && u.kind == "settler")
                && g.turn < r.until + g.standard_duration(CLEARANCE_TURNS)
        });
        let updates: Vec<_> = self
            .wonder_clearance
            .iter()
            .map(|(uid, r)| {
                let units = if self.wonder_clearance_site(g, pid, *uid).is_some() {
                    self.clearance_roster(g, pid, r)
                } else {
                    Vec::new()
                };
                (*uid, units)
            })
            .collect();
        for (uid, units) in updates {
            self.wonder_clearance.get_mut(&uid).unwrap().units = units;
        }
    }

    pub(super) fn reserve_wonder_clearance(
        &mut self,
        g: &Game,
        pid: usize,
        uid: u32,
        site: Pos,
    ) -> bool {
        if self.wonder_clearance_site(g, pid, uid) == Some(site) {
            return true;
        }
        if !self.base.garrison_under_fire
            || self.wonder_clearance.contains_key(&uid)
            || self.wonder_clearance.values().any(|r| r.until > g.turn)
            || !self.clearance_site_legal(g, pid, uid, site)
            || self.settle_value(g, pid, site) < MIN_SITE_VALUE
            || g.players.iter().any(|p| {
                p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && g.is_at_war(pid, p.id)
            })
        {
            return false;
        }
        let known_wonder = g.wdisk(site, 2).into_iter().any(|pos| {
            g.players[pid].explored.contains(&pos)
                && g.map
                    .get(pos)
                    .is_some_and(|tile| g.tile_is_natural_wonder(tile))
        });
        let visible = g.player_vision_frame(pid);
        let from = g.units[&uid].pos;
        let visible_barbarian = g.units.values().any(|u| {
            Some(u.owner) == g.barb_pid
                && g.rules.units[u.kind].class == "military"
                && g.sees(&visible, u.pos)
                && g.wdist(from, u.pos) + g.wdist(u.pos, site) <= g.wdist(from, site) + 3
        });
        if !known_wonder || !visible_barbarian {
            return false;
        }
        let mut request = Clearance {
            site,
            until: g.turn + g.standard_duration(CLEARANCE_TURNS),
            units: Vec::new(),
        };
        request.units = self.clearance_roster(g, pid, &request);
        // A promise with no available force would just trap the civilian.
        if request.units.len() < 3 {
            return false;
        }
        self.wonder_clearance.insert(uid, request);
        self.settler_threat_deferrals.remove(&site);
        crate::think!(self.journal(), Expansion, Decision,
            "Keep the wonder settlement at {site:?} and clear its approach";
            "two ranged defenders and one melee unit have a bounded clearing assignment; the Settler still refuses unsafe steps";
            site);
        true
    }

    pub(super) fn wonder_clearance_step(
        &mut self,
        g: &mut Game,
        pid: usize,
        uid: u32,
    ) -> Option<bool> {
        let (settler, request) = self
            .wonder_clearance
            .iter()
            .find(|(settler, r)| {
                r.units.contains(&uid) && self.wonder_clearance_site(g, pid, **settler).is_some()
            })
            .map(|(settler, r)| (*settler, r.clone()))?;
        if !self.clearance_unit_available(g, pid, uid, request.site) {
            return None;
        }
        let barb = g.barb_pid?;
        let visible = g.player_vision_frame(pid);
        let from = g.units[&settler].pos;
        let blocker = g
            .units
            .values()
            .filter(|u| {
                u.owner == barb
                    && g.rules.units[u.kind].class == "military"
                    && g.sees(&visible, u.pos)
                    && g.wdist(from, u.pos) + g.wdist(u.pos, request.site)
                        <= g.wdist(from, request.site) + 3
            })
            .min_by_key(|u| (g.wdist(from, u.pos), u.id))
            .map(|u| u.pos);
        let camp = g
            .barb_camps
            .keys()
            .filter(|pos| {
                g.players[pid].explored.contains(pos) && g.wdist(**pos, request.site) <= 4
            })
            .min_by_key(|pos| (g.wdist(**pos, request.site), **pos))
            .copied();
        let target = blocker.or(camp).unwrap_or(request.site);
        let available: Vec<_> = request
            .units
            .iter()
            .copied()
            .filter(|id| self.clearance_unit_available(g, pid, *id, request.site))
            .collect();
        let leader = available
            .iter()
            .find(|id| !g.rules.units[g.units[id].kind].has_ranged_attack())
            .copied();
        let anchor = leader.map_or(from, |id| g.units[&id].pos);
        let assembled = available.len() == 3
            && available
                .iter()
                .all(|id| g.wdist(g.units[id].pos, anchor) <= 3);
        if assembled && self.base.clear_adjacent_empty_barbarian_camp(g, pid, uid) {
            return Some(true);
        }
        let objective = if assembled { target } else { anchor };
        let range = if assembled {
            g.unit_attack_range(uid).max(1)
        } else {
            1
        };
        crate::think!(self.journal(), Military, Detail,
            "{} clears the wonder settlement approach", g.units[&uid].kind;
            "settlement {:?}; {}", request.site, if assembled { "the force is assembled" } else { "assembling the ranged pair and melee screen" };
            objective);
        Some(
            self.base
                .tactical_step(g, pid, uid, objective, &[barb], range),
        )
    }
}

#[cfg(test)]
mod tests;
