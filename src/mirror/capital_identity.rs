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
