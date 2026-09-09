//! `boost-planner`: the disciplined half of the boost problem — a short,
//! deadlined plan for the Eurekas and Inspirations the empire is about to walk
//! past, rather than another blanket chase.
//!
//! **The evidence.** At Emperor and above the handicap hands every rival a
//! flat science and culture multiplier (+16 % to +32 %) *and* free technology
//! and civic boosts. A boost is the one research multiplier the handicap does
//! not scale for us: 40 % of a node's cost (`data/techs.json`,
//! `data/civics.json`; Near Future Governance pays 90), earned by doing
//! something we were often going to do anyway. Measured on the live
//! King/Emperor ladder (ledger runs of 2026-08-30 to 2026-09-01, 32 runs past
//! turn 100, quoted in `advanced/chase_every_boost.rs`): of the technologies
//! the seat researched, **13–40 % had been boosted**; of the civics it
//! adopted, **0–26 %, typically 5–12 %**. A strong human boosts most of both
//! trees. That gap is the size of the whole late-game research deficit.
//!
//! **Why this is not another chase.** `chase-every-boost` (version one) closed
//! the *coverage* half — every trigger the engine can judge is now a chase —
//! and ships on. Its disciplined-looking sibling did not work:
//! `chase-every-boost-2` sits at rank 250 of `GENE_HEURISTIC_RANKING.md` with
//! a total on/off difference of **−0.57 %** and batch columns **−31 / −11 /
//! −3** per 10k seats, P(>0) 10.5 %. The lesson recorded there and in
//! `advanced/deity_habits.rs` is the same one twice: a premium that prices
//! *every* reachable trigger makes stale or irrelevant future boosts compete
//! with the current plan, and a city builds the trigger instead of the
//! Settler. Breadth is not the missing thing; **commitment with a deadline**
//! is.
//!
//! So this gene chases almost nothing. It looks only at the next
//! [`BOOST_HORIZON`] technologies and [`BOOST_CIVIC_HORIZON`] civics the
//! beeline is actually going to take, classifies each one's trigger by what it
//! would *cost the plan* to satisfy, and turns only the **cheap** ones — work
//! the empire already does — into at most [`BOOST_MAX_ACTIVE`] side
//! objectives, each carrying the turn it stops being worth anything.
//!
//! 1. **Horizon** ([`AdvancedAi::boost_horizon`]). The beeline picker's own
//!    comparator — `min_by(beeline_step_cost)` in `advanced_research` — run
//!    forward over the legal frontier, with the node under study first. Each
//!    entry carries its projected research start turn and a deadline.
//! 2. **Trigger cost table** ([`AdvancedAi::boost_trigger_class`]). Every
//!    horizon node's `BoostSpec` is read through the engine's own
//!    [`Game::boost_progress`] and classified `Satisfied`, `Cheap`,
//!    `Expensive` or `ImpossibleNow`. Only `Cheap` becomes a side objective.
//! 3. **Side objectives with deadlines**
//!    ([`AdvancedAi::boost_side_objectives`]). A cheap trigger becomes a
//!    [`BOOST_PREMIUM_PCT`] premium on the exact production, improvement or
//!    placement choice that satisfies it, expiring at the node's deadline. The
//!    premium is a share of the choice's *own* value, so it re-orders
//!    near-equals and can never invent a reason to build something.
//! 4. **Research deferral** ([`AdvancedAi::boost_planner_defer_pick`]). A node
//!    whose boost is committed and about to land within [`BOOST_DEFER_TURNS`]
//!    turns, and which the empire would otherwise finish first, yields its
//!    slot to another node **on the same beeline** costing no more than those
//!    three turns — so the lane's next unlock is never pushed back further
//!    than the boost window itself.
//! 5. **Journal.** Every creation, satisfaction, expiry and deferral is
//!    written with the boost's own name, so a run's `why.log` says what the
//!    planner committed to and whether it collected.
//!
//! Off, every entry point returns before reading anything and every path is
//! byte-identical.

use std::collections::BTreeSet;

use super::AdvancedAi;
use crate::game::{Game, Item};
use crate::name::Name;
use crate::reasoning::plain;
use crate::rules::BoostSpec;
use crate::think;
use crate::Pos;

/// How many technologies of the beeline the planner looks at. Six is roughly
/// twenty to forty turns of research in the classical and medieval eras — far
/// enough that a Builder charge or a unit can be steered in time, near enough
/// that the beeline projection is still the path the lane will actually walk.
pub(super) const BOOST_HORIZON: usize = 6;

