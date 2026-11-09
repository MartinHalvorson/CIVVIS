use super::*;

/// An existing wall and healthy melee garrison can cover a short reinforcement
/// build. Longer or unobserved timings retain the immediate melee answer.
const MAX_FIREPOWER_TURNS: f64 = 3.0;

impl AdvancedAi {
    /// Native Athens t76 already held a full-health Heavy Chariot behind walls,
    /// but kept training another 28-strength body against Pike and Shot. Its
    /// current host menu offered a three-turn Crossbowman: 30 melee defense and
    /// 40 ranged attack. Fill that role without displacing the first garrison,
    /// a fortification/repair, or a soldier already retained by the governor.
    pub(super) fn firepower_for_garrison(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        defence: &Item,
    ) -> Option<Item> {
        let Item::Unit { unit } = defence else {
            return None;
        };
        let incumbent = g.rules.units.get(unit)?;
        if !self.battlefront_observation
            || !incumbent.is_melee_capable()
            || incumbent.siege
            || incumbent.promotion_class == "recon"
            || matches!(incumbent.domain.as_deref(), Some("sea" | "air"))
            || !g.players.iter().any(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.is_at_war(pid, other.id)
            })
        {
            return None;
        }
        let city = g.cities.get(&cid).filter(|city| city.owner == pid)?;
        if g.city_max_wall_hp(city) <= 0
            || city.wall_hp <= 0
            || !g.unit_ids_at(city.pos).iter().any(|uid| {
                let guard = &g.units[uid];
                let spec = &g.rules.units[guard.kind];
                guard.owner == pid
                    && !g.is_embarked(guard)
                    && f64::from(guard.hp) > self.base.w.withdraw_hp
                    && spec.class == "military"
                    && spec.is_melee_capable()
                    && !spec.siege
                    && spec.promotion_class != "recon"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
            })
            || !Self::imminent_city_attack(g, pid, cid, &g.player_vision_frame(pid))
        {
            return None;
        }
        let incumbent_power = incumbent.strength.max(incumbent.ranged_attack_strength());
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.class == "military"
                    && !spec.siege
                    && spec.promotion_class != "recon"
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && spec.range >= 2
                    && spec.strength >= incumbent.strength
                    && spec.ranged_attack_strength() > incumbent_power
            })
            .filter_map(|(unit, spec)| {
                let item = Item::Unit { unit: *unit };
                let turns = g.host_production_turns(cid, &item)?;
                (turns.is_finite()
                    && turns > 0.0
                    && turns <= MAX_FIREPOWER_TURNS
                    && g.can_produce(pid, cid, &item))
                .then_some((
                    spec.ranged_attack_strength(),
                    spec.strength,
                    turns,
                    *unit,
                    item,
                ))
            })
            .max_by(|a, b| {
                a.0.total_cmp(&b.0)
                    .then(a.1.total_cmp(&b.1))
                    .then(b.2.total_cmp(&a.2))
                    .then(a.3.cmp(&b.3))
            })
            .map(|(_, _, _, _, item)| item)
    }
}

#[cfg(test)]
mod tests;
