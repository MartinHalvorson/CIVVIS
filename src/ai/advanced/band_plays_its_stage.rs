//! `band-plays-its-stage`: a Rock Band's promotion is priced by its career.
//!
//! Firaxis's six performance results make a concert's performance level (the
//! band's own level plus the venue's matching-promotion levels) decide both
//! the breakup odds and how fast album sales compound, and every album sale
//! adds to every later concert. Live: 86% of 355 concerts played at levels 1
//! or 2, 80 of 168 bands played once, and only 3 concerts used a two-level
//! stage. The shipped chooser priced Goes To 11 above the two-level stages.

use super::*;

/// Seen stages by family, foreign and unpillaged, that a band might play.
#[derive(Default)]
struct StageCounts {
    wonder: usize,
    nature: usize,
    theater: usize,
    entertainment: usize,
    water_park: usize,
    space: usize,
    surf: usize,
}

/// `reach`: under `band-plays-what-it-can-reach`, the band being promoted. A
/// stage then counts only in land open to our bands and within the tour's
/// fifteen turns of it: live 20261110T164254Z took Reggae Rock for three
/// Water Parks, two of them behind Music Censorship and the third a sea
/// voyage away.
fn stage_counts(g: &Game, pid: usize, reach: Option<u32>) -> StageCounts {
    let mut counts = StageCounts::default();
    let moves = reach.map_or(1.0, |uid| g.unit_max_moves(uid).max(1.0));
    let reachable = |pos: Pos| {
        reach.is_none_or(|uid| {
            g.route_distance(uid, pos, 0).is_some_and(|distance| {
                distance as f64 / moves <= crate::ai::ROCK_BAND_TOUR_MAX_TURNS
            })
        })
    };
    for (pos, tile) in &g.map.tiles {
        let Some(city) = tile.owner_city.and_then(|cid| g.cities.get(&cid)) else {
            continue;
        };
        let owner = &g.players[city.owner];
        if city.owner == pid
            || g.same_team(pid, city.owner)
            || owner.is_minor
            || owner.is_barbarian
            || !owner.alive
            || !g.players[pid].explored.contains(pos)
            || (reach.is_some() && g.bars_foreign_rock_bands(city.owner, pid))
        {
            continue;
        }
        if reach.is_some() && !reachable(*pos) {
            continue;
        }
        if tile.wonder.is_some() {
            counts.wonder += 1;
        }
        let natural_wonder = tile.feature.as_ref().is_some_and(|feature| {
            g.rules
                .features
                .get(feature)
                .is_some_and(|spec| spec.natural_wonder)
        });
        if natural_wonder || tile.improvement.as_deref() == Some("national_park") {
            counts.nature += 1;
        }
        if tile.pillaged {
            continue;
        }
        if tile.improvement.as_deref() == Some("seaside_resort") {
            counts.surf += 1;
        }
        let Some(district) = tile.district else {
            continue;
        };
        match g.district_family(district).as_str() {
            "theater_square" => counts.theater += 1,
            "entertainment_complex" => counts.entertainment += 1,
            "water_park" => counts.water_park += 1,
            "campus" | "spaceport" => counts.space += 1,
            "harbor" => counts.surf += 1,
            _ => {}
        }
    }
    counts
}

/// `band-plays-at-level-three`: the offered promotion that lifts the most
/// open, reachable stages to performance level 3; failing that, the one that
/// raises the level at the most of them (levels times stages); `None` when no
/// offered promotion raises the level anywhere the band can play, which
/// leaves the career chooser to decide. See `advanced/band_plays_at_level_three.rs`.
fn level_three_promotion(g: &Game, pid: usize, uid: u32, offered: &[Name]) -> Option<Name> {
    let unit = g.units.get(&uid)?;
    let counts = stage_counts(g, pid, Some(uid));
    let base = g.rock_band_performance_level(unit, 0);
    let mut best: Option<((bool, usize, usize), Name)> = None;
    for &name in offered {
        let Some(promotion) = g.rules.promotions.get(&name) else {
            continue;
        };
        let lift = [
            ("rock_theater_levels", counts.theater),
            ("rock_entertainment_levels", counts.entertainment),
            ("rock_water_park_levels", counts.water_park),
            ("rock_wonder_levels", counts.wonder),
            ("rock_nature_levels", counts.nature),
            ("rock_space_levels", counts.space),
            ("rock_surf_levels", counts.surf),
        ]
        .into_iter()
        .find_map(|(effect, stages)| {
            promotion
                .effects
                .get(effect)
                .map(|bonus| (*bonus as usize, stages))
        });
        let Some((bonus, stages)) = lift.filter(|(bonus, stages)| *bonus > 0 && *stages > 0) else {
            continue;
        };
        let reaches = base + bonus as i32 >= super::band_plays_at_level_three::LEVEL_THREE;
        let key = (
            reaches,
            if reaches { stages } else { bonus * stages },
            bonus,
        );
        if best.as_ref().is_none_or(|(held, held_name)| {
            key > *held || (key == *held && name.as_str() < held_name.as_str())
        }) {
            best = Some((key, name));
        }
    }
    best.map(|(_, name)| name)
}

