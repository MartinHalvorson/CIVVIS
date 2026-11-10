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
        self.remembered_governor_posts
            .retain(|name, _| roster.contains_key(name.as_str()));
        for (name, state) in roster {
            let key = name.to_string();
            let city = state.city.and_then(|cid| g.cities.get(&cid));
            let Some(city) = city.filter(|_| state.disabled_until <= g.turn) else {
                self.remembered_governor_posts.remove(&key);
                continue;
            };
            let Some(spec) = g.rules.governors.get(name.as_str()) else {
                self.remembered_governor_posts.remove(&key);
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
                .remembered_governor_posts
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

    pub(super) fn governor_residence_age(&self, g: &Game, pid: usize, governor: &str) -> u32 {
        let Some(state) = g.players[pid].governor_roster.get(governor) else {
            return 0;
        };
        let age = g.turn.saturating_sub(state.assigned_turn);
        if !self.live_governor_assignment_adapter {
            return age;
        }
        let post = self.remembered_governor_posts.get(governor).filter(|post| {
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

    /// Whether a relocation of `governor` was already requested this turn: an
    /// asynchronous host assignment can still show the old post on another
    /// frame of the same turn.
    pub(super) fn governor_requested_this_turn(&self, g: &Game, governor: &str) -> bool {
        self.live_governor_assignment_adapter
            && self
                .remembered_governor_posts
                .get(governor)
                .is_some_and(|post| post.last_request == Some(g.turn))
    }

    /// Record a relocation request of `governor` this turn; see
    /// [`Self::governor_requested_this_turn`].
    pub(super) fn note_governor_request(&mut self, g: &Game, governor: &str) {
        if let Some(post) = self.remembered_governor_posts.get_mut(governor) {
            post.last_request = Some(g.turn);
        }
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
        self.pingala_follows_research_2 = false;
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

    pub(super) fn governor_dividend_safe_city(
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
                    .remembered_governor_posts
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

    /// Space Initiative is not priced by the ordinary yield-only relocation.
    /// Move toward a committed first launch job, rather than freezing Pingala
    /// in a city with no Spaceport simply because he has that promotion.
    /// An ordinary Magnus may trade posts; specialist/defensive governors stay.
    fn pingala_launch_posts(
        &self,
        g: &Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Option<Name>)> {
        if !self.pingala_follows_research_2 || plan.strategy != GrandStrategy::Science {
            return None;
        }
        let state = g.players[pid].governor_roster.get("pingala")?;
        let old = state.city?;
        let old_city = g.cities.get(&old)?;
        let residence = g.standard_duration(20);
        if !state.promotions.contains("space_initiative")
            || state.disabled_until > g.turn
            || self.governor_residence_age(g, pid, "pingala") < residence
            || self
                .remembered_governor_posts
                .get("pingala")
                .is_some_and(|post| post.last_request == Some(g.turn))
            || !self.governor_dividend_safe_city(g, pid, old, plan)
            || !self.base.capital_governor_may_depart(g, pid, "pingala")
            || old_city
                .districts
                .keys()
                .any(|d| g.district_family(*d) == "spaceport")
            || matches!(old_city.queue.first(), Some(Item::Project { project }) if Game::is_space_project(project))
            || (state.promotions.contains("curator")
                && g.housed_great_works(pid)
                    .get(&old)
                    .is_some_and(|works| works.values().any(|count| *count > 0)))
        {
            return None;
        }
        // Native Online readback establishes ordinary governors in five turns.
        // Keep the duration-table bound too (Online is also five there).
        // Never budget less than the observed native interval.
        let duration = g.rules.governors["pingala"].establish_turns;
        let settle = if self.live_governor_assignment_adapter {
            duration.max(g.standard_duration(duration))
        } else {
            g.standard_duration(duration)
        }
        .max(1);
        let horizon = g
            .turn_limit()
            .map(|limit| limit.saturating_sub(g.turn))
            .unwrap_or(g.standard_duration(40))
            .min(g.standard_duration(40));
        if horizon <= residence + settle {
            return None;
        }
        let before = Self::governor_dividend_reading(g, pid, "pingala");
        let mut best: Option<(f64, u32, Option<Name>)> = None;
        for cid in g.player_city_ids(pid) {
            if cid == old || !self.governor_dividend_safe_city(g, pid, cid, plan) {
                continue;
            }
            let city = &g.cities[&cid];
            let Some(item) = city.queue.first() else {
                continue;
            };
            let pad = matches!(item, Item::District { district, .. } if g.district_family(*district) == "spaceport");
            let launch =
                matches!(item, Item::Project { project } if Game::is_space_project(project));
            if !pad && !launch {
                continue;
            }
            let turns = g
                .host_production_turns(cid, item)
                .filter(|n| n.is_finite() && *n >= 0.0)
                .unwrap_or_else(|| self.production_build_turns(g, pid, cid, item))
                .ceil();
            if !turns.is_finite()
                || turns <= 0.0
                || (pad && turns + settle as f64 >= horizon as f64)
                || (launch && turns <= (2 * settle) as f64)
            {
                continue;
            }
            let replacement = g.players[pid]
                .governor_roster
                .iter()
                .find(|(_, other)| other.city == Some(cid))
                .map(|(name, _)| Name::new(name));
            if let Some(name) = replacement {
                let other = &g.players[pid].governor_roster[name.as_str()];
                if name != "magnus"
                    || other.disabled_until > g.turn
                    || self.governor_residence_age(g, pid, "magnus") < residence
                    || !self.base.capital_governor_may_depart(g, pid, "magnus")
                    || other.promotions.contains("industrialist")
                    || other.promotions.contains("vertical_integration")
                    || self
                        .remembered_governor_posts
                        .get("magnus")
                        .is_some_and(|post| post.last_request == Some(g.turn))
                {
                    continue;
                }
            }
            let mut probe = g.clone();
            if probe
                .apply(
                    pid,
                    &Action::ReassignGovernor {
                        governor: Name::new("pingala"),
                        city: cid,
                    },
                )
                .is_err()
            {
                continue;
            }
            if let Some(name) = replacement {
                if probe
                    .apply(
                        pid,
                        &Action::ReassignGovernor {
                            governor: name,
                            city: old,
                        },
                    )
                    .is_err()
                {
                    continue;
                }
            }
            for governor in [Some(Name::new("pingala")), replacement]
                .into_iter()
                .flatten()
            {
                probe.players[pid]
                    .governor_roster
                    .get_mut(governor.as_str())
                    .unwrap()
                    .assigned_turn = g.turn.saturating_sub(
                    g.standard_duration(g.rules.governors[governor.as_str()].establish_turns),
                );
            }
            let after = Self::governor_dividend_reading(&probe, pid, "pingala");
            // The launch benefit is additional: retain at least the established
            // science/culture dividend, rather than buying production with it.
            if !after.is_finite() || after + f64::EPSILON < before {
                continue;
            }
            let score = g.city_yields(cid).production / (turns + settle as f64);
            if best
                .as_ref()
                .is_none_or(|(value, old, _)| score > *value || (score == *value && cid < *old))
            {
                best = Some((score, cid, replacement));
            }
        }
        best.map(|(_, cid, replacement)| (cid, replacement))
    }

    pub(super) fn relocate_governors_for_dividends(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        let enabled = self.pingala_earns_the_launch
            || self.pingala_follows_research
            || self.pingala_follows_research_2
            || self.magnus_follows_settlers
            || self.liang_follows_builders
            || self.reyna_follows_revenue
            || self.amani_follows_suzerainty;
        // `curator-sits-with-the-works` reads the same post clock.
        let observed = enabled || self.curator_sits_with_the_works;
        if observed && self.live_governor_assignment_adapter {
            self.observe_governor_posts(g, pid);
        } else {
            self.remembered_governor_posts.clear();
        }
        // `curator-sits-with-the-works`: every turn, at war or at peace, before
        // the reviews below. See `advanced/curator_sits_with_the_works.rs`.
        // Exact no-op while the gene is off.
        if self.curator_follows_the_works(g, pid, plan) {
            return;
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
        if let Some((city, replacement)) = self.pingala_launch_posts(g, pid, plan) {
            let old = g.players[pid].governor_roster["pingala"].city.unwrap();
            let mut actions = vec![Action::ReassignGovernor {
                governor: Name::new("pingala"),
                city,
            }];
            if let Some(governor) = replacement {
                actions.push(Action::ReassignGovernor {
                    governor,
                    city: old,
                });
            }
            // The same ordered pair was checked on the analysis world above.
            for action in actions {
                let Action::ReassignGovernor { governor, .. } = &action else {
                    unreachable!()
                };
                if g.apply(pid, &action).is_err() {
                    return;
                }
                if let Some(post) = self.remembered_governor_posts.get_mut(governor.as_str()) {
                    post.last_request = Some(g.turn);
                }
            }
            think!(self.journal(), Government, Decision,
                "Moving Pingala to {} for its launch job", g.cities[&city].name;
                "Space Initiative follows a committed producer; any ordinary Magnus keeps a post");
            return;
        }
        for (governor, enabled) in [
            (
                "pingala",
                self.pingala_follows_research || self.pingala_follows_research_2,
            ),
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
                    if let Some(post) = self.remembered_governor_posts.get_mut(governor) {
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

#[path = "governor_dividends/pingala_launch.rs"]
mod pingala_launch;

#[cfg(test)]
mod tests;
