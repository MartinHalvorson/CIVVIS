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
//! The leading racer is the one with the most space projects landed, where
//! a racer one project behind but making [`RACER_SCIENCE_EDGE`] times the
//! Science a turn of the racer ahead counts as level with it; level racers
//! are told apart by the turns their Exoplanet Expedition still needs (when
//! both have launched it), then Science a turn. Live Emperor 100910Z: the
//! Mapuche landed each stage first, but Greece made 1.3-1.6 times their
//! Science from turn 201, launched the Exoplanet four turns after them at
//! 4-5 points a turn against 1, and won at 233; every post of the gene's
//! first version went to the Mapuche.
//!
//! A Spy standing in a racer's city that is no longer the leader's moves to
//! the leader unless Disrupt Rocketry is offered where it stands, and a
//! Listening Post or Gain Sources it is running there does not hold it (a
//! Disrupt Rocketry does). In the leader's city the every-pad re-post no
//! longer pulls it away toward another racer's pad.
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

/// A racer one space project behind the leader counts as level with it when
/// its Science a turn is at least this many times the leader's.
pub(crate) const RACER_SCIENCE_EDGE: f64 = 1.25;

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

    /// The racer the Spies go to: the most space projects landed, a racer
    /// one project short of the most counting level when its Science a turn
    /// is [`RACER_SCIENCE_EDGE`] times the best of those ahead of it; then,
    /// between two that have both launched the Exoplanet Expedition, the
    /// fewer turns it still needs; then Science a turn, technologies and the
    /// lowest seat.
    pub(crate) fn racer_spy_leader(g: &Game, racers: &BTreeSet<usize>) -> Option<usize> {
        let stages = |rival: usize| Self::science_denial_stages(g, rival);
        let science = |rival: usize| Self::seat_science_per_turn(g, rival);
        let top = racers.iter().map(|rival| stages(*rival)).max()?;
        let top_science = racers
            .iter()
            .filter(|rival| stages(**rival) == top)
            .map(|rival| science(*rival))
            .fold(0.0_f64, f64::max);
        let level = |rival: usize| {
            let landed = stages(rival);
            if landed + 1 == top
                && top_science > 0.0
                && science(rival) >= RACER_SCIENCE_EDGE * top_science
            {
                top
            } else {
                landed
            }
        };
        let finish = |rival: usize| {
            let speed = g.exoplanet_speed(rival);
            (speed > 0.0).then(|| {
                (g.science_victory_points_needed(rival) - g.science_victory_points(rival)).max(0.0)
                    / speed
            })
        };
        racers.iter().copied().max_by(|left, right| {
            level(*left)
                .cmp(&level(*right))
                .then_with(|| match (finish(*left), finish(*right)) {
                    (Some(left), Some(right)) => right.total_cmp(&left),
                    _ => std::cmp::Ordering::Equal,
                })
                .then_with(|| science(*left).total_cmp(&science(*right)))
                .then_with(|| {
                    g.players[*left]
                        .techs
                        .len()
                        .cmp(&g.players[*right].techs.len())
                })
                .then_with(|| right.cmp(left))
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
    /// be posted to, if any: none while it stands in the leader's city or a
    /// culture racer's, nor in another racer's city where Disrupt Rocketry
    /// is offered; otherwise the leader's city no other Spy of ours holds,
    /// ranked by a seen standing Spaceport within three tiles, then the
    /// capital, then nearness to the capital. Only cities the host lets us
    /// post to (`legal`) count.
    pub(crate) fn racer_spy_post(
        g: &Game,
        spy: u32,
        current_city: Option<u32>,
        leader: Option<usize>,
        racers: &BTreeSet<usize>,
        culture_racers: &BTreeSet<usize>,
        legal: &[Action],
    ) -> Option<u32> {
        let leader = leader?;
        let pid = g.spies.get(&spy)?.owner;
        let here = current_city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner);
        if here == Some(leader)
            || here.is_some_and(|owner| culture_racers.contains(&owner))
            || (here.is_some_and(|owner| racers.contains(&owner))
                && Self::racer_spy_disrupt_offered(g, spy, legal))
        {
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

    /// Whether Disrupt Rocketry is offered to `spy` where it stands, by the
    /// board's menu or the host's.
    pub(crate) fn racer_spy_disrupt_offered(g: &Game, spy: u32, legal: &[Action]) -> bool {
        legal.iter().any(|action| {
            matches!(action, Action::SpyMission { mission, .. } if mission == "disrupt_rocketry")
        }) || g
            .host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_missions.as_ref())
            .is_some_and(|menu| menu.contains("disrupt_rocketry"))
    }

    /// Whether `spy`, whose last order is still running, should move on
    /// anyway: it stands in a racer's city that is not the leader's and is
    /// running only a Listening Post or Gain Sources there (a Disrupt
    /// Rocketry, a journey or any other operation runs to its end). Never
    /// with the gene off (`leader` is `None`).
    pub(crate) fn racer_spy_moves_on(
        g: &Game,
        spy: u32,
        leader: Option<usize>,
        racers: &BTreeSet<usize>,
    ) -> bool {
        let Some(leader) = leader else {
            return false;
        };
        let Some(agent) = g.spies.get(&spy) else {
            return false;
        };
        let owner = agent
            .city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner);
        if owner == Some(leader) || !owner.is_some_and(|owner| racers.contains(&owner)) {
            return false;
        }
        let running = g
            .host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_operation.clone())
            .or_else(|| agent.mission.as_ref().map(|mission| mission.kind.clone()));
        running.is_some_and(|operation| RACER_SPY_SAFE_OPERATIONS.contains(&operation.as_str()))
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