/// How many civics of the beeline the planner looks at. Fewer than the
/// technologies on purpose: civics are cheaper, the tree branches harder, and
/// the measured inspiration rate (0–26 %) says the seat rarely holds one long
/// enough for a distant projection to survive.
pub(super) const BOOST_CIVIC_HORIZON: usize = 4;

/// The premium a live side objective pays, as a percentage of the candidate's
/// **own** value. Fifteen percent is a tie-break: it lifts a choice over one
/// the planner already rated within 15 % of it and over nothing else. The two
/// genes that measured negative in this family both paid an *absolute*
/// premium — `eureka-chasing-production`'s 4.0 per beaker added +234 to a
/// Trebuchet worth 95, and `boost-first-research` v1 added +37 against unlocks
/// worth single digits. A relative premium cannot do that.
pub(super) const BOOST_PREMIUM_PCT: f64 = 15.0;

/// How many side objectives may be live at once. Three: a Builder charge, a
/// production slot and a settle site are about as many independent decisions
/// as an empire makes in a turn, and a planner holding more of them is
/// chasing again.
pub(super) const BOOST_MAX_ACTIVE: usize = 3;

/// The deferral window, in turns. A boost committed and landing inside three
/// turns is worth waiting for; beyond that the engine's own mid-research
/// credit reaches the node anyway (`Game::do_research` credits a boost that
/// lands on a node already being worked), and a longer wait is
/// `boost-wait-research` version one's mistake — a six-turn window that
/// repeatedly delayed useful prerequisites for a trigger that never arrived.
pub(super) const BOOST_DEFER_TURNS: f64 = 3.0;

/// A Builder improvement is assumed to land this many turns out once a
/// Builder with charges is standing: the charge is spent the turn the job is
/// chosen. Named so the deferral's arithmetic is not a bare `1.0`.
const BOOST_BUILDER_FIRE_TURNS: f64 = 1.0;

/// What one horizon node's boost trigger costs the plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum BoostTriggerClass {
    /// `have >= need` already — the engine will credit it; nothing to plan.
    Satisfied,
    /// Work the empire already does. The only class that becomes a side
    /// objective.
    Cheap(BoostAction),
    /// A wonder, a specific war act, a religion, a great person: real
    /// strategic spending that must win on its own merits, never because a
    /// boost hangs off it.
    Expensive,
    /// Nothing the empire may do this turn advances it — the trigger's own
    /// technology or civic is unresearched, or only time, growth or contact
    /// moves it.
    ImpossibleNow,
}

/// Which decision a cheap trigger attaches its premium to.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum BoostAction {
    /// A Builder job: this improvement, under this constraint on the tile.
    Improvement {
        improvement: String,
        on: TileRequirement,
    },
    /// One more unit of this kind out of a city queue.
    Unit(String),
    /// One more district of this family.
    District(String),
    /// The next city, placed on the coast.
    CoastalCity,
}

/// What the tile under a chased improvement must carry, in the spelling
/// `Game::boost_progress` counts it.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum TileRequirement {
    /// `improvement:` — any owned tile.
    Any,
    /// `improvement_on_resource:` — any tile carrying any resource.
    AnyResource,
    /// `improve_resource:` — a tile carrying this named resource.
    Named(String),
}

/// One live side objective: a cheap trigger on a horizon node, the decision it
/// attaches to, and the turn it stops being worth anything.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct BoostSideObjective {
    /// The technology or civic whose boost this is.
    pub node: Name,
    /// `true` for a technology, `false` for a civic.
    pub techs: bool,
    /// The trigger verbatim, for the journal.
    pub trigger: String,
    /// The decision the premium attaches to.
    pub action: BoostAction,
    /// The last turn the premium is paid; see [`BoostHorizonNode::deadline`].
    pub deadline: u32,
    /// Research the boost grants at this game speed — the ranking key when
    /// more than [`BOOST_MAX_ACTIVE`] triggers are cheap.
    pub payout: f64,
}

/// One projected step of the beeline.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct BoostHorizonNode {
    pub node: Name,
    /// The turn the empire is projected to *start* researching this node; the
    /// current turn for the node already under study.
    pub start_turn: u32,
    /// The last turn a premium for this node's boost is paid. The projected
    /// start turn for a node not yet begun — after that the empire is paying
    /// full price and the trigger has missed its window. For the node
    /// **already under study** it is the projected completion turn instead,
    /// because the engine credits a boost that lands mid-research
    /// (`advanced/boost_research.rs`), so the window has not closed.
    pub deadline: u32,
}

