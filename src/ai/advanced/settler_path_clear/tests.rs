use super::*;
use crate::world::TileBits;

/// Two majors at war on flat, explored ground; our capital at `home`, a
/// Settler two tiles out on a road two more tiles to its site (four from home).
fn opening() -> (Game, u32, Pos, Pos, Pos) {
    let mut game = Game::new_full(2, 30, 20, 8_121, 120, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let pos = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, pos, None);
    }
    for unit in game
        .player_unit_ids(0)
        .into_iter()
        .chain(game.player_unit_ids(1))
    {
        game.remove_unit(unit);
    }
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in positions {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        tile.improvement = None;
        game.players[0].explored.insert(position);
    }
    game.barb_camps.clear();
    game.current = 0;
    game.turn = 22;
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    let enemy_home = game.cities[&game.player_city_ids(1)[0]].pos;
    // The site lies four tiles from home on the side away from the rival.
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| game.wdist(*p, home) == 4)
        .collect();
    sites.sort_by_key(|p| (std::cmp::Reverse(game.wdist(*p, enemy_home)), *p));
    let site = sites[0];
    let mut mids: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| game.wdist(*p, home) == 2 && game.wdist(*p, site) == 2)
        .collect();
    mids.sort_unstable();
    let settler = game.spawn_test_unit("settler", 0, mids[0]);
    (game, settler, home, mids[0], site)
}

fn tile_far_from(game: &Game, avoid: &[Pos], min: i32) -> Pos {
    let mut far: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| avoid.iter().all(|a| game.wdist(*p, *a) >= min))
        .collect();
    far.sort_unstable();
    far[0]
}

fn all_seen(game: &Game) -> TileBits {
    TileBits::all(game.map.tiles.len())
}

#[test]
fn a_road_with_nothing_near_it_is_clear() {
    let (game, _settler, _home, here, site) = opening();
    let mut ai = AdvancedAi::new();
    ai.enable_settler_marches_alone_when_clear();
    assert!(ai.road_is_clear(&game, 0, here, site, 2, &all_seen(&game)));
}

#[test]
fn a_hostile_within_four_tiles_of_the_road_keeps_the_holds() {
    let (mut game, _settler, home, here, site) = opening();
    let ai = AdvancedAi::new();
    // Three tiles beyond the site, off the road: inside the radius.
    let mut near: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| game.wdist(*p, site) == 3 && game.wdist(*p, home) > 4)
        .collect();
    near.sort_unstable();
    game.spawn_test_unit("warrior", 1, near[0]);
    assert!(!ai.road_is_clear(&game, 0, here, site, 2, &all_seen(&game)));

    // A hostile well outside the radius of every road tile does not.
    let (mut game, _settler, _home, here, site) = opening();
    let far = tile_far_from(&game, &[here, site], 7);
    game.spawn_test_unit("warrior", 1, far);
    assert!(ai.road_is_clear(&game, 0, here, site, 2, &all_seen(&game)));
}

#[test]
fn a_known_camp_or_scarred_ground_near_the_road_keeps_the_holds() {
    let (mut game, _settler, _home, here, site) = opening();
    let ai = AdvancedAi::new();
    let mut near: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| game.wdist(*p, site) == 2 && game.wdist(*p, here) == 4)
        .collect();
    near.sort_unstable();
    game.barb_camps.insert(near[0], 0);
    assert!(!ai.road_is_clear(&game, 0, here, site, 2, &all_seen(&game)));

    let (game, _settler, _home, here, site) = opening();
    let mut scarred = AdvancedAi::new();
    scarred.settler_capture_scars.insert(site, 10);
    assert!(!scarred.road_is_clear(&game, 0, here, site, 2, &all_seen(&game)));
}

#[test]
fn a_road_into_fog_keeps_the_holds() {
    let (game, _settler, _home, here, site) = opening();
    let ai = AdvancedAi::new();
    let mut seen = TileBits::with_capacity(game.map.tiles.len());
    for pos in game.map.tiles.keys() {
        if *pos != site {
            seen.insert(game.map.tiles.index_of(*pos).unwrap());
        }
    }
    assert!(!ai.road_is_clear(&game, 0, here, site, 2, &seen));
}

