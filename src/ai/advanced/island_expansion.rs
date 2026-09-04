//! Exploration-led settlement across water.
//!
//! `island-exploration` creates the sea scout when the capital's connected
//! land has little room left. This companion keeps the discovery useful: the
//! next eligible Settler takes a known, viable foreign landfall before a
//! marginal home-island site consumes the remaining expansion window.

use super::{AdvancedAi, SETTLEMENT_SPACING, SETTLER_STEP_RISK_LIMIT};
use crate::game::Game;
use crate::Pos;
use std::collections::BTreeSet;

/// Do not leave the capital's landmass while it still has more than this many
/// independently usable city sites. The matching exploration policy lives in
/// `BasicAi`; this duplicate is intentionally local because the advanced
/// scorer owns the actual candidate, Loyalty, and risk judgement.
const OVERSEAS_HOME_SITE_LIMIT: usize = 2;

impl AdvancedAi {
    /// A discovered foreign settlement site that is closer than the empire's
    /// other valid overseas choices. The normal site scorer supplies legality
    /// and value; this layer adds the information boundary, landmass boundary,
    /// active-settler reservations, and a travel-first ordering so the colony
    /// is founded while the opportunity is still open.
    pub(super) fn overseas_settlement_target(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        avoid: Option<Pos>,
        home_landmass: &BTreeSet<Pos>,
    ) -> Option<Pos> {
        let water_world = g.map_script == crate::setup::MapScript::WaterWorld;
        if (!self.overseas_settlement && !water_world)
            || home_landmass.is_empty()
            || (!water_world && self.home_landmass_has_settlement_room(g, pid, uid, home_landmass))
        {
            return None;
        }
        let from = g.units.get(&uid)?.pos;
        let visible = self.battlefront_visibility(g, pid);
        self.settle_sites(g, pid, from, g.map.width + g.map.height)
            .into_iter()
            .filter(|(pos, _)| !home_landmass.contains(pos))
            // A foreign site becomes a target only after the player knows the
            // tile. The native map may be complete; the treatment must not be.
            .filter(|(pos, _)| g.players[pid].explored.contains(pos))
            .filter(|(pos, _)| Some(*pos) != avoid)
            .filter(|(pos, _)| !self.settler_site_is_dead(uid, *pos))
            .filter(|(pos, _)| {
                !self.settler_threat_detour_on() || !self.settler_threat_deferrals.contains_key(pos)
            })
            .filter(|(pos, _)| {
                !self
                    .settler_targets
                    .iter()
                    .any(|(other, target)| *other != uid && *target == *pos)
            })
            .filter(|(pos, _)| *pos == from || g.route_step(uid, *pos, 0).is_some())
            .filter(|(pos, _)| {
                !self.settlement_safety
                    || self.settlement_tile_risk(g, pid, Some(uid), *pos, &visible)
                        <= SETTLER_STEP_RISK_LIMIT
            })
            .filter(|(pos, _)| {
                if self.base.loyalty_rate_alarm {
                    self.settle_site_loyalty_verdict(g, pid, *pos).is_none()
                } else {
                    self.settle_site_frontier_loyalty_verdict(g, pid, *pos)
                        .is_none()
                }
            })
            .min_by(|left, right| {
                g.wdist(from, left.0)
                    .cmp(&g.wdist(from, right.0))
                    .then_with(|| right.1.total_cmp(&left.1))
                    .then_with(|| left.0.cmp(&right.0))
            })
            .map(|(pos, _)| pos)
    }

    /// Whether the home landmass still contains more than the small number of
    /// independent sites that should delay a colony expedition.
    fn home_landmass_has_settlement_room(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        home_landmass: &BTreeSet<Pos>,
    ) -> bool {
        let Some(unit) = g.units.get(&uid) else {
            return true;
        };
        let mut seats = Vec::new();
        for (pos, _) in self.settle_sites(g, pid, unit.pos, g.map.width + g.map.height) {
            if home_landmass.contains(&pos)
                && g.players[pid].explored.contains(&pos)
                && seats
                    .iter()
                    .all(|taken| g.wdist(*taken, pos) >= SETTLEMENT_SPACING)
            {
                seats.push(pos);
                if seats.len() > OVERSEAS_HOME_SITE_LIMIT {
                    return true;
                }
            }
        }
        false
    }
}
