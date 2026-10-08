//! `science-denial-every-pad`: a space race is answered at every pad.
//!
//! `science-threat-denial` aims one spy at one pad per threat — the Spaceport
//! city of theirs that produces the most — and `science-denial-spy-reads-the-
//! leader` makes that the posting that outranks the rest. A space project can
//! be built in any city with a Spaceport, and the Emperor AI builds several.
//!
//! Live Emperor G398 (civvis-20261008T123821Z) passed the production gate,
//! led Production and military at turn 200, and lost to Japan's Science
//! Victory at 210. Japan stood four Spaceports by turn 165. Both of our
//! Disrupt Rocketry runs (turns 178 and 197) landed — the Tokyo pad reads
//! pillaged on turns 182-189 and 201-202 — and Japan completed the Mars base
//! by 192 and the Exoplanet Expedition by 207 from the other three. Three of
//! our spies sat in Tokyo at once (turns 199-201) while the other pads had
//! none, and the one spy in a second pad city, Japan's (39, 25), ran Foment
//! Unrest on turns 184, 188 and 192 with the host offering Disrupt Rocketry
//! there each time: the board files a foreign Spaceport under the NEAREST
//! city of its owner, and (40, 27) is two tiles from both (39, 25) and
//! (40, 29), so the board never offered the operation the host did.
//!
//! Against a rival with [`EVERY_PAD_MIN_STAGES`] space projects landed, or a
//! science race reading [`EVERY_PAD_MIN_PRESSURE`]:
//!
//! - **Posting.** Every standing pad city we have explored — and, when the
//!   pad is equidistant from two of the owner's cities, each of them — is
//!   worth [`EVERY_PAD_ASSIGN_PRIORITY`] to a spy while no other spy of ours
//!   holds it, so the network spreads one to a pad.
//! - **Repost.** An idle spy in a foreign city that is no such pad city, or
//!   one another spy of ours already holds, leaves for the nearest free one —
//!   unless the host offers Disrupt Rocketry where it stands.
//! - **Disrupt.** In such a rival's city, Disrupt Rocketry is run whenever it
//!   is offered, whatever the stock table's value-times-chance reads; and when
//!   the host offers it from this city but the board filed the Spaceport under
//!   a neighbour, the pad is filed under this city first.
//!
//! Spies are not bought: the board prices a unit at four times its cost, and
//! a 337-Production Spy against G398's 140-407 Gold treasury over turns
//! 170-206 never came within reach.

use std::collections::BTreeSet;

use super::{AdvancedAi, GrandStrategy};
use crate::game::{Action, Game};
use crate::think;
use crate::Pos;

/// Space projects a rival must have landed before every pad of theirs is a
/// target: the Moon landing, so the Mars project is the one being built.
pub(crate) const EVERY_PAD_MIN_STAGES: usize = 2;

/// Or a science race reading this, in `rival_victory_pressure`'s currency.
pub(crate) const EVERY_PAD_MIN_PRESSURE: i32 = 80;

/// Added to a posting at a free pad city of such a rival. Clears the
/// leader's [`super::science_threat_denial::DENIAL_LEADER_SPY_ASSIGN_PRIORITY`]
/// plus the base pad bonus, so a second spy goes to a second pad rather than
/// to the stock table's favourite.
pub(crate) const EVERY_PAD_ASSIGN_PRIORITY: i32 = 1_500;

/// Civ VI places a district within three tiles of its city.
const PAD_CITY_REACH: i32 = 3;

impl AdvancedAi {
    /// The rivals whose every pad is a target. Empty with the gene off.
    pub(crate) fn every_pad_threats(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        if !self.science_denial_every_pad || !g.victory_conditions.science {
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
            .filter(|rival| {
                Self::science_denial_stages(g, rival.id) >= EVERY_PAD_MIN_STAGES || {
                    let pressure = self.rival_victory_pressure(g, rival.id);
                    pressure.strategy == GrandStrategy::Science
                        && pressure.progress >= EVERY_PAD_MIN_PRESSURE
                }
            })
            .map(|rival| rival.id)
            .collect()
    }

    /// The cities a spy can disrupt `threats`' standing, explored pads from:
    /// the city the board files each pad under, and any other city of the
    /// same owner exactly as near the pad, since the board's nearest-city
    /// filing cannot tell those apart and the host's can.
    pub(crate) fn every_pad_cities(
        g: &Game,
        pid: usize,
        threats: &BTreeSet<usize>,
    ) -> BTreeSet<u32> {
        let explored = &g.players[pid].explored;
        let mut cities = BTreeSet::new();
        for rival in threats {
            let owned = g.player_city_ids(*rival);
            for cid in &owned {
                for (district, pad) in g.cities[cid].districts.iter() {
                    if g.district_family(*district) != "spaceport"
                        || !explored.contains(pad)
                        || g.map.get(*pad).is_none_or(|tile| tile.pillaged)
                    {
                        continue;
                    }
                    let nearest = owned
                        .iter()
                        .map(|other| g.wdist(*pad, g.cities[other].pos))
                        .min()
                        .unwrap_or(i32::MAX);
                    cities.extend(owned.iter().copied().filter(|other| {
                        let reach = g.wdist(*pad, g.cities[other].pos);
                        reach == nearest && reach <= PAD_CITY_REACH
                    }));
                    cities.insert(*cid);
                }
            }
        }
        cities
    }

    /// The spy of ours that holds `cid`: the lowest id posted or travelling
    /// there, so of two in one city exactly one stays.
    fn every_pad_holder(g: &Game, pid: usize, cid: u32) -> Option<u32> {
        g.spies
            .values()
            .filter(|spy| spy.owner == pid && spy.captured_by.is_none() && spy.city == Some(cid))
            .map(|spy| spy.id)
            .min()
    }

    /// What posting `spy` to `cid` is worth under the gene:
    /// [`EVERY_PAD_ASSIGN_PRIORITY`] for a pad city no other spy of ours
    /// holds, nothing elsewhere or with the gene off (`pads` empty).
    pub(crate) fn every_pad_assignment_bonus(
        g: &Game,
        pid: usize,
        spy: u32,
        pads: &BTreeSet<u32>,
        cid: u32,
    ) -> i32 {
        if !pads.contains(&cid) {
            return 0;
        }
        let held = g
            .spies
            .values()
            .any(|other| other.owner == pid && other.id != spy && other.city == Some(cid));
        if held {
            0
        } else {
            EVERY_PAD_ASSIGN_PRIORITY
        }
    }

    /// Whether the host lists Disrupt Rocketry on `spy`'s own menu.
    fn every_pad_host_offers_disrupt(g: &Game, spy: u32) -> bool {
        g.host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_missions.as_ref())
            .is_some_and(|menu| menu.contains("disrupt_rocketry"))
    }

