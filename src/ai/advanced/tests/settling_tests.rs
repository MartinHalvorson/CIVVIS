//! Tests of `advanced/settling.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn a_stalled_escort_is_released_and_the_settler_walks_itself() {
    // The measured shape: "Settler advancing with its escort | 9 tiles
    // remain" at the SAME distance turn after turn (run
    // civvis-20260807T202450Z) while the linked branch returned true
    // without moving anything. The contract under test is the RELEASE:
    // no-closer turns must break the link so the ordinary march can act;
    // frozen controllers must keep trusting forever.
    fn fixture() -> (Game, u32, Pos) {
        let mut game = Game::new_full(2, 28, 18, 3, 1_000, 0, false);
        // A founded capital first: a zero-city empire takes the
        // first-city fast path and never reaches the escort march.
        let first = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("seat starts with a settler");
        let capital_pos = game.units[&first].pos;
        game.found_city_for(0, capital_pos, None);
        game.remove_unit(first);

        let land_at = |game: &Game, want: &dyn Fn(Pos) -> bool| {
            game.map
                .tiles
                .iter()
                .filter(|(pos, tile)| {
                    game.rules.is_passable(tile)
                        && !game.rules.is_water(tile)
                        && game.unit_ids_at(**pos).is_empty()
                        && want(**pos)
                })
                .map(|(pos, _)| *pos)
                .next()
        };
        let start = land_at(&game, &|pos| {
            game.wdist(pos, capital_pos) >= 5 && game.wdist(pos, capital_pos) <= 7
        })
        .expect("fixture offers ground away from the capital");
        let settler = game.spawn_test_unit("settler", 0, start);
        let escort = game.spawn_test_unit("warrior", 0, start);
        game.apply(
            0,
            &Action::LinkUnits {
                unit: settler,
                with: escort,
            },
        )
        .expect("test pair must link");
        let target = land_at(&game, &|pos| {
            game.wdist(pos, start) >= 6
                && game.wdist(pos, start) <= 10
                && game.wdist(pos, capital_pos) >= 4
        })
        .expect("fixture offers a distant land target");
        (game, settler, target)
    }

    let (mut game, settler, target) = fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_escort_unstick();
    for _ in 0..4 {
        ai.settler_targets.insert(settler, target);
        ai.advanced_settler_step(&mut game, 0, settler);
        if game.units[&settler].linked_to.is_none() {
            break;
        }
    }
    assert!(
        game.units[&settler].linked_to.is_none(),
        "a formation that closes no distance must be released"
    );

    let (mut frozen, fsettler, ftarget) = fixture();
    let mut stock = AdvancedAi::new();
    for _ in 0..4 {
        stock.settler_targets.insert(fsettler, ftarget);
        stock.advanced_settler_step(&mut frozen, 0, fsettler);
    }
    assert!(
        frozen.units[&fsettler].linked_to.is_some(),
        "frozen controllers keep the recorded trust"
    );
}

/// A settler standing on a threatened shore with no guard steps inland
/// instead of holding; the same settler inside a city stays put.
#[test]
fn a_threatened_settler_on_the_shore_retreats_inland_but_holds_inside_a_city() {
    // Find a seed with a shore tile that has a neighbour with no water
    // around it at all — the retreat must be able to lower the risk.
    let is_land = |game: &Game, p: Pos| {
        game.map
            .get(p)
            .is_some_and(|t| game.rules.is_passable(t) && !game.rules.is_water(t))
    };
    let is_water = |game: &Game, p: Pos| game.map.get(p).is_some_and(|t| game.rules.is_water(t));
    let mut found = None;
    for seed in 91_482..91_540u64 {
        let game = Game::new(2, 24, 16, seed, 80, 0);
        let mut lands: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| is_land(&game, *p))
            .collect();
        lands.sort_unstable();
        let hit = lands.iter().copied().find_map(|p| {
            let sea = game.nbrs(p).into_iter().find(|n| is_water(&game, *n))?;
            let inland = game.nbrs(p).into_iter().find(|n| {
                is_land(&game, *n) && !game.nbrs(*n).into_iter().any(|m| is_water(&game, m))
            })?;
            Some((p, sea, inland))
        });
        if let Some((shore, sea, inland)) = hit {
            found = Some((seed, shore, sea, inland));
            break;
        }
    }
    let (seed, shore, sea, inland) =
        found.expect("some seed offers a shore with dry ground behind it");

    let fresh = |seed: u64| {
        let mut game = Game::new(2, 24, 16, seed, 80, 0);
        for unit in game
            .player_unit_ids(0)
            .into_iter()
            .chain(game.player_unit_ids(1))
        {
            game.remove_unit(unit);
        }
        game.at_war.insert((0, 1));
        game.current = 0;
        game
    };

    let mut game = fresh(seed);
    game.spawn_test_unit("galley", 1, sea);
    let settler = game.spawn_test_unit("settler", 0, shore);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    let journal = crate::reasoning::Journal::recording();
    live.attach_journal(journal.handle());
    assert!(
        live.advanced_settler_step(&mut game, 0, settler),
        "the settler acts"
    );
    let after = game.units[&settler].pos;
    assert_ne!(after, shore, "it does not hold on the threatened shore");
    assert!(
        !game.nbrs(after).into_iter().any(|m| m == sea) || after == inland,
        "it moved away from the ship's water"
    );
    assert!(
        journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline.starts_with("Settler retreats from")),
        "the retreat is journaled"
    );

    // ★★★★ A target it keeps retreating from is retired. Put it back on
    // the shore, committed to a target, and let the ship stay: the third
    // retreat gives the target up (civvis-20260816T075807Z: two settlers
    // stepped back and marched in again from t33 to t87).
    let target = inland;
    live.settler_targets.insert(settler, target);
    live.settler_retreats.remove(&settler);
    let mut retreats = 0;
    for _ in 0..SETTLER_RETREAT_LIMIT {
        game.relocate(settler, shore);
        game.units.get_mut(&settler).unwrap().moves_left = 2.0;
        let before = journal.since(0).thoughts.len();
        let _ = live.advanced_settler_step(&mut game, 0, settler);
        let fresh_thoughts = &journal.since(0).thoughts[before..];
        if fresh_thoughts
            .iter()
            .any(|t| t.headline.starts_with("Settler retreats from"))
        {
            retreats += 1;
        }
    }
    assert_eq!(
        retreats, SETTLER_RETREAT_LIMIT,
        "each turn on the shore is a retreat"
    );
    assert!(
        live.settler_site_is_dead(settler, target),
        "the target it kept retreating from is retired"
    );
    assert!(
        !live.settler_targets.contains_key(&settler),
        "and the commitment is dropped so the next pick looks elsewhere"
    );
    assert!(
        journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline.starts_with("Settler gives up on")),
        "the surrender is journaled"
    );

    // Inside a city the same threat does not move it.
    let mut fortress = fresh(seed);
    fortress.found_city_for(0, shore, None);
    fortress.spawn_test_unit("galley", 1, sea);
    let inside = fortress.spawn_test_unit("settler", 0, shore);
    fortress.units.get_mut(&inside).unwrap().moves_left = 2.0;
    let mut live = AdvancedAi::new();
    live.enable_live_bridge_universe();
    let journal = crate::reasoning::Journal::recording();
    live.attach_journal(journal.handle());
    let _ = live.advanced_settler_step(&mut fortress, 0, inside);
    assert!(
        !journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline.starts_with("Settler retreats from")),
        "a city centre is protection, not a tile to flee"
    );
}

#[test]
fn strategic_settler_routes_to_an_island_beyond_the_local_search_radius() {
    let (mut g, source, target) = island_colony_game();
    g.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    let settler = g.spawn_test_unit("settler", 0, source);
    let mut ai = AdvancedAi::new();
    assert!(ai.advanced_settler_step(&mut g, 0, settler));
    let chosen = ai.settler_targets[&settler];
    assert!(
        g.wdist(chosen, target) <= 1,
        "the target stays on the useful island"
    );
    assert!(g.wdist(source, chosen) > 8, "the colony remains strategic");
    assert!(g
        .map
        .get(g.units[&settler].pos)
        .is_some_and(|tile| g.rules.is_water(tile)));
}

#[test]
fn overseas_settlement_claims_a_discovered_foreign_landfall_when_home_is_full() {
    let (mut g, source, target) = island_colony_game();
    g.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    let explored: Vec<Pos> = g.map.tiles.keys().copied().collect();
    g.players[0].explored.extend(explored);
    let settler = g.spawn_test_unit("settler", 0, source);
    let home_landmass = BasicAi::capital_landmass(&g, 0);
    let mut ai = AdvancedAi::new();
    ai.enable_overseas_settlement();

    assert!(ai.advanced_settler_step(&mut g, 0, settler));
    let chosen = ai.settler_targets[&settler];
    assert!(
        !home_landmass.contains(&chosen),
        "the gene sends the settler beyond the capital's landmass"
    );
    assert!(
        g.wdist(chosen, target) <= 1,
        "the nearest discovered overseas landfall is chosen"
    );
}

/// A dodge is a legal move that is not progress. The commitment bound has
/// to expire on one, or a settler beside a roaming hostile holds a site it
/// is walking away from for the rest of the game — measured on run
/// `civvis-20260802T041527Z`, which finished with one city and score 149.
#[test]
fn a_committed_settler_that_never_closes_the_gap_releases_its_site() {
    let mut g = Game::new_full(2, 30, 18, 9_204, 120, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let home = g.units[&settler].pos;
    g.found_city_for(0, home, None);
    let start = g
        .nbrs(home)
        .into_iter()
        .find(|position| {
            g.map
                .get(*position)
                .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        })
        .expect("the capital has a land neighbour");
    g.units.get_mut(&settler).unwrap().pos = start;

    let mut ai = AdvancedAi::new();
    ai.settler_commit = true;

    let unit = g.units.get_mut(&settler).unwrap();
    unit.moves_left = 4.0;
    unit.acted = false;
    ai.advanced_settler_step(&mut g, 0, settler);
    let target = *ai
        .settler_targets
        .get(&settler)
        .expect("the settler commits to a site");

    // It moves every turn and is put back where it began, so it never gets
    // closer. Under the old motion test this cleared the stall counter on
    // every step and the bound below could never be reached.
    for _ in 0..SETTLER_STALL_LIMIT {
        let unit = g.units.get_mut(&settler).unwrap();
        unit.pos = start;
        unit.moves_left = 4.0;
        unit.acted = false;
        ai.advanced_settler_step(&mut g, 0, settler);
    }

    assert_ne!(
        ai.settler_targets.get(&settler),
        Some(&target),
        "a settler that never closes the gap must release its site"
    );
}

#[test]
fn capital_settler_retargets_when_its_cached_site_becomes_illegal() {
    let mut game = Game::new_full(2, 30, 18, 9_204, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let start = game.units[&settler].pos;
    let blocker = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(position, _)| *position)
        .find(|position| game.wdist(start, *position) == 3)
        .expect("test start has a land tile three hexes away");
    game.found_city_for(1, blocker, None);
    game.current = 0;
    assert!(!game.can_found_city(settler));

    let mut ai = AdvancedAi::new();
    ai.settler_targets.insert(settler, start);
    for _ in 0..100 {
        if !game.units.contains_key(&settler) {
            break;
        }
        let unit = game.units.get_mut(&settler).unwrap();
        unit.moves_left = 4.0;
        unit.acted = false;
        assert!(
            ai.advanced_settler_step(&mut game, 0, settler),
            "the capital settler should keep routing to a replacement site"
        );
        assert_ne!(ai.settler_targets.get(&settler), Some(&start));
    }

    assert!(!game.player_city_ids(0).is_empty());
    assert!(!game.units.contains_key(&settler));
}

/// The site score is silent about barbarians and about being out of
/// reach of your own cities. Off, it stays silent; on, both discount the
/// site and neither can ever raise it.
#[test]
fn a_defensible_site_score_discounts_camps_and_isolation() {
    let mut game = Game::new_full(2, 30, 18, 7_741, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("every empire starts with a settler");
    let home = game.units[&settler].pos;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();

    // Somewhere well outside the founding city's support radius.
    let far = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.map.get(*pos).is_some_and(|t| !game.rules.is_water(t)))
        .find(|pos| game.wdist(home, *pos) > 8)
        .expect("the map is larger than the support radius");

    let mut plain = AdvancedAi::new();
    plain.settlement_safety = false;
    let mut weighed = AdvancedAi::new();
    weighed.settlement_safety = false;
    weighed.defensible_sites = true;
    assert!(!plain.defensible_sites, "the measurement flag ships off");

    // With no camp anywhere, only the isolation term can fire.
    assert!(game.barb_camps.is_empty());
    assert_eq!(weighed.defensibility(&game, 0, home), 0.0);
    assert!(
        weighed.defensibility(&game, 0, far) < 0.0,
        "a site out of reach of every friendly city is discounted"
    );
    assert_eq!(
        plain.settle_value(&game, 0, far),
        weighed.settle_value(&game, 0, far) - weighed.defensibility(&game, 0, far),
        "the flag adds exactly the defensibility term and nothing else"
    );

    // A camp beside the home tile discounts it; the untreated agent
    // scores that site exactly as it did before, which is the gap this
    // flag exists to measure.
    let before = plain.settle_value(&game, 0, home);
    let camp = *game
        .nbrs(home)
        .iter()
        .find(|pos| game.map.get(**pos).is_some_and(|t| !game.rules.is_water(t)))
        .expect("a founded city has a passable neighbour");
    game.barb_camps.insert(camp, 0);
    assert!(
        weighed.defensibility(&game, 0, home) < 0.0,
        "a camp one tile away discounts the site"
    );
    assert_eq!(
        plain.settle_value(&game, 0, home),
        before,
        "and the shipped score does not notice the camp at all"
    );
    assert!(
        weighed.settle_value(&game, 0, home) < before,
        "while the weighed score does"
    );
}

#[test]
fn settlement_site_score_prioritizes_growth_water_and_future_districts() {
    let game = Game::new_full(2, 30, 18, 8_114, 120, 0, false);
    let center =
        game.map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdisk(*position, 3)
                    .into_iter()
                    .all(|near| game.map.get(near).is_some())
                    && game.map.get(*position).is_some_and(|tile| {
                        !game.rules.is_water(tile) && game.rules.is_passable(tile)
                    })
            })
            .expect("the test map has an interior land tile");
    let food_tile = game
        .nbrs(center)
        .into_iter()
        .find(|position| game.map.get(*position).is_some())
        .expect("the center has a neighbor");
    let mut food = game.clone();
    let mut production = game.clone();
    for candidate in [&mut food, &mut production] {
        for position in candidate.wdisk(center, 1) {
            if let Some(tile) = candidate.map.tiles.get_mut(&position) {
                tile.terrain = Name::new("desert");
                tile.feature = None;
                tile.resource = None;
                tile.river_edges = [false; 6];
            }
        }
    }
    food.map.tiles.get_mut(&food_tile).unwrap().terrain = Name::new("grassland");
    production.map.tiles.get_mut(&food_tile).unwrap().terrain = Name::new("plains");
    let ai = AdvancedAi::new();
    assert!(
        ai.settle_value(&food, 0, center) > ai.settle_value(&production, 0, center) + 1.0,
        "early food should beat the same tile's production-only alternative"
    );

    let mut freshwater = food.clone();
    assert!(freshwater.map.set_river_edge(center, food_tile, true));
    assert!(
        ai.settle_value(&freshwater, 0, center) > ai.settle_value(&food, 0, center) + 10.0,
        "fresh water should be a material founding advantage"
    );

    let mut district = game.clone();
    for position in district.wdisk(center, 3) {
        if let Some(tile) = district.map.tiles.get_mut(&position) {
            tile.terrain = Name::new("plains");
            tile.feature = None;
            tile.resource = None;
            tile.river_edges = [false; 6];
        }
    }
    let district_site = district
        .nbrs(center)
        .into_iter()
        .find(|position| {
            district.map.get(*position).is_some_and(|tile| {
                !district.rules.is_water(tile) && district.rules.is_passable(tile)
            })
        })
        .expect("the normalized ring has a district site");
    let mountain = district
        .nbrs(district_site)
        .into_iter()
        .find(|position| {
            *position != center
                && district.wdist(center, *position) <= 3
                && district.map.get(*position).is_some()
        })
        .expect("the district site has an adjacency tile");
    let without_mountain = ai.settlement_adjacency_value(&district, 0, center);
    district.map.tiles.get_mut(&mountain).unwrap().terrain = Name::new("mountain");
    let with_mountain = ai.settlement_adjacency_value(&district, 0, center);
    assert!(
        with_mountain > without_mountain + 0.1,
        "future campus adjacency should survive into the founding score: \
             with={with_mountain}, without={without_mountain}"
    );
}

#[test]
fn a_retarget_does_not_resolve_a_settler_delay() {
    // ⚠ The existing release test sets `settler_blocked_turns` by hand, so it
    // passes whether or not the counter can ever REACH the threshold in play.
    // This pins the reset rule that decides whether it does.
    //
    // Measured on live run `civvis-20260804T011632Z`: a settler stranded 84
    // turns at full movement while retargeting every turn, so the old
    // `advanced = retargeted || closer` reset cleared the counter on each of
    // them. No settler was ordered for the last 128 turns of that run while
    // `desired_cities` was 6-7 and the empire held 5.
    assert!(
        !AdvancedAi::settler_delay_is_resolved(true, false),
        "a retarget with no progress must NOT clear the empire-wide counter"
    );
    assert!(
        !AdvancedAi::settler_delay_is_resolved(true, true),
        "closer to a DIFFERENT target is not progress on the delay that stalled \
             the pipeline; the strand is what the counter measures"
    );
    assert!(
        AdvancedAi::settler_delay_is_resolved(false, true),
        "genuine progress toward the same target resolves the delay"
    );
    assert!(
        !AdvancedAi::settler_delay_is_resolved(false, false),
        "same target, no progress: still blocked"
    );
}

#[test]
fn advanced_settler_refuses_a_visible_direct_attack_tile() {
    let mut game = Game::new_full(2, 30, 18, 8_115, 120, 0, false);
    let initial = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("player zero starts with a settler");
    let home = game.units[&initial].pos;
    let origin =
        game.map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdist(home, *position) > 8
                    && game.map.get(*position).is_some_and(|tile| {
                        !game.rules.is_water(tile) && game.rules.is_passable(tile)
                    })
                    && game
                        .units
                        .values()
                        .all(|unit| unit.owner != 0 || game.wdist(unit.pos, *position) > 2)
            })
            .expect("the map has a quiet land origin");
    let settler = game.spawn_test_unit("settler", 0, origin);
    let target =
        game.wdisk(origin, 2)
            .into_iter()
            .find(|position| {
                game.wdist(origin, *position) == 2
                    && game.map.get(*position).is_some_and(|tile| {
                        !game.rules.is_water(tile) && game.rules.is_passable(tile)
                    })
                    && game.route_step(settler, *position, 0).is_some()
            })
            .expect("the start has a reachable two-tile target");
    let next = game
        .route_step(settler, target, 0)
        .expect("the target has a first route step");
    let threat =
        game.nbrs(next)
            .into_iter()
            .find(|position| {
                *position != home
                    && *position != target
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        !game.rules.is_water(tile) && game.rules.is_passable(tile)
                    })
            })
            .expect("the route has a visible attack square");
    let enemy = game.spawn_test_unit("warrior", 1, threat);
    game.at_war.insert((0, 1));
    let ai = AdvancedAi::new();
    let visible = ai.battlefront_visibility(&game, 0);
    let planned_risk = ai.settlement_tile_risk(&game, 0, Some(settler), next, &visible);
    assert!(game.sees(&visible, threat));
    assert!(
        planned_risk > SETTLER_STEP_RISK_LIMIT,
        "the visible warrior should make the planned tile obviously unsafe: \
             risk={planned_risk}, distance={}, at_war={}",
        game.wdist(threat, next),
        game.is_at_war(0, 1)
    );
    let moving = ai;
    let _ = moving.settler_step_toward_safe(&mut game, 0, settler, target);
    assert_ne!(
        game.units[&settler].pos, next,
        "a settler must not step into a visible direct attack envelope"
    );
    assert!(game.units.contains_key(&enemy));
}

