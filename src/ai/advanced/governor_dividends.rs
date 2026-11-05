//! Revisit governor posts when the original city's job has moved elsewhere.
//! Five independent opt-ins; existing appointments and title spending are intact.
use super::{AdvancedAi, GrandStrategy, StrategicPlan};
use crate::game::{Action, Game, Item};
use crate::name::Name;
use crate::think;

#[derive(Clone)]
pub(super) struct ObservedGovernorPost {
    city: crate::Pos,
    owner: usize,
    assignment_bound: u32,
    last_seen: u32,
    last_request: Option<u32>,
}

impl AdvancedAi {
    /// The live mirror reports an established governor at only the minimum
    /// establishment age on every fresh board. Preserve the earliest observed
    /// assignment bound while its physical post stays unchanged. This is an
    /// observation clock for our relocation cooldown, not a host tenure claim
    /// or a change to the game's establishment timer.
    fn observe_governor_posts(&mut self, g: &Game, pid: usize) {
        let roster = &g.players[pid].governor_roster;
        self.observed_governor_posts
            .retain(|name, _| roster.contains_key(name.as_str()));
        for (name, state) in roster {
            let key = name.clone();
            let city = state.city.and_then(|cid| g.cities.get(&cid));
            let Some(city) = city.filter(|_| state.disabled_until <= g.turn) else {
                self.observed_governor_posts.remove(&key);
                continue;
            };
            let Some(spec) = g.rules.governors.get(name.as_str()) else {
                self.observed_governor_posts.remove(&key);
                continue;
            };
            let establish = g.standard_duration(spec.establish_turns);
            let fresh = || ObservedGovernorPost {
                city: city.pos,
                owner: city.owner,
                assignment_bound: state.assigned_turn.min(g.turn),
                last_seen: g.turn,
                last_request: None,
            };
            let post = self
                .observed_governor_posts
                .entry(key)
                .or_insert_with(fresh);
            if post.city != city.pos
                || post.owner != city.owner
                || g.turn < post.last_seen
                || g.turn.saturating_sub(state.assigned_turn) < establish
            {
                *post = fresh();
            } else {
                post.assignment_bound = post.assignment_bound.min(state.assigned_turn);
                post.last_seen = g.turn;
            }
        }
    }

    fn governor_residence_age(&self, g: &Game, pid: usize, governor: &str) -> u32 {
        let Some(state) = g.players[pid].governor_roster.get(governor) else {
            return 0;
        };
        let age = g.turn.saturating_sub(state.assigned_turn);
        if !self.live_governor_assignment_adapter {
            return age;
        }
        let post = self.observed_governor_posts.get(governor).filter(|post| {
            state.disabled_until <= g.turn
                && g.turn >= post.last_seen
                && state
                    .city
                    .and_then(|cid| g.cities.get(&cid))
                    .is_some_and(|city| city.pos == post.city && city.owner == post.owner)
        });
        post.map_or(age, |post| {
            age.max(g.turn.saturating_sub(post.assignment_bound))
        })
    }

    /// Enable this independently screenable governor relocation.
    pub fn enable_amani_follows_suzerainty(&mut self) {
        self.amani_follows_suzerainty = true;
    }
    /// Withhold this governor relocation.
    pub fn disable_amani_follows_suzerainty(&mut self) {
        self.amani_follows_suzerainty = false;
    }
    /// Enable this independently screenable governor relocation.
    pub fn enable_liang_follows_builders(&mut self) {
        self.liang_follows_builders = true;
    }
    /// Withhold this governor relocation.
    pub fn disable_liang_follows_builders(&mut self) {
        self.liang_follows_builders = false;
    }
    /// Enable this independently screenable governor relocation.
    pub fn enable_magnus_follows_settlers(&mut self) {
        self.magnus_follows_settlers = true;
    }
    /// Withhold this governor relocation.
    pub fn disable_magnus_follows_settlers(&mut self) {
        self.magnus_follows_settlers = false;
    }
    /// Enable this independently screenable governor relocation.
    pub fn enable_pingala_follows_research(&mut self) {
        self.pingala_follows_research = true;
    }
    /// Withhold this governor relocation.
    pub fn disable_pingala_follows_research(&mut self) {
        self.pingala_follows_research = false;
    }
    /// Enable this independently screenable governor relocation.
    pub fn enable_reyna_follows_revenue(&mut self) {
        self.reyna_follows_revenue = true;
    }
    /// Withhold this governor relocation.
    pub fn disable_reyna_follows_revenue(&mut self) {
        self.reyna_follows_revenue = false;
    }

