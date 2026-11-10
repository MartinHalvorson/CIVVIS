//! `bands-wait-at-the-door`: a Rock Band with nothing it can play waits at the
//! border of the land it would most like to play, so it plays as soon as that
//! land opens.
//!
//! **Why.** Music Censorship ("Rock Bands from other players may not enter
//! your territory", `POLICY_MUSIC_CENSORSHIP`, a Diplomatic card on
//! `CIVIC_SPACE_RACE`; no `AiFavoredItems` row names it, so the AI's own
//! policy scoring slots it) is now the cap on the Culture plan. Across the 11
//! Kristina games of 2026-11-10 with named bands, read from the host's own
//! concert highlights, 179 rival-turns were closed against 553 open; the bar
//! holder was closed on 83 of its 162 readable turns, twice the field's rate.
//! The closures are the AI re-slotting its cards, not a wall: 21 of 29 closed
//! spells reopened before the game ended, a closed spell lasted a median 5
//! turns, and 19 of 21 reopenings came on a turn the rival completed a civic
//! (free re-slot). Onsets came 19-60 turns after the rival's Space Race, not
//! at it, and only weakly after our own bands: 7 of 18 onsets followed a band
//! of ours entering or playing in that land within three turns (5.4% a turn
//! when exposed, 2.9% when not), and 10 rivals were already closed when our
//! first band appeared. So hiding bands and striking in waves would buy little;
//! being at the door when the land reopens buys the 3-5 turns 192354Z lost
//! walking in after Inca reopened at t173.
//!
//! **What the gene does.** When a band has no stage it can play or walk to
//! (`rock_band_step` would otherwise end its turn idle), it walks to the open
//! tile nearest the best stage of the best closed rival and waits there: the
//! rival with the most domestic tourists first (the Culture bar holder), then
//! a wonder over a district stage, then the nearest. A rival at war with us is
//! no door to wait at. A tile in land closed to our bands, a city centre or
//! water is no waiting place. When several rivals are open the tour is
//! unchanged: the seat's scorer already prefers the bar holder among the
//! stages the band can reach. On a live board the closed rivals come from
//! `mirror::band_closed_hosts`; natively from Music Censorship itself.

use crate::ai::BasicAi;
use crate::game::Game;
use crate::Pos;

/// How far from a closed rival's stage a band looks for its waiting tile.
const DOOR_RADIUS: i32 = 6;
/// Stages, best first, a band tries before it finds no door at all.
const DOOR_STAGE_TRIES: usize = 4;
/// Waiting tiles per stage, nearest first, tried for a route.
const DOOR_TILE_TRIES: usize = 8;

impl BasicAi {
    /// `bands-wait-at-the-door`: step toward the waiting tile, or hold there.
    pub(crate) fn rock_band_door_step(
        &self,
        g: &mut Game,
        pid: usize,
        uid: u32,
        here: Pos,
    ) -> bool {
        let Some(door) = Self::rock_band_door(g, pid, uid) else {
            return false;
        };
        if door == here {
            return false;
        }
        let Some(next) = g.route_step(uid, door, 0) else {
            return false;
        };
        self.path_move(g, pid, uid, next)
    }

