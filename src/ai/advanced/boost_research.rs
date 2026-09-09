//! Three boost-aware research habits as opt-in genes (operator, 2026-08-25:
//! *"consider the technologies and civics we already have boosted and maybe
//! research those first, as we maybe try to get boosts for the other techs and
//! civics in the tree … more intentional about trying to get boosts, as that
//! is free acceleration"*).
//!
//! Sixty-two technologies and fifty-three civics carry a boost worth 40% of
//! their cost (`data/techs.json`, `data/civics.json`; Near Future Governance
//! pays 90). `Game::do_research` and `Game::do_civic` credit `frac * cost` the
//! moment the node is selected, and the boost loop credits the same lump
//! mid-research when the trigger lands on a node already being worked — but a
//! node **finished** before its trigger lands never collects: the loop skips
//! anything in `player.techs` / `player.civics`. So the order the tree is
//! taken in is worth real research, and until now the whole of the agent's
//! opinion about it was one flat `+28` in `tech_value` and `civic_value`,
//! paid whatever era the node sat in and whatever the boost actually saved.
//!
//! What each gene changes, and why it is a gene rather than a fact the agent
//! already knew:
//!
//! - **`boost-first-research`.** A boost in hand **scales** the node's score
//!   rather than adding to it. `tech_value` and `civic_value` both end
//!   `(value + k) / cost.sqrt()`, so what they rank is value per root beaker;
//!   a boost makes the node cost `(1 - frac)` of its printed price, and the
//!   score that discount buys is therefore the old one times
//!   `1 / (1 - frac).sqrt()` — 1.29 at the shipped 40%, with no free
//!   parameter. The flat `+28` stays exactly where it was, above the divisor.
//!
//!   ⚠ THE FIRST CUT ADDED, AND THE PROBE RESOLVED IT NEGATIVE. It credited
//!   the boost the *turns of research it saves* — `frac * cost` over the
//!   empire's own science per turn — which reads three turns in every era on
//!   the shipped cost curve but blows up in the opening, where an empire makes
//!   two or three beakers a turn and every ancient boost is worth the twelve-
//!   turn cap. Added above a `sqrt(cost)` divisor that is only 7 for an
//!   ancient node, that is +37 against unlocks worth single digits: the gene
//!   stopped preferring the boosted node among comparable ones and started
//!   taking whatever was boosted, however little it unlocked. 24 games,
//!   144 seats: score share **-3.36 pp, z -3.21**, against a run resolving
//!   ±2.93 — outside its own noise, which the win column's -9.4 against ±17.0
//!   was not. See `docs/gene_screens/fires/boost-first-research-v1.json`.
//! - **`boost-first-research-2`.** The scale as a **tie-break**, which is what
//!   the paragraph above asked for and v1 did not deliver. v1 on the
//!   2026-09-08 13,938-seat screen (`docs/gene_screens/2026-09-08-standard-
//!   continuous-13938-total-seats-20260908T150313Z-6afe.json`): research pace
//!   **+0.42 techs at standard turn 150, z +5.6** — the third-best pace gene
//!   of 268 — and wins **−0.48 pp** (P(>0) 40.8%, batch columns +8/+32/−27,
//!   rank 241). More techs, and the wrong ones: a 1.29 multiplier on a boosted
//!   node beats the plain winner whenever the two are within 29% of each
//!   other, and inside `tech_value` it also reaches every reader of the score,
//!   so a Sailing or an Astrology with an inspiration in hand walks past the
//!   Campus building, the luxury, the wall the plan actually wanted. v2 keeps
//!   the multiplier and the cap and changes *where it may decide*:
//!   1. It lives in `advanced_research`, after every forced goal — the war
//!      breakthrough, the lane beeline, the luxury, the harbour, the science
//!      milestone — has stood down. A beeline step is never overturned.
//!   2. The ordinary argmax runs unscaled. A boosted candidate is admitted
//!      only when its **unscaled** score is at least
//!      `BOOST_TIEBREAK_BAND` (85%) of the ordinary winner's; among the
//!      admitted, the scaled score decides, and it must beat the winner
//!      outright. So the boost decides among near-equals (a runner-up at 0.85
//!      scales to 1.10 and takes the slot) and a node at 0.80 — which v1 lifts
//!      to 1.03 and takes — keeps its place behind the clear leader.
//!   3. `tech_value` / `civic_value` never see the scale: reasoning lines,
//!      the bargain genes and every other reader price the node as before.
//! - **`boost-wait-research-2`.** The other half of the same fact: a node we
//!   would *finish* inside two turns, whose final boost trigger is already at
//!   the front of an owned city queue, is taken after the eureka rather than
//!   before it — the discount survives a long node (it is credited
//!   mid-research) and dies on a short one. The penalty is a light tie-break
//!   on the boost at risk, scaled by how likely the node is to beat its own
//!   trigger home. Version one (`boost-wait-research`: a six-turn window,
//!   any buildable trigger, half the boost) left the code on 2026-09-08 with
//!   version two shipping on; its broader window repeatedly delayed useful
//!   prerequisites for a boost whose Builder or queue item never arrived.
//! - **`boost-unlock-research`.** Being *intentional* about earning the rest:
//!   a node is credited the boosts it makes chaseable at all. Masonry's quarry
//!   wants Mining, Machinery's three Archers want Archery, Guilds' two Markets
//!   want Currency — `eureka-chasing-builder` and `eureka-chasing-production`
//!   can only chase a trigger whose thing the empire is already allowed to
//!   build, and nothing ever went and bought them the permission.
//!
//! All three read the shipped `BoostSpec` rows through
//! [`AdvancedAi::eureka_chases`], the same per-turn table the two Deity eureka
//! genes read, so a trigger the engine cannot detect is never chased and
//! nothing past `EUREKA_CHASE_ERA_REACH` is priced. Off, `boost_research_value`
//! returns the old flat credit exactly and every path is byte-identical.

use super::deity_habits::EurekaChase;
use super::AdvancedAi;
use crate::game::{Game, Item};
use crate::name::Name;

