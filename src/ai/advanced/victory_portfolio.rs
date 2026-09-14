//! Persistent victory objectives, independent of the current military posture.
//!
//! Forecasts describe a feasible continuation of today's observed capacity;
//! they are estimates, not win probabilities or permission to issue an order.
//! Missing capacity stays visible as a bottleneck. In particular, progress
//! percentages from different victory conditions are never compared as clocks.

use super::{AdvancedAi, GrandStrategy, StrategicPlan, VictoryTarget};
use crate::game::{Game, Item};
use crate::name::Name;
use crate::rules::Yields;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};

const REVIEW_STANDARD_TURNS: u32 = 10;
const HOLD_STANDARD_TURNS: u32 = 20;
const TRACE_LIMIT: usize = 96;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DevelopmentPhase {
    #[default]
    Foundation,
    Buildup,
    Finish,
}

/// Absolute finish turns. `None` means that an actionable finish is not yet
/// supported, not that this lane is impossible for the remainder of the game.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VictoryEstimate {
    pub target: VictoryTarget,
    pub finish_turn: Option<f64>,
    pub optimistic_finish_turn: Option<f64>,
    pub confidence: f64,
    pub readiness: f64,
    pub preparation_turns: f64,
    pub bottleneck: String,
    pub committed: bool,
}

impl VictoryEstimate {
    fn new(target: VictoryTarget, readiness: f64, bottleneck: &str) -> Self {
        Self {
            target,
            finish_turn: None,
            optimistic_finish_turn: None,
            confidence: 0.2,
            readiness: readiness.clamp(0.0, 1.0),
            preparation_turns: 0.0,
            bottleneck: bottleneck.into(),
            committed: false,
        }
    }

    fn finish(&mut self, turn: u32, remaining: f64, confidence: f64) {
        if remaining.is_finite() && (0.0..=10_000.0).contains(&remaining) {
            self.finish_turn = Some(f64::from(turn) + remaining);
            self.optimistic_finish_turn = Some(f64::from(turn) + remaining * 0.8);
            self.confidence = confidence.clamp(0.0, 1.0);
        }
    }

    /// Bounded decision utility, deliberately not labelled probability. A
    /// functioning finish carries more evidence than an attractive yield.
    fn utility(&self, turn: u32, deadline: f64) -> f64 {
        let left = (deadline - f64::from(turn)).max(1.0);
        let timing = self.finish_turn.map_or(0.0, |finish| {
            let eta = (finish - f64::from(turn)).max(1.0);
            (left / (left + eta)).clamp(0.0, 1.0) * if finish <= deadline { 1.0 } else { 0.55 }
        });
        0.18 * self.readiness + timing * (0.45 + 0.55 * self.confidence)
    }

