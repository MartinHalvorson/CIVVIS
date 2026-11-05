//! `early-cities-keep-their-distance`: the second and third cities are
//! sited away from the rival cities the seat has already charted.
//!
//! **Why (operator rule 2026-11-04 ~21:15Z, PR #4435).** The live lane now
//! restarts a game once the capital or either of the next two cities it
//! founded is lost (`CIVVIS_RESTART_ON_EARLY_CITY_LOST=3`). A replay of 89
//! recent runs found the rule would fire in 52, and it retired 3 of the first
//! 8 Deity games under it. Losing city two or three now costs the whole game,
//! not one city.
//!
//! Census of 173 second and third cities (live Lv 7 Immortal and Lv 8 Deity,
//! 2026-11-03 21Z to 11-04 23Z): the share lost, among cities lost or kept to
//! turn 80, by distance from the nearest rival city the seat had charted when
//! it founded them: 5-6 tiles 56%, 7-8 40%, 9-11 23%, 12 or more 8%. By the
//! rival's border, 3-4 tiles out lost 45%, 7 or more 19%. The site value
//! already prices a border inside `FOREIGN_BORDER_RADIUS` (three tiles), so
//! nothing priced a rival city six or seven tiles out. Deity example:
//! `civvis-20261104T210549Z` founded Yangsan at turn 19, five tiles from a
//! Sumerian city charted at turn 16; Sumer took it at turn 27.
//!
//! **What it does.** While the empire holds fewer than
//! [`EARLY_CITY_COUNT`] cities, a site loses [`EARLY_CITY_TILE_PENALTY`] for
//! every tile it stands inside [`EARLY_CITY_RIVAL_BUFFER`] of the nearest
//! charted city of a living rival major. A site 5 tiles out loses 40, one 9
//! tiles out loses 8. The term reads only cities on ground the seat has
//! explored, so fog stays honest. From the third city on, and with the gene
//! off, it is zero.

use super::*;

/// The cities the restart rule protects: the capital and the next two.
pub(crate) const EARLY_CITY_COUNT: usize = 3;

/// A charted rival city this many tiles or more from the site costs nothing.
/// The census's lost share falls to 8% from twelve tiles and 23% at nine to
/// eleven.
pub(crate) const EARLY_CITY_RIVAL_BUFFER: i32 = 10;

/// Settle value lost per tile inside the buffer. Live sites are worth about
/// 100-150, so a site five tiles from a rival city gives up 40.
pub(crate) const EARLY_CITY_TILE_PENALTY: f64 = 8.0;

impl AdvancedAi {
    /// The settle-value penalty for founding one of the protected early
    /// cities at `pos`. Zero with the gene off, from the third city on, or
    /// with no charted rival city inside the buffer.
    pub(super) fn early_city_rival_penalty(&self, g: &Game, pid: usize, pos: Pos) -> f64 {
        if !self.early_cities_keep_their_distance
            || g.player_city_ids(pid).len() >= EARLY_CITY_COUNT
        {
            return 0.0;
        }
        let Some(seat) = g.players.get(pid) else {
            return 0.0;
        };
        let nearest = g
            .cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && seat.explored.contains(&city.pos)
                    && g.players
                        .get(city.owner)
                        .is_some_and(|owner| owner.alive && !owner.is_minor && !owner.is_barbarian)
            })
            .map(|city| g.wdist(city.pos, pos))
            .min();
        nearest.map_or(0.0, |distance| {
            f64::from((EARLY_CITY_RIVAL_BUFFER - distance).max(0)) * EARLY_CITY_TILE_PENALTY
        })
    }
}

#[cfg(test)]
mod tests;