/// The gene's whole state: the side objectives the empire has committed to,
/// and the turn and seat they were last advanced for. Objectives are carried
/// forward rather than re-derived, which is what gives their deadlines force;
/// see [`AdvancedAi::boost_planner_refresh`].
#[derive(Clone, Default)]
pub(super) struct BoostPlannerFrame {
    /// `(turn, pid)` the objectives below were computed for.
    stamp: Option<(u32, usize)>,
    objectives: Vec<BoostSideObjective>,
    /// Whether the empire-wide stand-down (defence) is in force this turn.
    defence_stand_down: bool,
}

/// The deferral the research picker is offered.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct BoostDeferral {
    /// The node to research instead.
    pub pick: Name,
    /// The node whose boost is being waited for.
    pub node: Name,
    /// That boost's trigger, for the journal.
    pub trigger: String,
}

impl AdvancedAi {
    // ---- the horizon ------------------------------------------------

    /// The next `limit` nodes of the beeline, in the order the empire is
    /// projected to take them.
    ///
    /// This is the beeline picker's own comparator — the
    /// `min_by(beeline_step_cost)` that `advanced_research`'s `goal_pick`
    /// uses — run forward over the legal frontier: at each step the unheld
    /// nodes whose prerequisites are all held or already picked, cheapest
    /// effective step first, ties by name exactly as the picker breaks them.
    /// The node under study, if any, comes first and keeps the current turn as
    /// its start.
    ///
    /// It is a projection, not a promise: a forced lane goal can overturn any
    /// step. That is precisely why a side objective carries a deadline and
    /// expires rather than persisting.
    pub(super) fn boost_horizon(
        &self,
        g: &Game,
        pid: usize,
        techs: bool,
        limit: usize,
    ) -> Vec<BoostHorizonNode> {
        let player = &g.players[pid];
        let (specs, known, studying, progress) = if techs {
            (
                &g.rules.techs,
                &player.techs,
                player.research.as_deref(),
                player.research_progress,
            )
        } else {
            (
                &g.rules.civics,
                &player.civics,
                player.civic.as_deref(),
                player.civic_progress,
            )
        };
        let rate = Self::research_rate(g, pid, techs);
        let cost_of = |node: &str| {
            if techs {
                g.tech_cost(node)
            } else {
                g.civic_cost(node)
            }
        };
        let mut horizon: Vec<BoostHorizonNode> = Vec::new();
        let mut picked: BTreeSet<Name> = BTreeSet::new();
        // Beakers the empire must spend before the next unpicked node starts.
        let mut committed = 0.0_f64;
        if let Some(node) = studying.filter(|node| specs.contains_key(node)) {
            let left = (cost_of(node) - progress).max(0.0);
            horizon.push(BoostHorizonNode {
                node: Name::new(node),
                start_turn: g.turn,
                // Already under study: the mid-research credit still lands, so
                // the window runs to completion.
                deadline: Self::boost_turn_after(g, left, rate),
            });
            picked.insert(Name::new(node));
            committed += left;
        }
        while horizon.len() < limit {
            let Some(next) = specs
                .iter()
                .filter(|(node, _)| !known.contains(*node) && !picked.contains(*node))
                .filter(|(_, spec)| {
                    spec.requires
                        .iter()
                        .all(|need| known.contains(need) || picked.contains(need))
                })
                .min_by(|(left, _), (right, _)| {
                    self.beeline_step_cost(g, pid, left.as_str(), techs)
                        .total_cmp(&self.beeline_step_cost(g, pid, right.as_str(), techs))
                        .then_with(|| left.cmp(right))
                })
                .map(|(node, _)| *node)
            else {
                break;
            };
            let start_turn = Self::boost_turn_after(g, committed, rate);
            horizon.push(BoostHorizonNode {
                node: next,
                start_turn,
                deadline: start_turn,
            });
            picked.insert(next);
            committed += self.boost_effective_cost(g, pid, next.as_str(), techs);
        }
        horizon
    }

    /// The turn `research` more beakers at `rate` a turn lands on, never
    /// before the current turn.
    fn boost_turn_after(g: &Game, research: f64, rate: f64) -> u32 {
        g.turn + (research.max(0.0) / rate.max(1.0)).ceil() as u32
    }

    /// What the empire will actually pay for `node`: the printed, speed-scaled
    /// cost less a boost already in hand. The projection spends what the
    /// engine will charge, not the sticker price.
    fn boost_effective_cost(&self, g: &Game, pid: usize, node: &str, techs: bool) -> f64 {
        let cost = if techs {
            g.tech_cost(node)
        } else {
            g.civic_cost(node)
        };
        if Self::boost_in_hand(g, pid, node, techs) {
            cost * (1.0 - Self::boost_frac(g, node, techs).clamp(0.0, 0.99))
        } else {
            cost
        }
    }

    // ---- the trigger cost table -------------------------------------

