use super::super::*;

/// One city of ours, Military Engineering and Metal Casting known (the
/// Bombard needs Niter), no Niter drawn, and an unowned plains tile in the
/// city's ring `ring` whose Niter `beside` says whether the tile touches the
/// city's own. Returns the game, the city and the deposit.
fn niter_case(ring: i32, beside: bool) -> (Game, u32, crate::Pos) {
    let mut g = Game::new_full(2, 40, 26, 81_407, 500, 0, false);
    g.current = 0;
    let settler = g
        .units
        .values()
        .find(|u| u.kind == "settler" && u.owner == 0)
        .map(|u| u.pos)
        .expect("our settler");
    let city = g.found_city_for(0, settler, None);
    g.players[0]
        .techs
        .extend(["military_engineering", "metal_casting"].map(crate::name::Name::new));
    let centre = g.cities[&city].pos;
    // No deposit of the map's own nearby: the fixture's is the only one.
    for pos in g.wdisk(centre, 6) {
        if let Some(tile) = g.map.tiles.get_mut(&pos) {
            if matches!(
                tile.resource.as_ref().map(|r| r.as_str()),
                Some("niter" | "oil")
            ) {
                tile.resource = None;
            }
        }
    }
    let ours = |g: &Game, pos: crate::Pos| {
        g.nbrs(pos).into_iter().any(|n| {
            g.map
                .tiles
                .get(&n)
                .is_some_and(|t| t.owner_city == Some(city))
        })
    };
    let land = |g: &Game, pos: crate::Pos| {
        g.map.get(pos).is_some_and(|tile| {
            tile.owner_city.is_none() && g.rules.is_passable(tile) && !g.rules.is_water(tile)
        })
    };
    let deposit = g
        .wdisk(centre, ring)
        .into_iter()
        .find(|pos| {
            g.wdist(*pos, centre) == ring
                && land(&g, *pos)
                && ours(&g, *pos) == beside
                && (beside
                    || g.nbrs(*pos)
                        .into_iter()
                        .any(|n| land(&g, n) && ours(&g, n) && g.wdist(n, centre) <= 3))
        })
        .expect("an unowned deposit tile for the fixture");
    {
        let tile = g.map.tiles.get_mut(&deposit).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.resource = Some(crate::name!("niter"));
    }
    for pos in g.wdisk(centre, 4) {
        g.players[0].explored.insert(pos);
    }
    g.players[0].gold = 500.0;
    assert_eq!(g.strategic_resource_rate(0, "niter"), 0.0);
    (g, city, deposit)
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_gun_resource_buys_the_tile();
    ai
}

#[test]
fn the_niter_deposit_is_bought_only_under_the_gene() {
    let (mut g, city, deposit) = niter_case(2, true);
    assert!(g.plot_purchase_cost(0, city, deposit).is_some());
    let off = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(
        !off.gun_resource_tile_purchase(&mut g, 0, &plan(), 100.0),
        "off by default"
    );
    assert_eq!(g.players[0].gold, 500.0);
    assert!(g.map.tiles[&deposit].owner_city.is_none());

    let ai = armed();
    assert_eq!(
        ai.gun_tile_resources_wanted(&g, 0),
        BTreeSet::from([crate::name!("niter")])
    );
    // The purchase pass's war reserve (450 of 500) does not hold it: the
    // tile keeps only the upkeep floor.
    // A second buyable deposit beside the city.
    let second = g
        .wdisk(g.cities[&city].pos, 2)
        .into_iter()
        .find(|pos| {
            *pos != deposit
                && g.plot_purchase_cost(0, city, *pos).is_some()
                && g.map
                    .get(*pos)
                    .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
        })
        .expect("a second buyable tile");
    g.map.tiles.get_mut(&second).unwrap().resource = Some(crate::name!("niter"));
    // The purchase pass's war reserve (450 of 500) does not hold it: the
    // tile keeps only the upkeep floor.
    assert!(ai.gun_resource_tile_purchase(&mut g, 0, &plan(), 450.0));
    let bought = [deposit, second]
        .into_iter()
        .filter(|pos| g.map.tiles[pos].owner_city == Some(city))
        .count();
    assert_eq!(bought, 1, "one deposit bought");
    assert!(g.players[0].gold < 500.0);
    // One deposit inside our borders waits on its Builder: no second tile.
    assert!(ai.gun_tile_resources_wanted(&g, 0).is_empty());
    assert!(!ai.gun_resource_tile_purchase(&mut g, 0, &plan(), 0.0));
    let bought = [deposit, second]
        .into_iter()
        .filter(|pos| g.map.tiles[pos].owner_city == Some(city))
        .count();
    assert_eq!(bought, 1, "still one");
}

