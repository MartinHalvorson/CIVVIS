//! `chase-every-boost`: one opt-in gene that hunts Eurekas AND Inspirations
//! everywhere the planner already decides something a boost trigger names.
//!
//! Version one is deliberately the broad historical control described below.
//! It expands every trigger into the chase table and lets the resulting
//! research value influence beelines, queue items, Builder work, and clean
//! kills. That breadth made stale or irrelevant future boosts compete with
//! current plans.
//!
//! `chase-every-boost-2` keeps only a narrow, cashable slice: a city may break
//! a close production tie for the *final* unit, building, or district trigger
//! of the technology or civic already under study. The city must finish before
//! the study itself, and another city already queued to finish the same trigger
//! suppresses the premium. Version two does not alter research selection,
//! Builder work, or combat; v1 remains available as the family control.
//!
//! Measured on the live King/Emperor ladder (ledger runs of 2026-08-30 to
//! 2026-09-01, 32 runs past turn 100): of the technologies the seat
//! researched, **13–40 % had been boosted**; of the civics it adopted,
//! **0–26 %, typically 5–12 %**. Each boost is 40 % of the node's cost. The
//! one near-win on Emperor (run 20260901T132005Z) lost the space race by a
//! single turn with 29 of 72 techs boosted and 11 of 43 civics inspired. A
//! strong human science player boosts most of both trees; the gap is a
//! research-pace lever the size of the whole late-game deficit.
//!
//! Why the six existing boost genes did not close it: three of them only
//! re-order research (`boost-first-research`, `boost-wait-research-2`,
//! `boost-unlock-research`), two chase a trigger by building the thing it
//! names (`eureka-chasing-builder`, `eureka-chasing-production`), and all
//! five read [`AdvancedAi::eureka_chases`], whose progress reader knew **six
//! buildable trigger prefixes** and returned `None` for every other one —
//! kills, camps, contacts, trade routes, population, districts, wonders,
//! great people. Nothing anywhere steered a unit action toward a boost, and
//! the science lane's beeline (`goal_pick` in `plan_research`) walked the
//! cheapest prerequisite by printed cost, blind to which prerequisite was
//! already 40 % paid for — on the live seat the lane beelines nearly every
//! turn, so the ordering genes were inert exactly where they mattered.
//!
//! What this gene does, as one arm so the live ladder can screen it whole:
//!
//! 1. **Every trigger is a chase.** [`Game::boost_progress`] is the engine's
//!    own `(have, need)` for every one of the ~45 trigger forms, so the chase
//!    table covers both trees completely. Counter-backed triggers (`kills`,
//!    `barbs_killed`, `kill_with:*`, `camps`, …) read zero on the host seat,
//!    where the mirror never fills `player.counters`; that keeps them open as
//!    chases-by-action until the host reports the boost fired, which is the
//!    truthful answer.
//! 2. **Research and civics.** The in-hand scale (`boost_in_hand_scale`) is
//!    deliberately NOT armed (its isolation read negative, see that function);
//!    a node whose boost is
//!    **one actionable step away** is deferred (`boost_wait_penalty`, in its
//!    tight form); a node is credited the chases it opens, under a cap a
//!    third of the standalone gene's; and the **beeline** takes the goal's
//!    prerequisites by effective cost — printed cost less the boost in hand,
//!    or half the boost when one action away — which never lengthens the
//!    path because every prerequisite is walked anyway.
//! 3. **Production.** A unit, building, district or wonder that advances an
//!    open chase is worth the research per step it earns, discounted by the
//!    steps still to go, less what other cities already have queued for the
//!    same trigger, so two cities do not both build "the second Archer".
//! 4. **Builders** keep `eureka-chasing-builder`'s premium, the strongest of
//!    the old family (+11.7 pp wins in its own probe).
//! 5. **Kills.** A kill that fires `kill_with:<kind>`, `kills`, `barbs_killed`
//!    or clears a camp is worth the research it earns — only when the exact
//!    evaluation says the attacker survives, so the premium can raise a kill
//!    above the threshold but never make a losing exchange acceptable.
//!
//! Off, every path is byte-identical: each hook is gated on the flag and the
//! old genes keep their own flags and behaviour.
use super::deity_habits::EurekaChase;
use super::AdvancedAi;
use crate::game::{Game, Item};
use crate::Pos;

