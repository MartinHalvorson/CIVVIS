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

fn stage_counts(g: &Game, pid: usize) -> StageCounts {
    let mut counts = StageCounts::default();
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
        {
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
        let counts = stage_counts(g, pid);
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