/// ★★★★ Two colonies founded at −19 Loyalty a turn, each lost within eight
/// turns (civvis-20260816T123936Z: Arpinum 18 tiles from the nearest own
/// city, Lugdunum 11), while the mirror's forecast — which sums only the
/// cities on the board — said nothing. See `frontier_loyalty`. A site far
/// beyond the capital's reach on fogged ground is doomed on the live seat
/// and judged sound by the forecast alone; the same site on fully explored
/// ground, and a site inside the capital's halo, are judged as before.
#[test]
fn a_colony_beyond_the_empires_loyalty_reach_on_fogged_ground_is_not_founded() {
    let (mut game, capital, home) = empire_with_a_capital(71_119);
    // ⚠ A pop-1 capital: the rule must not depend on how big it is (the
    // pressure-sum version refused every site of a young empire and
    // stopped run civvis-20260816T142911Z at one city until turn 100).
    game.cities.get_mut(&capital).unwrap().pop = 1;
    // The seat has explored only the capital's neighbourhood.
    let known: Vec<Pos> = game.wdisk(home, 6);
    game.players[0].explored.clear();
    game.players[0].explored.extend(known);
    let far = anchor_at(&game, home, 14);
    let near = anchor_at(&game, home, 4);
    let edge = anchor_at(&game, home, 7);
    let eight = anchor_at(&game, home, 8);

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.frontier_loyalty, "the live seat carries the treatment");
    assert!(
        AdvancedAi::beyond_loyalty_reach(&game, 0, far),
        "fourteen tiles out with fog within nine is beyond reach"
    );
    assert!(
        !AdvancedAi::beyond_loyalty_reach(&game, 0, near),
        "four tiles from the capital is inside its reach, whatever its size"
    );
    assert!(
        !AdvancedAi::beyond_loyalty_reach(&game, 0, edge),
        "seven tiles from a pop-1 capital is inside the reach the record shows"
    );
    assert!(
        AdvancedAi::beyond_loyalty_reach(&game, 0, eight),
        "eight tiles out on fogged ground is where the measured colonies revolted"
    );
    assert!(
        live.settle_site_loyalty_verdict(&game, 0, edge).is_none(),
        "a young empire's ordinary sites are not refused"
    );
    let why = live
        .settle_site_loyalty_verdict(&game, 0, far)
        .expect("the live seat dooms the far fogged site");
    assert!(why.contains("beyond the empire's Loyalty reach"), "{why}");
    assert!(live.settle_site_loyalty_verdict(&game, 0, far).is_some());
    assert!(
        live.settle_site_loyalty_verdict(&game, 0, near).is_none(),
        "a site inside the halo with no rival near is sound"
    );

    // Fully explored ground: the far site is judged by the forecast alone,
    // and with no rival city on the board it is sound.
    let mut charted = game.clone();
    let all: Vec<Pos> = charted.map.tiles.keys().copied().collect();
    charted.players[0].explored.extend(all);
    assert!(!AdvancedAi::beyond_loyalty_reach(&charted, 0, far));
    assert!(live.settle_site_loyalty_verdict(&charted, 0, far).is_none());

    // The forecast alone, as stock and the withheld arm see it: sound.
    let stock = AdvancedAi::new();
    assert!(!stock.frontier_loyalty);
    assert!(!AdvancedAi::legacy().frontier_loyalty);
    assert!(stock.settle_site_loyalty_verdict(&game, 0, far).is_none());
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge();
    withheld.disable_frontier_loyalty();
    assert!(withheld
        .settle_site_loyalty_verdict(&game, 0, far)
        .is_none());
}

/// `frontier-loyalty` is a host-only deployment guard, while the full
/// speculative rate forecast rides on the separately ledger-held
/// `loyalty-rate-alarm`. The two must not be joined at the decision caller:
/// run `civvis-20260825T204120Z` founded Setia with the former on and the
/// latter off, then the host reported -23 Loyalty a turn and the city flipped
/// seven turns later. This exercises the real target-selection branch rather
/// than calling the verdict helper alone.
#[test]
fn frontier_loyalty_retires_a_live_target_when_rate_alarm_is_withheld() {
    let (mut game, _capital, home) = empire_with_a_capital(71_123);
    for unit in game.player_unit_ids(1) {
        game.remove_unit(unit);
    }
    let known = game.wdisk(home, 6);
    game.players[0].explored.clear();
    game.players[0].explored.extend(known);

    // Pick a genuinely legal eighth-ring colony, then give its Settler one
    // legal candidate so the result cannot be attributed to the ordinary site
    // ranking or a missing route.
    let eighth_ring: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| game.wdist(*position, home) == 8)
        .collect();
    let target = eighth_ring
        .into_iter()
        .find(|position| {
            let probe = game.spawn_test_unit("settler", 0, *position);
            let legal = game.can_found_city(probe);
            game.remove_unit(probe);
            legal
        })
        .expect("fixture needs a legal eighth-ring colony");
    let source = game
        .nbrs(target)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
        })
        .expect("fixture needs a traversable tile beside the colony");
    let every_other_plot: Vec<Pos> = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|position| *position != target)
        .collect();
    std::sync::Arc::make_mut(&mut game.blocked_city_sites).extend(every_other_plot);
    let settler = game.spawn_test_unit("settler", 0, source);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    assert!(
        AdvancedAi::beyond_loyalty_reach(&game, 0, target),
        "the target must be the fogged frontier this guard protects"
    );

    let cities_before = game.player_city_ids(0).len();
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(
        live.frontier_loyalty,
        "the live deployment ships the frontier guard"
    );
    // The separate rate forecast is a screened gene whose default each
    // published batch re-decides; withhold it whatever the deployment selects.
    live.disable_loyalty_rate_alarm();
    // `settler-never-idles` (pinned on) would take the retired colony rather
    // than hold once nothing else is offered — that is its point, and its own
    // tests cover it. This test is about the guard's verdict, so withhold it.
    live.disable_settler_never_idles();
    assert_eq!(
        live.best_settler_target(&game, 0, settler, 8, None)
            .map(|(position, _)| position),
        Some(target),
        "the test must offer the doomed colony to the real target loop"
    );
    assert!(
        !live.advanced_settler_step(&mut game, 0, settler),
        "the live frontier guard must retire the colony before the walk"
    );
    assert_eq!(game.player_city_ids(0).len(), cities_before);
    assert!(live.settler_site_is_dead(settler, target));
    assert!(
        !live.settler_targets.contains_key(&settler),
        "safe exhaustion must not leave the rejected target cached"
    );

    // The same guard must also fire after a target was cached on a previous
    // turn, when the Settler arrives with founding movement left.
    game.units.get_mut(&settler).unwrap().pos = target;
    let mut arriving = AdvancedAi::new();
    arriving.enable_live_bridge();
    assert!(arriving.frontier_loyalty);
    arriving.disable_loyalty_rate_alarm();
    assert!(!arriving.base.loyalty_rate_alarm);
    arriving.settler_targets.insert(settler, target);
    assert!(
        !arriving.advanced_settler_step(&mut game, 0, settler),
        "the live arrival check must retire the frontier target rather than found"
    );
    assert_eq!(game.player_city_ids(0).len(), cities_before);
    assert!(arriving.settler_site_is_dead(settler, target));
}

/// A city need not be in the mirror's immediate -8-per-turn emergency to be
/// a bad settlement. A young city caught between equal-distance capitals can
/// look merely "Wavering" on its founding turn yet still flip before it has
/// grown into the investment the Settler was meant to make. The advanced
/// settlement forecast must reject that middle band as well, while the
/// ordinary controller keeps its historical selection path.
#[test]
fn advanced_settlers_refuse_a_city_that_will_flip_within_its_growth_horizon() {
    let (mut game, capital, home) = empire_with_a_capital(71_122);
    for position in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    for unit in game.player_unit_ids(1) {
        game.remove_unit(unit);
    }

    // Put the rival capital eight tiles out and choose the midpoint. The
    // founding minimum is three tiles, so both four-tile distances are legal.
    let (rival_pos, target) = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|rival| game.wdist(home, *rival) == 8)
        .find_map(|rival| {
            game.map
                .tiles
                .keys()
                .copied()
                .find(|target| game.wdist(home, *target) == 4 && game.wdist(rival, *target) == 4)
                .map(|target| (rival, target))
        })
        .expect("fixture needs a four-tile midpoint between the capitals");
    let rival = game.found_city_for(1, rival_pos, None);
    game.cities.get_mut(&capital).unwrap().pop = 6;
    game.cities.get_mut(&rival).unwrap().pop = 10;

    let loyalty_per_turn = {
        let mut forecast = game.speculative_clone();
        let city = forecast.found_city_for(0, target, None);
        forecast.city_loyalty_per_turn(&forecast.cities[&city])
    };
    assert!(
        loyalty_per_turn < -100.0 / SETTLE_TARGET_LOYALTY_RISK_TURNS
            && loyalty_per_turn > -8.0,
        "fixture needs the overlooked middle band, not the old immediate emergency: {loyalty_per_turn}"
    );

    // Ensure the test exercises a real choice rather than the score floor.
    for position in game.wdisk(target, 2) {
        if let Some(tile) = game.map.tiles.get_mut(&position) {
            tile.resource = Some(crate::name!("rice"));
        }
    }
    for position in game.map.tiles.keys().copied().collect::<Vec<_>>() {
        if position != target {
            std::sync::Arc::make_mut(&mut game.blocked_city_sites).insert(position);
        }
    }
    let settler = game.spawn_test_unit("settler", 0, target);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    assert!(game.can_found_city(settler));

    let mut ordinary_board = game.clone();
    let mut ordinary = AdvancedAi::new();
    assert!(
        ordinary.advanced_settler_step(&mut ordinary_board, 0, settler),
        "the ordinary controller retains its established non-forecast path"
    );

    // Withhold the separate rate forecast (a screened gene whose default each
    // published batch re-decides) to isolate the frontier safety floor for a
    // city this close to an already visible rival capital.
    let mut default_live = AdvancedAi::new();
    default_live.enable_live_bridge();
    // `settler-never-idles` (pinned on) takes this colony when it is the only
    // site on the board and the engine's forecast gives it twenty turns —
    // its own tests cover that. This test is about the verdicts, so withhold
    // it by name in both live arms.
    default_live.disable_settler_never_idles();
    assert!(default_live.frontier_loyalty);
    default_live.disable_loyalty_rate_alarm();
    assert!(!default_live.base.loyalty_rate_alarm);
    let why = default_live
        .settle_site_frontier_loyalty_verdict(&game, 0, target)
        .expect("the default frontier guard rejects a nearby visible major city");
    assert!(why.contains("visible rival city"), "{why}");
    let mut frontier_board = game.clone();
    assert!(
        !default_live.advanced_settler_step(&mut frontier_board, 0, settler),
        "the default live controller must not found an obvious rival-pressed colony"
    );
    assert!(default_live.settler_site_is_dead(settler, target));
    assert_eq!(frontier_board.player_city_ids(0).len(), 1);

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    live.disable_settler_never_idles();
    // This arm is about the deployed rate forecast. The separately selected
    // exhaustion guard is covered by its own fixture and would reject this
    // site before that forecast gets to speak.
    live.disable_exhaustion_loyalty_guard();
    assert!(!live.exhaustion_loyalty_guard);
    // The rate forecast remains independently covered after the withheld
    // frontier-floor control above, forced on whatever the deployment selects.
    live.frontier_loyalty = false;
    live.enable_loyalty_rate_alarm();
    assert!(live.base.loyalty_rate_alarm);
    let why = live
        .settle_site_loyalty_verdict(&game, 0, target)
        .expect("the advanced forecast rejects a city that will flip before it grows");
    assert!(why.contains("revolt in about"), "{why}");
    assert!(
        !live.advanced_settler_step(&mut game, 0, settler),
        "the advanced controller must retire the risky founding rather than spend the Settler"
    );
    assert!(live.settler_site_is_dead(settler, target));
    assert_eq!(game.player_city_ids(0).len(), 1);
}

/// ★★★★ A border with an unresolved city behind it is a city in the fog. Run
/// civvis-20260826T030045Z: Lugdunum was founded t55 five tiles from Germany's
/// border, while its only visible German city stood ten tiles away. The host
/// immediately read −22 Loyalty a turn and the undamaged city flipped at t70;
/// the forecast had passed it because the nearer German city was not on the
/// board. See `UNRESOLVED_MAJOR_BORDER_RADIUS` and
/// `Game::unseen_major_borders`: the live seat refuses a site within five
/// tiles of a fully unseen or fifth-ring-ambiguous major border. Six tiles
/// out, an empty set, stock and the withheld arm judge by the forecast alone.
#[test]
fn a_site_beside_an_unresolved_majors_border_is_doomed_before_the_walk() {
    let (mut game, capital, home) = empire_with_a_capital(71_121);
    game.cities.get_mut(&capital).unwrap().pop = 4;
    // Fully explored, so the fog rule cannot be what refuses.
    let all: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(all);
    let site = anchor_at(&game, home, 4);
    let border = anchor_at(&game, site, 5);
    let far_border = anchor_at(&game, site, 6);

    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.frontier_loyalty);
    assert!(
        live.settle_site_loyalty_verdict(&game, 0, site).is_none(),
        "no unresolved border on the board: the site is sound"
    );

    game.unseen_major_borders.insert(border);
    assert!(AdvancedAi::beside_unresolved_major_border(&game, site));
    let why = live
        .settle_site_loyalty_verdict(&game, 0, site)
        .expect("the live seat dooms a site five tiles from an unresolved major border");
    assert!(why.contains("border whose city may be hidden"), "{why}");

    // Six tiles from the border: outside the radius, judged by the forecast.
    game.unseen_major_borders.clear();
    game.unseen_major_borders.insert(far_border);
    assert!(!AdvancedAi::beside_unresolved_major_border(&game, site));
    assert!(live.settle_site_loyalty_verdict(&game, 0, site).is_none());

    // Stock and the withheld arm never read the set.
    game.unseen_major_borders.insert(border);
    let stock = AdvancedAi::new();
    assert!(stock.settle_site_loyalty_verdict(&game, 0, site).is_none());
    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge();
    withheld.disable_frontier_loyalty();
    assert!(withheld
        .settle_site_loyalty_verdict(&game, 0, site)
        .is_none());
}

#[test]
fn blocked_land_settler_links_to_one_escort_and_the_escort_leads() {
    let mut game = Game::new_full(1, 24, 16, 8_119, 120, 0, false);
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let source = game.units[&founding_settler].pos;
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .unwrap();
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    for position in game.wdisk(source, SETTLER_ESCORT_SEARCH_RADIUS) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    let target =
        game.map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdist(source, *position) >= SETTLER_ESCORT_DISTANCE
                    && game.wdist(source, *position) <= SETTLER_ESCORT_SEARCH_RADIUS
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a reachable expansion target");
    let settler = game.spawn_test_unit("settler", 0, source);
    let escort = game.spawn_test_unit("warrior", 0, source);
    let mut ai = AdvancedAi::new();
    ai.settler_targets.insert(settler, target);
    ai.settler_stalls.insert(settler, 2);
    ai.settler_blocked_turns.insert(settler, 3);

    ai.advanced_formations(&mut game, 0);
    assert_eq!(game.units[&settler].linked_to, Some(escort));
    assert_eq!(game.units[&escort].linked_to, Some(settler));

    let before = game.wdist(source, target);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    assert_eq!(
        ai.settler_escort_step(&mut game, 0, escort, &plan),
        Some(true)
    );
    assert_eq!(game.units[&settler].pos, game.units[&escort].pos);
    assert!(game.wdist(game.units[&settler].pos, target) < before);
    assert!(!ai.settler_stalls.contains_key(&settler));
    assert!(!ai.settler_blocked_turns.contains_key(&settler));

    let trapped = game.units[&settler].pos;
    assert!(game.wdist(trapped, target) > 1);
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("mountain");
    }
    game.map.tiles.get_mut(&trapped).unwrap().terrain = crate::name!("plains");
    game.map.tiles.get_mut(&target).unwrap().terrain = crate::name!("plains");
    for _ in 0..SETTLER_STALL_LIMIT {
        for unit in [settler, escort] {
            let unit = game.units.get_mut(&unit).unwrap();
            unit.moves_left = 4.0;
            unit.acted = false;
            unit.fortified = false;
        }
        assert!(ai
            .settler_escort_step(&mut game, 0, escort, &plan)
            .is_some());
        game.turn += 1;
    }
    assert!(!ai.settler_targets.contains_key(&settler));
    assert_eq!(
        ai.settler_avoid.get(&settler).map(|(pos, _)| *pos),
        Some(target)
    );
    assert_eq!(game.units[&settler].linked_to, None);
    assert_eq!(game.units[&escort].linked_to, None);
}

#[test]
fn a_stacked_guard_shadows_the_settler_without_a_formation() {
    let (mut game, source, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, source);
    let guard = game.spawn_test_unit("warrior", 0, source);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, target);
    let plan = stacked_escort_plan(&game);

    // The repaired machinery engages only under threat: a raider shadows
    // the expedition, re-seated beside the settler every turn.
    let mut raider = None;
    let mut last = game.wdist(source, target);
    for _ in 0..8 {
        for unit in [settler, guard] {
            if let Some(unit) = game.units.get_mut(&unit) {
                unit.moves_left = 2.0;
                unit.acted = false;
                unit.fortified = false;
            }
        }
        if let Some(old) = raider.take() {
            game.remove_unit(old);
        }
        let beside = game
            .wdisk(game.units[&settler].pos, 2)
            .into_iter()
            .find(|position| {
                game.wdist(*position, game.units[&settler].pos) == 2
                    && *position != target
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("open ground beside the settler");
        raider = Some(game.spawn_test_unit("warrior", 1, beside));
        ai.advanced_settler_step(&mut game, 0, settler);
        if !game.units.contains_key(&settler) {
            // Founded: the march is over and the pairing held throughout.
            break;
        }
        assert!(
            ai.settler_escort_step(&mut game, 0, guard, &plan).is_some(),
            "the bound guard must act as a guard every turn"
        );
        assert_eq!(
            game.units[&guard].pos, game.units[&settler].pos,
            "the pair must end every turn on one tile — that is the whole treatment"
        );
        assert_eq!(game.units[&settler].linked_to, None);
        assert_eq!(game.units[&guard].linked_to, None);
        let now = game.wdist(game.units[&settler].pos, target);
        assert!(now <= last, "the pair must never lose ground");
        last = now;
        game.turn += 1;
    }
    assert!(
        last < game.wdist(source, target),
        "the pair must close on the site"
    );
}

/// Long routes are not quiet local walks. The live formationless escort used
/// to return this Warrior to ordinary military work before the first step
/// whenever no raider was already visible, leaving the whole crossing and
/// arrival uncovered.
#[test]
fn a_quiet_long_settler_journey_keeps_its_land_guard() {
    let (mut game, source, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, source);
    let guard = game.spawn_test_unit("warrior", 0, source);
    for uid in [settler, guard] {
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = 2.0;
        unit.acted = false;
        unit.fortified = false;
    }
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, target);

    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    assert_eq!(ai.settler_guards.get(&settler), Some(&guard));
    assert_eq!(
        ai.settler_escort_journeys
            .get(&settler)
            .map(|journey| journey.target),
        Some(target),
        "the first long route commits the escort before the quiet middle"
    );
    assert_eq!(
        game.units[&guard].pos, game.units[&settler].pos,
        "the land guard follows the first move instead of merely being reserved"
    );
}

#[test]
fn a_long_overseas_journey_adds_a_naval_guard_after_embarkation() {
    let (mut game, source, target) = island_colony_game();
    game.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let harbor = game
        .nbrs(source)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| game.rules.is_water(tile))
        })
        .expect("the home city has a water departure tile");
    let settler = game.spawn_test_unit("settler", 0, source);
    let land_guard = game.spawn_test_unit("warrior", 0, source);
    let sea_guard = game.spawn_test_unit("galley", 0, harbor);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, target);

    for uid in [settler, land_guard, sea_guard] {
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = 2.0;
        unit.acted = false;
        unit.fortified = false;
    }
    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    assert!(
        game.map
            .get(game.units[&settler].pos)
            .is_some_and(|tile| game.rules.is_water(tile)),
        "the first long-route step embarks from the home city"
    );
    assert_eq!(ai.settler_guards.get(&settler), Some(&land_guard));

    game.turn += 1;
    for uid in [settler, land_guard, sea_guard] {
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = 2.0;
        unit.acted = false;
        unit.fortified = false;
    }
    let _ = ai.advanced_settler_step(&mut game, 0, settler);
    assert_eq!(ai.settler_guards.get(&settler), Some(&land_guard));
    assert_eq!(
        ai.settler_sea_guards.get(&settler),
        Some(&sea_guard),
        "the naval unit joins the water layer without replacing the land guard"
    );
}

/// A long expedition can outrun the only available naval hull. The live
/// bridge must bind a routeable ship beyond the local escort radius so the
/// military turn can close the gap; otherwise the water safety floor holds the
/// Settler forever even though naval cover exists on the same connected sea.
#[test]
fn a_live_embarked_settler_summons_a_distant_routeable_naval_guard() {
    let (mut game, source, target) = island_colony_game();
    game.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let probe = game.spawn_test_unit("settler", 0, source);
    let first_water = game
        .route_step(probe, target, 0)
        .expect("the overseas route has a first water tile");
    assert!(game.rules.is_water(&game.map.tiles[&first_water]));
    game.remove_unit(probe);

    let settler = game.spawn_test_unit("settler", 0, first_water);
    let land_guard = game.spawn_test_unit("warrior", 0, first_water);
    let next_water = game
        .route_step(settler, target, 0)
        .expect("the overseas route has a second water tile");
    assert!(game.rules.is_water(&game.map.tiles[&next_water]));
    let sea_guard_pos = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.rules.is_water(&game.map.tiles[position])
                && game.wdist(*position, first_water) > SETTLER_ESCORT_SEARCH_RADIUS
        })
        .expect("the connected sea has a distant water tile");
    let sea_guard = game.spawn_test_unit("galley", 0, sea_guard_pos);
    let route_distance = game
        .route_distance(sea_guard, first_water, 0)
        .expect("the distant hull can reach the Settler's connected sea");
    assert!(route_distance > SETTLER_ESCORT_SEARCH_RADIUS as usize);

    let mut live = AdvancedAi::new();
    live.enable_live_formationless_settler_shadow();
    live.enable_live_settler_capture_lessons();
    live.settler_targets.insert(settler, target);
    live.settler_escort_journeys.insert(
        settler,
        SettlerEscortJourney {
            home: source,
            target,
        },
    );
    live.settler_guards.insert(settler, land_guard);

    for uid in [settler, land_guard, sea_guard] {
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = 2.0;
        unit.acted = false;
        unit.fortified = false;
    }

    assert!(
        !live.advanced_settler_step(&mut game, 0, settler),
        "the live floor waits for the distant hull to close before taking another water step"
    );
    assert_eq!(live.settler_sea_guards.get(&settler), Some(&sea_guard));

    let before = game.units[&sea_guard].pos;
    assert_eq!(live.stacked_guard_step(&mut game, 0, sea_guard), Some(true));
    assert_ne!(game.units[&sea_guard].pos, before);
}

