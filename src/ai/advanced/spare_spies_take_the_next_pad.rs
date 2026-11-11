//! `spare-spies-take-the-next-pad` (opt-in): on a Culture or Domination
//! seat, once a Spy of ours holds every Spaceport city of the leading space
//! racer we know, a further Spy goes to the next racer's free Spaceport city
//! instead of searching the leader's cities that have none.
//!
//! ## Why
//!
//! Live Deity Pericles and Kristina, 11-10 21Z to 11-11 01Z: 15 culture-lane
//! games, 9 lost to a rival Science Victory. Of 277 Spy journeys, 106 were
//! `racer-spies-search-for-the-pad` moves between the leading racer's cities,
//! and 88 of the 136 searches (65%) found no Disrupt Rocketry within eight
//! turns of setting out; 46 of them came when every city of the leader was
//! already on our board. A journey lifts the Spy off the map for two or three
//! turns, so the searches alone cost about 18 slot-turns a game, while the
//! Spies in transit held a quarter of all slot-turns after the first
//! Rocketry.
//!
//! The leader the Spies answer is often not the civilization that wins. In
//! four of the nine losses the eventual winner was a second racer whose pad
//! the network could see: civvis-20261111T000134Z (Arabia's pad seen at
//! t146, nine of its twelve cities known from t150; Ethiopia and Germany led
//! the race, Arabia won at t174 with no Disrupt against it),
//! civvis-20261110T223338Z (Cree led on projects to the end and our Spies
//! searched its cities 24 times; Gran Colombia won at t189). The every-pad
//! repost already moves a Spy idle in another foreign city to the nearest
//! racer's pad, but a Spy in the leader's city never leaves it.
//!
//! ## What the gene does
//!
//! Inert with the flag off, without `racer-keeps-the-spies`, off a Culture or
//! Domination aim or with the Science Victory off. On, for each Spy, when the
//! leading racer has at least one Spaceport city we know (a pad seen within
//! three tiles, or a city where Disrupt Rocketry was offered to a Spy of ours)
//! and every such city already holds another Spy of ours:
//!
//! - a Spy standing in another racer's known Spaceport city stays there,
//!   running Disrupt Rocketry whenever it is offered (Listening Post or Gain
//!   Sources while the pad lies pillaged), instead of returning to the
//!   leader;
//! - any other Spy (from home, in a city of the leader with no pad, or the
//!   second of two in one pad city) goes to the free known Spaceport city of
//!   the next racer, ranked as `racer_spy_leader` ranks them, the nearest
//!   first.
//!
//! While the leader has a known pad city free, or none known at all, nothing
//! changes: the posts and the search run as before, so the leader is never
//! left without a Spy at a pad we know.

use super::*;

/// What the gene decides for one Spy this pass.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SparePost {
    /// Keep standing in the next racer's pad city.
    Stay,
    /// Travel to this pad city of the next racer.
    Go(u32),
}

impl AdvancedAi {
    /// Whether the gene acts on this board.
    pub(super) fn spare_spies_on(&self, g: &Game) -> bool {
        self.spare_spies_take_the_next_pad
            && self.racer_keeps_the_spies
            && g.victory_conditions.science
            && matches!(
                self.active_victory_target(g),
                Some(VictoryTarget::Culture | VictoryTarget::Domination)
            )
    }

    /// Whether `cid` is a Spaceport city seat `pid` knows: a standing pad it
    /// has seen within three tiles, or a city where Disrupt Rocketry was
    /// offered to one of its Spies (`racer-spies-search-for-the-pad`).
    fn spare_pad_city(&self, g: &Game, pid: usize, cid: u32) -> bool {
        if self.racer_spy_found_pads.contains(&cid) {
            return true;
        }
        let Some(city) = g.cities.get(&cid) else {
            return false;
        };
        let explored = &g.players[pid].explored;
        city.districts.iter().any(|(district, pad)| {
            g.district_family(*district) == "spaceport"
                && explored.contains(pad)
                && g.wdist(city.pos, *pad) <= 3
                && g.map.get(*pad).is_some_and(|tile| !tile.pillaged)
        })
    }

