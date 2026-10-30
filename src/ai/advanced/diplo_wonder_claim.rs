//! `diplo-seat-wonder-claim`: a seat assigned the Diplomatic lane keeps one
//! Diplomatic Victory Point wonder in production in its strongest city.
//!
//! **Why.** The Statue of Liberty pays four of the twenty points, the
//! Mahabodhi Temple two, the Potala Palace one
//! (`Expansion1_Expansion2.xml` / `Expansion2_Buildings.xml`
//! `MODIFIER_PLAYER_ADJUST_DIPLOMATIC_VICTORY_POINTS`). Live King
//! civvis-20261030T074500Z (Diplomatic) stood at 18 points at t201 and lost to
//! a rival Culture victory at t207; the first live Diplomatic win,
//! 081846Z at t208, needed one point between sessions. Under the live
//! `lane-delegates-production-2` an assigned lane's routine queues belong to
//! the baseline governor, which builds a wonder only once a city holds every
//! building, and `strategic_wonder_value` -- the only place these points are
//! priced -- is never asked. -43's Culture wonder lane measured harm natively
//! for Culture wonders (forced extras displace compounding builds); a
//! Diplomatic point is worth far more per Production, so this is one lane,
//! in the top-production city, with no wonder-gap gate.
//!
//! **What the gene does.** For an assigned Diplomatic seat that has not lost
//! its lane, with no Diplomatic-point wonder already at the head of any of
//! its queues: the unthreatened city that can finish one in
//! [`DIPLO_WONDER_MAX_TURNS`] (the most points first, then the most
//! Production, then the quickest) claims it -- an idle queue, or one over a
//! head [`AdvancedAi::culture_theater_may_defer`] lets wait and that no other
//! claim holds. The strategic governor's rescoring leaves the claimed wonder
//! alone ([`AdvancedAi::diplo_wonder_holds`]).
//!
//! Off, or for a seat not assigned Diplomacy, nothing changes.

use super::*;

/// The slowest a claimed Diplomatic-point wonder may be, in turns at the
/// claiming city's Production. The Statue of Liberty (1,240) at Modern-era
/// Production of 45-60 is 21-28.
pub(super) const DIPLO_WONDER_MAX_TURNS: f64 = 30.0;

/// The fewest cities a seat holds before it claims one.
pub(super) const DIPLO_WONDER_MIN_CITIES: usize = 3;

fn diplomatic_points(g: &Game, item: &Item) -> f64 {
    match item {
        Item::Wonder { wonder, .. } => g
            .rules
            .wonders
            .get(wonder)
            .and_then(|spec| spec.effects.get("diplomatic_victory_points").copied())
            .unwrap_or(0.0),
        _ => 0.0,
    }
}

impl AdvancedAi {
    /// Whether the gene acts for this seat this turn.
    fn diplo_wonder_claim_on(&self, g: &Game) -> bool {
        self.diplo_seat_wonder_claim
            && self.active_victory_target(g) == Some(VictoryTarget::Diplomacy)
            && !self.lane_lost
    }

    /// The Diplomatic-point wonder `cid` would claim: its points and build
    /// turns. `None` with the gene off, for another target, a threatened or
    /// struck city, or nothing it can finish in [`DIPLO_WONDER_MAX_TURNS`].
    pub(super) fn diplo_wonder_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64, f64)> {
        if !self.diplo_wonder_claim_on(g) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || g.player_city_ids(pid).len() < DIPLO_WONDER_MIN_CITIES
        {
            return None;
        }
        let refused = g.blocked_wonders.get(&cid);
        let production = g.city_yields(cid).production.max(0.5);
        g.producible_items(pid, cid)
            .into_iter()
            .filter_map(|item| {
                let Item::Wonder { wonder, .. } = &item else {
                    return None;
                };
                if refused.is_some_and(|blocked| blocked.contains(wonder))
                    || g.host_unavailable_wonders.contains(wonder)
                {
                    return None;
                }
                let points = diplomatic_points(g, &item);
                if points <= 0.0 {
                    return None;
                }
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    let remaining = g.item_cost_for_city(pid, cid, &item)
                        - g.item_invested_production(cid, &item);
                    remaining.max(0.0) / production
                });
                (turns <= DIPLO_WONDER_MAX_TURNS).then_some((item, points, turns))
            })
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| b.2.total_cmp(&a.2))
                    .then_with(|| format!("{:?}", b.0).cmp(&format!("{:?}", a.0)))
            })
    }

    /// Whether `item` at the head of `cid` is a Diplomatic-point wonder this
    /// gene would keep: still legal, in an unthreatened city of ours.
    pub(super) fn diplo_wonder_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.diplo_wonder_claim_on(g) || diplomatic_points(g, item) <= 0.0 {
            return false;
        }
        g.cities
            .get(&cid)
            .is_some_and(|city| city.owner == pid && plan.threatened_city != Some(cid))
            && g.can_produce(pid, cid, item)
    }

    /// See the module: one Diplomatic-point wonder in production, in the
    /// strongest city that can finish one. Exact no-op with the gene off or
    /// for another target.
    pub(super) fn claim_diplo_wonder(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.diplo_wonder_claim_on(g) {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        if city_ids.iter().any(|cid| {
            g.cities[cid]
                .queue
                .first()
                .is_some_and(|head| diplomatic_points(g, head) > 0.0)
        }) {
            return;
        }
        let at_major_war = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_barbarian
                && !other.is_minor
                && g.is_at_war(pid, other.id)
        });
        let mut best: Option<(f64, f64, f64, u32, Item)> = None;
        for cid in city_ids {
            let Some((item, points, turns)) = self.diplo_wonder_item(g, pid, cid, plan) else {
                continue;
            };
            if let Some(head) = g.cities[&cid].queue.first() {
                if self.early_settler_floor_holds(g, pid, cid, plan)
                    || self.bound_granary_holds(g, pid, cid, plan, head)
                    || self.capital_monument_holds(g, pid, cid, plan, head)
                    || self.deterrence_unit_holds(g, pid, head)
                    || self.idle_route_trader_holds(g, pid, cid, plan, head)
                    || self.backlog_builder_holds(g, pid, cid, plan, head)
                    || self.capped_aqueduct_holds(g, pid, cid, plan, head)
                    || Self::queued_wall_breaker(g, head)
                    || !Self::culture_theater_may_defer(g, pid, cid, head, at_major_war)
                {
                    continue;
                }
            }
            let production = g.city_yields(cid).production;
            if best
                .as_ref()
                .is_none_or(|(old_points, old_production, old_turns, old_city, _)| {
                    (-points, -production, turns, cid)
                        < (-*old_points, -*old_production, *old_turns, *old_city)
                })
            {
                best = Some((points, production, turns, cid, item));
            }
        }
        let Some((points, production, turns, cid, item)) = best else {
            return;
        };
        let head = g.cities[&cid].queue.first().cloned();
        let name = g.cities[&cid].name.clone();
        if g.apply(
            pid,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .is_ok()
        {
            think!(self.journal(), Cities, Decision,
                   "{name} claims the {} for the Diplomatic victory", Self::plain_item(&item);
                   "diplo-seat-wonder-claim: {points:.0} of the twenty points, \
                    {production:.0} Production, {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
