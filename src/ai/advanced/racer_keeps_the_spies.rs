//! `racer-keeps-the-spies`: once a space racer exists, our Spies go to it and
//! stop gambling their slots elsewhere.
//!
//! Live Immortal 050758Z (pin 06882f25c) passed the production gate and lost
//! to Gaul's Science Victory at turn 185. Gaul took Rocketry at 130 and
//! landed the Satellite at 150, the Moon at 157, Mars at 164 and the
//! Exoplanet at 173. The denial network named Gaul a racer from turn 130
//! (`space-denial-spies-from-rocketry`), but our two Spies stood in London,
//! the war plan's capital, running Foment Unrest (four times), Listening Post
//! and Gain Sources. Both were lost on Foment Unrest, at 141 and 146, and
//! neither freed its slot: the host keeps a captured Spy's place in our
//! capacity while the export, and so the board, drops the agent. The host
//! refused a Spy in every city from 142 to 160 (`buildable` listed it in 0 of
//! 10) while `science-denial-trains-spies` wanted two and held none, so no
//! Spy stood in Gaul until 166 and the first Disrupt Rocketry ran at 176.
//! Neither London Spy was ever sent to Gaul: the leader and every-pad
//! re-posts move only an idle Spy, and only toward a Spaceport we have seen,
//! which Gaul's was not before about 167.
//!
//! Across the four Immortal passers of 10-12/14 that lost to Science or
//! Culture, 17 of 33 Spy losses came on Foment Unrest, 11 losses held their
//! slot (the host still refusing a Spy three turns later), and after the
//! winner's Rocketry 25 of 41 (050758Z) and 55 of 115 (163500Z) Spy-turns
//! were spent in another rival's cities.
//!
//! While `denial_spy_racers` names a rival (Rocketry or a space project under
//! `space-denial-spies-from-rocketry`, the decisive race otherwise):
//!
//! 1. a Spy standing outside every racer's cities starts no capture-risk
//!    operation ([`RACER_SPY_RISKY_OPERATIONS`]);
//! 2. an idle Spy outside every racer's cities, or one whose operation has
//!    just ended, is posted to the leading racer: its free city nearest a
//!    Spaceport we have seen, else its capital, else its city nearest the
//!    capital — before any pad is seen;
//! 3. in a racer's city it runs Disrupt Rocketry when offered, and otherwise
//!    only Listening Post or Gain Sources, so it keeps its slot for the
//!    disruption.
//!
//! The home counter-spy and the culture-heist posting keep their places
//! ahead of the post, and a Spy in a culture racer's city keeps the heist.
//! Under the gene the Spy claim also says when every city is refused a Spy
//! below our capacity: those slots are held by captured agents the board
//! cannot see.

use super::*;

/// Operations a Spy outside every space racer's cities does not start while
/// a racer exists: each risks the agent, and a captured agent keeps its slot.
pub(crate) const RACER_SPY_RISKY_OPERATIONS: &[&str] = &[
    "foment_unrest",
    "siphon_funds",
    "fabricate_scandal",
    "sabotage_production",
    "recruit_partisans",
    "neutralize_governor",
    "great_work_heist",
    "steal_tech_boost",
];

/// Operations a Spy in a racer's city runs while Disrupt Rocketry is not
/// offered, best first.
const RACER_SPY_SAFE_OPERATIONS: &[&str] = &["listening_post", "gain_sources"];

/// Standard turns between two journal notes of slots held by captured agents.
const RACER_SPY_SLOT_NOTE_TURNS: u32 = 10;

/// Standard turns the refusal must run before it is noted: a Spy in transit
/// also leaves the export for a turn or two while it keeps its slot.
const RACER_SPY_SLOT_PERSIST_TURNS: u32 = 3;

impl AdvancedAi {
    /// The space racers the Spies answer. Empty with the gene off.
    pub(crate) fn racer_spy_racers(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        if !self.racer_keeps_the_spies {
            return BTreeSet::new();
        }
        self.denial_spy_racers(g, pid)
    }

    /// The racer the Spies go to: most space projects landed, then most
    /// technologies, then the lowest seat.
    pub(crate) fn racer_spy_leader(g: &Game, racers: &BTreeSet<usize>) -> Option<usize> {
        racers.iter().copied().max_by_key(|rival| {
            (
                Self::science_denial_stages(g, *rival),
                g.players[*rival].techs.len(),
                std::cmp::Reverse(*rival),
            )
        })
    }

    /// Whether `mission` is an operation the gene withholds outside a racer's
    /// cities.
    pub(crate) fn racer_spy_risky(mission: &str) -> bool {
        RACER_SPY_RISKY_OPERATIONS.contains(&mission)
    }

    /// Whether `city` belongs to one of `racers`.
    pub(crate) fn racer_spy_city_is_a_racers(
        g: &Game,
        racers: &BTreeSet<usize>,
        city: Option<u32>,
    ) -> bool {
        city.and_then(|city| g.cities.get(&city))
            .is_some_and(|city| racers.contains(&city.owner))
    }