#[test]
fn the_treasury_holds_a_tile_it_cannot_pay() {
    let (mut g, city, deposit) = niter_case(2, true);
    let cost = g.plot_purchase_cost(0, city, deposit).unwrap();
    g.players[0].gold = cost - 1.0;
    let ai = armed();
    assert!(!ai.gun_resource_tile_purchase(&mut g, 0, &plan(), 0.0));
    assert!(g.map.tiles[&deposit].owner_city.is_none());
    assert_eq!(g.players[0].gold, cost - 1.0);
}

#[test]
fn only_a_domination_seat_without_the_gun_resource_buys() {
    // A Score seat does not.
    let (mut g, _, deposit) = niter_case(2, true);
    let mut score = AdvancedAi::targeting(VictoryTarget::Score);
    score.enable_gun_resource_buys_the_tile();
    assert!(!score.gun_resource_tile_purchase(&mut g, 0, &plan(), 0.0));
    assert!(g.map.tiles[&deposit].owner_city.is_none());

    // Before Metal Casting there is no Bombard to want it.
    let (mut g, _, deposit) = niter_case(2, true);
    g.players[0].techs.remove(&crate::name!("metal_casting"));
    assert!(armed().gun_tile_resources_wanted(&g, 0).is_empty());
    assert!(!armed().gun_resource_tile_purchase(&mut g, 0, &plan(), 0.0));
    assert!(g.map.tiles[&deposit].owner_city.is_none());

    // A stockpile of one Bombard's 20 Niter: nothing wanted.
    let (mut g, _, deposit) = niter_case(2, true);
    g.players[0]
        .strategic_resources
        .insert(crate::name!("niter"), 20.0);
    assert!(armed().gun_tile_resources_wanted(&g, 0).is_empty());
    assert!(!armed().gun_resource_tile_purchase(&mut g, 0, &plan(), 0.0));
    assert!(g.map.tiles[&deposit].owner_city.is_none());
}

#[test]
fn a_third_ring_deposit_is_reached_through_the_tile_beside_it() {
    let (mut g, city, deposit) = niter_case(3, false);
    assert!(
        g.plot_purchase_cost(0, city, deposit).is_none(),
        "fixture: the deposit is out of the menu"
    );
    let ai = armed();
    assert!(ai.gun_resource_tile_purchase(&mut g, 0, &plan(), 0.0));
    assert!(
        g.map.tiles[&deposit].owner_city.is_none(),
        "the step, not the deposit"
    );
    assert!(
        g.nbrs(deposit).into_iter().any(|n| g
            .map
            .tiles
            .get(&n)
            .is_some_and(|t| t.owner_city == Some(city))),
        "the bought step touches the deposit"
    );
    assert!(g.plot_purchase_cost(0, city, deposit).is_some());
    assert!(ai.gun_resource_tile_purchase(&mut g, 0, &plan(), 0.0));
    assert_eq!(g.map.tiles[&deposit].owner_city, Some(city));
}

#[test]
fn artillery_wants_its_oil() {
    let (mut g, _, _) = niter_case(2, true);
    g.players[0]
        .techs
        .extend(["steel", "refining"].map(crate::name::Name::new));
    assert!(armed()
        .gun_tile_resources_wanted(&g, 0)
        .contains(&crate::name!("oil")));
}
