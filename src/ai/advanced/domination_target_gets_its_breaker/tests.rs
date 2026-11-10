use super::*;

/// Two founded empires at peace, met, our army of two Warriors at home, idle
/// queues, the rival's capital unwalled, turn 60, the Domination lane with
/// the gene `on`.
fn case(on: bool) -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 26, 79_301, 500, 0, false);
    g.current = 0;
    let settlers: Vec<_> = g
        .units
        .values()
        .filter(|u| u.kind == "settler")
        .map(|u| (u.owner, u.pos))
        .collect();
    for (owner, pos) in settlers {
        g.found_city_for(owner, pos, None);
    }
    let home = g.player_city_ids(0)[0];
    let target = g.player_city_ids(1)[0];
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for cid in [home, target] {
        g.cities.get_mut(&cid).unwrap().queue.clear();
    }
    let pos = g.cities[&home].pos;
    for kind in ["warrior", "warrior"] {
        g.spawn_unit(kind, 0, pos);
    }
    g.players[0]
        .techs
        .extend(["masonry", "bronze_working"].map(crate::name::Name::new));
    g.players[1].techs.insert(crate::name::Name::new("masonry"));
    g.players[0].met.insert(1);
    g.players[1].met.insert(0);
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 10.0;
    g.turn = 60;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    if on {
        ai.enable_domination_target_gets_its_breaker();
    }
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, home, target)
}

fn head(g: &Game, cid: u32) -> Option<&str> {
    match g.cities[&cid].queue.first() {
        Some(Item::Unit { unit }) => Some(unit.as_str()),
        _ => None,
    }
}

#[test]
fn the_nearest_land_capital_is_the_target_before_any_war() {
    let (g, ai, plan, _, target) = case(true);
    assert!(BasicAi::capital_landmass(&g, 0).contains(&g.cities[&target].pos));
    assert_eq!(ai.domination_breach_target(&g, 0, &plan), Some(target));
    // Off, on another lane, too early, or an unmet rival: no target.
    let (g, ai, plan, _, _) = case(false);
    assert_eq!(ai.domination_breach_target(&g, 0, &plan), None);
    let (g, mut ai, plan, _, _) = case(true);
    ai.victory_target = Some(VictoryTarget::Science);
    assert_eq!(ai.domination_breach_target(&g, 0, &plan), None);
    let (mut g, ai, plan, _, _) = case(true);
    g.turn = 20;
    assert_eq!(ai.domination_breach_target(&g, 0, &plan), None);
    let (mut g, ai, plan, _, _) = case(true);
    g.players[0].met.clear();
    assert_eq!(ai.domination_breach_target(&g, 0, &plan), None);
}

#[test]
fn a_conquest_or_war_target_is_the_target_at_any_turn() {
    let (mut g, ai, mut plan, _, target) = case(true);
    g.turn = 20;
    plan.target_player = Some(1);
    plan.target_city = Some(target);
    // An Expansion plan's target is not yet a war.
    assert_eq!(ai.domination_breach_target(&g, 0, &plan), None);
    plan.strategy = GrandStrategy::Conquest;
    assert_eq!(ai.domination_breach_target(&g, 0, &plan), Some(target));
    plan.strategy = GrandStrategy::Expansion;
    g.at_war.insert((0, 1));
    assert_eq!(ai.domination_breach_target(&g, 0, &plan), Some(target));
}

#[test]
fn ancient_walls_take_a_ram_before_the_war() {
    for on in [false, true] {
        let (mut g, ai, plan, home, _) = case(on);
        let claim = ai.claim_domination_breaker(&mut g, 0, &plan);
        assert_eq!(claim.is_some(), on, "gene {on}");
        assert_eq!(
            head(&g, home),
            on.then_some("battering_ram"),
            "gene {on}: the home queue"
        );
    }
}

#[test]
fn a_held_or_queued_breaker_closes_the_claim() {
    let (mut g, ai, plan, home, _) = case(true);
    g.spawn_unit("battering_ram", 0, g.cities[&home].pos);
    assert!(ai.claim_domination_breaker(&mut g, 0, &plan).is_none());
    let (mut g, ai, plan, _, _) = case(true);
    assert!(ai.claim_domination_breaker(&mut g, 0, &plan).is_some());
    assert!(
        ai.claim_domination_breaker(&mut g, 0, &plan).is_none(),
        "the queued ram closes it"
    );
}

