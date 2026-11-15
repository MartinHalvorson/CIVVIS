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
//! both have launched it), then by Production a turn, which builds the
//! projects. Live 192420Z-cont1: France and Hungary stood level on two
//! projects at turn 205 and on three from 219 to 229; Science a turn named
//! Hungary (298-462 against 264-359) and the Spies left France, while France
//! out-built it (606-747 Production against 449-641), launched Mars eight
//! turns and the Exoplanet first, and won at 234 with no Disrupt Rocketry
//! against it after turn 197. Live Emperor 100910Z: the
//! Mapuche landed each stage first, but Greece made 1.3-1.6 times their
//! Science from turn 201, launched the Exoplanet four turns after them at
//! 4-5 points a turn against 1, and won at 233; every post of the gene's
//! first version went to the Mapuche.
//!
//! A Spy standing in a racer's city that is no longer the leader's moves to
//! the leader unless this seat can run the Disrupt Rocketry offered where it
//! stands, and a Listening Post or Gain Sources it is running there does not
//! hold it (a Disrupt Rocketry does). In the leader's city the every-pad
//! re-post no longer pulls it away toward another racer's pad.
//!
//! "Can run" is the board's own offer, or the host's when
//! `science-denial-every-pad` can file a standing pad of that owner under the
//! city ([`AdvancedAi::racer_spy_disrupt_runnable`]). The host's menu alone
//! held a Spy the step below never sent: live Deity Gran Colombia
//! civvis-20261112T075931Z kept its last Spy in Australia's Melbourne from
//! turn 176 to 210, nine Listening Posts and no Disrupt Rocketry, while the
//! gene named Byzantium the leader from turn 179 ("Byzantium leads the space
//! race") and Byzantium landed Mars at 198, launched at 204 and won at 210.
//! The host offered the disruption there on every pass ("Disrupt Rocketry is
//! offered in Australia's Melbourne", t179-t209); the only Australian pad
//! within reach, (32, 18), last seen at turn 169, read pillaged on our board
//! after Australia had repaired it, so the board listed no disruption and
//! the every-pad filing found no standing pad. With every city of ours
//! refused a Spy from turn 180 (slots held by captured agents), it was the
//! network's only agent from 193.
//!
//! The home counter-spy and the culture-heist posting keep their places
//! ahead of the post, and a Spy in a culture racer's city keeps the heist.
//! Under the gene the Spy claim also says when every city is refused a Spy
//! below our capacity: those slots are held by captured agents the board
//! cannot see.
//!
//! **`racer-spies-search-for-the-pad`.** Rule 2 posts a Spy to the leader's
//! city nearest a Spaceport we have seen, else its capital, and once a Spy
//! stands in any of the leader's cities nothing moves it again. When the pad
//! is in a city we have not seen, the Spy runs Listening Posts in a city with
//! no pad for the rest of the race. Across the 13 live losses to a Science
//! Victory of 2026-10-30, the first Disrupt Rocketry came 30 turns after the
//! racer's first project in civvis-20261030T153459Z (Culture lane: four Spies
//! in four Aztec cities from t176, the pad at Tepexpan, found at t217 by a
//! Spy trained at t209), 24 in 162440Z (a Spy in Australia's Bendigo from
//! t175 while Australia landed two projects; it was never disrupted) and 13
//! in 084235Z (Scythia's Solocha from t166, the pad at Pazyryk, found at
//! t179 by a new Spy).
//!
//! Under the gene, once the leader has landed a space project (it owns a
//! pad), a Spy standing in one of the leader's cities where Disrupt Rocketry
//! is not offered, with no pad seen there and none found there before,
//! moves on: to the leader's city with a pad seen or found, else to the city
//! no Spy of ours holds and none has searched in the last
//! [`RACER_SPY_SEARCH_MEMORY_TURNS`], the one searched least recently first
//! (never searched before any), the nearest on a tie, so it cycles through
//! every city before it returns to one. A Listening Post or Gain Sources it is
//! running there does not hold it. A city where Disrupt Rocketry was offered
//! to a Spy of ours is a found pad: a Spy there stays while it is pillaged,
//! and the posts rank it with the pads we have seen. With every city of the
//! leader searched, the Spy stays where it is until the memory lapses.

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

/// `spies-steal-the-lead`: standard turns a city whose host menu offered a
/// Spy of ours no Steal Tech Boost is passed over by the posts.
pub(crate) const STEAL_DRY_MEMORY_TURNS: u32 = 20;

/// Standard turns between two journal notes of slots held by captured agents.
const RACER_SPY_SLOT_NOTE_TURNS: u32 = 10;

