//! `band-plays-at-level-three`: a Rock Band's first concerts are played where
//! its performance level reaches 3, and its promotions are taken to get it
//! there.
//!
//! **Why.** A concert's performance level is the band's Rock Band level plus
//! the matching promotion's levels at that stage (Glam Rock, Arena Rock and
//! Reggae Rock +2 at Theater Squares, Entertainment Complexes and Water Parks;
//! Album Cover Art, Music Festival, Space Rock and Surf Rock +1 at wonders,
//! natural wonders and national parks, Campuses and Spaceports, Harbors and
//! Seaside Resorts; `Expansion2_UnitPromotions.xml`,
//! `MODIFIER_PLAYER_UNIT_ADJUST_ROCK_BAND_LEVEL_*`), and the level sets the
//! breakup odds: 45% at level 1, 34% at 2, 25% at 3. Nothing else in the
//! shipped data raises it (`ROCK_BAND_MAX_LEVEL` 4; the Nobel Prize in
//! Literature competition adds album sales and a cheaper band, not levels);
//! Legends and Headliners results raise the band's own level by one.
//!
//! Live, the 104 concerts of 2026-11-10 played 39% at level 1 (51% of them
//! broke the band up), 19% at level 2 (40%), and 42% at 3 or more (13%).
//! 24 of 48 first concerts used no stage bonus at all and 11 used a +2 stage.
//! 205256Z bought nine bands, eight of which broke up within two concerts;
//! 173750Z's 109k concert Tourism came from the two bands that lived seven
//! and eight concerts. A band's first two concerts decide its career.
//!
//! **What the gene does.**
//! - Tour: a band below level 3 ranks the stages by the level it would play
//!   them at: level 3 or more within four turns first, then level 3 or more
//!   farther within the tour's reach, then level 2, then level 1, each group in
//!   the seat's scorer order (so the bar holder first within it). It never
//!   plays the stage under it while a stage of a higher level is on that
//!   list: a Glam Rock band skips a wonder (level 1) for a Theater Square
//!   (level 3), because a level-3 concert breaks a band up a sixth as often.
//! - Promotion (inside `band-plays-its-stage`'s chooser): among the offered
//!   promotions that lift an open, reachable stage to level 3, the one with
//!   the most levels (a +2 over a +1 at a band's first level-up), then the
//!   most such stages; failing that, the one that raises the level at the
//!   most of them; failing that, the career-value choice as before. Stages in
//!   land closed to our bands, or beyond the tour's fifteen turns, do not
//!   count. Live, 37 of 69 bands were offered a +2 promotion first (three of
//!   the twelve are drawn), so the first promotion is a real choice about
//!   half the time.

//!
//! **Version 2 (`band-plays-at-level-three-2`).** Version 1 moved every
//! level-3 stage within the tour's fifteen turns ahead of everything else and
//! vetoed a lower-level show while one was listed, on top of a scorer that
//! priced travel at a tenth of the stage per turn. Live 20261111T023321Z
//! (16 bands, 299 band-turns, 19 concerts, 0.067 a band-turn): on 249
//! band-turns without a concert the nearest listed stage was within 2 tiles
//! on 60 and within 4 on 125, and on 52 of those 125 the band was farther
//! from it the next turn; an Arena Rock band sailed an ocean for two concerts
//! in 45 turns. Version 2 ranks a stage by what it is worth per turn of
//! travel: P(survive) x (venue x (1 + result %) + 10 x album sales) x the bar
//! factor (2 for the bar holder) / (turns to reach + 1), where a stage the
//! band can walk to and play this turn is 0 turns away. The level enters only
//! through the survival odds and the result table, never as a veto. On top:
//! a level-3 stage it can reach and play this turn is always played; a band
//! below level 3 travels at most three turns while any stage is that near;
//! and no stage across water while one is reachable over land. Break-even:
//! at equal venue value a level-1 show now (55% survival, expected result
//! x1.47) beats a level-3 stage (75%, x1.65) more than half a turn away, so a
//! level-3 stage wins only when it is in this turn's reach or its venue is
//! worth more. An expelled band re-plans from where the host put it (the
//! tour keeps no target between turns).