#[test]
fn medieval_walls_take_a_tower_and_higher_walls_a_gun() {
    let (mut g, ai, plan, home, target) = case(true);
    g.players[0]
        .techs
        .extend(["machinery", "engineering"].map(crate::name::Name::new));
    let city = g.cities.get_mut(&target).unwrap();
    city.buildings.push(crate::name!("walls"));
    city.buildings.push(crate::name!("medieval_walls"));
    city.wall_hp = 200;
    assert!(!AdvancedAi::domination_breaker_fits(
        &g,
        "battering_ram",
        target
    ));
    assert!(AdvancedAi::domination_breaker_fits(
        &g,
        "siege_tower",
        target
    ));
    // A ram already held does not open these walls.
    g.spawn_unit("battering_ram", 0, g.cities[&home].pos);
    assert!(ai.claim_domination_breaker(&mut g, 0, &plan).is_some());
    assert_eq!(head(&g, home), Some("siege_tower"));

    let (mut g, ai, plan, home, target) = case(true);
    g.players[0]
        .techs
        .extend(["machinery", "engineering"].map(crate::name::Name::new));
    let city = g.cities.get_mut(&target).unwrap();
    for building in ["walls", "medieval_walls", "renaissance_walls"] {
        city.buildings.push(crate::name::Name::new(building));
    }
    city.wall_hp = 300;
    assert!(!AdvancedAi::domination_breaker_fits(
        &g,
        "siege_tower",
        target
    ));
    let claim = ai.claim_domination_breaker(&mut g, 0, &plan);
    if AdvancedAi::domination_breaker_fits(&g, "catapult", target) {
        assert!(claim.is_some());
        assert_eq!(head(&g, home), Some("catapult"));
    } else {
        assert!(claim.is_none(), "no gun of ours dents these walls");
    }
}

#[test]
fn the_held_declaration_buys_the_breaker_once() {
    let (mut g, mut ai, mut plan, home, target) = case(true);
    plan.strategy = GrandStrategy::Conquest;
    plan.target_player = Some(1);
    plan.target_city = Some(target);
    let objective = g.cities[&target].pos;
    g.cities.get_mut(&target).unwrap().wall_hp = 100;
    g.cities
        .get_mut(&target)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    // Nothing waits on the breaker yet: no purchase.
    assert!(!ai.buy_domination_breaker(&mut g, 0, &plan));
    ai.declaration_breaker_hold = Some((objective, g.turn));
    ai.declaration_hold_seen = g.turn;
    let warrior = g
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "warrior")
        .map(|unit| unit.id)
        .unwrap();
    g.units.get_mut(&warrior).unwrap().pos = objective;
    let gold = g.players[0].gold;
    let near = g.wdist(g.cities[&home].pos, objective) <= BREAKER_BUY_REACH;
    assert_eq!(ai.buy_domination_breaker(&mut g, 0, &plan), near);
    if near {
        assert!(g.players[0].gold < gold);
        assert!(g
            .units
            .values()
            .any(|unit| unit.owner == 0 && unit.kind == "battering_ram"));
        assert!(
            !ai.buy_domination_breaker(&mut g, 0, &plan),
            "one is alive now"
        );
    }
    // Gene off: nothing.
    let (mut g, mut ai, mut plan, _, target) = case(false);
    plan.target_player = Some(1);
    plan.target_city = Some(target);
    g.cities.get_mut(&target).unwrap().wall_hp = 100;
    ai.declaration_breaker_hold = Some((objective, g.turn));
    ai.declaration_hold_seen = g.turn;
    assert!(!ai.buy_domination_breaker(&mut g, 0, &plan));
}

#[test]
fn a_ram_walks_to_the_army_before_the_war() {
    let (mut g, ai, plan, home, target) = case(true);
    let objective = g.cities[&target].pos;
    let ram = g.spawn_unit("battering_ram", 0, g.cities[&home].pos);
    // No melee of ours near the walls yet: the ram stays to the ladder.
    assert_eq!(ai.domination_breaker_step(&mut g, 0, ram, &plan), None);
    let warrior = g
        .units
        .values()
        .find(|unit| unit.owner == 0 && unit.kind == "warrior")
        .map(|unit| unit.id)
        .unwrap();
    let ring = g
        .wdisk(objective, 3)
        .into_iter()
        .filter(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                && g.city_at(*pos).is_none()
        })
        .min_by_key(|pos| (g.wdist(*pos, g.cities[&home].pos), *pos))
        .unwrap();
    g.units.get_mut(&warrior).unwrap().pos = ring;
    let before = g.wdist(g.units[&ram].pos, ring);
    assert_eq!(
        ai.domination_breaker_step(&mut g, 0, ram, &plan),
        Some(true)
    );
    assert!(g.wdist(g.units[&ram].pos, ring) < before);
    // At war the siege's own movers take it.
    g.at_war.insert((0, 1));
    assert_eq!(ai.domination_breaker_step(&mut g, 0, ram, &plan), None);
}
