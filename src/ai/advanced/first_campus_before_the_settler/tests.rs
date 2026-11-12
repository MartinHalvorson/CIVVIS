use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::install_test_district;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn settler() -> Item {
    AdvancedAi::early_settler_item()
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

/// The live Gran Colombia shape at Online t35 (a model board, not a native
/// replay): three cities, the capital training the floor's Settler with 20
/// Production paid and a five-turn Campus quoted there, the other two at one
/// citizen. Every tile is flat grassland hills, so every site ties.
fn board() -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let mut g = Game::new_full(2, 36, 20, 95_344, 250, 0, false);
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
    g.turn = 35;
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
    capital.queue = vec![settler()];
    capital.production = 20.0;
    let campus = BasicAi::first_campus_item(&g, 0, cid).unwrap();
    quote(&mut g, cid, &campus, 5.0, 56.0);
    quote(&mut g, cid, &settler(), 8.0, 85.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_campus_claims_the_free_slot_2();
    ai.enable_early_settler_floor();
    ai.enable_first_campus_before_the_settler();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 9,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(ai.early_settler_floor_holds(&g, 0, cid, &plan));
    assert!(ai
        .free_slot_campus_item(&g, 0, cid, &plan)
        .is_some_and(|(item, turns)| item == campus && turns == 5.0));
    assert!(!AdvancedAi::empire_has_a_campus(&g, 0));
    (g, ai, cid, plan, campus)
}

fn other_city(g: &Game, cid: u32) -> u32 {
    g.player_city_ids(0)
        .into_iter()
        .find(|id| *id != cid)
        .unwrap()
}

#[test]
fn ships_off() {
    opt_in_off_in_both_controllers("first-campus-before-the-settler", |ai| {
        ai.first_campus_before_the_settler
    });
}

#[test]
fn first_campus_goes_ahead_of_the_floor_settler_which_keeps_its_progress() {
    let (mut g, mut ai, cid, plan, campus) = board();
    let mut off = ai.clone();
    off.disable_first_campus_before_the_settler();
    let mut control = g.clone();
    off.claim_free_slot_campus(&mut control, 0, &plan);
    assert_eq!(control.cities[&cid].queue.first(), Some(&settler()));

    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(g.turn, 0);
    ai.attach_journal(journal.handle());
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
    assert_eq!(g.item_invested_production(cid, &settler()), 20.0);
    assert!(AdvancedAi::empire_has_a_campus(&g, 0));
    let thoughts = journal.since(0).thoughts;
    assert!(
        thoughts.iter().any(|thought| thought
            .detail
            .starts_with("first-campus-before-the-settler:")),
        "{thoughts:?}"
    );
}

#[test]
fn the_exception_keeps_the_settler_without_its_conditions() {
    let (base, armed, cid, base_plan, campus) = board();
    for case in [
        "off",
        "science target",
        "war",
        "rush",
        "threat",
        "recent attack",
        "one city",
        "campus standing elsewhere",
        "campus queued elsewhere",
        "eight turn campus",
        "better site elsewhere",
        "not a settler",
    ] {
        let mut g = base.clone();
        let mut ai = armed.clone();
        let mut plan = base_plan.clone();
        let other = other_city(&g, cid);
        match case {
            "off" => ai.disable_first_campus_before_the_settler(),
            "science target" => ai.victory_target = Some(VictoryTarget::Science),
            "war" => {
                g.at_war.insert((0, 1));
            }
            "rush" => plan.rush = true,
            "threat" => plan.threatened_city = Some(cid),
            "recent attack" => g.cities.get_mut(&cid).unwrap().last_attacked = g.turn - 1,
            "one city" => {
                for id in g.player_city_ids(0) {
                    if id != cid {
                        g.cities.get_mut(&id).unwrap().owner = 1;
                    }
                }
            }
            "campus standing elsewhere" => {
                install_test_district(&mut g, other, "campus");
            }
            "campus queued elsewhere" => {
                let there = Item::District {
                    district: crate::name!("campus"),
                    pos: g.cities[&other].pos,
                };
                g.cities.get_mut(&other).unwrap().queue.push(there);
            }
            "eight turn campus" => quote(&mut g, cid, &campus, 8.0, 56.0),
            "better site elsewhere" => {
                // Mountains around the other city: its Campus sites read
                // +1 Science for each, the capital's none.
                let home = g.cities[&other].pos;
                for pos in g.wdisk(home, 2) {
                    if g.wdist(pos, home) == 2 {
                        if let Some(tile) = g.map.tiles.get_mut(&pos) {
                            tile.terrain = crate::name!("mountain");
                            tile.hills = false;
                        }
                    }
                }
                g.cities.get_mut(&other).unwrap().pop = 5;
                let there = BasicAi::first_campus_item(&g, 0, other).unwrap();
                quote(&mut g, other, &there, 6.0, 56.0);
                assert!(ai.free_slot_campus_item(&g, 0, other, &plan).is_some());
                let site = |item: &Item| match item {
                    Item::District { district, pos } => {
                        g.district_site_yields(*district, *pos).total()
                    }
                    _ => unreachable!(),
                };
                assert!(site(&there) > site(&campus), "fixture: a better site");
            }
            "not a settler" => g.cities.get_mut(&cid).unwrap().queue.clear(),
            _ => unreachable!(),
        }
        let turns = g.host_production_turns(cid, &campus).unwrap();
        assert!(
            !ai.first_campus_precedes_the_settler(&g, 0, cid, &plan, &campus, turns),
            "{case}"
        );
    }
}

#[test]
fn the_floor_trains_its_next_settler_elsewhere_and_leaves_the_campus() {
    let (mut g, ai, cid, plan, campus) = board();
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
    // A fresh replan: nothing paid into the Campus yet.
    g.cities.get_mut(&cid).unwrap().production = 0.0;
    assert!(AdvancedAi::early_settler_may_displace(&g, 0, cid));
    assert!(ai.first_campus_holds_the_settler(&g, 0, cid, &plan));
    assert!(ai.science_campus_holds_repeat_settler(&g, 0, cid, &plan));

    // Gene off, the floor puts the Settler back over the unpaid Campus.
    let mut control = g.clone();
    let mut off = ai.clone();
    off.disable_first_campus_before_the_settler();
    off.claim_early_settler_floor(&mut control, 0, &plan);
    assert_eq!(control.cities[&cid].queue.first(), Some(&settler()));

    // Gene on, no other city can train one yet: the Campus keeps its queue.
    let mut alone = g.clone();
    ai.claim_early_settler_floor(&mut alone, 0, &plan);
    assert_eq!(alone.cities[&cid].queue.first(), Some(&campus));

    // A second city grown to train it takes the Settler instead.
    let other = other_city(&g, cid);
    let city = g.cities.get_mut(&other).unwrap();
    city.pop = 5;
    city.queue = vec![Item::Building {
        building: crate::name!("monument"),
    }];
    ai.claim_early_settler_floor(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
    assert_eq!(g.cities[&other].queue.first(), Some(&settler()));
}

#[test]
fn the_commitment_yields_to_war_threat_and_a_standing_campus() {
    let (mut g, ai, cid, plan, _) = board();
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    assert!(ai.first_campus_holds_the_settler(&g, 0, cid, &plan));
    let mut threatened = plan.clone();
    threatened.threatened_city = Some(cid);
    assert!(!ai.first_campus_holds_the_settler(&g, 0, cid, &threatened));
    let mut war = g.clone();
    war.at_war.insert((0, 1));
    assert!(!ai.first_campus_holds_the_settler(&war, 0, cid, &plan));
    let mut standing = g.clone();
    let other = other_city(&standing, cid);
    install_test_district(&mut standing, other, "campus");
    assert!(!ai.first_campus_holds_the_settler(&standing, 0, cid, &plan));
}