use crate::ai::BasicAi;
use crate::game::Game;
use crate::Pos;

/// The performance level from which a concert's breakup odds drop to 25%.
pub(crate) const LEVEL_THREE: i32 = 3;
/// How far, in turns at full movement, a band below level 3 travels for a
/// level-3 stage before it settles for the scorer's choice.
const LEVEL_THREE_TRIP_TURNS: f64 = 4.0;

/// `band-plays-at-level-three-2`: turns of travel a band below level 3 takes
/// while any stage is within them.
const NEAR_TRIP_TURNS: usize = 3;

/// One stage the version-2 tour weighs.
struct PerTurnStage {
    pos: Pos,
    turns: usize,
    distance: usize,
    level: i32,
    value: f64,
    water: bool,
}

impl BasicAi {
    /// `band-plays-at-level-three-2`: every stage the band can play, best
    /// value per turn of travel first. See the module notes.
    pub(crate) fn rock_band_per_turn_ranking(&self, g: &Game, pid: usize, uid: u32) -> Vec<Pos> {
        let Some(unit) = g.units.get(&uid) else {
            return Vec::new();
        };
        let below_three = g.rock_band_performance_level(unit, 0) < LEVEL_THREE;
        let moves = g.unit_max_moves(uid).max(1.0);
        let mut stages: Vec<PerTurnStage> = Vec::new();
        for pos in g.map.tiles.keys().copied() {
            let Some(value) = g.rock_concert_turn_value(pid, uid, pos) else {
                continue;
            };
            let Some(distance) = g.route_distance(uid, pos, 0) else {
                continue;
            };
            // A band walks onto a stage and plays it the same turn.
            let turns = if distance == 0 {
                0
            } else {
                ((distance as f64 / moves).ceil() as usize).saturating_sub(1)
            };
            if turns as f64 > crate::ai::ROCK_BAND_TOUR_MAX_TURNS {
                continue;
            }
            let bar = if self.bands_draw_from_the_bar {
                g.rock_concert_host(pid, uid, pos)
                    .map_or(1.0, |host| g.rock_concert_bar_factor(pid, host))
            } else {
                1.0
            };
            stages.push(PerTurnStage {
                pos,
                turns,
                distance,
                level: g.rock_concert_performance_level(pid, uid, pos).unwrap_or(1),
                value: value * bar / (turns as f64 + 1.0),
                water: g.route_crosses_water(uid, pos).unwrap_or(true),
            });
        }
        if stages.iter().any(|stage| !stage.water) {
            stages.retain(|stage| !stage.water);
        }
        if stages
            .iter()
            .any(|stage| stage.turns == 0 && stage.level >= LEVEL_THREE)
        {
            stages.retain(|stage| stage.turns == 0 && stage.level >= LEVEL_THREE);
        } else if below_three && stages.iter().any(|stage| stage.turns <= NEAR_TRIP_TURNS) {
            stages.retain(|stage| stage.turns <= NEAR_TRIP_TURNS);
        }
        stages.sort_by(|left, right| {
            right
                .value
                .total_cmp(&left.value)
                .then(left.distance.cmp(&right.distance))
                .then(left.pos.cmp(&right.pos))
        });
        stages.into_iter().map(|stage| stage.pos).collect()
    }

    /// The level class a stage gives this band: 3 for level 3 or more,
    /// otherwise its performance level; 0 where it has no concert.
    fn rock_band_level_class(g: &Game, pid: usize, uid: u32, pos: Pos) -> i32 {
        g.rock_concert_performance_level(pid, uid, pos)
            .map_or(0, |level| level.min(LEVEL_THREE))
    }

    /// Whether the band is below level 3 and so tours by level class.
    fn rock_band_below_level_three(g: &Game, uid: u32) -> bool {
        g.units
            .get(&uid)
            .is_some_and(|unit| g.rock_band_performance_level(unit, 0) < LEVEL_THREE)
    }