/// Standard turns the refusal must run before it is noted: a Spy in transit
/// also leaves the export for a turn or two while it keeps its slot.
const RACER_SPY_SLOT_PERSIST_TURNS: u32 = 3;

/// `racer-spies-search-for-the-pad`: standard turns a racer's city searched
/// without finding a pad is passed over by the search. A pad the racer
/// builds there later is found when the memory lapses.
pub(crate) const RACER_SPY_SEARCH_MEMORY_TURNS: u32 = 10;

/// `racer-spies-search-for-the-pad`: what the search reads, borrowed from
/// the seat. `None` with the gene off.
#[derive(Clone, Copy)]
pub(crate) struct RacerSpySearch<'a> {
    /// The racers' cities where Disrupt Rocketry was offered to a Spy of ours.
    pub(crate) found: &'a BTreeSet<u32>,
    /// The leader's cities a Spy of ours stood in without the offer, and the
    /// turn it last did.
    pub(crate) searched: &'a BTreeMap<u32, u32>,
}

impl AdvancedAi {
    /// The space racers the Spies answer. Empty with the gene off.
    pub(crate) fn racer_spy_racers(&self, g: &Game, pid: usize) -> BTreeSet<usize> {
        if !self.racer_keeps_the_spies {
            return BTreeSet::new();
        }
        self.denial_spy_racers(g, pid)
    }

    /// The cities of `seat` we have not seen: the host's public city count
    /// (`ObservedPublicEmpireStats::city_count`) less the cities on our
    /// board. Zero off the live adapter (no public figure) or when we see
    /// them all. A Spy is assigned only to a city we know, so the pad search
    /// cannot reach these: live Science seat civvis-20261031T095908Z
    /// searched the Inca's four known cities sixteen times from t154 while
    /// its pads stood in three of the eight it had not seen, which first
    /// reached our board at t178 and were all disrupted at t181.
    pub(crate) fn racer_unseen_cities(g: &Game, seat: usize) -> usize {
        g.observed_public_empire_stats
            .get(&seat)
            .and_then(|stats| stats.city_count)
            .map_or(0, |public| {
                public.saturating_sub(g.player_city_ids(seat).len())
            })
    }

    /// The leading space racer the Spies answer and its cities we have not
    /// seen; `None` with `racer-keeps-the-spies` off or no racer. For the
    /// contact-fleet's gate (a scout toward the racer's coast) and the pad
    /// search's journal.
    pub(crate) fn racer_spy_unseen_cities(&self, g: &Game, pid: usize) -> Option<(usize, usize)> {
        let racer = Self::racer_spy_leader(g, &self.racer_spy_racers(g, pid))?;
        Some((racer, Self::racer_unseen_cities(g, racer)))
    }

