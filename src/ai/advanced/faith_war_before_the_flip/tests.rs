use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use std::sync::Arc;

const FAITH: &str = "Orthodoxy";

/// Seat 1 founded Orthodoxy and holds only its own majority. Of our three
/// cities one follows the faith, so our majority still holds out, and one of
/// its Missionaries stands two tiles from that city. We are at peace with
/// everyone, solvent, and at 100 power against its 120.
fn before_the_flip() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(4, 40, 24, 581144, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    let leaning = g.found_city_for(0, (2, 8), None);
    g.found_city_for(0, (7, 8), None);
    g.found_city_for(0, (12, 8), None);
    g.found_city_for(1, (33, 8), None);
    g.found_city_for(2, (18, 8), None);
    g.found_city_for(3, (28, 8), None);
    g.players[1].religion = Some(FAITH.into());
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, FAITH.into());
    let city = g.cities.get_mut(&leaning).unwrap();
    city.atheist_pressure = 0.0;
    city.pressure.insert(FAITH.into(), 100.0);
    for rival in 1..4 {
        g.record_contact(0, rival);
    }
    g.at_war.clear();
    g.current = 0;
    g.turn = 60;
    g.players[0].gold = 10000.0;
    g.players[0].gold_per_turn = 5.0;
    let missionary = g.spawn_test_unit("missionary", 1, (4, 8));
    g.units.get_mut(&missionary).unwrap().religion = Some(FAITH.into());
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 100.0), (1, 120.0)]);
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(!g.civ_follows_religion(0, FAITH));
    (g, ai, plan, leaning, missionary)
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("faith-war-before-the-flip", |ai| {
        ai.faith_war_before_the_flip
    });
}

/// The founder's spreader at our leaning city opens the defensive war under
/// the gene; off, the seat keeps the peace.
#[test]
fn the_founders_spreader_at_a_leaning_city_opens_the_faith_war() {
    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "off: unchanged");
    assert_eq!(ai.faith_war_before_the_flip_war, None);

    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "on: the founder is declared on");
    let war = ai
        .faith_war_before_the_flip_war
        .expect("the gene's war is recorded");
    assert_eq!((war.rival, war.opened, war.spreader_seen), (1, 60, 60));
}

/// No trigger: our majority already follows the faith, no city of ours leans
/// to it, or no spreader stands at our cities.
#[test]
fn no_war_after_the_flip_without_a_leaning_city_or_without_a_spreader() {
    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    Arc::make_mut(&mut g.observed_majority_religion).insert(0, FAITH.into());
    assert!(g.civ_follows_religion(0, FAITH));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "our majority already flipped");

    let (mut g, mut ai, plan, leaning, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    let city = g.cities.get_mut(&leaning).unwrap();
    city.pressure.clear();
    city.atheist_pressure = 50.0;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "no city of ours leans to the faith");

    let (mut g, mut ai, plan, _, missionary) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    g.units.get_mut(&missionary).unwrap().pos = (24, 8);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "no spreader within six tiles");
}

/// Each gate holds the war: power under 0.7 times the founder's steady power
/// (which is also the rout guard's side), a city of ours falling, negative
/// income, the treasury reserve, or two major wars already running.
#[test]
fn each_gate_holds_the_faith_war() {
    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    Arc::make_mut(&mut g.observed_military_power).insert(1, 160.0);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "100 against 160 is under 0.7 times");
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("power")
    );

    let (mut g, mut ai, plan, leaning, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    let turn = g.turn;
    let city = g.cities.get_mut(&leaning).unwrap();
    city.last_attacked = turn;
    city.hp = 80;
    assert!(AdvancedAi::our_cities_falling(&g, 0));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "a falling city of ours");

    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    g.players[0].gold_per_turn = -3.0;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "negative income");
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("income")
    );

    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    ai.war_needs_a_treasury = true;
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -1.0;
    assert!(!ai.war_is_affordable(&g, 0));
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("treasury"),
        "the treasury-reserve line holds"
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "the treasury reserve");

    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    for enemy in [2, 3] {
        g.at_war.insert((0, enemy));
        g.at_war.insert((enemy, 0));
    }
    Arc::make_mut(&mut g.observed_military_power).extend([(2, 10.0), (3, 10.0)]);
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("enemies")
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "two major wars already run");
}

/// The gene's war with a city of ours still converting: our offer is
/// withheld and the founder's is refused; with no city converting the
/// outmatched offer is made.
fn the_genes_war_outmatched() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut g, mut ai, mut plan, leaning, missionary) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.turn = 90;
    ai.faith_war_before_the_flip_war = Some(FaithWar {
        rival: 1,
        opened: 70,
        spreader_seen: 90,
    });
    // Converting, not yet following: the faith at 60% of the city's
    // strongest pressure.
    let city = g.cities.get_mut(&leaning).unwrap();
    city.pressure.clear();
    city.pressure.insert("Islam".into(), 100.0);
    city.pressure.insert(FAITH.into(), 70.0);
    Arc::make_mut(&mut g.observed_military_power).extend([(0, 100.0), (1, 300.0)]);
    plan.strategy = GrandStrategy::Conquest;
    (g, ai, plan, leaning, missionary)
}

