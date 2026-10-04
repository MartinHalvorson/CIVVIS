//! A Spy's posting follows the host purchase city, not its operation tile.
//!
//! The shipped UnitPanel.lua:2151-2155 and EspionageOverview.lua:644-647
//! read Cities.GetPlotPurchaseCity(Map.GetPlot(spy:GetX(), spy:GetY())).
//! A running district mission moves the native unit off the city center.
//! Losing that city also loses the controller's one-spy-per-pad accounting.

use super::{host_major_seat_map, modeled_major_player_count, StateSnapshot};
use crate::{game::Game, hex::offset_to_axial, Pos};

/// Only the host city identity/position of our own Spy's observed plot.
/// Native city IDs are player-local, so owner and center must cross together.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StateSpyCity {
    pub id: i64,
    pub player: usize,
    pub x: i32,
    pub y: i32,
}

pub(super) fn city_of(game: &Game, state: &StateSnapshot, uid: u32, pos: Pos) -> Option<u32> {
    let observed = game
        .host_unit_facts
        .get(&uid)
        .and_then(|facts| facts.civ6_id)
        .and_then(|native| state.units.iter().find(|unit| unit.id == native))
        .and_then(|unit| unit.spy_city.as_ref());
    let Some(observed) = observed else {
        // Older archives retain their exact-center behavior. Never infer a
        // foreign purchase city from a nearest city or estimated plot owner.
        return game.city_at(pos);
    };
    let local = usize::try_from(state.seat.local_player).ok()?;
    let (cities, minor) = if observed.player == local {
        (&state.cities, false)
    } else if let Some(rival) = state
        .rivals
        .iter()
        .find(|rival| rival.player == observed.player)
    {
        (&rival.cities, false)
    } else if let Some(minor) = state
        .minors
        .iter()
        .find(|minor| minor.player == observed.player)
    {
        (&minor.cities, true)
    } else {
        return None;
    };
    let known = cities
        .iter()
        .find(|city| city.id == observed.id && city.x == observed.x && city.y == observed.y)?;
    let cid = game.city_at(offset_to_axial(known.x, known.y))?;
    let owner = game.cities.get(&cid)?.owner;
    if minor {
        game.players.get(owner)?.is_minor.then_some(cid)
    } else {
        (host_major_seat_map(state, modeled_major_player_count(game)).get(&observed.player)
            == Some(&owner))
        .then_some(cid)
    }
}

#[cfg(test)]
mod tests;