    /// What satisfying `boost` would cost the plan.
    ///
    /// The cheap classes are exactly the four kinds of work the empire is
    /// already doing, and each is guarded by a test that it is *already* doing
    /// it — an improvement type it has built before, a unit type it already
    /// fields, a district family it already builds, a Settler already walking.
    /// Everything the empire would have to *start* doing is `Expensive`,
    /// including buildings: `chase-every-boost` and
    /// `eureka-chasing-production` already price those, and this gene
    /// deliberately adds nothing on top of a premium that already ships.
    /// Anything gated on a node the empire does not hold, and anything only
    /// time, growth or contact advances, is `ImpossibleNow`.
    pub(super) fn boost_trigger_class(
        &self,
        g: &Game,
        pid: usize,
        boost: &BoostSpec,
    ) -> BoostTriggerClass {
        let (have, need) = g.boost_progress(pid, boost);
        if have >= need {
            return BoostTriggerClass::Satisfied;
        }
        let trigger = boost.trigger.as_str();
        if !Self::boost_trigger_unlocked(g, pid, trigger) {
            return BoostTriggerClass::ImpossibleNow;
        }
        if let Some(improvement) = trigger.strip_prefix("improvement:") {
            return Self::boost_improvement_class(g, pid, improvement, TileRequirement::Any);
        }
        if let Some(improvement) = trigger.strip_prefix("improvement_on_resource:") {
            return Self::boost_improvement_class(
                g,
                pid,
                improvement,
                TileRequirement::AnyResource,
            );
        }
        if let Some(resource) = trigger.strip_prefix("improve_resource:") {
            let Some(spec) = g.rules.resources.get(resource) else {
                return BoostTriggerClass::ImpossibleNow;
            };
            return Self::boost_improvement_class(
                g,
                pid,
                spec.improvement.as_str(),
                TileRequirement::Named(resource.to_string()),
            );
        }
        if let Some(kind) = trigger.strip_prefix("units_of:") {
            // "A unit type we already own at least one of, and need one more."
            // Both halves matter: the first says the empire builds these, the
            // second says one ordinary build finishes the trigger.
            let owned = g
                .units
                .values()
                .filter(|unit| unit.owner == pid && unit.kind.as_str() == kind)
                .count() as i64;
            return if owned >= 1 && need - have == 1 {
                BoostTriggerClass::Cheap(BoostAction::Unit(kind.to_string()))
            } else {
                BoostTriggerClass::Expensive
            };
        }
        if let Some(family) = trigger.strip_prefix("district:") {
            // "A district we plan anyway": one of this family already stands,
            // so the next is the empire's ordinary build order, not a detour.
            let built = g
                .cities
                .values()
                .filter(|city| city.owner == pid)
                .flat_map(|city| city.districts.keys())
                .any(|have| g.district_family(*have).as_str() == family);
            return if built {
                BoostTriggerClass::Cheap(BoostAction::District(family.to_string()))
            } else {
                BoostTriggerClass::Expensive
            };
        }
        if trigger == "coastal_city" {
            // "A city on the coast we are about to found": cheap only while a
            // Settler is already walking. With none, this is a whole extra
            // city, which is not a boost decision.
            let settling = g
                .units
                .values()
                .any(|unit| unit.owner == pid && unit.kind.as_str() == "settler");
            return if settling {
                BoostTriggerClass::Cheap(BoostAction::CoastalCity)
            } else {
                BoostTriggerClass::Expensive
            };
        }
        if Self::boost_trigger_is_strategic_spending(trigger) {
            return BoostTriggerClass::Expensive;
        }
        BoostTriggerClass::ImpossibleNow
    }

    /// A trigger whose thing the empire is not yet allowed to build cannot be
    /// planned for at all. `trigger_gates` returns the nodes that gate the
    /// improvement, unit, building or district a trigger names, and an empty
    /// list for every trigger that names none of those.
    fn boost_trigger_unlocked(g: &Game, pid: usize, trigger: &str) -> bool {
        Self::trigger_gates(g, trigger)
            .iter()
            .all(|(gate, techs)| Self::node_known(g, pid, gate.as_str(), *techs))
    }

