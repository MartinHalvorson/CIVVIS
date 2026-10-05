use super::*;

fn capital_and_cheaper_city_state() -> (Game, AdvancedAi, usize, u32) {
    let mut g = Game::new_full(2, 48, 28, 91_002, 300, 1, false);
    let majors: Vec<_> = g
        .players
        .iter()
        .filter(|p| !p.is_minor && !p.is_barbarian)
        .map(|p| p.id)
        .collect();
    for pid in majors {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
    }
    g.current = 0;
    g.turn = 200;
    let minor = g.players.iter().find(|p| p.is_minor).unwrap().id;
    g.record_contact(0, 1);
    g.record_contact(0, minor);
    let capital = g.player_city_ids(1)[0];
    let capital_pos = g.cities[&capital].pos;
    let city_state = g.player_city_ids(minor)[0];
    let city_state_pos = g.cities[&city_state].pos;
    g.cities.get_mut(&capital).unwrap().wall_hp = 400;
    g.cities.get_mut(&capital).unwrap().buildings.extend([
        crate::name!("walls"),
        crate::name!("medieval_walls"),
        crate::name!("renaissance_walls"),
    ]);
    for _ in 0..3 {
        g.spawn_test_unit("giant_death_robot", 1, capital_pos);
    }
    g.cities.get_mut(&city_state).unwrap().wall_hp = 0;
    g.cities.get_mut(&city_state).unwrap().hp = 25;
    g.spawn_test_unit("scout", 0, capital_pos);
    g.spawn_test_unit("scout", 0, city_state_pos);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.belief.observe(&g, 0);
    assert_eq!(ai.domination_capital_target(&g, 0), Some((1, capital)));
    let minor_value = ai.campaign_target_value_with_culture(&g, 0, minor, None);
    let major_value = ai.campaign_target_value_with_culture(&g, 0, 1, None);
    assert!(
        minor_value < major_value,
        "the generic scorer must prefer the detour: minor={minor_value}, major={major_value}"
    );
    (g, ai, minor, capital)
}

#[test]
fn domination_chooses_a_required_capital_before_a_cheaper_city_state() {
    let (g, ai, _, capital) = capital_and_cheaper_city_state();
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(capital));
}

#[test]
fn capital_priority_preserves_a_forced_target_and_an_existing_war() {
    let (mut g, mut ai, minor, _) = capital_and_cheaper_city_state();
    ai.forced_target_player = Some(minor);
    assert_eq!(ai.assess(&g, 0).target_player, Some(minor));
    ai.forced_target_player = None;
    g.at_war.insert((0, minor));
    assert_eq!(ai.assess(&g, 0).target_player, Some(minor));
}

#[test]
fn selecting_a_required_capital_still_waits_for_a_ready_army() {
    let (mut g, mut ai, _, capital) = capital_and_cheaper_city_state();
    ai.enable_war_policy_via_board();
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.target_city, Some(capital));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !g.is_at_war(0, 1),
        "a preparation target does not waive the war-readiness checks"
    );
}

#[test]
fn capital_focus_preserves_the_required_capitals_owner_as_the_next_opponent() {
    let (g, mut ai, _, capital) = capital_and_cheaper_city_state();
    ai.enable_domination_capital_focus();
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(capital));
}

