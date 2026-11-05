//! `war-defense-spends-the-runway`: a war we are losing keeps its production
//! queues while the treasury can carry the deficit.
//!
//! **The finding (live Deity Korea science, 2026-11-04/05).** The insolvency
//! guard (`live_war_economy_requires_recovery`) hands every queue to
//! `economic_recovery_item` whenever Gold per turn is below -0.5 and the
//! treasury is under `100 + 25 x cities`. Its only wartime exception is the
//! first garrison (fewer military units than cities). Since 11-01, 176 of its
//! decisions came on turns the plan read "recovery | at war and losing ground
//! at home". The median treasury covered the deficit for 13.6 turns, a quarter
//! covered 30 or more, and 66 decisions in 15 games had 20 turns or more. The
//! picks were Campus Research Grants (34), Theater Square Festivals (25),
//! Commercial Hubs and their investment project (35), Banks, Stock Exchanges
//! and Markets (31), Harbors, Granaries and Traders.
//!
//! In `civvis-20261105T033244Z` the Aztecs declared at t151. Our power fell
//! from 1009 to 590 by t160 and stayed at 440-500 while theirs went from 2383
//! to 6046 by t180. At t161 the guard read "168 Gold at -1.7/turn; recovery
//! avoids further upkeep" and started a Stock Exchange in the capital and a
//! Commercial Hub in Jeonju. At t168 it read 93 Gold at -1.0/turn and started a
//! Theater Square Festival, a Market and another Stock Exchange. Five cities
//! fell between t176 and t202, and the seat was eliminated at t203.
//!
//! **The mechanism.** The guard was written for the Rome run, which held 160
//! Gold at -24.39 per turn: about 7 turns of runway, then units disbanded for
//! unpaid maintenance. At -1.7 per turn the same reserve lasts about 100
//! turns. The guard reads the treasury level and ignores the runway, so a
//! seat that is losing a war but can still pay does the economic work and
//! loses its cities.
//!
//! **The gene.** While a living major at war with us fields at least
//! [`OUTMATCHED_RATIO`] times our military power, and the treasury covers the
//! current deficit for at least [`WAR_DEFENSE_RUNWAY_TURNS`] turns, the guard
//! stands aside and the city queue goes through the ordinary war and Recovery
//! scoring. A short runway still enters recovery, as before. Each new
//! defender's upkeep shortens the runway, and the guard comes back once it
//! drops under the floor. With the gene off, nothing changes.

use super::AdvancedAi;
use crate::game::Game;
use crate::think;

/// A major at war with us fielding this multiple of our military power is a
/// war we are losing. `civvis-20261105T033244Z` stood at 4.3x (561 against
/// 2409) when its queues went to a Stock Exchange.
pub(super) const OUTMATCHED_RATIO: f64 = 1.5;

/// Game turns of the current deficit the treasury must cover before a losing
/// war keeps its queues. The Rome run the guard was written for had about 7.
pub(super) const WAR_DEFENSE_RUNWAY_TURNS: f64 = 15.0;

impl AdvancedAi {
    /// `war-defense-spends-the-runway`: whether the insolvency guard should
    /// stand aside this frame. True only with the gene on, a deficit the
    /// treasury covers for [`WAR_DEFENSE_RUNWAY_TURNS`] or more, and a living
    /// major at war with `pid` at [`OUTMATCHED_RATIO`] or more of our power.
    pub(super) fn war_defense_spends_the_runway_holds(&self, g: &Game, pid: usize) -> bool {
        if !self.war_defense_spends_the_runway {
            return false;
        }
        let player = &g.players[pid];
        let deficit = -player.gold_per_turn;
        if deficit <= 0.0 {
            return false;
        }
        let runway = player.gold.max(0.0) / deficit;
        if runway < WAR_DEFENSE_RUNWAY_TURNS {
            return false;
        }
        let ours = g.military_power(pid).max(1.0);
        let attacker = g
            .players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && g.is_at_war(pid, rival.id)
            })
            .map(|rival| (rival.id, g.military_power(rival.id)))
            .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
        let Some((rival, power)) = attacker else {
            return false;
        };
        if power < ours * OUTMATCHED_RATIO {
            return false;
        }
        think!(self.journal(), Economy, Detail,
               "The war keeps its queues";
               "war-defense-spends-the-runway: player {rival} fields {power:.0} against our \
                {ours:.0}, and {:.0} Gold covers the {deficit:.1}/turn deficit for {runway:.0} turns",
               player.gold);
        true
    }
}

#[cfg(test)]
mod tests;