    /// `band-plays-at-level-three`: the scorer's ranking regrouped by the
    /// level a band below level 3 would play each stage at (level 3 within
    /// four turns, level 3 farther, level 2, level 1), keeping the scorer's
    /// order inside each group.
    pub(crate) fn rock_band_level_three_first(
        g: &Game,
        pid: usize,
        uid: u32,
        ranked: Vec<Pos>,
    ) -> Vec<Pos> {
        if !Self::rock_band_below_level_three(g, uid) {
            return ranked;
        }
        let moves = g.unit_max_moves(uid).max(1.0);
        let mut grouped: Vec<(u8, Pos)> = ranked
            .into_iter()
            .map(|pos| {
                let group = match Self::rock_band_level_class(g, pid, uid, pos) {
                    LEVEL_THREE => {
                        let near = g.route_distance(uid, pos, 0).is_some_and(|distance| {
                            distance as f64 / moves <= LEVEL_THREE_TRIP_TURNS
                        });
                        if near {
                            0
                        } else {
                            1
                        }
                    }
                    2 => 2,
                    _ => 3,
                };
                (group, pos)
            })
            .collect();
        grouped.sort_by_key(|(group, _)| *group);
        grouped.into_iter().map(|(_, pos)| pos).collect()
    }

    /// `band-plays-at-level-three`: whether a band below level 3 may play the
    /// stage under it, which it may not while a stage of a higher level class
    /// is on its tour.
    pub(crate) fn rock_band_may_play_here(
        g: &Game,
        pid: usize,
        uid: u32,
        here: Pos,
        ranked: &[Pos],
    ) -> bool {
        if !Self::rock_band_below_level_three(g, uid) {
            return true;
        }
        let class = Self::rock_band_level_class(g, pid, uid, here);
        ranked
            .iter()
            .all(|pos| *pos == here || Self::rock_band_level_class(g, pid, uid, *pos) <= class)
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::advanced::AdvancedAi;
    use crate::ai::BasicAi;
    use crate::game::Game;
    use crate::name;
    use crate::Pos;

    /// Two majors on open grassland. The rival's land holds a wonder two hexes
    /// from our band and a Theater Square with an Amphitheater five hexes
    /// away.
    fn stages() -> (Game, u32, Pos, Pos) {
        let mut game = Game::new_full(2, 40, 16, 472_501, 250, 0, false);
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
        game.found_city_for(0, (4, 6), None);
        let host = game.found_city_for(1, (16, 6), None);
        let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
        for pos in positions {
            if (10..=22).contains(&pos.0) && (2..=10).contains(&pos.1) {
                game.map.tiles.get_mut(&pos).unwrap().owner_city = Some(host);
            }
        }
        let wonder = (11, 6);
        let theater = (14, 6);
        game.map.tiles.get_mut(&wonder).unwrap().wonder = Some(name!("pyramids"));
        game.map.tiles.get_mut(&theater).unwrap().district = Some(name!("theater_square"));
        let city = game.cities.get_mut(&host).unwrap();
        city.districts.insert(name!("theater_square"), theater);
        city.buildings.push(name!("amphitheater"));
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.record_contact(0, 1);
        let band = game.spawn_test_unit("rock_band", 0, (9, 6));
        (game, band, wonder, theater)
    }

    #[test]
    fn a_level_one_band_plays_its_two_level_stage_first() {
        let (mut game, band, wonder, theater) = stages();
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("glam_rock"));
        assert_eq!(
            game.rock_concert_performance_level(0, band, wonder),
            Some(1)
        );
        assert_eq!(
            game.rock_concert_performance_level(0, band, theater),
            Some(3)
        );
        let stock = BasicAi::new();
        assert_eq!(stock.rock_band_ranked_venues(&game, 0, band)[0], wonder);
        let mut ai = BasicAi::new();
        ai.band_plays_at_level_three = true;
        assert_eq!(ai.rock_band_ranked_venues(&game, 0, band)[0], theater);
        // Standing on the wonder, it walks on to the Theater rather than play
        // a level-1 show.
        game.relocate(band, wonder);
        let full = game.unit_max_moves(band);
        game.units.get_mut(&band).unwrap().moves_left = full;
        assert!(game.rock_concert_tourism(0, band).is_some());
        assert!(ai.rock_band_step(&mut game, 0, band));
        assert!(game.wdist(game.units[&band].pos, theater) < game.wdist(wonder, theater));
        game.relocate(band, (9, 6));
        // A band already at level 3 keeps the scorer's order.
        game.units.get_mut(&band).unwrap().level = 3;
        assert_eq!(ai.rock_band_ranked_venues(&game, 0, band)[0], wonder);
    }