/// A band needs about five stages to spend a career on.
fn coverage(stages: usize) -> f64 {
    (stages as f64 / 5.0).min(1.0)
}

impl AdvancedAi {
    /// The offered promotion that makes this band worth the most over its
    /// career, or `None` for a band this gene does not price.
    pub(super) fn band_stage_promotion(
        &self,
        g: &Game,
        pid: usize,
        uid: u32,
        offered: &[Name],
    ) -> Option<Name> {
        let unit = g.units.get(&uid).filter(|unit| unit.kind == "rock_band")?;
        if offered.is_empty() {
            return None;
        }
        if self.base.band_plays_at_level_three {
            if let Some(promotion) = level_three_promotion(g, pid, uid, offered) {
                return Some(promotion);
            }
        }
        let counts = stage_counts(
            g,
            pid,
            self.base.band_plays_what_it_can_reach.then_some(uid),
        );
        let level = g.rock_band_performance_level(unit, 0).clamp(1, 4) as usize;
        let multiple = |bonus: usize| Game::rock_band_stage_multiple(level, bonus).0;
        let held_bonus = unit
            .promotions
            .iter()
            .filter_map(|held| g.rules.promotions.get(held))
            .flat_map(|promotion| &promotion.effects)
            .filter(|(effect, _)| effect.starts_with("rock_") && effect.ends_with("_levels"))
            .map(|(_, amount)| *amount as usize)
            .max()
            .unwrap_or(0);
        let held_stages: usize = unit
            .promotions
            .iter()
            .filter_map(|held| g.rules.promotions.get(held))
            .map(|promotion| {
                let reads = |effect: &str| promotion.effects.contains_key(effect);
                if reads("rock_theater_levels") {
                    counts.theater
                } else if reads("rock_entertainment_levels") {
                    counts.entertainment
                } else if reads("rock_water_park_levels") {
                    counts.water_park
                } else if reads("rock_wonder_levels") {
                    counts.wonder
                } else if reads("rock_nature_levels") {
                    counts.nature
                } else if reads("rock_space_levels") {
                    counts.space
                } else if reads("rock_surf_levels") {
                    counts.surf
                } else {
                    0
                }
            })
            .max()
            .unwrap_or(0);
        let mut ranked: Vec<(f64, Name)> = offered
            .iter()
            .filter_map(|&name| {
                let promotion = g.rules.promotions.get(&name)?;
                let effects = &promotion.effects;
                let stage = [
                    ("rock_theater_levels", counts.theater, 500.0),
                    ("rock_entertainment_levels", counts.entertainment, 500.0),
                    ("rock_water_park_levels", counts.water_park, 500.0),
                    ("rock_wonder_levels", counts.wonder, 1_000.0),
                    ("rock_nature_levels", counts.nature, 1_000.0),
                    ("rock_space_levels", counts.space, 500.0),
                    ("rock_surf_levels", counts.surf, 500.0),
                ]
                .into_iter()
                .find_map(|(effect, stages, base)| {
                    effects
                        .get(effect)
                        .map(|bonus| multiple(*bonus as usize) * base * coverage(stages))
                });
                let score = if let Some(stage) = stage {
                    stage
                } else if effects.contains_key("movement") {
                    // Roadies reaches more stages; worth more once the band
                    // already has somewhere it can survive.
                    let mobility = if held_bonus > 0 && held_stages >= 5 {
                        1.15
                    } else {
                        0.8
                    };
                    multiple(held_bonus) * 700.0 * mobility
                } else if effects.contains_key("rock_nearby_tourism_pct") {
                    multiple(0) * 1_000.0 * 1.25
                } else {
                    multiple(0) * 700.0
                };
                Some((score, name))
            })
            .collect();
        ranked.sort_by(|left, right| {
            right
                .0
                .total_cmp(&left.0)
                .then(left.1.as_str().cmp(right.1.as_str()))
        });
        ranked.first().map(|(_, name)| *name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::BasicAi;
    use crate::name;

    fn world_with_theaters() -> (Game, u32) {
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
        let host = game.found_city_for(1, (12, 6), None);
        let hall = game.cities.get_mut(&host).unwrap();
        hall.buildings.push(name!("amphitheater"));
        hall.buildings.push(name!("art_museum"));
        for (offset, pos) in [(8, 6), (9, 6), (10, 6), (11, 6), (8, 7), (9, 7)]
            .into_iter()
            .enumerate()
        {
            let city = game.cities.get_mut(&host).unwrap();
            if offset == 0 {
                city.districts.insert(name!("theater_square"), pos);
            }
            let tile = game.map.tiles.get_mut(&pos).unwrap();
            tile.owner_city = Some(host);
            tile.district = Some(name!("theater_square"));
        }
        game.players[0]
            .explored
            .extend(game.map.tiles.keys().copied());
        game.record_contact(0, 1);
        let band = game.spawn_test_unit("rock_band", 0, (7, 6));
        (game, band)
    }

    #[test]
    fn the_two_level_stage_outranks_goes_to_11_where_theaters_stand() {
        let (game, band) = world_with_theaters();
        let ai = AdvancedAi::new();
        let offered = [name!("goes_to_11"), name!("glam_rock"), name!("pop_star")];
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("glam_rock"))
        );
    }