    /// `racer-spies-search-for-the-pad`: what the search reads; `None` with
    /// the gene off.
    pub(crate) fn racer_spy_search(&self) -> Option<RacerSpySearch<'_>> {
        self.racer_spies_search_for_the_pad
            .then_some(RacerSpySearch {
                found: &self.racer_spy_found_pads,
                searched: &self.racer_spy_searched,
            })
    }

    /// A seat's Production a turn: its cities as the board rebuilds them
    /// plus the host's public figure's difference (`observed_yield_adjustments`),
    /// so the sum is the host's figure.
    fn racer_spy_production(g: &Game, seat: usize) -> f64 {
        // One memo for the whole sweep; see `Game::query_memo`.
        let _memo = g.query_memo();
        g.player_city_ids(seat)
            .into_iter()
            .map(|city| g.city_yields(city).production)
            .sum::<f64>()
            + g.player_yield_extras(seat).production
            + g.arena_side_yields(seat).production
            + g.observed_yield_adjustments
                .get(&seat)
                .map_or(0.0, |adjustment| adjustment.production)
    }

    /// The racer the Spies go to: the most space projects landed, a racer
    /// one project short of the most counting level when its Science a turn
    /// is [`RACER_SCIENCE_EDGE`] times the best of those ahead of it; then,
    /// between two that have both launched the Exoplanet Expedition, the
    /// fewer turns it still needs; then Production a turn (the projects are
    /// built with it), Science a turn, technologies and the lowest seat.
    pub(crate) fn racer_spy_leader(g: &Game, racers: &BTreeSet<usize>) -> Option<usize> {
        let stages = |rival: usize| Self::science_denial_stages(g, rival);
        let science = |rival: usize| Self::seat_science_per_turn(g, rival);
        let production = |rival: usize| Self::racer_spy_production(g, rival);
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
                .then_with(|| production(*left).total_cmp(&production(*right)))
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
    /// culture racer's, nor in another racer's city where this seat can run
    /// the Disrupt Rocketry offered there ([`Self::racer_spy_disrupt_runnable`],
    /// with every racer's pad re-filable, as live arms `science-denial-every-pad`);
    /// otherwise the leader's city no other Spy of ours holds,
    /// ranked by a seen standing Spaceport within three tiles, then the
    /// capital, then nearness to the capital. Only cities the host lets us
    /// post to (`legal`) count.
    ///
    /// With `search` (`racer-spies-search-for-the-pad`), a found pad ranks as
    /// a seen one, and a Spy searching the leader's city (see
    /// [`Self::racer_spy_searching`]) moves to a pad seen or found, else to
    /// the leader's city not searched lately that was searched least recently
    /// (never first), the nearest on a tie; none left, it stays.
    ///
    /// Only the tests call this wrapper; the diplomatic turn calls
    /// [`Self::racer_spy_post_for`] directly.
    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn racer_spy_post(
        g: &Game,
        spy: u32,
        current_city: Option<u32>,
        leader: Option<usize>,
        racers: &BTreeSet<usize>,
        culture_racers: &BTreeSet<usize>,
        legal: &[Action],
        search: Option<RacerSpySearch<'_>>,
    ) -> Option<u32> {
        Self::racer_spy_post_for(
            g,
            spy,
            current_city,
            leader,
            racers,
            culture_racers,
            legal,
            search,
            false,
            &BTreeMap::new(),
            racers,
        )
    }

    /// `racer_spy_post`, and under `spies-steal-the-lead`
    /// (`prefer_campus`, a Science aim) outside a pad search: the leader's
    /// cities with a Campus we have seen rank right after a seen pad, and a
    /// Spy standing in a leader city with no Campus and no pad, where Disrupt
    /// Rocketry is not offered, moves to a free one with a Campus. Steal Tech
    /// Boost needs a Campus: live 072215Z posted a Spy to Canada's Halifax (its
    /// capital, no Campus) at turn 152, where the host offered only Foment
    /// Unrest and Listening Post, and it ran Gain Sources and Listening Posts
    /// to the end of the game.
    ///
    /// `dry` (the Science aim's `steal_dry_cities`) names the cities where
    /// the host's menu offered a Spy of ours no steal, within
    /// [`STEAL_DRY_MEMORY_TURNS`]. A Spy standing in a dry leader city also
    /// moves when no Campus of the leader has been seen: to the most
    /// populous leader city not dry. Live 080206Z: a Spy stood in Zulu cities
    /// whose host menu held no Steal Tech Boost from turn 127 to 147 while no
    /// Zulu Campus was known to the board. Under the gene the post's order
    /// after a seen pad is: a seen Campus, a city not dry, the most populous.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn racer_spy_post_for(
        g: &Game,
        spy: u32,
        current_city: Option<u32>,
        leader: Option<usize>,
        racers: &BTreeSet<usize>,
        culture_racers: &BTreeSet<usize>,
        legal: &[Action],
        search: Option<RacerSpySearch<'_>>,
        prefer_campus: bool,
        dry_cities: &BTreeMap<u32, u32>,
        refile: &BTreeSet<usize>,
    ) -> Option<u32> {
        let leader = leader?;
        let pid = g.spies.get(&spy)?.owner;
        let here = current_city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner);
        let searching = search.is_some_and(|search| {
            Self::racer_spy_searching(g, spy, current_city, Some(leader), search, legal)
        });
        // `spies-steal-the-lead`: a Spy in a leader city where it can neither
        // steal nor disrupt leaves for a free leader city with a seen Campus.
        let dry = |cid: u32| {
            prefer_campus
                && dry_cities
                    .get(&cid)
                    .is_some_and(|seen| g.turn < seen + g.standard_duration(STEAL_DRY_MEMORY_TURNS))
        };
        let campus_move = prefer_campus
            && !searching
            && here == Some(leader)
            && current_city.is_some_and(|city| {
                (dry(city) || !Self::racer_spy_seen_campus(g, pid, city))
                    && !Self::racer_spy_seen_pad(g, pid, city)
            })
            && !Self::racer_spy_disrupt_offered(g, spy, legal);
        let standing_dry = campus_move && current_city.is_some_and(dry);
        if (here == Some(leader) && !searching && !campus_move)
            || here.is_some_and(|owner| culture_racers.contains(&owner))
            || (here.is_some_and(|owner| racers.contains(&owner))
                && Self::racer_spy_disrupt_runnable(g, spy, legal, refile))
        {
            return None;
        }
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
            Self::racer_spy_seen_pad(g, pid, cid)
                || search.is_some_and(|search| search.found.contains(&cid))
        };
        let from = current_city
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.pos);
        let fresh =
            |cid: u32| search.is_none_or(|search| !Self::racer_spy_searched_lately(g, search, cid));
        let assignable = |cid: u32| {
            legal
                .iter()
                .any(|action| matches!(action, Action::AssignSpy { city, .. } if *city == cid))
        };
        let campus = |cid: u32| Self::racer_spy_seen_campus(g, pid, cid) && !dry(cid);
        let campus_free = cities.iter().any(|cid| {
            Some(*cid) != current_city && !held(*cid) && campus(*cid) && assignable(*cid)
        });
        cities
            .into_iter()
            .filter(|cid| {
                !held(*cid)
                    && Some(*cid) != current_city
                    && (!searching || seen_pad(*cid) || fresh(*cid))
                    && (!campus_move
                        || campus(*cid)
                        || (standing_dry && !campus_free && !dry(*cid)))
                    && legal.iter().any(
                        |action| matches!(action, Action::AssignSpy { city, .. } if city == cid),
                    )
            })
            .min_by_key(|cid| {
                let city = &g.cities[cid];
                // A search goes to the city it searched least recently (one
                // never searched first), the nearest on a tie; a post from
                // outside goes to the capital and the cities nearest it.
                // Live 011801Z: "nearest not searched lately" re-picked Rome's
                // Hispalis and Antium (t179, t189) once the memory lapsed,
                // while the pad stood in Rome itself (first Disrupt t203).
                let (searched_at, capital_first, distance) = if searching {
                    (
                        search.and_then(|search| search.searched.get(cid).copied()),
                        false,
                        from.map_or(0, |from| g.wdist(from, city.pos)),
                    )
                } else {
                    (
                        None,
                        city.is_capital && !prefer_campus,
                        capital.map_or(0, |capital| g.wdist(capital, city.pos)),
                    )
                };
                let stealing = prefer_campus && !searching;
                (
                    std::cmp::Reverse(seen_pad(*cid)),
                    // `spies-steal-the-lead`: a Campus to steal from next,
                    // then a city the host has not shown dry, then the
                    // most populous.
                    std::cmp::Reverse(stealing && campus(*cid)),
                    std::cmp::Reverse(stealing && !dry(*cid)),
                    searched_at,
                    std::cmp::Reverse(capital_first),
                    std::cmp::Reverse(if stealing { city.pop } else { 0 }),
                    distance,
                    *cid,
                )
            })
    }

    /// Whether seat `pid` has seen `cid`'s Campus (the district on a tile it
    /// has explored): where Steal Tech Boost can run.
    pub(crate) fn racer_spy_seen_campus(g: &Game, pid: usize, cid: u32) -> bool {
        g.cities.get(&cid).is_some_and(|city| {
            city.districts.iter().any(|(district, tile)| {
                g.district_family(*district) == "campus" && g.players[pid].explored.contains(tile)
            })
        })
    }

    /// Whether seat `pid` has seen a standing Spaceport within three tiles of
    /// `cid`.
    fn racer_spy_seen_pad(g: &Game, pid: usize, cid: u32) -> bool {
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

    /// `racer-spies-search-for-the-pad`: whether a Spy of ours searched `cid`
    /// within the last [`RACER_SPY_SEARCH_MEMORY_TURNS`].
    fn racer_spy_searched_lately(g: &Game, search: RacerSpySearch<'_>, cid: u32) -> bool {
        search
            .searched
            .get(&cid)
            .is_some_and(|turn| g.turn < turn + g.standard_duration(RACER_SPY_SEARCH_MEMORY_TURNS))
    }

    /// `racer-spies-search-for-the-pad`: whether `spy`, standing at `city`,
    /// is searching for the leader's pad: the city is the leader's, the
    /// leader has landed a space project, Disrupt Rocketry is not offered
    /// there, and no pad is seen or was found there.
    pub(crate) fn racer_spy_searching(
        g: &Game,
        spy: u32,
        city: Option<u32>,
        leader: Option<usize>,
        search: RacerSpySearch<'_>,
        legal: &[Action],
    ) -> bool {
        let (Some(leader), Some(city)) = (leader, city) else {
            return false;
        };
        let Some(pid) = g.spies.get(&spy).map(|agent| agent.owner) else {
            return false;
        };
        g.cities.get(&city).is_some_and(|here| here.owner == leader)
            && Self::science_denial_stages(g, leader) >= 1
            && !search.found.contains(&city)
            && !Self::racer_spy_seen_pad(g, pid, city)
            && !Self::racer_spy_disrupt_offered(g, spy, legal)
    }

    /// `racer-spies-search-for-the-pad`: whether the leader has a city other
    /// than `city` that no Spy of ours holds and none searched lately.
    fn racer_spy_search_left(
        g: &Game,
        spy: u32,
        city: u32,
        leader: usize,
        search: RacerSpySearch<'_>,
    ) -> bool {
        let Some(pid) = g.spies.get(&spy).map(|agent| agent.owner) else {
            return false;
        };
        g.player_city_ids(leader).into_iter().any(|cid| {
            cid != city
                && !g.spies.values().any(|other| {
                    other.owner == pid
                        && other.id != spy
                        && other.captured_by.is_none()
                        && other.city == Some(cid)
                })
                && (search.found.contains(&cid) || !Self::racer_spy_searched_lately(g, search, cid))
        })
    }

    /// `racer-spies-search-for-the-pad`: what `spy` learned where it stands
    /// this turn: a racer's city that offers Disrupt Rocketry is a found pad,
    /// and a leader's city that does not, once the leader has landed a
    /// project, is searched.
    pub(crate) fn racer_spy_note_the_search(
        &mut self,
        g: &Game,
        spy: u32,
        leader: Option<usize>,
        racers: &BTreeSet<usize>,
        legal: &[Action],
    ) {
        if !self.racer_spies_search_for_the_pad {
            return;
        }
        let Some(city) = g.spies.get(&spy).and_then(|agent| agent.city) else {
            return;
        };
        let Some(owner) = g.cities.get(&city).map(|here| here.owner) else {
            return;
        };
        if !racers.contains(&owner) {
            return;
        }
        if Self::racer_spy_disrupt_offered(g, spy, legal) {
            self.racer_spy_found_pads.insert(city);
        } else if Some(owner) == leader && Self::science_denial_stages(g, owner) >= 1 {
            self.racer_spy_searched.insert(city, g.turn);
        }
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

    /// Whether this seat can run the Disrupt Rocketry offered to `spy` where
    /// it stands: the board lists it (`legal`), or the host's menu offers it
    /// and `science-denial-every-pad` (`refile`, its threats; empty with that
    /// gene off) can file a standing pad of the city's owner under this city
    /// (`every_pad_refile_pad`). A host offer the board cannot file is not
    /// runnable: its only pad within reach reads pillaged on our board, the
    /// sighting a fogged tile keeps after the owner repaired it.
    pub(crate) fn racer_spy_disrupt_runnable(
        g: &Game,
        spy: u32,
        legal: &[Action],
        refile: &BTreeSet<usize>,
    ) -> bool {
        if legal.iter().any(|action| {
            matches!(action, Action::SpyMission { mission, .. } if mission == "disrupt_rocketry")
        }) {
            return true;
        }
        let host_offers = g
            .host_unit_facts
            .get(&spy)
            .and_then(|facts| facts.spy_missions.as_ref())
            .is_some_and(|menu| menu.contains("disrupt_rocketry"));
        host_offers
            && g.spies
                .get(&spy)
                .and_then(|agent| agent.city)
                .and_then(|city| g.cities.get(&city))
                .is_some_and(|city| {
                    refile.contains(&city.owner)
                        && Self::every_pad_refile_pad(g, city.owner, city.pos).is_some()
                })
    }

    /// Whether `spy`, whose last order is still running, should move on
    /// anyway: it stands in a racer's city that is not the leader's and is
    /// running only a Listening Post or Gain Sources there (a Disrupt
    /// Rocketry, a journey or any other operation runs to its end). Never
    /// with the gene off (`leader` is `None`). With `search`
    /// (`racer-spies-search-for-the-pad`), the same in the leader's own city
    /// while the Spy is searching it and a city is left to search.
    pub(crate) fn racer_spy_moves_on(
        g: &Game,
        spy: u32,
        leader: Option<usize>,
        racers: &BTreeSet<usize>,
        search: Option<RacerSpySearch<'_>>,
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
        // `racer-spies-search-for-the-pad`: in the leader's city, only a
        // search with a city left to search moves it on.
        let searching = owner == Some(leader)
            && agent.city.is_some_and(|city| {
                search.is_some_and(|search| {
                    Self::racer_spy_searching(g, spy, Some(city), Some(leader), search, &[])
                        && Self::racer_spy_search_left(g, spy, city, leader, search)
                })
            });
        if (owner == Some(leader) && !searching)
            || !owner.is_some_and(|owner| racers.contains(&owner))
        {
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
