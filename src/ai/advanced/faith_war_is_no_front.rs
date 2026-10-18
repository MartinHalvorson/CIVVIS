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
//! **Version 2** (`faith-war-is-no-front-2`) also exempts the war a religious
//! interception opened (`religious_interception_war`) and a war on a major
//! none of whose cities we have located. civvis-20261018T115456Z, the first
//! game under version one, opened an interception on the Khmer "as its last
//! holdout" at turn 78; the campaign read "already at war; no city objective
//! yet" until the Khmer won on Culture at 144, while India, three cities
//! beside us at 33-40 power against our 591-646, was never attacked.
//!
//! Off, nothing changes.

use super::*;

impl AdvancedAi {
    /// The rivals at war with us that are no front: the faith war's, and
    /// under version 2 the religious interception's and every major none of
    /// whose cities we have located. Empty with both versions off.
    pub(super) fn no_front_rivals(&self, g: &Game, pid: usize) -> Vec<usize> {
        let mut exempt: Vec<usize> = Vec::new();
        if self.faith_war_is_no_front || self.faith_war_is_no_front_2 {
            if let Some(war) = self.faith_war_before_the_flip_war.as_ref() {
                exempt.push(war.rival);
            }
        }
        if self.faith_war_is_no_front_2 {
            if let Some((rival, _)) = self.religious_interception_war {
                exempt.push(rival);
            }
            exempt.extend(
                g.players
                    .iter()
                    .filter(|other| {
                        other.id != pid
                            && other.alive
                            && !other.is_minor
                            && !other.is_barbarian
                            && g.player_city_ids(other.id).is_empty()
                    })
                    .map(|other| other.id),
            );
        }
        exempt.retain(|rival| g.is_at_war(pid, *rival));
        exempt.sort_unstable();
        exempt.dedup();
        exempt
    }

    /// `enemies` less [`Self::no_front_rivals`]; unchanged with both
    /// versions off or no such war running.
    pub(super) fn without_faith_war(
        &self,
        g: &Game,
        pid: usize,
        mut enemies: Vec<usize>,
    ) -> Vec<usize> {
        let exempt = self.no_front_rivals(g, pid);
        enemies.retain(|enemy| !exempt.contains(enemy));
        enemies
    }
}

#[cfg(test)]
mod tests;