/// The flat credit `tech_value` and `civic_value` pay for a boost in hand.
/// Unconditional, exactly as before the genes: `boost_first_research` scales
/// the finished score instead of touching this.
pub(super) const BOOST_IN_HAND_FLAT_VALUE: f64 = 28.0;

/// `boost_first_research`: the ceiling on the score multiplier. `1 /
/// (1 - frac).sqrt()` is 1.29 at the shipped 40% and 3.16 at Near Future
/// Governance's 90%, and a node worth three times its neighbours for one
/// inspiration is the additive mistake in another costume.
pub(super) const BOOST_SCALE_CAP: f64 = 2.0;

/// `boost_first_research_2`: how close, in unscaled score, a boosted node must
/// be to the ordinary argmax winner before its discount is allowed to decide.
/// 85%: with the shipped 40% boost (scale 1.29) a node at the band's edge
/// scores 1.10 of the winner and takes the slot, and a node at 80% — which v1
/// lifts to 1.03 and takes, and which is where v1's +0.42 techs / −0.48 pp
/// record says the wrong nodes come from — stays behind the clear leader.
pub(super) const BOOST_TIEBREAK_BAND: f64 = 0.85;

/// `boost_wait_research_2` / `boost_unlock_research`: what one turn of research
/// is worth on the `tech_value` / `civic_value` scale, above the `sqrt(cost)`
/// divisor. Both of those genes price something the node does for the rest of
/// the tree — a boost lost, a permission bought — which is an ordinary
/// addition like a building unlock, not a discount on this node.
pub(super) const BOOST_TURN_VALUE: f64 = 22.0;

/// `boost_wait_research_2` / `boost_unlock_research`: the ceiling on how many
/// turns of research one boost is allowed to be worth. An empire whose science
/// has collapsed against a mid-game tree can price a single boost at forty
/// turns, which would dictate the tree rather than order it.
pub(super) const BOOST_TURNS_CAP: f64 = 12.0;

/// `chase_every_boost`'s wait: a node the empire needs longer than this to
/// finish is not at risk — its trigger has room to land while the node runs,
/// and the engine credits the boost mid-research. Six turns is the shortest
/// ordinary gap between two eurekas earned by ordinary building. (This was
/// `boost-wait-research` v1's window; that gene left the code on 2026-09-08.)
pub(super) const BOOST_WAIT_HORIZON_TURNS: f64 = 6.0;

/// `boost_wait_research_2`: only a node within two turns is close enough to
/// justify yielding its slot to the final outstanding trigger step. The
/// broader six-turn v1 window repeatedly delayed useful prerequisites for a
/// boost whose Builder or queue item never arrived.
pub(super) const BOOST_WAIT_V2_HORIZON_TURNS: f64 = 2.0;

/// `chase_every_boost`'s wait: how much of the boost at risk the wait
/// subtracts. Half, so the penalty reorders the cheap end of the list against
/// nodes of comparable value and never outweighs a node that is wanted on
/// its merits. (`boost-wait-research` v1's factor; that gene left the code.)
pub(super) const BOOST_WAIT_FACTOR: f64 = 0.5;

/// `boost_wait_research_2`: a light tie-break rather than v1's half-boost
/// veto. The last trigger still has to win its own Builder or production
/// decision, so its uncertain saving must not overrule a wanted node.
pub(super) const BOOST_WAIT_V2_FACTOR: f64 = 0.2;

/// `boost_unlock_research`: what a boost this node makes chaseable is worth
/// against a boost already in hand. Under a third: the permission is not the
/// boost, something still has to be built, and the trigger may never be
/// completed at all.
pub(super) const BOOST_UNLOCK_FACTOR: f64 = 0.3;

/// `boost_unlock_research`: how the second, third and fourth trigger a node
/// opens count against the first. Halving: a Builder or a city works one
/// trigger at a time, so the permission is worth most for the best thing it
/// opens and progressively less for the queue behind it. A plain sum instead
/// made every node that opened anything at all worth the same ceiling.
pub(super) const BOOST_UNLOCK_RANK_DECAY: f64 = 0.5;

/// `boost_unlock_research`: the ceiling, in turns of research, on one node's
/// whole unlock credit. Binding only in the opening, where the empire's few
/// beakers make every ancient boost worth ten turns and Mining, Pottery and
/// Archery would otherwise each price themselves above a lane's own goal.
pub(super) const BOOST_UNLOCK_TURNS_CAP: f64 = 6.0;

impl AdvancedAi {
    /// The whole boost-related term in `tech_value` (`techs`) or `civic_value`
    /// (`!techs`) for `node`: the credit for a boost in hand, the wait for one
    /// nearly earned, and the credit for the boosts this node makes chaseable.
    ///
    /// With all three genes off this is `BOOST_IN_HAND_FLAT_VALUE` for a
    /// boosted node and zero otherwise — the pre-gene expression exactly.
    pub(super) fn boost_research_value(
        &self,
        g: &Game,
        pid: usize,
        node: &str,
        techs: bool,
    ) -> f64 {
        let value = if Self::boost_in_hand(g, pid, node, techs) {
            // Held: nothing left to wait for, and the discount itself is
            // `boost_in_hand_scale`'s business, below the divisor.
            BOOST_IN_HAND_FLAT_VALUE
        } else {
            -self.boost_wait_penalty(g, pid, node, techs)
        };
        value + self.boost_unlock_credit(g, pid, node, techs)
    }

