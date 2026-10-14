//! `gun-queue-follows-the-road`: on the no-route call the queue step of
//! `gun-queues-behind-the-column` follows the router's road when its length
//! to the post falls, not only when the hex distance does, may land two
//! tiles from a city that cannot strike, and a gun short of moves for its
//! next tile says so. Traced replays at 851192f2e (Tula,
//! civvis-20261014T080351Z, turns 136-141): Bombards with full moves stood
//! because the router's first step went round terrain or our column at an
//! equal or greater hex distance.

use super::tests::{at_distance, walled_city};
use super::*;

/// `walled_city` on open grassland, at war; with `walls` false the city has
/// no wall pool and cannot strike.
fn open_city(walls: bool) -> (Game, u32) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    if !walls {
        g.cities.get_mut(&cid).unwrap().wall_hp = 0;
        std::sync::Arc::make_mut(&mut g.observed_city_max_wall_hp).insert(cid, 0);
        if let Some(memory) = g.players[0].remembered_cities.get_mut(&cid) {
            memory.wall_hp = 0;
        }
        assert_eq!(
            g.city_max_wall_hp(&g.cities[&cid]),
            0,
            "fixture: no wall pool"
        );
    } else {
        assert!(g.cities[&cid].wall_hp > 0, "fixture: walls stand");
    }
    (g, cid)
}

/// A siege train AI with the column queue, and the road gene as asked.
fn train(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_shared_danger();
    ai.enable_gun_queues_behind_the_column();
    if gene {
        ai.enable_gun_queue_follows_the_road();
    }
    ai
}

/// The siege in Invest with `gun` posted on `post`.
fn investing(ai: &mut AdvancedAi, cid: u32, gun: u32, post: Pos) {
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Invest,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: [(gun, post)].into_iter().collect(),
            short_since: None,
        },
    );
}

/// A walled road from a firing post two tiles from the city: a corridor of
/// [`ROAD`] tiles beyond the post, none beside another but its neighbours on
/// the road, and a pocket tile `gun` beside the road's last tile alone, whose
/// hex distance to the post compares to that tile's as `order` asks. Every
/// other tile beyond the city's first ring is a mountain, so the gun's only
/// road is the corridor, and a Warrior of ours stands two road tiles ahead of
/// its first step: the train's router treats it as a wall, and nothing of
/// ours stands beside the gun to cross.
fn pocket(g: &mut Game, cid: u32, order: std::cmp::Ordering) -> (u32, Pos, Vec<Pos>) {
    const ROAD: usize = 5;
    let city = g.cities[&cid].pos;
    let open = |g: &Game, pos: Pos| {
        g.map
            .get(pos)
            .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
    };
    // Depth-first over induced paths: each new tile three or more from the
    // city, beside the previous tile and no earlier one.
    fn extend(
        g: &Game,
        city: Pos,
        road: &mut Vec<Pos>,
        open: &dyn Fn(&Game, Pos) -> bool,
        found: &mut dyn FnMut(&[Pos]) -> bool,
    ) -> bool {
        if road.len() == ROAD + 1 {
            return found(road);
        }
        let last = *road.last().unwrap();
        let mut next: Vec<Pos> = g.nbrs(last).into_iter().collect();
        next.sort_unstable();
        for pos in next {
            if g.wdist(pos, city) < 3
                || !open(g, pos)
                || road[..road.len() - 1]
                    .iter()
                    .any(|tile| g.wdist(*tile, pos) <= 1)
            {
                continue;
            }
            road.push(pos);
            if extend(g, city, road, open, found) {
                return true;
            }
            road.pop();
        }
        false
    }
    let mut chosen: Option<(Vec<Pos>, Pos)> = None;
    for post in at_distance(g, cid, 2) {
        let mut road = vec![post];
        let mut found = |road: &[Pos]| {
            let last = road[ROAD];
            let mut sides: Vec<Pos> = g.nbrs(last).into_iter().collect();
            sides.sort_unstable();
            let pick = sides.into_iter().find(|pos| {
                open(g, *pos)
                    && g.wdist(*pos, city) >= 3
                    && road[..ROAD].iter().all(|tile| g.wdist(*tile, *pos) > 1)
                    && g.wdist(last, post).cmp(&g.wdist(*pos, post)) == order
                    && ROAD < usize::try_from(g.wdist(*pos, post)).unwrap() + ROAD_DETOUR_SLACK
            });
            if let Some(start) = pick {
                chosen = Some((road.to_vec(), start));
                return true;
            }
            false
        };
        if extend(g, city, &mut road, &open, &mut found) {
            break;
        }
    }
    let (road, start) = chosen.expect("fixture: a walled road with a pocket");
    for tile in g.map.tiles.values_mut() {
        let pos = tile.pos;
        let near_city = {
            let (dq, dr) = (pos.0 - city.0, pos.1 - city.1);
            (dq.abs() + dr.abs() + (dq + dr).abs()) / 2 <= 1
        };
        if !near_city && !road.contains(&pos) && pos != start {
            tile.terrain = crate::name!("mountain");
        }
    }
    g.spawn_unit("warrior", 0, road[ROAD - 2]);
    let gun = g.spawn_unit("catapult", 0, start);
    (gun, road[0], road)
}