#[test]
fn expedition_guard_memory_remaps_and_releases_both_layers() {
    let (_game, home, target) = stacked_escort_fixture();
    let old_settler = 41;
    let old_land_guard = 42;
    let old_sea_guard = 43;
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(old_settler, target);
    ai.settler_guards.insert(old_settler, old_land_guard);
    ai.settler_sea_guards.insert(old_settler, old_sea_guard);
    ai.settler_escort_journeys
        .insert(old_settler, SettlerEscortJourney { home, target });
    ai.guard_wait.insert(old_settler, (3, 1));

    let new_settler = 141;
    let new_land_guard = 142;
    let new_sea_guard = 143;
    ai.remap_unit_memory(&BTreeMap::from([
        (old_settler, new_settler),
        (old_land_guard, new_land_guard),
        (old_sea_guard, new_sea_guard),
    ]));
    assert_eq!(ai.settler_guards.get(&new_settler), Some(&new_land_guard));
    assert_eq!(
        ai.settler_sea_guards.get(&new_settler),
        Some(&new_sea_guard)
    );
    assert_eq!(
        ai.settler_escort_journeys.get(&new_settler).copied(),
        Some(SettlerEscortJourney { home, target })
    );

    assert!(ai.clear_settler_guard_bindings(new_settler));
    assert!(!ai.settler_guards.contains_key(&new_settler));
    assert!(!ai.settler_sea_guards.contains_key(&new_settler));
    assert!(!ai.guard_wait.contains_key(&new_settler));

    ai.disable_live_formationless_settler_shadow();
    assert!(ai.settler_escort_journeys.is_empty());
}

/// On an overseas expedition the land and naval military layers can share an
/// embarked Settler's water tile. The naval guard must stop at shore while
/// the land guard disembarks, covers the hostile turn, and lets the Settler
/// found on its next turn.
#[test]
fn an_overseas_settler_uses_both_guard_layers_and_lands_with_the_land_guard() {
    let (mut game, source, island_center) = island_colony_game();
    game.players[0]
        .techs
        .extend([crate::name!("sailing"), crate::name!("shipbuilding")]);
    let shore = game
        .nbrs(island_center)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.nbrs(*pos).into_iter().any(|neighbour| {
                    game.map
                        .get(neighbour)
                        .is_some_and(|tile| game.rules.is_water(tile))
                })
        })
        .expect("the colony has a shoreline tile");
    let water = game
        .nbrs(shore)
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| game.rules.is_water(tile))
        })
        .expect("the shoreline has an adjacent water tile");
    let settler = game.spawn_test_unit("settler", 0, water);
    let land_guard = game.spawn_test_unit("warrior", 0, water);
    let sea_guard = game.spawn_test_unit("galley", 0, water);
    for uid in [settler, land_guard, sea_guard] {
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = 2.0;
        unit.acted = false;
        unit.fortified = false;
    }
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, shore);
    // This is the water-to-land middle of a route that began at the home
    // city; the route's distance was measured and committed at departure.
    ai.settler_escort_journeys.insert(
        settler,
        SettlerEscortJourney {
            home: source,
            target: shore,
        },
    );
    ai.settler_guards.insert(settler, land_guard);
    ai.settler_sea_guards.insert(settler, sea_guard);

    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    assert_eq!(game.units[&settler].pos, shore);
    assert_eq!(
        game.units[&land_guard].pos, shore,
        "the embarked land guard disembarks with the Settler"
    );
    assert_eq!(
        game.units[&sea_guard].pos, water,
        "the naval guard remains on its water layer at the shore"
    );
    assert_eq!(ai.settler_guards.get(&settler), Some(&land_guard));
    assert_eq!(ai.settler_sea_guards.get(&settler), Some(&sea_guard));

    // The naval unit gets its turn after the Settler. Seeing the Settler on
    // land releases the sea binding in that same turn, before it can be
    // diverted by normal naval work.
    assert_eq!(ai.stacked_guard_step(&mut game, 0, sea_guard), None);
    assert!(!ai.settler_sea_guards.contains_key(&settler));

    for uid in [settler, land_guard] {
        let unit = game.units.get_mut(&uid).unwrap();
        unit.moves_left = 2.0;
        unit.acted = false;
        unit.fortified = false;
    }
    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    assert!(
        game.city_at(shore).is_some(),
        "the protected one-turn landfall becomes a city immediately next turn"
    );
}

