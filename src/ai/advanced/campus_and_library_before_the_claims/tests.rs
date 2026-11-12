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

fn granary() -> Item {
    Item::Building {
        building: crate::name!("granary"),
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
/// 11 Production) building a Granary with 5 paid and a five-turn first
/// Campus quoted there, the other two at one citizen. The gene is armed;
/// `near-done-work-finishes` is off so only this gene decides.
fn campus_board() -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    campus_board_with(3)
}

/// [`campus_board`] with `cities` founded cities (three to six).
fn campus_board_with(cities: usize) -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
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
    let ids: Vec<_> = [(5, 5), (15, 5), (25, 5), (15, 14), (5, 14), (25, 14)]
        .into_iter()
        .take(cities)
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
    capital.queue = vec![granary()];
    capital.production = 5.0;
    let campus = BasicAi::first_campus_item(&g, 0, cid).unwrap();
    quote(&mut g, cid, &campus, 5.0, 56.0);
    // The host's quote: a Granary is 65 Production at Online.
    quote(&mut g, cid, &granary(), 6.0, 65.0);
    assert!(
        g.item_cost_for_city(0, cid, &granary()) - 5.0 > 2.0 * g.city_yields(cid).production,
        "fixture: the Granary is turns from done"
    );
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
/// `producer_empire` (a standing Campus each) with no Library built, every
/// city making 10 Production and building a Commercial Hub with 2 paid, so
/// the Hub is several turns from done. The gene is armed;
/// `near-done-work-finishes` is off.
fn library_board() -> (Game, Vec<u32>, AdvancedAi, StrategicPlan) {
    let (mut g, ranked) = producer_empire("CAMPUSLIBRARYFIRST", 94_131, 1);
    for cid in &ranked {
        let hub = BasicAi::civ_district(&g, 0, "commercial_hub");
        // Any free tile of the city: the head's progress is what is tested,
        // not where the Hub stands.
        let center = g.cities[cid].pos;
        let site = g.cities[cid]
            .owned_tiles
            .iter()
            .copied()
            .filter(|pos| *pos != center && g.map.tiles[pos].district.is_none())
            .min()
            .expect("a free tile");
        let city = g.cities.get_mut(cid).unwrap();
        city.buildings.retain(|building| *building != "library");
        city.queue = vec![Item::District {
            district: hub,
            pos: site,
        }];
        city.production = 2.0;
        let hub = city.queue[0].clone();
        let adjustment = Arc::make_mut(&mut g.observed_city_yield_adjustments)
            .remove(cid)
            .unwrap_or_default();
        let base = g.city_yields(*cid).production;
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                production: 10.0 - base,
                ..adjustment
            },
        );
        let left = g.item_cost_for_city(0, *cid, &hub) - 2.0;
        let rate = g.city_yields(*cid).production;
        assert!(
            left > 2.0 * rate,
            "fixture: the Hub is {left} from done at {rate} a turn"
        );
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
fn first_campus_goes_ahead_of_a_granary_head_only_with_the_gene() {
    let (g, mut ai, cid, plan, campus) = campus_board();

    let mut off = ai.clone();
    off.disable_campus_and_library_before_the_claims();
    let mut control = g.clone();
    off.claim_free_slot_campus(&mut control, 0, &plan);
    assert_eq!(
        head(&control, cid),
        Some(granary()),
        "off: the Granary keeps the queue"
    );

    let mut on = g.clone();
    let details = journalled(&mut ai, &mut on, |ai, g| {
        ai.claim_free_slot_campus(g, 0, &plan)
    });
    assert_eq!(head(&on, cid), Some(campus.clone()));
    assert_eq!(on.item_invested_production(cid, &granary()), 5.0);
    assert!(
        details.iter().any(|detail| detail.starts_with(TAG)
            && detail.contains("first Campus goes ahead of the Granary")),
        "{details:?}"
    );
    // The writers that would take it back leave it alone.
    assert!(ai.campus_and_library_holds(&on, 0, cid, &plan, &campus));
    assert!(!off.campus_and_library_holds(&on, 0, cid, &plan, &campus));
}

#[test]
fn a_builder_head_keeps_its_queue_against_the_first_campus() {
    let (mut g, mut ai, cid, plan, _) = campus_board();
    g.cities.get_mut(&cid).unwrap().queue = vec![unit("builder")];
    let details = journalled(&mut ai, &mut g, |ai, g| {
        ai.claim_free_slot_campus(g, 0, &plan)
    });
    assert_eq!(head(&g, cid), Some(unit("builder")));
    assert!(
        !details.iter().any(|detail| detail.starts_with(TAG)),
        "{details:?}"
    );
}

#[test]
fn a_settler_head_keeps_the_first_campus_waiting() {
    let (mut g, ai, cid, plan, _) = campus_board();
    g.cities.get_mut(&cid).unwrap().queue = vec![AdvancedAi::early_settler_item()];
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(head(&g, cid), Some(AdvancedAi::early_settler_item()));
}

#[test]
fn a_threatened_or_attacked_city_or_another_target_keeps_its_granary() {
    let (base, armed, cid, base_plan, _) = campus_board();
    for case in ["threat", "recent attack", "science target", "walls"] {
        let mut g = base.clone();
        let mut ai = armed.clone();
        let mut plan = base_plan.clone();
        let kept = match case {
            "threat" => {
                plan.threatened_city = Some(cid);
                granary()
            }
            "recent attack" => {
                g.cities.get_mut(&cid).unwrap().last_attacked = g.turn - 1;
                granary()
            }
            "science target" => {
                ai.victory_target = Some(VictoryTarget::Science);
                granary()
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
fn a_major_war_keeps_the_soldier_but_not_the_granary() {
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
    assert_eq!(head(&g, cid), Some(campus), "a rear city's Granary at war");
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
    assert_eq!(on.item_invested_production(ranked[0], &hub), 2.0);
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
fn a_half_built_settler_and_a_threatened_city_keep_their_queue_against_the_library() {
    let (mut g, ranked, mut ai, mut plan) = library_board();
    let threatened = ranked[0];
    let half_built = ranked[1];
    let fresh = ranked[2];
    plan.threatened_city = Some(threatened);
    let settler = AdvancedAi::early_settler_item();
    let cost = g.item_cost_for_city(0, half_built, &settler);
    let city = g.cities.get_mut(&half_built).unwrap();
    city.queue = vec![settler.clone()];
    city.production = (0.6 * cost).floor();
    let city = g.cities.get_mut(&fresh).unwrap();
    city.queue = vec![settler.clone()];
    city.production = 0.0;
    let details = journalled(&mut ai, &mut g, |ai, g| {
        ai.claim_campus_building(g, 0, &plan)
    });
    assert!(
        hub_head(&g, threatened),
        "the threatened city keeps its Hub"
    );
    assert_eq!(head(&g, half_built), Some(settler.clone()), "half built");
    assert_eq!(
        head(&g, fresh),
        Some(library()),
        "six cities: a fresh Settler waits"
    );
    assert!(
        details.iter().any(|detail| detail.starts_with(TAG)
            && detail.contains("Library goes ahead of the Settler (0 of")),
        "{details:?}"
    );
    for cid in &ranked[3..] {
        assert_eq!(head(&g, *cid), Some(library()));
    }
    // The Settler claims leave the claimed Library alone from four cities.
    assert!(ai.science_campus_holds_repeat_settler(&g, 0, fresh, &plan));
}

/// [`campus_board_with`]'s capital training the early floor's Settler, `paid`
/// of 85 Production in, with `no-city-limit` armed as on the live seat (the
/// floor reads the plan's nine cities) and `first-campus-before-the-settler`
/// off.
fn floor_settler_board(cities: usize, paid: f64) -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let (mut g, mut ai, cid, plan, campus) = campus_board_with(cities);
    let settler = AdvancedAi::early_settler_item();
    quote(&mut g, cid, &settler, 8.0, 85.0);
    let capital = g.cities.get_mut(&cid).unwrap();
    capital.queue = vec![settler];
    capital.production = paid;
    ai.enable_early_settler_floor();
    ai.enable_no_city_limit();
    assert!(ai.early_settler_floor_holds(&g, 0, cid, &plan));
    (g, ai, cid, plan, campus)
}

#[test]
fn a_six_city_empire_puts_its_first_campus_ahead_of_a_fresh_floor_settler() {
    // No progress, and 20 of 85 paid (under half): the Settler keeps it.
    for paid in [0.0, 20.0] {
        let (g, mut ai, cid, plan, campus) = floor_settler_board(6, paid);
        let settler = AdvancedAi::early_settler_item();

        let mut off = ai.clone();
        off.disable_campus_and_library_before_the_claims();
        let mut control = g.clone();
        off.claim_free_slot_campus(&mut control, 0, &plan);
        assert_eq!(
            head(&control, cid),
            Some(settler.clone()),
            "off, {paid} paid"
        );

        let mut on = g.clone();
        let details = journalled(&mut ai, &mut on, |ai, g| {
            ai.claim_free_slot_campus(g, 0, &plan)
        });
        assert_eq!(head(&on, cid), Some(campus), "{paid} paid");
        assert_eq!(on.item_invested_production(cid, &settler), paid);
        let line = format!("first Campus goes ahead of the Settler ({paid:.0} of 85");
        assert!(
            details
                .iter()
                .any(|detail| detail.starts_with(TAG) && detail.contains(&line)),
            "{details:?}"
        );
        // Both Settler claims leave it alone now.
        assert!(ai.science_campus_holds_repeat_settler(&on, 0, cid, &plan));
    }
}

#[test]
fn a_five_city_empire_keeps_the_floor_settler_ahead_of_its_first_campus() {
    for cities in [3, 4, 5] {
        let (mut g, ai, cid, plan, _) = floor_settler_board(cities, 0.0);
        ai.claim_free_slot_campus(&mut g, 0, &plan);
        assert_eq!(
            head(&g, cid),
            Some(AdvancedAi::early_settler_item()),
            "{cities} cities"
        );
        assert!(!ai.campus_and_library_holds_the_settler(&g, 0, cid, &plan));
    }
}

#[test]
fn a_half_built_floor_settler_keeps_the_queue() {
    let (mut g, ai, cid, plan, _) = floor_settler_board(6, 50.0);
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(head(&g, cid), Some(AdvancedAi::early_settler_item()));
}

#[test]
fn a_builder_keeps_its_queue_against_the_library() {
    let (mut g, ranked, ai, plan) = library_board();
    for cid in &ranked {
        let city = g.cities.get_mut(cid).unwrap();
        city.queue = vec![unit("builder")];
        city.production = 0.0;
    }
    ai.claim_campus_building(&mut g, 0, &plan);
    for cid in &ranked {
        assert_eq!(head(&g, *cid), Some(unit("builder")));
    }
}

fn university() -> Item {
    Item::Building {
        building: crate::name!("university"),
    }
}

/// The six-city empire of `producer_empire` with its Libraries standing and
/// Education in hand, every city making 15 Production (a University well
/// inside ten Online turns), its queue set by `heads`.
fn university_board() -> (Game, Vec<u32>, AdvancedAi, StrategicPlan) {
    let (mut g, ranked) = producer_empire("CAMPUSUNIVERSITYFIRST", 94_133, 1);
    for tech in ["education", "mathematics"] {
        g.players[0].techs.insert(Name::new(tech));
    }
    for cid in &ranked {
        let adjustment = Arc::make_mut(&mut g.observed_city_yield_adjustments)
            .remove(cid)
            .unwrap_or_default();
        let base = g.city_yields(*cid).production;
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                production: 15.0 - base,
                ..adjustment
            },
        );
        assert_eq!(
            BasicAi::campus_building_item(&g, 0, *cid),
            Some(university()),
            "fixture: the University is buildable"
        );
        let turns = g.item_cost_for_city(0, *cid, &university()) / 15.0;
        assert!(
            AdvancedAi::campus_and_library_building(&g, 1, turns),
            "fixture: the University is {turns} turns away"
        );
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_campus_building_claims_the_queue();
    ai.disable_near_done_work_finishes();
    ai.enable_campus_and_library_before_the_claims();
    let plan = calm_plan(&g);
    (g, ranked, ai, plan)
}

/// A district head on a free tile of `cid` with `paid` Production in.
fn district_head(g: &mut Game, cid: u32, family: &str, paid: f64) -> Item {
    let center = g.cities[&cid].pos;
    let site = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != center && g.map.tiles[pos].district.is_none())
        .min()
        .expect("a free tile");
    let item = Item::District {
        district: BasicAi::civ_district(g, 0, family),
        pos: site,
    };
    let city = g.cities.get_mut(&cid).unwrap();
    city.queue = vec![item.clone()];
    city.production = paid;
    item
}

#[test]
fn the_university_passes_a_hub_but_not_a_builder_granary_aqueduct_zone_or_settler() {
    let (mut g, ranked, mut ai, plan) = university_board();
    // Every head is held by the claim's own rules: a Builder, Granary,
    // Aqueduct, Industrial Zone and Settler are never deferred, a Hub with
    // progress is not either.
    let hub = district_head(&mut g, ranked[0], "commercial_hub", 2.0);
    let builder = unit("builder");
    let settler = AdvancedAi::early_settler_item();
    let aqueduct = district_head(&mut g, ranked[3], "aqueduct", 0.0);
    let zone = district_head(&mut g, ranked[4], "industrial_zone", 0.0);
    for (cid, item) in [
        (ranked[1], &builder),
        (ranked[2], &granary()),
        (ranked[5], &settler),
    ] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.queue = vec![item.clone()];
        city.production = 0.0;
    }
    let kept: Vec<(u32, Item)> = vec![
        (ranked[1], builder),
        (ranked[2], granary()),
        (ranked[3], aqueduct),
        (ranked[4], zone),
        (ranked[5], settler),
    ];
    for (cid, item) in &kept {
        assert!(
            g.item_cost_for_city(0, *cid, item) > g.city_yields(*cid).production,
            "fixture: {item:?} is more than a turn from done"
        );
    }
    let details = journalled(&mut ai, &mut g, |ai, g| {
        ai.claim_campus_building(g, 0, &plan)
    });
    assert_eq!(head(&g, ranked[0]), Some(university()), "the Hub waits");
    assert_eq!(g.item_invested_production(ranked[0], &hub), 2.0);
    assert!(
        details.iter().any(|detail| detail.starts_with(TAG)
            && detail.contains("University goes ahead of the Commercial Hub")),
        "{details:?}"
    );
    for (cid, item) in kept {
        assert_eq!(
            head(&g, cid),
            Some(item.clone()),
            "{item:?} keeps its queue"
        );
    }
}

#[test]
fn a_counterweight_holy_site_yields_with_nothing_paid_but_not_with_progress() {
    let (base, armed, cid, plan, campus) = campus_board();
    let center = base.cities[&cid].pos;
    let campus_site = match &campus {
        Item::District { pos, .. } => *pos,
        _ => unreachable!(),
    };
    let site = base.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != center && *pos != campus_site)
        .min()
        .expect("a free tile");
    let holy_site = Item::District {
        district: crate::name!("holy_site"),
        pos: site,
    };
    for (paid, gene, passes) in [(0.0, true, true), (5.0, true, false), (0.0, false, false)] {
        let mut g = base.clone();
        let mut ai = armed.clone();
        if !gene {
            ai.disable_campus_and_library_before_the_claims();
        }
        let capital = g.cities.get_mut(&cid).unwrap();
        capital.queue = vec![holy_site.clone()];
        capital.production = paid;
        let details = journalled(&mut ai, &mut g, |ai, g| {
            ai.claim_free_slot_campus(g, 0, &plan)
        });
        let expected = if passes {
            campus.clone()
        } else {
            holy_site.clone()
        };
        assert_eq!(head(&g, cid), Some(expected), "paid {paid}, gene {gene}");
        assert_eq!(
            details.iter().any(|detail| detail.starts_with(TAG)
                && detail.contains("first Campus goes ahead of the Holy Site")),
            passes,
            "{details:?}"
        );
    }
}