    /// `boost_first_research`: what to multiply a finished `tech_value` /
    /// `civic_value` by. Both end `(value + k) / cost.sqrt()`, so they rank
    /// value per root beaker; a boost in hand makes this node cost `1 - frac`
    /// of its printed price, and the score that buys is the old one times
    /// `1 / (1 - frac).sqrt()`. One with the gene off, and one for a node
    /// whose boost is not in hand — so nothing else in the tree moves.
    pub(super) fn boost_in_hand_scale(&self, g: &Game, pid: usize, node: &str, techs: bool) -> f64 {
        // `chase-every-boost` carries this scale as part of its union; see
        // `advanced/chase_every_boost.rs`.
        // `chase-every-boost` deliberately does NOT arm this scale. Its 24-game
        // isolation on seeds 26090140..63 read the whole gene at share -2.6 pp
        // (z -2.7) and every variant with this scale off as neutral, while the
        // scale's own ancestor (`boost-first-research` v1) is the one member of
        // the family that ever measured significantly negative (share -3.4 pp,
        // z -3.2). The gene keeps the wait, unlock, beeline, production, builder
        // and kill hooks; the live screen prices the union.
        if !self.boost_first_research || !Self::boost_in_hand(g, pid, node, techs) {
            return 1.0;
        }
        Self::boost_discount_scale(g, node, techs)
    }

    /// What a node's boost buys under the `sqrt(cost)` divisor, read without
    /// asking whether it is in hand or whether any gene is on: `1 / (1 -
    /// frac).sqrt()`, capped at `BOOST_SCALE_CAP`. Version one applies it
    /// inside the value functions; version two applies it in the argmax.
    pub(super) fn boost_discount_scale(g: &Game, node: &str, techs: bool) -> f64 {
        let frac = Self::boost_frac(g, node, techs).clamp(0.0, 0.99);
        (1.0 / (1.0 - frac).sqrt()).min(BOOST_SCALE_CAP)
    }

    /// `boost_first_research_2`: the scale as a tie-break on the ordinary
    /// research argmax. `ordinary` is the node the plain `tech_value` (`techs`)
    /// or `civic_value` (`!techs`) argmax chose over `available`; the answer is
    /// a boosted node whose **unscaled** score is within `BOOST_TIEBREAK_BAND`
    /// of the winner's and whose scaled score beats the winner outright — the
    /// best such by scaled score, ties to the name the argmax itself would
    /// prefer — or `None`, which leaves the ordinary pick standing. `None`
    /// with the gene off, so the argmax is byte-identical. Called only after
    /// every forced goal has stood down: a beeline step is never overturned.
    pub(super) fn boost_tiebreak_pick(
        &self,
        g: &Game,
        pid: usize,
        strategy: super::GrandStrategy,
        available: &[Name],
        ordinary: &Name,
        techs: bool,
    ) -> Option<Name> {
        if !self.boost_first_research_2 {
            return None;
        }
        let value = |node: &Name| {
            if techs {
                self.tech_value(g, pid, node, strategy)
            } else {
                self.civic_value(g, pid, node, strategy)
            }
        };
        let winner = value(ordinary);
        let floor = winner * BOOST_TIEBREAK_BAND;
        available
            .iter()
            .filter(|node| *node != ordinary && Self::boost_in_hand(g, pid, node, techs))
            .filter_map(|node| {
                let unscaled = value(node);
                (unscaled >= floor)
                    .then(|| (unscaled * Self::boost_discount_scale(g, node, techs), *node))
            })
            .filter(|(scaled, _)| *scaled > winner)
            .max_by(|left, right| {
                left.0
                    .total_cmp(&right.0)
                    .then_with(|| right.1.cmp(&left.1))
            })
            .map(|(_, node)| node)
    }

    /// Is this node's boost already banked?
    pub(super) fn boost_in_hand(g: &Game, pid: usize, node: &str, techs: bool) -> bool {
        let player = &g.players[pid];
        let name = Name::new(node);
        if techs {
            player.boosted_techs.contains(&name)
        } else {
            player.boosted_civics.contains(&name)
        }
    }

    /// The fraction of a node's cost its boost pays, read the way
    /// `Game::node_boost_frac` reads it: the row's own percentage, or the
    /// shipped 40 for a node with no boost row that was granted one anyway (a
    /// goody hut, a Great Scientist, a stolen boost).
    pub(super) fn boost_frac(g: &Game, node: &str, techs: bool) -> f64 {
        let percent = if techs {
            g.rules.techs[node].boost.as_ref().and_then(|b| b.percent)
        } else {
            g.rules.civics[node].boost.as_ref().and_then(|b| b.percent)
        };
        percent.unwrap_or(40.0) / 100.0
    }

    /// `boost_wait_research_2`: what to subtract from a node whose boost is
    /// still earnable and which the empire would finish before the trigger
    /// lands. Zero with the gene off, for a node nothing buildable can boost,
    /// and for one far enough out that the mid-research credit will reach it.
    fn boost_wait_penalty(&self, g: &Game, pid: usize, node: &str, techs: bool) -> f64 {
        if !(self.boost_wait_research_2 || self.chase_every_boost) {
            return 0.0;
        }
        // ⚠ THE RISK TEST COMES FIRST, AND IT IS THE CHEAP ONE. How likely the
        // node is to beat its own trigger home depends on nothing but its cost
        // and the empire's rate: full weight on one that finishes next turn,
        // nothing on one still running when the engine's mid-research credit
        // could reach it. In the opening every node is a long node, so this
        // returns before the chase table is touched — and the table is a clone
        // of every boost in reach, taken once per candidate in the argmax. The
        // first cut looked the node up first and the probe measured the gene
        // at +10.4% compute a seat.
        let rate = Self::research_rate(g, pid, techs);
        let cost = if techs {
            g.tech_cost(node)
        } else {
            g.civic_cost(node)
        };
        let horizon = if self.boost_wait_research_2 {
            BOOST_WAIT_V2_HORIZON_TURNS
        } else {
            BOOST_WAIT_HORIZON_TURNS
        };
        let risk = (1.0 - (cost / rate) / horizon).clamp(0.0, 1.0);
        if risk <= 0.0 {
            return 0.0;
        }
        let chases = self.eureka_chases(g, pid);
        let Some(chase) = chases
            .iter()
            .find(|chase| chase.node == node && Self::chase_is_tech(g, chase) == techs)
        else {
            return 0.0;
        };
        // `chase-every-boost` alone waits only for a boost one actionable step
        // away: the thing its trigger names can be done now, and one step
        // finishes it. A trigger three builds out, or one only growth or a
        // contact advances, is not worth holding a node for. See
        // `advanced/chase_every_boost.rs`. With the wait gene on, that
        // gene's own rule below decides instead.
        if !self.boost_wait_research_2 && !self.chase_one_action_away(g, pid, chase) {
            return 0.0;
        }
        if self.boost_wait_research_2
            && (chase.remaining != 1 || !Self::boost_trigger_is_queued(g, pid, &chase.trigger))
        {
            return 0.0;
        }
        let turns = (chase.research / rate).min(BOOST_TURNS_CAP);
        let factor = if self.boost_wait_research_2 {
            BOOST_WAIT_V2_FACTOR
        } else {
            BOOST_WAIT_FACTOR
        };
        turns * BOOST_TURN_VALUE * factor * risk
    }

