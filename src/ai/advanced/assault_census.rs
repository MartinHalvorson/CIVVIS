//! The assault census: what a named corps reads against every foreign city in
//! reach, for `civvis_orders --explain-assault`.
//!
//! A read-only caller of the assault forecast
//! (`advanced/assault_forecast.rs`). It asks the at-peace reading for each
//! charted city of a living major or city-state within `reach` tiles of one
//! of our cities, twice: with the land soldiers the seat holds now, and with
//! a named set of recruits added (units it may not own yet, such as a
//! civilization's unique soldier before its technology). It decides nothing.
//!
//! Why: the live Deity seat declared 193 wars on majors in about 113 games
//! of 2026-11-12/13 and captured 10 cities, none as Rome (session -13's
//! census). Before a campaign is built for the Score seat, the forecast can
//! say on recorded turns whether any corps the seat could afford would have
//! taken a neighbour's city, and when.

use super::assault_forecast::Recruit;
use super::*;

/// One city's row of the census.
#[derive(Clone, Debug)]
pub struct AssaultCensusRow {
    pub city: String,
    pub owner: String,
    pub city_state: bool,
    pub capital: bool,
    pub pop: i32,
    /// Tiles from the nearest of our cities.
    pub distance: i32,
    pub strength: f64,
    pub walls: i32,
    pub max_walls: i32,
    /// The owner's units the forecast counted as hostiles.
    pub hostiles: usize,
    /// Raw P(capture) within 4, 8 and 12 turns with the army we hold.
    pub held: [f32; 3],
    /// The same with the recruits added.
    pub recruited: [f32; 3],
    /// The recruited reading's calibrated chance within 8 turns.
    pub calibrated: f32,
    /// Own units the recruited reading expects to lose.
    pub units_lost: f32,
    /// What the recruited corps is still short of.
    pub short_of: String,
}

impl AdvancedAi {
    /// See the module. `recruits` are `(unit kind, count, turns until they
    /// reach the board's edge nearest our cities)`; an unknown kind is
    /// skipped. Rows are sorted by the recruited reading, best first.
    pub fn assault_census(
        &self,
        g: &Game,
        pid: usize,
        reach: i32,
        recruits: &[(String, u32, u32)],
    ) -> Vec<AssaultCensusRow> {
        let ours: Vec<Pos> = g
            .player_city_ids(pid)
            .into_iter()
            .map(|cid| g.cities[&cid].pos)
            .collect();
        let force: Vec<u32> = g
            .player_unit_ids(pid)
            .into_iter()
            .filter(|uid| Self::holdable_land_soldier(g, &g.units[uid]))
            .collect();
        let recruits: Vec<Recruit> = recruits
            .iter()
            .filter(|(kind, count, _)| *count > 0 && g.rules.units.contains_key(&Name::new(kind)))
            .map(|(kind, count, turns)| Recruit {
                kind: Name::new(kind),
                count: *count,
                turns: *turns,
            })
            .collect();
        let mut targets: Vec<(u32, i32)> = g
            .cities
            .values()
            .filter(|city| {
                city.owner != pid
                    && g.players
                        .get(city.owner)
                        .is_some_and(|owner| owner.alive && !owner.is_barbarian)
            })
            .filter_map(|city| {
                let distance = ours.iter().map(|pos| g.wdist(*pos, city.pos)).min()?;
                (distance <= reach).then_some((city.id, distance))
            })
            .collect();
        targets.sort_by_key(|(cid, distance)| (*distance, *cid));
        let marks = |forecast: &assault_forecast::AssaultForecast| {
            [forecast.within(4), forecast.within(8), forecast.within(12)]
        };
        let mut rows: Vec<AssaultCensusRow> = targets
            .into_iter()
            .filter_map(|(cid, distance)| {
                let city = &g.cities[&cid];
                let held = self.assault_forecast_at_peace(g, pid, cid, &force)?;
                let recruited = self.assault_forecast_recruiting(g, pid, cid, &force, &recruits)?;
                let owner = &g.players[city.owner];
                Some(AssaultCensusRow {
                    city: city.name.clone(),
                    owner: owner.civ.clone(),
                    city_state: owner.is_minor,
                    capital: city.is_capital,
                    pop: city.pop,
                    distance,
                    strength: g.city_strength(cid),
                    walls: city.wall_hp,
                    max_walls: g.city_max_wall_hp(city),
                    hostiles: recruited.hostiles,
                    held: marks(&held),
                    recruited: marks(&recruited),
                    calibrated: recruited.calibrated_within(8, g.city_max_wall_hp(city)),
                    units_lost: recruited.units_lost,
                    short_of: recruited
                        .short_of
                        .map_or(String::from("-"), |short| format!("{short:?}")),
                })
            })
            .collect();
        rows.sort_by(|a, b| {
            b.recruited[1]
                .total_cmp(&a.recruited[1])
                .then(a.distance.cmp(&b.distance))
                .then(a.city.cmp(&b.city))
        });
        rows
    }
}
