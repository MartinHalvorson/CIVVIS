//! `band-buys-with-a-stage-in-reach`: no Rock Band is bought while no stage a
//! band could play lies within four turns of a city of ours; the Faith stays
//! banked, and the band is bought the first turn a stage is in reach again.
//!
//! **Why (live Culture seats, 2026-11-15: 264 bands bought in 16 games).**
//! 17 of them were bought with no stage the host offered our bands within 16
//! hexes of any city of ours, and they played 14 concerts: 0.82 a band and
//! 0.057 a band-turn, against 2.13 and 0.153 for the other 247. In
//! civvis-20261115T142213Z (Russia) every one of Vietnam's 27-33 nearby stages
//! left every band's list at t185 at once (no war, Open Borders on: Music
//! Censorship is the host rule that does that). Only Maya's stages, 22-35
//! hexes overseas, were left, and `band-buys-near-the-stage` still bought
//! seven more bands ("22-26 hexes from the nearest of N stages"); those seven
//! played one concert, and the game's 19 bands five. A band is bought with
//! Faith only, and each one raises the next one's price
//! (`DLC/Expansion2/Data/Expansion2_Units.xml:42`: `MustPurchase="TRUE"`,
//! `PurchaseYield="YIELD_FAITH"`, `BaseMoves="4"`,
//! `CostProgressionModel="COST_PROGRESSION_PREVIOUS_COPIES"`); a Culture seat
//! has no better late use of the Faith than the next band that can play.
//!
//! **What the gene does.** The stages are the ones `band-buys-near-the-stage`
//! ranks cities by: the host's concert highlights for our bands, else the
//! wonders and Theater Squares of open rivals we have seen. A city buys a band
//! only when one of them lies within [`PURCHASE_REACH_TURNS`] turns at a
//! band's full movement (16 hexes at its 4 moves), by hex distance, the
//! purchase code's own measure: a city has no band yet to route. With no city
//! in reach the band waits, the journal names the nearest stage, and the band
//! bank (`culture_band_bank`) stays open past Cold War, so the Great Person,
//! worship-building and religious-unit passes leave the Faith. The Naturalist
//! purchase is unchanged. Every other purchase rule is unchanged.
//!
//! **Known limit.** A band's highlights leave out the stages it has already
//! played, so the union of our bands' highlights can miss a stage a new band
//! could play: in 1 of the 17 live purchases above the new band's own first
//! highlights held a stage 6 hexes from a city of ours
//! (civvis-20261115T133923Z t169; it played three concerts).

use super::band_buys_near_the_stage::band_stages;
use super::*;

/// Turns at a band's full movement within which a stage is in reach of the
/// city buying the band: `band-plays-the-stage-at-hand`'s longest trip for a
/// stage one level better. At a band's 4 moves that is 16 hexes; live, the
/// bands bought farther than that from every offered stage played 0.82
/// concerts each.
pub(super) const PURCHASE_REACH_TURNS: f64 =
    super::band_plays_the_stage_at_hand::ONE_LEVEL_TRIP_TURNS;

/// Hexes a new band covers in [`PURCHASE_REACH_TURNS`] turns.
fn purchase_reach(g: &Game) -> i32 {
    let moves = g
        .rules
        .units
        .get("rock_band")
        .map_or(4.0, |spec| spec.moves);
    (PURCHASE_REACH_TURNS * moves).floor() as i32
}

/// Hex distance from `city` to the nearest of `stages`, `None` with none.
fn nearest_stage(g: &Game, stages: &BTreeSet<Pos>, city: u32) -> Option<i32> {
    let pos = g.cities[&city].pos;
    stages.iter().map(|stage| g.wdist(pos, *stage)).min()
}

impl AdvancedAi {
    /// `band-buys-with-a-stage-in-reach`: `cities`, in their order, less those
    /// with no stage in reach. Empty when the band waits, which the journal
    /// says.
    pub(super) fn band_cities_in_reach(&self, g: &Game, pid: usize, cities: Vec<u32>) -> Vec<u32> {
        let stages = band_stages(g, pid);
        let reach = purchase_reach(g);
        let nearest = cities
            .iter()
            .filter_map(|city| nearest_stage(g, &stages, *city))
            .min();
        let kept: Vec<u32> = cities
            .into_iter()
            .filter(|city| nearest_stage(g, &stages, *city).is_some_and(|hexes| hexes <= reach))
            .collect();
        if kept.is_empty() {
            let nearest =
                nearest.map_or("none known".to_string(), |hexes| format!("{hexes} hexes"));
            think!(self.journal(), Faith, Detail,
                   "Holding the Rock Band: nearest stage {}", nearest;
                   "band-buys-with-a-stage-in-reach: no stage of {} within {} hexes ({} turns) of our cities; {:.0} Faith stays banked",
                   stages.len(), reach, PURCHASE_REACH_TURNS, g.players[pid].faith);
        }
        kept
    }