#[test]
fn fog_beyond_this_turns_steps_does_not_keep_the_holds() {
    let (game, _settler, _home, here, _site) = opening();
    let ai = AdvancedAi::new();
    let mut far: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|p| game.wdist(*p, here) == 4)
        .collect();
    far.sort_unstable();
    let site = far[0];
    let mut seen = TileBits::with_capacity(game.map.tiles.len());
    for pos in game.map.tiles.keys() {
        if game.wdist(*pos, here) <= SETTLER_ALONE_SIGHT_AHEAD {
            seen.insert(game.map.tiles.index_of(*pos).unwrap());
        }
    }
    assert!(ai.road_is_clear(&game, 0, here, site, 2, &seen));
}

#[test]
fn past_the_opening_or_with_the_gene_off_nothing_changes() {
    let (mut game, settler, _home, _here, site) = opening();
    let mut off = AdvancedAi::new();
    off.settler_targets.insert(settler, site);
    assert_eq!(off.settler_path_clear(&game, 0, settler), None);

    let mut on = AdvancedAi::new();
    on.enable_settler_marches_alone_when_clear();
    on.settler_targets.insert(settler, site);
    game.turn = SETTLER_ALONE_LAST_TURN + 1;
    assert_eq!(on.settler_path_clear(&game, 0, settler), None);
}

/// The guard hold itself: a visible raider six tiles off engages the escort
/// pacing and the guard is two tiles behind, so the stock seat waits; with
/// the gene the raider is outside four tiles of the road, which is all in
/// sight, and the Settler marches.
#[test]
fn the_guard_wait_gives_way_on_a_clear_road() {
    let build = || {
        let (mut game, settler, home, here, site) = opening();
        // Guard two tiles behind the settler, on open ground.
        let mut behind: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| {
                game.wdist(*p, here) == 2 && game.wdist(*p, site) == 4 && game.city_at(*p).is_none()
            })
            .collect();
        behind.sort_unstable();
        let guard = game.spawn_test_unit("warrior", 0, behind[0]);
        game.units.get_mut(&guard).unwrap().moves_left = 0.0;
        // A raider six or seven tiles from the settler, five or more from
        // every tile of the road, kept in sight by a scout beside it.
        let mut raider_at: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| {
                let d = game.wdist(*p, here);
                (6..=7).contains(&d)
                    && game.wdist(*p, here) + game.wdist(*p, site)
                        > game.wdist(here, site) + 2 * SETTLER_ALONE_CLEAR_RADIUS
                    && game.wdist(*p, home) > 3
            })
            .collect();
        raider_at.sort_unstable();
        let raider = game.spawn_test_unit("warrior", 1, raider_at[0]);
        let mut beside: Vec<Pos> = game
            .nbrs(raider_at[0])
            .into_iter()
            .filter(|p| game.map.get(*p).is_some() && game.wdist(*p, here) >= 5)
            .collect();
        beside.sort_unstable();
        let scout = game.spawn_test_unit("scout", 0, beside[0]);
        game.units.get_mut(&scout).unwrap().moves_left = 0.0;
        (game, settler, guard, raider, site)
    };
    for gene in [false, true] {
        let (mut game, settler, guard, raider, site) = build();
        let mut ai = AdvancedAi::new();
        ai.enable_live_formationless_settler_shadow();
        if gene {
            ai.enable_settler_marches_alone_when_clear();
        }
        ai.settler_targets.insert(settler, site);
        ai.bind_settler_guard(&game, settler, guard);
        assert!(
            game.unit_visible_to(raider, 0),
            "the scout keeps the raider in sight"
        );
        let journal = crate::reasoning::Journal::recording();
        ai.attach_journal(journal.handle());
        let paced = ai.stacked_escort_pace(&mut game, 0, settler);
        let headlines: Vec<String> = journal
            .since(0)
            .thoughts
            .iter()
            .map(|thought| thought.headline.clone())
            .collect();
        if gene {
            assert_eq!(
                paced, None,
                "the clear road hands the settler to its march: {headlines:?}"
            );
            assert!(
                headlines
                    .iter()
                    .any(|h| h.starts_with("Settler marches alone: path clear")),
                "{headlines:?}"
            );
            assert!(
                ai.settler_guards.contains_key(&settler),
                "the guard stays assigned"
            );
        } else {
            assert!(
                headlines
                    .iter()
                    .any(|h| h.starts_with("Settler waits for its guard")),
                "the stock seat waits for the guard: {headlines:?}"
            );
        }
    }
}
