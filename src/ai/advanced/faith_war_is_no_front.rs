//! `faith-war-is-no-front`: the war `faith-war-before-the-flip` opens to
//! condemn a founder's spreaders stays open, but its rival is not a front:
//! the campaign target is chosen as though that war were not running, the
//! one-war front is appointed among the other enemies, and the war does not
//! count as the one already being fought when the plan's target is declared
//! on (`major_wars` and `one_war_holds_declaration`).
//!
//! **Why (live Emperor, the 45 pin-era games of 2026-10-18).** 51 of the 106
//! war periods that began between turns 60 and 170 were faith wars. The
//! journal opens them "defensive, no siege", yet as the only war they are
//! the only active front, and the campaign aimed at their rival on 810 of
//! 1,023 campaign turns. They took 8 cities in 51 wars, 1.0 per 100 campaign
//! turns, against 6.4 for the wars we declared to conquer (23 cities in
//! 16). None captured a city with the rival 15 or more tiles away or at our
//! power or above. civvis-20261018T105455Z marched 24 units on Lisbon,
//! 14-24 tiles out behind the Inca's closed borders, from the faith war of
//! turn 104; Lisbon walled up by 129, nothing was taken, and the adjacent
//! Inca, at a sixth of our power by 150, took the capital at 180. The one
//! domination win (090059Z) declared its own war on adjacent America.
//!
//! Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// The rival of the running faith war while the gene is on.
    pub(super) fn faith_war_front_exempt(&self, g: &Game, pid: usize) -> Option<usize> {
        if !self.faith_war_is_no_front {
            return None;
        }
        self.faith_war_before_the_flip_war
            .as_ref()
            .map(|war| war.rival)
            .filter(|rival| g.is_at_war(pid, *rival))
    }

    /// `enemies` less the running faith war's rival; unchanged with the gene
    /// off or no faith war running.
    pub(super) fn without_faith_war(
        &self,
        g: &Game,
        pid: usize,
        mut enemies: Vec<usize>,
    ) -> Vec<usize> {
        if let Some(rival) = self.faith_war_front_exempt(g, pid) {
            enemies.retain(|enemy| *enemy != rival);
        }
        enemies
    }
}

#[cfg(test)]
mod tests;