    /// Is the final buildable boost trigger already at the front of an owned
    /// city queue? V2 waits only for work the empire has actually committed,
    /// never for a hypothetical Builder job or a merely legal queue item.
    fn boost_trigger_is_queued(g: &Game, pid: usize, trigger: &str) -> bool {
        g.cities
            .values()
            .filter(|city| city.owner == pid)
            .any(|city| {
                city.queue
                    .first()
                    .is_some_and(|item| match (trigger, item) {
                        (trigger, Item::Unit { unit } | Item::Formation { unit, .. }) => trigger
                            .strip_prefix("units_of:")
                            .is_some_and(|want| unit.as_str() == want),
                        (trigger, Item::Building { building }) => trigger
                            .strip_prefix("building:")
                            .is_some_and(|want| building.as_str() == want),
                        (trigger, Item::District { district, .. }) => trigger
                            .strip_prefix("district:")
                            .is_some_and(|want| g.district_family(*district) == want),
                        _ => false,
                    })
            })
    }

    /// `boost_unlock_research`: what the boosts this node makes chaseable are
    /// worth. A chase is counted when the thing its trigger names is gated on
    /// a node in this tree that the empire does not hold and `node` is on the
    /// way to it. Zero with the gene off.
    fn boost_unlock_credit(&self, g: &Game, pid: usize, node: &str, techs: bool) -> f64 {
        if !(self.boost_unlock_research || self.chase_every_boost) {
            return 0.0;
        }
        // Under `chase-every-boost` alone the credit is capped at a third of
        // the standalone gene's ceiling; see `CHASE_UNLOCK_TURNS_CAP`.
        let turns_cap = if self.boost_unlock_research {
            BOOST_UNLOCK_TURNS_CAP
        } else {
            super::chase_every_boost::CHASE_UNLOCK_TURNS_CAP
        };
        // Both rates once: this runs for every candidate node in the argmax,
        // and every chase inside each of those.
        let science = Self::research_rate(g, pid, true);
        let culture = Self::research_rate(g, pid, false);
        let mut opened: Vec<f64> = Vec::new();
        for chase in self.eureka_chases(g, pid) {
            if !self.opens_the_trigger(g, pid, node, techs, &chase.trigger) {
                continue;
            }
            let rate = if Self::chase_is_tech(g, &chase) {
                science
            } else {
                culture
            };
            opened.push((chase.research / rate).min(BOOST_TURNS_CAP));
        }
        opened.sort_by(|left, right| right.total_cmp(left));
        let turns: f64 = opened
            .iter()
            .enumerate()
            .map(|(rank, turns)| turns * BOOST_UNLOCK_RANK_DECAY.powi(rank as i32))
            .sum();
        (turns * BOOST_UNLOCK_FACTOR).min(turns_cap) * BOOST_TURN_VALUE
    }

    /// Is `node` on the way to a permission this trigger needs and the empire
    /// does not hold? A gate in the other tree is another gene's business.
    fn opens_the_trigger(
        &self,
        g: &Game,
        pid: usize,
        node: &str,
        techs: bool,
        trigger: &str,
    ) -> bool {
        Self::trigger_gates(g, trigger)
            .into_iter()
            .filter(|(gate, gate_is_tech)| {
                *gate_is_tech == techs && !Self::node_known(g, pid, gate.as_str(), *gate_is_tech)
            })
            .any(|(gate, _)| {
                if techs {
                    self.tech_leads_to(g, node, gate.as_str())
                } else {
                    self.civic_leads_to(g, node, gate.as_str())
                }
            })
    }

    /// The empire's science (`techs`) or culture per turn, read from the same
    /// city yields the war desk's own horizon reads. Floored at one so a
    /// yield-less empire cannot divide by zero.
    pub(super) fn research_rate(g: &Game, pid: usize, techs: bool) -> f64 {
        g.player_city_ids(pid)
            .into_iter()
            .map(|cid| {
                let yields = g.city_yields(cid);
                if techs {
                    yields.science
                } else {
                    yields.culture
                }
            })
            .sum::<f64>()
            .max(1.0)
    }

    /// Which tree a chase's node belongs to. The two trees share no names
    /// today; this is read rather than assumed so that a future one cannot
    /// price a civic's inspiration against the science rate.
    fn chase_is_tech(g: &Game, chase: &EurekaChase) -> bool {
        g.rules.techs.contains_key(&chase.node)
    }

    pub(super) fn node_known(g: &Game, pid: usize, node: &str, techs: bool) -> bool {
        let player = &g.players[pid];
        let name = Name::new(node);
        if techs {
            player.techs.contains(&name)
        } else {
            player.civics.contains(&name)
        }
    }