    /// Whether another uncaptured Spy of `pid` than `spy` stands in `cid`;
    /// with `lower`, only one whose id is lower than `spy`'s.
    fn spare_held(g: &Game, pid: usize, spy: u32, cid: u32, lower: bool) -> bool {
        g.spies.values().any(|other| {
            other.owner == pid
                && other.id != spy
                && (!lower || other.id < spy)
                && other.captured_by.is_none()
                && other.city == Some(cid)
        })
    }

    /// Whether the leader's known pad cities are all held by other Spies of
    /// ours, at least one of them known: the condition under which `spy` is
    /// spare. A Spy standing in a leader pad city it shares only with Spies
    /// of higher id is that city's keeper and is not spare.
    fn spare_spy_is_spare(&self, g: &Game, pid: usize, spy: u32, leader: usize) -> bool {
        let here = g.spies.get(&spy).and_then(|agent| agent.city);
        let pads: Vec<u32> = g
            .player_city_ids(leader)
            .into_iter()
            .filter(|cid| self.spare_pad_city(g, pid, *cid))
            .collect();
        if pads.is_empty() {
            return false;
        }
        if here.is_some_and(|cid| pads.contains(&cid) && !Self::spare_held(g, pid, spy, cid, true))
        {
            return false;
        }
        pads.iter()
            .all(|cid| Self::spare_held(g, pid, spy, *cid, Some(*cid) == here))
    }

    /// Whether the gene keeps `spy` where it stands this pass: it is spare
    /// and stands in a known pad city of a racer other than the leader. Read
    /// before a running order's patience lets the Spy move on.
    pub(super) fn spare_spy_keeps(
        &self,
        g: &Game,
        pid: usize,
        spy: u32,
        leader: Option<usize>,
        racers: &BTreeSet<usize>,
    ) -> bool {
        let Some(leader) = leader.filter(|_| self.spare_spies_on(g)) else {
            return false;
        };
        let Some(here) = g.spies.get(&spy).and_then(|agent| agent.city) else {
            return false;
        };
        g.cities
            .get(&here)
            .is_some_and(|city| city.owner != leader && racers.contains(&city.owner))
            && self.spare_pad_city(g, pid, here)
            && self.spare_spy_is_spare(g, pid, spy, leader)
    }

    /// See the module: `Stay`, `Go` to the next racer's free pad city, or
    /// `None` to leave the Spy to the ordinary posts.
    pub(super) fn spare_spy_post(
        &self,
        g: &Game,
        pid: usize,
        spy: u32,
        leader: Option<usize>,
        racers: &BTreeSet<usize>,
        legal: &[Action],
    ) -> Option<SparePost> {
        let leader = leader.filter(|_| self.spare_spies_on(g))?;
        if self.spare_spy_keeps(g, pid, spy, Some(leader), racers) {
            return Some(SparePost::Stay);
        }
        if !self.spare_spy_is_spare(g, pid, spy, leader) {
            return None;
        }
        let here = g.spies.get(&spy).and_then(|agent| agent.city);
        let from = here
            .and_then(|cid| g.cities.get(&cid))
            .map(|city| city.pos)
            .or_else(|| {
                g.player_city_ids(pid)
                    .into_iter()
                    .find(|cid| g.cities[cid].is_capital)
                    .map(|cid| g.cities[&cid].pos)
            });
        let assignable = |cid: u32| {
            legal
                .iter()
                .any(|action| matches!(action, Action::AssignSpy { city, .. } if *city == cid))
        };
        let mut rest = racers.clone();
        rest.remove(&leader);
        while let Some(next) = Self::racer_spy_leader(g, &rest) {
            rest.remove(&next);
            let post = g
                .player_city_ids(next)
                .into_iter()
                .filter(|cid| {
                    Some(*cid) != here
                        && self.spare_pad_city(g, pid, *cid)
                        && !Self::spare_held(g, pid, spy, *cid, false)
                        && assignable(*cid)
                })
                .min_by_key(|cid| {
                    (
                        from.map_or(0, |from| g.wdist(from, g.cities[cid].pos)),
                        *cid,
                    )
                });
            if let Some(cid) = post {
                return Some(SparePost::Go(cid));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests;
