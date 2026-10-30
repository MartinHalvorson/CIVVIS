//! `counterweight-defends-the-diplomat`: a Diplomatic seat with no faith of
//! its own in its cities buys the counterweight the Domination seat buys.
//!
//! A Religious Victory needs its faith to hold a strict majority in every
//! living major, so a seat whose own faith holds none of its cities vetoes it
//! only by keeping half of them on another faith. `religious_defense` buys
//! those counterweight Missionaries, and the counterweight family
//! (`counterweight-spends-the-bank`, `counterweight-from-the-first-convert`,
//! `counterweight-flips-the-small-towns`) sizes and funds them, but the whole
//! family answers only the Domination lane, and only a seat that never
//! founded: a founder goes to `religious_spending`, which buys units of its
//! own faith and so needs a city that still follows it.
//!
//! Live Immortal civvis-20261030T171157Z, the first Lv 7 diplomatic seat: we
//! founded Catholicism at turn 58; it held 3 of our 6 cities at 80 and none
//! from 100. Islam (the Aztecs) held 8 of our 10 cities from 145, Buddhism
//! (the Inca) the other 2. We stood at 16 Diplomatic Victory Points at turn
//! 160 against the rivals' 3-6, banked 300-780 Faith from turn 130 with no
//! religious unit in the field, and lost to Islam's Religious Victory at 168.
//!
//! With the gene on, on the Diplomacy lane:
//! - the counterweight family's Domination gate opens
//!   ([`AdvancedAi::counterweight_lane`]);
//! - a founder whose faith holds none of our cities defends as a faithless
//!   seat ([`AdvancedAi::counterweight_faithless`]), so `religious_defense`
//!   buys Missionaries of the faith a city of ours does follow.
//!
//! Off, and on every other lane: unchanged.
use super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

impl AdvancedAi {
    /// Whether the counterweight family answers this seat's lane: the
    /// Domination lane, and the Diplomacy lane under the gene.
    pub(super) fn counterweight_lane(&self, g: &Game) -> bool {
        match self.active_victory_target(g) {
            Some(VictoryTarget::Domination) => true,
            Some(VictoryTarget::Diplomacy) => self.counterweight_defends_the_diplomat,
            _ => false,
        }
    }

    /// Whether the seat defends as one with no faith of its own: it never
    /// founded, or (under the gene, on the Diplomacy lane) its faith holds
    /// none of its cities.
    pub(super) fn counterweight_faithless(&self, g: &Game, pid: usize) -> bool {
        let Some(own) = g.players[pid].religion.as_deref() else {
            return true;
        };
        self.counterweight_defends_the_diplomat
            && self.active_victory_target(g) == Some(VictoryTarget::Diplomacy)
            && !g
                .player_city_ids(pid)
                .iter()
                .any(|cid| g.city_religion(&g.cities[cid]) == Some(own))
    }
}

#[cfg(test)]
mod tests;