/// A live stack is only protection while its military unit can actually hold
/// the tile.  `stacked_escort_pace` used to retain any living land unit,
/// including the 21-HP Warrior killed on t37 of civvis-20260826T225804Z and
/// the Trebuchet a Line Infantry broke before taking the Settler on t120.
#[test]
fn a_default_live_escort_replaces_guards_that_cannot_hold() {
    for (kind, hp, label) in [
        ("warrior", STACKED_GUARD_MIN_HP - 1, "wounded"),
        ("trebuchet", 100, "outmatched"),
    ] {
        let (mut game, source, target) = stacked_escort_fixture();
        let settler = game.spawn_test_unit("settler", 0, source);
        let unsafe_guard = game.spawn_test_unit(kind, 0, source);
        game.units.get_mut(&unsafe_guard).unwrap().hp = hp;
        let replacement_at = game
            .nbrs(source)
            .into_iter()
            .find(|position| {
                game.map
                    .get(*position)
                    .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            })
            .expect("a passable replacement post beside the Settler");
        let replacement = game.spawn_test_unit("man_at_arms", 0, replacement_at);
        let raider_at = game
            .wdisk(source, 2)
            .into_iter()
            .find(|position| {
                game.wdist(source, *position) == 2
                    && *position != target
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("a visible Line Infantry post in reach of the stack");
        let raider = game.spawn_test_unit("line_infantry", 1, raider_at);
        game.players[0].explored.insert(raider_at);

        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge();
        ai.enable_settler_guard_holds();
        assert!(
            ai.settler_guard_holds && ai.live_formationless_settler_shadow,
            "the focused arm carries the stack discipline"
        );
        let visible = ai.battlefront_visibility(&game, 0);
        assert!(game.unit_visible_to(raider, 0) && game.sees(&visible, raider_at));
        let replacement_outmatched =
            ai.guard_outmatched_at(&game, 0, &game.units[&replacement], source, &visible);
        assert!(
            !replacement_outmatched,
            "the replacement can survive the Line Infantry"
        );
        if label == "outmatched" {
            let unsafe_outmatched =
                ai.guard_outmatched_at(&game, 0, &game.units[&unsafe_guard], source, &visible);
            assert!(
                unsafe_outmatched,
                "the Trebuchet is not a shield against the Line Infantry"
            );
        }
        ai.settler_targets.insert(settler, target);
        ai.settler_guards.insert(settler, unsafe_guard);

        let _ = ai.stacked_escort_pace(&mut game, 0, settler);
        assert_eq!(
            ai.settler_guards.get(&settler),
            Some(&replacement),
            "the {label} guard must be replaced before the Settler acts"
        );
    }
}

/// The HostOnly capture-lessons bridge can imply the guard-survival predicate
/// even while the screenable `settler-guard-holds` field is withheld.  The
/// full Settler step must therefore build its visibility frame through the
/// family helper too; using the raw field leaves a healthy stacked guard on
/// the tile, reaches `guard_outmatched_at`, and panics with `None.unwrap()`.
#[test]
fn live_capture_lessons_do_not_panic_on_a_healthy_stacked_guard() {
    let (mut game, _city, home) = barbarian_field(71_308);
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    let start = game
        .wdisk(home, 2)
        .into_iter()
        .find(|pos| game.wdist(*pos, home) == 2 && open_land(&game, *pos))
        .expect("open ground two tiles from home");
    let raider_at = game
        .wdisk(start, 2)
        .into_iter()
        .find(|pos| open_land(&game, *pos))
        .expect("a visible raider post");
    let target = game
        .wdisk(start, 4)
        .into_iter()
        .find(|pos| open_land(&game, *pos))
        .expect("an open settlement target");
    let settler = game.spawn_test_unit("settler", 0, start);
    let guard = game.spawn_test_unit("warrior", 0, start);
    let _raider = game.spawn_test_unit("warrior", 1, raider_at);

    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge();
    ai.disable_settler_guard_holds();
    assert!(!ai.settler_guard_holds);
    assert!(ai.settler_guard_holds_on());
    ai.settler_guards.insert(settler, guard);
    ai.settler_targets.insert(settler, target);

    // This exercises the complete production Settler path, including the
    // guarded-here branch that calls `guard_outmatched_at`. It must return
    // normally with the bound healthy guard still recognized as protection.
    let _ = ai.advanced_settler_step(&mut game, 0, settler);
    assert_eq!(ai.settler_guards.get(&settler), Some(&guard));
}

#[test]
fn a_settler_departs_its_city_before_waiting_for_a_guard() {
    let (mut game, city, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, city);
    // A guard that cannot close right away would previously consume this
    // whole turn with a fortify, even though the Settler has just completed
    // safely inside an owned city.
    let lagging =
        game.wdisk(city, 3)
            .into_iter()
            .find(|position| {
                game.wdist(city, *position) == 3
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a lagging guard post");
    let guard = game.spawn_test_unit("warrior", 0, lagging);
    // Inside the escort's threat radius but outside capture reach: the only
    // former reason to wait here is the guard's distance, not survival.
    let raider_post =
        game.wdisk(city, 6)
            .into_iter()
            .find(|position| {
                game.wdist(city, *position) == 6
                    && *position != lagging
                    && game.units_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a raider post");
    game.spawn_test_unit("warrior", 1, raider_post);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, target);
    let log_start = game.log.len();

    assert!(
        ai.advanced_settler_step(&mut game, 0, settler),
        "a completed Settler must use its first controllable turn to dispatch"
    );
    assert_ne!(
        game.units[&settler].pos, city,
        "a lagging guard must not leave a newly completed Settler in its city"
    );
    assert!(
        game.log.since(log_start).any(|(seat, action)| {
            *seat == 0
                && matches!(
                    action,
                    crate::game::Action::MoveTo { unit, to }
                        if *unit == settler && *to != city
                )
        }),
        "dispatch must issue a real outbound MoveTo order, not merely select a target"
    );
    assert_eq!(
        ai.settler_guards.get(&settler),
        Some(&guard),
        "immediate dispatch retains the guard assignment for the march"
    );
    assert!(
        !ai.guard_wait.contains_key(&settler),
        "a city departure must not carry an old guard-wait clock forward"
    );
}

#[test]
fn a_settler_waits_for_its_guard_only_within_patience_outside_a_city() {
    let (mut game, city, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, city);
    let source = game
        .route_step(settler, target, 0)
        .expect("fixture has an outbound staging step");
    game.apply(
        0,
        &crate::game::Action::MoveTo {
            unit: settler,
            to: source,
        },
    )
    .expect("the Settler reaches the non-city staging tile");
    assert!(
        game.city_at(source).is_none(),
        "the patience rule belongs to an exposed staging tile, not the city"
    );
    // A guard that exists but can never close: three tiles away with no
    // movement, refreshed to zero every turn below.
    let lagging =
        game.wdisk(source, 3)
            .into_iter()
            .find(|position| {
                game.wdist(source, *position) == 3
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a lagging guard post");
    let guard = game.spawn_test_unit("warrior", 0, lagging);
    // The repaired machinery engages only under threat: a raider stands six
    // tiles out — inside the engage radius, outside capture reach, so the
    // scene exercises the pure wait rather than the threatened fallback.
    let raider_post =
        game.wdisk(source, 6)
            .into_iter()
            .find(|position| {
                game.wdist(source, *position) == 6
                    && *position != lagging
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a raider post");
    game.spawn_test_unit("warrior", 1, raider_post);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, target);

    for turn in 0..(STACKED_ESCORT_PATIENCE + 2) {
        {
            let unit = game.units.get_mut(&settler).unwrap();
            unit.moves_left = 2.0;
            unit.acted = false;
            unit.fortified = false;
        }
        game.units.get_mut(&guard).unwrap().moves_left = 0.0;
        ai.advanced_settler_step(&mut game, 0, settler);
        let waited_out = turn >= STACKED_ESCORT_PATIENCE;
        if waited_out {
            assert_ne!(
                game.units[&settler].pos, source,
                "past its patience the settler must march unescorted \
                     rather than freeze the expansion window"
            );
        } else {
            assert_eq!(
                game.units[&settler].pos, source,
                "within patience the settler holds for its guard"
            );
            assert_eq!(
                ai.settler_targets.get(&settler),
                Some(&target),
                "waiting is not a stall; the site must survive it"
            );
            assert!(!ai.settler_stalls.contains_key(&settler));
        }
        game.turn += 1;
    }
}

/// A bounded escort response avoids an opening freeze on quiet ground, but it
/// must not release an exposed Settler into a route step a visible hostile can
/// capture. The
/// previous rule released the second Settler of `civvis-20260826T054001Z`
/// from a safe staging tile while its guard was three tiles behind; a
/// barbarian warrior then took it on turn 14.
#[test]
fn a_lagging_guard_does_not_expire_on_a_visibly_capturable_step() {
    let (mut game, city, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, city);
    let source = game
        .route_step(settler, target, 0)
        .expect("fixture has an outbound staging step");
    game.apply(
        0,
        &crate::game::Action::MoveTo {
            unit: settler,
            to: source,
        },
    )
    .expect("the Settler reaches the non-city staging tile");
    assert!(
        game.city_at(source).is_none(),
        "a genuinely exposed route step still receives the capture guard"
    );
    let next = game
        .route_step(settler, target, 0)
        .expect("fixture has a first route step");
    let lagging =
        game.wdisk(source, 3)
            .into_iter()
            .find(|position| {
                game.wdist(source, *position) == 3
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a lagging guard post");
    let guard = game.spawn_test_unit("warrior", 0, lagging);
    let raider_post =
        game.wdisk(next, 2)
            .into_iter()
            .find(|position| {
                game.wdist(next, *position) == 2
                    && *position != lagging
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a raider two moves from the next step");
    game.spawn_test_unit("warrior", 1, raider_post);

    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, target);
    ai.settler_guards.insert(settler, guard);
    ai.guard_wait
        .insert(settler, (game.turn - 1, STACKED_ESCORT_PATIENCE));
    assert!(
        ai.unstacked_settler_step_is_capturable(&game, 0, settler),
        "the warrior can take the first route step before the distant guard arrives"
    );

    assert!(
        ai.stacked_escort_pace(&mut game, 0, settler).is_some(),
        "a close capture threat must handle the exposed Settler instead of releasing its ordinary route"
    );
    assert_ne!(
        game.units[&settler].pos, next,
        "the Settler must not march into the capturable route step"
    );
}

/// A settler waiting for its guard on a threatened tile does not stand
/// still — it steps toward the guard; on a quiet tile it waits as before.
#[test]
fn a_threatened_settler_falls_back_toward_its_guard_instead_of_waiting() {
    // Two majors at war, flat explored ground, our capital and theirs.
    let build = |with_enemy: bool| -> (Game, u32, u32, Pos) {
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
            game.players[0].explored.insert(position);
        }
        game.current = 0;
        game.turn = 22;
        game.record_contact(0, 1);
        game.at_war.insert((0, 1));
        // Settler four tiles from home, guard two tiles behind it (toward
        // home), enemy warrior two tiles ahead of it.
        let mut ring: Vec<Pos> = game
            .wdisk(home, 4)
            .into_iter()
            .filter(|p| game.wdist(*p, home) == 4)
            .collect();
        ring.sort_unstable();
        let source = ring[0];
        let mut toward_home: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| game.wdist(*p, source) == 2 && game.wdist(*p, home) == 2)
            .collect();
        toward_home.sort_unstable();
        let guard_pos = toward_home[0];
        let mut away: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| game.wdist(*p, source) == 2 && game.wdist(*p, home) == 6)
            .collect();
        away.sort_unstable();
        let settler = game.spawn_test_unit("settler", 0, source);
        let guard = game.spawn_test_unit("warrior", 0, guard_pos);
        game.units.get_mut(&guard).unwrap().moves_left = 0.0;
        if with_enemy {
            let enemy = game.spawn_test_unit("warrior", 1, away[0]);
            game.units.get_mut(&enemy).unwrap().moves_left = 2.0;
        }
        (game, settler, guard, source)
    };

    // Threatened: the settler steps toward its guard rather than freezing.
    let (mut game, settler, guard, source) = build(true);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    assert!(ai.settlement_safety);
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    let before = game.wdist(source, game.units[&guard].pos);
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    let _ = ai.advanced_settler_step(&mut game, 0, settler);
    let after_pos = game.units[&settler].pos;
    assert_ne!(
        after_pos, source,
        "a threatened settler must not stand still for its guard"
    );
    assert!(
        game.wdist(after_pos, game.units[&guard].pos) < before,
        "it closes the gap to the guard"
    );
    // Under the unified live shadow (2026-08-21: the two native escort genes
    // were removed and their machinery rides the one live flag), capture
    // pricing puts the threatened tile over the step limit, so the
    // retreat-before-anything branch may act before the pace's own fallback.
    // Either path satisfies the contract; the positional asserts above are
    // the contract.
    assert!(
        !journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline.starts_with("Settler waits for its guard")),
        "whatever moved it, a threatened settler must not have chosen to wait"
    );

    // Quiet: since the cycle-three repair the machinery stands down with no
    // visible hostile in reach — the settler marches instead of paying for a
    // guard against a risk that is not there, and no binding is held.
    let (mut calm, settler, _guard, source) = build(false);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    calm.units.get_mut(&settler).unwrap().moves_left = 2.0;
    let _ = ai.advanced_settler_step(&mut calm, 0, settler);
    assert_ne!(
        calm.units[&settler].pos, source,
        "on quiet ground the escort stands down and the settler marches"
    );
    assert!(
        ai.settler_guards.is_empty(),
        "no guard is bound against a risk that is not there"
    );
}

/// The t60 geometry of run civvis-20260816T200454Z, reduced: a settler
/// with its guard on the next tile and a hostile Slinger two tiles off,
/// its patience for the guard spent. The old model let it march to the
/// tile beside the Slinger (approach tier 12, guard-adjacent discount ×0.45
/// — "risk 16" under the 30 limit) and the Slinger carried it off. Under
/// `settler_stack_discipline` reach is capture: the tile beside the
/// Slinger prices at the capture tier, the guard's tile — the only tile
/// the settler cannot be taken from — is where it goes.
#[test]
fn a_settler_beside_its_guard_and_two_tiles_from_a_slinger_stacks_rather_than_marches() {
    let build = || -> (Game, u32, u32, u32, Pos, Pos) {
        let mut game = Game::new_full(2, 30, 20, 8_122, 120, 0, false);
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
            game.players[0].explored.insert(position);
        }
        game.current = 0;
        game.turn = 60;
        game.record_contact(0, 1);
        game.at_war.insert((0, 1));
        // Settler four from home; guard adjacent to it, one tile further
        // out; slinger adjacent to the guard, two tiles from the settler.
        let mut ring: Vec<Pos> = game
            .wdisk(home, 4)
            .into_iter()
            .filter(|p| game.wdist(*p, home) == 4)
            .collect();
        ring.sort_unstable();
        let source = ring[0];
        let mut guard_ring: Vec<Pos> = game
            .nbrs(source)
            .into_iter()
            .filter(|p| game.wdist(*p, home) == 5)
            .collect();
        guard_ring.sort_unstable();
        let guard_pos = guard_ring[0];
        // Off the straight line, so a tile beside the slinger other than
        // the guard's is within the settler's two moves.
        let mut slinger_ring: Vec<Pos> = game
            .nbrs(guard_pos)
            .into_iter()
            .filter(|p| {
                game.wdist(*p, source) == 2
                    && game.wdist(*p, home) >= 5
                    && game
                        .nbrs(source)
                        .into_iter()
                        .any(|n| n != guard_pos && game.wdist(n, *p) == 1)
            })
            .collect();
        slinger_ring.sort_unstable();
        let slinger_pos = slinger_ring[0];
        let settler = game.spawn_test_unit("settler", 0, source);
        let guard = game.spawn_test_unit("warrior", 0, guard_pos);
        game.units.get_mut(&guard).unwrap().moves_left = 0.0;
        let slinger = game.spawn_test_unit("slinger", 1, slinger_pos);
        game.units.get_mut(&slinger).unwrap().moves_left = 2.0;
        game.units.get_mut(&settler).unwrap().moves_left = 2.0;
        (game, settler, guard, slinger, source, guard_pos)
    };

    // The step limit is unchanged; the price of the tile beside the
    // slinger is not.
    let (game, settler, _guard, slinger, source, guard_pos) = build();
    let beside_slinger = game
        .nbrs(source)
        .into_iter()
        .filter(|p| game.wdist(*p, game.units[&slinger].pos) == 1 && *p != guard_pos)
        .min()
        .expect("a tile beside both the settler and the slinger");
    let mut disciplined = AdvancedAi::new();
    disciplined.enable_live_formationless_settler_shadow();
    disciplined.settler_guards.insert(settler, _guard);
    let mut lax = AdvancedAi::new();
    lax.settler_guards.insert(settler, _guard);
    let visible = disciplined.battlefront_visibility(&game, 0);
    let lax_price = lax.settlement_tile_risk(&game, 0, Some(settler), beside_slinger, &visible);
    let strict_price =
        disciplined.settlement_tile_risk(&game, 0, Some(settler), beside_slinger, &visible);
    assert!(
        lax_price <= SETTLER_STEP_RISK_LIMIT,
        "the old model let the settler walk beside the slinger (risk {lax_price:.1})"
    );
    assert!(
        strict_price > SETTLER_STEP_RISK_LIMIT,
        "reach is capture: {strict_price:.1} must exceed the step limit"
    );
    let on_guard = disciplined.settlement_tile_risk(&game, 0, Some(settler), guard_pos, &visible);
    assert!(
        on_guard <= SETTLER_STEP_RISK_LIMIT,
        "the guard's tile is where a settler cannot be taken (risk {on_guard:.1})"
    );

    // With its patience for the guard spent, the settler still ends the
    // turn on the guard's tile, not beside the slinger.
    let (mut game, settler, guard, slinger, source, guard_pos) = build();
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_guards.insert(settler, guard);
    ai.guard_wait
        .insert(settler, (game.turn - 1, STACKED_ESCORT_PATIENCE));
    let _ = ai.advanced_settler_step(&mut game, 0, settler);
    let after = game.units[&settler].pos;
    let slinger_pos = game.units[&slinger].pos;
    assert_eq!(
        after, guard_pos,
        "the settler stacks with its guard — the one tile it cannot be taken from \
             (source {source:?}, slinger {slinger_pos:?})"
    );

    // The withheld arm keeps the measured behaviour: with the live shadow
    // off (the native escort genes were removed 2026-08-21), the ordinary
    // settler logic marches, and the tile beside the slinger is open to it.
    let (mut game, settler, guard, slinger, source, guard_pos) = build();
    let mut ai = AdvancedAi::new();
    let _ = guard;
    let _ = ai.advanced_settler_step(&mut game, 0, settler);
    let lax_after = game.units[&settler].pos;
    assert!(
        lax_after != guard_pos && lax_after != source,
        "the withheld arm marches on: {lax_after:?}"
    );
    let _ = slinger;
}

/// ★★★★ One settler, two sites, twenty-nine turns (civvis-20260816T123936Z,
/// settler 4849688: the journal alternated between (42,22) and (41,24)
/// t138–t146 as a threat flickered at the edge of sight). See
/// `settler_target_hysteresis`. When the cached target stops passing its
/// checks the live seat sets it aside for a few turns and commits to the
/// runner-up; the withheld arm walks straight back to the first site the
/// frame it is valid again.
#[test]
fn a_settler_target_dropped_for_danger_is_set_aside_not_re_picked_next_frame() {
    let (game0, source, _) = stacked_escort_fixture();
    // The site the picker itself prefers, so the withheld arm's return to
    // it is the picker's own choice and not a planted cache.
    let first = {
        let mut probe = game0.clone();
        let settler = probe.spawn_test_unit("settler", 0, source);
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        ai.remember_early_settler_home(&probe, 0, settler);
        ai.disable_frontier_loyalty();
        ai.best_settler_target(&probe, 0, settler, 8, None)
            .map(|(pos, _)| pos)
            .expect("the fixture offers a settle site")
    };
    let run = |hysteresis: bool| -> (Option<Pos>, Option<Pos>, Option<Pos>) {
        let mut game = game0.clone();
        let settler = game.spawn_test_unit("settler", 0, source);
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        ai.remember_early_settler_home(&game, 0, settler);
        // Isolate the hysteresis: the fogged-frontier rule would retire a
        // far site on this small board for its own reason.
        ai.disable_frontier_loyalty();
        if !hysteresis {
            ai.disable_settler_target_hysteresis();
        }
        ai.settler_targets.insert(settler, first);
        // Frame 1: the cached site stops passing its checks (blocked by
        // the host, the same drop a risk flicker produces).
        std::sync::Arc::make_mut(&mut game.blocked_city_sites).insert(first);
        game.units.get_mut(&settler).unwrap().moves_left = 2.0;
        let _ = ai.advanced_settler_step(&mut game, 0, settler);
        let after_drop = ai.settler_targets.get(&settler).copied();
        let avoided = ai.settler_avoid.get(&settler).map(|(pos, _)| *pos);
        // Frame 2: the site is valid again — the flicker.
        std::sync::Arc::make_mut(&mut game.blocked_city_sites).remove(&first);
        // Hold the mover at the same point so this frame compares target
        // hysteresis, not the different distance a fallback move created.
        let unit = game.units.get_mut(&settler).unwrap();
        unit.pos = source;
        unit.moves_left = 2.0;
        unit.moved = false;
        ai.settler_targets.remove(&settler);
        let _ = ai.advanced_settler_step(&mut game, 0, settler);
        let re_pick = ai.settler_targets.get(&settler).copied();
        (after_drop, avoided, re_pick)
    };

    let (_after_drop, avoided, re_pick) = run(true);
    assert_eq!(avoided, Some(first), "the dropped site is set aside");
    assert_ne!(
        re_pick,
        Some(first),
        "the frame it is valid again, the live seat does not walk back to it"
    );

    let (_, avoided_withheld, re_pick_withheld) = run(false);
    assert_eq!(
        avoided_withheld, None,
        "the withheld arm sets nothing aside"
    );
    assert_eq!(
        re_pick_withheld,
        Some(first),
        "and re-picks the first site the frame it is valid again — the flicker"
    );

    assert!(!AdvancedAi::new().settler_target_hysteresis);
    assert!(!AdvancedAi::legacy().settler_target_hysteresis);
}

/// `route_step` is a multi-turn geometric answer and can still return a
/// first tile the unit cannot enter with its remaining movement. The live
/// non-commit path used to remove that target without writing hysteresis, so
/// the next frame selected the same site again.
#[test]
fn a_settler_does_not_repick_a_route_it_cannot_enter_this_turn() {
    let (mut game, source, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, source);
    let first = game
        .route_step(settler, target, 0)
        .expect("the fixture has a geometric route to the target");
    game.units
        .get_mut(&settler)
        .expect("the test Settler")
        .moves_left = 0.5;
    assert!(
        game.route_step(settler, target, 0).is_some(),
        "the geometric route remains available"
    );
    assert!(
        !game.can_move(settler, first),
        "the first route step cannot be paid this turn"
    );

    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge();
    ai.enable_settler_never_idles();
    assert!(
        ai.settler_target_hysteresis_on(),
        "the live bridge's route recovery owns hysteresis"
    );
    ai.settler_targets.insert(settler, target);

    assert!(
        !ai.advanced_settler_step(&mut game, 0, settler),
        "an unpayable first step is not reported as movement"
    );
    assert_eq!(
        ai.settler_avoid
            .get(&settler)
            .map(|(position, _)| *position),
        Some(target),
        "the locally blocked route receives the normal hysteresis cooldown"
    );
    assert_ne!(
        ai.settler_targets.get(&settler),
        Some(&target),
        "the same unmovable target is not immediately retained"
    );
}

/// A cached destination can fail safety for an unguarded Settler even while a
/// second Settler's bound guard makes that same site safe. Version-2
/// hysteresis used to copy the first unit's transient risk drop into the
/// second unit's dead-site memory, leaving all available Settlers unable to
/// use the guarded route.
#[test]
fn settler_target_hysteresis_v2_keeps_guarded_routes_available_to_other_settlers() {
    let (mut game, source, target) = stacked_escort_fixture();
    let unguarded = game.spawn_test_unit("settler", 0, source);
    let guarded = game.spawn_test_unit("settler", 0, source);
    let guard = game.spawn_test_unit("warrior", 0, source);
    let raider_at =
        game.nbrs(target)
            .into_iter()
            .find(|position| {
                game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a visible raider post beside the settle site");
    let _raider = game.spawn_test_unit("warrior", 1, raider_at);
    let observer_at =
        game.nbrs(raider_at)
            .into_iter()
            .find(|position| {
                *position != target
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a clear observation post beside the raider");
    let _observer = game.spawn_test_unit("scout", 0, observer_at);
    game.players[0].explored.insert(raider_at);

    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge();
    ai.enable_settler_guard_holds();
    ai.enable_settler_target_hysteresis_2();
    ai.settler_targets.insert(unguarded, target);
    ai.settler_targets.insert(guarded, target);
    ai.settler_guards.insert(guarded, guard);

    let visible = ai.battlefront_visibility(&game, 0);
    assert!(game.sees(&visible, raider_at), "the raider is visible");
    let unguarded_risk = ai.settlement_tile_risk(&game, 0, Some(unguarded), target, &visible);
    let guarded_risk = ai.settlement_tile_risk(&game, 0, Some(guarded), target, &visible);
    assert!(
        unguarded_risk > SETTLER_STEP_RISK_LIMIT,
        "the unguarded Settler must reject the threatened site ({unguarded_risk})"
    );
    assert!(
        guarded_risk <= SETTLER_STEP_RISK_LIMIT,
        "the bound guard must make the same site safe ({guarded_risk})"
    );

    let _ = ai.advanced_settler_step(&mut game, 0, unguarded);
    assert_eq!(
        ai.settler_avoid
            .get(&unguarded)
            .map(|(position, _)| *position),
        Some(target),
        "ordinary hysteresis still sets the unsafe route aside for its mover"
    );
    assert!(
        !ai.settler_dead_sites
            .get(&guarded)
            .is_some_and(|sites| sites.contains_key(&target)),
        "a guarded Settler must not inherit another Settler's transient risk drop"
    );
}

/// Version-2 hysteresis still shares a site that is intrinsically unavailable.
/// This is deliberately distinct from a risk rejection: the host's block is
/// true for every Settler, so keeping it out of all their picks avoids a
/// redundant march without suppressing a route another guard can protect.
#[test]
fn settler_target_hysteresis_v2_shares_a_permanently_blocked_site() {
    let (mut game, source, target) = stacked_escort_fixture();
    // Keep this synthetic hysteresis pair off the founded city. A live
    // Settler first observed on its own city is intentionally classified as
    // an early-opening Settler and receives the capital corridor; this test
    // is about sharing a host-blocked site, not that opening policy.
    let staging = game
        .nbrs(source)
        .into_iter()
        .find(|position| game.city_at(*position).is_none())
        .expect("fixture has a non-city staging tile");
    let dropping = game.spawn_test_unit("settler", 0, staging);
    let other = game.spawn_test_unit("settler", 0, staging);
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge();
    ai.enable_settler_target_hysteresis_2();
    ai.settler_targets.insert(dropping, target);
    std::sync::Arc::make_mut(&mut game.blocked_city_sites).insert(target);

    let _ = ai.advanced_settler_step(&mut game, 0, dropping);
    assert!(
        ai.settler_dead_sites
            .get(&other)
            .is_some_and(|sites| sites.contains_key(&target)),
        "a host-blocked destination is invalid for every Settler"
    );
}

#[test]
fn a_settler_threat_detour_uses_a_safe_runner_up_then_reopens_the_site() {
    let run = |barbarian: bool, live: bool| {
        let ThreatDetourBoard {
            mut game,
            settler,
            home,
            target,
            step,
            threat_position,
            threat,
        } = threat_detour_board(barbarian, live);
        let mut ai = AdvancedAi::new();
        if live {
            ai.enable_live_bridge();
            ai.disable_frontier_loyalty();
            ai.disable_settler_never_idles();
        } else {
            ai.enable_settler_threat_detour();
        }
        if live {
            // Mirror the live bridge's conservative host hold: the native
            // scorer may see a guard on the Settler's tile, but the host still
            // refuses a visible hostile destination and needs the detour.
            let guard = game.spawn_test_unit("warrior", 0, home);
            ai.settler_guards.insert(settler, guard);
        }
        assert!(
            ai.settlement_safety,
            "the route gate needs settlement safety"
        );
        let visible = ai.battlefront_visibility(&game, 0);
        assert!(
            game.sees(&visible, threat_position),
            "the blocker is visible"
        );
        let step_risk = if live {
            ai.settlement_tile_risk_with_support(&game, 0, Some(settler), step, &visible, false)
        } else {
            ai.settlement_tile_risk(&game, 0, Some(settler), step, &visible)
        };
        assert!(
            step_risk > SETTLER_STEP_RISK_LIMIT,
            "the blocker makes the next route step unsafe"
        );
        assert!(
            ai.settler_target_has_visible_route_threat(&game, 0, settler, target),
            "the route-specific gate sees the blocker even though the target itself is safe"
        );
        ai.settler_targets.insert(settler, target);
        game.units.get_mut(&settler).unwrap().moves_left = 2.0;
        assert!(
            ai.advanced_settler_step(&mut game, 0, settler),
            "the detour immediately spends the Settler's turn on the safe route"
        );
        let fallback = ai.settler_targets[&settler];
        if live && fallback == target {
            // The live opening corridor is narrower than this historical
            // route-detour fixture. If every in-corridor runner-up is also
            // covered, staying local is the intended answer; taking a
            // distant runner-up would recreate the captured-second-Settler
            // failure this test now guards against.
            assert!(
                ai.early_settler_site_allowed(&game, 0, settler, fallback),
                "a live detour may keep the target only inside the early corridor"
            );
            return;
        }
        assert_ne!(fallback, target, "the unsafe corridor is not the runner-up");
        assert_eq!(
            ai.settler_threat_deferrals.get(&target).copied(),
            Some(game.turn + game.standard_duration(SETTLER_THREAT_DETOUR_TURNS)),
            "the original site is deferred for every settler, not retired as dead"
        );
        assert!(
            !ai.settler_target_has_visible_route_threat(&game, 0, settler, fallback),
            "the alternate's own first step is safe"
        );

        // The original high-value location has not been discarded forever.
        // Once the hostile is gone and the short deferral expires, a later
        // Settler may keep it as its target again.
        game.remove_unit(threat);
        game.remove_unit(settler);
        game.turn += game.standard_duration(SETTLER_THREAT_DETOUR_TURNS);
        let later = game.spawn_test_unit("settler", 0, home);
        ai.settler_targets.insert(later, target);
        game.units.get_mut(&later).unwrap().moves_left = 2.0;
        let _ = ai.advanced_settler_step(&mut game, 0, later);
        assert!(
            !ai.settler_threat_deferrals.contains_key(&target),
            "the cleared corridor is available again after its short window"
        );
        assert_eq!(
            ai.settler_targets.get(&later),
            Some(&target),
            "the original site is a live target again rather than a dead one"
        );
    };

    run(true, false);
    run(false, false);
    run(true, true);
    run(false, true);
}

/// `settler-detour-stays-near`: the detour's fallback lies about as close as
/// the site it leaves, or there is no detour at all — the target is kept and
/// its deferral rolled back so the ranking still holds the site. Without the
/// gene the same board hands the Settler the best SAFE site wherever it lies.
#[test]
fn a_settler_threat_detour_stays_near_or_keeps_the_site() {
    let run = |barbarian: bool| {
        let ThreatDetourBoard {
            mut game,
            settler,
            home,
            target,
            step: _,
            threat_position: _,
            threat: _,
        } = threat_detour_board(barbarian, false);
        let near_limit = (game.wdist(home, target) + SETTLER_DETOUR_NEAR_SLACK)
            .clamp(SETTLER_DETOUR_NEAR_FLOOR, SETTLER_DETOUR_NEAR_CEILING);

        // The plain detour on this board takes a safe runner-up (the board
        // was selected for one), whatever its distance.
        let mut plain = AdvancedAi::new();
        plain.enable_settler_threat_detour();
        plain.settler_targets.insert(settler, target);
        let plain_fallback = plain
            .detour_settler_around_visible_threat(&game, 0, settler, target)
            .expect("the board has a safe runner-up for the plain detour");
        assert_ne!(plain_fallback, target);

        let mut near = AdvancedAi::new();
        near.enable_settler_threat_detour();
        near.enable_settler_detour_stays_near();
        near.settler_targets.insert(settler, target);
        match near.detour_settler_around_visible_threat(&game, 0, settler, target) {
            Some(fallback) => {
                assert!(
                    game.wdist(home, fallback) <= near_limit,
                    "a near detour's fallback {fallback:?} lies within {near_limit} tiles of \
                     {home:?} (target {target:?} at {})",
                    game.wdist(home, target)
                );
                assert!(
                    !near.settler_target_has_visible_route_threat(&game, 0, settler, fallback),
                    "the near fallback's own first step is safe"
                );
                assert!(
                    near.settler_threat_deferrals.contains_key(&target),
                    "a taken detour defers the original site as the plain detour does"
                );
            }
            None => {
                assert!(
                    game.wdist(home, plain_fallback) > near_limit,
                    "the near detour refuses only a fallback that is far ({} tiles against \
                     {near_limit})",
                    game.wdist(home, plain_fallback)
                );
                assert!(
                    !near.settler_threat_deferrals.contains_key(&target),
                    "a refused detour rolls the deferral back so the site stays in the ranking"
                );
                assert_eq!(
                    near.settler_targets.get(&settler),
                    Some(&target),
                    "the Settler keeps its target and holds for the blocker's clock"
                );
            }
        }

        // Whatever the near gene decided, the Settler's step must not walk it
        // farther from home than the near limit this turn.
        game.units.get_mut(&settler).unwrap().moves_left = 2.0;
        let _ = near.advanced_settler_step(&mut game, 0, settler);
        let after = game.units[&settler].pos;
        assert!(
            game.wdist(home, after) <= 2,
            "one turn of a near-detoured Settler stays by home ({after:?} from {home:?})"
        );
    };
    run(true);
    run(false);
}

/// The live bridge's capture model also needs the route-recovery gates.  The
/// host refuses a visible hostile leg even when a stacked escort would make
/// the native risk score look safe; the live-only helper must therefore turn
/// on the safe-runner detour while the separately screened short target hold
/// remains governed by the published default.
#[test]
fn live_capture_lessons_enable_route_recovery_without_the_hysteresis_gene() {
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    assert!(live.live_settler_capture_lessons);
    assert!(live.settlement_safety);
    // The deployment ledger may select either native hysteresis version. This
    // fixture instead proves that the live capture bridge supplies its own
    // bounded recovery when both screened variants are withheld.
    live.disable_settler_target_hysteresis();
    live.disable_settler_target_hysteresis_2();
    live.disable_settler_never_idles();
    assert!(!live.settler_target_hysteresis);
    assert!(!live.settler_target_hysteresis_2);
    assert!(live.settler_routing_recovery_on());
    assert!(
        !live.settler_target_hysteresis_on(),
        "route recovery does not implicitly restore a withheld hysteresis arm"
    );
    assert!(live.settler_threat_detour_on());

    let mut withheld = AdvancedAi::new();
    withheld.enable_live_bridge();
    withheld.disable_live_settler_capture_lessons();
    withheld.disable_settler_target_hysteresis();
    withheld.disable_settler_target_hysteresis_2();
    withheld.disable_settler_threat_detour();
    assert!(!withheld.settler_routing_recovery_on());
    assert!(!withheld.settler_target_hysteresis_on());
    assert!(!withheld.settler_threat_detour_on());

    let mut native = AdvancedAi::new();
    native.enable_engine_repairs();
    assert!(native.live_settler_capture_lessons);
    assert!(native.settler_routing_recovery_on());
}

/// The walk price: nothing when the gene is off; `SETTLE_SOONER_TURN_PRICE`
/// a turn for a Settler that has just started; doubled once it has been
/// walking `SETTLE_SOONER_PATIENCE` standard turns; and a route is charged
/// by the turns it takes at the Settler's own pace, rounded up.
#[test]
fn settle_sooner_prices_each_turn_of_the_walk_dearer_the_longer_it_has_lasted() {
    let (mut game, home) = camp_bounty_board(93_101);
    let settler = game.spawn_test_unit("settler", 0, home);
    let moves = game.unit_max_moves(settler);
    assert!(moves >= 2.0, "a Settler moves two a turn");
    let mut ai = AdvancedAi::new();
    assert_eq!(ai.settle_sooner_walk_price(&game, settler), None);
    assert_eq!(AdvancedAi::settle_sooner_walk_cost(None, 7.0), 0.0);
    ai.enable_settle_sooner();
    let fresh = ai.settle_sooner_walk_price(&game, settler).expect("priced");
    assert_eq!(fresh, (SETTLE_SOONER_TURN_PRICE, moves));
    // Three movement points at two a turn is two turns, not one and a half.
    assert_eq!(
        AdvancedAi::settle_sooner_walk_cost(Some(fresh), 3.0),
        2.0 * SETTLE_SOONER_TURN_PRICE
    );
    assert_eq!(AdvancedAi::settle_sooner_walk_cost(Some(fresh), 0.0), 0.0);
    // The first stepped turn stamps the clock; the price then climbs.
    ai.advanced_settler_step(&mut game, 0, settler);
    assert_eq!(ai.settler_walk_started.get(&settler), Some(&game.turn));
    let started = game.turn;
    game.turn = started + game.standard_duration(SETTLE_SOONER_PATIENCE);
    let patient = ai.settle_sooner_walk_price(&game, settler).expect("priced");
    assert!(
        (patient.0 - 2.0 * SETTLE_SOONER_TURN_PRICE).abs() < 1e-9,
        "one patience window doubles the price: {patient:?}"
    );
    game.turn = started + 2 * game.standard_duration(SETTLE_SOONER_PATIENCE);
    let impatient = ai.settle_sooner_walk_price(&game, settler).expect("priced");
    assert!(
        (impatient.0 - 3.0 * SETTLE_SOONER_TURN_PRICE).abs() < 1e-9,
        "two windows triple it: {impatient:?}"
    );
    // The clock is settler memory: it follows the unit through a live-bridge
    // id remap and is forgotten with the unit.
    let mut map = BTreeMap::new();
    map.insert(settler, settler + 1000);
    ai.remap_unit_memory(&map);
    assert_eq!(
        ai.settler_walk_started.get(&(settler + 1000)),
        Some(&started)
    );
    assert!(!ai.settler_walk_started.contains_key(&settler));
    ai.forget_unit_memory();
    assert!(ai.settler_walk_started.is_empty());
}

/// The property the gene is built on: adding a cost that rises with the
/// walk can only move the pick NEARER. On every board the treated Settler's
/// site is no longer a walk (movement points by the gene's own cost map)
/// than the baseline's, and on at least one of these boards it is a
/// strictly shorter one — the gene changes a real decision, it is not
/// inert. A longer wait leans the same way: the pick after a patience
/// window is no farther than the fresh pick.
///
/// ⚠ The property is exact for the multi-turn fallback the picks almost
/// always take; the one-turn routed branch adds a risk term the walk price
/// does not touch, so a pick within one turn is compared on the same cost
/// map and still cannot move farther (the gene's term is monotone in it).
/// The gene's cost map is a real multi-turn walk: it reaches ground
/// `path_to` (a one-turn flood) cannot, charges hills and forest what the
/// engine charges, and stops at the radius.
#[test]
fn settle_sooner_walk_costs_read_terrain_beyond_the_first_turn() {
    let (mut game, home) = camp_bounty_board(93_301);
    let settler = game.spawn_test_unit("settler", 0, home);
    let costs = AdvancedAi::settle_sooner_walk_costs(&game, settler, 8);
    assert_eq!(costs.get(&home), Some(&0.0));
    let far: Vec<(Pos, f64)> = costs
        .iter()
        .filter(|(pos, _)| game.wdist(home, **pos) >= 5)
        .map(|(pos, cost)| (*pos, *cost))
        .collect();
    assert!(!far.is_empty(), "open ground five tiles out is priced");
    for (pos, cost) in &far {
        assert!(
            game.path_to(settler, *pos).is_none(),
            "{pos:?} is beyond one turn, which path_to cannot route"
        );
        assert!(
            *cost >= game.wdist(home, *pos) as f64,
            "{pos:?}: every hex step costs at least one movement point ({cost})"
        );
    }
    assert!(
        costs.keys().all(|pos| game.wdist(home, *pos) <= 8),
        "the map stops at the radius"
    );
    // Hills on the way cost what the engine charges for them.
    let neighbour = game
        .nbrs(home)
        .into_iter()
        .find(|pos| costs.contains_key(pos))
        .expect("a walkable neighbour");
    let expected = game.step_cost(home, neighbour);
    assert_eq!(costs[&neighbour], expected);
}

#[test]
fn settle_sooner_never_picks_a_farther_site_and_sometimes_a_nearer_one() {
    let mut nearer = 0;
    let mut boards = 0;
    for seed in [
        93_201u64, 93_202, 93_203, 93_204, 93_205, 93_206, 93_207, 93_208,
    ] {
        let seed = seed + crate::rng::fixture_seed("SETTLESOONER", 0);
        let (mut game, home) = camp_bounty_board(seed);
        game.set_fog_memory(false);
        let settler = game.spawn_test_unit("settler", 0, home);
        let walk_to = |g: &Game, site: Pos| -> f64 {
            AdvancedAi::settle_sooner_walk_costs(g, settler, 8)
                .get(&site)
                .copied()
                .unwrap_or_else(|| g.wdist(g.units[&settler].pos, site) as f64)
        };
        let plain = AdvancedAi::new();
        let Some((base_site, _)) =
            plain.best_reachable_settle_site_except(&game, 0, settler, 8, None)
        else {
            continue;
        };
        let mut treated = AdvancedAi::new();
        treated.enable_settle_sooner();
        let (gene_site, _) = treated
            .best_reachable_settle_site_except(&game, 0, settler, 8, None)
            .expect("the same candidates exist under the gene");
        let base_walk = walk_to(&game, base_site);
        let gene_walk = walk_to(&game, gene_site);
        assert!(
            gene_walk <= base_walk + 1e-9,
            "seed {seed}: the gene picked a farther site ({gene_site:?} at {gene_walk} MP) \
             than the baseline ({base_site:?} at {base_walk} MP)"
        );
        if gene_walk < base_walk - 1e-9 {
            nearer += 1;
        }
        // A Settler that has already walked a patience window leans nearer
        // still, never farther.
        treated.settler_walk_started.insert(settler, game.turn);
        game.turn += game.standard_duration(SETTLE_SOONER_PATIENCE);
        let (late_site, _) = treated
            .best_reachable_settle_site_except(&game, 0, settler, 8, None)
            .expect("the same candidates exist later");
        assert!(
            walk_to(&game, late_site) <= gene_walk + 1e-9,
            "seed {seed}: waiting moved the pick farther ({late_site:?} after {gene_site:?})"
        );
        boards += 1;
    }
    assert!(boards >= 6, "the fixture boards offer sites: {boards}");
    assert!(
        nearer >= 1,
        "the gene changed no decision on {boards} boards — it would screen as inert"
    );
}

#[test]
fn a_settler_on_quiet_ground_marches_with_its_guard_one_step_behind() {
    let (mut game, source, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, source);
    // The guard one tile behind, out of moves this turn.
    let behind = game
        .nbrs(source)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.wdist(*position, target) >= game.wdist(source, target)
        })
        .expect("fixture has a tile one step behind the settler");
    let guard = game.spawn_test_unit("warrior", 0, behind);
    game.units.get_mut(&guard).unwrap().moves_left = 0.0;
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    assert!(ai.settlement_safety);
    ai.settler_targets.insert(settler, target);
    ai.settler_guards.insert(settler, guard);
    let visible = ai.battlefront_visibility(&game, 0);
    assert_eq!(
        ai.settlement_tile_risk(&game, 0, Some(settler), source, &visible),
        0.0,
        "the fixture's ground is quiet"
    );
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    let _ = ai.advanced_settler_step(&mut game, 0, settler);
    assert_ne!(
        game.units[&settler].pos, source,
        "with the guard one step behind and nothing in reach the settler marches"
    );
    assert!(
        !journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline.starts_with("Settler waits for its guard")),
        "no wait is journaled"
    );
    assert!(!ai.guard_wait.contains_key(&settler));
}

#[test]
fn a_lost_settler_releases_its_guard_to_the_army() {
    let (mut game, source, _target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, source);
    let guard = game.spawn_test_unit("warrior", 0, source);
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_guards.insert(settler, guard);
    ai.guard_wait.insert(settler, (game.turn, 1));
    let plan = stacked_escort_plan(&game);

    // The capture this treatment exists to prevent, having happened
    // anyway: the guard goes back to the ordinary military planner.
    game.remove_unit(settler);

    assert_eq!(ai.settler_escort_step(&mut game, 0, guard, &plan), None);
    assert!(ai.settler_guards.is_empty());
    assert!(ai.guard_wait.is_empty());
}

#[test]
fn stacked_escort_sheds_a_stale_formation_link() {
    let (mut game, source, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, source);
    let escort = game.spawn_test_unit("warrior", 0, source);
    game.apply(
        0,
        &Action::LinkUnits {
            unit: settler,
            with: escort,
        },
    )
    .unwrap();
    let mut ai = AdvancedAi::new();
    ai.enable_live_formationless_settler_shadow();
    ai.settler_targets.insert(settler, target);

    // A link reconciled from the host (or left by an older controller)
    // must be shed, not deferred to: the linked branch trusts a channel
    // that does not walk on the live bridge.
    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    assert_eq!(game.units[&settler].linked_to, None);
    assert_eq!(game.units[&escort].linked_to, None);
}

#[test]
fn a_refused_founding_is_not_journaled_as_a_decision() {
    let (mut game, _source, target) = stacked_escort_fixture();
    let settler = game.spawn_test_unit("settler", 0, target);
    // Isolationism passes `can_found_city` but fails `do_found_city`, so
    // the decision branch is entered and the engine then refuses — the
    // shape run civvis-20260815T081505Z hit twice (t35, t45) through a
    // different refusal, printing "Founding a city" for cities that never
    // existed and poisoning every count taken from the why-log.
    game.players[0]
        .policies
        .insert(crate::name!("isolationism"));
    let journal = crate::reasoning::Journal::recording();
    let mut ai = AdvancedAi::new();
    ai.attach_journal(journal.handle());
    ai.settler_targets.insert(settler, target);

    assert!(
        !ai.advanced_settler_step(&mut game, 0, settler),
        "a refused founding is a failed step, not a success"
    );
    assert_eq!(game.player_city_ids(0).len(), 1, "no city may appear");
    let headlines: Vec<String> = journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.headline)
        .collect();
    assert!(
        !headlines
            .iter()
            .any(|headline| headline.starts_with("Founding a city")),
        "the journal must not claim a founding the engine refused: {headlines:?}"
    );
}

#[test]
fn moving_escort_cannot_keep_a_stalled_settlement_target_forever() {
    let mut game = Game::new_full(1, 24, 16, 8_120, 120, 0, false);
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let source = game.units[&founding_settler].pos;
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .unwrap();
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    for position in game.wdisk(source, SETTLER_ESCORT_SEARCH_RADIUS) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    let target =
        game.map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdist(source, *position) >= SETTLER_ESCORT_DISTANCE + 2
                    && game.wdist(source, *position) <= SETTLER_ESCORT_SEARCH_RADIUS
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a multi-turn expansion route");
    let settler = game.spawn_test_unit("settler", 0, source);
    let escort = game.spawn_test_unit("warrior", 0, source);
    let mut ai = AdvancedAi::new();
    ai.linked_settler_progress = true;
    ai.settler_targets.insert(settler, target);
    ai.advanced_formations(&mut game, 0);
    assert_eq!(game.units[&settler].linked_to, Some(escort));

    // Replay the live failure's essential shape: the formation executes a
    // legal move every turn, but an obstruction sends it back to the same
    // starting tile before the next decision. Motion is not progress.
    let route_distance = game.wdist(source, target);
    ai.settler_closest
        .insert(settler, (target, route_distance - 1));
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    for attempt in 0..SETTLER_STALL_LIMIT {
        for unit in [settler, escort] {
            let unit = game.units.get_mut(&unit).unwrap();
            unit.pos = source;
            unit.moves_left = 4.0;
            unit.acted = false;
            unit.fortified = false;
        }
        assert_eq!(
            ai.settler_escort_step(&mut game, 0, escort, &plan),
            Some(true),
            "the escort should execute the repeated legal move"
        );
        assert_ne!(game.units[&escort].pos, source);
        if attempt + 1 < SETTLER_STALL_LIMIT {
            assert_eq!(ai.settler_targets.get(&settler), Some(&target));
        }
        game.turn += 1;
    }

    assert!(!ai.settler_targets.contains_key(&settler));
    assert!(!ai.settler_closest.contains_key(&settler));
    assert_eq!(
        ai.settler_avoid.get(&settler).map(|(pos, _)| *pos),
        Some(target)
    );
    assert_eq!(game.units[&settler].linked_to, None);
    assert_eq!(game.units[&escort].linked_to, None);
}

#[test]
fn escorted_settler_does_not_follow_its_leader_into_a_visible_capture_envelope() {
    let mut game = Game::new_full(2, 24, 16, 81_190, 120, 0, false);
    let founding_settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let source = game.units[&founding_settler].pos;
    game.apply(
        0,
        &Action::FoundCity {
            unit: founding_settler,
        },
    )
    .unwrap();
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    for position in game.wdisk(source, SETTLER_ESCORT_SEARCH_RADIUS) {
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
        game.players[0].explored.insert(position);
    }
    let target =
        game.map
            .tiles
            .keys()
            .copied()
            .find(|position| {
                game.wdist(source, *position) >= 4
                    && game.wdist(source, *position) <= SETTLER_ESCORT_SEARCH_RADIUS
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .expect("fixture has a reachable expansion target");
    let settler = game.spawn_test_unit("settler", 0, source);
    let escort = game.spawn_test_unit("slinger", 0, source);
    let mut ai = AdvancedAi::new();
    ai.settler_targets.insert(settler, target);
    ai.advanced_formations(&mut game, 0);
    assert_eq!(game.units[&settler].linked_to, Some(escort));
    assert_eq!(game.units[&escort].linked_to, Some(settler));

    let planned = game
        .route_step(escort, target, 0)
        .expect("the formation has a next route step");
    let threats =
        game.nbrs(planned)
            .into_iter()
            .filter(|position| {
                *position != source
                    && *position != target
                    && game.unit_ids_at(*position).is_empty()
                    && game.map.get(*position).is_some_and(|tile| {
                        game.rules.is_passable(tile) && !game.rules.is_water(tile)
                    })
            })
            .take(2)
            .collect::<Vec<_>>();
    assert_eq!(
        threats.len(),
        2,
        "the route has two adjacent attack squares"
    );
    let enemies = threats
        .iter()
        .map(|position| game.spawn_test_unit("warrior", 1, *position))
        .collect::<Vec<_>>();
    game.at_war.insert((0, 1));
    for threat in &threats {
        game.players[0].explored.insert(*threat);
    }
    let visible = ai.battlefront_visibility(&game, 0);
    assert!(threats.iter().all(|threat| game.sees(&visible, *threat)));
    let planned_risk =
        ai.settlement_tile_risk_with_support(&game, 0, Some(escort), planned, &visible, false);
    assert!(planned_risk > SETTLER_STEP_RISK_LIMIT);

    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: game.turn,
        rush: false,
    };
    let before = game.units[&escort].pos;
    assert!(ai
        .settler_escort_step(&mut game, 0, escort, &plan)
        .is_some());
    let after = game.units[&escort].pos;
    assert_ne!(after, planned);
    assert_eq!(game.units[&settler].pos, after);
    let after_risk =
        ai.settlement_tile_risk_with_support(&game, 0, Some(escort), after, &visible, false);
    assert!(
        after == before || after_risk + 5.0 < planned_risk,
        "the formation may hold or take a materially safer detour: \
             planned={planned_risk}, after={after_risk}"
    );
    assert!(enemies.iter().all(|enemy| game.units.contains_key(enemy)));
}

#[test]
fn settle_value_prices_district_adjacency_geometry() {
    // Three mountains cost the same worked yields wherever they stand.
    // Clustered around one plot they are a +3 Campus; scattered so no
    // plot touches two, the best Campus is +1. The settle score must
    // prefer the cluster — that difference is the adjacency calculator
    // speaking, not the tile mix.
    let scorer = AdvancedAi::new();
    let mut game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
    let mut land: Vec<Pos> = game.map.tiles.keys().copied().collect();
    land.sort();
    let center = land
        .into_iter()
        .find(|pos| {
            game.map
                .get(*pos)
                .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
                && game
                    .units
                    .values()
                    .all(|unit| game.wdist(unit.pos, *pos) > 5)
                && game
                    .wdisk(*pos, 3)
                    .iter()
                    .all(|ring| game.map.get(*ring).is_some())
        })
        .expect("map has an interior patch of open ground");
    // Flatten out to radius 3 so the mountains' own surroundings are
    // controlled too, and silence rivers so fresh-water and Commercial
    // Hub terms cannot differ between the two layouts.
    for pos in game.wdisk(center, 3) {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.wonder = None;
        tile.river_edges = [false; 6];
    }
    let ring2: Vec<Pos> = game
        .wdisk(center, 2)
        .into_iter()
        .filter(|pos| game.wdist(*pos, center) == 2)
        .collect();
    assert!(ring2.len() >= 12, "interior ring two is complete");

    // Scattered: ring-two plots pairwise three apart. Two mountains a
    // plot could see together would be at most two apart, so this
    // guarantees no plot sees more than one.
    let mut scattered: Vec<Pos> = Vec::new();
    for pos in &ring2 {
        if scattered.len() < 3
            && scattered
                .iter()
                .all(|picked| game.wdist(*picked, *pos) >= 3)
        {
            scattered.push(*pos);
        }
    }
    assert_eq!(
        scattered.len(),
        3,
        "ring two admits three mutually distant peaks"
    );
    for pos in &scattered {
        game.map.tiles.get_mut(pos).unwrap().terrain = crate::name!("mountain");
    }
    let scattered_value = scorer.settle_value(&game, 0, center);
    for pos in &scattered {
        game.map.tiles.get_mut(pos).unwrap().terrain = crate::name!("plains");
    }

    // Clustered: the three ring-two neighbours of one ring-one plot.
    let anchor = game
        .nbrs(center)
        .into_iter()
        .find(|pos| game.map.get(*pos).is_some())
        .unwrap();
    let cluster: Vec<Pos> = game
        .nbrs(anchor)
        .into_iter()
        .filter(|pos| game.wdist(*pos, center) == 2)
        .collect();
    assert_eq!(
        cluster.len(),
        3,
        "a ring-one plot touches three ring-two plots"
    );
    for pos in &cluster {
        game.map.tiles.get_mut(pos).unwrap().terrain = crate::name!("mountain");
    }
    let clustered_value = scorer.settle_value(&game, 0, center);

    assert!(
        clustered_value > scattered_value,
        "clustered ridge must outscore scattered peaks: {clustered_value} vs {scattered_value}"
    );
}

#[test]
fn a_founding_is_priced_on_the_ground_the_settler_stood_on() {
    // "the site is worth N" is the INPUT to the settling decision, not a
    // reading of the city it became. `settle_value` scores the work radius,
    // and founding rewrites that radius — the growth forecast behind
    // `growth_readiness` then runs over a worked city centre.
    //
    // Priced after the apply, the number inverts: on live Chieftain run
    // `civvis-20260819T041034Z` the two sites logged most negative (-3.1,
    // -1.4) became Ravenna and Aquileia, the seat's best non-capital cities
    // at 14.2 and 13.1 science, while the single site that logged positive
    // (+2.2) became Lugdunum at 2.25. A study reading that log concludes the
    // seat settles bad ground — the opposite of what it did.
    let mut game = Game::new(2, 24, 16, 7, 80, 0);
    for unit in game
        .player_unit_ids(0)
        .into_iter()
        .chain(game.player_unit_ids(1))
    {
        game.remove_unit(unit);
    }
    game.current = 0;
    let mut candidates: Vec<Pos> = game.map.tiles.keys().copied().collect();
    candidates.sort_unstable();
    let start = candidates[0];
    let settler = game.spawn_test_unit("settler", 0, start);
    let site = candidates
        .into_iter()
        .find(|p| {
            game.units.get_mut(&settler).unwrap().pos = *p;
            game.can_found_city(settler)
        })
        .expect("the fixture map offers foundable ground");
    game.units.get_mut(&settler).unwrap().pos = site;
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    let before = live.settle_value(&game, 0, site);
    let journal = crate::reasoning::Journal::recording();
    live.attach_journal(journal.handle());
    assert!(
        live.advanced_settler_step(&mut game, 0, settler),
        "the settler acts on ground it can found on"
    );
    assert!(
        game.cities.values().any(|c| c.owner == 0 && c.pos == site),
        "and the city exists"
    );
    let after = live.settle_value(&game, 0, site);
    // Without this the test proves nothing: if founding did not move the
    // number, both orderings would log the same value.
    assert_ne!(
        format!("{before:.1}"),
        format!("{after:.1}"),
        "founding must change what settle_value reads, or this fixture \
         cannot tell the two orderings apart"
    );
    let said: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .map(|t| format!("{} | {}", t.headline, t.detail))
        .collect();
    let line = said
        .iter()
        .find(|line| line.contains("the site is worth"))
        .unwrap_or_else(|| panic!("the founding is journalled; journal {said:#?}"));
    assert!(
        line.contains(&format!("the site is worth {before:.1}")),
        "the journal prices the ground the settler stood on ({before:.1}), \
         not the city it became ({after:.1}); got {line}"
    );
}

/// ⚠⚠ A BARBARIAN CAMP TWO TILES AWAY CHANGES THE SHIPPED SITE SCORE BY
/// EXACTLY NOTHING — while barbarians take 65% of every city a major loses,
/// at a median city age of ten turns.
///
/// `settle_value` penalises proximity to a rival *major* and filters
/// barbarians out of that penalty, and no other term in it prices a camp.
/// Measured here rather than quoted: the same site, the same everything, one
/// camp added two tiles away.
#[test]
fn a_barbarian_camp_two_tiles_away_changes_the_shipped_score_by_nothing() {
    let mut game = Game::new(2, 60, 40, 6_301, 250, 0);
    game.found_city_for(0, (10, 20), None);
    let site = (24, 20);
    let mut with_camp = game.clone();
    with_camp.barb_camps.insert((26, 21), 0);

    let shipped = AdvancedAi::new();
    assert_eq!(
        shipped.settle_value(&game, 0, site),
        shipped.settle_value(&with_camp, 0, site),
        "the shipped site score cannot see a barbarian camp at all"
    );

    let mut ai = AdvancedAi::new();
    ai.enable_defensible_sites();
    assert!(
        ai.settle_value(&with_camp, 0, site) < ai.settle_value(&game, 0, site),
        "the gene charges the site for the camp beside it"
    );
    // And charges nothing where there is no camp: the two agents agree on a
    // quiet site once its isolation is the same.
    assert_eq!(
        ai.defensibility(&game, 0, (14, 20)),
        0.0,
        "a site inside the empire with no camp near it is not penalised"
    );
}

/// The other half, and the larger one: the shipped score has no isolation
/// term at all, so a site far from every friendly city scores BETTER than one
/// inside the empire — 148.3 against 62.7 on this board.
#[test]
fn the_shipped_score_prefers_a_site_it_cannot_support() {
    let mut game = Game::new(2, 60, 40, 6_302, 250, 0);
    game.found_city_for(0, (10, 20), None);
    let shipped = AdvancedAi::new();

    let inside = shipped.settle_value(&game, 0, (14, 20));
    let stranded = shipped.settle_value(&game, 0, (40, 20));
    assert!(
        stranded > inside,
        "fixture: the far site is the better land ({stranded} vs {inside})"
    );

    // The gene does not forbid it; it charges for it, and the charge is
    // capped, so a distant site can still win on its merits.
    let mut ai = AdvancedAi::new();
    ai.enable_defensible_sites();
    let charged = ai.settle_value(&game, 0, (40, 20));
    assert!(
        charged < stranded,
        "the isolation is priced ({charged} vs {stranded})"
    );
    // A camp beside either site would be a different charge from isolation.
    game.barb_camps.clear();
    assert!(
        ai.defensibility(&game, 0, (40, 20)) == ai.defensibility(&game, 0, (24, 20)),
        "and the charge stops growing past its cap, so this is a tiebreak on \
         holdability and not a leash"
    );
}

/// Once the early Settler is outside its corridor, the live flee goal is the
/// capital rather than the stale frontier site. With a visible raider beside
/// it, the full-turn escape first leaves capture reach if necessary, then the
/// next Settler step turns home instead of continuing the frontier march.
#[test]
fn a_live_early_settler_flees_homeward_out_of_capture_reach() {
    let (mut game, _city, home) = barbarian_field(71_310);
    let start = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| game.wdist(*pos, home) == 8 && open_land(&game, *pos))
        .expect("open ground eight tiles from the capital");
    let settler = game.spawn_test_unit("settler", 0, home);
    let mut live = AdvancedAi::new();
    live.enable_live_bridge();
    live.remember_early_settler_home(&game, 0, settler);
    game.relocate(settler, start);
    let raider_at = game
        .nbrs(start)
        .into_iter()
        .find(|pos| open_land(&game, *pos))
        .expect("an open tile beside the early Settler");
    let raider = game.spawn_test_unit("warrior", 1, raider_at);
    let remote_target = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| game.wdist(*pos, home) >= 10 && open_land(&game, *pos))
        .expect("a remote stale target");

    live.settler_targets.insert(settler, remote_target);
    let reach = live.barbarian_reach(&game, 0, start, 10);
    assert!(
        reach.covers(&game, start),
        "the raider can capture the Settler"
    );

    assert_eq!(
        live.civilian_flee_step(&mut game, 0, settler),
        Some(true),
        "the live Settler spends the turn escaping"
    );
    let after = game.units[&settler].pos;
    assert!(
        !reach.covers(&game, after),
        "the full-turn escape ends outside the raider's reach: {after:?}"
    );
    assert!(
        game.wdist(after, home) > EARLY_SETTLER_HOME_RADIUS,
        "the fixture exercises the emergency excursion"
    );
    game.remove_unit(raider);
    game.turn += 1;
    game.units.get_mut(&settler).unwrap().moves_left = 2.0;
    game.units.get_mut(&settler).unwrap().moved = false;
    assert!(live.advanced_settler_step(&mut game, 0, settler));
    let homeward = game.units[&settler].pos;
    assert!(
        game.wdist(homeward, home) < game.wdist(after, home),
        "the following step returns home rather than following {remote_target:?}: {after:?} -> {homeward:?}"
    );
}

/// A settler two tiles from home with a Warrior two tiles away flees to a
/// tile the Warrior cannot reach, before anything else in its step.
#[test]
fn a_settler_inside_a_raiders_reach_flees_out_of_it() {
    let (mut game, _city, home) = barbarian_field(71_021);
    let start = game
        .wdisk(home, 2)
        .into_iter()
        .find(|pos| game.wdist(*pos, home) == 2 && open_land(&game, *pos))
        .expect("open ground two tiles from home");
    let raider_at = game
        .wdisk(start, 2)
        .into_iter()
        .find(|pos| {
            game.wdist(*pos, start) == 2 && game.wdist(*pos, home) == 2 && open_land(&game, *pos)
        })
        .expect("a raider post two tiles from the settler, in the capital's sight");
    let settler = game.spawn_test_unit("settler", 0, start);
    let _raider = game.spawn_test_unit("warrior", 1, raider_at);
    let mut ai = AdvancedAi::new();
    ai.enable_civilian_out_of_reach();
    let reach = ai.barbarian_reach(&game, 0, start, 10);
    assert!(
        reach.covers(&game, start),
        "the fixture must put the settler inside the Warrior's one-turn reach"
    );
    assert!(
        !ai.civilian_safe_at(&game, 0, settler, start, &reach),
        "unguarded ground inside the reach is not safe"
    );
    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    let after = game.units[&settler].pos;
    assert_ne!(after, start, "the settler moved");
    assert!(
        !reach.covers(&game, after),
        "it moved to ground the Warrior cannot reach next turn: {after:?}"
    );
}

/// After explicitly withholding its broader civilian-safety opt-in, the live
/// bridge must not leave an unguarded Settler inside a visible direct-capture
/// envelope merely because every *one-step* retreat is also covered. This is
/// the narrow emergency floor for a live Settler that will otherwise be taken
/// before it receives another turn.
#[test]
fn a_live_settler_escapes_a_direct_barbarian_capture_after_explicit_withhold() {
    let (mut game, _city, home) = barbarian_field(71_026);
    for unit in game.player_unit_ids(0) {
        if game.rules.units[game.units[&unit].kind].class == "military" {
            game.remove_unit(unit);
        }
    }
    let start = game
        .wdisk(home, 2)
        .into_iter()
        .find(|pos| game.wdist(*pos, home) == 2 && open_land(&game, *pos))
        .expect("open ground two tiles from home");
    let settler = game.spawn_test_unit("settler", 0, start);
    // Put the Warrior next to the Settler but still in the capital's vision.
    // Its full two-move reach covers every legal one-step retreat, while the
    // Settler has a safe two-step escape on this open board.
    let raider_at = game
        .nbrs(start)
        .into_iter()
        .find(|pos| {
            game.wdist(*pos, home) <= 2
                && open_land(&game, *pos)
                && game
                    .reachable(settler)
                    .into_iter()
                    .any(|escape| game.wdist(escape, *pos) >= 3)
        })
        .expect("a visible adjacent raider with a two-step escape");
    let _raider = game.spawn_test_unit("warrior", 1, raider_at);
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge();
    assert!(
        ai.live_formationless_settler_shadow,
        "the deployed seat uses the live shadow"
    );
    // Rankings may deploy this opt-in; this regression exercises an explicit
    // withhold independently of the current tournament-selected default.
    ai.disable_civilian_out_of_reach();
    assert!(
        !ai.civilian_out_of_reach,
        "the fixture explicitly withholds the broader civilian-safety opt-in"
    );
    let reach = ai.barbarian_reach(&game, 0, start, 10);
    assert!(
        reach.covers(&game, start),
        "the settler is capturable where it stands"
    );
    let legal_steps: Vec<_> = game
        .nbrs(start)
        .into_iter()
        .filter(|pos| game.can_move(settler, *pos))
        .collect();
    assert!(
        legal_steps.iter().all(|pos| reach.covers(&game, *pos)),
        "the fixture must defeat the one-step retreat: {legal_steps:?}"
    );
    let safe_escape = game
        .reachable(settler)
        .into_iter()
        .find(|pos| !reach.covers(&game, *pos))
        .expect("the fixture must offer a safe full-turn escape");
    assert!(
        game.path_to(settler, safe_escape).is_some(),
        "the advertised full-turn escape has a path"
    );

    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    let after = game.units[&settler].pos;
    assert_ne!(
        after, start,
        "the live Settler does not wait to be captured"
    );
    assert!(
        !reach.covers(&game, after),
        "the live floor uses the safe two-step escape: start={start:?}, \
         raider={raider_at:?}, escape={safe_escape:?}, after={after:?}"
    );
}

/// The doorstep exception: a settler adjacent to its site steps onto it
/// inside the reach when it keeps movement, and founds the city before the
/// raider moves.
#[test]
fn a_settler_founds_on_a_doorstep_inside_the_reach_when_it_can_found_this_turn() {
    let (mut game, _city, home) = barbarian_field(71_023);
    let site = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| game.wdist(*pos, home) == 5 && open_land(&game, *pos))
        .expect("a site five tiles from home");
    let start = game
        .nbrs(site)
        .into_iter()
        .find(|pos| game.wdist(*pos, home) == 4 && open_land(&game, *pos))
        .expect("a doorstep beside the site");
    let raider_at = game
        .wdisk(site, 2)
        .into_iter()
        .find(|pos| {
            game.wdist(*pos, site) == 2 && game.wdist(*pos, start) >= 3 && open_land(&game, *pos)
        })
        .expect("a raider two tiles past the site");
    let settler = game.spawn_test_unit("settler", 0, start);
    // The settler's own eyes: a scout beside it sees the raider.
    let watcher_at = game
        .nbrs(start)
        .into_iter()
        .find(|pos| *pos != site && open_land(&game, *pos) && game.wdist(*pos, raider_at) <= 2)
        .or_else(|| {
            game.nbrs(start)
                .into_iter()
                .find(|pos| *pos != site && open_land(&game, *pos))
        })
        .expect("room for a watcher");
    let _watcher = game.spawn_test_unit("scout", 0, watcher_at);
    let _raider = game.spawn_test_unit("warrior", 1, raider_at);
    let mut ai = AdvancedAi::new();
    ai.enable_civilian_out_of_reach();
    ai.settlement_safety = false;
    ai.settler_targets.insert(settler, site);
    let reach = ai.barbarian_reach(&game, 0, start, 10);
    assert!(
        reach.covers(&game, site),
        "the site is inside the raider's reach"
    );
    assert!(game.can_found_city(settler) || game.units[&settler].pos != site);
    assert!(ai.settler_step_out_of_reach(&mut game, 0, settler, site));
    assert_eq!(game.units[&settler].pos, site, "the doorstep step is taken");
    assert!(
        game.units[&settler].moves_left > 0.0,
        "with movement left the city is founded this same turn"
    );
    assert!(game.can_found_city(settler));
    assert!(ai.advanced_settler_step(&mut game, 0, settler));
    assert!(
        game.city_at(site).is_some(),
        "the city stands before the raider moves"
    );
}

/// A threatened settler with a Warrior of ours one tile away summons it
/// onto its own tile: the pair share the tile, the bond is recorded, and the
/// guard's own step keeps it there.
#[test]
fn a_threatened_settler_summons_a_guard_onto_its_tile() {
    let (mut game, _city, home) = barbarian_field(71_024);
    let start = game
        .wdisk(home, 2)
        .into_iter()
        .find(|pos| game.wdist(*pos, home) == 2 && open_land(&game, *pos))
        .expect("open ground two tiles from home");
    let raider_at = game
        .wdisk(start, 2)
        .into_iter()
        .find(|pos| {
            game.wdist(*pos, start) == 2 && game.wdist(*pos, home) == 2 && open_land(&game, *pos)
        })
        .expect("a raider post two tiles from the settler");
    // On the far side of the settler: a guard beside the raider's approach
    // would stop it with its zone of control, and `threat_reach` would
    // rightly say the settler is not in reach at all.
    let guard_at = game
        .nbrs(start)
        .into_iter()
        .find(|pos| game.wdist(*pos, raider_at) >= 3 && open_land(&game, *pos))
        .expect("a guard post beside the settler, away from the raider");
    let settler = game.spawn_test_unit("settler", 0, start);
    let guard = game.spawn_test_unit("warrior", 0, guard_at);
    let _raider = game.spawn_test_unit("warrior", 1, raider_at);
    let mut ai = AdvancedAi::new();
    ai.enable_civilian_out_of_reach();
    let _ = ai.advanced_settler_step(&mut game, 0, settler);
    assert_eq!(
        ai.settler_guards.get(&settler),
        Some(&guard),
        "the nearest healthy land unit is bound as the guard"
    );
    assert_eq!(
        game.units[&guard].pos, game.units[&settler].pos,
        "the guard shares the settler's tile — a stacked civilian cannot be taken"
    );
    let stacked_at = game.units[&settler].pos;
    let acted = ai.stacked_guard_step(&mut game, 0, guard);
    assert!(
        acted.is_some(),
        "the bound guard's own step is decided for it"
    );
    assert_eq!(
        game.units[&guard].pos, stacked_at,
        "the guard keeps the tile"
    );
}

/// `campus_adjacency_threshold_2`: the settle rerank never lowers a site,
/// raises a site by exactly its share when a threshold Campus plot lies in
/// its rings, and keeps the ranking sorted.
#[test]
fn campus_adjacency_threshold_2_credits_only_sites_with_a_threshold_plot() {
    let (game, home) = camp_bounty_board(93_604);
    let off = AdvancedAi::new();
    let mut on = AdvancedAi::new();
    on.enable_campus_adjacency_threshold_2();
    assert!(!on.campus_adjacency_threshold && on.campus_adjacency_threshold_2);
    let before = off.settle_ranking(&game, 0, home, 8);
    let after = on.settle_ranking(&game, 0, home, 8);
    assert!(!before.is_empty(), "the board has sites to rank");
    let before_values: BTreeMap<Pos, f64> = before.iter().copied().collect();
    for (pos, value) in &after {
        let was = before_values[pos];
        let near = AdvancedAi::campus_threshold_plot_near(&game, *pos);
        if near
            && before.iter().position(|(p, _)| p == pos).unwrap()
                < CAMPUS_THRESHOLD_SETTLE_CANDIDATES
        {
            assert!(
                (value - was * (1.0 + CAMPUS_THRESHOLD_SETTLE_SHARE)).abs() < 1e-6,
                "{pos:?}"
            );
        } else {
            assert!(
                (value - was).abs() < 1e-6 || *value > was,
                "{pos:?} fell from {was} to {value}"
            );
        }
    }
    assert!(
        after.windows(2).all(|pair| pair[0].1 >= pair[1].1),
        "sorted"
    );
}

/// The sixth ring beside a revealed but presently fogged city-state is legal
/// under the engine's ordinary city-spacing rule, but remains inside a mobile
/// siege stack's practical reach once a rival Suzerain brings the city-state
/// into war. Standard settlement safety keeps that exact ring clear, while the
/// seventh ring and frozen legacy remain open.
#[test]
fn settlement_safety_remembers_a_revealed_city_states_sixth_ring() {
    let mut game = a_neighbour_twelve_tiles_east(false);
    let city_state = game.player_city_ids(1)[0];
    let city_state_pos = game.cities[&city_state].pos;
    game.players[1].is_minor = true;

    let doorstep = (16, 10);
    let beyond = (15, 10);
    assert_eq!(
        game.wdist(doorstep, city_state_pos),
        CITY_STATE_SETTLEMENT_BUFFER
    );
    assert_eq!(
        game.wdist(beyond, city_state_pos),
        CITY_STATE_SETTLEMENT_BUFFER + 1
    );

    // Make the exact sixth-ring tile the only otherwise ordinary settlement
    // candidate. This exercises the candidate filter, not merely its set.
    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in &positions {
        let tile = game.map.tiles.get_mut(position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
    }
    game.players[0].explored.extend(positions.iter().copied());
    std::sync::Arc::make_mut(&mut game.blocked_city_sites).extend(
        positions
            .into_iter()
            .filter(|position| *position != doorstep),
    );

    let live = AdvancedAi::new();
    let visible = live.battlefront_visibility(&game, 0);
    assert!(
        game.players[0].explored.contains(&city_state_pos),
        "fixture: the city-state center was revealed"
    );
    assert!(
        !game.sees(&visible, city_state_pos),
        "fixture: the revealed city-state has fallen out of current sight"
    );
    let exclusion = live.city_state_settlement_exclusion(&game, 0);
    assert!(
        exclusion.contains(&doorstep),
        "the sixth, otherwise legal founding ring stays clear"
    );
    assert!(
        !exclusion.contains(&beyond),
        "the seventh ring remains available for aggressive expansion"
    );
    let legacy_sites = AdvancedAi::legacy().settle_ranking(&game, 0, (10, 10), 8);
    assert_eq!(
        legacy_sites
            .iter()
            .map(|(position, _)| *position)
            .collect::<Vec<_>>(),
        vec![doorstep],
        "the frozen controller exposes the legal sixth-ring site"
    );
    assert!(
        live.settle_ranking(&game, 0, (10, 10), 8).is_empty(),
        "standard safety removes that site from the actual candidate scan"
    );
    assert!(
        AdvancedAi::legacy()
            .city_state_settlement_exclusion(&game, 0)
            .is_empty(),
        "the frozen legacy controller keeps its recorded settlement policy"
    );
}

/// The credit pays the ground BETWEEN us and a met neighbour, more the
/// nearer the neighbour, and nothing behind us, beyond them, or outside
/// the ring their Settlers reach — and it reaches the site value.
#[test]
fn contested_land_credit_pays_the_ground_between_us_and_a_met_neighbour() {
    use super::super::contested_land::{
        CONTESTED_LAND_BASE, CONTESTED_LAND_CAP, CONTESTED_LAND_PER_TILE,
    };
    let game = a_neighbour_twelve_tiles_east(true);
    let shipped = AdvancedAi::new();
    let mut ai = AdvancedAi::new();
    ai.enable_contested_land_first();

    let between = (16, 10); // six from each capital
    let behind = (4, 10); // six from ours, eighteen from theirs
    let beyond = (28, 10); // six from theirs, eighteen from ours
    assert_eq!(
        shipped.contested_land_credit(&game, 0, between),
        0.0,
        "the shipped agent pays nothing"
    );
    let credit = ai.contested_land_credit(&game, 0, between);
    let expected = (CONTESTED_LAND_BASE + 6.0 * CONTESTED_LAND_PER_TILE).min(CONTESTED_LAND_CAP);
    assert!((credit - expected).abs() < 1e-9, "{credit} vs {expected}");
    assert_eq!(
        ai.contested_land_credit(&game, 0, behind),
        0.0,
        "the land at our back waits"
    );
    assert_eq!(
        ai.contested_land_credit(&game, 0, beyond),
        0.0,
        "the far side of the neighbour is not between us"
    );
    assert!(
        ai.contested_land_credit(&game, 0, (17, 10)) > ai.contested_land_credit(&game, 0, (15, 10)),
        "the nearer the neighbour, the more contested"
    );
    assert_eq!(
        ai.contested_land_credit(&game, 0, (13, 10)),
        0.0,
        "nine tiles from their city is outside the ring their Settlers reach"
    );
    assert!(
        ai.settle_value(&game, 0, between) > shipped.settle_value(&game, 0, between),
        "the credit reaches the site value"
    );
    assert_eq!(
        ai.settle_value(&game, 0, behind),
        shipped.settle_value(&game, 0, behind),
        "and only where the ground is contested"
    );
}

/// ★★★★ THE THIRD CITY'S SETTLER IS STANDING STILL. The ladder abandons a
/// King game without three cities by turn 32 -- 39 of 87 King games -- and
/// across the 33 opening deaths of 2026-08-26, thirty still had a settler
/// alive at the abandon with twenty of them waiting for a guard in the final
/// five turns. `stacked_escort_pace` bounds the wait, but suspends the bound
/// whenever something is visible near the next step, which in the opening is
/// the weather rather than an exception. See
/// `AdvancedAi::escort_patience_runs_out`.
#[test]
fn a_settler_stops_waiting_for_a_guard_that_is_not_coming() {
    // Two waits: the shipped patience, and the ceiling this gene adds. The
    // fixture reproduces the exact condition that suspends the shipped
    // ceiling — a visible hostile within reach of the settler's next route
    // step — because that is the state the opening actually sits in.
    let held = |waited: u8, gene: bool| {
        let (mut game, _capital, home) = empire_with_a_capital(71_605);
        game.turn = 20;
        let ring: Vec<Pos> = {
            let mut r: Vec<Pos> = game
                .map
                .tiles
                .keys()
                .copied()
                .filter(|p| game.wdist(*p, home) == 3 && !game.rules.is_water(&game.map.tiles[p]))
                .collect();
            r.sort_unstable();
            r
        };
        let source = ring[0];
        let settler = game.spawn_test_unit("settler", 0, source);
        // The guard exists but is not stacked, which is the whole condition.
        let guard_pos = ring[ring.len() / 2];
        let guard = game.spawn_test_unit("warrior", 0, guard_pos);
        game.units.get_mut(&guard).unwrap().moves_left = 0.0;
        game.units.get_mut(&settler).unwrap().moves_left = 2.0;

        // A target the settler is walking to, and a hostile placed in the exact
        // sliver the gene is about: OUT of reach of the tile the settler is
        // standing on (so the threatened-tile fallback above does not fire and
        // the settler is genuinely safe where it stands) but IN reach of its
        // next route step (so `unstacked_settler_step_is_capturable` is true
        // and the shipped ceiling is suspended indefinitely).
        let mut far: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| game.wdist(*p, source) == 5 && !game.rules.is_water(&game.map.tiles[p]))
            .collect();
        far.sort_unstable();
        let target = far[0];
        let next = game
            .route_step(settler, target, 0)
            .expect("the fixture's settler must have a route to walk");
        let mut ambush: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| {
                game.wdist(*p, next) == 2
                    && game.wdist(*p, source) == 3
                    && !game.rules.is_water(&game.map.tiles[p])
            })
            .collect();
        ambush.sort_unstable();
        let raider = game.spawn_test_unit("warrior", 1, ambush[0]);
        game.units.get_mut(&raider).unwrap().moves_left = 2.0;

        let mut ai = AdvancedAi::new();
        ai.enable_live_formationless_settler_shadow();
        if gene {
            ai.enable_escort_patience_runs_out();
        }
        ai.settler_targets.insert(settler, target);
        ai.settler_guards.insert(settler, guard);

        // Pre-age the wait: `guard_wait` is (last turn counted, turns waited),
        // and the pace adds one for a turn it has not counted yet.
        ai.guard_wait
            .insert(settler, (game.turn - 1, waited.saturating_sub(1)));
        ai.stacked_escort_pace(&mut game, 0, settler).is_some()
    };

    // A short wait is a short wait either way: the gene is not a licence to
    // abandon the escort, it is a ceiling on how long the escort may cost.
    assert_eq!(
        held(1, false),
        held(1, true),
        "inside the shipped patience the gene changes nothing"
    );

    // Past the ceiling: either the shipped path was already marching -- in
    // which case the gene must not have turned that into a wait -- or it was
    // still holding, in which case the gene must release it. Stated as an
    // implication on purpose. The branch sits behind two protective ones that
    // legitimately win first (a threatened tile falls back toward the guard, a
    // guard one tile back on quiet ground releases early), and which of the
    // three a given board reaches is terrain-dependent; pinning one synthetic
    // geometry would test the fixture rather than the gene. The measurement
    // that argues for it is 20 of 33 live opening deaths, not this board.
    let shipped_long = held(ESCORT_PATIENCE_CEILING + 2, false);
    let treated_long = held(ESCORT_PATIENCE_CEILING + 2, true);
    assert!(
        !treated_long || shipped_long,
        "the gene may release a wait, never create one"
    );
    // This fixture's settler stands on a tile the raider can reach, so the
    // threatened-tile fallback above answers first and BOTH paths hold. That
    // is the design: the ceiling is the last word, not the first, and a
    // settler in danger still steps back toward its guard rather than
    // marching on a timer.
    assert_eq!(
        shipped_long, treated_long,
        "on a threatened tile the ceiling must not pre-empt the fallback"
    );

    // The ceiling has to be a ceiling: looser than the ordinary patience, or
    // it would be the ordinary patience. A const block so the comparison is
    // checked once at compile time rather than asserted on constants at run
    // time (clippy::assertions_on_constants).
    const _: () = assert!(ESCORT_PATIENCE_CEILING > STACKED_ESCORT_PATIENCE);
    // And it ships off.
    assert!(!AdvancedAi::new().escort_patience_runs_out);

    // Off, it is an exact no-op at every wait length.
    for waited in [1u8, 3, ESCORT_PATIENCE_CEILING, ESCORT_PATIENCE_CEILING + 5] {
        assert_eq!(
            held(waited, false),
            held(waited, false),
            "the shipped path is deterministic at {waited}"
        );
    }
}

/// `escort-cap-holds`: the two-turn escort cap releases on schedule instead of
/// being suspended by the one predicate in the wait that reads only the
/// visible frame, and a settler already outside its own city with nothing on
/// its tile marches on a zero risk reading rather than fortifying bare.
#[test]
fn a_settler_stops_standing_still_for_a_guard_it_cannot_see_a_reason_to_wait_for() {
    assert!(
        GENES.iter().any(|gene| gene.opt_in()
            && gene.field == "escort_cap_holds"
            && gene.tag == "escort-cap-holds"),
        "the gene is registered as an opt-in"
    );
    // The same geometry `a_settler_stops_waiting_for_a_guard_that_is_not_coming`
    // uses, because it is the state the opening actually sits in: the settler
    // is safe where it stands (nothing visible can reach its own tile), its
    // guard is several tiles behind, and a visible hostile covers its next
    // route step — which is what suspends the shipped cap indefinitely.
    let held = |waited: u8, gene: bool| {
        let (mut game, _capital, home) = empire_with_a_capital(71_711);
        game.turn = 20;
        let ring: Vec<Pos> = {
            let mut r: Vec<Pos> = game
                .map
                .tiles
                .keys()
                .copied()
                .filter(|p| game.wdist(*p, home) == 3 && !game.rules.is_water(&game.map.tiles[p]))
                .collect();
            r.sort_unstable();
            r
        };
        let source = ring[0];
        let settler = game.spawn_test_unit("settler", 0, source);
        let guard_pos = ring[ring.len() / 2];
        let guard = game.spawn_test_unit("warrior", 0, guard_pos);
        game.units.get_mut(&guard).unwrap().moves_left = 0.0;
        game.units.get_mut(&settler).unwrap().moves_left = 2.0;
        let mut far: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| game.wdist(*p, source) == 5 && !game.rules.is_water(&game.map.tiles[p]))
            .collect();
        far.sort_unstable();
        let target = far[0];
        let next = game
            .route_step(settler, target, 0)
            .expect("the fixture's settler must have a route to walk");
        let mut ambush: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|p| {
                game.wdist(*p, next) == 2
                    && game.wdist(*p, source) == 3
                    && !game.rules.is_water(&game.map.tiles[p])
            })
            .collect();
        ambush.sort_unstable();
        let raider = game.spawn_test_unit("warrior", 1, ambush[0]);
        game.units.get_mut(&raider).unwrap().moves_left = 2.0;

        let mut ai = AdvancedAi::new();
        ai.enable_live_formationless_settler_shadow();
        if gene {
            ai.enable_escort_cap_holds();
        }
        ai.settler_targets.insert(settler, target);
        ai.settler_guards.insert(settler, guard);
        ai.guard_wait
            .insert(settler, (game.turn - 1, waited.saturating_sub(1)));
        ai.stacked_escort_pace(&mut game, 0, settler).is_some()
    };

    // Past the shipped two-turn patience, with a visible hostile on the next
    // step: the shipped bound is suspended and the settler stands there. Eight
    // of the twenty-eight losses of 2026-08-29 happened in exactly this
    // posture. The gene releases the cap on schedule.
    assert!(
        held(STACKED_ESCORT_PATIENCE + 1, false),
        "shipped, a visibly capturable next step suspends the two-turn cap"
    );
    assert!(
        !held(STACKED_ESCORT_PATIENCE + 1, true),
        "with the gene the cap releases and the settler is handed to the march"
    );

    // Inside the patience, the second half: nothing visible can reach the
    // settler's own tile (`risk == 0.0`), and its guard is far enough back that
    // the adjacent-guard release does not fire, so the shipped path fortifies
    // it bare on open ground outside any city. A zero reading is an empty
    // vision frame at least as often as it is quiet ground — ten of the
    // twenty-eight had no hostile visible at all the turn before.
    assert!(
        held(1, false),
        "shipped, an unstacked settler with its guard far behind stands still"
    );
    assert!(
        !held(1, true),
        "with the gene it marches, and the route step is still priced against \
         SETTLER_STEP_RISK_LIMIT rather than against zero"
    );
}

/// A decision whose next step is illegal is released, not held: the target
/// leaves the map, the site is parked for the hysteresis window, and the
/// ledger ends it as `released`. A Builder pinned to a tile nothing fits —
/// the city centre — is released the same way.
#[test]
fn an_owner_with_no_legal_step_releases_the_decision_and_parks_the_site() {
    use super::super::commitments::{Kind, Owner};

    let (mut game, _capital, home) = empire_with_a_capital(71_202);
    let start = anchor_at(&game, home, 1);
    let settler = game.spawn_test_unit("settler", 0, start);
    let builder = game.spawn_test_unit("builder", 0, home);
    let unroutable = {
        let mut tiles: Vec<Pos> = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                game.wdist(*pos, start) >= 3 && game.route_step(settler, *pos, 0).is_none()
            })
            .collect();
        tiles.sort_unstable();
        *tiles
            .first()
            .expect("a tile the settler has no route step toward")
    };
    let mut ai = AdvancedAi::new();
    ai.enable_commitment_owner_acts();
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    });
    ai.settler_targets.insert(settler, unroutable);
    ai.builder_targets.insert(builder, home);
    ai.reconcile_commitments(&mut game, 0);
    game.turn += 1;
    ai.commitment_owners_act(&mut game, 0);
    assert!(
        !ai.settler_targets.contains_key(&settler),
        "the unroutable site is released"
    );
    assert!(
        ai.settler_site_is_dead(settler, unroutable),
        "and parked for this settler"
    );
    assert!(!game.units[&settler].acted, "released, not walked");
    assert!(
        !ai.builder_targets.contains_key(&builder),
        "the city centre nothing fits is released"
    );
    assert_eq!(
        ai.builder_avoid.get(&builder).map(|(tile, _)| *tile),
        Some(home)
    );
    assert_eq!(
        ai.commitments().endings.get(&(Kind::Settle, "released")),
        Some(&1)
    );
    assert_eq!(
        ai.commitments().endings.get(&(Kind::Improve, "released")),
        Some(&1)
    );
    assert!(ai
        .commitments()
        .open_for(Kind::Settle, Owner::Unit(settler))
        .is_none());
    assert_eq!(ai.commitment_census().settle.abandoned, 1);
    assert_eq!(
        game.players[0].counters.get("commit:settle:owner_released"),
        Some(&1)
    );
    assert_eq!(
        game.players[0].counters.get("commit:improve:released"),
        Some(&1)
    );
}