#[test]
fn a_gun_whose_road_bends_away_steps_onto_it_only_under_the_gene() {
    // The pocket is nearer the post as the crow flies than the lane tile
    // beside it: the road's first step is a hex step away from the post.
    for gene in [false, true] {
        let (mut g, cid) = open_city(true);
        let (gun, post, road) = pocket(&mut g, cid, std::cmp::Ordering::Greater);
        let start = g.units[&gun].pos;
        let step = *road.last().unwrap();
        assert!(
            g.wdist(step, post) > g.wdist(start, post),
            "fixture: the road bends away"
        );
        let mut ai = train(gene);
        investing(&mut ai, cid, gun, post);
        let city = CityView::of(&g, cid).unwrap();
        let moved = ai.post_step(&mut g, 0, gun, &city);
        if gene {
            assert_eq!(moved, Some(true), "the gun follows its road");
            assert!(
                road.contains(&g.units[&gun].pos),
                "onto the road, as far as its moves reach"
            );
        } else {
            assert_eq!(moved, None, "off, the hex-distance test refuses the bend");
            assert_eq!(g.units[&gun].pos, start);
        }
    }
}

#[test]
fn a_gun_whose_road_runs_level_steps_onto_it_only_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid) = open_city(true);
        let (gun, post, road) = pocket(&mut g, cid, std::cmp::Ordering::Equal);
        let start = g.units[&gun].pos;
        let step = *road.last().unwrap();
        assert_eq!(
            g.wdist(step, post),
            g.wdist(start, post),
            "fixture: a level step"
        );
        let mut ai = train(gene);
        investing(&mut ai, cid, gun, post);
        let city = CityView::of(&g, cid).unwrap();
        let moved = ai.post_step(&mut g, 0, gun, &city);
        assert_eq!(moved, gene.then_some(true));
        if gene {
            assert!(road.contains(&g.units[&gun].pos), "onto the road");
        } else {
            assert_eq!(g.units[&gun].pos, start);
        }
    }
}

#[test]
fn the_free_route_call_keeps_the_hex_distance_test() {
    // The call made when a free route exists but leads away keeps the old
    // rule: the column's road must be nearer as the crow flies.
    let (mut g, cid) = open_city(true);
    let (gun, post, _) = pocket(&mut g, cid, std::cmp::Ordering::Greater);
    let ai = train(true);
    let city = g.cities[&cid].pos;
    assert!(ai.column_queue_step(&g, 0, gun, post, city, true).is_some());
    assert_eq!(ai.column_queue_step(&g, 0, gun, post, city, false), None);
}

/// A gun three tiles out with its post two out beside it, on open ground.
fn three_out(g: &mut Game, cid: u32) -> (u32, Pos) {
    let city = g.cities[&cid].pos;
    let (start, post) = at_distance(g, cid, 2)
        .into_iter()
        .find_map(|post| {
            g.nbrs(post)
                .into_iter()
                .find(|start| g.wdist(*start, city) == 3)
                .map(|start| (start, post))
        })
        .expect("fixture: a start three out beside a post two out");
    (g.spawn_unit("catapult", 0, start), post)
}

