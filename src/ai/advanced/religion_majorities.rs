//! `religion-finishes-majorities`: a Religion-target founder's offensive
//! spreaders work on living major civilizations that do not follow its faith.
//!
//! Religious victory asks for a majority of cities in every living major,
//! not every city in the world. The ordinary city score rewards population,
//! capital status and rival pressure, so it can send the corps to a city-state
//! or the remaining holdout city of an already converted civilization while
//! another major still needs conversion. This filter leaves that city's
//! charges for a civilization still needed by the victory condition.
//!
//! `GetReligionInMajorityOfCities` (shipped
//! `Base/Assets/UI/PartialScreens/WorldRankings.lua:2049`) is the host verdict;
//! it includes cities we have not located. Never infer a live rival's majority
//! from the visible subset if that verdict is missing. Native CIVVIS boards
//! use the complete city's strict-majority count. A reverted majority makes
//! the civilization eligible again on the next observed board.
//!
//! Home religious support is selected outside this offensive-only filter.
//! The gene is off by default and changes no other target or adopted faith.

use super::*;

impl AdvancedAi {
    pub(super) fn religion_campaign_needs_city(
        &self,
        g: &Game,
        pid: usize,
        city: &crate::game::City,
        religion: &str,
    ) -> bool {
        if !self.religion_finishes_majorities
            || self.active_victory_target(g) != Some(VictoryTarget::Religion)
            || g.players[pid].religion.as_deref() != Some(religion)
            || city.owner == pid
        {
            return true;
        }
        let owner = &g.players[city.owner];
        if !owner.alive || owner.is_minor || owner.is_barbarian {
            return false;
        }
        let converted = match g.observed_majority_religion.get(&city.owner) {
            Some(faith) => faith == religion,
            None if g.observed_public_empire_stats.contains_key(&city.owner) => false,
            None => g.civ_follows_religion(city.owner, religion),
        };
        if converted {
            think!(self.journal(), Faith, Detail,
                "{} needs no foreign spread", city.name;
                "religion-finishes-majorities: civilization {} already follows {}",
                city.owner, religion);
        }
        !converted
    }
}

#[cfg(test)]
mod tests;