    fn governor_dividend_safe_city(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        plan: &StrategicPlan,
    ) -> bool {
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        plan.threatened_city != Some(cid)
            && !(city.last_attacked > 0 && g.turn.saturating_sub(city.last_attacked) <= 4)
            && (city.owner != pid
                || (city.loyalty >= 90.0
                    && self.base.loyalty_emergency(g, cid).is_none()
                    && !self.base.barbarian_local_alarm_for_controller(g, pid, cid)))
    }

    /// Clone only the analysis world. No trial orders reach the actual game.
    fn governor_dividend_world(g: &Game, pid: usize, governor: &str, city: Option<u32>) -> Game {
        let mut probe = g.clone();
        let duration = g.standard_duration(g.rules.governors[governor].establish_turns);
        let state = probe.players[pid]
            .governor_roster
            .get_mut(governor)
            .unwrap();
        state.city = city;
        state.assigned_turn = g.turn.saturating_sub(duration);
        probe.players[pid].governors = probe.players[pid]
            .governor_roster
            .values()
            .filter_map(|state| state.city)
            .filter(|cid| probe.cities.get(cid).is_some_and(|city| city.owner == pid))
            .collect();
        probe
    }

    fn governor_dividend_reading(g: &Game, pid: usize, governor: &str) -> f64 {
        let _memo = g.query_memo();
        g.player_city_ids(pid)
            .into_iter()
            .map(|cid| {
                let y = g.city_yields(cid);
                if governor == "pingala" {
                    y.science + 0.5 * y.culture
                } else {
                    y.gold
                }
            })
            .sum()
    }

    fn governor_dividend_destination(
        &self,
        g: &Game,
        pid: usize,
        governor: &str,
        plan: &StrategicPlan,
    ) -> Option<u32> {
        let state = g.players[pid].governor_roster.get(governor)?;
        let current = state.city?;
        let settle = g
            .standard_duration(g.rules.governors[governor].establish_turns)
            .max(1);
        let residence = g.standard_duration(20).max(settle);
        if state.disabled_until > g.turn
            || self.governor_residence_age(g, pid, governor) < residence
            || (self.live_governor_assignment_adapter
                && self
                    .observed_governor_posts
                    .get(governor)
                    .is_some_and(|post| post.last_request == Some(g.turn)))
            || !self.governor_dividend_safe_city(g, pid, current, plan)
            || !self.base.capital_governor_may_depart(g, pid, governor)
        {
            return None;
        }
        let horizon = g
            .turn_limit()
            .map(|limit| limit.saturating_sub(g.turn))
            .unwrap_or(g.standard_duration(40))
            .min(g.standard_duration(40));
        if horizon <= residence + settle {
            return None;
        }
        let old_city = &g.cities[&current];
        // These advanced promotions carry benefits outside the reading below.
        if (governor == "pingala"
            && (state.promotions.contains("space_initiative")
                || state.promotions.contains("curator")
                || old_city
                    .districts
                    .keys()
                    .any(|d| g.district_family(*d) == "spaceport")))
            || (governor == "magnus"
                && (!state.promotions.contains("provision")
                    || state.promotions.contains("vertical_integration")
                    || state.promotions.contains("industrialist")))
            || (governor == "liang"
                && (state.promotions.contains("reinforced_materials")
                    || state.promotions.contains("water_works")))
        {
            return None;
        }
        let job = match governor {
            "magnus" => Some("settler"),
            "liang" => Some("builder"),
            _ => None,
        };
        if let Some(job) = job {
            // Do not abandon an existing unit or district-production job.
            if old_city.queue.first().is_some_and(|item| {
                matches!(item,
                Item::Unit { unit } if unit == job)
                    || matches!(item, Item::District { .. })
            }) {
                return None;
            }
        }
        let economic = matches!(governor, "pingala" | "reyna");
        let baseline = economic.then(|| {
            Self::governor_dividend_reading(
                &Self::governor_dividend_world(g, pid, governor, None),
                pid,
                governor,
            )
        });
        let old_gain = baseline
            .map(|base| (Self::governor_dividend_reading(g, pid, governor) - base).max(0.0));
        let mut best: Option<(f64, u32)> = None;
        for cid in g.cities.keys().copied() {
            let city = &g.cities[&cid];
            let owner = &g.players[city.owner];
            let eligible = city.owner == pid
                || (governor == "amani"
                    && owner.alive
                    && owner.is_minor
                    && !owner.is_barbarian
                    && g.has_met(pid, owner.id)
                    && !g.is_at_war(pid, owner.id));
            if !eligible
                || cid == current
                || g.players[pid]
                    .governor_roster
                    .values()
                    .any(|s| s.city == Some(cid))
                || !self.governor_dividend_safe_city(g, pid, cid, plan)
            {
                continue;
            }
            let score = if let Some(job) = job {
                if city.owner != pid {
                    continue;
                }
                let Some(item @ Item::Unit { unit }) = city.queue.first() else {
                    continue;
                };
                if unit != job {
                    continue;
                }
                let turns = g
                    .host_production_turns(cid, item)
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .unwrap_or_else(|| self.production_build_turns(g, pid, cid, item))
                    .ceil();
                // The bonus must be established before the committed unit finishes.
                if turns <= settle as f64 || turns > horizon as f64 {
                    continue;
                }
                1.0 / turns
            } else if economic {
                let probe = Self::governor_dividend_world(g, pid, governor, Some(cid));
                let gain =
                    Self::governor_dividend_reading(&probe, pid, governor) - baseline.unwrap();
                let old = old_gain.unwrap();
                if gain <= old * 1.25 + 1.0 {
                    continue;
                }
                let net = gain * (horizon - settle) as f64 - old * horizon as f64;
                if net <= 5.0 {
                    continue;
                }
                net
            } else {
                if !owner.is_minor || g.suzerain_of(owner.id) == Some(pid) {
                    continue;
                }
                let probe = Self::governor_dividend_world(g, pid, governor, Some(cid));
                if probe.suzerain_of(owner.id) != Some(pid)
                    || (g.players[old_city.owner].is_minor
                        && g.suzerain_of(old_city.owner) == Some(pid)
                        && probe.suzerain_of(old_city.owner) != Some(pid))
                {
                    continue;
                }
                let rival = g
                    .players
                    .iter()
                    .filter(|p| p.id != pid && p.alive && !p.is_minor && !p.is_barbarian)
                    .map(|p| probe.envoys_at(p.id, owner.id))
                    .max()
                    .unwrap_or(0);
                (probe.envoys_at(pid, owner.id) - rival) as f64
            };
            if best.is_none_or(|(value, old)| score > value || (score == value && cid < old)) {
                best = Some((score, cid));
            }
        }
        best.map(|(_, cid)| cid)
    }