/// The native King seat saw Kongo's border city within a short march, but
/// repeatedly planned around Mali and then Inca cities beyond the ordinary
/// 18-tile declaration gate. A required capital remains the goal after the
/// frontier opens; it is not an actionable *first* objective from home.
#[test]
fn domination_opens_a_near_frontier_before_a_distant_capital() {
    let mut g = Game::new_full(3, 64, 40, 91_021, 650, 0, false);
    for pid in 0..3 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
    }
    g.current = 0;
    g.turn = 105;
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    while g.player_city_ids(0).len() < 4 {
        let site = g
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .filter(|pos| g.wdist(home, *pos) <= 12)
            .filter(|pos| g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 4))
            .min_by_key(|pos| (g.wdist(home, *pos), pos.0, pos.1))
            .unwrap();
        g.found_city_for(0, site, None);
    }
    let capitals = [g.player_city_ids(1)[0], g.player_city_ids(2)[0]];
    let mut far_sites: Vec<_> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .filter(|pos| {
            g.player_city_ids(0)
                .iter()
                .all(|city| g.wdist(g.cities[city].pos, *pos) > 22)
        })
        .collect();
    far_sites.sort_by_key(|pos| (pos.0, pos.1));
    let first_far = far_sites[0];
    let second_far = far_sites
        .into_iter()
        .find(|pos| g.wdist(*pos, first_far) >= 8)
        .unwrap();
    g.cities.get_mut(&capitals[0]).unwrap().pos = first_far;
    g.cities.get_mut(&capitals[1]).unwrap().pos = second_far;
    let frontier = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .filter(|pos| (6..=10).contains(&g.wdist(home, *pos)))
        .filter(|pos| g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 4))
        .min_by_key(|pos| (g.wdist(home, *pos), pos.0, pos.1))
        .unwrap();
    let border_city = g.found_city_for(1, frontier, Some("Frontier".to_string()));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.belief.observe(&g, 0);
    assert!(capitals
        .iter()
        .all(|city| !AdvancedAi::city_within_declaration_range(&g, 0, g.cities[city].pos)));
    assert!(AdvancedAi::city_within_declaration_range(
        &g,
        0,
        g.cities[&border_city].pos
    ));

    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(border_city));

    ai.forced_target_player = Some(2);
    assert_eq!(ai.assess(&g, 0).target_player, Some(2));
    ai.forced_target_player = None;
    g.at_war.insert((0, 2));
    assert_eq!(ai.assess(&g, 0).target_player, Some(2));
}

/// Native King run `civvis-20260928T082855Z` chose Thăng Long as its first
/// siege with a border city six tiles from home. After 33 turns at war, no
/// unit had reached the capital's ring and the peace desk called it stalled.
/// The 18-tile declaration gate is too broad for first-capture priority.
#[test]
fn domination_takes_a_short_border_city_before_a_declaration_reachable_capital() {
    let mut g = Game::new_full(2, 64, 40, 91_022, 650, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| g.units[id].kind == "settler")
            .unwrap();
        g.found_city_for(pid, g.units[&settler].pos, None);
    }
    g.current = 0;
    g.turn = 105;
    g.record_contact(0, 1);
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    while g.player_city_ids(0).len() < 4 {
        let site = g
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .filter(|pos| g.wdist(home, *pos) <= 12)
            .filter(|pos| g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 4))
            .min_by_key(|pos| (g.wdist(home, *pos), pos.0, pos.1))
            .unwrap();
        g.found_city_for(0, site, None);
    }
    let home_sites: Vec<_> = g
        .player_city_ids(0)
        .iter()
        .map(|city| g.cities[city].pos)
        .collect();
    let capital = g.player_city_ids(1)[0];
    let capital_site = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .find(|pos| {
            (14..=17).contains(
                &home_sites
                    .iter()
                    .map(|mine| g.wdist(*mine, *pos))
                    .min()
                    .unwrap(),
            )
        })
        .unwrap();
    g.cities.get_mut(&capital).unwrap().pos = capital_site;
    let frontier_site = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .find(|pos| {
            (5..=8).contains(
                &home_sites
                    .iter()
                    .map(|mine| g.wdist(*mine, *pos))
                    .min()
                    .unwrap(),
            ) && (4..=12).contains(&g.wdist(*pos, capital_site))
        })
        .unwrap();
    let frontier = g.found_city_for(1, frontier_site, Some("Frontier".to_string()));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.belief.observe(&g, 0);
    assert!(AdvancedAi::city_within_declaration_range(
        &g,
        0,
        g.cities[&capital].pos
    ));
    assert!(home_sites
        .iter()
        .any(|mine| g.wdist(*mine, g.cities[&frontier].pos) <= DOMINATION_FIRST_CAPTURE_MARCH));

    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Conquest);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(frontier));
}

