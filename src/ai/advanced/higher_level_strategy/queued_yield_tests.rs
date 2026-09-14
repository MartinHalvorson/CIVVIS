use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn add_district(g: &mut Game, cid: u32, kind: &str) {
    let pos = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| *pos != g.cities[&cid].pos && g.map.get(*pos).unwrap().district.is_none())
        .unwrap();
    g.map.tiles.get_mut(&pos).unwrap().district = Some(kind.into());
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(kind.into(), pos);
}

fn board(debt: Debt, gap: f64) -> (Game, AdvancedAi, StrategicPlan, [u32; 3], Item) {
    let mut g = Game::new(2, 32, 22, 914_357_000, 250, 0);
    g.game_speed = GameSpeed::Online;
    g.units.clear();
    g.cities.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
    }
    let cities = [(5, 5), (15, 5), (25, 5)].map(|pos| g.found_city_for(0, pos, None));
    for cid in cities {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue.clear();
        city.buildings.clear();
        city.production = 0.0;
    }
    let mut ai = AdvancedAi::new();
    let item = match debt {
        Debt::Culture => {
            ai.enable_culture_building_catchup_3();
            Item::Building {
                building: crate::name!("monument"),
            }
        }
        Debt::Research => {
            ai.enable_research_building_catchup_3();
            g.players[0].techs.insert(crate::name!("writing"));
            for cid in cities {
                add_district(&mut g, cid, "campus");
            }
            Item::Building {
                building: crate::name!("library"),
            }
        }
        _ => unreachable!(),
    };
    g.turn = 40;
    g.record_contact(0, 1);
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 10.0;
    for cid in &cities[1..] {
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                production: 100.0,
                ..Default::default()
            },
        );
    }
    let ours: f64 = cities
        .iter()
        .map(|cid| {
            let y = g.city_yields(*cid);
            if debt == Debt::Culture {
                y.culture
            } else {
                y.science
            }
        })
        .sum();
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        1,
        if debt == Debt::Culture {
            Yields {
                culture: (ours + gap) / 0.7,
                ..Default::default()
            }
        } else {
            Yields {
                science: (ours + gap) / 0.7,
                ..Default::default()
            }
        },
    );
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, cities, item)
}

fn queue_due_next_turn(g: &mut Game, cid: u32, item: &Item) {
    let cost = g.item_cost_for_city(0, cid, item);
    let city = g.cities.get_mut(&cid).unwrap();
    city.queue = vec![item.clone()];
    city.production = cost - 1.0;
}

#[test]
fn catchup_v3_adds_only_the_yield_missing_from_queued_investment() {
    for debt in [Debt::Culture, Debt::Research] {
        let (mut g, ai, plan, cities, item) = board(debt, 3.0);
        queue_due_next_turn(&mut g, cities[0], &item);
        let mut old = ai.clone();
        if debt == Debt::Culture {
            old.enable_culture_building_catchup_2();
        } else {
            old.enable_research_building_catchup_2();
        }
        assert!(old.higher_level_investment_target(&g, 0, &plan).is_none());
        let (chosen, planned, kind) = ai.higher_level_investment_target(&g, 0, &plan).unwrap();
        assert_eq!(kind, debt);
        assert_eq!(planned, item);
        assert!(cities[1..].contains(&chosen));
        ai.reserve_higher_level_investment(&mut g, 0, &plan);
        assert_eq!(g.cities[&chosen].queue, vec![item.clone()]);
        assert_eq!(g.cities[&cities[0]].queue, vec![item]);
        assert!(
            ai.higher_level_investment_target(&g, 0, &plan).is_none(),
            "the third city's queue stays free once incoming yield covers the gap"
        );
    }
}

#[test]
fn catchup_v3_waits_for_a_nearly_complete_sufficient_answer() {
    for debt in [Debt::Culture, Debt::Research] {
        let (mut g, ai, plan, cities, item) = board(debt, 1.0);
        queue_due_next_turn(&mut g, cities[0], &item);
        assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    }
}

