//! `culture-seat-wonder-lane`: on a seat aiming at a Culture victory, the
//! empire's strongest unthreatened cities keep a wonder in production, one
//! lane per `CULTURE_WONDER_CITIES_PER_LANE` cities, each lane on the
//! wonder that pays the Culture victory most among those it can finish in
//! `CULTURE_WONDER_MAX_TURNS`.
//!
//! **Why (live King Pericles, 2026-10-30, civvis-20261030T055006Z).**
//! - We finished one wonder against Gaul's twelve and Persia's eleven, and
//!   lost the Culture race to a rival at t180. Six to eleven wonders were on
//!   offer from t67 (Stonehenge from t22); the first we started was Petra at
//!   t148.
//! - `lane-delegates-production-2` hands a Culture seat's routine queues to
//!   the baseline governor, whose `fallback_wonder` is its last fallback: a
//!   city builds a wonder only after every building it can build. The
//!   strategic scorer's Culture plan pays a wonder only its ordinary value
//!   and +320 (the live race's +1 500 and the strategic and tally prices all
//!   need `!lane_opens`, which a Culture plan sets), so neither path starts
//!   one while a Library or a Builder is on the menu.
//! - Every wonder is two tourism and its own effects
//!   (`tourism_components_per_turn_with_sources`), and several hold Great
//!   Works. On the native screen the delegated governor's Culture wins carry
//!   11-21 wonders and its losses 0-2.
//!
//! **What the gene does.** For a Culture seat, a lane city (ranked by
//! production, unthreatened, two buildings or more, no wonder already
//! queued) claims the best wonder it can finish in time: great-work slots,
//! tourism and culture first, then the cheapest. An idle queue, or one over a
//! deferrable head that keeps its progress; the heads the Theater claim and
//! the other claims hold keep their place. The delegated governor's
//! rescoring leaves the claimed wonder alone ([`AdvancedAi::lane_wonder_holds`]).
//!
//! Off, or for a seat not aiming at Culture, nothing changes.

use super::*;

/// Build turns past which a lane city does not start a wonder: a King rival
/// finishes the cheap ones first, and a wonder lost to a rival is a lane
/// spent for a refund.
pub(super) const CULTURE_WONDER_MAX_TURNS: f64 = 20.0;

/// Cities per concurrent wonder lane; the first lane opens at the first city.
pub(super) const CULTURE_WONDER_CITIES_PER_LANE: usize = 4;

impl AdvancedAi {
    /// Whether a wonder lane runs for this seat's active target. The one
    /// switch every lane's gene adds its own arm to.
    pub(super) fn wonder_lane_on(&self, target: Option<VictoryTarget>) -> bool {
        self.culture_seat_wonder_lane && target == Some(VictoryTarget::Culture)
    }

    /// What `wonder` pays the lane `target` is playing for; `None` for a
    /// wonder the lane does not want. Culture: the two tourism every wonder
    /// earns, its own tourism, its great-work slots, its culture and its
    /// writer, artist and musician points.
    pub(super) fn wonder_lane_value(
        &self,
        target: VictoryTarget,
        spec: &crate::rules::WonderSpec,
    ) -> Option<f64> {
        match target {
            VictoryTarget::Culture => Some(
                2.0 + spec.effects.get("tourism").copied().unwrap_or(0.0)
                    + 2.0 * spec.great_work_slots.values().sum::<i32>().max(0) as f64
                    + spec.yields.culture
                    + ["writer", "artist", "musician"]
                        .iter()
                        .filter_map(|kind| spec.great_person_points.get(*kind))
                        .sum::<f64>(),
            ),
            _ => None,
        }
    }