/// Kill premium: what one research point of an open boost is worth on the
/// tactical scale, where a clean kill of a cheap unit scores ~190. Archery
/// (50 × 0.4 = 20 points) pays a Slinger kill +20; Military Tactics (300 ×
/// 0.4 = 120) would pay +120 and is capped below.
pub(super) const CHASE_KILL_VALUE_PER_POINT: f64 = 1.0;
/// The ceiling on the kill premium: a third of a kill's own base value, so
/// the premium re-orders which kill is taken and never invents one.
pub(super) const CHASE_KILL_VALUE_CAP: f64 = 60.0;
/// Production premium per research point, on `production_value`'s raw
/// scale. ⚠ THAT SCALE IS SMALL: measured on the probe's own board at turns
/// 60–180, an Archer reads 68, a Campus 51, an Amphitheater 46, a Trebuchet
/// 95–161, a Crossbowman 180. `eureka-chasing-production`'s 4.0 per point
/// (capped at 400) added +234 to a Trebuchet and +400 to an Entertainment
/// Complex — three to eight times the item's own worth — and a two-city
/// empire built the trigger instead of the Settler; that is why that gene
/// measured a null. A quarter point per beaker makes a 40-point step worth
/// +10 against items worth 50–200: a nudge among comparable builds.
pub(super) const CHASE_PRODUCTION_VALUE_PER_POINT: f64 = 0.25;
/// The production premium's absolute ceiling, on the same scale.
pub(super) const CHASE_PRODUCTION_VALUE_CAP: f64 = 60.0;
/// The production premium's relative ceiling: never more than this share
/// of the item's own positive value, so a trigger can only re-order builds
/// of comparable worth and never lifts a marginal item over a Settler or a
/// Campus.
pub(super) const CHASE_PRODUCTION_RAW_FRACTION: f64 = 0.5;
/// The builder premium's relative ceiling under this gene, against the
/// improvement's own yield value: the same discipline as production.
pub(super) const CHASE_BUILDER_VALUE_FRACTION: f64 = 0.5;
/// How much of a step's value survives for each further step the trigger
/// still needs: the third Archer is worth its full step only once two are
/// standing, and a chase three builds out may never be finished.
pub(super) const CHASE_STEP_DISCOUNT: f64 = 0.8;
/// Beeline: a boost one actionable step away is priced as this much of a
/// boost in hand — the action is available now, and the discount is
/// credited mid-research if the node is still running when it lands.
pub(super) const CHASE_ONE_STEP_FRACTION: f64 = 0.5;
/// The cap, in turns of research, on the unlock credit under this gene.
/// `boost-unlock-research` capped at six turns (132 on the value scale
/// against a flat 28 for a boost in hand) and measured slightly negative;
/// two turns keeps the permission worth less than the boost it opens.
pub(super) const CHASE_UNLOCK_TURNS_CAP: f64 = 2.0;
/// `chase-every-boost-2`: a cashable final trigger is a production tiebreak,
/// not a second strategic objective. The raw queue scale is hundreds to low
/// thousands, so half a raw point per granted research point is enough to
/// distinguish close local choices without displacing a Settler, district, or
/// urgent defender on the boost alone.
pub(super) const CHASE_V2_PRODUCTION_VALUE_PER_POINT: f64 = 0.5;
/// The absolute ceiling on version two's one-item production tiebreak.
pub(super) const CHASE_V2_PRODUCTION_VALUE_CAP: f64 = 50.0;
/// The same tiebreak may never exceed one quarter of the item's ordinary raw
/// score; the ordinary city plan remains the reason to build it.
pub(super) const CHASE_V2_PRODUCTION_RAW_FRACTION: f64 = 0.25;

