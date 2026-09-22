use super::*;
use crate::rules::Yields;
use std::sync::Arc;

fn fixture() -> (Game, u32, AdvancedAi, StrategicPlan, Item) {
    let mut g = Game::new_full(2, 32, 22, 936_035, 250, 0, false);
    for id in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(id);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    let cid = g.found_city_for(0, (8, 10), None);
    g.found_city_for(1, (24, 10), None);
    g.record_contact(0, 1);
    g.turn = 52;
    g.current = 0;
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 20.0;
    g.players[0]
        .techs
        .extend(["pottery", "writing", "education"].map(crate::name::Name::new));
    g.cities.get_mut(&cid).unwrap().pop = 8;
    let pos = g.district_sites(cid, crate::name!("campus"))[0];
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("campus"), pos);
    g.map.tiles.get_mut(&pos).unwrap().district = Some(crate::name!("campus"));
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        Yields {
            science: 100.0,
            ..Default::default()
        },
    );
    let mut ai = AdvancedAi::new();
    ai.retarget(VictoryTarget::Domination);
    ai.enable_research_building_catchup();
    ai.preempt_margin = 1.25;
    // Keep this a research reservation, without another enabled debt choosing first.
    ai.expansion_best_idle_city = false;
    ai.expansion_best_idle_city_2 = false;
    ai.expansion_scales_with_difficulty = false;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: g.turn,
        rush: false,
    };
    let item = Item::Building {
        building: crate::name!("library"),
    };
    (g, cid, ai, plan, item)
}

#[test]
fn the_catchup_library_survives_the_same_decisions_production_review() {
    let (mut g, cid, mut ai, plan, item) = fixture();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    assert_eq!(g.item_invested_production(cid, &item), 0.0);
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn a_fresh_university_also_survives_while_domination_research_is_behind() {
    let (mut g, cid, mut ai, plan, _) = fixture();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    let item = Item::Building {
        building: crate::name!("university"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: item.clone(),
        },
    )
    .unwrap();
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn unrelated_buildings_keep_the_ordinary_production_review() {
    let (mut g, cid, mut ai, plan, _) = fixture();
    let item = Item::Building {
        building: crate::name!("granary"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: item.clone(),
        },
    )
    .unwrap();
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn local_defense_can_still_replace_a_library() {
    let (mut g, cid, mut ai, plan, item) = fixture();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    ai.enable_garrison_under_fire();
    g.players[0].gold = 0.0;
    g.cities.get_mut(&cid).unwrap().hp = 100;
    ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, None);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
    assert!(matches!(
        g.cities[&cid].queue.first(),
        Some(Item::Unit { .. })
    ));
}

#[test]
fn parity_unmet_or_dead_rivals_do_not_create_a_research_commitment() {
    for control in ["parity", "unmet", "dead"] {
        let (mut g, cid, mut ai, plan, item) = fixture();
        match control {
            "parity" => {
                Arc::make_mut(&mut g.observed_yield_adjustments).clear();
            }
            "unmet" => {
                g.players[0].met.clear();
            }
            "dead" => {
                g.players[1].alive = false;
            }
            _ => unreachable!(),
        }
        assert!(
            !ai.domination_research_catchup_needed(&g, 0, &plan),
            "{control}"
        );
        g.apply(
            0,
            &Action::Produce {
                city: cid,
                item: item.clone(),
            },
        )
        .unwrap();
        ai.advanced_production(&mut g, 0, &plan, false);
        assert_ne!(g.cities[&cid].queue.first(), Some(&item), "{control}");
    }
}

#[test]
fn disabled_catchup_and_other_lanes_keep_the_existing_review() {
    let (g, _, ai, plan, _) = fixture();
    let mut disabled = ai.clone();
    disabled.research_building_catchup = false;
    assert!(!disabled.domination_research_catchup_needed(&g, 0, &plan));
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Culture,
        VictoryTarget::Score,
    ] {
        let mut other = ai.clone();
        other.retarget(target);
        assert!(!other.domination_research_catchup_needed(&g, 0, &plan));
    }
    let mut recovery = plan.clone();
    recovery.strategy = GrandStrategy::Recovery;
    assert!(!ai.domination_research_catchup_needed(&g, 0, &recovery));
}

#[test]
fn an_illegal_library_is_not_protected() {
    let (mut g, cid, mut ai, plan, item) = fixture();
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: item.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&cid).unwrap().districts.clear();
    assert!(!AdvancedAi::production_commitment_is_legal(
        &g, 0, cid, &item
    ));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn economic_recovery_does_not_gain_a_new_research_override() {
    let (mut g, cid, mut ai, plan, item) = fixture();
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: item.clone(),
        },
    )
    .unwrap();
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -20.0;
    let mut control_g = g.clone();
    let mut control_ai = ai.clone();
    control_ai.research_building_catchup = false;
    assert!(ai.live_war_economy_requires_recovery(&g, 0, &ai.counts(&g, 0)));
    ai.advanced_production(&mut g, 0, &plan, false);
    control_ai.advanced_production(&mut control_g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue, control_g.cities[&cid].queue);
}

