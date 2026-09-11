//! Soft rival-search priors from the scout's charted terrain only.
use super::*;

impl BasicAi {
    /// A bounded bias, not a claim that a rival exists at the destination.
    /// Information gain, route safety and explorer separation still matter.
    pub(super) fn rival_frontier_prior(
        g: &Game,
        pid: usize,
        uid: u32,
        target: Pos,
        home: Option<Pos>,
    ) -> i32 {
        if g.rules.units[g.units[&uid].kind].domain.as_deref() == Some("sea") {
            return 0;
        }
        let explored = &g.players[pid].explored;
        let mut evidence = 0;
        let mut weight = 0;
        // Nearby charted land suggests the continent continues; coast/ocean
        // suggests its boundary. Never inspect the candidate's hidden terrain.
        for pos in g.wdisk(target, 4) {
            if !explored.contains(&pos) {
                continue;
            }
            let Some(tile) = g.map.get(pos) else { continue };
            let value = match tile.terrain.as_str() {
                "ocean" => -4,
                "coast" => -2,
                "snow" => -3,
                "tundra" => -1,
                "grassland" | "plains" => 3,
                "desert" => 1,
                _ => 0,
            };
            let proximity = 5 - g.wdist(target, pos);
            evidence += value * proximity;
            weight += proximity;
        }
        let terrain = if weight > 0 { evidence * 2 / weight } else { 0 };
        // Starts tend to be separated. Prefer searching beyond home and beyond
        // cities we have actually discovered, without consulting hidden owners.
        let spacing = home.map_or(0, |home| (g.wdist(home, target) - 5).clamp(-5, 5));
        let known_rival = g.cities.values().any(|city| {
            city.owner != pid && explored.contains(&city.pos) && g.wdist(city.pos, target) < 6
        });
        // On cylindrical/rectangular maps, a charted north/south boundary is
        // evidence against spending the next search at that pole. Globe rows
        // are storage coordinates and carry no latitude information.
        let edge = if !matches!(g.map.topology, crate::world::Topology::Globe(_))
            && explored.iter().any(|pos| {
                (pos.1 == 0 || pos.1 == g.map.height - 1) && (target.1 - pos.1).abs() < 3
            }) {
            -3
        } else {
            0
        };
        (terrain + spacing + edge - if known_rival { 5 } else { 0 }).clamp(-12, 12)
    }
}

#[cfg(test)]
mod tests;
