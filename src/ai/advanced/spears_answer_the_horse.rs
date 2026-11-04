//! `spears-answer-the-horse`: refine an existing emergency land defender
//! against the visible local melee attackers. No additional unit demand.
//!
//! Native development case 061724Z t106 offered Spearman (25 strength) in
//! three turns and Heavy Chariot (28) in four, both for 33 production, while
//! a Knight approached the capital. Raw strength misses anti-cavalry's +10.
//! The final defense handoff remains authoritative; this only chooses its
//! unit, preserving walls, repairs, paid work and one-turn finishes.

use super::*;

const LOCAL_THREAT_STEPS: usize = 6;

/// Class modifiers used by `Game::matchup_bonus` in game/actions.rs. War
/// Carts are explicitly immune; ranged cavalry also receive the spear bonus.
/// This is a conservative class comparison, not a complete combat forecast.
fn class_bonus(g: &Game, own: Name, other: Name) -> f64 {
    let a = &g.rules.units[own];
    let b = &g.rules.units[other];
    if a.promotion_class == "anti_cavalry"
        && other != "war_cart"
        && (matches!(
            b.promotion_class.as_str(),
            "light_cavalry" | "heavy_cavalry"
        ) || (b.cavalry && b.promotion_class == "ranged"))
    {
        10.0
    } else if (a.promotion_class == "melee" || own == "nihang")
        && b.promotion_class == "anti_cavalry"
    {
        5.0
    } else {
        0.0
    }
}

impl AdvancedAi {
    /// A strictly better class matchup, no more expensive or slower than the exact
    /// defender otherwise retained/chosen by the existing emergency pass.
    pub(super) fn spear_for_local_horse(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        selected: &Item,
        committed: Option<&Item>,
    ) -> Option<Item> {
        if !self.spears_answer_the_horse || !matches!(selected, Item::Unit { .. }) {
            return None;
        }
        let local_melee = |kind: Name| {
            let spec = &g.rules.units[kind];
            spec.class == "military"
                && !spec.siege
                && !spec.has_ranged_attack()
                && spec.is_melee_capable()
                && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                && spec.promotion_class != "recon"
        };
        if !matches!(selected, Item::Unit { unit } if local_melee(*unit)) {
            return None;
        }
        let reference = match committed.filter(|item| Self::active_queue_answers_siege(g, item)) {
            Some(item @ Item::Unit { .. }) => item,
            Some(_) => return None,
            None => selected,
        };
        let Item::Unit { unit: baseline } = reference else {
            return None;
        };
        let original = &g.rules.units[baseline];
        if !local_melee(*baseline) || g.item_invested_production(cid, reference) != 0.0 {
            return None;
        }
        let turns = |item: &Item| {
            g.host_production_turns(cid, item)
                .filter(|value| value.is_finite() && *value > 0.0)
        };
        let baseline_turns = turns(reference)?;
        if baseline_turns <= 1.0 {
            return None;
        }
        let offered_cost = |item: &Item| {
            g.host_buildable
                .get(&cid)?
                .get(&Game::production_block_key(item))?
                .cost
                .filter(|value| value.is_finite() && *value > 0.0)
        };
        let cost = offered_cost(reference)?;
        let visible = g.player_vision_frame(pid);
        let center = g.cities.get(&cid).filter(|city| city.owner == pid)?.pos;
        let enemies: Vec<_> = g
            .units
            .values()
            .filter(|enemy| {
                let spec = &g.rules.units[enemy.kind];
                enemy.owner != pid
                    && g.is_at_war(pid, enemy.owner)
                    && enemy.hp > 0
                    && spec.class == "military"
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && !g.is_embarked(enemy)
                    && g.wdist(center, enemy.pos) <= LOCAL_THREAT_STEPS as i32
                    && g.sees(&visible, enemy.pos)
                    && g.unit_visible_to(enemy.id, pid)
                    && g.route_distance(enemy.id, center, 1)
                        .is_some_and(|steps| steps <= LOCAL_THREAT_STEPS)
            })
            .collect();
        if !enemies
            .iter()
            .any(|enemy| class_bonus(g, crate::name!("spearman"), enemy.kind) > 0.0)
        {
            return None;
        }
        let margin = |kind: Name| {
            enemies
                .iter()
                .map(|enemy| {
                    g.rules.units[kind].strength + class_bonus(g, kind, enemy.kind)
                        - crate::game::effective_strength(g.unit_strength(enemy, false), enemy.hp)
                        - class_bonus(g, enemy.kind, kind)
                })
                .fold(f64::INFINITY, f64::min)
        };
        let baseline_margin = margin(*baseline);
        g.rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.class == "military"
                    && spec.promotion_class == "anti_cavalry"
                    && !spec.siege
                    && !spec.has_ranged_attack()
                    && spec.is_melee_capable()
                    && !matches!(spec.domain.as_deref(), Some("sea" | "air"))
                    && spec.requires_resource.is_none()
                    && spec.resource_cost <= 0.0
                    && spec.resource_maintenance <= 0.0
                    && spec.maintenance <= original.maintenance
            })
            .filter_map(|(kind, _)| {
                let item = Item::Unit { unit: *kind };
                if !g.can_produce(pid, cid, &item) || turns(&item)? > baseline_turns {
                    return None;
                }
                let candidate_cost = offered_cost(&item)?;
                let score = margin(*kind);
                (candidate_cost.is_finite()
                    && candidate_cost > 0.0
                    && candidate_cost <= cost
                    && score.is_finite()
                    && score > baseline_margin)
                    .then_some((score, *kind, item))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)))
            .map(|(_, _, item)| item)
    }
}

#[cfg(test)]
mod tests;
