use super::*;

/// Native Domination counts original capitals and their founders. The Palace
/// can move without changing either fact (WorldRankings.lua:1842-1848).
pub(super) fn apply(game: &mut crate::game::Game, state: &StateSnapshot) {
    Arc::make_mut(&mut game.observed_city_palaces).clear();
    let seats = host_capture_seats(game, state);
    let cities = || {
        state
            .cities
            .iter()
            .chain(state.rivals.iter().flat_map(|rival| rival.cities.iter()))
            .chain(state.minors.iter().flat_map(|minor| minor.cities.iter()))
    };
    // Preserve the legacy interpretation only for snapshots that lack the new
    // fact. Old exports with no flagged capital retain the reconstruction.
    let flagged_owners: BTreeSet<_> = cities()
        .filter(|city| city.capital)
        .filter_map(|city| game.city_at(crate::hex::offset_to_axial(city.x, city.y)))
        .map(|cid| game.cities[&cid].owner)
        .collect();
    // ★★ AN ORIGINAL CAPITAL WHOSE FOUNDER HAS NO SEAT IS NOBODY'S CAPITAL HERE.
    // A city-state that lost its capital is gone from the board, so its host id
    // maps to no seat, and the board's default left `original_owner` at the
    // city's CURRENT owner. Live King civvis-20261004T160213Z (game 65): Fez, a
    // dead city-state's capital Germany had taken, read as Germany's original
    // capital beside Aachen, and Mohenjo-Daro and Nan Madol as ours. Every
    // "this rival's original capital" lookup (`check_domination`'s `find`, the
    // finishing-capital and defer-capture reads) then took whichever city came
    // first in an id order that churns every rebuild. Credit it to the
    // barbarian seat instead: still an original capital (no Raze, no Liberate),
    // never a major's Domination objective.
    let unmodelled_founder = game
        .barb_pid
        .or_else(|| game.players.iter().position(|player| player.is_free_city));
    for observed in cities() {
        let Some(cid) = game.city_at(crate::hex::offset_to_axial(observed.x, observed.y)) else {
            continue;
        };
        let founder = observed
            .original_owner
            .and_then(|host| seats.get(&host))
            .copied();
        let city = game.cities.get_mut(&cid).unwrap();
        if let Some(founder) = founder {
            city.original_owner = founder;
        } else if observed.original_capital == Some(true) && observed.original_owner.is_some() {
            if let Some(nobody) = unmodelled_founder {
                city.original_owner = nobody;
            }
        }
        if let Some(original) = observed.original_capital {
            // The original-capital flag is independently authoritative (and
            // prohibits razing) even if the founder cannot be mapped. Retain
            // the existing founder association rather than guessing a seat.
            city.is_capital = original;
            Arc::make_mut(&mut game.observed_city_palaces).insert(cid, observed.capital);
        } else if flagged_owners.contains(&city.owner) {
            city.is_capital = observed.capital;
        }
    }
}

#[cfg(test)]
mod tests;
