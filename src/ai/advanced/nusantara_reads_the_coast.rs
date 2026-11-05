//! `nusantara-reads-the-coast` (opt-in): an Indonesian seat's district site
//! pickers price Great Nusantara, the Coast adjacency the engine's flat
//! placeholder never models.
//!
//! **The rule (shipped data).** `Indonesia_Khmer_GameplayData.xml`,
//! `TRAIT_CIVILIZATION_INDONESIA_NUSANTARA`: four
//! `MODIFIER_PLAYER_CITIES_TERRAIN_ADJACENCY` modifiers, each `TerrainType`
//! `TERRAIN_COAST`, `Amount` 1, `TilesRequired` 2: a Campus gains +1 Science,
//! a Holy Site +1 Faith, an Industrial Zone +1 Production and a Theater
//! Square +1 Culture for every two adjacent Coast tiles (a Lake is Coast
//! terrain in the shipped map). `data/civs.json` carries the ability as a
//! placeholder, +1 Faith and +1 Food in every city, so the model's
//! `district_yields` never sees the Coast.
//!
//! **Why it matters live.** The operator moved the live seat to Indonesia
//! (Gitarja), Science only, on 2026-11-05. The host hands the seat only the
//! legal plots for a district; which plot to take is the model's ranking,
//! and every picker ranks by `district_yields`. A Campus beside four Coast
//! tiles is worth +2 Science the planner cannot see.
//!
//! **Scope.** Ranking only: [`Game::district_site_yields`] is what the AI's
//! site pickers read; real city yields still come from `district_yields`, so
//! the rules, the score and every yield report are unchanged. The seat sets
//! `Game::nusantara_site_planner` at the start of its own turn; with the
//! gene off, or for any other civilization, it stays `None` and nothing
//! changes.

use super::*;

/// Adjacent Coast tiles per point of Great Nusantara adjacency
/// (`TilesRequired` 2 in the shipped modifiers).
pub(crate) const NUSANTARA_TILES_REQUIRED: f64 = 2.0;

impl Game {
    /// Great Nusantara's Coast adjacency for `district` at `pos`, when the
    /// plot's owner is this turn's Nusantara planner: +1 of the district's
    /// yield per two adjacent Coast or Lake tiles, for a Campus (Science),
    /// Holy Site (Faith), Industrial Zone (Production) or Theater Square
    /// (Culture). Zero otherwise.
    pub(crate) fn nusantara_site_term(&self, district: Name, pos: Pos) -> Yields {
        let mut term = Yields::default();
        let Some(planner) = self.nusantara_site_planner else {
            return term;
        };
        if self
            .players
            .get(planner)
            .is_none_or(|player| player.civ != "Indonesia")
        {
            return term;
        }
        let owner = self
            .map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|cid| self.cities.get(&cid))
            .map(|city| city.owner);
        if owner != Some(planner) {
            return term;
        }
        let coast = self
            .nbrs(pos)
            .filter(|neighbor| {
                self.map.get(*neighbor).is_some_and(|tile| {
                    tile.terrain == crate::name!("coast") || tile.terrain == crate::name!("lake")
                })
            })
            .count() as f64;
        let points = (coast / NUSANTARA_TILES_REQUIRED).floor();
        if points <= 0.0 {
            return term;
        }
        match district.as_str() {
            "campus" => term.science = points,
            "holy_site" => term.faith = points,
            "industrial_zone" => term.production = points,
            "theater_square" => term.culture = points,
            _ => {}
        }
        term
    }

    /// The yields a district site picker ranks by: [`Game::district_yields`]
    /// plus [`Game::nusantara_site_term`]. Identical to `district_yields`
    /// unless `nusantara-reads-the-coast` named this turn's planner.
    pub(crate) fn district_site_yields(
        &self,
        district: impl crate::name::AsName,
        pos: Pos,
    ) -> Yields {
        let district = district.as_name();
        let mut yields = self.district_yields(district, pos);
        yields.add(self.nusantara_site_term(district, pos));
        yields
    }
}

impl AdvancedAi {
    /// Name this seat as the turn's Nusantara planner when the gene is on and
    /// it plays Indonesia; clear the slot otherwise.
    pub(super) fn seat_nusantara_site_planner(&self, g: &mut Game, pid: usize) {
        g.nusantara_site_planner =
            (self.nusantara_reads_the_coast && g.players[pid].civ == "Indonesia").then_some(pid);
    }
}

#[cfg(test)]
mod tests;
