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

/// See `FAITH_WAR_EDGE`: the fixture's founder at 120 against our 100 opens
/// the war without the gene; under it the war waits for the founder to stand
/// at 0.7 times our power or under.
#[test]
fn the_edge_holds_a_faith_war_on_a_founder_near_our_power() {
    let (g, mut ai, _, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip_2();
    assert_eq!(ai.faith_war_gate(&g, 0, 1), None, "fixture: the war opens");
    ai.enable_faith_war_needs_the_edge();
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("edge")
    );

    let (mut g, mut ai, plan, _, _) = before_the_flip();
    ai.enable_faith_war_before_the_flip_2();
    ai.enable_faith_war_needs_the_edge();
    Arc::make_mut(&mut g.observed_military_power).insert(1, 60.0);
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1),
        None,
        "60 against 100 is under 0.7"
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        g.is_at_war(0, 1),
        "the war opens on a founder under the edge"
    );
}

#[test]
fn the_small_world_gene_is_opt_in() {
    opt_in_off_in_both_controllers("faith-war-on-a-small-world", |ai| {
        ai.faith_war_on_a_small_world
    });
}

/// `before_the_flip` on a world of `majors` living majors: the founder
/// (seat 1) also holds seat 2's majority, version 2 and the armed edge are
/// on, and we stand at `ours` power against the founder's `theirs`. On four
/// majors the race reads 50%.
fn small_world(
    majors: usize,
    ours: f64,
    theirs: f64,
) -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(majors, 40, 24, 581144, 250, 0, false);
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
    let sites = [(33, 8), (18, 8), (28, 8), (20, 18), (30, 18)];
    for rival in 1..majors {
        g.found_city_for(rival, sites[rival - 1], None);
        g.record_contact(0, rival);
    }
    g.players[1].religion = Some(FAITH.into());
    let observed = Arc::make_mut(&mut g.observed_majority_religion);
    observed.insert(1, FAITH.into());
    observed.insert(2, FAITH.into());
    let city = g.cities.get_mut(&leaning).unwrap();
    city.atheist_pressure = 0.0;
    city.pressure.insert(FAITH.into(), 100.0);
    g.at_war.clear();
    g.current = 0;
    g.turn = 60;
    g.players[0].gold = 10000.0;
    g.players[0].gold_per_turn = 5.0;
    let missionary = g.spawn_test_unit("missionary", 1, (4, 8));
    g.units.get_mut(&missionary).unwrap().religion = Some(FAITH.into());
    Arc::make_mut(&mut g.observed_military_power).extend([(0, ours), (1, theirs)]);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_faith_war_before_the_flip_2();
    ai.enable_faith_war_needs_the_edge();
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
    assert!(g.civ_follows_religion(2, FAITH));
    (g, ai, plan, leaning, missionary)
}

/// Off, the founder at twice our power holds the war on power exactly as
/// today. On, on four majors with its race at 50%, the war opens with us at
/// half its power, at a quarter (past the rout ratio) and at twice it.
#[test]
fn a_small_world_opens_the_faith_war_without_the_edge() {
    let (mut g, mut ai, plan, _, _) = small_world(4, 100.0, 200.0);
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None, "off: no reading");
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("power")
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "off: held at 0.5x as today");

    for (ours, theirs) in [(100.0, 200.0), (100.0, 400.0), (200.0, 100.0)] {
        let (mut g, mut ai, plan, _, _) = small_world(4, ours, theirs);
        ai.enable_faith_war_on_a_small_world();
        assert_eq!(
            ai.faith_war_small_world(&g, 0, 1),
            Some(SmallWorldRace {
                majors: 4,
                race: 50,
                others: 1
            })
        );
        assert_eq!(ai.faith_war_gate(&g, 0, 1), None, "{ours} against {theirs}");
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(g.is_at_war(0, 1), "on: opened at {ours} against {theirs}");
        let war = ai
            .faith_war_before_the_flip_war
            .expect("the war is recorded");
        assert_eq!((war.rival, war.opened), (1, 60));
    }

    // Without version 2 the gene is inert.
    let (g, mut ai, _, _, _) = small_world(4, 100.0, 200.0);
    ai.disable_faith_war_before_the_flip_2();
    ai.enable_faith_war_before_the_flip();
    ai.enable_faith_war_on_a_small_world();
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None, "rides version 2");
}

/// The treasury reserve and the income gate are lifted with the edge.
#[test]
fn a_small_world_opens_the_faith_war_on_an_empty_treasury() {
    let (mut g, mut ai, plan, _, _) = small_world(4, 100.0, 200.0);
    ai.enable_faith_war_on_a_small_world();
    ai.war_needs_a_treasury = true;
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -1.0;
    assert!(!ai.war_is_affordable(&g, 0));
    assert_eq!(ai.faith_war_gate(&g, 0, 1), None);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1));
}