#[test]
fn before_an_unwalled_city_the_queue_step_may_land_on_a_firing_stand() {
    for gene in [false, true] {
        let (mut g, cid) = open_city(false);
        let (gun, post) = three_out(&mut g, cid);
        let ai = train(gene);
        let city = g.cities[&cid].pos;
        let step = ai.column_queue_step(&g, 0, gun, post, city, true);
        assert_eq!(step, gene.then_some(post), "gene {gene}");
    }
}

#[test]
fn before_a_walled_city_the_queue_step_stays_out_of_its_reach() {
    for gene in [false, true] {
        let (mut g, cid) = open_city(true);
        let (gun, post) = three_out(&mut g, cid);
        let ai = train(gene);
        let city = g.cities[&cid].pos;
        assert_eq!(
            ai.column_queue_step(&g, 0, gun, post, city, true),
            None,
            "gene {gene}"
        );
    }
}

#[test]
fn a_walled_encampment_still_counts_as_a_striker() {
    let (mut g, cid) = open_city(false);
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.encampment_hp = 100;
        city.encampment_wall_hp = 100;
        city.encampment_pillaged = false;
    }
    let pos = g.cities[&cid].pos;
    let has_camp = g
        .city_district_family_position(&g.cities[&cid], crate::name!("encampment"))
        .is_some();
    // Without an Encampment district on the board the city still cannot
    // strike; with one, a walled unpillaged camp shoots in its place.
    assert_eq!(city_cannot_strike(&g, pos), !has_camp);
    let walled = {
        let (g, cid) = open_city(true);
        let pos = g.cities[&cid].pos;
        city_cannot_strike(&g, pos)
    };
    assert!(!walled, "a standing wall strikes");
}

#[test]
fn a_gun_short_of_moves_for_its_next_tile_says_so_only_under_the_gene() {
    let run = |gene: bool| -> Vec<String> {
        let (mut g, cid) = open_city(true);
        let city = g.cities[&cid].pos;
        let post = at_distance(&g, cid, 2)[0];
        // Four out on a line to the post, the tile ahead a hill.
        let mut lane = vec![post];
        while lane.len() < 4 {
            let last = *lane.last().unwrap();
            let next = g
                .nbrs(last)
                .into_iter()
                .filter(|pos| g.wdist(*pos, city) == g.wdist(last, city) + 1)
                .min()
                .expect("a tile one further out");
            lane.push(next);
        }
        for pos in g.nbrs(lane[3]) {
            if let Some(tile) = g.map.tiles.get_mut(&pos) {
                tile.hills = true;
            }
        }
        let gun = g.spawn_unit("catapult", 0, lane[3]);
        g.units.get_mut(&gun).unwrap().moves_left = 1.0;
        assert!(
            g.unit_max_moves(gun) > 1.0,
            "fixture: part of the moves spent"
        );
        let mut ai = train(gene);
        investing(&mut ai, cid, gun, post);
        let journal = crate::reasoning::Journal::recording();
        crate::ai::Ai::attach_journal(&mut ai, journal.handle());
        let view = CityView::of(&g, cid).unwrap();
        assert_eq!(
            ai.post_step(&mut g, 0, gun, &view),
            None,
            "the gun cannot step"
        );
        assert_eq!(g.units[&gun].pos, lane[3]);
        journal
            .since(0)
            .thoughts
            .iter()
            .filter(|t| t.headline.contains("stops short of its post"))
            .map(|t| t.detail.clone())
            .collect()
    };
    let off = run(false);
    assert!(
        off.iter()
            .any(|d| d.ends_with("no route step and no pass-through destination")),
        "{off:#?}"
    );
    let on = run(true);
    assert!(
        on.iter()
            .any(|d| d.ends_with("out of moves for the next tile")),
        "{on:#?}"
    );
    assert!(!on.iter().any(|d| d.contains("no route step")), "{on:#?}");
}
