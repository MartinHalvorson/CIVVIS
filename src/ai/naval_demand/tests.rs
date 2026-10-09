use super::*;

fn board() -> (Game, BasicAi, Vec<u32>) {
    let mut g = Game::new_full(3, 60, 36, 40_230_001, 250, 0, true);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = false;
        tile.feature = None;
        tile.resource = None;
    }
    let cities: Vec<_> = [(8, 8), (16, 8), (24, 8), (32, 8)]
        .into_iter()
        .map(|pos| {
            let cid = g.found_city_for(0, pos, None);
            g.map.tiles.get_mut(&(pos.0, pos.1 + 1)).unwrap().terrain = crate::name!("coast");
            cid
        })
        .collect();
    g.players[0].techs.extend(
        ["sailing", "shipbuilding", "cartography"]
            .into_iter()
            .map(Name::new),
    );
    let mut ai = BasicAi::new();
    ai.open_water_navy = true;
    (g, ai, cities)
}

fn raider(g: &mut Game, pos: Pos) -> u32 {
    g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    let barb = g.barb_pid.expect("fixture has barbarians");
    g.spawn_unit("ironclad", barb, pos)
}

#[test]
fn remote_barbarian_ship_keeps_the_peacetime_fleet_budget() {
    let (mut g, ai, cities) = board();
    let distant = (42, 26);
    assert!(cities
        .iter()
        .all(|cid| g.wdist(g.cities[cid].pos, distant) > HOME_THREAT_RADIUS));
    raider(&mut g, distant);
    assert_eq!(ai.desired_navy(&g, 0), 2);
}

#[test]
fn local_barbarian_defense_counts_exposed_coasts_instead_of_every_port() {
    let (mut g, ai, _) = board();
    raider(&mut g, (8, 10));
    assert_eq!(ai.desired_navy(&g, 0), 2, "one exposed coast plus a spare");
    raider(&mut g, (16, 10));
    assert_eq!(ai.desired_navy(&g, 0), 3, "two exposed coasts plus a spare");
}

#[test]
fn several_raiders_at_one_port_do_not_count_the_same_city_twice() {
    let (mut g, ai, _) = board();
    for pos in [(8, 10), (8, 11), (9, 10)] {
        raider(&mut g, pos);
    }
    assert_eq!(ai.desired_navy(&g, 0), 2);
}

#[test]
fn a_naval_war_with_a_civilization_keeps_the_full_fleet() {
    for with_city in [false, true] {
        let (mut g, ai, cities) = board();
        g.at_war.insert((0, 1));
        if with_city {
            let pos = (42, 26);
            g.found_city_for(1, pos, None);
            g.map.tiles.get_mut(&(42, 27)).unwrap().terrain = crate::name!("coast");
        } else {
            g.map.tiles.get_mut(&(42, 26)).unwrap().terrain = crate::name!("coast");
            g.spawn_unit("ironclad", 1, (42, 26));
        }
        assert_eq!(ai.desired_navy(&g, 0), cities.len() + 1);
    }
}

#[test]
fn cities_without_a_launch_site_and_an_empire_without_sailing_need_no_fleet() {
    let (mut g, ai, cities) = board();
    raider(&mut g, (8, 10));
    g.players[0].techs.remove(&crate::name!("sailing"));
    assert_eq!(ai.desired_navy(&g, 0), 0);
    g.players[0].techs.insert(crate::name!("sailing"));
    for cid in cities {
        let pos = g.cities[&cid].pos;
        g.map.tiles.get_mut(&(pos.0, pos.1 + 1)).unwrap().terrain = crate::name!("grassland");
    }
    assert_eq!(ai.desired_navy(&g, 0), 0);
}

#[test]
fn distant_raiders_keep_one_early_explorer_and_two_settler_escorts() {
    let (mut g, ai, _) = board();
    g.players[0].techs.remove(&crate::name!("cartography"));
    raider(&mut g, (42, 26));
    assert_eq!(ai.desired_navy(&g, 0), 1);
    g.spawn_unit("settler", 0, (8, 8));
    assert_eq!(ai.desired_navy(&g, 0), 2);
}

#[test]
fn a_raider_at_an_inland_city_does_not_raise_the_launchable_coasts_budget() {
    let (mut g, ai, _) = board();
    let inland = g.found_city_for(0, (42, 20), None);
    raider(&mut g, (42, 26));
    assert!(!ai.naval_city_can_launch(&g, inland));
    assert_eq!(ai.desired_navy(&g, 0), 2);
}

#[test]
fn a_harbor_counts_as_an_exposed_launch_site_and_lakes_keep_their_existing_gate() {
    let (mut g, mut ai, _) = board();
    let port = g.found_city_for(0, (42, 20), None);
    g.map.tiles.get_mut(&(42, 22)).unwrap().terrain = crate::name!("coast");
    g.cities
        .get_mut(&port)
        .unwrap()
        .districts
        .insert(crate::name!("harbor"), (42, 22));
    raider(&mut g, (42, 26));
    raider(&mut g, (8, 10));
    assert_eq!(ai.desired_navy(&g, 0), 3);
    g.map.tiles.get_mut(&(42, 22)).unwrap().terrain = crate::name!("lake");
    assert_eq!(
        ai.desired_navy(&g, 0),
        2,
        "a lake harbor cannot launch into open water"
    );
    g.map.tiles.get_mut(&(42, 21)).unwrap().terrain = crate::name!("lake");
    ai.open_water_navy = false;
    assert_eq!(
        ai.desired_navy(&g, 0),
        3,
        "explicit withholding retains the lake-inclusive launch rule"
    );
}
