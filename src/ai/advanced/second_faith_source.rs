//! `founder-keeps-two-sources`: a founder keeps two cities that follow its
//! faith and hold a Shrine, from the moment it founds.
//!
//! A founder buys its own Missionaries, Apostles and Inquisitors only in a
//! city that follows its faith and holds a Shrine: the host sells a religious
//! unit of the purchase city's majority and none at all in a city without one.
//! The sanctuary (`adopted_faith_sanctuary_choice`) stopped at the first such
//! city, the Holy City, and waited for a conversion threat to start even that
//! one. So when the Holy City flipped, the defence ended that turn. In the
//! Emperor Religious defeats of October 6 we founded in five of six (G186,
//! G195, G212, G213, G215); four had one Shrine city until the end, and the
//! defence ended with it: G186 lost Bogotá 9 turns after founding, G212 11
//! turns after, G215 47 turns after (then banked 610 Faith it could not
//! spend). G213, the one that held (the last holdout from turn 100 to 149,
//! the Inquisition launched at 64), had a Holy Site in Maracaibo from 54 that
//! never got its Shrine; its second source, Popayán, came at 114.
//!
//! Under the gene a founder whose world holds another religion builds a second
//! source as soon as it founds, threat or not: the city that follows our faith
//! and finishes a Holy Site and Shrine (or a Shrine on an existing Holy Site)
//! soonest. The sanctuary stops at two sources. Purchases already look at
//! every city that follows our faith (`religious_spending_with_reserve`,
//! `prepare_defensive_inquisition`), so the second source is used without
//! further change. Off: unchanged.

use super::*;

impl AdvancedAi {
    /// Our cities that follow our founded faith and hold a working Shrine:
    /// where our own religious units are bought. Zero for a seat with no
    /// religion.
    pub(super) fn own_faith_sources(g: &Game, pid: usize) -> usize {
        let Some(own) = g.players[pid].religion.as_deref() else {
            return 0;
        };
        g.player_city_ids(pid)
            .into_iter()
            .filter(|cid| {
                let city = &g.cities[cid];
                g.city_religion(city) == Some(own)
                    && city.buildings.iter().any(|building| {
                        !city.pillaged_buildings.contains(building)
                            && g.building_is_family(building, crate::name!("shrine"))
                    })
            })
            .count()
    }

    /// Whether the founder's sanctuary wants another source: the gene is on,
    /// we founded a religion, another religion exists, and fewer than two of
    /// our cities are sources ([`Self::own_faith_sources`]).
    pub(super) fn founder_wants_a_second_source(&self, g: &Game, pid: usize) -> bool {
        self.founder_keeps_two_sources
            && g.players[pid].religion.is_some()
            && g.religions_founded() > 1
            && Self::own_faith_sources(g, pid) < 2
    }
}

#[cfg(test)]
mod tests;