    /// Whether this turn's band waits for a stage: the gene is on, Cold War is
    /// known, and no city of ours has a stage in reach. The band bank reads it.
    pub(super) fn band_waits_for_a_stage(&self, g: &Game, pid: usize) -> bool {
        self.band_buys_with_a_stage_in_reach
            && g.players[pid].civics.contains(&crate::name!("cold_war"))
            && no_stage_in_reach(g, pid)
    }
}

/// Whether no city of `pid` has a stage within a new band's reach.
pub(super) fn no_stage_in_reach(g: &Game, pid: usize) -> bool {
    let stages = band_stages(g, pid);
    let reach = purchase_reach(g);
    !g.player_city_ids(pid).iter().any(|city| {
        let pos = g.cities[city].pos;
        stages
            .iter()
            .any(|stage| g.wdist_capped(pos, *stage, reach) <= reach)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::HostUnitFacts;
    use crate::name;
    use std::sync::Arc;

    /// Cold War known and Faith to spare. Our capital stands 27 hexes west of
    /// rival 1's wonder; with `near`, a second city of ours stands 10 from it.
    fn board(near: bool) -> (Game, Option<u32>) {
        let mut game = Game::new_full(2, 60, 16, 472_501, 250, 0, false);
        for uid in game.units.keys().copied().collect::<Vec<_>>() {
            game.remove_unit(uid);
        }
        for tile in game.map.tiles.values_mut() {
            tile.terrain = name!("grassland");
            tile.feature = None;
            tile.hills = false;
            tile.owner_city = None;
            tile.improvement = None;
            tile.district = None;
            tile.wonder = None;
        }
        game.found_city_for(0, (3, 6), None);
        let front = near.then(|| game.found_city_for(0, (20, 6), None));
        let host = game.found_city_for(1, (32, 6), None);
        let tile = game.map.tiles.get_mut(&(30, 6)).unwrap();
        tile.owner_city = Some(host);
        tile.wonder = Some(name!("pyramids"));
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.record_contact(0, 1);
        game.players[0].civics.insert(name!("cold_war"));
        game.players[0].faith = 10_000.0;
        (game, front)
    }

    /// The live seat's purchase genes, with or without this one.
    fn seat(on: bool) -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
        ai.enable_band_buys_near_the_stage();
        if on {
            ai.enable_band_buys_with_a_stage_in_reach();
        }
        ai
    }

    fn bands(game: &Game) -> Vec<Pos> {
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.kind == "rock_band")
            .map(|unit| unit.pos)
            .collect()
    }

    #[test]
    fn the_reach_is_four_turns_of_a_band() {
        let (game, _) = board(false);
        assert_eq!(purchase_reach(&game), 16);
    }

    #[test]
    fn off_a_band_is_bought_with_no_stage_in_reach() {
        let (mut game, _) = board(false);
        seat(false).culture_spending(&mut game, 0);
        assert_eq!(bands(&game), vec![(3, 6)]);
    }

    #[test]
    fn the_band_waits_while_the_nearest_stage_is_out_of_reach() {
        let (mut game, _) = board(false);
        seat(true).culture_spending(&mut game, 0);
        assert!(bands(&game).is_empty());
        assert_eq!(game.players[0].faith, 10_000.0);
    }

    #[test]
    fn the_band_is_bought_in_the_city_with_a_stage_in_reach() {
        let (mut game, front) = board(true);
        seat(true).culture_spending(&mut game, 0);
        assert_eq!(bands(&game), vec![game.cities[&front.unwrap()].pos]);
    }

    #[test]
    fn a_waiting_band_keeps_the_bank_but_not_the_naturalist() {
        let (game, _) = board(false);
        assert_eq!(seat(false).culture_band_bank(&game, 0), 0.0);
        assert!(seat(true).culture_band_bank(&game, 0) > 0.0);
        assert!(seat(true).culture_band_bank_for_people(&game, 0) > 0.0);
        assert!(!seat(true).culture_band_bank_holds_naturalists(&game, 0));
        // With a stage in reach the bank closes at Cold War as before.
        let (near, _) = board(true);
        assert_eq!(seat(true).culture_band_bank(&near, 0), 0.0);
    }

    #[test]
    fn a_seen_stage_the_host_no_longer_offers_does_not_count() {
        // civvis-20261115T142213Z: the near rival's stages left every band's
        // highlights at once; only stages far overseas were offered.
        let (mut game, _) = board(true);
        let band = game.spawn_test_unit("rock_band", 0, (5, 6));
        Arc::make_mut(&mut game.host_unit_facts).insert(
            band,
            HostUnitFacts {
                concert_plots: Some(BTreeSet::from([(40, 6)])),
                ..Default::default()
            },
        );
        let mut held = game.clone();
        seat(true).culture_spending(&mut held, 0);
        assert_eq!(bands(&held), vec![(5, 6)]);
        let mut bought = game.clone();
        seat(false).culture_spending(&mut bought, 0);
        assert_eq!(bands(&bought).len(), 2);
    }
}