    /// The nodes that gate the thing a boost trigger names, each flagged as a
    /// technology (`true`) or a civic (`false`). Only the triggers
    /// `eureka_trigger_progress` can count reach here, so every one of them
    /// names something the rules gate on a node.
    pub(super) fn trigger_gates(g: &Game, trigger: &str) -> Vec<(Name, bool)> {
        let improvement_gates = |improvement: &str| -> Vec<(Name, bool)> {
            g.rules
                .improvements
                .get(improvement)
                .map(|spec| Self::gates(spec.tech, spec.civic))
                .unwrap_or_default()
        };
        if let Some(improvement) = trigger
            .strip_prefix("improvement:")
            .or_else(|| trigger.strip_prefix("improvement_on_resource:"))
        {
            return improvement_gates(improvement);
        }
        if let Some(resource) = trigger.strip_prefix("improve_resource:") {
            // Both permissions: the strategic resource stays hidden until its
            // own node, and the improvement that connects it needs its own.
            let Some(spec) = g.rules.resources.get(resource) else {
                return Vec::new();
            };
            let mut gates = Self::gates(spec.tech, spec.civic);
            gates.extend(improvement_gates(spec.improvement.as_str()));
            return gates;
        }
        if let Some(kind) = trigger.strip_prefix("units_of:") {
            return g
                .rules
                .units
                .get(kind)
                .map(|spec| Self::gates(spec.tech, spec.civic))
                .unwrap_or_default();
        }
        if let Some(building) = trigger.strip_prefix("building:") {
            return g
                .rules
                .buildings
                .get(building)
                .map(|spec| Self::gates(spec.tech, spec.civic))
                .unwrap_or_default();
        }
        if let Some(district) = trigger.strip_prefix("district:") {
            return g
                .rules
                .districts
                .get(district)
                .map(|spec| Self::gates(spec.tech, spec.civic))
                .unwrap_or_default();
        }
        Vec::new()
    }

    fn gates(tech: Option<Name>, civic: Option<Name>) -> Vec<(Name, bool)> {
        tech.map(|name| (name, true))
            .into_iter()
            .chain(civic.map(|name| (name, false)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::opt_in_off_in_both_controllers;
    use super::super::{AdvancedAi, GrandStrategy};
    use super::*;
    use crate::game::{Action, Game};
    use crate::name;

    #[test]
    fn boost_first_research_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("boost-first-research", |ai| ai.boost_first_research);
    }

    #[test]
    fn boost_first_research_2_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("boost-first-research-2", |ai| ai.boost_first_research_2);
    }

    #[test]
    fn boost_wait_research_2_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("boost-wait-research-2", |ai| ai.boost_wait_research_2);
    }

    #[test]
    fn boost_unlock_research_is_a_native_opt_in_off_in_both_controllers() {
        opt_in_off_in_both_controllers("boost-unlock-research", |ai| ai.boost_unlock_research);
    }

    /// One founded capital and nothing else on the board.
    fn capital_board(seed: u64) -> Game {
        let mut game = Game::new_full(1, 20, 14, seed, 200, 0, false);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("the player opens with a settler");
        game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
        for uid in game.player_unit_ids(0) {
            game.remove_unit(uid);
        }
        game
    }

    /// Off, the whole term is the flat credit the two value functions paid
    /// before the genes existed — 28 for a boost in hand, nothing otherwise.
    #[test]
    fn every_gene_off_is_the_old_flat_credit_exactly() {
        let mut game = capital_board(53_001);
        let ai = AdvancedAi::new();
        assert_eq!(ai.boost_research_value(&game, 0, "masonry", true), 0.0);
        assert_eq!(
            ai.boost_research_value(&game, 0, "craftsmanship", false),
            0.0
        );
        game.players[0].boosted_techs.insert(name!("masonry"));
        game.players[0]
            .boosted_civics
            .insert(name!("craftsmanship"));
        assert_eq!(
            ai.boost_research_value(&game, 0, "masonry", true),
            BOOST_IN_HAND_FLAT_VALUE
        );
        assert_eq!(
            ai.boost_research_value(&game, 0, "craftsmanship", false),
            BOOST_IN_HAND_FLAT_VALUE
        );
    }

    /// The empire's science per turn, as the gene reads it.
    fn science_rate(game: &Game) -> f64 {
        game.player_city_ids(0)
            .into_iter()
            .map(|cid| game.city_yields(cid).science)
            .sum::<f64>()
            .max(1.0)
    }