    #[test]
    fn a_stage_with_no_seen_venue_does_not_outrank_a_real_one() {
        let (game, band) = world_with_theaters();
        let ai = AdvancedAi::new();
        // No Entertainment Complex stands, so Arena Rock is worth nothing.
        let offered = [name!("arena_rock"), name!("roadies")];
        assert_eq!(
            ai.band_stage_promotion(&game, 0, band, &offered),
            Some(name!("roadies"))
        );
    }

    #[test]
    fn career_value_grows_with_the_stage_and_with_album_sales() {
        let (game, band) = world_with_theaters();
        let mut stage = game.units[&band].clone();
        stage.promotions.insert(name!("glam_rock"));
        let mut plain = game.units[&band].clone();
        plain.promotions.insert(name!("goes_to_11"));
        let stage_future = game.rock_band_stage_future(&stage);
        assert!(stage_future > 3.0 * game.rock_band_stage_future(&plain));
        let mut signed = stage.clone();
        signed.album_sales = 500;
        assert!(game.rock_band_stage_future(&signed) > stage_future);
    }

    #[test]
    fn the_rival_holding_the_culture_bar_is_the_better_audience() {
        let (mut game, band) = world_with_theaters();
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("glam_rock"));
        let near = game.rock_concert_stage_value(0, band, (8, 6)).unwrap();
        game.players[1].culture_lifetime = 50_000.0;
        let holding = game.rock_concert_stage_value(0, band, (8, 6)).unwrap();
        assert!(holding > near * 1.5, "{holding} vs {near}");
    }

    #[test]
    fn a_band_with_a_stage_tours_the_stage_over_a_nearer_wonder() {
        let (mut game, band) = world_with_theaters();
        game.units
            .get_mut(&band)
            .unwrap()
            .promotions
            .insert(name!("glam_rock"));
        let host = game.map.get((8, 6)).unwrap().owner_city.unwrap();
        // A wonder at the door and a stage down the road.
        let tile = game.map.tiles.get_mut(&(7, 7)).unwrap();
        tile.owner_city = Some(host);
        tile.wonder = Some(name!("pyramids"));
        assert_eq!(
            BasicAi::rock_band_stage_destination(&game, 0, band),
            Some((8, 6))
        );
    }
}