    pub(super) fn relocate_governors_for_dividends(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let enabled = self.pingala_follows_research
            || self.magnus_follows_settlers
            || self.liang_follows_builders
            || self.reyna_follows_revenue
            || self.amani_follows_suzerainty;
        if enabled && self.live_governor_assignment_adapter {
            self.observe_governor_posts(g, pid);
        } else {
            self.observed_governor_posts.clear();
        }
        if !enabled
            || self.base.minor
            || self.base.barb
            || !g.turn.is_multiple_of(g.standard_duration(16).max(1))
            || matches!(
                plan.strategy,
                GrandStrategy::Recovery | GrandStrategy::Conquest
            )
            || self.war_plan.is_some()
            || g.players.iter().any(|p| {
                p.id != pid && p.alive && !p.is_minor && !p.is_barbarian && g.is_at_war(pid, p.id)
            })
        {
            return;
        }
        for (governor, enabled) in [
            ("pingala", self.pingala_follows_research),
            ("magnus", self.magnus_follows_settlers),
            ("liang", self.liang_follows_builders),
            ("reyna", self.reyna_follows_revenue),
            ("amani", self.amani_follows_suzerainty),
        ] {
            if !enabled {
                continue;
            }
            if let Some(city) = self.governor_dividend_destination(g, pid, governor, plan) {
                if g.apply(
                    pid,
                    &Action::ReassignGovernor {
                        governor: Name::new(governor),
                        city,
                    },
                )
                .is_ok()
                {
                    if let Some(post) = self.observed_governor_posts.get_mut(governor) {
                        // An asynchronous host assignment can still show
                        // the old post on another frame of this turn.
                        post.last_request = Some(g.turn);
                    }
                    think!(self.journal(), Government, Decision,
                        "Moving {} to {}", governor, g.cities[&city].name;
                        "the governor-dividend gene found a safe, timely replacement job");
                    break; // One relocation per review; no empire-wide disruption.
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
