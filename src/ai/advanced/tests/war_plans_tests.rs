//! Tests of `advanced/war_plans.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn selective_timed_war_requires_one_organic_target_and_three_prebuilt_bodies() {
    let (mut game, _, objective) = timed_war_fixture(940_014);
    clear_military(&mut game, 0);
    let mut ai = AdvancedAi::selective_timing_attack();

    assert!(
        ai.choose_war_plan(&game, 0).is_none(),
        "selective v2 cannot manufacture Conquest before ordinary assessment chooses it"
    );
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    });
    assert!(
        ai.choose_war_plan(&game, 0).is_none(),
        "an organic target without a real army cannot redirect the economy into v1 mobilization"
    );

    game.players[0].techs.insert(crate::name!("bronze_working"));
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..SELECTIVE_TIMED_WAR_PREBUILT_BODIES {
        game.spawn_test_unit("spearman", 0, home);
    }
    let plan = ai
        .choose_war_plan(&game, 0)
        .expect("three direct predecessors make the organic campaign eligible");
    assert_eq!(plan.target_player, 1);
    assert_eq!(plan.assault_unit, crate::name!("pikeman"));
    assert_eq!(plan.predecessor, Some(crate::name!("spearman")));

    ai.war_census.appointments = 1;
    assert!(
        ai.choose_war_plan(&game, 0).is_none(),
        "selective v2 gets one appointment, even after that campaign ends"
    );
}

#[test]
fn selective_timed_war_strengthens_the_same_staging_gate() {
    let v1 = AdvancedAi::timing_attack();
    let v2 = AdvancedAi::selective_timing_attack();
    let mut status = WarPackageStatus {
        staged_bodies: TIMED_WAR_BODIES - 1,
        fourth_one_turn_away: true,
        breach_staged: true,
        local_strength_ratio: 1.0,
        ..WarPackageStatus::default()
    };
    assert!(v1.timed_war_staging_ready(&status));
    assert!(
        !v2.timed_war_staging_ready(&status),
        "v2 cannot call three staged bodies a strike package"
    );

    status.staged_bodies = TIMED_WAR_BODIES;
    status.local_strength_ratio = SELECTIVE_TIMED_WAR_STRENGTH_FLOOR - 0.01;
    assert!(!v2.timed_war_staging_ready(&status));
    status.local_strength_ratio = SELECTIVE_TIMED_WAR_STRENGTH_FLOOR;
    assert!(
        v2.timed_war_staging_ready(&status),
        "the exact preregistered four-body, 1.25-strength gate launches"
    );
}

#[test]
fn rapid_timed_war_requires_a_complete_near_term_package() {
    let (mut game, _, objective) = timed_war_fixture(940_015);
    clear_military(&mut game, 0);
    let mut ai = AdvancedAi::rapid_timing_attack();
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(objective),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    });

    let goal = crate::name!("military_tactics");
    for tech in [
        "mining",
        "bronze_working",
        "pottery",
        "writing",
        "currency",
        "mathematics",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.players[0].research = Some(goal.as_str().to_string());
    game.players[0].research_progress = game.tech_cost(goal.as_str()) - 1.0;
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..TIMED_WAR_BODIES - 1 {
        game.spawn_test_unit("spearman", 0, home);
    }
    assert!(
        ai.choose_war_plan(&game, 0).is_none(),
        "v3 cannot appoint a package that still needs its fourth body built"
    );
    game.spawn_test_unit("spearman", 0, home);
    assert_eq!(
        ai.war_existing_or_queued_count(
            &game,
            0,
            crate::name!("pikeman"),
            Some(crate::name!("spearman"))
        ),
        TIMED_WAR_BODIES
    );
    assert!(ai
        .war_staging_route(&game, 0, 1, game.cities[&objective].pos)
        .is_some());
    assert_eq!(
        ai.war_breach_requirement(&game, 0, &game.cities[&objective]),
        Some(None)
    );
    let open = ai
        .choose_war_plan(&game, 0)
        .expect("four bodies and a one-turn breakthrough fit the rapid window");
    assert_eq!(open.assault_unit, crate::name!("pikeman"));

    game.cities.get_mut(&objective).unwrap().wall_hp = 100;
    game.players[0].techs.insert(crate::name!("engineering"));
    assert!(
        ai.choose_war_plan(&game, 0).is_none(),
        "a trainable but absent breach element is not a ready-force package"
    );
    game.spawn_test_unit("catapult", 0, home);
    let walled = ai
        .choose_war_plan(&game, 0)
        .expect("the same package qualifies once its compatible breach element exists");
    assert_eq!(walled.breach_unit, Some(crate::name!("catapult")));
    assert!(
        walled.estimated_research_turns
            + walled.estimated_production_turns
            + walled.estimated_march_turns
            <= game.standard_duration(RAPID_TIMED_WAR_LAUNCH_WINDOW)
    );
}