    /// An improvement trigger is cheap when the empire has already put that
    /// improvement on the ground somewhere: it is a Builder job the planner
    /// already chooses, so a premium re-orders charges rather than inventing
    /// a new habit.
    fn boost_improvement_class(
        g: &Game,
        pid: usize,
        improvement: &str,
        on: TileRequirement,
    ) -> BoostTriggerClass {
        let built = g
            .cities
            .values()
            .filter(|city| city.owner == pid)
            .flat_map(|city| city.owned_tiles.iter())
            .any(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.improvement.as_deref() == Some(improvement))
            });
        if built {
            BoostTriggerClass::Cheap(BoostAction::Improvement {
                improvement: improvement.to_string(),
                on,
            })
        } else {
            BoostTriggerClass::Expensive
        }
    }

    /// The triggers that name real strategic spending — a wonder, a war act, a
    /// religion, a great person, a national park, a themed museum. Each is a
    /// decision that must win on its own merits; a boost is never the reason
    /// to take one.
    fn boost_trigger_is_strategic_spending(trigger: &str) -> bool {
        matches!(
            trigger,
            "wonders"
                | "wonder_era"
                | "war"
                | "received_dow"
                | "casus_belli"
                | "kills"
                | "barbs_killed"
                | "camps"
                | "captures"
                | "religion"
                | "religion_cities"
                | "pantheon"
                | "great_people"
                | "national_park"
                | "themed_buildings"
                | "artifacts"
                | "airbase_foreign_continent"
        ) || trigger.starts_with("kill_with:")
            || trigger.starts_with("kill_kind:")
            || trigger.starts_with("trained:")
            || trigger.starts_with("great_person_of:")
            || trigger.starts_with("building:")
            || trigger.starts_with("building_near_mountain:")
            || trigger.starts_with("unit_and_improve:")
    }

    // ---- side objectives --------------------------------------------

    /// The live side objectives, at most [`BOOST_MAX_ACTIVE`] of them, ranked
    /// by the research at stake. Empty with the gene off. Memoised per turn
    /// and player; the journal lines are written by the refresh.
    pub(super) fn boost_side_objectives(&self, g: &Game, pid: usize) -> Vec<BoostSideObjective> {
        if !self.boost_planner {
            return Vec::new();
        }
        self.boost_planner_refresh(g, pid);
        self.boost_planner_frame.borrow().objectives.clone()
    }

    /// The candidate objectives the horizon offers this turn, richest first.
    /// The cap is applied by the refresh, after standing commitments are kept.
    fn boost_horizon_candidates(&self, g: &Game, pid: usize) -> Vec<BoostSideObjective> {
        let mut found: Vec<BoostSideObjective> = Vec::new();
        for (techs, limit) in [(true, BOOST_HORIZON), (false, BOOST_CIVIC_HORIZON)] {
            for step in self.boost_horizon(g, pid, techs, limit) {
                // The node the empire is about to buy this very turn has no
                // window left in which to earn its trigger.
                if g.turn >= step.deadline {
                    continue;
                }
                if let Some(objective) =
                    self.boost_objective_for(g, pid, step.node, techs, step.deadline)
                {
                    found.push(objective);
                }
            }
        }
        found.sort_by(Self::boost_by_payout);
        found
    }

    /// The objective `node` implies, if its boost is still open and its
    /// trigger is cheap.
    fn boost_objective_for(
        &self,
        g: &Game,
        pid: usize,
        node: Name,
        techs: bool,
        deadline: u32,
    ) -> Option<BoostSideObjective> {
        if Self::boost_in_hand(g, pid, node.as_str(), techs)
            || Self::node_known(g, pid, node.as_str(), techs)
        {
            return None;
        }
        let specs = if techs {
            &g.rules.techs
        } else {
            &g.rules.civics
        };
        let spec = specs.get(node.as_str())?;
        let boost = spec.boost.as_ref()?;
        let BoostTriggerClass::Cheap(action) = self.boost_trigger_class(g, pid, boost) else {
            return None;
        };
        Some(BoostSideObjective {
            node,
            techs,
            trigger: boost.trigger.clone(),
            action,
            deadline,
            payout: Self::boost_payout(g, spec.cost, boost),
        })
    }

    /// The research at stake decides which objectives the cap keeps; the name
    /// and the tree break ties so the choice is deterministic.
    fn boost_by_payout(
        left: &BoostSideObjective,
        right: &BoostSideObjective,
    ) -> std::cmp::Ordering {
        right
            .payout
            .total_cmp(&left.payout)
            .then_with(|| left.node.cmp(&right.node))
            .then_with(|| left.techs.cmp(&right.techs))
    }

    /// The research a boost grants at this game speed.
    fn boost_payout(g: &Game, printed: f64, boost: &BoostSpec) -> f64 {
        g.game_speed.scale(printed) * boost.percent.unwrap_or(40.0) / 100.0
    }

    /// Advance the frame to this turn, and write what changed.
    ///
    /// A side objective is a **commitment**, not a re-derivation: one taken up
    /// stands, with the deadline it was given, until the boost is collected,
    /// the node is researched, the trigger stops being cheap, or the deadline
    /// passes. That is what makes the deadline load-bearing — and it is the
    /// difference between this gene and a chase, which re-prices every
    /// reachable trigger every turn and therefore follows the beeline's every
    /// wobble. Only the free slots under [`BOOST_MAX_ACTIVE`] are filled from
    /// the horizon.
    fn boost_planner_refresh(&self, g: &Game, pid: usize) {
        let stamp = self.boost_planner_frame.borrow().stamp;
        if stamp == Some((g.turn, pid)) {
            return;
        }
        // A different seat, or a rewound clock, is a different game: nothing
        // is carried into it.
        let carried = match stamp {
            Some((turn, owner)) if owner == pid && turn <= g.turn => {
                self.boost_planner_frame.borrow().objectives.clone()
            }
            _ => Vec::new(),
        };
        let mut kept: Vec<BoostSideObjective> = Vec::new();
        let mut dropped: Vec<BoostSideObjective> = Vec::new();
        for standing in carried {
            match self.boost_objective_for(g, pid, standing.node, standing.techs, standing.deadline)
            {
                Some(fresh) if g.turn <= standing.deadline => kept.push(fresh),
                _ => dropped.push(standing),
            }
        }
        let held: BTreeSet<(Name, bool)> = kept
            .iter()
            .map(|objective| (objective.node, objective.techs))
            .collect();
        let mut created: Vec<BoostSideObjective> = Vec::new();
        for candidate in self.boost_horizon_candidates(g, pid) {
            if kept.len() + created.len() >= BOOST_MAX_ACTIVE {
                break;
            }
            if held.contains(&(candidate.node, candidate.techs)) {
                continue;
            }
            created.push(candidate);
        }
        if self.journal().wants(crate::reasoning::Level::Decision) {
            self.boost_planner_journal_diff(g, pid, &created, &dropped);
        }
        let mut objectives = kept;
        objectives.extend(created);
        objectives.sort_by(Self::boost_by_payout);
        let defence_stand_down = self.threatened_city(g, pid).is_some();
        *self.boost_planner_frame.borrow_mut() = BoostPlannerFrame {
            stamp: Some((g.turn, pid)),
            objectives,
            defence_stand_down,
        };
    }

    /// Say what was taken up, and what was collected or let go.
    fn boost_planner_journal_diff(
        &self,
        g: &Game,
        pid: usize,
        created: &[BoostSideObjective],
        dropped: &[BoostSideObjective],
    ) {
        for objective in created {
            think!(self.journal(), Research, Decision,
                   "Chasing the boost for {}", plain(objective.node.as_str());
                   "{} completes it; worth {:.0} research, wanted by turn {}",
                   objective.trigger, objective.payout, objective.deadline);
        }
        for objective in dropped {
            let collected = Self::boost_in_hand(g, pid, objective.node.as_str(), objective.techs);
            think!(self.journal(), Research, Decision,
                   "{} the boost chase for {}",
                   if collected { "Collected" } else { "Dropped" },
                   plain(objective.node.as_str());
                   "{} ({}), deadline turn {}",
                   if collected {
                       "the trigger fired and the discount is banked"
                   } else {
                       "the deadline passed, the node landed, or the trigger stopped being cheap"
                   },
                   objective.trigger, objective.deadline);
        }
    }

    // ---- the premium ------------------------------------------------

    /// The premium a live side objective pays on a production candidate.
    /// Zero with the gene off, when the planner stands down, and for every
    /// item no objective names.
    pub(super) fn boost_planner_production_premium(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        raw: f64,
        plan: &super::StrategicPlan,
    ) -> f64 {
        if !self.boost_planner {
            return 0.0;
        }
        // The match is tested first: it is a scan of at most three objectives,
        // where the stand-down reaches the vision frame and the science
        // drive's launch city. This runs for every candidate item in every
        // city, every turn.
        if !self.boost_planner_names(g, pid, |action| Self::boost_item_satisfies(g, action, item)) {
            return 0.0;
        }
        if self.boost_planner_stands_down(g, pid, Some((cid, plan))) {
            return 0.0;
        }
        Self::boost_premium_on(raw)
    }

    /// The premium a live side objective pays on a Builder job.
    pub(super) fn boost_planner_builder_premium(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        value: f64,
    ) -> f64 {
        if !self.boost_planner {
            return 0.0;
        }
        let Some(pid) = g
            .map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.owner)
        else {
            return 0.0;
        };
        if !self.boost_planner_names(g, pid, |action| {
            Self::boost_tile_satisfies(g, action, pos, improvement)
        }) {
            return 0.0;
        }
        if self.boost_planner_stands_down(g, pid, None) {
            return 0.0;
        }
        Self::boost_premium_on(value)
    }

    /// The premium a live coastal-city objective pays on a settle site.
    pub(super) fn boost_planner_site_premium(
        &self,
        g: &Game,
        pid: usize,
        pos: Pos,
        value: f64,
    ) -> f64 {
        if !self.boost_planner {
            return 0.0;
        }
        if !self.boost_planner_names(g, pid, |action| *action == BoostAction::CoastalCity) {
            return 0.0;
        }
        let coastal = g
            .nbrs(pos)
            .iter()
            .any(|nb| g.map.get(*nb).is_some_and(|tile| g.rules.is_water(tile)));
        if !coastal || self.boost_planner_stands_down(g, pid, None) {
            return 0.0;
        }
        Self::boost_premium_on(value)
    }

    /// Does any live side objective name this decision? The frame is read in
    /// place rather than cloned: every premium seam asks this question for
    /// every candidate it prices.
    fn boost_planner_names(
        &self,
        g: &Game,
        pid: usize,
        names: impl Fn(&BoostAction) -> bool,
    ) -> bool {
        self.boost_planner_refresh(g, pid);
        self.boost_planner_frame
            .borrow()
            .objectives
            .iter()
            .any(|objective| names(&objective.action))
    }

    /// [`BOOST_PREMIUM_PCT`] of the candidate's own positive value. A share
    /// rather than a sum: it can re-order two choices the planner already
    /// rated within 15 % of each other and can do nothing else — in
    /// particular it can never lift a choice the planner priced at or below
    /// zero.
    fn boost_premium_on(value: f64) -> f64 {
        value.max(0.0) * BOOST_PREMIUM_PCT / 100.0
    }

    /// Does building `item` advance this objective?
    fn boost_item_satisfies(g: &Game, action: &BoostAction, item: &Item) -> bool {
        match (action, item) {
            (BoostAction::Unit(kind), Item::Unit { unit } | Item::Formation { unit, .. }) => {
                unit.as_str() == kind
            }
            (BoostAction::District(family), Item::District { district, .. }) => {
                g.district_family(*district).as_str() == family
            }
            _ => false,
        }
    }

    /// Does putting `improvement` on `pos` advance this objective?
    fn boost_tile_satisfies(g: &Game, action: &BoostAction, pos: Pos, improvement: &str) -> bool {
        let BoostAction::Improvement {
            improvement: want,
            on,
        } = action
        else {
            return false;
        };
        if want != improvement {
            return false;
        }
        let Some(tile) = g.map.get(pos) else {
            return false;
        };
        match on {
            TileRequirement::Any => true,
            TileRequirement::AnyResource => tile.resource.is_some(),
            TileRequirement::Named(resource) => tile.resource.as_deref() == Some(resource.as_str()),
        }
    }

    /// The three things a boost never outranks.
    ///
    /// 1. **Defence.** While any owned city is under real pressure
    ///    (`threatened_city`, the empire-level test the recovery plan itself
    ///    uses) every premium stands down.
    /// 2. **Settlers in the opening band.** Inside the expansion band and
    ///    behind its winning pace (`expansion_band_turn` / `expansion_pace`;
    ///    every recorded win came from four to six cities by turn 60), the
    ///    *production* premium stands down, so a trigger can never take a
    ///    Settler's slot. Builder charges and settle sites are untouched —
    ///    they do not compete with a Settler, and the opening is where a
    ///    Eureka is worth most.
    /// 3. **Victory-project reservations.** In the science drive's launch
    ///    city the queue belongs to the space projects; no premium is paid
    ///    there at all.
    fn boost_planner_stands_down(
        &self,
        g: &Game,
        pid: usize,
        production: Option<(u32, &super::StrategicPlan)>,
    ) -> bool {
        self.boost_planner_refresh(g, pid);
        if self.boost_planner_frame.borrow().defence_stand_down {
            return true;
        }
        let Some((cid, plan)) = production else {
            return false;
        };
        if plan.threatened_city == Some(cid) {
            return true;
        }
        if Self::boost_planner_opening_band(g, pid) {
            return true;
        }
        self.science_drive_launch_city(g, pid) == Some(cid)
    }

    /// Is the empire inside the expansion band and behind its winning pace —
    /// the state in which a Settler outranks everything else in a queue?
    fn boost_planner_opening_band(g: &Game, pid: usize) -> bool {
        g.turn <= Self::expansion_band_turn(g)
            && g.cities.values().filter(|city| city.owner == pid).count() < Self::expansion_pace(g)
    }

    // ---- research deferral -------------------------------------------

    /// A node to research *instead of* `ordinary`, so a committed boost lands
    /// before the empire pays full price for the node it belongs to.
    ///
    /// The whole rule, and every bound on it:
    ///
    /// - `ordinary` must itself be a live side objective's node — so its
    ///   trigger is cheap, in the horizon, and inside its deadline.
    /// - That trigger must be **in progress**, not merely possible: a unit,
    ///   building or district at the front of an owned city queue
    ///   (`boost_trigger_is_queued`, the strict test
    ///   `boost-wait-research-2` uses), or an improvement with a Builder
    ///   standing that has a charge to spend.
    /// - The boost must land within [`BOOST_DEFER_TURNS`], and the empire
    ///   must be about to finish `ordinary` first — otherwise the engine's
    ///   own mid-research credit reaches it and nothing needs deferring.
    /// - The replacement must be **on the same beeline**: when the lane forced
    ///   a goal, it must lead to that goal, so the lane loses nothing but
    ///   order. And it must cost no more than [`BOOST_DEFER_TURNS`] of
    ///   research, which is the bound the design asks for — the lane's next
    ///   unlock is never pushed back further than the boost window itself.
    ///
    /// `None` with the gene off, so the picker is byte-identical.
    pub(super) fn boost_planner_defer_pick(
        &self,
        g: &Game,
        pid: usize,
        available: &[Name],
        ordinary: &Name,
        goal: Option<&str>,
        techs: bool,
    ) -> Option<BoostDeferral> {
        if !self.boost_planner {
            return None;
        }
        let objective = self
            .boost_side_objectives(g, pid)
            .into_iter()
            .find(|objective| objective.techs == techs && objective.node == *ordinary)?;
        let fire_turns = self.boost_trigger_fire_turns(g, pid, &objective)?;
        if fire_turns > BOOST_DEFER_TURNS {
            return None;
        }
        let rate = Self::research_rate(g, pid, techs);
        let finish_turns = self.boost_effective_cost(g, pid, ordinary.as_str(), techs) / rate;
        if finish_turns > fire_turns {
            // The node outlives its own trigger; the mid-research credit lands
            // on it and there is nothing to defer.
            return None;
        }
        let pick = available
            .iter()
            .filter(|node| *node != ordinary)
            .filter(|node| match goal {
                Some(goal) if techs => self.tech_leads_to(g, node.as_str(), goal),
                Some(goal) => self.civic_leads_to(g, node.as_str(), goal),
                None => true,
            })
            .filter(|node| {
                self.boost_effective_cost(g, pid, node.as_str(), techs) / rate <= BOOST_DEFER_TURNS
            })
            .min_by(|left, right| {
                self.beeline_step_cost(g, pid, left.as_str(), techs)
                    .total_cmp(&self.beeline_step_cost(g, pid, right.as_str(), techs))
                    .then_with(|| left.cmp(right))
            })
            .copied()?;
        Some(BoostDeferral {
            pick,
            node: objective.node,
            trigger: objective.trigger,
        })
    }

    /// How many turns until this objective's trigger fires, if the empire has
    /// actually committed to it; `None` when it has not.
    fn boost_trigger_fire_turns(
        &self,
        g: &Game,
        pid: usize,
        objective: &BoostSideObjective,
    ) -> Option<f64> {
        match &objective.action {
            BoostAction::Improvement { .. } => {
                // A Builder with a charge spends it on the job it is offered
                // this turn, and the premium above is what offers this one.
                let ready = g.units.values().any(|unit| {
                    unit.owner == pid && unit.kind.as_str() == "builder" && unit.charges > 0
                });
                ready.then_some(BOOST_BUILDER_FIRE_TURNS)
            }
            BoostAction::Unit(_) | BoostAction::District(_) => {
                Self::boost_trigger_is_queued(g, pid, &objective.trigger)
                    .then(|| self.boost_queued_trigger_turns(g, pid, &objective.trigger))
                    .flatten()
            }
            // A city is founded when the Settler arrives, which is not a
            // schedule research may wait on.
            BoostAction::CoastalCity => None,
        }
    }

    /// The build turns left on the queue front that carries this trigger, over
    /// every city holding one; the soonest wins.
    fn boost_queued_trigger_turns(&self, g: &Game, pid: usize, trigger: &str) -> Option<f64> {
        g.cities
            .values()
            .filter(|city| city.owner == pid)
            .filter_map(|city| {
                let item = city.queue.first()?;
                let key = Self::item_trigger_key(g, item)?;
                (key == trigger).then(|| {
                    let production = (g.city_yields(city.id).production
                        * g.item_prod_mult(pid, city.id, Some(item)))
                    .max(1.0);
                    g.item_remaining_cost_for_city(pid, city.id, item) / production
                })
            })
            .min_by(f64::total_cmp)
    }
}

#[cfg(test)]
mod tests;