impl AdvancedAi {
    /// The open chase for `node`, if its boost is still earnable.
    pub(super) fn chase_for(&self, g: &Game, pid: usize, node: &str) -> Option<EurekaChase> {
        self.eureka_chases(g, pid)
            .into_iter()
            .find(|chase| chase.node.as_str() == node)
    }

    /// Is this chase one actionable step from firing: exactly one trigger
    /// step left, and the thing that step names is something the empire can
    /// do right now?
    pub(super) fn chase_one_action_away(&self, g: &Game, pid: usize, chase: &EurekaChase) -> bool {
        chase.remaining == 1 && self.chase_actionable(g, pid, chase)
    }

    /// Can the empire act on this trigger now — build the thing, improve the
    /// tile, make the kill? Triggers that only time or growth advance
    /// (population, contacts, alliances, great people) are not actions and
    /// read `false`, so nothing is deferred waiting for them.
    pub(super) fn chase_actionable(&self, g: &Game, pid: usize, chase: &EurekaChase) -> bool {
        let trigger = chase.trigger.as_str();
        let owns_unit = |want: &dyn Fn(&str) -> bool| {
            g.units
                .values()
                .any(|unit| unit.owner == pid && want(unit.kind.as_str()))
        };
        let military = |kind: &str| {
            g.rules
                .units
                .get(kind)
                .is_some_and(|spec| spec.class == "military")
        };
        if let Some(kind) = trigger.strip_prefix("kill_with:") {
            return owns_unit(&|have| have == kind);
        }
        if matches!(trigger, "kills" | "barbs_killed" | "camps") {
            return owns_unit(&military);
        }
        if trigger == "cities" {
            return owns_unit(&|have| have == "settler");
        }
        if trigger == "trade_routes" {
            return owns_unit(&|have| have == "trader")
                || Self::trigger_gates(g, "units_of:trader")
                    .iter()
                    .all(|(gate, techs)| Self::node_known(g, pid, gate.as_str(), *techs));
        }
        if trigger == "wonders" {
            return g
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .any(|city| {
                    city.queue
                        .iter()
                        .any(|item| matches!(item, Item::Wonder { .. }))
                });
        }
        let gates = Self::trigger_gates(g, trigger);
        if gates.is_empty() {
            return false;
        }
        gates
            .iter()
            .all(|(gate, techs)| Self::node_known(g, pid, gate.as_str(), *techs))
    }

    /// The cost the beeline should walk a prerequisite at: the printed cost
    /// less the boost in hand, or less half the boost when it is one action
    /// away; the printed cost with the gene off. Ordering the goal's own
    /// prerequisites this way cannot lengthen the path — every one of them is
    /// researched before the goal whatever the order.
    pub(super) fn beeline_step_cost(&self, g: &Game, pid: usize, node: &str, techs: bool) -> f64 {
        let cost = if techs {
            g.rules.techs[node].cost
        } else {
            g.rules.civics[node].cost
        };
        if !self.chase_every_boost {
            return cost;
        }
        let frac = Self::boost_frac(g, node, techs).clamp(0.0, 0.99);
        if Self::boost_in_hand(g, pid, node, techs) {
            return cost * (1.0 - frac);
        }
        match self.chase_for(g, pid, node) {
            Some(chase) if self.chase_one_action_away(g, pid, &chase) => {
                cost * (1.0 - frac * CHASE_ONE_STEP_FRACTION)
            }
            _ => cost,
        }
    }

    /// The production premium `production_value` adds for `item` in city
    /// `cid`: the old `eureka-chasing-production` rate when only that gene is
    /// on, v1's broad chase-aware version when it is on, or v2's active-study
    /// final-trigger tiebreak. Zero with all three genes off.
    pub(super) fn production_boost_premium(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        raw: f64,
    ) -> f64 {
        if self.chase_every_boost {
            self.chase_production_premium(g, pid, cid, item)
                .min(raw.max(0.0) * CHASE_PRODUCTION_RAW_FRACTION)
        } else if self.chase_every_boost_2 {
            self.chase_current_production_premium(g, pid, cid, item)
                .min(raw.max(0.0) * CHASE_V2_PRODUCTION_RAW_FRACTION)
        } else {
            self.eureka_production_premium(g, pid, item)
        }
    }