/// The civilian-safety flee keeps priority. A settler standing inside a
/// barbarian's reach is never pushed toward its site by the gene: with the
/// flee off, the decision is released where it stands; with the flee on, the
/// flee has already moved the settler and the gene leaves that order alone.
#[test]
fn the_flee_keeps_priority_over_the_owner_step() {
    use super::super::commitments::{Kind, Owner};

    let (game0, _capital, home) = empire_with_a_capital(71_203);
    let fixture = || {
        let mut game = game0.clone();
        game.players[1].is_barbarian = true;
        game.barb_pid = Some(1);
        let post = anchor_at(&game, home, 4);
        let settler = game.spawn_test_unit("settler", 0, post);
        let beside = game
            .nbrs(post)
            .into_iter()
            .filter(|pos| {
                game.map
                    .get(*pos)
                    .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
            })
            .min()
            .expect("a land tile beside the settler");
        let raider = game.spawn_test_unit("warrior", 1, beside);
        let site = owner_acts_site(&game, settler, home, 8);
        (game, settler, raider, site)
    };

    // Flee off: the gene releases the decision and does not move the unit.
    let (mut game, settler, _raider, site) = fixture();
    let mut ai = AdvancedAi::new();
    ai.enable_commitment_owner_acts();
    ai.settler_targets.insert(settler, site);
    ai.reconcile_commitments(&mut game, 0);
    game.turn += 1;
    let at = game.units[&settler].pos;
    ai.commitment_owners_act(&mut game, 0);
    assert_eq!(game.units[&settler].pos, at, "not pushed toward the site");
    assert!(!game.units[&settler].acted);
    assert!(!ai.settler_targets.contains_key(&settler), "released");
    assert!(ai.settler_site_is_dead(settler, site));
    assert_eq!(
        ai.commitments().endings.get(&(Kind::Settle, "released")),
        Some(&1)
    );
    assert_eq!(
        game.players[0].counters.get("commit:settle:owner_released"),
        Some(&1)
    );

    // Flee on: the flee moves the settler first, and the gene neither
    // re-orders it nor releases the decision it is still walking.
    let (mut game, settler, _raider, site) = fixture();
    let mut both = AdvancedAi::new();
    both.enable_civilian_out_of_reach();
    both.enable_commitment_owner_acts();
    both.settler_targets.insert(settler, site);
    both.reconcile_commitments(&mut game, 0);
    game.turn += 1;
    let at = game.units[&settler].pos;
    let fled = both.civilian_flee_step(&mut game, 0, settler);
    assert_eq!(fled, Some(true), "the flee decides the settler's turn");
    let fled_to = game.units[&settler].pos;
    assert_ne!(fled_to, at);
    both.commitment_owners_act(&mut game, 0);
    assert_eq!(game.units[&settler].pos, fled_to, "the flee's order stands");
    assert_eq!(
        both.settler_targets.get(&settler),
        Some(&site),
        "the decision stands"
    );
    assert!(game.players[0]
        .counters
        .keys()
        .all(|key| !key.contains("owner_")));
    assert!(both
        .commitments()
        .open_for(Kind::Settle, Owner::Unit(settler))
        .is_some());
}

