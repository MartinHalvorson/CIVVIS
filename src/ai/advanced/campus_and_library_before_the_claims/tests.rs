use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::industrial_zones::tests::producer_empire;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

const TAG: &str = "campus-and-library-before-the-claims:";

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn library() -> Item {
    Item::Building {
        building: crate::name!("library"),
    }
}

fn head(g: &Game, cid: u32) -> Option<Item> {
    g.cities[&cid].queue.first().cloned()
}

fn calm_plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 9,
        assessed_turn: g.turn,
        rush: false,
    }
}

fn quote(g: &mut Game, cid: u32, item: &Item, turns: f64, cost: f64) {
    Arc::make_mut(&mut g.host_buildable)
        .entry(cid)
        .or_default()
        .insert(
            Game::production_block_key(item),
            crate::game::HostMenuEntry {
                cost: Some(cost),
                turns: Some(turns),
            },
        );
}

/// Run `claim` with a recording journal and return its thoughts' details.
fn journalled(
    ai: &mut AdvancedAi,
    g: &mut Game,
    claim: impl FnOnce(&AdvancedAi, &mut Game),
) -> Vec<String> {
    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(g.turn, 0);
    ai.attach_journal(journal.handle());
    claim(ai, g);
    journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.detail)
        .collect()
}

/// A model of the live Gran Colombia seat at Online t45, not a native
/// replay: three cities on flat grassland hills, the capital (5 citizens,
/// 11 Production) training a Builder with 5 paid and a five-turn first
/// Campus quoted there, the other two at one citizen. The gene is armed;
/// `near-done-work-finishes` is off so only this gene decides.
fn campus_board() -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let mut g = Game::new_full(2, 36, 20, 95_345, 250, 0, false);
    g.units.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
    }
    g.players[0].civ = "Gran Colombia".to_string();
    for tech in ["pottery", "writing", "mining", "masonry", "bronze_working"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let ids: Vec<_> = [(5, 5), (15, 5), (25, 5)]
        .into_iter()
        .map(|pos| g.found_city_for(0, pos, None))
        .collect();
    g.game_speed = GameSpeed::Online;
    g.turn = 45;
    for &id in &ids {
        let city = g.cities.get_mut(&id).unwrap();
        city.pop = 1;
        city.production = 0.0;
        city.queue = vec![Item::Building {
            building: crate::name!("walls"),
        }];
        let production = g.city_yields(id).production;
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            id,
            Yields {
                production: 11.0 - production,
                ..Default::default()
            },
        );
    }
    let cid = ids[0];
    let capital = g.cities.get_mut(&cid).unwrap();
    capital.pop = 5;
    capital.queue = vec![unit("builder")];
    capital.production = 5.0;
    let campus = BasicAi::first_campus_item(&g, 0, cid).unwrap();
    quote(&mut g, cid, &campus, 5.0, 56.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_campus_claims_the_free_slot_2();
    ai.disable_near_done_work_finishes();
    ai.enable_campus_and_library_before_the_claims();
    let plan = calm_plan(&g);
    assert!(ai
        .free_slot_campus_item(&g, 0, cid, &plan)
        .is_some_and(|(item, turns)| item == campus && turns == 5.0));
    (g, ai, cid, plan, campus)
}

