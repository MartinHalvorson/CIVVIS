//! An active Science project's completion, including the cost of changing posts.
//!
//! Independent opt-in. A district under construction earns no Space Initiative
//! production. Native project quotes calibrate remaining work; only relative
//! model production changes price the move, including Magnus's lost effects.
//! Both governors lose their effects during establishment. The proposed finish
//! must beat the native quote by a full turn, while losing at most 5% of the
//! empire's science + half-culture dividend over that interval. This budget is
//! an experimental policy, not evidence of a stronger strategy.

use super::*;
use crate::ai::VictoryTarget;

struct LaunchMove {
    city: u32,
    replacement: Option<Name>,
    turns_saved: f64,
    finish: f64,
}

fn project_rate(g: &Game, pid: usize, cid: u32, item: &Item) -> f64 {
    g.city_yields(cid).production * g.item_prod_mult(pid, cid, Some(item))
}

impl AdvancedAi {
    fn earned_launch_move(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> Option<LaunchMove> {
        if !self.pingala_earns_the_launch
            || self.base.minor
            || self.base.barb
            || self.active_victory_target(g) != Some(VictoryTarget::Science)
            || plan.strategy == GrandStrategy::Recovery
        {
            return None;
        }
        let state = g.players[pid].governor_roster.get("pingala")?;
        let old = state.city?;
        let old_city = g.cities.get(&old)?;
        let residence = g.standard_duration(20);
        think!(self.journal(), Government, Detail,
            "Reviewing Pingala's committed launch post";
            "pingala-earns-the-launch review: posture {:?}; age {}/{}; safe old {}; departure {}; old Spaceport {}; disabled {}; promotion {}",
            plan.strategy, self.governor_residence_age(g, pid, "pingala"), residence,
            self.governor_dividend_safe_city(g, pid, old, plan),
            self.base.capital_governor_may_depart(g, pid, "pingala"),
            old_city.districts.keys().any(|d| g.district_family(*d) == "spaceport"),
            state.disabled_until > g.turn, state.promotions.contains("space_initiative"));
        if old_city.owner != pid
            || !state.promotions.contains("space_initiative")
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
            || (state.promotions.contains("curator")
                && g.housed_great_works(pid)
                    .get(&old)
                    .is_some_and(|works| works.values().any(|n| *n > 0)))
        {
            return None;
        }
        let duration = g.rules.governors["pingala"].establish_turns;
        let settle = if self.live_governor_assignment_adapter {
            duration.max(g.standard_duration(duration))
        } else {
            g.standard_duration(duration)
        }
        .max(1) as f64;
        let before = Self::governor_dividend_reading(g, pid, "pingala");
        if !before.is_finite() || before <= 0.0 {
            return None;
        }
        let mut best: Option<LaunchMove> = None;
        for cid in g.player_city_ids(pid) {
            if matches!(g.cities[&cid].queue.first(), Some(Item::Project { project }) if Game::is_space_project(project))
            {
                think!(self.journal(), Government, Detail,
                    "Reviewing {} as Pingala's launch producer", g.cities[&cid].name;
                    "pingala-earns-the-launch producer: local safe {}; old post {}",
                    self.governor_dividend_safe_city(g, pid, cid, plan), cid == old);
            }
            if cid == old || !self.governor_dividend_safe_city(g, pid, cid, plan) {
                continue;
            }
            let Some(item @ Item::Project { project }) = g.cities[&cid].queue.first() else {
                continue;
            };
            if !Game::is_space_project(project) {
                continue;
            }
            let turns = g
                .host_production_turns(cid, item)
                .unwrap_or_else(|| self.production_build_turns(g, pid, cid, item));
            if !turns.is_finite() || turns <= settle + 1.0 {
                continue;
            }
            let replacement = g.players[pid]
                .governor_roster
                .iter()
                .find(|(_, other)| other.city == Some(cid))
                .map(|(name, _)| Name::new(name));
            if let Some(name) = replacement {
                let other = &g.players[pid].governor_roster[name.as_str()];
                // Defensive specialists and builders retain their posts.
                // Magnus is priced, including Industrialist/Vertical Integration.
                if name != "magnus"
                    || other.disabled_until > g.turn
                    || self.governor_residence_age(g, pid, name.as_str()) < residence
                    || !self.base.capital_governor_may_depart(g, pid, name.as_str())
                    || self
                        .remembered_governor_posts
                        .get(name.as_str())
                        .is_some_and(|post| post.last_request == Some(g.turn))
                {
                    continue;
                }
            }
            let baseline = project_rate(g, pid, cid, item);
            if !baseline.is_finite() || baseline <= 0.0 {
                continue;
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
            if let Some(governor) = replacement {
                if probe
                    .apply(
                        pid,
                        &Action::ReassignGovernor {
                            governor,
                            city: old,
                        },
                    )
                    .is_err()
                {
                    continue;
                }
            }
            let transition_rate = project_rate(&probe, pid, cid, item);
            let transition_dividend = Self::governor_dividend_reading(&probe, pid, "pingala");
            for name in [Some(Name::new("pingala")), replacement]
                .into_iter()
                .flatten()
            {
                probe.players[pid]
                    .governor_roster
                    .get_mut(name.as_str())
                    .unwrap()
                    .assigned_turn = g.turn.saturating_sub(
                    g.standard_duration(g.rules.governors[name.as_str()].establish_turns),
                );
            }
            let after_rate = project_rate(&probe, pid, cid, item);
            let after_dividend = Self::governor_dividend_reading(&probe, pid, "pingala");
            think!(self.journal(), Government, Detail,
                "Pricing Pingala's launch in {}", g.cities[&cid].name;
                "pingala-earns-the-launch forecast: quote {turns:.2}, establish {settle:.2}; production rates {baseline:.2}/{transition_rate:.2}/{after_rate:.2}; weighted dividends {before:.2}/{transition_dividend:.2}/{after_dividend:.2}");
            if [
                transition_rate,
                transition_dividend,
                after_rate,
                after_dividend,
            ]
            .iter()
            .any(|n| !n.is_finite())
                || transition_rate < 0.0
                || after_rate <= 0.0
            {
                continue;
            }
            let remaining = turns * baseline - settle * transition_rate;
            if remaining <= 0.0 {
                continue; // The current job ends before the new bonus arrives.
            }
            let finish = (settle + remaining / after_rate).ceil();
            let saved = turns.ceil() - finish;
            if saved < 1.0
                || g.turn_limit()
                    .is_some_and(|limit| g.turn as f64 + finish > limit as f64)
            {
                continue;
            }
            let dividend_loss = before * finish
                - (transition_dividend * settle + after_dividend * (finish - settle));
            if dividend_loss > before * finish * 0.05 {
                continue;
            }
            if best.as_ref().is_none_or(|old| {
                saved > old.turns_saved
                    || (saved == old.turns_saved && (finish, cid) < (old.finish, old.city))
            }) {
                best = Some(LaunchMove {
                    city: cid,
                    replacement,
                    turns_saved: saved,
                    finish,
                });
            }
        }
        best
    }

    /// Before title spending, so the existing native exchange continuation can
    /// retain the return leg. No assignment is emitted until both legs validate.
    pub(in crate::ai::advanced) fn relocate_pingala_for_launch(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.pingala_earns_the_launch {
            return false;
        }
        if self.live_governor_assignment_adapter {
            self.observe_governor_posts(g, pid);
        }
        let Some(proposal) = self.earned_launch_move(g, pid, plan) else {
            return false;
        };
        let old = g.players[pid].governor_roster["pingala"].city.unwrap();
        for (governor, city) in [
            (Some(Name::new("pingala")), proposal.city),
            (proposal.replacement, old),
        ] {
            let Some(governor) = governor else {
                continue;
            };
            if g.apply(pid, &Action::ReassignGovernor { governor, city })
                .is_err()
            {
                return true; // Retain an already-applied first leg for readback.
            }
            if let Some(post) = self.remembered_governor_posts.get_mut(governor.as_str()) {
                post.last_request = Some(g.turn);
            }
        }
        think!(self.journal(), Government, Decision,
            "Pingala earns the launch post in {}", g.cities[&proposal.city].name;
            "pingala-earns-the-launch: {:.0} quoted project turns saved after establishment and displaced governor effects; projected finish {:.0}",
            proposal.turns_saved, proposal.finish);
        true
    }
}

#[cfg(test)]
#[path = "pingala_launch_tests.rs"]
mod tests;
