use super::*;

impl AdvancedAi {
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
        let visible = g.player_visibility(pid);
        for (rival, pressure) in self.ranked_rival_victory_pressures(g, pid, &BTreeMap::new()) {
            if pressure.strategy != GrandStrategy::Religion
                || !self.victory_pressure_is_urgent(g, rival, pressure)
                || !self.campaign_target_legal(g, pid, rival)
            {
                continue;
            }
            let Some(faith) = g.players[rival].religion.as_deref() else {
                continue;
            };
            let spreaders: Vec<_> = g
                .player_unit_ids(rival)
                .into_iter()
                .filter(|uid| {
                    let u = &g.units[uid];
                    g.rules.units[u.kind].class == "religious"
                        && u.religion.as_deref() == Some(faith)
                        && visible.contains(&u.pos)
                        && g.player_city_ids(pid)
                            .iter()
                            .any(|cid| g.wdist(g.cities[cid].pos, u.pos) <= 6)
                })
                .collect();
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
