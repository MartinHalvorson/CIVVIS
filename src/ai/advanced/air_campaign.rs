use super::*;

impl AdvancedAi {
    /// A current mission must not indefinitely keep a siege bomber outside
    /// its city's range. Compare it with one sortie after a legal rebase,
    /// using the existing strike valuation on the otherwise unchanged board.
    pub(super) fn air_campaign_rebase_is_better(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        plan: &StrategicPlan,
        to: Pos,
        mission_value: f64,
    ) -> bool {
        if self.active_victory_target(g) != Some(VictoryTarget::Domination)
            || plan.strategy != GrandStrategy::Conquest
            || plan.threatened_city.is_some()
        {
            return false;
        }
        let Some(city) = plan.target_city.and_then(|cid| g.cities.get(&cid)) else {
            return false;
        };
        let range = g.unit_attack_range(uid);
        if city.owner == pid
            || !g.is_at_war(pid, city.owner)
            || g.players[city.owner].is_minor
            || (city.wall_hp <= 0 && city.hp <= 1)
            || g.wdist(g.units[&uid].pos, city.pos) <= range
            || g.wdist(to, city.pos) > range
        {
            return false;
        }
        let taker_ready = g.units.values().any(|unit| {
            let spec = &g.rules.units[unit.kind];
            unit.owner == pid
                && unit.hp >= 50
                && spec.class == "military"
                && spec.is_melee_capable()
                && !matches!(spec.domain.as_deref(), Some("air" | "sea"))
                && !g.is_embarked(unit)
                && g.wdist(unit.pos, city.pos) <= 3
        });
        if !taker_ready {
            return false;
        }

        let mut forecast = g.speculative_clone();
        if forecast
            .apply(pid, &Action::AirRebase { unit: uid, to })
            .is_err()
        {
            return false;
        }
        // Rebasing consumes this turn. Price a single later sortie rather
        // than pretending the real aircraft can attack after this move.
        let bomber = forecast.units.get_mut(&uid).unwrap();
        bomber.moves_left = 1.0;
        bomber.attacks_left = 1;
        bomber.moved = false;
        let projected = self.air_strike_value(&forecast, pid, uid, city.pos, plan);
        if projected <= mission_value {
            return false;
        }
        think!(self.journal(), Military, Decision,
            "Rebasing for the siege of {}", city.name;
            "a city strike from the new base is worth {projected:.0}, versus {mission_value:.0} for the current mission; a capture unit is nearby");
        true
    }
}

#[cfg(test)]
mod tests;