    /// The tile this band waits on: the open, reachable land tile nearest the
    /// best stage of the best rival whose land is closed to our bands.
    pub(crate) fn rock_band_door(g: &Game, pid: usize, uid: u32) -> Option<Pos> {
        let here = g.units.get(&uid)?.pos;
        let land_owner = |pos: Pos| {
            g.map
                .get(pos)
                .and_then(|tile| tile.owner_city)
                .and_then(|city| g.cities.get(&city))
                .map(|city| city.owner)
        };
        let mut stages: Vec<(i64, u32, i32, Pos)> = Vec::new();
        for (pos, tile) in &g.map.tiles {
            let Some(host) = land_owner(*pos) else {
                continue;
            };
            let player = &g.players[host];
            if host == pid
                || player.is_minor
                || player.is_barbarian
                || !player.alive
                || !g.players[pid].explored.contains(pos)
                || g.is_at_war(pid, host)
                || !g.bars_foreign_rock_bands(host, pid)
            {
                continue;
            }
            let value = if tile.wonder.is_some() {
                1_000
            } else if !tile.pillaged
                && tile.district.is_some_and(|district| {
                    matches!(
                        g.district_family(district).as_str(),
                        "theater_square" | "entertainment_complex" | "water_park"
                    )
                })
            {
                500
            } else {
                continue;
            };
            let domestic = g.domestic_tourists(host).max(0);
            stages.push((domestic, value, g.wdist(here, *pos), *pos));
        }
        stages.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then(right.1.cmp(&left.1))
                .then(left.2.cmp(&right.2))
                .then(left.3.cmp(&right.3))
        });
        for (_, _, _, stage) in stages.into_iter().take(DOOR_STAGE_TRIES) {
            let mut doors: Vec<(i32, Pos)> =
                g.wdisk(stage, DOOR_RADIUS)
                    .into_iter()
                    .filter(|pos| {
                        g.map.get(*pos).is_some_and(|tile| {
                            g.rules.is_passable(tile) && !g.rules.is_water(tile)
                        }) && g.city_at(*pos).is_none()
                            && land_owner(*pos)
                                .is_none_or(|owner| !g.bars_foreign_rock_bands(owner, pid))
                    })
                    .map(|pos| (g.wdist(pos, stage), pos))
                    .collect();
            doors.sort();
            for (_, door) in doors.into_iter().take(DOOR_TILE_TRIES) {
                if door == here || g.route_distance(uid, door, 0).is_some() {
                    return Some(door);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use crate::ai::BasicAi;
    use crate::game::Game;
    use crate::name;
    use crate::Pos;

    /// Three majors on open grassland. Rival 1 holds a block of land with a
    /// wonder at (17, 6); rival 2 holds another block beyond it with a wonder
    /// at (29, 6). Both slot Music Censorship. Our band starts at home.
    fn two_closed_rivals() -> (Game, u32, Pos, Pos) {
        let mut game = Game::new_full(3, 40, 16, 472_501, 250, 0, false);
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
        let first = game.found_city_for(1, (20, 6), None);
        let second = game.found_city_for(2, (32, 6), None);
        let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
        for pos in positions {
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.owner_city = if (2..=10).contains(&pos.1) && (16..=24).contains(&pos.0) {
                Some(first)
            } else if (2..=10).contains(&pos.1) && (28..=36).contains(&pos.0) {
                Some(second)
            } else if tile.owner_city == Some(first) || tile.owner_city == Some(second) {
                None
            } else {
                tile.owner_city
            };
        }
        let near = (17, 6);
        let far = (29, 6);
        game.map.tiles.get_mut(&near).unwrap().wonder = Some(name!("pyramids"));
        game.map.tiles.get_mut(&far).unwrap().wonder = Some(name!("colosseum"));
        for rival in [1, 2] {
            game.players[rival]
                .policies
                .insert(name!("music_censorship"));
            game.record_contact(0, rival);
        }
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        let band = game.spawn_test_unit("rock_band", 0, (5, 6));
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("album_cover_art"));
        (game, band, near, far)
    }

    fn new_turn(game: &mut Game, band: u32) {
        game.turn += 1;
        let full = game.unit_max_moves(band);
        game.units.get_mut(&band).unwrap().moves_left = full;
    }

    fn waiting_ai() -> BasicAi {
        let mut ai = BasicAi::new();
        ai.band_plays_what_it_can_reach = true;
        ai.bands_wait_at_the_door = true;
        ai
    }

    /// Step the band turn by turn until it stops acting.
    fn walk_until_it_waits(ai: &BasicAi, game: &mut Game, band: u32) -> Pos {
        for _ in 0..40 {
            new_turn(game, band);
            let mut acted = false;
            for _ in 0..8 {
                if game.units[&band].moves_left <= 0.0 || !ai.rock_band_step(game, 0, band) {
                    break;
                }
                acted = true;
            }
            if !acted {
                break;
            }
        }
        game.units[&band].pos
    }

    fn owner_of(game: &Game, pos: Pos) -> Option<usize> {
        game.map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| game.cities.get(&city))
            .map(|city| city.owner)
    }

    #[test]
    fn with_every_rival_closed_the_band_waits_at_the_bar_holders_door() {
        let (mut game, band, near, far) = two_closed_rivals();
        // Rival 2 holds the bar.
        game.players[2].culture_lifetime = 50_000.0;
        let ai = BasicAi::new();
        assert!(!ai.rock_band_step(&mut game, 0, band), "nothing to play");
        let ai = waiting_ai();
        let at = walk_until_it_waits(&ai, &mut game, band);
        assert_eq!(game.wdist(at, far), 2, "{at:?}");
        assert_eq!(owner_of(&game, at), None);
        assert!(game.wdist(at, near) > 2);

        // Rival 2 drops the card: the band plays its wonder within a turn.
        game.players[2].policies.clear();
        new_turn(&mut game, band);
        for _ in 0..8 {
            if !game.units.contains_key(&band) || !ai.rock_band_step(&mut game, 0, band) {
                break;
            }
        }
        let played = game.players[0]
            .counters
            .iter()
            .any(|(key, count)| key.starts_with("rock_concert:") && *count > 0);
        assert!(played || !game.units.contains_key(&band));
    }

    #[test]
    fn the_bar_holder_comes_before_the_nearer_rival() {
        let (mut game, band, near, far) = two_closed_rivals();
        game.players[1].culture_lifetime = 50_000.0;
        let door = BasicAi::rock_band_door(&game, 0, band).unwrap();
        assert_eq!(game.wdist(door, near), 2, "{door:?}");
        game.players[1].culture_lifetime = 0.0;
        game.players[2].culture_lifetime = 50_000.0;
        let door = BasicAi::rock_band_door(&game, 0, band).unwrap();
        assert_eq!(game.wdist(door, far), 2, "{door:?}");
    }

    #[test]
    fn no_door_at_a_rival_at_war_or_with_its_land_open() {
        let (mut game, band, _, _) = two_closed_rivals();
        game.players[1].policies.clear();
        game.players[2].policies.clear();
        assert_eq!(BasicAi::rock_band_door(&game, 0, band), None);
        let (mut game, band, _, _) = two_closed_rivals();
        game.at_war.insert((0, 1));
        game.at_war.insert((0, 2));
        assert_eq!(BasicAi::rock_band_door(&game, 0, band), None);
    }
}
