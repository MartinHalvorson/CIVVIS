//! Independent, default-off relocation of the existing Royal Society credit.
//!
//! Native Indonesia215700Z offered Royal Society in Lampung from turn149;
//! the committed Spaceport was in Majapahit. Both science-drive versions put
//! Royal Society's credit behind a launch-city check, although the shipped
//! modifier is player-wide (Expansion2/Data/Expansion1_Buildings.xml:318,
//! MODIFIER_PLAYER_ADJUST_UNIT_PROJECT_PERCENT, amount2 at623).
//!
//! Extend that existing credit to its Plaza city only on an assigned Science
//! seat with an active drive, a committed pad and a positive native remaining
//! quote that fits the known support horizon. Keep the original launch-city
//! result, including v2's own horizon handling. No price is retuned, no queue
//! is directly claimed, and no Builder or contribution job is fabricated.
//! A quoted completion is a forecast, not proof of a strength improvement.

use super::*;

impl AdvancedAi {
    pub(super) fn royal_society_serves_the_launch_bonus(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
    ) -> Option<f64> {
        if !self.royal_society_serves_the_launch
            || self.victory_target != Some(VictoryTarget::Science)
            || !matches!(item, Item::Building { building } if building == "royal_society")
        {
            return None;
        }
        let drive = self.science_drive?;
        let legacy_launch = if self.science_victory_drive_2 {
            self.science_drive_launch_city(g, pid)
        } else {
            drive.launch_city
        };
        // Never double-pay or replace the old local valuation.
        if legacy_launch == Some(cid) {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid
            || !g.city_has_district_family(city, crate::name!("government_plaza"))
            || !g
                .player_city_ids(pid)
                .into_iter()
                .any(|city| Self::city_has_spaceport_commitment(g, city))
        {
            return None;
        }
        let turns = g.host_production_turns(cid, item)?;
        if !turns.is_finite() || turns <= 0.0 || turns >= Self::science_support_horizon(g, pid) {
            return None;
        }
        science_victory_drive::SCIENCE_DRIVE_BUILDING_BONUS
            .iter()
            .find(|(building, _)| *building == "royal_society")
            .map(|(_, value)| *value)
    }
}

#[cfg(test)]
mod tests;