    /// The builder premium `improvement_value_with_appeal` adds: the old
    /// `eureka-chasing-builder` rate when that gene is on, the same rate held
    /// under half the improvement's own value when only this gene is on.
    pub(super) fn builder_boost_premium(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        value: f64,
    ) -> f64 {
        if self.eureka_chasing_builder_2 && !self.chase_every_boost {
            return self.eureka_builder_v2_premium(g, pos, improvement, value);
        }
        let premium = self.eureka_builder_premium(g, pos, improvement);
        if self.chase_every_boost && !self.eureka_chasing_builder {
            premium.min(value.max(0.0) * CHASE_BUILDER_VALUE_FRACTION)
        } else {
            premium
        }
    }

    /// The trigger key an item advances, in the chase table's own spelling.
    /// A wonder advances the tree-agnostic `wonders` trigger (Drama and
    /// Poetry's inspiration); projects and repairs advance nothing.
    fn item_trigger_key(g: &Game, item: &Item) -> Option<String> {
        Some(match item {
            Item::Unit { unit } => format!("units_of:{unit}"),
            Item::Building { building } => format!("building:{building}"),
            Item::District { district, .. } => {
                format!("district:{}", g.district_family(*district))
            }
            Item::Wonder { .. } => "wonders".to_string(),
            _ => return None,
        })
    }

    /// The deliberately smaller item surface that version two may price.
    /// Wonders remain a v1-only global chase: their shared trigger is too
    /// broad for a city-local, already-active-study tiebreak.
    fn v2_item_trigger_key(g: &Game, item: &Item) -> Option<String> {
        match item {
            Item::Unit { .. } | Item::Building { .. } | Item::District { .. } => {
                Self::item_trigger_key(g, item)
            }
            _ => None,
        }
    }

    /// The final remaining production trigger for one active study, together
    /// with its research payout and the time before that study completes.
    /// This deliberately reads the engine's progress predicate directly,
    /// rather than expanding the whole v1 chase table.
    fn active_study_production_trigger(
        &self,
        g: &Game,
        pid: usize,
        node: &str,
        techs: bool,
    ) -> Option<(String, f64, f64)> {
        let player = &g.players[pid];
        let (boost, progress, cost, base_cost) = if techs {
            let spec = g.rules.techs.get(node)?;
            (
                spec.boost.as_ref()?,
                player.research_progress,
                g.tech_cost(node),
                spec.cost,
            )
        } else {
            let spec = g.rules.civics.get(node)?;
            (
                spec.boost.as_ref()?,
                player.civic_progress,
                g.civic_cost(node),
                spec.cost,
            )
        };
        if Self::boost_in_hand(g, pid, node, techs) {
            return None;
        }
        let (have, need) = g.boost_progress(pid, boost);
        if need.saturating_sub(have) != 1 {
            return None;
        }
        let study_left = (cost - progress).max(0.0);
        if study_left <= 0.0 {
            return None;
        }
        let payout = g.game_speed.scale(base_cost) * boost.percent.unwrap_or(40.0) / 100.0;
        let study_turns = study_left / Self::research_rate(g, pid, techs);
        // A boost can only save the research still outstanding. Near the end
        // of a study the nominal 40% is not a reason to buy a costly trigger.
        Some((boost.trigger.clone(), payout.min(study_left), study_turns))
    }