#[test]
fn a_threatened_city_does_not_gain_the_fresh_research_commitment() {
    let (mut g, cid, mut ai, mut plan, item) = fixture();
    plan.threatened_city = Some(cid);
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: item.clone(),
        },
    )
    .unwrap();
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_ne!(g.cities[&cid].queue.first(), Some(&item));
}

fn placed_campus_fixture() -> (Game, u32, AdvancedAi, StrategicPlan, Item) {
    let (mut g, cid, ai, plan, _) = fixture();
    let district = crate::name!("campus");
    let pos = *g.cities[&cid].districts.get(district).unwrap();
    g.cities.get_mut(&cid).unwrap().districts.clear();
    let tile = g.map.tiles.get_mut(&pos).unwrap();
    tile.district = None;
    tile.district_foundation = Some(crate::world::DistrictFoundation {
        district,
        cost: 70.0,
    });
    (g, cid, ai, plan, Item::District { district, pos })
}

#[test]
fn placed_campus_catchup_is_reserved_and_survives_production_review() {
    let (mut g, cid, mut ai, plan, item) = placed_campus_fixture();
    assert!(g.can_produce(0, cid, &item));
    assert!(ai.domination_research_catchup_needed(&g, 0, &plan));
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
}

#[test]
fn campus_catchup_requires_maturity_for_new_sites_and_a_domination_shortfall() {
    for case in [
        "small_new_site",
        "other_foundation",
        "other_lane",
        "caught_up",
    ] {
        let (mut g, cid, mut ai, plan, item) = placed_campus_fixture();
        let Item::District { pos, .. } = item else {
            unreachable!()
        };
        match case {
            "small_new_site" => {
                g.map.tiles.get_mut(&pos).unwrap().district_foundation = None;
                g.cities.get_mut(&cid).unwrap().pop = 3;
            }
            "other_foundation" => {
                g.map
                    .tiles
                    .get_mut(&pos)
                    .unwrap()
                    .district_foundation
                    .as_mut()
                    .unwrap()
                    .district = crate::name!("holy_site")
            }
            "other_lane" => ai.retarget(VictoryTarget::Science),
            "caught_up" => Arc::make_mut(&mut g.observed_yield_adjustments).clear(),
            _ => unreachable!(),
        }
        ai.reserve_higher_level_investment(&mut g, 0, &plan);
        assert_ne!(g.cities[&cid].queue.first(), Some(&item), "{case}");
    }
}

#[test]
fn campus_completion_keeps_emergency_and_active_queue_guards() {
    for case in ["threatened", "recovery", "busy"] {
        let (mut g, cid, ai, mut plan, item) = placed_campus_fixture();
        match case {
            "threatened" => plan.threatened_city = Some(cid),
            "recovery" => plan.strategy = GrandStrategy::Recovery,
            "busy" => g.cities.get_mut(&cid).unwrap().queue.push(Item::Unit {
                unit: crate::name!("builder"),
            }),
            _ => unreachable!(),
        }
        ai.reserve_higher_level_investment(&mut g, 0, &plan);
        assert_ne!(g.cities[&cid].queue.first(), Some(&item), "{case}");
    }
}

#[test]
fn queued_campus_does_not_block_a_library_in_a_completed_campus() {
    let (mut g, cid, ai, plan, library) = fixture();
    let other = g.found_city_for(0, (14, 10), None);
    g.cities.get_mut(&other).unwrap().pop = 4;
    let district = crate::name!("campus");
    let pos = g.district_sites(other, district)[0];
    g.apply(
        0,
        &Action::Produce {
            city: other,
            item: Item::District { district, pos },
        },
    )
    .unwrap();
    ai.reserve_higher_level_investment(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&library));
}
