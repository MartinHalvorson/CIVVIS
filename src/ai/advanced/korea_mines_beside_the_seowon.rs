//! `korea-mines-beside-the-seowon`: Korea's Builders price Three Kingdoms.
//!
//! **The rule.** Civilization VI's Three Kingdoms (Expansion1 text, the live
//! host's ruleset) reads: "Mines receive +1 Science for every adjacent
//! Seowon district. Farms receive +1 Food for every adjacent Seowon
//! district." A Seowon must be built on Hills, so the plots around it are
//! the empire's natural Mine sites. The live host pays exactly this. The
//! engine's `data/civs.json` still carries a flat placeholder (+1 Science
//! and +1 Food in every city), and fixing that data moves the rules
//! fingerprint the Elo ledger checks, which is an operator decision. So
//! `AdvancedAi::improvement_value_with_appeal` priced a Mine beside a Seowon
//! at its Production alone.
//!
//! **The gene.** On Korea's own ground, a Mine is priced +1 Science and a
//! Farm +1 Food for each adjacent Seowon district Korea owns. This is the
//! same rule `korea-reads-three-kingdoms` (#4468) prices from the district
//! side when it sites the Seowon. The premium is added to the improvement's
//! yields before the lane prices them, so a standing Mine and a candidate
//! Mine are priced alike, and replacing one with the other is never read as
//! an upgrade. Only the Builder's valuation changes; the rules are
//! untouched. Off, or for any other civilization, nothing changes.
//!
//! **Size (25 live Deity Online Korea games, 2026-11-05).** Of each
//! Seowon's six neighbours, about two are not Korea's land at t80-100. The
//! Seowon-adjacent plain Hills left unmined fell from 0.36 per Seowon at t80
//! to 0.20 at t100 and 0.14 at t120. So the reach is modest, about one to
//! two Science a game through the mid-game, plus the Farm Food.

use super::*;

impl AdvancedAi {
    /// `korea-mines-beside-the-seowon`: the Three Kingdoms yields
    /// `improvement` earns on `pos` from the Seowons `pid` owns beside it.
    /// Zero with the gene off, for another civilization, for any improvement
    /// but a Mine or a Farm, and on ground another player owns.
    pub(super) fn three_kingdoms_improvement_yields(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
    ) -> Yields {
        let mut yields = Yields::default();
        if !self.korea_mines_beside_the_seowon || !matches!(improvement, "mine" | "farm") {
            return yields;
        }
        let Some(pid) = g
            .map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner)
        else {
            return yields;
        };
        if g.players[pid].civ != "Korea" {
            return yields;
        }
        let seowons = g
            .nbrs(pos)
            .into_iter()
            .filter(|neighbor| {
                g.map.get(*neighbor).is_some_and(|tile| {
                    tile.district == Some(crate::name!("seowon"))
                        && tile
                            .owner_city
                            .and_then(|city| g.cities.get(&city))
                            .is_some_and(|city| city.owner == pid)
                })
            })
            .count() as f64;
        match improvement {
            "mine" => yields.science += seowons,
            _ => yields.food += seowons,
        }
        yields
    }
}

#[cfg(test)]
mod tests;