#[test]
fn peace_is_withheld_while_a_city_of_ours_is_converting() {
    let (mut g, mut ai, plan, _, _) = the_genes_war_outmatched();
    assert!(ai.faith_war_keeps_the_war(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1), "withheld while converting");
    assert!(g.is_at_war(0, 1));

    let (mut g, mut ai, plan, leaning, _) = the_genes_war_outmatched();
    let city = g.cities.get_mut(&leaning).unwrap();
    city.pressure.clear();
    city.atheist_pressure = 50.0;
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1), "no city converting: offered");

    let (mut g, mut ai, plan, _, _) = the_genes_war_outmatched();
    ai.disable_faith_war_before_the_flip();
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1), "off: nothing kept");
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        ai.peace_offers.contains(&1),
        "off: the outmatched offer is made"
    );
}

#[test]
fn the_founders_peace_offer_is_refused_while_converting() {
    let (mut g, mut ai, plan, _, _) = the_genes_war_outmatched();
    g.current = 1;
    g.apply(
        1,
        &Action::ProposeDeal {
            player: 0,
            give_gold: 0.0,
            request_gold: 0.0,
            open_borders: false,
            friendship: false,
            peace: true,
            alliance: None,
        },
    )
    .unwrap();
    g.current = 0;
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "the founder's peace is refused");
}

/// Peace after five clear turns: no spreader of the faith at our cities for
/// five turns and no city converting offers the founder peace from strength;
/// two clear turns do not.
#[test]
fn the_war_ends_after_five_clear_turns() {
    let clear = |seen: u32| {
        let (mut g, mut ai, mut plan, leaning, missionary) = before_the_flip();
        ai.enable_faith_war_before_the_flip();
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        g.turn = 90;
        g.remove_unit(missionary);
        let city = g.cities.get_mut(&leaning).unwrap();
        city.pressure.clear();
        city.atheist_pressure = 50.0;
        ai.faith_war_before_the_flip_war = Some(FaithWar {
            rival: 1,
            opened: 70,
            spreader_seen: seen,
        });
        Arc::make_mut(&mut g.observed_military_power).extend([(0, 300.0), (1, 100.0)]);
        plan.strategy = GrandStrategy::Conquest;
        ai.advanced_diplomacy(&mut g, 0, &plan);
        ai.peace_offers.contains(&1)
    };
    assert!(clear(84), "six clear turns: peace offered");
    assert!(!clear(88), "two clear turns: not yet");
}

/// The record follows the war: a spreader at our cities moves the clear
/// clock, and peace drops the record.
#[test]
fn the_record_follows_the_war() {
    let (mut g, mut ai, _, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.turn = 90;
    ai.faith_war_before_the_flip_war = Some(FaithWar {
        rival: 1,
        opened: 70,
        spreader_seen: 80,
    });
    ai.faith_war_track(&g, 0);
    assert_eq!(ai.faith_war_before_the_flip_war.unwrap().spreader_seen, 90);
    g.at_war.clear();
    ai.faith_war_track(&g, 0);
    assert_eq!(ai.faith_war_before_the_flip_war, None);
}

#[test]
fn version_two_is_opt_in() {
    opt_in_off_in_both_controllers("faith-war-before-the-flip-2", |ai| {
        ai.faith_war_before_the_flip_2
    });
}

const OURS: &str = "Buddhism";

/// We founded Buddhism and it holds our majority: `ours` of our cities follow
/// it, `leaning` follow the founder's Orthodoxy, and the rest follow
/// nothing. Seat 1's Missionary still stands at our leaning city.
fn founder_majority(
    ours: usize,
    leaning: usize,
    plain: usize,
) -> (Game, AdvancedAi, StrategicPlan) {
    let (mut g, mut ai, plan, first, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip_2();
    // The fixture's three cities: the first leans to Orthodoxy.
    let mut ids: Vec<u32> = g
        .player_city_ids(0)
        .into_iter()
        .filter(|cid| *cid != first)
        .collect();
    for y in 0..(ours + leaning + plain).saturating_sub(3) {
        ids.push(g.found_city_for(0, (2 + 5 * (y as i32 % 3), 14 + 4 * (y as i32 / 3)), None));
    }
    let mut order = vec![first];
    order.extend(ids);
    g.players[0].religion = Some(OURS.into());
    for (i, cid) in order.iter().enumerate() {
        let city = g.cities.get_mut(cid).unwrap();
        city.pressure.clear();
        if i < leaning {
            city.atheist_pressure = 0.0;
            city.pressure.insert(FAITH.into(), 100.0);
        } else if i < leaning + ours {
            city.atheist_pressure = 0.0;
            city.pressure.insert(OURS.into(), 100.0);
        } else {
            city.atheist_pressure = 50.0;
        }
    }
    Arc::make_mut(&mut g.observed_majority_religion).insert(0, OURS.into());
    assert!(g.civ_follows_religion(0, OURS));
    (g, ai, plan)
}

/// Our founded faith holds 4 of our 5 cities and only one leans to the
/// founder's: version 2 holds with the founder line; the original fires.
#[test]
fn a_founder_majority_holds_version_two() {
    let (mut g, mut ai, plan) = founder_majority(4, 1, 0);
    assert_eq!(ai.faith_war_founder_holds(&g, 0, 1), Some((4, 5)));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "version 2: the founder majority holds");

    let (mut g, mut ai, plan) = founder_majority(4, 1, 0);
    ai.disable_faith_war_before_the_flip_2();
    ai.enable_faith_war_before_the_flip();
    assert_eq!(ai.faith_war_founder_holds(&g, 0, 1), None);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "the original fires");
}

