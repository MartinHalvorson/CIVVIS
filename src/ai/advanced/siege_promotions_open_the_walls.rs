//! Promote siege guns for the known walls on their current major-war campaign.
use super::*;

impl AdvancedAi {
    pub(super) fn wall_campaign_promotion(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        strategy: GrandStrategy,
        available: &[Name],
        target_city: Option<u32>,
    ) -> Option<Name> {
        if !self.siege_promotions_open_the_walls || strategy != GrandStrategy::Conquest {
            return None;
        }
        let unit = g.units.get(&uid)?;
        let spec = g.rules.units.get(&unit.kind)?;
        if unit.owner != pid || spec.promotion_class != "siege" || !spec.siege {
            return None;
        }
        // An already-running war need not have a prewar CampaignPlan. The
        // command phase supplies the current StrategicPlan's actual city.
        let cid = target_city.or_else(|| {
            let campaign = self.campaign.as_ref()?;
            self.campaign_objective_city(g, pid, Some(campaign.target))
        })?;
        let city = g.cities.get(&cid)?;
        let owner = g.players.get(city.owner)?;
        if !owner.alive || owner.is_minor || owner.is_barbarian || !g.is_at_war(pid, city.owner) {
            return None;
        }
        // Hidden live state cannot supply a wall pool. A remembered city is
        // usable only while its identity, position and observed owner agree.
        let walls = if g.player_visibility(pid).contains(&city.pos) {
            Some(city.wall_hp)
        } else {
            g.players[pid]
                .remembered_cities
                .get(&cid)
                .filter(|memory| memory.owner == city.owner && memory.pos == city.pos)
                .map(|memory| memory.wall_hp)
        }?;
        if walls <= 0 {
            return None;
        }
        // A rules mod without this district branch keeps the normal scorer.
        let shells = g.rules.promotions.get("shells")?;
        if shells.class != "siege"
            || !shells.requires.contains(&crate::name!("crew_weapons"))
            || shells
                .effects
                .get("ranged_vs_district")
                .copied()
                .unwrap_or(0.0)
                <= 0.0
        {
            return None;
        }
        // Keep attack-after-moving when already offered. Otherwise take the
        // district strength, or its defensive prerequisite before experience
        // is committed to a branch that only strengthens shots at units.
        ["expert_crew", "shells", "crew_weapons"]
            .into_iter()
            .find_map(|wanted| {
                available
                    .iter()
                    .find(|name| name.as_str() == wanted)
                    .copied()
            })
    }
}

#[cfg(test)]
mod tests;