/// Six living majors: held on power as today, gene or not. Four majors with
/// the founder holding only itself (25%): held as today.
#[test]
fn a_larger_world_or_a_lone_founder_is_held_as_today() {
    let (mut g, mut ai, plan, _, _) = small_world(6, 100.0, 200.0);
    ai.enable_faith_war_on_a_small_world();
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None, "six majors");
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("power")
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "six majors: held as today");

    let (mut g, mut ai, plan, _, _) = small_world(4, 100.0, 200.0);
    ai.enable_faith_war_on_a_small_world();
    Arc::make_mut(&mut g.observed_majority_religion).remove(&2);
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None, "race 25%");
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "a lone founder: held as today");

    // Three majors: the founder and the third major read 67%.
    let (mut g, mut ai, plan, _, _) = small_world(3, 100.0, 200.0);
    ai.enable_faith_war_on_a_small_world();
    assert_eq!(
        ai.faith_war_small_world(&g, 0, 1),
        Some(SmallWorldRace {
            majors: 3,
            race: 66,
            others: 1
        })
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "three majors: opened");
}

/// A city of ours falling still holds the war; so does a threatened one.
#[test]
fn a_small_world_keeps_the_falling_and_threatened_holds() {
    let (mut g, mut ai, plan, leaning, _) = small_world(4, 100.0, 200.0);
    ai.enable_faith_war_on_a_small_world();
    let turn = g.turn;
    let city = g.cities.get_mut(&leaning).unwrap();
    city.last_attacked = turn;
    city.hp = 80;
    assert!(AdvancedAi::our_cities_falling(&g, 0));
    let gate = ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate);
    assert!(matches!(gate, Some("threatened" | "falling")), "{gate:?}");
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "a falling city of ours");

    let (mut g, mut ai, plan, leaning, _) = small_world(4, 100.0, 200.0);
    ai.enable_faith_war_on_a_small_world();
    g.at_war.insert((0, 3));
    g.at_war.insert((3, 0));
    Arc::make_mut(&mut g.observed_military_power).insert(3, 300.0);
    let at = g.cities[&leaning].pos;
    for step in [(1, 0), (1, 1), (0, 1), (-1, 1)] {
        g.spawn_test_unit("swordsman", 3, (at.0 + step.0, at.1 + step.1));
    }
    assert!(ai.threatened_city(&g, 0).is_some(), "fixture: threatened");
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("threatened")
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "a threatened city of ours");
}

/// A declared friend is not declared on; the hold names the friendship.
#[test]
fn a_small_world_never_declares_on_a_friend() {
    let (mut g, mut ai, plan, _, _) = small_world(4, 100.0, 200.0);
    ai.enable_faith_war_on_a_small_world();
    ai.base.journal = crate::reasoning::Journal::recording();
    let until = g.turn + 30;
    g.players[0].friends_until.insert(1, until);
    g.players[1].friends_until.insert(0, until);
    assert!(g.are_friends(0, 1));
    let (gate, reason) = ai.faith_war_gate(&g, 0, 1).expect("held");
    assert_eq!(gate, "legal");
    assert!(reason.contains("a declared friend or ally"), "{reason}");
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "never on a friend");
    let thoughts = ai.journal().since(0).thoughts;
    assert!(
        thoughts.iter().any(|thought| thought.headline
            == "Holding the faith war on ".to_string() + &g.players[1].civ
            && thought.detail.contains("faith-war-on-a-small-world")
            && thought.detail.contains("a declared friend or ally")),
        "the friendship hold is journalled"
    );
}

/// The opening line names the gene, the world, the race and the edge it did
/// not ask; off, the held line is version 2's power line.
#[test]
fn a_small_world_journals_its_opening() {
    let (mut g, mut ai, plan, _, _) = small_world(4, 100.0, 200.0);
    ai.enable_faith_war_on_a_small_world();
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    let civ = g.players[1].civ.clone();
    let thoughts = ai.journal().since(0).thoughts;
    let opened = thoughts
        .iter()
        .find(|thought| thought.headline == format!("Opening the faith war on {civ}"))
        .expect("the opening is journalled");
    assert!(
        opened.detail.starts_with(
            "faith-war-on-a-small-world: 4 living majors, its race reads 50% (1 other major(s) on its faith), 1 spreader(s) at our cities and 1 of ours lean to its faith; the edge is not asked (ours 100 against their steady 200)"
        ),
        "{}",
        opened.detail
    );

    let (mut g, mut ai, plan, _, _) = small_world(4, 100.0, 200.0);
    ai.base.journal = crate::reasoning::Journal::recording();
    ai.advanced_diplomacy(&mut g, 0, &plan);
    let thoughts = ai.journal().since(0).thoughts;
    assert!(
        thoughts.iter().any(|thought| thought.headline
            == format!("Holding the faith war on {civ}")
            && thought.detail.starts_with("faith-war-before-the-flip-2: ")
            && thought.detail.contains("needs 0.7x")),
        "off: version 2's power line"
    );
}