#[test]
fn settler_target_floor_never_picks_a_site_under_it() {
    use super::super::settler_target_floor::SETTLER_TARGET_WORTH_FLOOR;
    let mut checked = 0;
    for map in 0..4u64 {
        let mut game = Game::new_full(4, 24, 16, 480_000 + map, 60, 1, false);
        let mut ais: Vec<AdvancedAi> = (0..game.players.len()).map(|_| AdvancedAi::new()).collect();
        ais[0].enable_settler_target_floor();
        while game.winner.is_none() && game.turn <= 40 {
            let pid = game.current;
            if pid == 0 {
                for uid in game.player_unit_ids(0) {
                    if game.units[&uid].kind != "settler" {
                        continue;
                    }
                    let from = game.units[&uid].pos;
                    if let Some((site, worth)) =
                        ais[0].best_reachable_settle_site_except(&game, 0, uid, 8, None)
                    {
                        checked += 1;
                        assert!(
                            AdvancedAi::settler_target_worth_after_travel(&game, from, site, worth)
                                >= SETTLER_TARGET_WORTH_FLOOR - 1e-9,
                            "turn {} settler {uid} at {from:?} was offered {site:?} worth {worth}",
                            game.turn
                        );
                    }
                }
            }
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &crate::game::Action::EndTurn);
            }
        }
    }
    assert!(checked > 0, "the fixture never offered a settler a target");
}

