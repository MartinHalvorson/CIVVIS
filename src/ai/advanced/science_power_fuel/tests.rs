use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

fn board() -> (Game, AdvancedAi) {
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
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 7;
        city.districts.insert(crate::name!("campus"), site);
        city.buildings
            .extend([crate::name!("library"), crate::name!("university")]);
        g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    }
    g.players[0].techs = g.rules.techs.keys().copied().collect();
    g.players[0].gold = 300.0;
    g.players[0].gold_per_turn = 50.0;
    g.turn = 150;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_science_power_buys_fuel();
    (g, ai)
}

#[test]
fn science_power_fuel_is_an_opt_in() {
    opt_in_off_in_both_controllers("science-power-buys-fuel", |ai| ai.science_power_buys_fuel);
}

#[test]
fn ready_research_power_names_a_bounded_coal_block() {
    let (g, ai) = board();
    assert!(g
        .player_city_ids(0)
        .iter()
        .all(|cid| g.city_power_demand(&g.cities[cid]) > 0.0));
    let wants = ai.science_power_fuel_wants(&g, 0);
    assert_eq!(wants.len(), 1);
    assert_eq!(wants[0].resource, "RESOURCE_COAL");
    assert_eq!((wants[0].amount, wants[0].minimum), (20, 10));
}

#[test]
fn another_lane_or_a_late_deadline_keeps_the_market_closed() {
    let (mut g, mut ai) = board();
    ai.retarget(VictoryTarget::Culture);
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
    ai.retarget(VictoryTarget::Science);
    g.turn = 249;
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
}

#[test]
fn treasury_and_income_protect_the_reserve() {
    let (mut g, ai) = board();
    g.players[0].gold = 99.0;
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
    g.players[0].gold = 300.0;
    g.players[0].gold_per_turn = -1.0;
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
}

#[test]
fn a_fuelled_buildable_alternative_does_not_buy_coal() {
    let (mut g, ai) = board();
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), 10.0);
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
}

#[test]
fn a_pillaged_lab_is_not_power_demand_to_buy_for() {
    let (mut g, ai) = board();
    let first = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&first)
        .unwrap()
        .pillaged_buildings
        .insert(crate::name!("research_lab"));
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
}

#[test]
fn an_inactive_existing_plant_does_not_buy_fuel() {
    let (mut g, ai) = board();
    let first = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&first)
        .unwrap()
        .buildings
        .push(crate::name!("coal_power_plant"));
    let site = *g.cities[&first]
        .districts
        .get(crate::name!("industrial_zone"))
        .unwrap();
    g.map.tiles.get_mut(&site).unwrap().pillaged = true;
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
}

#[test]
fn no_ready_plant_or_only_one_lab_does_not_buy() {
    let (mut g, ai) = board();
    let first = g.player_city_ids(0)[0];
    g.cities
        .get_mut(&first)
        .unwrap()
        .buildings
        .retain(|b| *b != "factory");
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
    g.cities
        .get_mut(&first)
        .unwrap()
        .buildings
        .push(crate::name!("factory"));
    let second = g.player_city_ids(0)[1];
    g.cities.get_mut(&second).unwrap().buildings.clear();
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
}

#[test]
fn a_stocked_or_connected_coal_source_releases_the_want() {
    let (mut g, ai) = board();
    g.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 10.0);
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
    g.players[0]
        .strategic_resources
        .insert(crate::name!("coal"), 0.0);
    let cid = g.player_city_ids(0)[0];
    let pos = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&cid].pos && g.map.tiles[pos].district.is_none())
        .unwrap();
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.resource = Some(crate::name!("coal"));
    tile.improvement = Some(crate::name!("mine"));
    tile.pillaged = false;
    assert!(g.strategic_resource_rate(0, "coal") > 0.0);
    assert!(ai.science_power_fuel_wants(&g, 0).is_empty());
}