/// Two of our cities leaning to the founder's faith, or our faith one city
/// from losing its majority, open version 2's war.
#[test]
fn a_founder_with_two_leaning_or_one_city_from_flipping_fires() {
    let (mut g, mut ai, plan) = founder_majority(5, 2, 0);
    assert_eq!(
        ai.faith_war_founder_holds(&g, 0, 1),
        None,
        "two lean to its faith"
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1));

    let (mut g, mut ai, plan) = founder_majority(3, 1, 1);
    assert_eq!(
        ai.faith_war_founder_holds(&g, 0, 1),
        None,
        "3 of 5: one more loss ends our majority"
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1));
}

/// A seat that founded no faith, or whose majority does not follow its own,
/// fires under version 2 exactly as under the original.
#[test]
fn version_two_fires_for_a_non_founder_as_before() {
    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip_2();
    assert_eq!(ai.faith_war_founder_holds(&g, 0, 1), None);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "non-founder: fires");
    let war = ai.faith_war_before_the_flip_war.unwrap();
    assert_eq!((war.rival, war.opened), (1, 60));
}

/// Version 2's war limit is version 1's: beside one running major war both
/// open (two wars at most); beside two both hold.
#[test]
fn version_two_keeps_version_ones_war_limit() {
    let at_war_with = |two: bool, enemies: &[usize]| {
        let (mut g, mut ai, plan, _, _) = before_the_flip();
        if two {
            ai.enable_faith_war_before_the_flip_2();
        } else {
            ai.enable_faith_war_before_the_flip();
        }
        for enemy in enemies {
            g.at_war.insert((0, *enemy));
            g.at_war.insert((*enemy, 0));
            Arc::make_mut(&mut g.observed_military_power).insert(*enemy, 10.0);
        }
        let gate = ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate);
        ai.advanced_diplomacy(&mut g, 0, &plan);
        (gate, g.is_at_war(0, 1))
    };
    assert_eq!(
        at_war_with(true, &[2]),
        (None, true),
        "version 2 opens beside one war"
    );
    assert_eq!(
        at_war_with(false, &[2]),
        (None, true),
        "as the original does"
    );
    assert_eq!(
        at_war_with(true, &[2, 3]),
        (Some("wars"), false),
        "version 2 holds beside two"
    );
    assert_eq!(
        at_war_with(false, &[2, 3]),
        (Some("enemies"), false),
        "as the original does"
    );
}

/// Version 2's same-founder cooldown: a peace with the founder 9 turns ago
/// holds the faith war; a city of ours being converted lifts it; 21 turns on
/// it lapses. The record comes from any peace the board shows.
#[test]
fn version_two_waits_twenty_turns_after_a_peace_with_the_founder() {
    let reopen = |peace_ago: u32, converting: bool| {
        let (mut g, mut ai, plan, leaning, _) = before_the_flip();
        ai.enable_faith_war_before_the_flip_2();
        if converting {
            let city = g.cities.get_mut(&leaning).unwrap();
            city.pressure.clear();
            city.pressure.insert("Islam".into(), 100.0);
            city.pressure.insert(FAITH.into(), 70.0);
        }
        // At war with the founder until the peace, then at peace.
        let peace_turn = g.turn - peace_ago;
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        let now = g.turn;
        g.turn = peace_turn - 1;
        ai.faith_war_track(&g, 0);
        g.at_war.clear();
        g.turn = peace_turn;
        ai.faith_war_track(&g, 0);
        assert_eq!(ai.faith_war_peace_at.get(&1), Some(&peace_turn));
        g.turn = now;
        let held = ai.faith_war_cooldown(&g, 0, 1);
        ai.advanced_diplomacy(&mut g, 0, &plan);
        (held, g.is_at_war(0, 1))
    };
    assert_eq!(
        reopen(9, false),
        (Some(9), false),
        "nine turns after the peace: held"
    );
    assert_eq!(
        reopen(9, true),
        (None, true),
        "a converting city lifts the cooldown"
    );
    assert_eq!(reopen(21, false), (None, true), "21 turns on: reopened");

    let (g, mut ai, _, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip();
    ai.faith_war_peace_at.insert(1, g.turn - 1);
    assert_eq!(
        ai.faith_war_cooldown(&g, 0, 1),
        None,
        "version 1 has no cooldown"
    );
}
