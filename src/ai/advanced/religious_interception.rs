use super::*;

fn visible_home_spreaders(g: &Game, pid: usize, rival: usize, faith: &str) -> Vec<u32> {
    let visible = g.player_visibility(pid);
    g.player_unit_ids(rival)
        .into_iter()
        .filter(|uid| {
            let unit = &g.units[uid];
            g.rules.units[unit.kind].class == "religious"
                && unit.religion.as_deref() == Some(faith)
                && visible.contains(&unit.pos)
                && g.player_city_ids(pid)
                    .iter()
                    .any(|cid| g.wdist(g.cities[cid].pos, unit.pos) <= 6)
        })
        .collect()
}

/// The interception opens a major war to condemn one spreader, so it needs
/// at least the rival's power behind it. Live King 2026-10-03T100536Z
/// declared on Phoenicia at turn 50 with 228 power against their 317. The
/// next turn the plan read "at war and losing ground at home", and the war
/// ended in our own peace offer at turn 80 ("the last window was a rout").
pub(crate) const RELIGIOUS_INTERCEPTION_POWER_FLOOR: f64 = 1.0;

impl AdvancedAi {
    /// A defensive condemnation must not occupy the only major-war slot
    /// after its victory threat is gone and a different visible city can be
    /// campaigned against. The host still decides whether to accept white
    /// peace; this only asks when the game's minimum war term has elapsed.
    pub(super) fn religious_interception_handoff_peace(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
        plan: &StrategicPlan,
    ) -> bool {
        let Some((intercepted, opened)) = self.religious_interception_war else {
            return false;
        };
        if intercepted != rival
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !g.is_at_war(pid, rival)
            || g.turn.saturating_sub(opened) < g.standard_duration(10).max(1)
            || g.peace_available_at(pid, rival).is_some()
            || g.emergency_war_pair(pid, rival)
            || self.urgent_victory_threat(g, rival)
            || self.religious_interception_holds_war(g, pid, rival)
            || plan.strategy != GrandStrategy::Conquest
            || plan.threatened_city.is_some()
            || plan.target_player == Some(rival)
            || self.one_war_prizes_in_reach(g, pid)
        {
            return false;
        }
        plan.target_city
            .and_then(|cid| g.cities.get(&cid))
            .is_some_and(|city| {
                Some(city.owner) == plan.target_player
                    && city.owner != pid
                    && !g.same_team(pid, city.owner)
                    && g.sees(&g.player_vision_frame(pid), city.pos)
            })
    }

    /// A war opened to stop an approaching religious victory should remain
    /// open while that founder still has visible spreaders near our cities.
    /// Once the local threat or the rival's victory stake recedes, ordinary
    /// peace evaluation resumes.
    pub(super) fn religious_interception_holds_war(
        &self,
        g: &Game,
        pid: usize,
        rival: usize,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.deny_leaders
            || !g.is_at_war(pid, rival)
        {
            return false;
        }
        let Some(stakes) = self.religious_veto_engaged(g, pid) else {
            return false;
        };
        if stakes.founder != rival || stakes.our_converted == 0 {
            return false;
        }
        let Some(faith) = g.players[rival].religion.as_deref() else {
            return false;
        };
        !visible_home_spreaders(g, pid, rival, faith).is_empty()
    }

    /// A religious match point can be intercepted at home even before a scout
    /// finds the founder's cities. Promise a legal condemnation, not a distant
    /// siege: replay the declaration and the same-turn interception first.
    pub(super) fn religious_interception_opening(&mut self, g: &mut Game, pid: usize) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || !self.deny_leaders
            || !g.victory_conditions.religious
            || !self.war_is_affordable(g, pid)
            || self.threatened_city(g, pid).is_some()
            || g.players
                .iter()
                .any(|p| !p.is_minor && !p.is_barbarian && p.id != pid && g.is_at_war(pid, p.id))
        {
            return false;
        }
        for (rival, pressure) in self.ranked_rival_victory_pressures(g, pid, &BTreeMap::new()) {
            if pressure.strategy != GrandStrategy::Religion
                || !self.victory_pressure_is_urgent(g, rival, pressure)
                || !self.campaign_target_legal(g, pid, rival)
                || g.military_power(pid)
                    < RELIGIOUS_INTERCEPTION_POWER_FLOOR * g.military_power(rival)
            {
                continue;
            }
            let Some(faith) = g.players[rival].religion.as_deref() else {
                continue;
            };
            let spreaders = visible_home_spreaders(g, pid, rival, faith);
            if spreaders.is_empty() {
                continue;
            }
            let Some(opening) = self.preferred_war_opening(g, pid, rival) else {
                continue;
            };
            if !matches!(
                opening,
                Action::DeclareWar { .. } | Action::DeclareWarWithCasusBelli { .. }
            ) {
                continue;
            }
            let mut declared = g.speculative_clone();
            if declared.apply(pid, &opening).is_err() {
                continue;
            }
            for uid in g.player_unit_ids(pid) {
                let unit = &g.units[&uid];
                let spec = &g.rules.units[unit.kind];
                if spec.class != "military"
                    || spec.domain.as_deref() == Some("air")
                    || unit.hp as f64 <= self.base.w.withdraw_hp
                    || unit.moves_left <= 0.0
                {
                    continue;
                }
                for target_unit in &spreaders {
                    let position = g.units[target_unit].pos;
                    if g.wdist(unit.pos, position) > 1 {
                        continue;
                    }
                    let mut forecast = declared.speculative_clone();
                    let movement = (unit.pos != position).then_some(Action::Move {
                        unit: uid,
                        to: position,
                    });
                    if movement
                        .as_ref()
                        .is_some_and(|a| forecast.apply(pid, a).is_err())
                    {
                        continue;
                    }
                    let condemn = Action::CondemnHeretic {
                        unit: uid,
                        target_unit: *target_unit,
                    };
                    if forecast.apply(pid, &condemn).is_err() {
                        continue;
                    }
                    if g.apply(pid, &opening).is_err() {
                        return false;
                    }
                    self.religious_interception_war = Some((rival, g.turn));
                    if let Some(action) = movement {
                        if g.apply(pid, &action).is_err() {
                            return true;
                        }
                    }
                    let condemned = g.apply(pid, &condemn).is_ok();
                    think!(self.journal(), Military, Strategy,
                        "Opening a religious interception against {}", g.players[rival].civ;
                        "a visible spreader at home supplies an immediate counter to the religious match point; condemnation executed: {condemned}");
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests;