    /// The city of `leader` an idle `spy` standing at `current_city` should
    /// be posted to, if any: none while it stands in a city of one of
    /// `stays` (the space racers and the culture racers), otherwise the
    /// leader's city no other Spy of ours holds, ranked by a seen standing
    /// Spaceport within three tiles, then the capital, then nearness to the
    /// capital. Only cities the host lets us post to (`legal`) count.
    pub(crate) fn racer_spy_post(
        g: &Game,
        spy: u32,
        current_city: Option<u32>,
        leader: Option<usize>,
        stays: &BTreeSet<usize>,
        legal: &[Action],
    ) -> Option<u32> {
        let leader = leader?;
        let pid = g.spies.get(&spy)?.owner;
        if Self::racer_spy_city_is_a_racers(g, stays, current_city) {
            return None;
        }
        let explored = &g.players[pid].explored;
        let cities = g.player_city_ids(leader);
        let capital = cities
            .iter()
            .copied()
            .find(|cid| g.cities[cid].is_capital)
            .map(|cid| g.cities[&cid].pos);
        let held = |cid: u32| {
            g.spies.values().any(|other| {
                other.owner == pid
                    && other.id != spy
                    && other.captured_by.is_none()
                    && other.city == Some(cid)
            })
        };
        let seen_pad = |cid: u32| {
            let centre = g.cities[&cid].pos;
            g.cities[&cid].districts.iter().any(|(district, pad)| {
                g.district_family(*district) == "spaceport"
                    && explored.contains(pad)
                    && g.wdist(centre, *pad) <= 3
                    && g.map.get(*pad).is_some_and(|tile| !tile.pillaged)
            })
        };
        cities
            .into_iter()
            .filter(|cid| {
                !held(*cid)
                    && legal.iter().any(
                        |action| matches!(action, Action::AssignSpy { city, .. } if city == cid),
                    )
            })
            .min_by_key(|cid| {
                let city = &g.cities[cid];
                (
                    std::cmp::Reverse(seen_pad(*cid)),
                    std::cmp::Reverse(city.is_capital),
                    capital.map_or(0, |capital| g.wdist(capital, city.pos)),
                    *cid,
                )
            })
    }

    /// The operation a Spy in a racer's city runs under the gene: Disrupt
    /// Rocketry when offered, else the first offered of
    /// [`RACER_SPY_SAFE_OPERATIONS`]. `None` leaves the Spy where it is.
    pub(crate) fn racer_spy_operation(legal: &[Action]) -> Option<&Action> {
        let offered = |wanted: &str| {
            legal.iter().find(
                |action| matches!(action, Action::SpyMission { mission, .. } if mission == wanted),
            )
        };
        offered("disrupt_rocketry").or_else(|| {
            RACER_SPY_SAFE_OPERATIONS
                .iter()
                .find_map(|wanted| offered(wanted))
        })
    }

    /// Under the gene, a journal note when the Spy claim wants a Spy, holds
    /// fewer than our capacity and no city is let train one, turn after turn
    /// for [`RACER_SPY_SLOT_PERSIST_TURNS`]: the host is keeping those slots
    /// for captured agents the board cannot see. At most once every
    /// [`RACER_SPY_SLOT_NOTE_TURNS`] standard turns.
    pub(crate) fn racer_spy_note_held_slots(
        &self,
        g: &Game,
        pid: usize,
        held: usize,
        needed: usize,
        refused_everywhere: bool,
    ) {
        if !self.racer_keeps_the_spies || !refused_everywhere || held >= needed {
            return;
        }
        let capacity = g.spy_capacity(pid).max(0) as usize;
        if held >= capacity {
            return;
        }
        // (first turn of the unbroken refusal, last turn seen, last note).
        let (since, seen, last) = self.racer_spies_slot_note.get();
        let since = if since > 0 && seen + 1 == g.turn {
            since
        } else {
            g.turn
        };
        self.racer_spies_slot_note.set((since, g.turn, last));
        if g.turn < since + g.standard_duration(RACER_SPY_SLOT_PERSIST_TURNS) {
            return;
        }
        if last > 0 && g.turn < last + g.standard_duration(RACER_SPY_SLOT_NOTE_TURNS) {
            return;
        }
        self.racer_spies_slot_note.set((since, g.turn, g.turn));
        let missing = capacity - held;
        let turns = g.turn - since + 1;
        think!(self.journal(), Military, Detail,
               "Spy slots held by captured agents";
               "racer-keeps-the-spies: {held} Spy(s) held against a capacity of {capacity} and \
                {needed} wanted, yet no city may train one for {turns} turns; the host is \
                keeping {missing} slot(s) for agents it captured");
    }
}

#[cfg(test)]
mod tests;
