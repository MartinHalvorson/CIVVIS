//! A default-off physical air interception beside a slower elimination war.
//! Staging is a forecast; only two fresh, selected sorties authorize a war.
use super::*;

impl AdvancedAi {
    fn culture_air_policy_allowed(&self, g: &Game, pid: usize, plan: &StrategicPlan) -> bool {
        self.elimination_yields_to_a_launch_2
            && self.active_victory_target(g) == Some(VictoryTarget::Domination)
            && self.forced_target_player.is_none()
            && plan.strategy != GrandStrategy::Recovery
            && plan.threatened_city.is_none()
            && self.threatened_city(g, pid).is_none()
    }

    fn culture_air_clock(&self, g: &Game, pid: usize, target: usize) -> Option<f64> {
        let rival = g.players.get(target)?;
        if target == pid
            || !rival.alive
            || rival.is_minor
            || rival.is_barbarian
            || g.foreign_tourists(target) < denial_nearest_finish::ENGINE_CLOCK_MIN_TOURISTS
        {
            return None;
        }
        let clock = g.culture_turns_to_victory(target)?;
        (clock.is_finite() && (0.0..=f64::from(g.standard_duration(40))).contains(&clock))
            .then_some(clock)
    }

    fn culture_air_opening_allowed(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        plan: &StrategicPlan,
    ) -> bool {
        self.culture_air_policy_allowed(g, pid, plan)
            && g.turn >= 35
            && g.turn >= self.peace_until
            && g.player_city_ids(pid).len() >= 2
            && self.culture_air_clock(g, pid, target).is_some()
            && self.campaign_target_legal(g, pid, target)
            && self.war_is_affordable(g, pid)
            && self.treasury_can_carry_a_war(g, pid)
    }

    fn culture_air_ready(g: &Game, pid: usize, uid: u32) -> bool {
        g.units.get(&uid).is_some_and(|unit| {
            unit.owner == pid
                && g.air_capacity_at(pid, unit.pos) > 0
                && unit.hp >= 80
                && unit.moves_left > 0.0
                && unit.attacks_left > 0
                && g.rules.units[unit.kind].siege
                && g.rules.units[unit.kind].domain.as_deref() == Some("air")
        })
    }

    fn culture_air_visible(g: &Game, pid: usize, target: Pos) -> bool {
        if pid == crate::game::MIRRORED_SEAT && !g.host_observed.is_empty() {
            g.host_observed.contains(&target)
        } else {
            g.player_can_see(pid, target)
        }
    }

