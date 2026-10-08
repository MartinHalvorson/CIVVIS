//! `culture-denial-heist`: a culture race is answered with the spies too.
//!
//! The science genes post a spy to every pad of a decisive space racer and run
//! Disrupt Rocketry there. A culture winner had no espionage answer at all, and
//! the board could not have sent one: the mirror carries only OUR Great Works,
//! so a rival city's `spy_city_has_stealable_work` always read false and the
//! Great Work heist was never a legal mission live.
//!
//! Today's eight culture losses (civvis-20261008T*, victory 3): the host
//! listed the heist on a spy's menu in seven of them, for 2-15 turns each, and
//! in the WINNER's city in three — St. Petersburg 14 turns (T062856Z, Russia
//! won at 160), Tokyo 12 (T083432Z, Japan at 169), Mistahi-Sipihk 8
//! (T090439Z, Cree at 145). Not one heist was sent. In the other four the spy
//! sat in a city of a rival that did not win. In all eight we captured none
//! of the winner's cities, held neither open borders nor a trade route with
//! it, and its power was within 0.5-1.4 times ours in five of them, so a war
//! was not the answer the seat had either.
//!
//! **What a heist is worth.** One Great Work's Tourism (two to four, more in
//! a themed building) out of the 200-450 a turn these winners made. This gene
//! takes the lever the host offers; it is not expected to turn a race on its
//! own.
//!
//! Against a rival the culture counter reads ([`AdvancedAi::culture_clock_rival`]:
//! a projected culture finish within the denial horizon, or a culture race the
//! Domination counter answers):
//!
//! - **Legal.** The host's menu makes the heist a legal mission
//!   (`Game::heist_reads_the_host_menu`).
//! - **Heist.** In such a rival's city the heist is run whenever it is
//!   offered, whatever the stock table's value-times-chance reads (Foment
//!   Unrest's 230 x 0.56 against the heist's 135 x 0.20 under Conquest).
//! - **Posting.** Every explored, standing Theater Square city of such a rival
//!   is worth [`CULTURE_HEIST_ASSIGN_PRIORITY`] to a spy while no other spy of
//!   ours holds it — below a decisive space racer's pad.
//! - **Repost.** An idle spy in a foreign city that is no such city, or one
//!   another spy of ours already holds, leaves for the nearest free one —
//!   unless the host offers the heist where it stands.

use std::collections::BTreeSet;

use super::AdvancedAi;
use crate::game::{Action, Game};
use crate::think;

/// Added to a posting at a free Theater Square city of a culture-clock
/// rival. Under `science-denial-every-pad`'s 1,500, so a launch pad keeps
/// first call on the network; well over the stock table's Culture 140.
pub(crate) const CULTURE_HEIST_ASSIGN_PRIORITY: i32 = 700;

impl AdvancedAi {
    /// The rivals whose Great Works are a target. Empty with the gene off.
    pub(crate) fn culture_heist_targets(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        if !self.culture_denial_heist || !g.victory_conditions.culture {
            return BTreeSet::new();
        }
        g.players
            .iter()
            .filter(|rival| {
                rival.id != pid
                    && rival.alive
                    && !rival.is_minor
                    && !rival.is_barbarian
                    && !g.same_team(pid, rival.id)
                    && g.has_met(pid, rival.id)
            })
            .map(|rival| rival.id)
            .filter(|rival| self.culture_clock_rival(g, *rival))
            .collect()
    }

    /// The cities of `targets` a heist is run from: each one with a standing
    /// Theater Square we have explored.
    pub(crate) fn culture_heist_cities(
        g: &Game,
        pid: usize,
        targets: &BTreeSet<usize>,
    ) -> BTreeSet<u32> {
        let explored = &g.players[pid].explored;
        targets
            .iter()
            .flat_map(|rival| g.player_city_ids(*rival))
            .filter(|cid| {
                g.cities[cid].districts.iter().any(|(district, position)| {
                    g.district_family(*district) == "theater_square"
                        && explored.contains(position)
                        && g.map.get(*position).is_some_and(|tile| !tile.pillaged)
                })
            })
            .collect()
    }

    /// The spy of ours that holds `cid`: the lowest id posted or travelling
    /// there, so of two in one city exactly one stays.
    fn culture_heist_holder(g: &Game, pid: usize, cid: u32) -> Option<u32> {
        g.spies
            .values()
            .filter(|spy| spy.owner == pid && spy.captured_by.is_none() && spy.city == Some(cid))
            .map(|spy| spy.id)
            .min()
    }

    /// What posting `spy` to `cid` is worth under the gene:
    /// [`CULTURE_HEIST_ASSIGN_PRIORITY`] for a heist city no other spy of
    /// ours holds, nothing elsewhere or with the gene off (`cities` empty).
    pub(crate) fn culture_heist_assignment_bonus(
        g: &Game,
        pid: usize,
        spy: u32,
        cities: &BTreeSet<u32>,
        cid: u32,
    ) -> i32 {
        if !cities.contains(&cid) {
            return 0;
        }
        let held = g
            .spies
            .values()
            .any(|other| other.owner == pid && other.id != spy && other.city == Some(cid));
        if held {
            0
        } else {
            CULTURE_HEIST_ASSIGN_PRIORITY
        }
    }

    /// Whether the host lists the heist on `spy`'s own menu.
    fn culture_heist_host_offers(g: &Game, spy: u32) -> bool {
        g.host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_missions.as_ref())
            .is_some_and(|menu| menu.contains("great_work_heist"))
    }

    /// The free heist city an idle `spy` standing in a foreign city should
    /// leave for: when its city is no heist city of a target, or another spy
    /// of ours holds it. A spy the host offers the heist to stays.
    pub(crate) fn culture_heist_repost(
        g: &Game,
        pid: usize,
        spy: u32,
        current_city: Option<u32>,
        cities: &BTreeSet<u32>,
    ) -> Option<u32> {
        let here = current_city?;
        let city = g.cities.get(&here)?;
        if city.owner == pid || Self::culture_heist_host_offers(g, spy) {
            return None;
        }
        if cities.contains(&here) && Self::culture_heist_holder(g, pid, here) == Some(spy) {
            return None;
        }
        cities
            .iter()
            .copied()
            .filter(|cid| *cid != here && Self::culture_heist_holder(g, pid, *cid).is_none())
            .min_by_key(|cid| (g.wdist(city.pos, g.cities[cid].pos), *cid))
    }

    /// The heist to run for `spy` in a target's city, if one is legal.
    pub(crate) fn culture_heist_action(
        g: &Game,
        pid: usize,
        spy: u32,
        targets: &BTreeSet<usize>,
    ) -> Option<Action> {
        let here = g.spies.get(&spy)?.city?;
        if !targets.contains(&g.cities.get(&here)?.owner) {
            return None;
        }
        g.legal_spy_actions(pid, spy).into_iter().find(
            |action| matches!(action, Action::SpyMission { mission, .. } if mission == "great_work_heist"),
        )
    }

    /// The journal line for a heist this gene sent.
    pub(crate) fn culture_heist_note(&self, g: &Game, spy: u32) {
        let Some(city) = g
            .spies
            .get(&spy)
            .and_then(|spy| spy.city)
            .and_then(|cid| g.cities.get(&cid))
        else {
            return;
        };
        think!(self.journal(), Military, Decision,
               "Stealing a Great Work from {}", city.name;
               "culture-denial-heist: {} is racing to a Culture Victory; each work taken is Tourism it no longer makes",
               g.players[city.owner].civ;
               city.pos);
    }
}

#[cfg(test)]
mod tests;
