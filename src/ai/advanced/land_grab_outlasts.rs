//! `land-grab-outlasts-the-opening`: under `rapid-city-expansion-2` the city
//! target keeps the land grab's land-aware horizon instead of stopping at the
//! shared floor of ten.
//!
//! The strategic plan's city target picks ONE of three sources:
//! `rapid-city-expansion-2`'s opening target, else `land_grab`'s land-aware
//! capacity (2 + one city per [`LAND_GRAB_TILES_PER_CITY`] tiles, clamped to
//! [`LAND_GRAB_CITY_FLOOR`]..[`LAND_GRAB_CITY_CEILING`]), else the ordinary
//! era cadence. `rapid-city-expansion-2` says the ordinary land-aware target
//! "remains authoritative" and only lifts the opening to the measured
//! five-city pace. But because it is asked FIRST, a seat with both on (every
//! live seat: `land-grab` is HostOnly, `rapid-city-expansion-2` is armed)
//! never reads the land grab's capacity. Its target is the ordinary cadence
//! (six plus one city an era, capped by `wide-map-capacity`'s twelve), and
//! the shared contract floors that at ten.
//!
//! Live Immortal 6p Continents, 2026-10-30/31, 38 seats past turn 110:
//! - 1,223 of 1,894 "N cities of M wanted" readings say M = 10.
//! - The median seat holds 8 cities at t120, 10 at t150 and 10 at t180.
//! - The rival with the most production holds 9, 11 and 12; the widest holds 9, 11 and 13.
//! - Our production share plateaus at 0.46-0.48.
//! - Three of the four losses past the t80 gate were rival space races.
//!
//! With the gene on and both expansion genes on, the target is the larger of
//! the two answers. `city_target_meets_the_map`'s practical-site room, the
//! shared contract's ceiling of fifteen, the Settler pipeline and the payback
//! window still pace the count, exactly as they do for the land grab alone.
//!
//! Off, or with either expansion gene off: unchanged.
use super::*;

impl AdvancedAi {
    /// The land grab's city target for `land` known passable tiles: the
    /// fog-estimated capacity at one city per [`LAND_GRAB_TILES_PER_CITY`]
    /// tiles, wider on the Science lane while its cities can still mature.
    pub(super) fn land_grab_city_target(
        &self,
        g: &Game,
        land: usize,
        science_targeted: bool,
    ) -> usize {
        let land = if self.fog_land_capacity {
            Self::fog_land_estimate(g, land)
        } else {
            land
        };
        // The science lane widens while a founded city can still mature
        // into the super-linear pop term; see
        // `SCIENCE_EXPANSION_CITY_CEILING`. Every other lane, and the
        // science lane's second half, keep the shared ceiling.
        let ceiling = if self.science_expansion_phase
            && science_targeted
            && g.turn < g.standard_duration(SCIENCE_EXPANSION_UNTIL_TURN)
        {
            SCIENCE_EXPANSION_CITY_CEILING
        } else {
            LAND_GRAB_CITY_CEILING
        };
        (2 + land / LAND_GRAB_TILES_PER_CITY).clamp(LAND_GRAB_CITY_FLOOR, ceiling)
    }

    /// `rapid-city-expansion-2`'s target, widened to the land grab's under
    /// this gene.
    pub(super) fn rapid_city_target(
        &self,
        g: &Game,
        rapid: usize,
        land: usize,
        science_targeted: bool,
    ) -> usize {
        if self.land_grab_outlasts_the_opening && self.land_grab {
            rapid.max(self.land_grab_city_target(g, land, science_targeted))
        } else {
            rapid
        }
    }
}

#[cfg(test)]
mod tests;