    /// Put the capital on a chosen research rate.
    fn set_science(game: &mut Game, science: f64) {
        let cid = game.player_city_ids(0)[0];
        std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            cid,
            crate::rules::Yields {
                science,
                ..Default::default()
            },
        );
    }

    /// `boost-first-research` multiplies the finished score by exactly what
    /// the discount buys under the function's own `sqrt(cost)` divisor, and
    /// leaves an unboosted node alone.
    #[test]
    fn a_boost_in_hand_scales_the_score_by_what_the_discount_buys() {
        let mut game = capital_board(53_002);
        game.players[0].boosted_techs.insert(name!("masonry"));
        game.players[0]
            .boosted_civics
            .insert(name!("craftsmanship"));
        let plain = AdvancedAi::new();
        let mut ai = AdvancedAi::new();
        ai.enable_boost_first_research();
        let strategy = GrandStrategy::Expansion;
        let expected = 1.0 / (1.0f64 - 0.4).sqrt();
        for (node, techs) in [("masonry", true), ("craftsmanship", false)] {
            let off = if techs {
                plain.tech_value(&game, 0, node, strategy)
            } else {
                plain.civic_value(&game, 0, node, strategy)
            };
            let on = if techs {
                ai.tech_value(&game, 0, node, strategy)
            } else {
                ai.civic_value(&game, 0, node, strategy)
            };
            assert!(off > 0.0);
            assert!(
                (on / off - expected).abs() < 1e-9,
                "{node} scaled by {} where the 40% discount buys {expected}",
                on / off
            );
        }
        // A node whose boost is not in hand is untouched, whatever its cost.
        for node in ["pottery", "mining", "irrigation"] {
            assert_eq!(
                ai.tech_value(&game, 0, node, strategy),
                plain.tech_value(&game, 0, node, strategy),
                "{node} has no boost in hand and must not move"
            );
        }
    }

    /// The multiplier is a ratio, not a lump: a boosted node that unlocks
    /// nothing still loses to an unboosted node that unlocks something. This
    /// is the property the additive first cut did not have, and the reason its
    /// probe resolved a score-share loss.
    #[test]
    fn a_boosted_node_that_unlocks_nothing_does_not_outrank_a_useful_one() {
        let mut game = capital_board(53_011);
        set_science(&mut game, 3.0);
        let strategy = GrandStrategy::Expansion;
        let plain = AdvancedAi::new();
        let mut ai = AdvancedAi::new();
        ai.enable_boost_first_research();
        // The two ends of the opening tree by plain merit.
        let available = game.available_techs(0);
        let best = *available
            .iter()
            .max_by(|a, b| {
                plain
                    .tech_value(&game, 0, a, strategy)
                    .total_cmp(&plain.tech_value(&game, 0, b, strategy))
            })
            .expect("the opening tree offers something");
        let worst = *available
            .iter()
            .filter(|tech| **tech != best)
            .min_by(|a, b| {
                plain
                    .tech_value(&game, 0, a, strategy)
                    .total_cmp(&plain.tech_value(&game, 0, b, strategy))
            })
            .expect("and something else");
        let gap = plain.tech_value(&game, 0, &best, strategy)
            / plain.tech_value(&game, 0, &worst, strategy);
        assert!(
            gap > BOOST_SCALE_CAP,
            "the opening tree spreads wider than the cap ({gap} > {BOOST_SCALE_CAP}), \
             so this test can tell a ratio from a lump"
        );
        game.players[0].boosted_techs.insert(Name::new(&worst));
        assert!(
            ai.tech_value(&game, 0, &best, strategy) > ai.tech_value(&game, 0, &worst, strategy),
            "boosting {worst} must not lift it over {best}"
        );
    }

    /// The cap: no inspiration is allowed to make a node worth several of its
    /// neighbours, which is the additive mistake in another costume.
    #[test]
    fn the_scale_is_capped() {
        let mut game = capital_board(53_003);
        let mut ai = AdvancedAi::new();
        ai.enable_boost_first_research();
        for tech in game.rules.techs.keys().copied().collect::<Vec<_>>() {
            game.players[0].boosted_techs.insert(tech);
        }
        for civic in game.rules.civics.keys().copied().collect::<Vec<_>>() {
            game.players[0].boosted_civics.insert(civic);
        }
        for tech in game.rules.techs.keys() {
            let scale = ai.boost_in_hand_scale(&game, 0, tech.as_str(), true);
            assert!(
                (1.0..=BOOST_SCALE_CAP).contains(&scale),
                "{tech} scales by {scale}"
            );
        }
        for civic in game.rules.civics.keys() {
            let scale = ai.boost_in_hand_scale(&game, 0, civic.as_str(), false);
            assert!(
                (1.0..=BOOST_SCALE_CAP).contains(&scale),
                "{civic} scales by {scale}"
            );
        }
    }

    /// The wait keeps only the high-confidence edge: a short node and the
    /// final outstanding trigger step, with a small penalty that breaks a
    /// close choice instead of vetoing useful research.
    #[test]
    fn boost_wait_research_2_is_a_short_final_step_tiebreak() {
        let mut game = capital_board(53_014);
        let mut v2 = AdvancedAi::new();
        v2.enable_boost_wait_research_2();
        let cost = game.tech_cost("engineering");
        set_science(&mut game, cost);

        assert_eq!(
            v2.boost_research_value(&game, 0, "engineering", true),
            0.0,
            "a merely buildable Wall does not delay Engineering"
        );
        let capital = game.player_city_ids(0)[0];
        game.cities.get_mut(&capital).unwrap().queue = vec![Item::Building {
            building: name!("walls"),
        }];
        game.turn += 1;
        let v2_penalty = v2.boost_research_value(&game, 0, "engineering", true);
        assert!(v2_penalty < 0.0, "the queued final Wall can justify a wait");
        assert!(
            v2_penalty.abs() <= BOOST_TURNS_CAP * BOOST_TURN_VALUE * BOOST_WAIT_V2_FACTOR,
            "a tie-break, never a veto: {v2_penalty}"
        );

        set_science(&mut game, cost / (BOOST_WAIT_V2_HORIZON_TURNS * 2.0));
        assert_eq!(
            v2.boost_research_value(&game, 0, "engineering", true),
            0.0,
            "a node outside v2's short window keeps its place"
        );
    }

    /// A boost already in hand is never waited for — there is nothing left to
    /// earn, and the node should be taken now.
    #[test]
    fn a_boost_in_hand_is_never_waited_for() {
        let mut game = capital_board(53_005);
        game.players[0].techs.insert(name!("mining"));
        game.players[0].boosted_techs.insert(name!("masonry"));
        let mut waiting = AdvancedAi::new();
        waiting.enable_boost_wait_research_2();
        let capital = game.player_city_ids(0)[0];
        let rich = crate::rules::Yields {
            science: game.tech_cost("masonry"),
            ..Default::default()
        };
        std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(capital, rich);
        assert_eq!(
            waiting.boost_research_value(&game, 0, "masonry", true),
            BOOST_IN_HAND_FLAT_VALUE
        );
    }

    /// `boost-unlock-research` pays the node that buys the permission a boost
    /// trigger needs — Mining, for the quarry Masonry's eureka wants — and
    /// stops paying for that trigger once the permission is held.
    #[test]
    fn the_node_that_opens_a_trigger_is_paid_for_it() {
        let mut game = capital_board(53_006);
        set_science(&mut game, 40.0);
        let plain = AdvancedAi::new();
        let mut opening = AdvancedAi::new();
        opening.enable_boost_unlock_research();
        assert_eq!(
            game.rules.improvements["quarry"].tech.as_deref(),
            Some("mining"),
            "the quarry Masonry's boost wants is gated on Mining"
        );
        assert!(
            opening
                .eureka_chases(&game, 0)
                .iter()
                .any(|chase| chase.trigger == "improvement_on_resource:quarry"),
            "the quarry trigger is on the chase table"
        );
        let off = plain.boost_research_value(&game, 0, "mining", true);
        let on = opening.boost_research_value(&game, 0, "mining", true);
        assert_eq!(off, 0.0);
        assert!(on > 0.0, "Mining is paid for the triggers it opens: {on}");
        // Held, the quarry is buildable and its trigger is the Builder gene's
        // business, not the research argmax's. Mining keeps only what it still
        // opens further down its own branch, which is strictly less.
        game.players[0].techs.insert(name!("mining"));
        game.turn += 1;
        let held = opening.boost_research_value(&game, 0, "mining", true);
        assert!(
            held < on,
            "the permission already held is not paid for twice: {held} < {on}"
        );
    }

    /// A node that opens nothing is untouched by the gene.
    #[test]
    fn a_node_that_opens_no_trigger_is_not_paid() {
        let mut game = capital_board(53_010);
        set_science(&mut game, 40.0);
        let mut opening = AdvancedAi::new();
        opening.enable_boost_unlock_research();
        let untouched: Vec<&str> =
            game.rules
                .techs
                .keys()
                .map(|tech| tech.as_str())
                .filter(|tech| {
                    opening.eureka_chases(&game, 0).iter().all(|chase| {
                        !opening.opens_the_trigger(&game, 0, tech, true, &chase.trigger)
                    })
                })
                .collect();
        assert!(
            !untouched.is_empty(),
            "some technology on the board opens no trigger in reach"
        );
        for tech in untouched {
            assert_eq!(
                opening.boost_research_value(&game, 0, tech, true),
                0.0,
                "{tech} opens nothing and is priced exactly as before"
            );
        }
    }

    /// The credit is bounded: a node that opens many triggers at once cannot
    /// price itself above the ceiling.
    #[test]
    fn the_unlock_credit_is_capped() {
        let game = capital_board(53_007);
        let mut opening = AdvancedAi::new();
        opening.enable_boost_unlock_research();
        let ceiling = BOOST_UNLOCK_TURNS_CAP * BOOST_TURN_VALUE;
        for tech in game.rules.techs.keys() {
            let value = opening.boost_research_value(&game, 0, tech.as_str(), true);
            assert!(
                value <= ceiling + 1e-9,
                "{tech} is credited {value}, over the {ceiling} ceiling"
            );
        }
    }

    /// A civic chase is priced against culture and a technology chase against
    /// science: the two rates differ and the trees must not be crossed.
    #[test]
    fn the_two_trees_are_priced_against_their_own_rate() {
        let mut game = capital_board(53_008);
        set_science(&mut game, 400.0);
        let mut opening = AdvancedAi::new();
        opening.enable_boost_unlock_research();
        let science = science_rate(&game);
        let culture: f64 = game
            .player_city_ids(0)
            .into_iter()
            .map(|cid| game.city_yields(cid).culture)
            .sum::<f64>()
            .max(1.0);
        assert!(
            science > culture * 10.0,
            "the capital's science is far ahead of its culture: {science} vs {culture}"
        );
        // Every chase this board offers is a technology's, so a credit priced
        // against culture instead of science would be an order out.
        let chases = opening.eureka_chases(&game, 0);
        assert!(!chases.is_empty());
        for chase in &chases {
            let is_tech = game.rules.techs.contains_key(&chase.node);
            assert_eq!(
                is_tech,
                !game.rules.civics.contains_key(&chase.node),
                "{} belongs to exactly one tree",
                chase.node
            );
        }
    }

    /// A seat plays at most one version of the family: arming either version
    /// stands the other down.
    #[test]
    fn boost_first_research_versions_are_mutually_exclusive() {
        let mut ai = AdvancedAi::new();
        ai.enable_boost_first_research();
        ai.enable_boost_first_research_2();
        assert!(!ai.boost_first_research && ai.boost_first_research_2);
        ai.enable_boost_first_research();
        assert!(ai.boost_first_research && !ai.boost_first_research_2);
        ai.disable_boost_first_research();
        assert!(!ai.boost_first_research && !ai.boost_first_research_2);
    }

    /// Version two never touches the value functions — a boosted node prices
    /// exactly as with every gene off — and the tie-break answers nothing
    /// while the gene is off, however boosted the runner-up.
    #[test]
    fn boost_first_research_2_leaves_the_value_functions_and_the_off_argmax_alone() {
        let mut game = capital_board(53_020);
        let strategy = GrandStrategy::Expansion;
        let plain = AdvancedAi::new();
        let mut v2 = AdvancedAi::new();
        v2.enable_boost_first_research_2();
        let available = game.available_techs(0);
        for tech in game.rules.techs.keys().copied().collect::<Vec<_>>() {
            game.players[0].boosted_techs.insert(tech);
        }
        for civic in game.rules.civics.keys().copied().collect::<Vec<_>>() {
            game.players[0].boosted_civics.insert(civic);
        }
        for tech in game.rules.techs.keys() {
            assert_eq!(
                v2.tech_value(&game, 0, tech.as_str(), strategy),
                plain.tech_value(&game, 0, tech.as_str(), strategy),
                "{tech} is priced the same under v2 as with every gene off"
            );
        }
        for civic in game.rules.civics.keys() {
            assert_eq!(
                v2.civic_value(&game, 0, civic.as_str(), strategy),
                plain.civic_value(&game, 0, civic.as_str(), strategy),
                "{civic} is priced the same under v2 as with every gene off"
            );
        }
        let ordinary = available[0];
        assert_eq!(
            plain.boost_tiebreak_pick(&game, 0, strategy, &available, &ordinary, true),
            None,
            "off, the tie-break never speaks"
        );
    }

    /// The band's arithmetic: at the shipped 40% boost a node at the band's
    /// edge is carried over the winner (so the tie-break can speak at all),
    /// and there is a gap below the band where v1's scale alone would carry
    /// a node over the winner and v2 refuses it — the whole difference
    /// between the versions.
    #[test]
    fn the_band_admits_its_edge_and_leaves_v1_a_gap_below_it() {
        let scale = 1.0 / (1.0f64 - 0.4).sqrt();
        assert!(
            BOOST_TIEBREAK_BAND * scale > 1.0,
            "a node at the band's edge scales to {:.3} and must beat the winner",
            BOOST_TIEBREAK_BAND * scale
        );
        let v1_floor = 1.0 / scale;
        assert!(
            v1_floor < BOOST_TIEBREAK_BAND,
            "v1 lifts anything above {v1_floor:.3}; v2 admits nothing below {BOOST_TIEBREAK_BAND}"
        );
        assert!(
            (1.0..=BOOST_SCALE_CAP).contains(&scale),
            "the shipped scale sits under the cap"
        );
    }

    /// The band, on real trees: boosting one node at a time under each grand
    /// strategy, on the opening tree and again on the classical tier, the
    /// tie-break takes it exactly when its unscaled score is within
    /// `BOOST_TIEBREAK_BAND` of the plain winner AND the scale carries it over
    /// — so a comparable node is admitted and a clear leader is never
    /// overturned. (The real trees are lumpy: no node on these boards falls
    /// in the gap v1 takes and v2 refuses, which the arithmetic test above
    /// covers.)
    #[test]
    fn the_tiebreak_admits_a_comparable_boost_and_refuses_a_clear_leader_overturn() {
        let mut game = capital_board(53_009);
        let plain = AdvancedAi::new();
        let mut v2 = AdvancedAi::new();
        v2.enable_boost_first_research_2();
        let (mut admitted, mut refused) = (0, 0);
        let mut sweep = |game: &mut Game| {
            let available = game.available_techs(0);
            let argmax = |ai: &AdvancedAi, game: &Game, strategy: GrandStrategy| {
                available
                    .iter()
                    .max_by(|a, b| {
                        ai.tech_value(game, 0, a, strategy)
                            .partial_cmp(&ai.tech_value(game, 0, b, strategy))
                            .unwrap()
                            .then_with(|| b.cmp(a))
                    })
                    .cloned()
                    .expect("the tree offers something")
            };
            for strategy in [
                GrandStrategy::Expansion,
                GrandStrategy::Science,
                GrandStrategy::Culture,
            ] {
                game.players[0].boosted_techs.clear();
                let ordinary = argmax(&plain, game, strategy);
                let winner = plain.tech_value(game, 0, &ordinary, strategy);
                for node in available.iter().filter(|node| **node != ordinary) {
                    game.players[0].boosted_techs.clear();
                    game.players[0].boosted_techs.insert(*node);
                    // The plain score of a boosted node already carries the
                    // pre-gene flat credit; the band reads that score.
                    let unscaled = plain.tech_value(game, 0, node, strategy);
                    let scale = AdvancedAi::boost_discount_scale(game, node, true);
                    let expected = (unscaled >= winner * BOOST_TIEBREAK_BAND
                        && unscaled * scale > winner)
                        .then_some(*node);
                    let pick =
                        v2.boost_tiebreak_pick(game, 0, strategy, &available, &ordinary, true);
                    assert_eq!(
                        pick, expected,
                        "{node} at {unscaled:.1} against {ordinary} at {winner:.1} ({strategy:?})"
                    );
                    match pick {
                        Some(_) => admitted += 1,
                        None => refused += 1,
                    }
                }
                // The clear leader case by name: the worst node on offer,
                // boosted, is never admitted.
                let worst = *available
                    .iter()
                    .filter(|tech| **tech != ordinary)
                    .min_by(|a, b| {
                        plain
                            .tech_value(game, 0, a, strategy)
                            .total_cmp(&plain.tech_value(game, 0, b, strategy))
                    })
                    .expect("and something else");
                game.players[0].boosted_techs.clear();
                game.players[0].boosted_techs.insert(worst);
                assert_eq!(
                    v2.boost_tiebreak_pick(game, 0, strategy, &available, &ordinary, true),
                    None,
                    "boosting {worst} must not lift it over {ordinary}"
                );
            }
        };
        sweep(&mut game);
        for tech in [
            "pottery",
            "animal_husbandry",
            "mining",
            "sailing",
            "astrology",
            "irrigation",
            "writing",
            "archery",
            "masonry",
            "bronze_working",
            "wheel",
        ] {
            game.players[0].techs.insert(Name::new(tech));
        }
        game.turn += 1;
        sweep(&mut game);
        assert!(
            admitted > 0,
            "some runner-up is comparable and takes the slot"
        );
        assert!(refused > 0, "some node is a clear loser and stays put");
    }

    /// The whole point, on the real argmax: among nodes the plain function
    /// ranks within the multiplier of each other, the boosted one wins.
    #[test]
    fn the_research_pick_prefers_a_boost_it_already_holds() {
        let mut game = capital_board(53_009);
        let strategy = GrandStrategy::Expansion;
        let plain = AdvancedAi::new();
        let mut ai = AdvancedAi::new();
        ai.enable_boost_first_research();
        let argmax = |ai: &AdvancedAi, game: &Game| {
            game.available_techs(0)
                .into_iter()
                .max_by(|a, b| {
                    ai.tech_value(game, 0, a, strategy)
                        .partial_cmp(&ai.tech_value(game, 0, b, strategy))
                        .unwrap()
                        .then_with(|| b.cmp(a))
                })
                .expect("the opening tree offers something")
        };
        let before = argmax(&plain, &game);
        // The runner-up, boosted, takes the slot: it is inside the ratio.
        let runner_up = game
            .available_techs(0)
            .into_iter()
            .filter(|tech| *tech != before)
            .max_by(|a, b| {
                plain
                    .tech_value(&game, 0, a, strategy)
                    .partial_cmp(&plain.tech_value(&game, 0, b, strategy))
                    .unwrap()
                    .then_with(|| b.cmp(a))
            })
            .expect("and a runner-up");
        let ratio = plain.tech_value(&game, 0, &before, strategy)
            / plain.tech_value(&game, 0, &runner_up, strategy);
        assert!(
            ratio < 1.0 / (1.0f64 - 0.4).sqrt(),
            "the runner-up is within the discount ({ratio}), so boosting it should win"
        );
        game.players[0].boosted_techs.insert(runner_up);
        assert_eq!(
            argmax(&ai, &game),
            runner_up,
            "the boost in hand takes the slot from {before}"
        );
        assert_eq!(
            argmax(&plain, &game),
            before,
            "and the gene off leaves the old pick alone"
        );
    }
}
