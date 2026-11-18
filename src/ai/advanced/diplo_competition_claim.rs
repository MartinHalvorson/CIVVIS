//! `diplo-seat-claims-the-competition`: a seat assigned the Diplomatic lane
//! keeps one scored competition's project in production while the
//! competition runs.
//!
//! **Why (live Deity Rome, 6-player Continents, Online, 2026-11-13).** First
//! place in a scored competition pays Diplomatic Victory points: two for an
//! Aid Request, one for the World Games or the International Space Station
//! (`Game::competition_victory_points`). The day's Diplomatic seats ended on
//! 11, 16, 17, 19 and 16 of twenty.
//! - The seat only became a member of these competitions today (the World
//!   Congress vote is the join, and the tracker's type was exported as a row
//!   number; control-mod fixes #5285 and #5288). The first World Games it was
//!   in, `civvis-20261113T204218Z`, ended with one rival on 60 points and
//!   every other member on 0. One Training Athletes project is 50.
//! - In `211246Z` the seat was the ONLY member of an Aid Request from t120.
//!   One Send Aid project is 200. For eleven turns the planner named the
//!   project as two cities' next item (`produce_next PROJECT_SEND_AID`, 30
//!   hints) and no city ever built it: under `lane-delegates-production-2`
//!   an assigned lane's routine queues belong to the baseline governor,
//!   which took a Builder, a Trebuchet, an Ironclad and a Harbor instead.
//!   `host_competition_score_value`, the only place the competition is
//!   priced, is not what fills the queue. It is the Statue of Liberty's
//!   story again (`diplo_wonder_claim.rs`), so it gets the same answer.
//!
//! **What the gene does.** For an assigned Diplomatic seat that has not lost
//! its lane, with no competition project already at the head of any of its
//! queues: the unthreatened city that can finish one before its competition
//! ends (the most first-place points, then the soonest finish, then the most
//! Production; `diplo_wonder_claim::diplo_wonder_ranks_before`) claims it:
//! an idle queue, or one over a head the wonder claim's own rule lets wait
//! and that no other claim holds, the Diplomatic-point wonder and the
//! Statue's Harbor among them. The project is repeatable, so the claim is
//! made again when it completes: one city, for as long as the competition
//! runs. The strategic governor's rescoring leaves the claimed project alone
//! ([`AdvancedAi::diplo_competition_holds`]).
//!
//! Only the three projects that cost nothing but Production: Send Aid,
//! Training Athletes, Training Astronauts. The Climate Accords are scored by
//! decommissioning our own power plants and are left to the routine queue.
//!
//! Off, or for a seat not assigned Diplomacy, nothing changes. Grep the
//! journal for `diplo-seat-claims-the-competition:`.

use super::*;

/// The fewest cities a seat holds before one of them is lent to a
/// competition (the wonder claim's own floor).
pub(super) const DIPLO_COMPETITION_MIN_CITIES: usize = 3;

/// The competition projects the claim takes: Production in, score out, and
/// nothing of ours given up.
const DIPLO_COMPETITION_PROJECTS: [&str; 3] = ["send_aid", "train_athletes", "train_astronauts"];

/// Whether `item` is one of the claim's competition projects.
fn competition_project(item: &Item) -> bool {
    matches!(item, Item::Project { project }
        if DIPLO_COMPETITION_PROJECTS.contains(&project.as_str()))
}

impl AdvancedAi {
    /// Whether the gene acts for this seat this turn.
    fn diplo_competition_claim_on(&self, g: &Game) -> bool {
        self.diplo_seat_claims_the_competition
            && !self.lane_lost
            && self.active_victory_target(g) == Some(VictoryTarget::Diplomacy)
    }

    /// The competition project `cid` would claim: the item, the points first
    /// place in its competition pays, and its build turns. `None` with the
    /// gene off, for another target, a threatened or struck city, a seat of
    /// fewer than [`DIPLO_COMPETITION_MIN_CITIES`], or no project it can
    /// finish before its competition ends.
    pub(super) fn diplo_competition_item(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> Option<(Item, f64, f64)> {
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = g.query_memo();
        if !self.diplo_competition_claim_on(g) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || plan.threatened_city == Some(cid)
            || (city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            || g.player_city_ids(pid).len() < DIPLO_COMPETITION_MIN_CITIES
        {
            return None;
        }
        let production = g.city_yields(cid).production.max(0.5);
        g.producible_items(pid, cid)
            .into_iter()
            .filter(competition_project)
            .filter_map(|item| {
                let Item::Project { project } = &item else {
                    return None;
                };
                let spec = g.rules.projects.get(project)?;
                // The competition it serves that pays the most, with the
                // turns that one has left.
                let (points, left) = spec
                    .host_competition_kinds()
                    .filter_map(|kind| {
                        let competition = g.host_competition(pid, kind)?;
                        let points = Game::competition_victory_points(kind) as f64;
                        (points > 0.0)
                            .then(|| (points, competition.ends.saturating_sub(g.turn) as f64))
                    })
                    .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.total_cmp(&b.1)))?;
                let turns = g.host_production_turns(cid, &item).unwrap_or_else(|| {
                    let remaining = g.item_cost_for_city(pid, cid, &item)
                        - g.item_invested_production(cid, &item);
                    remaining.max(0.0) / production
                });
                (turns.is_finite() && turns <= left).then_some((item, points, turns))
            })
            .max_by(|a, b| {
                a.1.total_cmp(&b.1)
                    .then_with(|| b.2.total_cmp(&a.2))
                    .then_with(|| format!("{:?}", b.0).cmp(&format!("{:?}", a.0)))
            })
    }

    /// Whether `item` at the head of `cid` is a competition project this gene
    /// would keep: still legal (its competition still runs), in an
    /// unthreatened city of ours.
    pub(super) fn diplo_competition_holds(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
        item: &Item,
    ) -> bool {
        if !self.diplo_competition_claim_on(g) || !competition_project(item) {
            return false;
        }
        g.cities
            .get(&cid)
            .is_some_and(|city| city.owner == pid && plan.threatened_city != Some(cid))
            && g.can_produce(pid, cid, item)
    }

    /// See the module: one competition project in production, in the city
    /// that finishes it soonest. Exact no-op with the gene off or for another
    /// target.
    pub(super) fn claim_diplo_competition(&self, g: &mut Game, pid: usize, plan: &StrategicPlan) {
        if !self.diplo_competition_claim_on(g) {
            return;
        }
        let city_ids = g.player_city_ids(pid);
        if city_ids
            .iter()
            .any(|cid| g.cities[cid].queue.first().is_some_and(competition_project))
        {
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
            let Some((item, points, turns)) = self.diplo_competition_item(g, pid, cid, plan) else {
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
                    || self.diplo_wonder_holds(g, pid, cid, plan, head)
                    || self.statue_harbor_holds(g, pid, cid, plan, head)
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
                    super::diplo_wonder_claim::diplo_wonder_ranks_before(
                        (points, turns, production, cid),
                        (*old_points, *old_turns, *old_production, *old_city),
                    )
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
                   "{name} claims {} for the competition", Self::plain_item(&item);
                   "diplo-seat-claims-the-competition: first place pays {points:.0} of the twenty points, \
                    {production:.0} Production, {turns:.1} turns, ahead of {}",
                   head.map_or_else(|| "an idle queue".to_string(), |item| format!(
                       "{} (it keeps its progress)", Self::plain_item(&item))));
        }
    }
}

#[cfg(test)]
mod tests;
