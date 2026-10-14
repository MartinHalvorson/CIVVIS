//! `strategic-deposit-prey`: aim the Domination campaign at the city whose
//! tiles hold the strategic resource the army cannot modernize without.
//!
//! Live King civvis-20261003T052455Z drew no Oil and no Aluminum at turn 182,
//! twelve turns after Refining and forty after Radio. Every Bombard, Line
//! Infantry and Ironclad upgrade read "1 Oil required", and the air surge
//! stood down at turn 161 for want of Aluminum. Not one of the eleven Oil or
//! five Aluminum deposits in sight lay inside our borders: Spain, the rival
//! the army was fighting, worked two Oil wells and held two Aluminum tiles.
//! Meanwhile the siege train stood with three Bombards before Valladolid's
//! 400 walls.

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::{City, Game};
use crate::name::Name;
use std::collections::BTreeSet;

/// What each wanted strategic resource on a rival city's tiles adds to that
/// city's campaign value: half a capital's 180, about a 13-population city's
/// development.
pub(super) const STRATEGIC_DEPOSIT_PREY_VALUE: f64 = 90.0;

/// How far from a city its owned tiles are read: a city's working radius.
const STRATEGIC_DEPOSIT_RADIUS: i32 = 3;

impl AdvancedAi {
    /// The campaign value a Domination conquest adds for `city`'s strategic
    /// deposits the army needs and the empire draws none of. See
    /// [`STRATEGIC_DEPOSIT_PREY_VALUE`].
    pub(super) fn strategic_deposit_prey_value(
        &self,
        g: &Game,
        pid: usize,
        city: &City,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.strategic_deposit_prey
            || strategy != GrandStrategy::Conquest
            || city.owner == pid
            || self.active_victory_target(g) != Some(VictoryTarget::Domination)
        {
            return 0.0;
        }
        let wanted = self.strategic_deposits_wanted(g, pid);
        if wanted.is_empty() {
            return 0.0;
        }
        let held: BTreeSet<&Name> = g
            .map
            .disk(city.pos, STRATEGIC_DEPOSIT_RADIUS)
            .into_iter()
            .filter_map(|pos| g.map.get(pos))
            .filter(|tile| tile.owner_city == Some(city.id))
            .filter_map(|tile| tile.resource.as_ref())
            .filter(|resource| wanted.contains(*resource))
            .collect();
        held.len() as f64 * STRATEGIC_DEPOSIT_PREY_VALUE
    }

    /// Strategic resources a military unit we have the tech for requires,
    /// revealed to us, of which the empire draws no income.
    pub(super) fn strategic_deposits_wanted(&self, g: &Game, pid: usize) -> BTreeSet<Name> {
        let player = &g.players[pid];
        let known =
            |tech: &Option<Name>| tech.as_ref().is_none_or(|tech| player.techs.contains(tech));
        g.rules
            .units
            .values()
            .filter(|spec| spec.class == "military" && spec.tech.is_some() && known(&spec.tech))
            .filter_map(|spec| spec.requires_resource.clone())
            .filter(|resource| {
                g.rules
                    .resources
                    .get(resource)
                    .is_some_and(|spec| spec.class == "strategic" && known(&spec.tech))
            })
            .filter(|resource| g.strategic_resource_rate(pid, resource.as_str()) <= 0.0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AdvancedAi, GrandStrategy, VictoryTarget};
    use super::STRATEGIC_DEPOSIT_PREY_VALUE;
    use crate::game::Game;

    fn board() -> (Game, u32, u32) {
        let mut g = Game::new_full(2, 40, 24, 373201, 300, 0, false);
        for id in g.units.keys().copied().collect::<Vec<_>>() {
            g.remove_unit(id);
        }
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.resource = None;
        }
        g.found_city_for(0, (8, 10), None);
        g.found_city_for(1, (30, 10), None);
        let plain = g.found_city_for(1, (20, 6), None);
        let oil = g.found_city_for(1, (20, 14), None);
        g.players[0].met.insert(1);
        g.players[1].met.insert(0);
        g.players[0].techs.insert(crate::name!("refining"));
        g.players[0].techs.insert(crate::name!("steel"));
        let well = g
            .map
            .disk(g.cities[&oil].pos, 1)
            .into_iter()
            .find(|pos| {
                *pos != g.cities[&oil].pos
                    && g.map.get(*pos).is_some_and(|t| t.owner_city == Some(oil))
            })
            .expect("a tile the oil city owns");
        g.map.tiles.get_mut(&well).unwrap().resource = Some(crate::name!("oil"));
        (g, plain, oil)
    }

    #[test]
    fn a_rival_city_holding_the_oil_we_lack_is_worth_taking_first() {
        let (g, plain, oil) = board();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        let value = |ai: &AdvancedAi, city| {
            ai.strategic_deposit_prey_value(&g, 0, &g.cities[&city], GrandStrategy::Conquest)
        };
        assert_eq!(value(&ai, oil), 0.0, "off by default");
        ai.enable_strategic_deposit_prey();
        assert!(ai
            .strategic_deposits_wanted(&g, 0)
            .contains(&crate::name!("oil")));
        assert_eq!(value(&ai, oil), STRATEGIC_DEPOSIT_PREY_VALUE);
        assert_eq!(value(&ai, plain), 0.0);
        assert!(
            ai.campaign_city_value(&g, 0, &g.cities[&oil], GrandStrategy::Conquest)
                < ai.campaign_city_value(&g, 0, &g.cities[&plain], GrandStrategy::Conquest),
            "the oil city ranks ahead of its twin"
        );
        assert_eq!(
            ai.strategic_deposit_prey_value(&g, 0, &g.cities[&oil], GrandStrategy::Recovery),
            0.0,
            "only a conquest prices it"
        );
    }

    #[test]
    fn a_resource_we_already_draw_or_cannot_use_is_not_prey() {
        let (mut g, _, oil) = board();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_strategic_deposit_prey();
        // No unit we can field needs Oil before Steel or Replaceable Parts.
        g.players[0].techs.remove(&crate::name!("steel"));
        assert!(!ai
            .strategic_deposits_wanted(&g, 0)
            .contains(&crate::name!("oil")));
        assert_eq!(
            ai.strategic_deposit_prey_value(&g, 0, &g.cities[&oil], GrandStrategy::Conquest),
            0.0
        );
        // An income of it already supplies the army.
        g.players[0].techs.insert(crate::name!("steel"));
        std::sync::Arc::make_mut(&mut g.observed_strategic_income_adjustments)
            .entry(0)
            .or_default()
            .insert(crate::name!("oil"), 2.0);
        assert!(g.strategic_resource_rate(0, "oil") > 0.0);
        assert_eq!(
            ai.strategic_deposit_prey_value(&g, 0, &g.cities[&oil], GrandStrategy::Conquest),
            0.0
        );
    }
}