    fn keeps_incumbent(
        &self,
        challenger: &Self,
        turn: u32,
        deadline: f64,
        age: u32,
        minimum_hold: u32,
    ) -> bool {
        let can_escape = self.finish_turn.is_none_or(|finish| finish > deadline)
            && challenger
                .finish_turn
                .is_some_and(|finish| finish <= deadline)
            && challenger.confidence >= 0.45;
        let margin = if self.committed { 0.12 } else { 0.06 };
        !can_escape
            && (age < minimum_hold
                || challenger.utility(turn, deadline) <= self.utility(turn, deadline) + margin)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PortfolioTrace {
    pub turn: u32,
    pub posture: String,
    pub phase: DevelopmentPhase,
    pub primary: Option<VictoryTarget>,
    pub secondary: Option<VictoryTarget>,
    pub reason: String,
    pub expected_finish: Option<f64>,
    pub rival_finish: Option<f64>,
    pub bottleneck: Option<String>,
    pub cities: usize,
    pub science: f64,
    pub culture: f64,
    pub military: f64,
    pub gold: f64,
    pub faith: f64,
    pub science_projects: usize,
    pub visitors: i64,
    pub diplomatic_points: i64,
    pub capitals: usize,
    /// Observed queue allocation at this snapshot, before issuing orders.
    /// Rates are unmodified city production, not realized expenditure.
    #[serde(default)]
    pub production_allocation: Option<ProductionAllocation>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProductionAllocation {
    pub primary: f64,
    pub secondary: f64,
    pub other: f64,
    pub idle: f64,
    /// A subset of `other`, kept separately to expose late expansion spending.
    pub settlers_and_builders: f64,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PortfolioReport {
    pub enabled: bool,
    pub assigned_target: Option<VictoryTarget>,
    pub primary: Option<VictoryTarget>,
    pub secondary: Option<VictoryTarget>,
    pub phase: DevelopmentPhase,
    /// A smooth preference strength, not a compulsory resource quota.
    pub focus: f64,
    pub committed_turn: Option<u32>,
    pub first_commitment_turn: Option<u32>,
    pub primary_switches: u32,
    pub secondary_switches: u32,
    pub rival_finish: Option<f64>,
    pub reason: String,
    pub forecasts: Vec<VictoryEstimate>,
    pub trace: Vec<PortfolioTrace>,
    /// Number omitted from the bounded trace; aggregate counters never reset.
    pub dropped_trace_points: usize,
}

#[derive(Clone, Debug)]
struct Pace {
    turn: u32,
    yields: Yields,
    visitors: i64,
    dvp: i64,
}

#[derive(Clone, Debug, Default)]
pub(super) struct PortfolioState {
    pub report: PortfolioReport,
    reviewed: Option<u32>,
    sampled: Option<u32>,
    seat: Option<usize>,
    faith_opening: Option<(u32, f64)>,
    samples: BTreeMap<usize, VecDeque<Pace>>,
    last_signature: Option<(usize, usize, usize, i64, Option<VictoryTarget>, u8)>,
}

fn empire_yields(g: &Game, pid: usize) -> Yields {
    let mut yields = g.player_yield_extras(pid);
    for cid in g.player_city_ids(pid) {
        yields.add(g.city_yields(cid));
    }
    if let Some(adjustment) = g.observed_yield_adjustments.get(&pid) {
        yields.add(*adjustment);
    }
    yields
}

fn opponents(g: &Game, pid: usize) -> Vec<usize> {
    g.players
        .iter()
        .filter(|p| {
            p.id != pid
                && p.alive
                && !p.is_minor
                && !p.is_barbarian
                && !g.same_team(pid, p.id)
                && g.has_met(pid, p.id)
        })
        .map(|p| p.id)
        .collect()
}

fn finite_clock(g: &Game) -> Option<f64> {
    g.turn_limit()
        .filter(|limit| *limit > 0 && *limit < 100_000)
        .map(f64::from)
}

fn development_clock(g: &Game) -> f64 {
    finite_clock(g)
        .unwrap_or(f64::from(g.game_speed.turn_limit()))
        .min(f64::from(g.game_speed.turn_limit()))
}

fn old_sample(samples: Option<&VecDeque<Pace>>, turn: u32, gap: u32) -> Option<&Pace> {
    samples?
        .iter()
        .find(|sample| turn.saturating_sub(sample.turn) >= gap)
}

impl AdvancedAi {
    /// Screen the complete decision policy against the existing controller.
    pub fn enable_victory_portfolio(&mut self) {
        self.victory_portfolio = true;
        self.portfolio.reviewed = None;
        self.plan = None;
    }

    pub fn disable_victory_portfolio(&mut self) {
        self.victory_portfolio = false;
        self.portfolio = PortfolioState::default();
        self.plan = None;
    }

    pub fn victory_portfolio_report(&self) -> PortfolioReport {
        self.portfolio.report.clone()
    }

    /// Live frames carry the current decision; the screen writes the full
    /// bounded history once, at the end of the game.
    pub fn victory_portfolio_snapshot(&self) -> PortfolioReport {
        let mut report = self.victory_portfolio_report();
        report.trace = report.trace.into_iter().rev().take(1).collect();
        report
    }

    /// Forecast from the supplied observed board. Used by macro search as well
    /// as the acting controller; callers must not supply hidden opponent facts.
    pub fn victory_forecasts(g: &Game, pid: usize) -> Vec<VictoryEstimate> {
        Self::forecast_portfolio(g, pid, None)
    }

    fn forecast_portfolio(
        g: &Game,
        pid: usize,
        samples: Option<&VecDeque<Pace>>,
    ) -> Vec<VictoryEstimate> {
        let _memo = g.query_memo();
        let yields = empire_yields(g, pid);
        let old = old_sample(samples, g.turn, g.standard_duration(10).max(3));
        let mut result = Vec::new();
        for target in VictoryTarget::ALL {
            if !g.victory_conditions.is_enabled(target.as_str()) {
                continue;
            }
            result.push(match target {
                VictoryTarget::Science => Self::forecast_science(g, pid, yields, old),
                VictoryTarget::Culture => Self::forecast_culture(g, pid, yields, old),
                VictoryTarget::Religion => Self::forecast_religion(g, pid, yields),
                VictoryTarget::Diplomacy => Self::forecast_diplomacy(g, pid, old),
                VictoryTarget::Domination => Self::forecast_domination(g, pid),
                VictoryTarget::Score => Self::forecast_score(g, pid),
            });
        }
        let unknown = g.players.iter().any(|other| {
            other.id != pid
                && other.alive
                && !other.is_minor
                && !other.is_barbarian
                && !g.same_team(pid, other.id)
                && !g.has_met(pid, other.id)
        });
        if unknown {
            for forecast in &mut result {
                if matches!(
                    forecast.target,
                    VictoryTarget::Culture
                        | VictoryTarget::Religion
                        | VictoryTarget::Domination
                        | VictoryTarget::Score
                ) {
                    forecast.optimistic_finish_turn =
                        forecast.finish_turn.or(forecast.optimistic_finish_turn);
                    forecast.finish_turn = None;
                    forecast.confidence = forecast.confidence.min(0.25);
                    forecast.bottleneck = "unmet_rivals".into();
                }
            }
        }
        result
    }

    fn tech_work(g: &Game, pid: usize, goal: Name) -> f64 {
        Self::tech_work_many(g, pid, &[goal])
    }

    fn tech_work_many(g: &Game, pid: usize, goals: &[Name]) -> f64 {
        let player = &g.players[pid];
        let mut required = std::collections::BTreeSet::new();
        for goal in goals {
            if let Some(ancestors) = g.rules.tech_ancestors.get(goal) {
                required.extend(ancestors.iter().cloned());
            }
            required.insert(goal.to_string());
        }
        required
            .iter()
            .filter(|tech| !player.techs.contains(&Name::new(tech)))
            .map(|tech| {
                let invested = if player.research.as_deref() == Some(tech.as_str()) {
                    player.research_progress
                } else {
                    0.0
                };
                (g.tech_cost(tech) - invested).max(0.0)
            })
            .sum()
    }

    fn forecast_science(
        g: &Game,
        pid: usize,
        yields: Yields,
        old: Option<&Pace>,
    ) -> VictoryEstimate {
        let player = &g.players[pid];
        let mut forecast = VictoryEstimate::new(
            VictoryTarget::Science,
            player.techs.len() as f64 / g.rules.techs.len().max(1) as f64,
            "research",
        );
        if player.science_projects.contains("exoplanet_expedition") {
            let distance =
                (g.science_victory_points_needed(pid) - g.science_victory_points(pid)).max(0.0);
            let speed = g.exoplanet_speed(pid);
            forecast.readiness = 0.9;
            forecast.committed = true;
            forecast.bottleneck = "expedition_speed".into();
            if speed > 0.0 {
                forecast.finish(g.turn, distance / speed, 0.95);
            }
            return forecast;
        }
        let Some(cid) = Self::science_drive_pick_launch_city_v2(g, pid) else {
            forecast.bottleneck = "launch_site".into();
            return forecast;
        };
        let city = &g.cities[&cid];
        let production = g.city_yields(cid).production;
        if production <= 0.0 || yields.science <= 0.0 {
            return forecast;
        }
        let science = yields.science;
        // A measured increase can widen the optimistic bound, but never invent
        // a faster central estimate or erase a missing project prerequisite.
        let growth = old.map_or(1.0, |before| {
            (science / before.yields.science.max(1.0)).clamp(1.0, 1.5)
        });
        let pad = crate::name!("spaceport");
        let mut production_ready: f64 = 0.0;
        if !Self::city_has_spaceport(g, cid) {
            let Some(pos) = city
                .queue
                .iter()
                .find_map(|item| match item {
                    Item::District { district, pos } if *district == pad => Some(*pos),
                    _ => None,
                })
                .or_else(|| g.district_sites(cid, pad).into_iter().next())
            else {
                return forecast;
            };
            let item = Item::District { district: pad, pos };
            let bank = if city.queue.first() == Some(&item) {
                city.production
            } else {
                0.0
            };
            production_ready = Self::tech_work(g, pid, crate::name!("rocketry")) / science
                + (g.item_cost_for_city(pid, cid, &item) - bank).max(0.0)
                    / (production * g.item_prod_mult(pid, cid, Some(&item))).max(0.1);
            forecast.preparation_turns = production_ready;
        }
        let mut research_ready: f64 = 0.0;
        let mut research_goals = vec![crate::name!("rocketry")];
        for name in [
            "launch_earth_satellite",
            "launch_moon_landing",
            "launch_mars_colony",
            "exoplanet_expedition",
        ] {
            if player.science_projects.contains(name) {
                forecast.committed = true;
                continue;
            }
            let Some(project) = g.rules.projects.get(name) else {
                return forecast;
            };
            let item = Item::Project {
                project: Name::new(name),
            };
            if let Some(tech) = project.tech {
                research_goals.push(tech);
            }
            let ready = Self::tech_work_many(g, pid, &research_goals) / science;
            research_ready = research_ready.max(ready);
            let bank = if city.queue.first() == Some(&item) {
                forecast.committed = true;
                city.production
            } else {
                0.0
            };
            let build = (g.item_cost_for_city(pid, cid, &item) - bank).max(0.0)
                / (production * g.item_prod_mult(pid, cid, Some(&item))).max(0.1);
            production_ready = production_ready.max(ready) + build;
        }
        forecast.bottleneck = if research_ready > production_ready * 0.55 {
            "research"
        } else {
            "launch_production"
        }
        .into();
        // No free future lasers. Existing production unlocks are options the
        // budget can fund; the central clock assumes the base flight speed.
        forecast.finish(
            g.turn,
            production_ready + g.science_victory_points_needed(pid),
            0.6,
        );
        forecast.optimistic_finish_turn = Some(
            f64::from(g.turn)
                + production_ready / growth
                + g.science_victory_points_needed(pid) / 2.0,
        );
        forecast
    }

    fn forecast_culture(
        g: &Game,
        pid: usize,
        _yields: Yields,
        old: Option<&Pace>,
    ) -> VictoryEstimate {
        let rivals = opponents(g, pid);
        let visitors = g.foreign_tourists(pid) as f64;
        let bar = rivals
            .iter()
            .map(|other| g.domestic_tourists(*other))
            .max()
            .unwrap_or(0) as f64
            + 1.0;
        let mut forecast = VictoryEstimate::new(
            VictoryTarget::Culture,
            visitors / bar.max(1.0),
            "tourism_sources",
        );
        if rivals.is_empty() {
            forecast.bottleneck = "foreign_access".into();
            return forecast;
        }
        let tourism = g.tourism_per_turn(pid).max(0.0);
        let religious = g.religious_tourism_per_turn(pid).clamp(0.0, tourism);
        let starting = g
            .players
            .iter()
            .filter(|p| !p.is_minor && !p.is_barbarian)
            .count()
            .max(1);
        let native_rate = rivals
            .iter()
            .map(|other| {
                (tourism - religious) * g.international_tourism_multiplier(pid, *other, false)
                    + religious * g.international_tourism_multiplier(pid, *other, true)
            })
            .sum::<f64>()
            / (starting as f64 * crate::game::TOURISM_PER_VISITOR);
        let rate = old
            .map(|before| {
                (visitors - before.visitors as f64)
                    / f64::from(g.turn.saturating_sub(before.turn).max(1))
            })
            .filter(|rate| *rate > 0.0)
            .map_or(native_rate, |observed| 0.5 * (observed + native_rate));
        if rate <= 0.0 {
            return forecast;
        }
        // Solve against EVERY defender's growing bar, not today's leader only.
        let mut eta: f64 = 0.0;
        for rival in rivals {
            let gap = g.domestic_tourists(rival) as f64 + 1.0 - visitors;
            if gap <= 0.0 {
                continue;
            }
            let growth = empire_yields(g, rival).culture.max(0.0) / 100.0;
            if rate <= growth {
                forecast.bottleneck = "tourism_growth".into();
                forecast.optimistic_finish_turn = Some(f64::from(g.turn) + gap / rate);
                return forecast;
            }
            eta = eta.max(gap / (rate - growth));
        }
        forecast.committed = visitors > 0.0;
        forecast.bottleneck = "tourism_multipliers".into();
        forecast.finish(g.turn, eta, if old.is_some() { 0.7 } else { 0.45 });
        forecast
    }

    fn forecast_religion(g: &Game, pid: usize, yields: Yields) -> VictoryEstimate {
        let mut forecast =
            VictoryEstimate::new(VictoryTarget::Religion, 0.0, "religion_foundation");
        let Some(faith) = g.players[pid].religion.as_deref() else {
            if g.religions_founded() >= g.max_religions() {
                forecast.bottleneck = "religion_unavailable".into();
            }
            return forecast;
        };
        forecast.committed = true;
        let rivals = opponents(g, pid);
        if rivals.is_empty() {
            forecast.bottleneck = "foreign_access".into();
            return forecast;
        }
        let mut missing_cities = 0usize;
        let mut weakest: f64 = 1.0;
        let mut farthest = 0i32;
        let own = g.player_city_ids(pid);
        for other in rivals.iter().copied().chain(std::iter::once(pid)) {
            if g.civ_follows_religion(other, faith) {
                continue;
            }
            let cities = g.player_city_ids(other);
            if cities.is_empty() {
                forecast.bottleneck = "unknown_conversion_targets".into();
                return forecast;
            }
            let majority = cities.len() / 2 + 1;
            let held = cities
                .iter()
                .filter(|cid| g.city_religion(&g.cities[cid]) == Some(faith))
                .count();
            weakest = weakest.min(held as f64 / majority as f64);
            missing_cities += majority.saturating_sub(held);
            for cid in cities {
                if let Some(distance) = own
                    .iter()
                    .map(|home| g.wdist(g.cities[home].pos, g.cities[&cid].pos))
                    .min()
                {
                    farthest = farthest.max(distance);
                }
            }
        }
        forecast.readiness = 0.25 + 0.75 * weakest;
        forecast.bottleneck = "conversion_holdouts".into();
        let charges: i32 = g
            .player_unit_ids(pid)
            .iter()
            .filter_map(|uid| {
                let unit = &g.units[uid];
                (unit.religion.as_deref() == Some(faith)
                    && g.rules.units[&unit.kind].religious_spread > 0.0)
                    .then_some(unit.charges)
            })
            .sum();
        // Two applications per missing majority city is a capacity estimate;
        // foreign pressure, defenses and access keep its confidence low.
        let needed_charges = (missing_cities as f64 * 2.0 - f64::from(charges.max(0))).max(0.0);
        let price = own
            .iter()
            .filter_map(|cid| {
                g.unit_purchase_cost(pid, *cid, "missionary", "faith")
                    .or_else(|| g.unit_purchase_cost(pid, *cid, "apostle", "faith"))
            })
            .min_by(f64::total_cmp);
        let Some(price) = price.or_else(|| (needed_charges == 0.0).then_some(0.0)) else {
            forecast.bottleneck = "religious_purchase_capacity".into();
            return forecast;
        };
        let funding = (needed_charges / 3.0 * price - g.players[pid].faith.max(0.0)).max(0.0);
        if funding > 0.0 && yields.faith <= 0.0 {
            forecast.bottleneck = "faith_income".into();
            return forecast;
        }
        forecast.preparation_turns = funding / yields.faith.max(1.0);
        forecast.finish(
            g.turn,
            forecast.preparation_turns + f64::from(farthest) / 2.0 + missing_cities as f64 * 3.0,
            0.35,
        );
        forecast
    }

    fn forecast_diplomacy(g: &Game, pid: usize, old: Option<&Pace>) -> VictoryEstimate {
        let points = g.players[pid].dvp.max(0);
        let required = crate::game::DIPLOMATIC_VICTORY_POINTS;
        let mut forecast = VictoryEstimate::new(
            VictoryTarget::Diplomacy,
            points as f64 / required as f64,
            "diplomatic_point_sources",
        );
        let left = (required - points).max(0) as f64;
        if left == 0.0 {
            forecast.finish(g.turn, 0.0, 0.95);
            return forecast;
        }
        let Some(before) = old.filter(|before| points > before.dvp) else {
            return forecast;
        };
        let period = f64::from(g.standard_duration(30).max(1));
        // Banked Favor is not a victory point. Only observed point production
        // supports a central clock, capped at the current Congress cadence.
        let rate = ((points - before.dvp) as f64
            / f64::from(g.turn.saturating_sub(before.turn).max(1)))
        .min(if g.world_era >= 5 { 4.0 } else { 2.0 } / period);
        let sessions = (left / (rate * period).max(0.01)).ceil();
        let first = g.congress.as_ref().map_or(period, |session| {
            f64::from(session.closes.saturating_sub(g.turn))
        });
        forecast.bottleneck = "congress_and_competitions".into();
        forecast.committed = points >= required / 2;
        forecast.finish(g.turn, first + (sessions - 1.0).max(0.0) * period, 0.4);
        forecast
    }

    fn forecast_domination(g: &Game, pid: usize) -> VictoryEstimate {
        let capitals: Vec<_> = g
            .cities
            .values()
            .filter(|city| {
                city.is_capital
                    && !g.players[city.original_owner].is_minor
                    && !g.players[city.original_owner].is_barbarian
            })
            .collect();
        let held = capitals
            .iter()
            .filter(|city| city.owner == pid || g.same_team(pid, city.owner))
            .count();
        let mut forecast = VictoryEstimate::new(
            VictoryTarget::Domination,
            held as f64 / capitals.len().max(1) as f64,
            "capital_access",
        );
        let required = g
            .players
            .iter()
            .filter(|p| !p.is_minor && !p.is_barbarian && p.alive)
            .count();
        if capitals.len() < required {
            return forecast;
        }
        let own = g.player_city_ids(pid);
        if own.is_empty() {
            return forecast;
        }
        let mut travel = 0.0;
        let mut strongest: f64 = 0.0;
        let mut remaining = 0usize;
        for city in capitals
            .iter()
            .filter(|city| city.owner != pid && !g.same_team(pid, city.owner))
        {
            let Some(distance) = own
                .iter()
                .map(|home| g.wdist(g.cities[home].pos, city.pos))
                .min()
            else {
                continue;
            };
            travel += f64::from(distance) / 2.0;
            strongest = strongest.max(g.military_power(city.owner));
            remaining += 1;
        }
        let units = g.player_unit_ids(pid);
        let capture = units
            .iter()
            .any(|uid| g.rules.units[&g.units[uid].kind].is_melee_capable());
        let breach = units
            .iter()
            .any(|uid| g.rules.units[&g.units[uid].kind].siege);
        let walls = capitals
            .iter()
            .any(|city| city.owner != pid && g.city_can_strike(city));
        let ratio = g.military_power(pid) / strongest.max(1.0);
        forecast.committed = held > 1;
        if !capture || (walls && !breach) || ratio < 1.15 {
            forecast.bottleneck = if !capture {
                "capture_unit"
            } else if walls && !breach {
                "wall_damage"
            } else {
                "military_advantage"
            }
            .into();
            return forecast;
        }
        forecast.bottleneck = "capital_capture_and_hold".into();
        forecast.preparation_turns = travel;
        forecast.finish(
            g.turn,
            travel + remaining as f64 * f64::from(g.standard_duration(20).max(5)),
            0.3,
        );
        forecast
    }

    fn forecast_score(g: &Game, pid: usize) -> VictoryEstimate {
        let own = g.team_score_rank_key(pid).0.max(0) as f64;
        let best = opponents(g, pid)
            .iter()
            .map(|other| g.team_score_rank_key(*other).0)
            .max()
            .unwrap_or(0)
            .max(1) as f64;
        let mut forecast = VictoryEstimate::new(
            VictoryTarget::Score,
            (own / best).min(1.0),
            "score_standing",
        );
        if own >= best {
            if let Some(clock) = finite_clock(g) {
                forecast.finish(g.turn, (clock - f64::from(g.turn)).max(0.0), 0.3);
                forecast.optimistic_finish_turn = Some(clock);
            }
        }
        forecast
    }

    pub(super) fn maintain_victory_portfolio(&mut self, g: &Game, pid: usize) {
        if !self.victory_planning || g.players[pid].is_minor || g.players[pid].is_barbarian {
            return;
        }
        if !self.victory_portfolio {
            return;
        }
        if self.portfolio.seat != Some(pid)
            || self.portfolio.sampled.is_some_and(|turn| turn > g.turn)
        {
            self.portfolio = PortfolioState::default();
            self.portfolio.seat = Some(pid);
        }
        if self
            .portfolio
            .faith_opening
            .is_none_or(|(turn, _)| turn != g.turn)
        {
            self.portfolio.faith_opening = Some((g.turn, g.players[pid].faith));
        }
        let target = self.active_victory_target(g);
        let mask = VictoryTarget::ALL
            .iter()
            .enumerate()
            .fold(0u8, |mask, (index, lane)| {
                mask | (u8::from(g.victory_conditions.is_enabled(lane.as_str())) << index)
            });
        let signature = (
            g.player_city_ids(pid).len(),
            g.players[pid].techs.len(),
            g.players[pid].science_projects.len(),
            g.players[pid].dvp,
            target,
            mask,
        );
        let due = self.portfolio.reviewed.is_none_or(|turn| {
            g.turn.saturating_sub(turn) >= g.standard_duration(REVIEW_STANDARD_TURNS).max(1)
        }) || self.portfolio.last_signature != Some(signature);
        if self.portfolio.sampled != Some(g.turn) {
            for other in opponents(g, pid).into_iter().chain(std::iter::once(pid)) {
                let history = self.portfolio.samples.entry(other).or_default();
                history.push_back(Pace {
                    turn: g.turn,
                    yields: empire_yields(g, other),
                    visitors: g.foreign_tourists(other),
                    dvp: g.players[other].dvp,
                });
                while history.front().is_some_and(|sample| {
                    g.turn.saturating_sub(sample.turn) > g.standard_duration(60).max(20)
                }) {
                    history.pop_front();
                }
            }
            self.portfolio.sampled = Some(g.turn);
        }
        if !due {
            return;
        }
        let forecasts = Self::forecast_portfolio(g, pid, self.portfolio.samples.get(&pid));
        let rival_finish = opponents(g, pid)
            .into_iter()
            .flat_map(|other| {
                Self::forecast_portfolio(g, other, self.portfolio.samples.get(&other))
            })
            .filter(|forecast| {
                forecast.confidence >= 0.35 && forecast.target != VictoryTarget::Score
            })
            .filter_map(|forecast| forecast.finish_turn)
            .min_by(f64::total_cmp);
        let deadline = rival_finish
            .unwrap_or(development_clock(g))
            .min(finite_clock(g).unwrap_or(f64::INFINITY));
        let previous = self.portfolio.report.primary;
        let mut primary = target.or_else(|| {
            forecasts
                .iter()
                .filter(|forecast| {
                    forecast.target != VictoryTarget::Score
                        || f64::from(g.turn) >= development_clock(g) * 0.65
                })
                .max_by(|a, b| {
                    a.utility(g.turn, deadline)
                        .total_cmp(&b.utility(g.turn, deadline))
                })
                .map(|forecast| forecast.target)
        });
        let mut reason = if target.is_some() {
            "assigned_target"
        } else {
            "best_supported_finish"
        };
        if target.is_none() && previous != primary {
            if let (Some(incumbent), Some(challenger)) = (
                forecasts
                    .iter()
                    .find(|forecast| Some(forecast.target) == previous),
                forecasts
                    .iter()
                    .find(|forecast| Some(forecast.target) == primary),
            ) {
                let age = self
                    .portfolio
                    .report
                    .committed_turn
                    .map_or(u32::MAX, |turn| g.turn.saturating_sub(turn));
                if incumbent.keeps_incumbent(
                    challenger,
                    g.turn,
                    deadline,
                    age,
                    g.standard_duration(HOLD_STANDARD_TURNS).max(1),
                ) {
                    primary = previous;
                    reason = "retain_invested_plan";
                } else {
                    reason = if incumbent.finish_turn.is_none_or(|finish| finish > deadline) {
                        "incumbent_bottleneck"
                    } else {
                        "better_finish_clears_switch_cost"
                    };
                }
            }
        }
        let secondary = primary.and_then(|primary| {
            let main = forecasts
                .iter()
                .find(|forecast| forecast.target == primary)?;
            forecasts
                .iter()
                .filter(|forecast| {
                    forecast.target != primary && Self::compatible_backup(primary, forecast.target)
                })
                .filter(|forecast| {
                    forecast.readiness >= 0.35
                        && forecast.finish_turn.is_some_and(|finish| {
                            finish <= deadline + f64::from(g.standard_duration(20))
                        })
                })
                .filter(|forecast| {
                    forecast.preparation_turns
                        <= f64::from(g.standard_duration(30)).min(
                            main.finish_turn.map_or(30.0, |finish| {
                                (finish - f64::from(g.turn)).max(0.0) * 0.25
                            }),
                        )
                })
                .max_by(|a, b| {
                    a.utility(g.turn, deadline)
                        .total_cmp(&b.utility(g.turn, deadline))
                })
                .map(|forecast| forecast.target)
        });
        let chosen = forecasts
            .iter()
            .find(|forecast| Some(forecast.target) == primary);
        let city_count = g.player_city_ids(pid).len();
        let matured = city_count >= 4 || g.world_era >= 4;
        let clock_fraction = f64::from(g.turn) / development_clock(g).max(1.0);
        let preparation = chosen.map_or(0.0, |forecast| forecast.preparation_turns);
        let approaching =
            deadline - f64::from(g.turn) <= preparation + f64::from(g.standard_duration(100));
        let durable =
            chosen.is_some_and(|forecast| forecast.committed && forecast.readiness >= 0.5);
        let buildup = matured && (approaching || durable || clock_fraction >= 0.35);
        let mut focus = if buildup {
            ((clock_fraction - 0.25) * 1.5
                + if approaching { 0.2 } else { 0.0 }
                + if durable { 0.2 } else { 0.0 })
            .clamp(0.2, 1.0)
        } else {
            0.0
        };
        let phase = if focus >= 0.75
            || chosen.is_some_and(|forecast| {
                forecast.finish_turn.is_some_and(|finish| {
                    finish - f64::from(g.turn) <= f64::from(g.standard_duration(50))
                }) && forecast.confidence >= 0.6
            }) {
            focus = focus.max(0.85);
            DevelopmentPhase::Finish
        } else if buildup {
            DevelopmentPhase::Buildup
        } else {
            DevelopmentPhase::Foundation
        };
        let report = &mut self.portfolio.report;
        if primary != previous {
            report.primary_switches += u32::from(previous.is_some());
            report.committed_turn = None;
        }
        if primary.is_some()
            && phase != DevelopmentPhase::Foundation
            && report.committed_turn.is_none()
        {
            report.committed_turn = Some(g.turn);
            report.first_commitment_turn.get_or_insert(g.turn);
        }
        report.secondary_switches +=
            u32::from(report.secondary != secondary && report.secondary.is_some());
        let changed =
            report.primary != primary || report.secondary != secondary || report.phase != phase;
        report.enabled = self.victory_portfolio;
        report.assigned_target = target;
        report.primary = primary;
        report.secondary = secondary;
        report.phase = phase;
        report.focus = focus;
        report.rival_finish = rival_finish;
        report.reason = reason.into();
        report.forecasts = forecasts;
        self.portfolio.reviewed = Some(g.turn);
        self.portfolio.last_signature = Some(signature);
        if self.victory_portfolio && changed {
            self.plan = None;
        }
    }

    fn compatible_backup(primary: VictoryTarget, secondary: VictoryTarget) -> bool {
        use VictoryTarget::*;
        matches!(
            (primary, secondary),
            (Science, Culture | Diplomacy)
                | (Culture, Science | Diplomacy | Religion)
                | (Diplomacy, Science | Culture)
                | (Religion, Culture)
                | (Domination, Science)
                | (Score, Science | Culture | Diplomacy)
        )
    }

    pub(super) fn portfolio_target(&self) -> Option<VictoryTarget> {
        self.victory_portfolio
            .then_some(self.portfolio.report.primary)
            .flatten()
    }

    pub(super) fn portfolio_specializing(&self) -> Option<bool> {
        (self.victory_portfolio && self.portfolio.reviewed.is_some())
            .then_some(self.portfolio.report.phase != DevelopmentPhase::Foundation)
    }

    /// A war/defense posture owns urgent choices. During foundation, ordinary
    /// economy choices remain general; the durable objective is still stored.
    pub(super) fn portfolio_objective(&self, posture: GrandStrategy) -> GrandStrategy {
        if matches!(posture, GrandStrategy::Recovery | GrandStrategy::Conquest)
            || self.portfolio_specializing() != Some(true)
        {
            return posture;
        }
        self.portfolio_target()
            .map_or(posture, VictoryTarget::strategy)
    }

    pub(super) fn decision_objective(&self, posture: GrandStrategy) -> GrandStrategy {
        if self.victory_portfolio {
            self.portfolio_objective(posture)
        } else {
            self.victory_target.map_or(posture, VictoryTarget::strategy)
        }
    }

    /// A fallback may spend only a small part of an existing Faith bank.
    /// Foundation and defensive postures keep the ordinary spending rules.
    pub(super) fn portfolio_culture_budget(&self, faith: f64) -> f64 {
        if self.victory_portfolio && self.portfolio.report.primary != Some(VictoryTarget::Culture) {
            if self.portfolio_supports(VictoryTarget::Culture) {
                let opening = self.portfolio.faith_opening.map_or(faith, |(_, bank)| bank);
                (opening * 0.2 - (opening - faith).max(0.0))
                    .max(0.0)
                    .min(faith)
            } else {
                0.0
            }
        } else {
            faith
        }
    }

    pub(super) fn portfolio_tech_bonus(&self, g: &Game, pid: usize, tech: &str) -> f64 {
        if !self.victory_portfolio
            || self.portfolio_specializing() != Some(true)
            || self.plan.as_ref().is_some_and(|plan| {
                matches!(
                    plan.strategy,
                    GrandStrategy::Recovery | GrandStrategy::Conquest
                )
            })
        {
            return 0.0;
        }
        let goal = |target| match target {
            VictoryTarget::Science => Self::science_drive_milestone(g, pid),
            VictoryTarget::Culture => ["printing", "computers"]
                .into_iter()
                .find(|goal| !g.players[pid].techs.contains(&Name::new(goal))),
            _ => None,
        };
        let primary = self
            .portfolio
            .report
            .primary
            .and_then(goal)
            .is_some_and(|goal| self.tech_leads_to(g, tech, goal));
        let backup = self
            .portfolio
            .report
            .secondary
            .and_then(goal)
            .is_some_and(|goal| self.tech_leads_to(g, tech, goal))
            && g.tech_cost(tech) / empire_yields(g, pid).science.max(1.0)
                <= f64::from(g.standard_duration(8).max(1));
        self.portfolio.report.focus
            * if primary {
                180.0
            } else if backup {
                25.0
            } else {
                0.0
            }
    }

    /// Near-term victory conversion can outweigh score at rollout endpoints.
    /// Low-confidence or distant estimates retain the economic evaluator.
    pub fn victory_finish_value(g: &Game, pid: usize, economic: f64) -> f64 {
        let view = g.player_decision_view(pid);
        let horizon = f64::from(g.standard_duration(80));
        let best = |seat| {
            Self::victory_forecasts(&view, seat)
                .into_iter()
                .filter(|estimate| {
                    estimate.confidence >= 0.6 && estimate.target != VictoryTarget::Score
                })
                .filter_map(|estimate| {
                    estimate
                        .finish_turn
                        .map(|finish| (finish, estimate.confidence))
                })
                .filter(|(finish, _)| *finish <= f64::from(g.turn) + horizon)
                .min_by(|a, b| a.0.total_cmp(&b.0))
        };
        let ours = best(pid);
        let rival = opponents(&view, pid)
            .into_iter()
            .filter_map(best)
            .min_by(|a, b| a.0.total_cmp(&b.0));
        let (race, confidence) = match (ours, rival) {
            (Some((ours, a)), Some((theirs, b))) => {
                ((0.5 + (theirs - ours) / horizon).clamp(0.0, 1.0), a.min(b))
            }
            (Some((_, confidence)), None) => (0.9, confidence),
            (None, Some((_, confidence))) => (0.05, confidence),
            (None, None) => return economic,
        };
        economic + 0.65 * confidence * (race - economic)
    }

    pub(super) fn portfolio_supports(&self, target: VictoryTarget) -> bool {
        self.victory_portfolio
            && self.portfolio_specializing() == Some(true)
            && (self.portfolio.report.primary == Some(target)
                || self.portfolio.report.secondary == Some(target))
    }

    pub(super) fn record_portfolio_trace(&mut self, g: &Game, pid: usize, plan: &StrategicPlan) {
        if !self.victory_planning {
            return;
        }
        if !self.victory_portfolio {
            let focus = self.victory_focus(g, pid).strategy;
            let primary = self.active_victory_target(g).or_else(|| {
                VictoryTarget::ALL
                    .into_iter()
                    .find(|target| target.strategy() == focus)
            });
            let phase = if self.phase_specialization_active(g) {
                DevelopmentPhase::Buildup
            } else {
                DevelopmentPhase::Foundation
            };
            let report = &mut self.portfolio.report;
            if primary != report.primary {
                report.primary_switches += u32::from(report.primary.is_some());
                report.committed_turn = None;
            }
            if primary.is_some()
                && phase != DevelopmentPhase::Foundation
                && report.committed_turn.is_none()
            {
                report.committed_turn = Some(g.turn);
                report.first_commitment_turn.get_or_insert(g.turn);
            }
            report.primary = primary;
            report.assigned_target = self.victory_target;
            report.phase = phase;
            report.reason = "legacy_focus".into();
        }
        let report = &mut self.portfolio.report;
        let changed = report.trace.last().is_none_or(|last| {
            last.phase != report.phase
                || last.primary != report.primary
                || last.secondary != report.secondary
                || last.posture != plan.strategy.as_str()
        });
        if !changed
            && report.trace.last().is_some_and(|last| {
                g.turn.saturating_sub(last.turn) < g.standard_duration(25).max(5)
            })
        {
            return;
        }
        let estimate = report
            .forecasts
            .iter()
            .find(|forecast| Some(forecast.target) == report.primary);
        let yields = empire_yields(g, pid);
        let mut allocation = ProductionAllocation::default();
        for cid in g.player_city_ids(pid) {
            let production = g.city_yields(cid).production.max(0.0);
            let Some(item) = g.cities[&cid].queue.first() else {
                allocation.idle += production;
                continue;
            };
            if report
                .primary
                .is_some_and(|target| Self::item_lane_affinity(g, item, target) > 0.0)
            {
                allocation.primary += production;
            } else if report
                .secondary
                .is_some_and(|target| Self::item_lane_affinity(g, item, target) > 0.0)
            {
                allocation.secondary += production;
            } else {
                allocation.other += production;
                if matches!(item, Item::Unit { unit } if unit == "settler" || unit == "builder") {
                    allocation.settlers_and_builders += production;
                }
            }
        }
        let point = PortfolioTrace {
            turn: g.turn,
            posture: plan.strategy.as_str().into(),
            phase: report.phase,
            primary: report.primary,
            secondary: report.secondary,
            reason: report.reason.clone(),
            expected_finish: estimate.and_then(|forecast| forecast.finish_turn),
            rival_finish: report.rival_finish,
            bottleneck: estimate.map(|forecast| forecast.bottleneck.clone()),
            cities: g.player_city_ids(pid).len(),
            science: yields.science,
            culture: yields.culture,
            military: g.military_power(pid),
            gold: g.players[pid].gold,
            faith: g.players[pid].faith,
            science_projects: g.players[pid].science_projects.len(),
            visitors: g.foreign_tourists(pid),
            diplomatic_points: g.players[pid].dvp,
            production_allocation: Some(allocation),
            capitals: g
                .cities
                .values()
                .filter(|city| city.is_capital && city.owner == pid)
                .count(),
        };
        if report.trace.last().is_some_and(|last| last.turn == g.turn) {
            report.trace.pop();
        }
        report.trace.push(point);
        if report.trace.len() > TRACE_LIMIT {
            report.trace.remove(1);
            report.dropped_trace_points += 1;
        }
    }

    fn item_lane_affinity(g: &Game, item: &Item, target: VictoryTarget) -> f64 {
        let family = match item {
            Item::District { district, .. } => Some(g.district_family(*district)),
            Item::Building { building } => g.rules.buildings[building]
                .district
                .map(|district| g.district_family(district)),
            Item::Project { project } => g.rules.projects[project]
                .district
                .map(|district| g.district_family(district)),
            _ => None,
        };
        match (target, family.as_deref()) {
            (VictoryTarget::Science, Some("campus" | "spaceport" | "industrial_zone"))
            | (VictoryTarget::Culture, Some("theater_square"))
            | (VictoryTarget::Religion, Some("holy_site"))
            | (VictoryTarget::Diplomacy, Some("diplomatic_quarter"))
            | (VictoryTarget::Domination, Some("encampment" | "aerodrome")) => 1.0,
            (VictoryTarget::Culture, Some("campus" | "holy_site"))
            | (VictoryTarget::Domination, Some("campus" | "industrial_zone")) => 0.4,
            _ => 0.0,
        }
    }

    pub(super) fn portfolio_production_adjustment(
        &self,
        g: &Game,
        pid: usize,
        item: &Item,
        plan: &StrategicPlan,
        quote: super::victory_conversion::ProductionQuote,
    ) -> f64 {
        let super::victory_conversion::ProductionQuote { raw, turns } = quote;
        if !self.victory_portfolio
            || raw <= 0.0
            || matches!(
                plan.strategy,
                GrandStrategy::Recovery | GrandStrategy::Conquest
            )
            || plan.threatened_city.is_some()
        {
            return 0.0;
        }
        let report = &self.portfolio.report;
        let Some(primary) = report.primary else {
            return 0.0;
        };
        // An extra city must repay its launch, walk and development before the
        // finish. This is a bounded discount, never a production refusal.
        if matches!(item, Item::Unit { unit } if unit == "settler" || unit == "builder") {
            let left = report.rival_finish.unwrap_or(development_clock(g)) - f64::from(g.turn);
            return if report.phase == DevelopmentPhase::Finish
                && turns + f64::from(g.standard_duration(40)) > left
            {
                -raw * 0.4
            } else {
                0.0
            };
        }
        // Military safety and finite opening opportunities retain their bids.
        if matches!(item, Item::Unit { .. } | Item::Formation { .. }) {
            return 0.0;
        }
        if let Item::Building { building } = item {
            let spec = &g.rules.buildings[building];
            if spec.yields.gold > 0.0
                && g.players[pid].gold_per_turn < 2.0 * g.player_city_ids(pid).len() as f64
            {
                return 0.0;
            }
            if spec.housing > 0.0 || spec.amenity > 0.0 || building.contains("walls") {
                return 0.0;
            }
        }
        let affinity = Self::item_lane_affinity(g, item, primary);
        // A backup finishes installed infrastructure; it cannot reserve a new
        // district or launch a second empire-wide construction program.
        let backup = if matches!(item, Item::Building { .. }) {
            report
                .secondary
                .map_or(0.0, |target| Self::item_lane_affinity(g, item, target))
                * 0.15
        } else {
            0.0
        };
        let left = report.rival_finish.unwrap_or(development_clock(g)) - f64::from(g.turn);
        let useful = turns + f64::from(g.standard_duration(15)) < left;
        let terminal = matches!(item, Item::Project { project } if g.rules.projects[project].district.as_deref() == Some("spaceport"));
        let premium = if useful || terminal {
            affinity * 0.55 + backup
        } else {
            0.0
        };
        let late_detour = if report.phase == DevelopmentPhase::Finish
            && affinity == 0.0
            && backup == 0.0
            && !useful
        {
            0.4
        } else {
            0.0
        };
        raw * report.focus * (premium - late_detour)
    }
}

#[cfg(test)]
mod tests;