    #[test]
    fn the_first_promotion_lifts_the_open_stages_to_level_three() {
        let (mut game, band, _, _) = stages();
        // Ten wonders against one Theater Square.
        for q in 12..=21 {
            game.map.tiles.get_mut(&(q, 9)).unwrap().wonder = Some(name!("colosseum"));
        }
        let offered = [name!("glam_rock"), name!("album_cover_art")];
        let mut ai = AdvancedAi::new();
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("album_cover_art")),
            "the career chooser takes the +1 stage it sees ten of"
        );
        ai.base.band_plays_at_level_three = true;
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("glam_rock"))
        );
        // Behind Music Censorship the Theater lifts nothing, and the career
        // chooser decides as before.
        game.players[1].policies.insert(name!("music_censorship"));
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("album_cover_art"))
        );
    }

    fn version_two() -> BasicAi {
        let mut ai = BasicAi::new();
        ai.band_plays_at_level_three_2 = true;
        ai
    }

    fn glam(game: &mut Game, band: u32) {
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("glam_rock"));
    }

    #[test]
    fn version_two_plays_the_near_wonder_instead_of_walking_off_to_its_stage() {
        // Live 20261111T023321Z: version 1 walked a level-1 band away from a
        // stage two tiles off toward a level-3 stage further away.
        let (mut game, band, wonder, theater) = stages();
        glam(&mut game, band);
        let mut first = BasicAi::new();
        first.band_plays_at_level_three = true;
        assert_eq!(first.rock_band_ranked_venues(&game, 0, band)[0], theater);
        let ai = version_two();
        assert_eq!(ai.rock_band_ranked_venues(&game, 0, band)[0], wonder);
        let mut played = false;
        for _ in 0..4 {
            if !game.units.contains_key(&band) || !ai.rock_band_step(&mut game, 0, band) {
                break;
            }
            played |= game.players[0]
                .counters
                .keys()
                .any(|key| key.starts_with("rock_concert:"));
        }
        assert!(played || !game.units.contains_key(&band));
    }

    #[test]
    fn version_two_always_plays_a_level_three_stage_in_reach_this_turn() {
        let (mut game, band, wonder, theater) = stages();
        glam(&mut game, band);
        game.relocate(band, (12, 6));
        // The wonder is worth more for this one concert, but the Theater is
        // a level-3 show the band can walk to and play this turn.
        assert!(
            game.rock_concert_turn_value(0, band, wonder).unwrap()
                > game.rock_concert_turn_value(0, band, theater).unwrap()
        );
        assert_eq!(
            version_two().rock_band_ranked_venues(&game, 0, band),
            vec![theater]
        );
    }

    #[test]
    fn version_two_does_not_sail_while_a_stage_is_reachable_over_land() {
        let (mut game, band, wonder, theater) = stages();
        glam(&mut game, band);
        game.players[0].techs.insert(name!("shipbuilding"));
        // A channel between the wonder and the Theater.
        let channel: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| (12..=13).contains(&pos.0))
            .collect();
        for pos in channel {
            game.map.tiles.get_mut(&pos).unwrap().terrain = name!("coast");
        }
        game.relocate(band, (10, 6));
        assert_eq!(game.route_crosses_water(band, wonder), Some(false));
        assert_eq!(game.route_crosses_water(band, theater), Some(true));
        assert_eq!(
            version_two().rock_band_ranked_venues(&game, 0, band),
            vec![wonder]
        );
    }
}