/// The gene's war on four majors with no city of ours converting (our
/// leaning city already follows the faith, so the shipped keep is spent):
/// at 100 against 300 the outmatched offer is withheld and the founder's
/// offer refused while a spreader stood at our cities within five turns;
/// after five clear turns, or off, or on six majors, the offer is made.
fn small_world_war(
    majors: usize,
    on: bool,
    spreader_seen: u32,
) -> (Game, AdvancedAi, StrategicPlan) {
    let (mut g, mut ai, mut plan, leaning, _) = small_world(majors, 100.0, 300.0);
    if on {
        ai.enable_faith_war_on_a_small_world();
    }
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.turn = 90;
    ai.faith_war_before_the_flip_war = Some(FaithWar {
        rival: 1,
        opened: 70,
        spreader_seen,
    });
    // Following, not converting: the leaning city's majority is the faith.
    let city = g.cities.get_mut(&leaning).unwrap();
    city.pressure.clear();
    city.atheist_pressure = 0.0;
    city.pressure.insert(FAITH.into(), 100.0);
    assert_eq!(AdvancedAi::faith_war_converting(&g, 0, 1), 0);
    plan.strategy = GrandStrategy::Conquest;
    (g, ai, plan)
}

#[test]
fn a_small_world_keeps_the_faith_war_while_the_race_holds() {
    // A spreader stood at our cities this turn.
    let (mut g, mut ai, plan) = small_world_war(4, true, 90);
    ai.base.journal = crate::reasoning::Journal::recording();
    assert!(ai.faith_war_keeps_the_war(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        !ai.peace_offers.contains(&1),
        "on: withheld while the race holds"
    );
    let civ = g.players[1].civ.clone();
    let thoughts = ai.journal().since(0).thoughts;
    assert!(
        thoughts.iter().any(|thought| thought.headline
            == format!("Keeping the faith war with {civ}")
            && thought
                .detail
                .starts_with("faith-war-on-a-small-world: 4 living majors and its race reads 50%")),
        "the keep is journalled"
    );

    let (mut g, mut ai, plan) = small_world_war(4, false, 90);
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        ai.peace_offers.contains(&1),
        "off: the outmatched offer is made"
    );

    let (mut g, mut ai, plan) = small_world_war(6, true, 90);
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1), "six majors: as today");

    // Five clear turns: the shipped exit is kept. The missionary stands
    // away from our cities so the record's clock does not move.
    let (mut g, mut ai, plan) = small_world_war(4, true, 84);
    let missionary = g
        .units
        .values()
        .find(|unit| unit.kind == "missionary")
        .map(|unit| unit.id)
        .unwrap();
    g.units.get_mut(&missionary).unwrap().pos = (24, 8);
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        ai.peace_offers.contains(&1),
        "after five clear turns: offered"
    );

    // A city of ours falling ends the keep.
    let (mut g, ai, _) = small_world_war(4, true, 90);
    let turn = g.turn;
    let leaning = g.player_city_ids(0)[0];
    let city = g.cities.get_mut(&leaning).unwrap();
    city.last_attacked = turn;
    city.hp = 80;
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1), "falling: released");
}

#[test]
fn a_small_world_refuses_the_founders_peace_while_the_race_holds() {
    let refused = |on: bool| {
        let (mut g, mut ai, plan) = small_world_war(4, on, 90);
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
        g.is_at_war(0, 1)
    };
    assert!(refused(true), "on: the founder's peace is refused");
    assert!(!refused(false), "off: the founder's peace is taken");
}

/// In the war the gene opened, our soldiers at home condemn the founder's
/// spreaders: a Warrior beside the Missionary condemns it (`condemn_step`),
/// and the Domination seat's heretic hunt (`air-surge-2`) does the same.
#[test]
fn our_soldiers_condemn_the_spreaders_in_the_war_it_opened() {
    let opened = || {
        let (mut g, mut ai, plan, _, missionary) = small_world(4, 100.0, 200.0);
        ai.enable_faith_war_on_a_small_world();
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(g.is_at_war(0, 1));
        let warrior = g.spawn_test_unit("warrior", 0, (3, 8));
        (g, ai, missionary, warrior)
    };
    let (mut g, mut ai, missionary, warrior) = opened();
    assert!(ai.condemn_step(&mut g, 0, warrior));
    assert!(!g.units.contains_key(&missionary), "condemned");

    let (mut g, mut ai, missionary, warrior) = opened();
    ai.enable_air_surge_2();
    let hunters = ai.plan_heretic_hunt(&mut g, 0, &std::collections::BTreeSet::new());
    assert!(hunters.contains(&warrior));
    assert!(!g.units.contains_key(&missionary), "hunted and condemned");
}