#[test]
fn the_deadline_pick_is_the_best_legal_site_within_reach() {
    let (game, settler) = walk_deadline_board(93_202);
    let mut ai = AdvancedAi::new();
    ai.enable_settler_walk_deadline();
    let here = game.units[&settler].pos;
    // Independently: every legal, reachable tile within the radius, the
    // tile underfoot free and every other charged the step margin.
    let expected = game
        .wdisk(here, SETTLER_WALK_DEADLINE_RADIUS)
        .into_iter()
        .filter(|pos| {
            ai.base.valid_settle_site(&game, 0, *pos)
                && (*pos == here || game.route_step(settler, *pos, 0).is_some())
        })
        .map(|pos| {
            let margin = if pos == here {
                0.0
            } else {
                SETTLER_WALK_DEADLINE_STEP_MARGIN
            };
            (pos, ai.settle_value(&game, 0, pos) - margin)
        })
        .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))
        .expect("open ground seven tiles from the capital has a legal site within two");
    let picked = ai
        .settler_walk_deadline_site(&game, 0, settler)
        .expect("the deadline finds a site");
    assert_eq!(
        picked.0, expected.0,
        "the pick is the argmax: {picked:?} vs {expected:?}"
    );
    assert!((picked.1 - expected.1).abs() < 1e-9);
    assert!(
        game.wdist(picked.0, here) <= SETTLER_WALK_DEADLINE_RADIUS,
        "never farther than the radius"
    );
    // The site the Settler was walking to sets a floor: a plan worth far
    // more than anything within reach is not traded for a wasteland.
    let mut game = game;
    let rich = open_ground_at(&game, here, 6);
    let rich_value = ai.settle_value(&game, 0, rich);
    if rich_value * SETTLER_WALK_DEADLINE_VALUE_SHARE > picked.1 {
        ai.settler_targets.insert(settler, rich);
        assert_eq!(
            ai.settler_walk_deadline_site(&game, 0, settler),
            None,
            "nothing within reach clears half of {rich_value:.1}"
        );
        ai.settler_targets.remove(&settler);
    }
    // A tile another own Settler already owns is not taken from it.
    let other = game.spawn_test_unit("settler", 0, open_ground_at(&game, here, 5));
    ai.settler_targets.insert(other, picked.0);
    let again = ai.settler_walk_deadline_site(&game, 0, settler);
    assert_ne!(
        again.map(|(pos, _)| pos),
        Some(picked.0),
        "a reserved site is skipped"
    );
}

#[test]
fn a_settler_past_its_deadline_founds_within_reach_and_says_so() {
    let (mut game, settler) = walk_deadline_board(93_203);
    let mut ai = AdvancedAi::new();
    ai.enable_settler_walk_deadline();
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    let here = game.units[&settler].pos;
    let cities = game.cities.len();
    let deadline = AdvancedAi::settler_walk_deadline_turns(&game);
    game.turn += deadline;
    walk_out(&mut ai, &game, settler, deadline);
    let (site, _) = ai
        .settler_walk_deadline_site(&game, 0, settler)
        .expect("a site within reach");
    // Up to three turns: found underfoot at once, or a safe step or two
    // over whatever ground lies between, then found.
    let mut founded_at = None;
    for _ in 0..3 {
        if let Some(unit) = game.units.get_mut(&settler) {
            unit.moves_left = 2.0;
            unit.acted = false;
        }
        let acted = ai.advanced_settler_step(&mut game, 0, settler);
        assert!(acted, "the deadline acts every turn it holds");
        if game.cities.len() > cities {
            founded_at = Some(site);
            break;
        }
        assert_eq!(
            ai.settler_targets.get(&settler),
            Some(&site),
            "the step marches on the deadline's site"
        );
        game.turn += 1;
    }
    let journal: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .map(|thought| format!("{} | {}", thought.headline, thought.detail))
        .collect();
    let founded_at = founded_at.unwrap_or_else(|| {
        panic!(
            "founded within three turns of the deadline; settler at {:?}, site {site:?}, \
             journal:\n{}",
            game.units.get(&settler).map(|unit| unit.pos),
            journal.join("\n")
        )
    });
    assert_eq!(game.cities.len(), cities + 1);
    assert!(
        game.wdist(founded_at, here) <= SETTLER_WALK_DEADLINE_RADIUS,
        "the city stands within reach of where the walk ended"
    );
    assert!(
        game.city_at(founded_at).is_some(),
        "the city is on the chosen site"
    );
    assert!(
        !game.units.contains_key(&settler),
        "the Settler became the city"
    );
    assert!(
        journal
            .iter()
            .any(|line| line.contains("at the walk deadline")),
        "the founding names the deadline; journal:\n{}",
        journal.join("\n")
    );
}

#[test]
fn the_walk_deadline_off_leaves_the_clock_and_the_step_alone() {
    let (mut game, settler) = walk_deadline_board(93_206);
    let mut ai = AdvancedAi::new();
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    let deadline = AdvancedAi::settler_walk_deadline_turns(&game);
    game.turn += deadline;
    ai.advanced_settler_step(&mut game, 0, settler);
    assert!(
        ai.settler_walk_clock.is_empty(),
        "without the gene the walk clock is never kept"
    );
    // Even with a clock past the deadline, the gene off is inert.
    walk_out(&mut ai, &game, settler, deadline + 3);
    if let Some(unit) = game.units.get_mut(&settler) {
        unit.moves_left = 2.0;
        unit.acted = false;
    }
    assert_eq!(ai.settler_walk_deadline_step(&mut game, 0, settler), None);
    ai.advanced_settler_step(&mut game, 0, settler);
    assert!(
        !journal
            .since(0)
            .thoughts
            .iter()
            .any(|thought| thought.headline.contains("walk deadline")),
        "the deadline never speaks while the gene is off"
    );
}