#[test]
fn catchup_v3_does_not_wait_for_an_answer_due_after_the_useful_window() {
    for debt in [Debt::Culture, Debt::Research] {
        let (mut g, ai, plan, cities, item) = board(debt, 1.0);
        // Founding and civilization bonuses can otherwise make the first
        // city fast enough to service this small deficit. Fix its observed
        // production at one and leave the two developed cities fast.
        let production = g.city_yields(cities[0]).production;
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            cities[0],
            Yields {
                production: 1.0 - production,
                ..Default::default()
            },
        );
        g.cities.get_mut(&cities[0]).unwrap().queue = vec![item.clone()];
        let slow = ai.production_build_turns(&g, 0, cities[0], &item).ceil();
        let fast = ai.production_build_turns(&g, 0, cities[1], &item).ceil();
        assert!(
            slow > fast + g.standard_duration(20) as f64,
            "fixture must expose a slow build"
        );
        let (chosen, _, _) = ai.higher_level_investment_target(&g, 0, &plan).unwrap();
        assert_ne!(chosen, cities[0]);
    }
}

#[test]
fn catchup_v3_does_not_credit_a_building_behind_unrelated_work() {
    for debt in [Debt::Culture, Debt::Research] {
        let (mut g, ai, plan, cities, item) = board(debt, 1.0);
        g.cities.get_mut(&cities[0]).unwrap().queue = vec![
            Item::Unit {
                unit: crate::name!("warrior"),
            },
            item,
        ];
        assert!(ai.higher_level_investment_target(&g, 0, &plan).is_some());
    }
}

#[test]
fn catchup_v3_preserves_recovery_threat_and_spaceport_reservations() {
    for debt in [Debt::Culture, Debt::Research] {
        let (mut g, ai, mut plan, cities, item) = board(debt, 100.0);
        g.cities.get_mut(&cities[0]).unwrap().queue = vec![item];
        plan.strategy = GrandStrategy::Recovery;
        assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
        plan.strategy = GrandStrategy::Expansion;
        g.cities.get_mut(&cities[1]).unwrap().last_attacked = g.turn;
        plan.threatened_city = Some(cities[2]);
        assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
        g.cities.get_mut(&cities[1]).unwrap().last_attacked = 0;
        plan.threatened_city = None;
        if debt == Debt::Research {
            for cid in &cities[1..] {
                add_district(&mut g, *cid, "spaceport");
            }
            assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
        }
        g.turn = 249;
        assert!(ai.higher_level_investment_target(&g, 0, &plan).is_none());
    }
}

#[test]
fn catchup_v3_versions_are_exclusive_and_default_off() {
    use super::super::test_support::opt_in_off_in_both_controllers as check;
    check("culture-building-catchup-3", |ai| {
        ai.culture_building_catchup_3
    });
    check("research-building-catchup-3", |ai| {
        ai.research_building_catchup_3
    });
    let mut ai = AdvancedAi::new();
    for debt in [Debt::Culture, Debt::Research] {
        let tags = if debt == Debt::Culture {
            [
                "culture-building-catchup",
                "culture-building-catchup-2",
                "culture-building-catchup-3",
            ]
        } else {
            [
                "research-building-catchup",
                "research-building-catchup-2",
                "research-building-catchup-3",
            ]
        };
        for first in tags {
            for second in tags {
                for tag in [first, second] {
                    let gene = super::super::genes::GENES
                        .iter()
                        .find(|g| g.tag == tag)
                        .unwrap();
                    (gene.enable)(&mut ai);
                }
                assert_eq!(debt.tag(&ai), second);
                assert_eq!(debt.prices_queued_yield(&ai), second.ends_with("-3"));
            }
        }
    }
}

#[test]
fn catchup_v3_live_identity_matches_the_active_flags() {
    for forced in ["culture-building-catchup-3", "research-building-catchup-3"] {
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        ai.apply_gene_ledger_with_forced_live(&[forced]);
        let tags = super::super::gene_ledger::deployment_treatments_with_forced_live(&[forced]);
        assert!(tags.contains(&forced));
        for (tag, enabled) in [
            ("culture-building-catchup", ai.culture_building_catchup),
            ("culture-building-catchup-2", ai.culture_building_catchup_2),
            ("culture-building-catchup-3", ai.culture_building_catchup_3),
            ("research-building-catchup", ai.research_building_catchup),
            (
                "research-building-catchup-2",
                ai.research_building_catchup_2,
            ),
            (
                "research-building-catchup-3",
                ai.research_building_catchup_3,
            ),
        ] {
            assert_eq!(tags.contains(&tag), enabled, "{forced}: {tag}");
        }
    }
}