/// The live Library gap as a model board: the six-city Online empire of
/// `producer_empire` (a standing Campus each, Production 40 down to 10)
/// with no Library built, every city two-thirds into nothing but a
/// Commercial Hub with 20 Production paid. The gene is armed;
/// `near-done-work-finishes` is off.
fn library_board() -> (Game, Vec<u32>, AdvancedAi, StrategicPlan) {
    let (mut g, ranked) = producer_empire("CAMPUSLIBRARYFIRST", 94_131, 1);
    for cid in &ranked {
        let hub = BasicAi::civ_district(&g, 0, "commercial_hub");
        let site = g
            .district_sites(*cid, hub)
            .into_iter()
            .min()
            .expect("a Hub site");
        let city = g.cities.get_mut(cid).unwrap();
        city.buildings.retain(|building| *building != "library");
        city.queue = vec![Item::District {
            district: hub,
            pos: site,
        }];
        city.production = 20.0;
        assert_eq!(
            BasicAi::campus_building_item(&g, 0, *cid),
            Some(library()),
            "fixture: the Library is buildable"
        );
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_campus_building_claims_the_queue();
    ai.disable_near_done_work_finishes();
    ai.enable_campus_and_library_before_the_claims();
    let plan = calm_plan(&g);
    (g, ranked, ai, plan)
}

fn hub_head(g: &Game, cid: u32) -> bool {
    matches!(head(g, cid), Some(Item::District { district, .. })
        if g.district_family(district) == "commercial_hub")
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("campus-and-library-before-the-claims", |ai| {
        ai.campus_and_library_before_the_claims
    });
}

#[test]
fn first_campus_goes_ahead_of_a_builder_head_only_with_the_gene() {
    let (g, mut ai, cid, plan, campus) = campus_board();

    let mut off = ai.clone();
    off.disable_campus_and_library_before_the_claims();
    let mut control = g.clone();
    off.claim_free_slot_campus(&mut control, 0, &plan);
    assert_eq!(
        head(&control, cid),
        Some(unit("builder")),
        "off: the Builder keeps the queue"
    );

    let mut on = g.clone();
    let details = journalled(&mut ai, &mut on, |ai, g| {
        ai.claim_free_slot_campus(g, 0, &plan)
    });
    assert_eq!(head(&on, cid), Some(campus.clone()));
    assert_eq!(on.item_invested_production(cid, &unit("builder")), 5.0);
    assert!(
        details.iter().any(|detail| detail.starts_with(TAG)
            && detail.contains("first Campus goes ahead of the Builder")),
        "{details:?}"
    );
    // The writers that would take it back leave it alone.
    assert!(ai.campus_and_library_holds(&on, 0, cid, &plan, &campus));
    assert!(!off.campus_and_library_holds(&on, 0, cid, &plan, &campus));
}

#[test]
fn a_settler_head_keeps_the_first_campus_waiting() {
    let (mut g, ai, cid, plan, _) = campus_board();
    g.cities.get_mut(&cid).unwrap().queue = vec![AdvancedAi::early_settler_item()];
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(head(&g, cid), Some(AdvancedAi::early_settler_item()));
}

#[test]
fn a_threatened_or_attacked_city_or_another_target_keeps_its_builder() {
    let (base, armed, cid, base_plan, _) = campus_board();
    for case in ["threat", "recent attack", "science target", "walls"] {
        let mut g = base.clone();
        let mut ai = armed.clone();
        let mut plan = base_plan.clone();
        let kept = match case {
            "threat" => {
                plan.threatened_city = Some(cid);
                unit("builder")
            }
            "recent attack" => {
                g.cities.get_mut(&cid).unwrap().last_attacked = g.turn - 1;
                unit("builder")
            }
            "science target" => {
                ai.victory_target = Some(VictoryTarget::Science);
                unit("builder")
            }
            "walls" => {
                let walls = Item::Building {
                    building: crate::name!("walls"),
                };
                g.cities.get_mut(&cid).unwrap().queue = vec![walls.clone()];
                walls
            }
            _ => unreachable!(),
        };
        ai.claim_free_slot_campus(&mut g, 0, &plan);
        assert_eq!(head(&g, cid), Some(kept), "{case}");
    }
}

#[test]
fn a_major_war_keeps_the_soldier_but_not_the_builder() {
    let (mut g, ai, cid, plan, campus) = campus_board();
    g.at_war.insert((0, 1));
    let mut soldier = g.clone();
    soldier.cities.get_mut(&cid).unwrap().queue = vec![unit("warrior")];
    ai.claim_free_slot_campus(&mut soldier, 0, &plan);
    assert_eq!(
        head(&soldier, cid),
        Some(unit("warrior")),
        "a rear city's soldier at war"
    );
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(head(&g, cid), Some(campus), "a rear city's Builder at war");
}

#[test]
fn every_library_goes_ahead_of_a_commercial_hub_only_with_the_gene() {
    let (g, ranked, mut ai, plan) = library_board();

    let mut off = ai.clone();
    off.disable_campus_and_library_before_the_claims();
    let mut control = g.clone();
    off.claim_campus_building(&mut control, 0, &plan);
    assert!(
        ranked.iter().all(|cid| hub_head(&control, *cid)),
        "off: the Hub with progress keeps every queue"
    );

    let mut on = g.clone();
    let details = journalled(&mut ai, &mut on, |ai, g| {
        ai.claim_campus_building(g, 0, &plan)
    });
    for cid in &ranked {
        assert_eq!(
            head(&on, *cid),
            Some(library()),
            "every city's Library, one turn"
        );
        assert!(ai.campus_and_library_holds(&on, 0, *cid, &plan, &library()));
    }
    let passed = details
        .iter()
        .filter(|detail| {
            detail.starts_with(TAG) && detail.contains("Library goes ahead of the Commercial Hub")
        })
        .count();
    assert_eq!(passed, ranked.len(), "{details:?}");
    let hub = g.cities[&ranked[0]].queue[0].clone();
    assert_eq!(on.item_invested_production(ranked[0], &hub), 20.0);
}

#[test]
fn the_gene_opens_the_library_claim_with_the_building_gene_off() {
    let (mut g, ranked, mut ai, plan) = library_board();
    ai.disable_campus_building_claims_the_queue();
    ai.claim_campus_building(&mut g, 0, &plan);
    assert!(ranked.iter().all(|cid| head(&g, *cid) == Some(library())));
    ai.disable_campus_and_library_before_the_claims();
    assert!(ai
        .campus_building_claim_item(&g, 0, ranked[0], &plan)
        .is_none());
}

#[test]
fn settlers_and_a_threatened_city_keep_their_queue_against_the_library() {
    let (mut g, ranked, ai, mut plan) = library_board();
    let threatened = ranked[0];
    let settling = ranked[1];
    plan.threatened_city = Some(threatened);
    let city = g.cities.get_mut(&settling).unwrap();
    city.queue = vec![AdvancedAi::early_settler_item()];
    city.production = 20.0;
    ai.claim_campus_building(&mut g, 0, &plan);
    assert!(
        hub_head(&g, threatened),
        "the threatened city keeps its Hub"
    );
    assert_eq!(head(&g, settling), Some(AdvancedAi::early_settler_item()));
    for cid in &ranked[2..] {
        assert_eq!(head(&g, *cid), Some(library()));
    }
}