#[test]
fn timed_war_uses_unique_replacements_and_exact_upgrade_quotes() {
    let (mut game, _, _) = timed_war_fixture(940_002);
    game.players[0].civ = "Rome".to_string();
    game.players[0].techs.insert(crate::name!("iron_working"));
    let catalog = AdvancedAi::player_unit_catalog(&game, 0);
    assert!(catalog.contains(&crate::name!("legion")));
    assert!(!catalog.contains(&crate::name!("swordsman")));
    assert_eq!(
        AdvancedAi::war_direct_predecessor(&game, 0, crate::name!("man_at_arms")),
        Some(crate::name!("legion"))
    );
    assert!(
        game.unit_upgrade_price_to(0, crate::name!("legion"), crate::name!("man_at_arms"))
            .is_some(),
        "the reserve quotes the civilization's real direct edge"
    );
}

/// The one-pass catalog names, for every civilization, exactly the units the
/// per-unit `Game::player_unit_replacement` walk named.
#[test]
fn the_unit_catalog_matches_the_per_unit_replacement_walk_for_every_civilization() {
    let (mut game, _, _) = timed_war_fixture(940_004);
    let civs: Vec<String> = game.rules.civs.keys().map(|civ| civ.to_string()).collect();
    assert!(civs.len() > 10, "the rules carry the civilization roster");
    let mut replaced = 0;
    for civ in civs {
        game.players[0].civ = civ.clone();
        let walked: Vec<Name> = game
            .rules
            .units
            .iter()
            .filter(|(_, spec)| {
                spec.buildable && spec.unique_to.as_deref().is_none_or(|owner| owner == civ)
            })
            .map(|(name, _)| game.player_unit_replacement(0, *name))
            .filter(|actual| {
                let spec = &game.rules.units[*actual];
                spec.buildable && spec.unique_to.as_deref().is_none_or(|owner| owner == civ)
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let catalog = AdvancedAi::player_unit_catalog(&game, 0);
        replaced += usize::from(
            catalog
                .iter()
                .any(|unit| game.rules.units[*unit].replaces.is_some()),
        );
        assert_eq!(catalog, walked, "{civ}");
    }
    assert!(
        replaced > 10,
        "most civilizations field a unique replacement"
    );
}

#[test]
fn timed_war_rejects_an_unavailable_strategic_resource() {
    let (mut game, _, _) = timed_war_fixture(940_003);
    game.players[0].strategic_resources.clear();
    assert!(!AdvancedAi::war_resource_feasible(
        &game,
        0,
        crate::name!("swordsman"),
        20
    ));
    game.players[0]
        .strategic_resources
        .insert(crate::name!("iron"), 80.0);
    assert!(AdvancedAi::war_resource_feasible(
        &game,
        0,
        crate::name!("swordsman"),
        20
    ));
}

/// `nuclear-goal-needs-a-launcher`: on a seat that cannot launch a device the
/// goal is gone, and with it the value of every nuclear project.
#[test]
fn a_nuclear_goal_needs_a_platform_that_reaches_the_target() {
    let (mut game, launch_city, _, _, _) = nuclear_war_fixture();
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(ai
        .nuclear_war_goal(&game, 0, GrandStrategy::Conquest, Some(1))
        .is_some());
    let silo = game.cities[&launch_city].pos;
    game.map.tiles.get_mut(&silo).unwrap().improvement = None;
    assert!(
        ai.nuclear_war_goal(&game, 0, GrandStrategy::Conquest, Some(1))
            .is_none(),
        "a device nothing can deliver is not worth planning"
    );
}

#[test]
fn the_nuclear_goal_needs_a_launcher_under_the_gene() {
    let (game, _, _, spaceport_city, _) = nuclear_war_fixture();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(spaceport_city),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(
        ai.nuclear_war_goal(&game, 0, GrandStrategy::Conquest, Some(1))
            .is_some(),
        "off: the revealed launch site is a goal"
    );
    assert!(ai.nuclear_project_value(&game, 0, "manhattan_project", &plan) > 0.0);
    ai.enable_nuclear_goal_needs_a_launcher();
    assert!(ai
        .nuclear_war_goal(&game, 0, GrandStrategy::Conquest, Some(1))
        .is_none());
    for project in [
        "manhattan_project",
        "operation_ivy",
        "build_nuclear_device",
        "build_thermonuclear_device",
    ] {
        assert!(
            ai.nuclear_project_value(&game, 0, project, &plan) <= -10_000.0,
            "{project}"
        );
    }
}

/// What the shipped scorer pays for fifteen turns of siege, and what the
/// treatment pays.
///
/// `defenses` credits a broken city `0.12` per point of health, so the
/// whole health bar is worth 21 — and `core_distance * 7.0` plus
/// `military_distance * 5.0` swing about 60 over five tiles. That is the
/// arithmetic behind live run `civvis-20260808T142724Z` leaving Biruta at
/// 25 hp with its walls down on t90 and re-aiming at Ziz on t91.
#[test]
fn siege_commitment_prices_the_health_already_stripped_from_a_breach() {
    let mut game = Game::new_full(2, 30, 18, 7_111, 300, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    let enemy = game.player_city_ids(1)[0];
    let enemy_position = game.cities[&enemy].pos;
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    // A besieger of ours stands on the ring, so the city is both observed
    // and inside `SIEGE_COMMITMENT_REACH`.
    let besieger = game
        .nbrs(enemy_position)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        })
        .expect("the enemy capital has a land approach");
    game.spawn_test_unit("warrior", 0, besieger);

    let value = |ai: &AdvancedAi, game: &Game| {
        ai.campaign_city_value(game, 0, &game.cities[&enemy], GrandStrategy::Conquest)
    };
    let breach_to = |game: &mut Game, hp: i32| {
        let city = game.cities.get_mut(&enemy).unwrap();
        city.hp = hp;
        city.wall_hp = 0;
        city.buildings.retain(|building| {
            !matches!(
                building.as_str(),
                "walls" | "medieval_walls" | "renaissance_walls"
            )
        });
    };

    let mut shipped = AdvancedAi::targeting(VictoryTarget::Domination);
    let mut live = AdvancedAi::targeting(VictoryTarget::Domination);
    live.enable_siege_commitment();

    breach_to(&mut game, CITY_MAX_HP);
    shipped.belief.observe(&game, 0);
    live.belief.observe(&game, 0);
    let shipped_whole = value(&shipped, &game);
    let live_whole = value(&live, &game);
    assert_eq!(
        shipped_whole, live_whole,
        "a city at full health has had nothing stripped, so the treatment is inert"
    );

    breach_to(&mut game, 25);
    shipped.belief.observe(&game, 0);
    live.belief.observe(&game, 0);
    let stripped = (CITY_MAX_HP - 25) as f64;
    let shipped_broken = value(&shipped, &game);
    let live_broken = value(&live, &game);
    // The shipped credit for breaking a city almost open. `core_distance`
    // alone swings 7.0 a tile, so this loses to a fresh objective three
    // tiles nearer — which is exactly what the live run did.
    assert!(
        shipped_whole - shipped_broken < 40.0,
        "the shipped scorer prices the whole health bar at under 40 points \
             (measured {}), less than the distance terms swing",
        shipped_whole - shipped_broken
    );
    // Differencing the two controllers over the identical board isolates
    // the new term from everything else that moves with a city's health.
    assert!(
        ((shipped_broken - live_broken) - stripped * SIEGE_COMMITMENT_PER_HP).abs() < 1e-6,
        "the treatment is worth exactly {SIEGE_COMMITMENT_PER_HP} per stripped point — {} \
             here — and nothing else changes",
        stripped * SIEGE_COMMITMENT_PER_HP
    );
}

/// Walls standing means capture is not close, and damage nobody of ours is
/// near is not our investment. Both keep the term at zero, so it stays
/// inert outside the case it was measured on.
#[test]
fn siege_commitment_ignores_an_unbreached_city_and_a_siege_out_of_reach() {
    let mut game = Game::new_full(2, 30, 18, 7_111, 300, 0, false);
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
    }
    let enemy = game.player_city_ids(1)[0];
    let enemy_position = game.cities[&enemy].pos;
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    let mut shipped = AdvancedAi::targeting(VictoryTarget::Domination);
    let mut live = AdvancedAi::targeting(VictoryTarget::Domination);
    live.enable_siege_commitment();

    // Differencing the two controllers over one board isolates the term.
    let gap = |shipped: &AdvancedAi, live: &AdvancedAi, game: &Game| {
        shipped.campaign_city_value(game, 0, &game.cities[&enemy], GrandStrategy::Conquest)
            - live.campaign_city_value(game, 0, &game.cities[&enemy], GrandStrategy::Conquest)
    };
    let set = |game: &mut Game, hp: i32, wall_hp: i32| {
        let city = game.cities.get_mut(&enemy).unwrap();
        city.hp = hp;
        city.wall_hp = wall_hp;
    };

    // Beaten down to 25 hp but the walls still stand: capture is not close,
    // so the term stays out of it.
    let ring = game
        .nbrs(enemy_position)
        .into_iter()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        })
        .expect("the enemy capital has a land approach");
    let besieger = game.spawn_test_unit("warrior", 0, ring);
    set(&mut game, 25, 100);
    shipped.belief.observe(&game, 0);
    live.belief.observe(&game, 0);
    assert_eq!(
        gap(&shipped, &live, &game),
        0.0,
        "with the walls up there is no breach to finish"
    );

    // Breached and beaten down, with a besieger on the ring: full credit.
    set(&mut game, 25, 0);
    shipped.belief.observe(&game, 0);
    live.belief.observe(&game, 0);
    assert!(
        (gap(&shipped, &live, &game) - 175.0 * SIEGE_COMMITMENT_PER_HP).abs() < 1e-6,
        "a breach our army is standing on is worth the health already stripped"
    );

    // Same breach, but our nearest soldier is beyond the reach horizon.
    game.remove_unit(besieger);
    let far = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| {
            game.map
                .get(*position)
                .is_some_and(|tile| game.rules.is_passable(tile) && !game.rules.is_water(tile))
                && game.wdist(enemy_position, *position) > SIEGE_COMMITMENT_REACH
        })
        .expect("the test map has ground beyond the reach horizon");
    game.spawn_test_unit("warrior", 0, far);
    shipped.belief.observe(&game, 0);
    live.belief.observe(&game, 0);
    assert_eq!(
        gap(&shipped, &live, &game),
        0.0,
        "damage dealt where no soldier of ours can finish it is not our siege"
    );
}
