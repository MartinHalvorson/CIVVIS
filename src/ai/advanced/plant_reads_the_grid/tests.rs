use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

/// Two science cities four tiles apart: the first holds an Industrial Zone,
/// a Factory and a Research Lab (demand 5, the live shape), the second a Lab.
fn board() -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(1, 24, 16, 91_989, 250, 0, false);
    let uid = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let home = g.units[&uid].pos;
    let first = g.found_city_for(0, home, None);
    g.remove_unit(uid);
    let pos = g
        .map
        .tiles
        .iter()
        .find(|(pos, tile)| {
            g.wdist(home, **pos) == 4
                && !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && tile.owner_city.is_none()
        })
        .map(|(p, _)| *p)
        .unwrap();
    let second = g.found_city_for(0, pos, None);
    let site = g.cities[&first]
        .owned_tiles
        .iter()
        .copied()
        .find(|p| *p != home)
        .unwrap();
    g.cities
        .get_mut(&first)
        .unwrap()
        .districts
        .insert(crate::name!("industrial_zone"), site);
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("industrial_zone"));
    g.cities.get_mut(&first).unwrap().buildings.extend([
        crate::name!("workshop"),
        crate::name!("factory"),
        crate::name!("research_lab"),
    ]);
    g.cities
        .get_mut(&second)
        .unwrap()
        .buildings
        .push(crate::name!("research_lab"));
    for cid in [first, second] {
        let site = g.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .find(|pos| {
                g.map.tiles[pos].district.is_none()
                    && !g.rules.is_water(&g.map.tiles[pos])
                    && g.rules.is_passable(&g.map.tiles[pos])
            })
            .unwrap();
        g.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .insert(crate::name!("campus"), site);
        g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    }
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.turn = 150;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_plant_reads_the_grid();
    (g, ai, first, second)
}

fn stock(g: &mut Game, fuel: &str, amount: f64) {
    g.players[0]
        .strategic_resources
        .insert(Name::new(fuel), amount);
}

/// What the plant `plant` built in `cid` is credited.
fn credit(ai: &AdvancedAi, g: &Game, cid: u32, plant: &str) -> Yields {
    ai.plant_grid_yields(g, &g.cities[&cid], &g.rules.buildings[plant])
}

#[test]
fn plant_reads_the_grid_is_an_opt_in() {
    opt_in_off_in_both_controllers("plant-reads-the-grid", |ai| ai.plant_reads_the_grid);
}

/// The stock credit reads 4 Power against a demand of 5 and pays nothing in
/// exactly the Lab-and-Factory city; the host burns a second unit instead.
#[test]
fn a_lab_and_factory_city_is_credited_where_the_stock_read_pays_nothing() {
    let (mut g, ai, first, _) = board();
    assert_eq!(
        g.city_power_demand(&g.cities[&first]),
        5.0,
        "a Factory and a Lab"
    );
    assert_eq!(
        AdvancedAi::power_switched_on(&g, &g.cities[&first], &g.rules.buildings["oil_power_plant"]),
        Yields::default(),
        "the stock read: 4 Power leaves 5 short, so nothing"
    );
    // Forty banked over twenty turns is the two units 5 Power burns.
    stock(&mut g, "oil", 40.0);
    let credited = credit(&ai, &g, first, "oil_power_plant");
    assert_eq!(credited.science, 5.0, "the Lab's powered half");
    assert_eq!(
        credited.production, 0.0,
        "a Factory is regional, so its powered half is left out, as in the stock read"
    );
}

/// Twenty-five banked is 1.25 a turn: one unit for the Lab alone, never the
/// two the Lab-and-Factory city burns, and part power switches nothing on.
#[test]
fn the_fuel_feeds_every_city_in_reach_it_can_afford() {
    let (mut g, ai, first, _) = board();
    stock(&mut g, "oil", 60.0);
    assert_eq!(credit(&ai, &g, first, "oil_power_plant").science, 10.0);
    stock(&mut g, "oil", 40.0);
    assert_eq!(credit(&ai, &g, first, "oil_power_plant").science, 5.0);
    stock(&mut g, "oil", 25.0);
    assert_eq!(credit(&ai, &g, first, "oil_power_plant").science, 5.0);
}