/// In native King run `civvis-20260928T215559Z`, the first objective stayed
/// Pokrovka from turn 69 despite unwalled Pazyryk appearing two tiles nearer
/// to Guayaquil. The army opened on Pokrovka at turn 94, lost its early siege
/// guns there, and still had no original capital at the turn-235 loss.
#[test]
fn domination_opens_the_unwalled_foothold_before_a_near_walled_capital() {
    let mut g = Game::new_full(2, 36, 22, 91_023, 650, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 12), None);
    let capital = g.found_city_for(1, (13, 12), None);
    let foothold = g.found_city_for(1, (11, 11), None);
    g.cities.get_mut(&capital).unwrap().wall_hp = 100;
    g.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    g.current = 0;
    g.turn = 69;
    g.record_contact(0, 1);
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    assert_eq!(
        g.wdist(
            g.cities[&g.player_city_ids(0)[0]].pos,
            g.cities[&capital].pos
        ),
        7
    );
    assert_eq!(
        g.wdist(
            g.cities[&g.player_city_ids(0)[0]].pos,
            g.cities[&foothold].pos
        ),
        5
    );

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.belief.observe(&g, 0);
    let plan = ai.assess(&g, 0);
    assert_eq!(plan.strategy, GrandStrategy::Expansion);
    assert_eq!(plan.target_player, Some(1));
    assert_eq!(plan.target_city, Some(foothold));

    let mut walled_frontier = g.clone();
    walled_frontier.cities.get_mut(&foothold).unwrap().wall_hp = 100;
    assert_eq!(ai.assess(&walled_frontier, 0).target_city, Some(capital));

    ai.enable_siege_commitment();
    ai.plan = Some(plan);
    g.at_war.insert((0, 1));
    assert_eq!(ai.assess(&g, 0).target_city, Some(foothold));
}

/// The King Japan front kept a fully walled Muscat order while a land army
/// stood 3-5 tiles from three other Japanese cities and 17 from Muscat.
/// Commitment should preserve a real siege, not a march that never arrived.
#[test]
fn domination_retargets_untouched_distant_city_to_the_armys_front() {
    let mut g = Game::new_full(2, 64, 40, 91_024, 650, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(1, (45, 30), Some("Enemy Capital".to_string()));
    let prior = g.found_city_for(1, (35, 20), Some("Distant Walls".to_string()));
    let nearer = g.found_city_for(1, (13, 12), Some("Near Walls".to_string()));
    for city in [prior, nearer] {
        let target = g.cities.get_mut(&city).unwrap();
        target.buildings.extend([
            crate::name!("walls"),
            crate::name!("medieval_walls"),
            crate::name!("renaissance_walls"),
        ]);
        let wall_hp = g.city_max_wall_hp(&g.cities[&city]);
        g.cities.get_mut(&city).unwrap().wall_hp = wall_hp;
    }
    let soldier = g.spawn_test_unit("tank", 0, (11, 12));
    g.current = 0;
    g.turn = 200;
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_commitment();
    ai.belief.observe(&g, 0);
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(prior),
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    });
    assert!(g.wdist(g.units[&soldier].pos, g.cities[&prior].pos) >= 17);
    assert!(g.wdist(g.units[&soldier].pos, g.cities[&nearer].pos) <= 3);
    assert!(!g.cities[&prior].is_capital);
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, prior, GrandStrategy::Conquest),
        Some(nearer)
    );
    assert_eq!(ai.assess(&g, 0).target_city, Some(nearer));

    let screen = g.spawn_test_unit("tank", 0, (32, 20));
    assert!(g.wdist(g.units[&screen].pos, g.cities[&prior].pos) <= 6);
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, prior, GrandStrategy::Conquest),
        None,
        "a staged force keeps its walled objective"
    );
    g.remove_unit(screen);
    g.cities.get_mut(&prior).unwrap().wall_hp -= 50;
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, prior, GrandStrategy::Conquest),
        None,
        "damage already done to the original wall keeps the siege"
    );

    // The live King front held an untouched, distant unwalled Trier from
    // t123 to t138 while its army stood beside unwalled Munich. Munich built
    // its first wall on t139 and finished 400 wall HP by t141; only then did
    // the walled-objective rule release the stale Trier order.
    for city in [prior, nearer] {
        let target = g.cities.get_mut(&city).unwrap();
        target.buildings.clear();
        target.wall_hp = 0;
    }
    assert_eq!(g.city_max_wall_hp(&g.cities[&prior]), 0);
    assert_eq!(g.city_max_wall_hp(&g.cities[&nearer]), 0);
    Arc::make_mut(&mut g.observed_city_strength).insert(prior, 59.0);
    Arc::make_mut(&mut g.observed_city_strength).insert(nearer, 74.0);
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, prior, GrandStrategy::Conquest),
        Some(nearer),
        "an open city beside the army is a better first capture than a weaker remote city"
    );
    assert_eq!(ai.assess(&g, 0).target_city, Some(nearer));

    let screen = g.spawn_test_unit("tank", 0, (32, 20));
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, prior, GrandStrategy::Conquest),
        None,
        "a unit already on the old approach keeps the committed objective"
    );
    g.remove_unit(screen);
    g.cities.get_mut(&prior).unwrap().hp -= 20;
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, prior, GrandStrategy::Conquest),
        None,
        "an open city already damaged keeps its siege"
    );
}

