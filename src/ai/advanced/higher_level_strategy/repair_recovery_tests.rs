use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn board() -> (Game, AdvancedAi, StrategicPlan, u32, Vec<(i32, i32)>) {
    let mut g = Game::new(2, 32, 22, 914_357_700, 250, 0);
    g.game_speed = GameSpeed::Online;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.improvement = None;
        tile.district = None;
    }
    let cities = [(5, 5), (19, 5)].map(|pos| g.found_city_for(0, pos, None));
    for cid in cities {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.production = 0.0;
        for pos in city.owned_tiles.clone() {
            if pos != city.pos {
                g.map.tiles.get_mut(&pos).unwrap().improvement = Some(crate::name!("farm"));
            }
        }
    }
    let cid = cities[0];
    g.cities.get_mut(&cities[1]).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("warrior"),
    }];
    g.spawn_test_unit("settler", 0, (8, 8));
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            production: 100.0,
            ..Default::default()
        },
    );
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 10.0;
    g.current = 0;
    g.turn = 40;
    let jobs = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != g.cities[&cid].pos)
        .take(3)
        .collect::<Vec<_>>();
    assert_eq!(jobs.len(), 3);
    let mut ai = AdvancedAi::new();
    ai.enable_builder_workforce_recovery_3();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 2,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, cid, jobs)
}

fn pillage(g: &mut Game, jobs: &[(i32, i32)]) {
    for pos in jobs {
        g.map.tiles.get_mut(pos).unwrap().pillaged = true;
        // Isolate repair work even when rebuilding the same improvement is
        // otherwise a legal option on a pillaged tile.
        Arc::make_mut(&mut g.blocked_improvement_sites).insert(*pos);
    }
}

#[test]
fn builder_repair_v3_reserves_one_replacement_for_three_repairs() {
    let (mut g, ai, plan, cid, jobs) = board();
    pillage(&mut g, &jobs);
    let mut old = ai.clone();
    old.enable_builder_workforce_recovery_2();
    assert!(old.higher_level_investment_target(&g, 0, &plan).is_none());
    let (chosen, item, debt) = ai.higher_level_investment_target(&g, 0, &plan).unwrap();
    assert_eq!(chosen, cid);
    assert_eq!(debt, Debt::Builder);
    assert_eq!(
        item,
        Item::Unit {
            unit: crate::name!("builder")
        }
    );
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue, vec![item]);
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
}

#[test]
fn builder_repair_v3_combines_jobs_and_retains_the_three_tile_threshold() {
    let (mut g, ai, plan, _, jobs) = board();
    pillage(&mut g, &jobs[..2]);
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    g.map.tiles.get_mut(&jobs[2]).unwrap().improvement = None;
    assert!(!g.valid_improvements(0, jobs[2]).is_empty());
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_some());
}

#[test]
fn builder_repair_v3_still_accepts_three_new_improvements() {
    let (mut g, ai, plan, cid, jobs) = board();
    for pos in jobs {
        g.map.tiles.get_mut(&pos).unwrap().improvement = None;
    }
    let mut old = ai.clone();
    old.enable_builder_workforce_recovery_2();
    let old_target = old.higher_level_investment_target(&g, 0, &plan).unwrap();
    assert_eq!(
        ai.higher_level_investment_target(&g, 0, &plan),
        Some(old_target)
    );
    assert!(g.cities[&cid].queue.is_empty());
}

#[test]
fn builder_repair_v3_keeps_existing_builder_and_military_reservations() {
    let (mut g, ai, mut plan, cid, jobs) = board();
    pillage(&mut g, &jobs);
    let unit = g.spawn_test_unit("builder", 0, g.cities[&cid].pos);
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    g.remove_unit(unit);
    plan.strategy = GrandStrategy::Recovery;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    plan.strategy = GrandStrategy::Expansion;
    plan.threatened_city = Some(cid);
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    plan.threatened_city = None;
    g.cities.get_mut(&cid).unwrap().last_attacked = g.turn;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    g.cities.get_mut(&cid).unwrap().last_attacked = 0;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_some());
    g.turn = 249;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
}

#[test]
fn builder_repair_v3_ignores_repairs_no_longer_in_the_launch_city() {
    let (mut g, ai, plan, _, jobs) = board();
    pillage(&mut g, &jobs);
    g.map.tiles.get_mut(&jobs[0]).unwrap().owner_city = None;
    assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
}

#[test]
fn builder_repair_v3_is_exclusive_and_default_off() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "builder-workforce-recovery-3",
        |ai| ai.builder_workforce_recovery_3,
    );
    let mut ai = AdvancedAi::new();
    for old in ["builder-workforce-recovery", "builder-workforce-recovery-2"] {
        let original = super::super::genes::gene(old).unwrap();
        (original.enable)(&mut ai);
        ai.enable_builder_workforce_recovery_3();
        assert!(!ai.builder_workforce_recovery && !ai.builder_workforce_recovery_2);
        assert!(ai.builder_workforce_recovery_3);
        (original.enable)(&mut ai);
        assert!(!ai.builder_workforce_recovery_3);
    }
    ai.enable_builder_workforce_recovery_3();
    ai.disable_builder_workforce_recovery_3();
    assert!(!ai.builder_workforce_recovery_3);
}

#[test]
fn builder_repair_v3_live_identity_matches_the_selected_flags() {
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    ai.apply_gene_ledger_with_forced_live(&["builder-workforce-recovery-3"]);
    let tags = super::super::gene_ledger::deployment_treatments_with_forced_live(&[
        "builder-workforce-recovery-3",
    ]);
    for (tag, enabled) in [
        ("builder-workforce-recovery", ai.builder_workforce_recovery),
        (
            "builder-workforce-recovery-2",
            ai.builder_workforce_recovery_2,
        ),
        (
            "builder-workforce-recovery-3",
            ai.builder_workforce_recovery_3,
        ),
    ] {
        assert_eq!(tags.contains(&tag), enabled, "{tag}");
    }
    assert!(ai.builder_workforce_recovery_3);
}