    /// The wonder `cid` would claim for its lane, its build turns and its
    /// value: the most valuable the city can finish in
    /// `CULTURE_WONDER_MAX_TURNS`, cheapest on a tie, that no city of ours is
    /// already building and the host has not refused here.
    pub(super) fn lane_wonder_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        target: VictoryTarget,
    ) -> Option<(Item, f64)> {
        let refused = g.blocked_wonders.get(&cid);
        let ours_queued: Vec<Name> = g
            .player_city_ids(pid)
            .into_iter()
            .filter_map(|other| match g.cities[&other].queue.first() {
                Some(Item::Wonder { wonder, .. }) => Some(*wonder),
                _ => None,
            })
            .collect();
        let production = g.city_yields(cid).production.max(0.5);
        g.producible_items(pid, cid)
            .into_iter()
            .filter_map(|item| {
                let Item::Wonder { wonder, .. } = &item else {
                    return None;
                };
                if ours_queued.contains(wonder)
                    || refused.is_some_and(|blocked| blocked.contains(wonder))
                    || g.host_unavailable_wonders.contains(wonder)
                {
                    return None;
                }
                let value = self.wonder_lane_value(target, g.rules.wonders.get(wonder)?)?;
                let cost = g.item_cost_for_city(pid, cid, &item);
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    (cost - g.item_invested_production(cid, &item)) / production
                });
                (turns <= CULTURE_WONDER_MAX_TURNS).then_some((item, turns, value, cost))
            })
            .max_by(|a, b| {
                a.2.total_cmp(&b.2)
                    .then_with(|| b.3.total_cmp(&a.3))
                    .then_with(|| format!("{:?}", b.0).cmp(&format!("{:?}", a.0)))
            })
            .map(|(item, turns, _, _)| (item, turns))
    }

    /// Whether `item` at the head of `cid` is a wonder this gene's lane keeps:
    /// the delegated governor's rescoring leaves it alone. Always false with
    /// the gene off.
    pub(super) fn lane_wonder_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        let target = self.active_victory_target(g);
        if !self.wonder_lane_on(target) {
            return false;
        }
        let Item::Wonder { wonder, .. } = item else {
            return false;
        };
        target.is_some_and(|target| {
            g.rules
                .wonders
                .get(wonder)
                .and_then(|spec| self.wonder_lane_value(target, spec))
                .is_some()
        }) && g
            .cities
            .get(&cid)
            .is_some_and(|city| city.owner == pid && plan.threatened_city != Some(cid))
    }

    /// See the module: the strongest cities of a lane seat keep a wonder in
    /// production. Exact no-op with the gene off or for another target.
    pub(super) fn claim_lane_wonders(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        let Some(target) = self
            .active_victory_target(g)
            .filter(|target| self.wonder_lane_on(Some(*target)))
        else {
            return;
        };
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let ours = g.player_city_ids(pid);
        let lanes = ours.len().div_ceil(CULTURE_WONDER_CITIES_PER_LANE);
        let mut in_flight = ours
            .iter()
            .filter(|cid| matches!(g.cities[cid].queue.first(), Some(Item::Wonder { .. })))
            .count();
        let mut ranked: Vec<(f64, u32)> = ours
            .iter()
            .copied()
            .filter(|cid| {
                let city = &g.cities[cid];
                plan.threatened_city != Some(*cid)
                    && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
                    && city.buildings.len() >= 2
                    && !matches!(city.queue.first(), Some(Item::Wonder { .. }))
            })
            .map(|cid| (g.city_yields(cid).production, cid))
            .collect();
        ranked.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        for (_, cid) in ranked {
            if in_flight >= lanes {
                break;
            }
            let Some((wonder, turns)) = self.lane_wonder_item(g, pid, cid, target) else {
                continue;
            };
            let head = g.cities[&cid].queue.first().cloned();
            if let Some(item) = &head {
                if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, item)
                    || self.capital_monument_holds(g, pid, cid, plan, item)
                    || self.culture_theater_holds(g, pid, cid, plan, item)
                    || self.deterrence_unit_holds(g, pid, item)
                    || self.idle_route_trader_holds(g, pid, cid, plan, item)
                    || self.backlog_builder_holds(g, pid, cid, plan, item)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, item)
                    || !Self::culture_theater_may_defer(g, pid, cid, item, at_major_war)
                {
                    continue;
                }
            }
            let name = g.cities[&cid].name.clone();
            if g.apply(
                pid,
                &Action::Produce {
                    city: cid,
                    item: wonder.clone(),
                },
            )
            .is_ok()
            {
                in_flight += 1;
                think!(self.journal(), Cities, Decision,
                       "{name} claims the wonder {} for the {target:?} victory", Self::plain_item(&wonder);
                       "culture-seat-wonder-lane: {turns:.1} turns, lane {in_flight} of {lanes}, ahead of {}",
                       head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                           "{} (it keeps its progress)", Self::plain_item(&item))));
            }
        }
    }
}

#[cfg(test)]
mod tests;