/// `stale-swap-reads-the-march`: live King civvis-20261005T124739Z (game
/// 130) held walled Delhi, India's capital, at an army median of 15-20 tiles
/// while walled Jabalpur stood 4 tiles from our city. Every distance gate of
/// the stale swap passed; the capital's bonus failed the value margin.
#[test]
fn a_distant_capital_yields_to_a_near_city_when_the_army_body_is_far() {
    let mut g = Game::new_full(2, 64, 40, 91_031, 650, 0, false);
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(0, (20, 14), None);
    let capital = g.found_city_for(1, (28, 12), Some("Distant Capital".to_string()));
    let nearer = g.found_city_for(1, (13, 12), Some("Near Walls".to_string()));
    for city in [capital, nearer] {
        let target = g.cities.get_mut(&city).unwrap();
        target.buildings.extend([
            crate::name!("walls"),
            crate::name!("medieval_walls"),
            crate::name!("renaissance_walls"),
        ]);
        let wall_hp = g.city_max_wall_hp(&g.cities[&city]);
        g.cities.get_mut(&city).unwrap().wall_hp = wall_hp;
    }
    g.spawn_test_unit("tank", 0, (11, 12));
    g.current = 0;
    g.turn = 200;
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_commitment();
    ai.belief.observe(&g, 0);
    assert!(g.cities[&capital].is_capital);

    // The gene off, the capital's bonus holds the stale march.
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, capital, GrandStrategy::Conquest),
        None
    );
    // On, the army's body 17 tiles out releases it for the city beside it.
    ai.enable_stale_swap_reads_the_march();
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, capital, GrandStrategy::Conquest),
        Some(nearer)
    );

    // A body within 16 tiles keeps the margin: two tanks 12 tiles out make
    // the median 12, every distance gate still passes, and the capital holds.
    let closer: Vec<u32> = (0..2)
        .map(|_| g.spawn_test_unit("tank", 0, (16, 12)))
        .collect();
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, capital, GrandStrategy::Conquest),
        None,
        "an army body within the march keeps the capital"
    );
    for uid in closer {
        g.remove_unit(uid);
    }

    // A unit on the old approach keeps the siege, gene or not.
    let screen = g.spawn_test_unit("tank", 0, (24, 12));
    assert_eq!(
        ai.stale_domination_objective_city(&g, 0, capital, GrandStrategy::Conquest),
        None,
        "a unit already within the siege reach keeps the committed objective"
    );
    g.remove_unit(screen);
}

/// `campaign-weighs-the-tourism-leader`: the rival leading the board's
/// tourism by the margin, over the floor, gets `TOURISM_LEADER_TILES` of march
/// in the campaign's rival ranking; a close race or a thin one gets none.
#[test]
fn the_campaign_weighs_the_tourism_leader_under_the_gene() {
    let mut g = Game::new_full(3, 40, 24, 109_106_002, 300, 0, false);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
    }
    g.found_city_for(0, crate::hex::offset_to_axial(4, 8), None);
    g.found_city_for(1, crate::hex::offset_to_axial(20, 8), None);
    g.found_city_for(2, crate::hex::offset_to_axial(20, 18), None);
    let set = |g: &mut Game, one: f64, two: f64| {
        g.observed_tourism_per_turn =
            std::sync::Arc::new([(1, one), (2, two)].into_iter().collect());
    };
    set(&mut g, 200.0, 60.0);
    assert_eq!(AdvancedAi::tourism_leader(&g, 0), Some(1));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let off = ai.campaign_target_value_with_culture(&g, 0, 1, None);
    let other_off = ai.campaign_target_value_with_culture(&g, 0, 2, None);
    ai.enable_campaign_weighs_the_tourism_leader();
    let on = ai.campaign_target_value_with_culture(&g, 0, 1, None);
    assert!(
        (off - on - TOURISM_LEADER_TILES * 7.0).abs() < 1e-9,
        "off {off} on {on}"
    );
    assert_eq!(
        ai.campaign_target_value_with_culture(&g, 0, 2, None),
        other_off
    );
    set(&mut g, 70.0, 60.0);
    assert_eq!(AdvancedAi::tourism_leader(&g, 0), None, "inside the margin");
    set(&mut g, 40.0, 10.0);
    assert_eq!(AdvancedAi::tourism_leader(&g, 0), None, "under the floor");
}