    /// This exact selector also runs before siege-wall and rebase priorities.
    /// `observations` keeps a staging forecast from gaining invented sight.
    fn culture_air_sortie(
        &self,
        g: &Game,
        observations: &Game,
        pid: usize,
        uid: u32,
        target: usize,
    ) -> Option<Action> {
        if !Self::culture_air_ready(g, pid, uid)
            || !g.is_at_war(pid, target)
            || !self.air_base_rebase_allowed(g, pid, uid, g.units[&uid].pos)
        {
            return None;
        }
        g.legal_doctrine_actions(pid, uid)
            .into_iter()
            .filter_map(|action| {
                let Action::AirPillage { target: pos, .. } = action else {
                    return None;
                };
                let tile = g.map.get(pos)?;
                let city = tile.owner_city?;
                if tile.pillaged
                    || g.cities.get(&city)?.owner != target
                    || tile
                        .district
                        .is_none_or(|kind| g.district_family(kind) != "theater_square")
                    || !Self::culture_air_visible(observations, pid, pos)
                {
                    return None;
                }
                let value = self.air_pillage_value(g, pid, uid, pos);
                if value <= 0.0 {
                    return None;
                }
                let before = g.cities[&city].pillaged_buildings.len();
                let mut after = g.speculative_clone();
                if after.apply(pid, &action).is_err()
                    || after.units.get(&uid).is_none_or(|plane| plane.hp < 60)
                    || !(after.map.get(pos).is_some_and(|tile| tile.pillaged)
                        || after.cities[&city].pillaged_buildings.len() > before)
                {
                    return None;
                }
                Some((value, pos, action))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            .map(|(_, _, action)| action)
    }

    fn culture_air_opening(&self, g: &Game, pid: usize, target: usize) -> Option<Action> {
        self.preferred_war_opening(g, pid, target).filter(|action| {
            matches!(
                action,
                Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
            )
        })
    }

    /// Two distinct planes, selected and applied sequentially on this board.
    /// Do not substitute a future rebase/readiness promise for either sortie.
    fn culture_air_package(
        &self,
        g: &Game,
        pid: usize,
        target: usize,
        plan: &StrategicPlan,
    ) -> Option<(u32, Option<Action>)> {
        if !self.culture_air_opening_allowed(g, pid, target, plan) {
            return None;
        }
        let opening = if g.is_at_war(pid, target) {
            None
        } else {
            Some(self.culture_air_opening(g, pid, target)?)
        };
        let mut future = g.speculative_clone();
        if let Some(action) = &opening {
            future.apply(pid, action).ok()?;
        }
        let mut counter = plan.clone();
        counter.strategy = GrandStrategy::Conquest;
        counter.target_player = Some(target);
        if !self.culture_air_policy_allowed(&future, pid, &counter) {
            return None;
        }
        let mut first_city = None;
        let mut sorties = 0;
        for uid in g.player_unit_ids(pid) {
            if let Some(action @ Action::AirPillage { target: pos, .. }) =
                self.culture_air_priority_action(&future, pid, uid, &counter)
            {
                let city = future.map.get(pos)?.owner_city?;
                future.apply(pid, &action).ok()?;
                first_city.get_or_insert(city);
                sorties += 1;
                if sorties == 2 {
                    return Some((first_city?, opening));
                }
            }
        }
        None
    }

    pub(super) fn culture_air_priority_action(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
    ) -> Option<Action> {
        if !self.culture_air_policy_allowed(g, pid, plan)
            || plan.strategy != GrandStrategy::Conquest
        {
            return None;
        }
        let target = plan.target_player?;
        self.culture_air_clock(g, pid, target)?;
        self.culture_air_sortie(g, g, pid, uid, target)
    }

    fn culture_air_racers(&self, g: &Game, pid: usize) -> Vec<usize> {
        let mut rivals: Vec<_> = g
            .players
            .iter()
            .filter_map(|p| {
                self.culture_air_clock(g, pid, p.id)
                    .map(|clock| (clock, p.id))
            })
            .collect();
        rivals.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
        rivals.into_iter().map(|(_, pid)| pid).collect()
    }

    /// Run after both persistent campaign overlays, even on an otherwise fresh plan.
    pub(super) fn apply_culture_air_interception_to_strategy(
        &self,
        g: &Game,
        pid: usize,
        plan: &mut StrategicPlan,
    ) {
        if !self.culture_air_policy_allowed(g, pid, plan) {
            return;
        }
        for target in self.culture_air_racers(g, pid) {
            if let Some((city, _)) = self.culture_air_package(g, pid, target, plan) {
                plan.strategy = GrandStrategy::Conquest;
                plan.target_player = Some(target);
                plan.target_city = Some(city);
                plan.rush = false;
                return;
            }
        }
    }

    /// A narrowly preflighted opening; ordinary declarations keep their gates.
    pub(super) fn open_culture_air_interception(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) -> bool {
        if !self.elimination_yields_to_a_launch_2 || plan.strategy != GrandStrategy::Conquest {
            return false;
        }
        let Some(target) = plan
            .target_player
            .filter(|target| !g.is_at_war(pid, *target))
        else {
            return false;
        };
        let Some((_, Some(opening))) = self.culture_air_package(g, pid, target, plan) else {
            return false;
        };
        if g.apply(pid, &opening).is_err() {
            return false;
        }
        *g.players[pid]
            .counters
            .entry("elimination-yields-to-a-launch-2:opened".to_string())
            .or_insert(0) += 1;
        think!(self.journal(), Military, Decision,
            "Opening a Culture air interception on {}", g.players[target].civ;
            "elimination-yields-to-a-launch-2: two distinct ready aircraft select useful Theater sorties on the fresh board, each surviving at least 60 HP; the home, treasury and legal-opening guards pass");
        true
    }

    /// Spend the fresh selected sorties before fire plans or city-assault
    /// groups can assign the same planes to a different mission.
    pub(super) fn execute_culture_air_interception(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.culture_air_policy_allowed(g, pid, plan) {
            return;
        }
        let mut flown = 0;
        for uid in g.player_unit_ids(pid) {
            let Some(action @ Action::AirPillage { target, .. }) =
                self.culture_air_priority_action(g, pid, uid, plan)
            else {
                continue;
            };
            if g.apply(pid, &action).is_ok() {
                *g.players[pid]
                    .counters
                    .entry("elimination-yields-to-a-launch-2:sorties".to_string())
                    .or_insert(0) += 1;
                think!(self.journal(), Military, Decision,
                    "Intercepting a Culture run from the air";
                    "elimination-yields-to-a-launch-2: the freshly selected useful Theater sortie is applied before siege-wall and rebase assignments";
                    target);
                flown += 1;
                if flown == 2 {
                    break;
                }
            }
        }
    }

    /// Move one plane; future readiness is used only on a disposable forecast.
    pub(super) fn stage_culture_air_interception(
        &mut self,
        g: &mut Game,
        pid: usize,
        plan: &StrategicPlan,
    ) {
        if !self.culture_air_policy_allowed(g, pid, plan)
            || self.culture_air_stage_turn == Some(g.turn)
        {
            return;
        }
        let ready: Vec<_> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| Self::culture_air_ready(g, pid, *uid))
            .collect();
        if ready.len() < 2 {
            return;
        }
        for target in self.culture_air_racers(g, pid) {
            if g.is_at_war(pid, target) || !self.culture_air_opening_allowed(g, pid, target, plan) {
                continue;
            }
            let Some(opening) = self.culture_air_opening(g, pid, target) else {
                continue;
            };
            let mut war = g.speculative_clone();
            if war.apply(pid, &opening).is_err() {
                continue;
            }
            for &uid in &ready {
                if self.culture_air_sortie(&war, g, pid, uid, target).is_some() {
                    continue;
                }
                for action in g.legal_doctrine_actions(pid, uid) {
                    let Action::AirRebase { to, .. } = action else {
                        continue;
                    };
                    if !self.air_base_rebase_allowed(g, pid, uid, to) {
                        continue;
                    }
                    let mut future = g.speculative_clone();
                    if future.apply(pid, &action).is_err() {
                        continue;
                    }
                    let original = &g.units[&uid];
                    let moved = future.units.get_mut(&uid).unwrap();
                    moved.moves_left = original.moves_left;
                    moved.attacks_left = original.attacks_left;
                    if future.apply(pid, &opening).is_err()
                        || !self.culture_air_policy_allowed(&future, pid, plan)
                        || self
                            .culture_air_sortie(&future, g, pid, uid, target)
                            .is_none()
                    {
                        continue;
                    }
                    if g.apply(pid, &action).is_ok() {
                        self.culture_air_stage_turn = Some(g.turn);
                        *g.players[pid]
                            .counters
                            .entry("elimination-yields-to-a-launch-2:staged".to_string())
                            .or_insert(0) += 1;
                        think!(self.journal(), Military, Decision,
                            "Staging a bomber to intercept {}'s Culture run", g.players[target].civ;
                            "elimination-yields-to-a-launch-2: this legal rebase forecasts a currently visible useful Theater sortie with at least 60 HP; future readiness and the two-sortie opening are rechecked on a fresh turn";
                            to);
                        return;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