    /// `chase-every-boost-2`: reward only a current study's final city
    /// production trigger, and only if this city can complete it before the
    /// study. A matching front-of-queue item elsewhere has already claimed
    /// the one remaining step, so this city receives no duplicate premium.
    pub(super) fn chase_current_production_premium(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> f64 {
        if !self.chase_every_boost_2 {
            return 0.0;
        }
        let Some(key) = Self::v2_item_trigger_key(g, item) else {
            return 0.0;
        };
        let claimed_elsewhere = g.cities.values().any(|city| {
            city.owner == pid
                && city.id != cid
                && city.queue.first().is_some_and(|queued| {
                    Self::v2_item_trigger_key(g, queued).as_deref() == Some(key.as_str())
                })
        });
        if claimed_elsewhere {
            return 0.0;
        }
        let production =
            (g.city_yields(cid).production * g.item_prod_mult(pid, cid, Some(item))).max(1.0);
        let build_turns = g.item_remaining_cost_for_city(pid, cid, item) / production;
        let active = [
            (g.players[pid].research.as_deref(), true),
            (g.players[pid].civic.as_deref(), false),
        ];
        let payout: f64 = active
            .into_iter()
            .filter_map(|(node, techs)| node.map(|node| (node, techs)))
            .filter_map(|(node, techs)| self.active_study_production_trigger(g, pid, node, techs))
            .filter(|(trigger, _, study_turns)| trigger == &key && build_turns <= *study_turns)
            .map(|(_, payout, _)| payout)
            .sum();
        (payout * CHASE_V2_PRODUCTION_VALUE_PER_POINT).min(CHASE_V2_PRODUCTION_VALUE_CAP)
    }

    /// What `item` is worth toward open chases, less what the rest of the
    /// empire has already queued for the same trigger. A chase needing three
    /// Archers with two already queued elsewhere pays this city for the
    /// third and nothing for a fourth.
    pub(super) fn chase_production_premium(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> f64 {
        let Some(key) = Self::item_trigger_key(g, item) else {
            return 0.0;
        };
        let queued_elsewhere = g
            .cities
            .values()
            .filter(|city| city.owner == pid && city.id != cid)
            .flat_map(|city| city.queue.iter())
            .filter(|queued| Self::item_trigger_key(g, queued).as_deref() == Some(key.as_str()))
            .count() as i64;
        let premium: f64 = self
            .eureka_chases(g, pid)
            .iter()
            .filter(|chase| chase.trigger == key)
            .map(|chase| {
                let left = chase.remaining - queued_elsewhere;
                if left <= 0 {
                    return 0.0;
                }
                chase.per_step()
                    * CHASE_PRODUCTION_VALUE_PER_POINT
                    * CHASE_STEP_DISCOUNT.powi((left - 1) as i32)
            })
            .sum();
        premium.min(CHASE_PRODUCTION_VALUE_CAP)
    }

    /// What a kill by `uid` on `target` earns in open boosts: `kill_with:` the
    /// attacker's own kind, `kills`, `barbs_killed` against the Barbarian
    /// seat, and `camps` for a defender standing on a camp. Zero with the gene
    /// off. The caller adds this only to a kill the exact evaluation says the
    /// attacker survives.
    pub(super) fn chase_kill_premium(&self, g: &Game, pid: usize, uid: u32, target: Pos) -> f64 {
        if !self.chase_every_boost {
            return 0.0;
        }
        let Some(attacker) = g.units.get(&uid) else {
            return 0.0;
        };
        let kill_with = format!("kill_with:{}", attacker.kind);
        let barbarian = g
            .unit_ids_at(target)
            .iter()
            .any(|other| g.players[g.units[other].owner].is_barbarian);
        let camp = g
            .map
            .get(target)
            .is_some_and(|tile| tile.improvement.as_deref() == Some("barbarian_camp"));
        let premium: f64 = self
            .eureka_chases(g, pid)
            .iter()
            .filter(|chase| {
                let trigger = chase.trigger.as_str();
                trigger == kill_with
                    || trigger == "kills"
                    || (barbarian && trigger == "barbs_killed")
                    || (camp && trigger == "camps")
            })
            .map(|chase| chase.per_step() * CHASE_KILL_VALUE_PER_POINT)
            .sum();
        premium.min(CHASE_KILL_VALUE_CAP)
    }
}

#[cfg(test)]
mod tests;