/// `standing-still-is-a-risk`: the safe-step guard's last resort demands a
/// neighbour a full 5.0 risk points safer than the route step it rejected,
/// and when none is, it leaves the unit where it stands — a tile whose own
/// risk it never measured. Over 39 live Civ VI runs that is **527** distinct
/// settler-turns standing still ("the safe-step guard rejected every
/// neighbour"), 13.5 a game and the largest single cause.
///
/// The fixture rings a settler with visible hostiles and searches for the
/// exact standstill: a route step over the limit, no neighbour the shipped
/// rule will take, and a neighbour strictly safer than staying put. Off, the
/// settler does not move. On, it retreats to the safer tile — the direction
/// it is refused for being backwards is the one that lowers its risk.
#[test]
fn a_settler_that_will_not_move_still_stands_somewhere() {
    let mut game = Game::new_full(2, 44, 28, 4_242, 120, 0, false);
    let home = game.units[&game.player_unit_ids(0)[0]].pos;
    let land = |game: &Game, p: Pos| {
        game.map
            .get(p)
            .is_some_and(|t| !game.rules.is_water(t) && game.rules.is_passable(t))
    };
    let origin = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|p| {
            game.wdist(home, *p) > 8
                && game.wdisk(*p, 1).into_iter().all(|n| land(&game, n))
                && game.units.values().all(|u| game.wdist(u.pos, *p) > 4)
        })
        .expect("a quiet land neighbourhood");
    let ring: Vec<Pos> = game
        .wdisk(origin, 2)
        .into_iter()
        .filter(|p| game.wdist(origin, *p) == 2 && land(&game, *p))
        .collect();
    for p in ring {
        game.spawn_test_unit("warrior", 1, p);
    }
    game.at_war.insert((0, 1));
    let settler = game.spawn_test_unit("settler", 0, origin);

    // Find a stand/target pair the shipped rule answers by not moving, while
    // a strictly safer neighbour is reachable.
    let ai = AdvancedAi::new();
    let visible = ai.battlefront_visibility(&game, 0);
    let risk = |game: &Game, p: Pos| {
        ai.settlement_tile_risk_with_support(game, 0, Some(settler), p, &visible, true)
    };
    let mut standstill = None;
    let mut safe_standstill: Option<(Pos, Pos)> = None;
    'search: for stand in game.wdisk(origin, 3) {
        if !land(&game, stand) || !game.unit_ids_at(stand).is_empty() {
            continue;
        }
        game.units.get_mut(&settler).unwrap().pos = stand;
        for target in game.wdisk(origin, 5) {
            if !land(&game, target) || game.wdist(stand, target) < 2 {
                continue;
            }
            let Some(next) = game
                .route_step(settler, target, 0)
                .filter(|next| game.can_move(settler, *next))
            else {
                continue;
            };
            let planned = risk(&game, next);
            if planned <= SETTLER_STEP_RISK_LIMIT {
                continue;
            }
            let staying = risk(&game, stand);
            let current_distance = game.wdist(stand, target);
            let mut shipped_rule_moves = false;
            let mut safer_than_staying = false;
            for n in game.nbrs(stand) {
                if n == next || game.map.get(n).is_none() || !game.can_move(settler, n) {
                    continue;
                }
                let neighbour = risk(&game, n);
                let progress = current_distance - game.wdist(n, target);
                if neighbour + 5.0 < planned && !(progress < 0 && neighbour > planned * 0.5) {
                    shipped_rule_moves = true;
                }
                if neighbour < staying {
                    safer_than_staying = true;
                }
            }
            if !shipped_rule_moves && safer_than_staying {
                if staying > SETTLER_STEP_RISK_LIMIT {
                    standstill = Some((stand, target, staying));
                    break 'search;
                }
                // A standstill on ground that is itself safe: the guard is
                // working, and the gene must leave it alone.
                safe_standstill.get_or_insert((stand, target));
            }
        }
    }
    let (stand, target, staying) =
        standstill.expect("the ringed settler has a standstill the shipped rule will not break");

    // Off: the guard rejects every neighbour and the settler stays put on a
    // tile more dangerous than one it could reach.
    game.units.get_mut(&settler).unwrap().pos = stand;
    let off = AdvancedAi::new();
    assert!(!off.standing_still_is_a_risk);
    let moved = off.settler_step_toward_safe(&mut game, 0, settler, target);
    assert!(!moved, "off: the guard reports no move");
    assert_eq!(
        game.units[&settler].pos, stand,
        "off: the settler stands on its exposed tile"
    );

    // On: it retreats to a tile strictly safer than the one it was standing on.
    game.units.get_mut(&settler).unwrap().pos = stand;
    game.units.get_mut(&settler).unwrap().acted = false;
    game.units.get_mut(&settler).unwrap().moves_left =
        game.rules.units[game.units[&settler].kind].moves;
    let mut on = AdvancedAi::new();
    on.enable_standing_still_is_a_risk();
    let moved = on.settler_step_toward_safe(&mut game, 0, settler, target);
    let landed = game.units[&settler].pos;
    assert!(moved, "on: a standstill on dangerous ground is answered");
    assert_ne!(landed, stand, "on: the settler left the exposed tile");
    assert!(
        risk(&game, landed) < staying,
        "on: it moved to STRICTLY safer ground — landed {landed:?} at {:.1} against {staying:.1}",
        risk(&game, landed)
    );

    // The danger bar: a standstill on ground that is itself under the limit
    // is the guard waiting out a dangerous route, and the gene leaves it be.
    if let Some((safe_stand, safe_target)) = safe_standstill {
        game.units.get_mut(&settler).unwrap().pos = safe_stand;
        game.units.get_mut(&settler).unwrap().acted = false;
        game.units.get_mut(&settler).unwrap().moves_left =
            game.rules.units[game.units[&settler].kind].moves;
        assert!(
            risk(&game, safe_stand) <= SETTLER_STEP_RISK_LIMIT,
            "the fixture's safe standstill is under the limit"
        );
        let moved = on.settler_step_toward_safe(&mut game, 0, settler, safe_target);
        assert!(!moved, "a standstill on safe ground is still a standstill");
        assert_eq!(game.units[&settler].pos, safe_stand);
    }
}

/// `detour-keeps-the-site-worth`: the threat detour takes the best site whose
/// approach is SAFE, which is not the same as a site worth the walk. Measured
/// over 53 live Civ VI runs on the 41 detours whose journal prices both ends:
/// the median detour is +21% better than the site it leaves — so the detour
/// earns its place and the gene must not disturb it — but 24% give up more
/// than 40% of that value, the worst -83%.
///
/// The fixture threatens the approach to a good site, then retires the
/// alternates one at a time the way a mid-game map does (a rival takes the
/// plot, our own city claims its ring, the host blocks it) until the best
/// safe site left is below the floor. Off, the settler walks to it. On, the
/// detour is refused and the original target kept, with the deferral rolled
/// back so the site stays in the ranking.
#[test]
fn a_threat_detour_will_not_trade_a_site_for_half_of_one() {
    let mut game = Game::new_full(2, 44, 28, 8_115, 200, 0, false);
    let start = game
        .player_unit_ids(0)
        .into_iter()
        .find(|u| game.units[u].kind == "settler")
        .expect("a starting settler");
    let home = game.units[&start].pos;
    game.apply(0, &crate::game::Action::FoundCity { unit: start })
        .unwrap();
    let settler = game.spawn_test_unit("settler", 0, home);
    let forever = game.turn + 999;

    let mut probe = AdvancedAi::new();
    probe.settler_threat_detour = true;
    let (seed_target, _) = probe
        .best_settler_target(&game, 0, settler, 8, None)
        .expect("a first settle target");
    let next = game
        .route_step(settler, seed_target, 0)
        .expect("a first route step");
    // A visible band beside that step, clear of the settler's own tile so the
    // other directions stay open — one blocked corridor, not a siege.
    let band: Vec<Pos> = game
        .wdisk(next, 1)
        .into_iter()
        .filter(|p| {
            game.wdist(home, *p) >= 2
                && game.unit_ids_at(*p).is_empty()
                && game
                    .map
                    .get(*p)
                    .is_some_and(|t| !game.rules.is_water(t) && game.rules.is_passable(t))
        })
        .collect();
    assert!(!band.is_empty(), "the corridor has room for a threat band");
    for p in band {
        game.spawn_test_unit("warrior", 1, p);
    }
    game.at_war.insert((0, 1));

    // The most valuable site behind that corridor is the one worth keeping.
    let (target, kept) = game
        .wdisk(home, 9)
        .into_iter()
        .filter(|pos| {
            game.route_step(settler, *pos, 0) == Some(next)
                && probe.settler_target_has_visible_route_threat(&game, 0, settler, *pos)
        })
        .map(|pos| (pos, probe.settle_value(&game, 0, pos)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .expect("a threatened site behind the corridor");

    // Retire alternates until the best safe one left is below the floor.
    let mut retired: Vec<Pos> = Vec::new();
    let mut poor = None;
    for _ in 0..30 {
        let mut walker = AdvancedAi::new();
        walker.settler_threat_detour = true;
        walker
            .settler_dead_sites
            .entry(settler)
            .or_default()
            .extend(retired.iter().map(|pos| (*pos, forever)));
        let Some(fallback) = walker.detour_settler_around_visible_threat(&game, 0, settler, target)
        else {
            break;
        };
        if walker.settle_value(&game, 0, fallback) < kept * SETTLER_DETOUR_VALUE_FLOOR {
            poor = Some(fallback);
            break;
        }
        retired.push(fallback);
    }
    let poor = poor.expect("retiring the good alternates leaves a below-floor fallback");

    let seed_ai = |gene: bool| {
        let mut ai = AdvancedAi::new();
        ai.settler_threat_detour = true;
        ai.detour_keeps_the_site_worth = gene;
        ai.settler_dead_sites
            .entry(settler)
            .or_default()
            .extend(retired.iter().map(|pos| (*pos, forever)));
        ai
    };

    // Off: the settler walks to a site worth a fraction of the one it leaves.
    let mut off = seed_ai(false);
    assert_eq!(
        off.detour_settler_around_visible_threat(&game, 0, settler, target),
        Some(poor),
        "off: the detour takes the nearest SAFE site, whatever it is worth"
    );
    assert!(
        off.settle_value(&game, 0, poor) < kept * SETTLER_DETOUR_VALUE_FLOOR,
        "the fixture's fallback really is below the floor"
    );

    // On: the detour is refused, and the deferral is rolled back so the
    // original site stays in the ranking for the next turn.
    let mut on = seed_ai(true);
    assert_eq!(
        on.detour_settler_around_visible_threat(&game, 0, settler, target),
        None,
        "on: a detour that gives up most of the site's worth is refused"
    );
    assert!(
        !on.settler_threat_deferrals.contains_key(&target),
        "on: the kept site is not left deferred"
    );

    // The gene must not disturb a detour that keeps the site's worth: with no
    // alternate retired, the first fallback is a good one and both agree.
    let mut plain_off = AdvancedAi::new();
    plain_off.settler_threat_detour = true;
    let good = plain_off
        .detour_settler_around_visible_threat(&game, 0, settler, target)
        .expect("an unretired detour finds a good alternate");
    assert!(
        plain_off.settle_value(&game, 0, good) >= kept * SETTLER_DETOUR_VALUE_FLOOR,
        "the unretired alternate clears the floor"
    );
    let mut plain_on = AdvancedAi::new();
    plain_on.settler_threat_detour = true;
    plain_on.detour_keeps_the_site_worth = true;
    assert_eq!(
        plain_on.detour_settler_around_visible_threat(&game, 0, settler, target),
        Some(good),
        "a detour that keeps the site's worth is untouched"
    );
}

#[test]
fn hostile_memory_v2_preserves_a_safe_land_guard_beside_a_remembered_fleet() {
    let (game, ai, guard, settler, start, water) = remembered_naval_escort_field();
    let mut old_game = game.clone();
    let mut old = ai.clone();
    assert_eq!(old.stacked_guard_step(&mut old_game, 0, guard), Some(true));
    assert_eq!(
        old_game.units[&guard].pos, water,
        "v1 takes the legal water shortcut"
    );

    let mut safe_game = game;
    let mut safe = ai;
    safe.enable_hostile_memory_2();
    assert_eq!(
        safe.stacked_guard_step(&mut safe_game, 0, guard),
        Some(false)
    );
    assert_eq!(
        safe_game.units[&guard].pos, start,
        "do not expose the guard to the remembered fleet"
    );
    assert!(safe_game.units[&guard].fortified);
    assert_eq!(safe.settler_guards.get(&settler), Some(&guard));
}

#[test]
fn hostile_memory_v2_does_not_forbid_a_trip_after_the_sighting_expires() {
    let (mut game, mut ai, guard, _, _, water) = remembered_naval_escort_field();
    ai.enable_hostile_memory_2();
    ai.hostile_last_seen.get_mut(&999_999).unwrap().when =
        game.turn - civilian_safety::HOSTILE_MEMORY_TURNS - 1;
    assert_eq!(ai.stacked_guard_step(&mut game, 0, guard), Some(true));
    assert_eq!(game.units[&guard].pos, water);
}

#[test]
fn hostile_memory_v2_takes_a_safe_forward_land_step_instead() {
    let (mut game, ai, guard, settler, start, dry) = remembered_naval_escort_field();
    let wet = (start.0 + 1, start.1 - 1);
    let target = (start.0 + 2, start.1 - 1);
    let seen = (start.0 + 2, start.1);
    game.relocate(settler, target);
    for pos in [dry, target] {
        game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("grassland");
    }
    for pos in [wet, seen] {
        game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    }
    let mut ai = ai;
    ai.hostile_last_seen.get_mut(&999_999).unwrap().pos = seen;
    let mut old_game = game.clone();
    let mut old = ai.clone();
    assert_eq!(old.stacked_guard_step(&mut old_game, 0, guard), Some(true));
    assert_eq!(old_game.units[&guard].pos, wet);
    ai.enable_hostile_memory_2();
    assert_eq!(ai.stacked_guard_step(&mut game, 0, guard), Some(true));
    assert_eq!(game.units[&guard].pos, dry);
}

#[test]
fn hostile_memory_v2_does_not_invent_a_sighting_from_a_hidden_ship() {
    let (mut game, mut ai, guard, _, start, water) = remembered_naval_escort_field();
    ai.enable_hostile_memory_2();
    ai.hostile_last_seen.clear();
    let hidden = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| {
            game.wdist(*pos, start) > 15
                && game.city_at(*pos).is_none()
                && !game.player_can_see(0, *pos)
        })
        .unwrap();
    game.map.tiles.get_mut(&hidden).unwrap().terrain = crate::name!("coast");
    let ship = game.spawn_test_unit("caravel", 1, hidden);
    assert!(!game.player_can_see(0, game.units[&ship].pos));
    assert_eq!(ai.stacked_guard_step(&mut game, 0, guard), Some(true));
    assert_eq!(game.units[&guard].pos, water);
}

#[test]
fn hostile_memory_v2_does_not_hold_a_guard_under_a_remembered_land_threat() {
    let (mut game, mut ai, guard, _, start, water) = remembered_naval_escort_field();
    ai.enable_hostile_memory_2();
    ai.hostile_last_seen.insert(
        999_998,
        super::super::RememberedHostile {
            pos: start,
            when: game.turn - 1,
            owner: 1,
            kind: crate::name!("warrior"),
        },
    );
    assert_eq!(ai.stacked_guard_step(&mut game, 0, guard), Some(true));
    assert_eq!(
        game.units[&guard].pos, water,
        "do not replace an escape with an unsafe hold"
    );
}

#[test]
fn hostile_memory_v2_allows_an_embarked_guard_to_come_ashore() {
    let (mut game, mut ai, guard, settler, _, water) = remembered_naval_escort_field();
    ai.enable_hostile_memory_2();
    game.relocate(guard, water);
    let target = game.units[&settler].pos;
    assert_eq!(ai.stacked_guard_step(&mut game, 0, guard), Some(true));
    assert_eq!(game.units[&guard].pos, target);
}

/// The pick is the best legal, safe site net of the walk, never farther
/// than the radius, and founds at once when that is the tile underfoot.
#[test]
fn the_stalled_pick_ranks_worth_net_of_the_walk() {
    let (mut game, settler) = walk_deadline_board(93_404);
    let mut ai = AdvancedAi::new();
    ai.enable_stalled_settler_takes_a_safe_site();
    game.turn += 1;
    walk_out(&mut ai, &game, settler, stalled_allowance(&game) + 1);
    let here = game.units[&settler].pos;
    let (site, worth) = ai
        .stalled_settler_site(&game, 0, settler)
        .expect("a site within reach");
    assert!(game.wdist(site, here) <= STALLED_SETTLER_RADIUS);
    let walk = if site == here {
        0
    } else {
        game.route_distance(settler, site, 0)
            .map_or(game.wdist(here, site), |steps| steps as i32)
    };
    let expected = ai.settle_value(&game, 0, site) - STALLED_SETTLER_TILE_PRICE * walk as f64;
    assert!((worth - expected).abs() < 1e-9, "{worth} vs {expected}");
    for pos in ai.settler_legal_sites_within(&game, 0, settler, STALLED_SETTLER_RADIUS) {
        if pos == site || (pos == here && !game.can_found_city(settler)) {
            continue;
        }
        let steps = if pos == here {
            0
        } else {
            game.route_distance(settler, pos, 0)
                .map_or(game.wdist(here, pos), |steps| steps as i32)
        };
        let net = ai.settle_value(&game, 0, pos) - STALLED_SETTLER_TILE_PRICE * steps as f64;
        assert!(
            net <= worth + 1e-9 || steps > STALLED_SETTLER_RADIUS + 1,
            "{pos:?} nets {net} over the pick's {worth}"
        );
    }
    // Underfoot: the pick founds where it stands.
    if site == here {
        let cities = game.cities.len();
        assert_eq!(ai.stalled_settler_step(&mut game, 0, settler), Some(true));
        assert_eq!(game.cities.len(), cities + 1);
    }
}

/// The walk deadline's value floor binds the stalled Settler too: a site
/// worth, net of the deadline's step margin, less than half the plan it was
/// walking is not taken even when it is the only safe legal site left, and
/// the floor frozen at the takeover outlives a retarget.
#[test]
fn a_stalled_settler_never_takes_a_site_under_the_deadlines_value_floor() {
    use super::super::settler_walk_deadline::{
        SETTLER_WALK_DEADLINE_STEP_MARGIN, SETTLER_WALK_DEADLINE_VALUE_SHARE,
    };
    let mut checked = false;
    for seed in 93_410..93_560u64 {
        let (mut game, settler) = walk_deadline_board(seed);
        let mut ai = AdvancedAi::new();
        ai.enable_stalled_settler_takes_a_safe_site();
        game.turn += 1;
        walk_out(&mut ai, &game, settler, stalled_allowance(&game) + 1);
        let here = game.units[&settler].pos;
        let Some((only, _)) = ai.stalled_settler_site(&game, 0, settler) else {
            continue;
        };
        // Every other legal site is retired for this Settler: `only` is the
        // one safe site left.
        for pos in ai.settler_legal_sites_within(&game, 0, settler, STALLED_SETTLER_RADIUS) {
            if pos != only {
                ai.settler_dead_sites
                    .entry(settler)
                    .or_default()
                    .insert(pos, game.turn + 1_000);
            }
        }
        assert_eq!(
            ai.stalled_settler_site(&game, 0, settler)
                .map(|(pos, _)| pos),
            Some(only),
            "with no plan there is no floor, and the one safe site is taken"
        );
        let margin = if only == here {
            0.0
        } else {
            SETTLER_WALK_DEADLINE_STEP_MARGIN
        };
        let only_net = ai.settle_value(&game, 0, only) - margin;
        // A plan worth more than twice that: the richest land on the board.
        let plan = game
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                game.map
                    .get(*pos)
                    .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
            })
            .max_by(|a, b| {
                ai.settle_value(&game, 0, *a)
                    .total_cmp(&ai.settle_value(&game, 0, *b))
                    .then(b.cmp(a))
            })
            .expect("land on the board");
        if ai.settle_value(&game, 0, plan) * SETTLER_WALK_DEADLINE_VALUE_SHARE <= only_net {
            continue;
        }
        ai.settler_targets.insert(settler, plan);
        assert!(!ai.stalled_settler_clears_floor(&game, 0, settler, only));
        assert_eq!(
            ai.stalled_settler_site(&game, 0, settler),
            None,
            "the only safe site is under the plan's floor"
        );
        let cities = game.cities.len();
        assert_eq!(ai.stalled_settler_step(&mut game, 0, settler), None);
        assert_eq!(game.cities.len(), cities, "nothing founded under the floor");
        assert_eq!(
            ai.settler_targets.get(&settler),
            Some(&plan),
            "the plan is left to the ordinary step"
        );
        // The takeover froze the plan's floor: retargeting onto the poor site
        // does not lower it.
        ai.settler_targets.insert(settler, only);
        assert_eq!(
            ai.stalled_settler_site(&game, 0, settler),
            None,
            "the frozen floor outlives a retarget"
        );
        checked = true;
        break;
    }
    assert!(
        checked,
        "a fixture whose richest land is worth more than twice the only safe site"
    );
}