    /// The free pad city an idle `spy` standing in a foreign city should
    /// leave for: when its city is no pad city of a threat, or another spy of
    /// ours holds it. A spy the host offers Disrupt Rocketry to stays.
    pub(crate) fn every_pad_repost(
        g: &Game,
        pid: usize,
        spy: u32,
        current_city: Option<u32>,
        pads: &BTreeSet<u32>,
    ) -> Option<u32> {
        let here = current_city?;
        let city = g.cities.get(&here)?;
        if city.owner == pid || Self::every_pad_host_offers_disrupt(g, spy) {
            return None;
        }
        if pads.contains(&here) && Self::every_pad_holder(g, pid, here) == Some(spy) {
            return None;
        }
        pads.iter()
            .copied()
            .filter(|cid| *cid != here && Self::every_pad_holder(g, pid, *cid).is_none())
            .min_by_key(|cid| (g.wdist(city.pos, g.cities[cid].pos), *cid))
    }

    /// The Disrupt Rocketry to run for `spy` in a threat's city, if any. When
    /// the host offers it from here and the board does not, the nearest
    /// standing Spaceport of the same owner within reach is filed under this
    /// city first — the host's menu names the pad's owner, the board's
    /// nearest-city filing only guesses it.
    pub(crate) fn every_pad_disrupt(
        g: &mut Game,
        pid: usize,
        spy: u32,
        threats: &BTreeSet<usize>,
    ) -> Option<Action> {
        let here = g.spies.get(&spy)?.city?;
        let (owner, centre) = g.cities.get(&here).map(|city| (city.owner, city.pos))?;
        if !threats.contains(&owner) {
            return None;
        }
        let disrupt = |g: &Game| {
            g.legal_spy_actions(pid, spy).into_iter().find(|action| {
                matches!(action, Action::SpyMission { mission, .. } if mission == "disrupt_rocketry")
            })
        };
        if let Some(action) = disrupt(g) {
            return Some(action);
        }
        if !Self::every_pad_host_offers_disrupt(g, spy) {
            return None;
        }
        let pad: Pos = g
            .wdisk(centre, PAD_CITY_REACH)
            .into_iter()
            .filter(|pos| {
                g.map.get(*pos).is_some_and(|tile| {
                    !tile.pillaged
                        && tile
                            .district
                            .is_some_and(|district| g.district_family(district) == "spaceport")
                        && tile
                            .owner_city
                            .and_then(|cid| g.cities.get(&cid))
                            .is_some_and(|city| city.owner == owner)
                })
            })
            .min_by_key(|pos| (g.wdist(centre, *pos), *pos))?;
        let district = g.map.get(pad)?.district?;
        let previous = g.map.get(pad).and_then(|tile| tile.owner_city);
        if let Some(old) = previous
            .filter(|cid| *cid != here)
            .and_then(|cid| g.cities.get_mut(&cid))
        {
            let kept: Vec<Pos> = old
                .districts
                .positions(district)
                .iter()
                .copied()
                .filter(|pos| *pos != pad)
                .collect();
            old.districts.remove(district);
            for pos in kept {
                old.districts.insert(district, pos);
            }
            old.owned_tiles.retain(|held| *held != pad);
        }
        if let Some(tile) = g.map.tiles.get_mut(&pad) {
            tile.owner_city = Some(here);
        }
        let city = g.cities.get_mut(&here)?;
        if !city.districts.positions(district).contains(&pad) {
            city.districts.insert(district, pad);
        }
        if !city.owned_tiles.contains(&pad) {
            city.owned_tiles.push(pad);
        }
        disrupt(g)
    }

    /// The journal line for a disruption this gene sent.
    pub(crate) fn every_pad_note(&self, g: &Game, spy: u32) {
        let Some(city) = g
            .spies
            .get(&spy)
            .and_then(|spy| spy.city)
            .and_then(|cid| g.cities.get(&cid))
        else {
            return;
        };
        think!(self.journal(), Military, Decision,
               "Disrupting the Spaceport at {}", city.name;
               "science-denial-every-pad: {} has landed {} space projects; every pad they stand is a launch site",
               g.players[city.owner].civ, Self::science_denial_stages(g, city.owner);
               city.pos);
    }
}

#[cfg(test)]
mod tests;