#[test]
fn no_fuel_buys_no_plant_and_the_wrong_fuel_counts_for_nothing() {
    let (mut g, ai, first, _) = board();
    assert_eq!(credit(&ai, &g, first, "oil_power_plant"), Yields::default());
    stock(&mut g, "coal", 60.0);
    assert_eq!(
        credit(&ai, &g, first, "oil_power_plant"),
        Yields::default(),
        "Coal cannot run an Oil plant"
    );
}

#[test]
fn a_city_a_burning_plant_serves_is_not_paid_twice() {
    let (mut g, ai, first, second) = board();
    stock(&mut g, "oil", 60.0);
    stock(&mut g, "coal", 60.0);
    // A burning Coal plant beside the second city, whose reach covers both.
    let site = g.cities[&second]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| {
            g.map.tiles[pos].district.is_none()
                && !g.rules.is_water(&g.map.tiles[pos])
                && g.rules.is_passable(&g.map.tiles[pos])
        })
        .unwrap();
    g.cities
        .get_mut(&second)
        .unwrap()
        .districts
        .insert(crate::name!("industrial_zone"), site);
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("industrial_zone"));
    g.cities
        .get_mut(&second)
        .unwrap()
        .buildings
        .push(crate::name!("coal_power_plant"));
    assert_eq!(credit(&ai, &g, first, "oil_power_plant"), Yields::default());
    // With the Coal gone that plant is dark, and the Oil plant is worth both.
    stock(&mut g, "coal", 0.0);
    assert_eq!(credit(&ai, &g, first, "oil_power_plant").science, 10.0);
}

#[test]
fn a_city_beyond_reach_earns_nothing_and_a_building_is_not_a_plant() {
    let (mut g, ai, first, second) = board();
    stock(&mut g, "oil", 60.0);
    let home = g.cities[&first].pos;
    let far = g
        .map
        .tiles
        .keys()
        .copied()
        .find(|pos| g.wdist(*pos, home) > 8)
        .expect("a tile beyond reach");
    g.cities.get_mut(&second).unwrap().pos = far;
    assert_eq!(credit(&ai, &g, first, "oil_power_plant").science, 5.0);
    assert_eq!(credit(&ai, &g, first, "research_lab"), Yields::default());
}

/// The one claim that names a plant named Coal alone, so an Oil empire's
/// plant hung on hints that do not land. Under the gene it names the plant
/// the fuel can run; with no fuel, or with the gene off, it names nothing.
#[test]
fn the_claim_takes_the_plant_the_fuel_can_run() {
    let (mut g, mut ai, first, _) = board();
    assert!(ai.grid_plant_claim(&g, 0, first).is_none(), "no fuel");
    stock(&mut g, "oil", 40.0);
    let (item, reach) = ai.grid_plant_claim(&g, 0, first).expect("Oil runs a plant");
    assert!(
        item == Item::Building {
            building: crate::name!("oil_power_plant")
        }
    );
    assert_eq!(reach, 1, "two units feed the Lab-and-Factory city alone");
    stock(&mut g, "oil", 60.0);
    assert_eq!(ai.grid_plant_claim(&g, 0, first).unwrap().1, 2);
    ai.disable_plant_reads_the_grid();
    assert!(ai.grid_plant_claim(&g, 0, first).is_none(), "off");
}

/// A claimed plant stays at the head: the review and the deterrent leave what
/// `factory_claim_holds` keeps alone. Off, an Oil plant is not held.
#[test]
fn the_claimed_oil_plant_keeps_the_queue() {
    let (g, mut ai, first, _) = board();
    ai.enable_industrial_zone_claims_the_factory();
    let plan = ai.assess(&g, 0);
    let oil = Item::Building {
        building: crate::name!("oil_power_plant"),
    };
    assert!(ai.factory_claim_holds(&g, 0, first, &plan, &oil));
    ai.disable_plant_reads_the_grid();
    assert!(!ai.factory_claim_holds(&g, 0, first, &plan, &oil));
}

#[test]
fn a_diplomacy_seat_keeps_its_refusal_of_fossil_power() {
    let (mut g, mut ai, first, _) = board();
    stock(&mut g, "oil", 60.0);
    ai.retarget(VictoryTarget::Diplomacy);
    assert_eq!(credit(&ai, &g, first, "oil_power_plant"), Yields::default());
}
