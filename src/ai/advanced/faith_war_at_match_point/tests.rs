use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use std::sync::Arc;

const FAITH: &str = "Orthodoxy";

/// Four majors. Seat 1 founded Orthodoxy, which also holds seats 2 and 3:
/// only our majority is missing. One of our three cities follows it (our
/// majority still holds out) and one of its Missionaries stands two tiles
/// from that city. We founded no faith, are at peace with everyone, solvent,
/// and stand at `ours` power against the founder's `theirs`. Version 2 and
/// the armed edge are on.
fn match_point(ours: f64, theirs: f64) -> (Game, AdvancedAi, StrategicPlan, u32) {
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
    for (rival, site) in [(1, (20, 8)), (2, (28, 8)), (3, (20, 18))] {
        g.found_city_for(rival, site, None);
        g.record_contact(0, rival);
    }
    g.players[1].religion = Some(FAITH.into());
    let observed = Arc::make_mut(&mut g.observed_majority_religion);
    for rival in 1..4 {
        observed.insert(rival, FAITH.into());
    }
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
    assert!(g.players[0].religion.is_none(), "we founded no faith");
    assert!(!g.civ_follows_religion(0, FAITH), "our majority holds out");
    assert!(AdvancedAi::we_are_the_last_holdout(&g, 0, 1));
    (g, ai, plan, leaning)
}

const READING: MatchPoint = MatchPoint {
    majors: 4,
    race: 75,
    leaning: 1,
    cities: 3,
};

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("faith-war-at-match-point", |ai| ai.faith_war_at_match_point);
}

/// At a quarter of the founder's power the war is held on the power floor
/// with the gene off and opens with it on; the record is the family's, and
/// the opening and each waived hold are journalled under the gene's tag.
#[test]
fn opens_at_a_quarter_of_the_founders_power_and_holds_off() {
    let (mut g, mut ai, plan, _) = match_point(100.0, 400.0);
    assert_eq!(ai.faith_war_match_point(&g, 0, 1), None, "off: no reading");
    assert_eq!(
        ai.faith_war_holds(&g, 0, 1, false)
            .first()
            .map(|(gate, _)| *gate),
        Some("power")
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "off: held on power");
    assert_eq!(ai.faith_war_before_the_flip_war, None);

    let (mut g, mut ai, plan, _) = match_point(100.0, 400.0);
    ai.enable_faith_war_at_match_point();
    ai.base.journal = crate::reasoning::Journal::recording();
    assert_eq!(ai.faith_war_match_point(&g, 0, 1), Some(READING));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "on: opened at 100 against 400");
    let war = ai
        .faith_war_before_the_flip_war
        .expect("the family's record");
    assert_eq!((war.rival, war.opened, war.spreader_seen), (1, 60, 60));
    let civ = g.players[1].civ.clone();
    let thoughts = ai.journal().since(0).thoughts;
    let opened = thoughts
        .iter()
        .find(|thought| thought.headline == format!("Opening the faith war on {civ}"))
        .expect("the opening is journalled");
    assert!(
        opened.detail.starts_with(
            "faith-war-at-match-point: its faith stands at match point (75% of 4 living majors; only our flip is missing), 1 of our 3 cities lean to it and 1 spreader(s) stand at our cities;"
        ),
        "{}",
        opened.detail
    );
    assert!(opened.detail.contains("defensive, no siege"));
    assert!(
        thoughts.iter().any(|thought| thought.headline
            == format!("Waiving the power hold on the faith war on {civ}")
            && thought.detail.starts_with(
                "faith-war-at-match-point: our 100 power against their steady 400 (needs 0.7x)"
            )),
        "the waived power floor is journalled"
    );
    assert!(
        thoughts.iter().any(|thought| thought.headline
            == format!("Waiving the counter-floor hold on the faith war on {civ}")
            && thought.detail.starts_with("faith-war-at-match-point: ")),
        "the waived counter-war floor is journalled"
    );

    // Without version 2, or for a seat with a faith of its own, no reading.
    let (g, mut ai, _, _) = match_point(100.0, 400.0);
    ai.enable_faith_war_at_match_point();
    ai.disable_faith_war_before_the_flip_2();
    ai.enable_faith_war_before_the_flip();
    assert_eq!(ai.faith_war_match_point(&g, 0, 1), None, "rides version 2");
    let (mut g, mut ai, _, _) = match_point(100.0, 400.0);
    ai.enable_faith_war_at_match_point();
    g.players[0].religion = Some("Islam".into());
    assert_eq!(ai.faith_war_match_point(&g, 0, 1), None, "a founder seat");
}

/// The live arming: `faith-war-on-a-small-world` lifts the power gates but
/// holds the war on a threatened city of ours. Under the gene that hold is
/// waived too.
#[test]
fn opens_with_a_threatened_city_of_ours() {
    let opened = |on: bool| {
        let (mut g, mut ai, plan, leaning) = match_point(100.0, 200.0);
        ai.enable_faith_war_on_a_small_world();
        if on {
            ai.enable_faith_war_at_match_point();
        }
        ai.base.journal = crate::reasoning::Journal::recording();
        g.at_war.insert((0, 3));
        g.at_war.insert((3, 0));
        Arc::make_mut(&mut g.observed_military_power).insert(3, 300.0);
        let at = g.cities[&leaning].pos;
        for step in [(1, 0), (1, 1), (0, 1), (-1, 1)] {
            g.spawn_test_unit("swordsman", 3, (at.0 + step.0, at.1 + step.1));
        }
        assert!(ai.threatened_city(&g, 0).is_some(), "fixture: threatened");
        assert_eq!(
            ai.faith_war_holds(&g, 0, 1, true)
                .first()
                .map(|(gate, _)| *gate),
            Some("threatened")
        );
        ai.advanced_diplomacy(&mut g, 0, &plan);
        let civ = g.players[1].civ.clone();
        let waived = ai.journal().since(0).thoughts.iter().any(|thought| {
            thought.headline == format!("Waiving the threatened hold on the faith war on {civ}")
                && thought
                    .detail
                    .starts_with("faith-war-at-match-point: a city of ours is threatened;")
        });
        (g.is_at_war(0, 1), waived)
    };
    assert_eq!(opened(false), (false, false), "off: held on the threat");
    assert_eq!(opened(true), (true, true), "on: opened past it");
}

/// The founder's faith holds seat 2 but not seat 3: not match point, so
/// the gene reads nothing and the war is held as today.
#[test]
fn does_not_open_when_the_faith_lacks_one_other_major() {
    let (mut g, mut ai, plan, _) = match_point(100.0, 400.0);
    ai.enable_faith_war_at_match_point();
    Arc::make_mut(&mut g.observed_majority_religion).remove(&3);
    assert!(!AdvancedAi::we_are_the_last_holdout(&g, 0, 1));
    assert!(!ai.faith_at_match_point(&g, 1), "the lane reads 50%");
    assert_eq!(ai.faith_war_match_point(&g, 0, 1), None);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "held on power as today");

    // Nor with no city of ours on the faith.
    let (mut g, mut ai, plan, leaning) = match_point(100.0, 400.0);
    ai.enable_faith_war_at_match_point();
    let city = g.cities.get_mut(&leaning).unwrap();
    city.pressure.clear();
    city.atheist_pressure = 50.0;
    assert_eq!(ai.faith_war_match_point(&g, 0, 1), None);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "no city of ours leans to it");
}

/// A declared friend is never declared on; the hold names the friendship
/// and the turn it ends.
#[test]
fn does_not_open_against_a_declared_friend() {
    let (mut g, mut ai, plan, _) = match_point(100.0, 400.0);
    ai.enable_faith_war_at_match_point();
    ai.base.journal = crate::reasoning::Journal::recording();
    let until = g.turn + 30;
    g.players[0].friends_until.insert(1, until);
    g.players[1].friends_until.insert(0, until);
    assert!(g.are_friends(0, 1));
    assert_eq!(ai.faith_war_match_point(&g, 0, 1), Some(READING));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "never on a friend");
    assert_eq!(ai.faith_war_before_the_flip_war, None);
    let civ = g.players[1].civ.clone();
    let thoughts = ai.journal().since(0).thoughts;
    assert!(
        thoughts.iter().any(|thought| thought.headline
            == format!("Holding the faith war on {civ}")
            && thought.detail.starts_with("faith-war-at-match-point: ")
            && thought.detail.contains("a declared friend until turn 90")),
        "the friendship hold is journalled"
    );
}

/// When the host allows no war on the founder (Canada takes no Surprise
/// War, a Formal War needs a mature denouncement), the gene denounces it.
#[test]
fn a_founder_the_host_shields_is_denounced_first() {
    let denounced = |on: bool| {
        let (mut g, mut ai, plan, _) = match_point(100.0, 400.0);
        if on {
            ai.enable_faith_war_at_match_point();
        }
        ai.base.journal = crate::reasoning::Journal::recording();
        Arc::make_mut(&mut g.host_war_blocks).insert((0, 1, g.turn));
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(!g.is_at_war(0, 1), "the host refuses the war");
        let civ = g.players[1].civ.clone();
        let line = ai.journal().since(0).thoughts.iter().any(|thought| {
            thought.headline == format!("Denouncing {civ} for the faith war")
                && thought.detail.starts_with("faith-war-at-match-point: ")
        });
        let active = g.players[0]
            .denounced_until
            .get(&1)
            .is_some_and(|until| *until > g.turn);
        (active, line)
    };
    assert_eq!(denounced(false), (false, false), "off: nothing sent");
    assert_eq!(denounced(true), (true, true), "on: denounced");
}

/// Off, the declaration desk holds the war on the counter war's floor and
/// says "cannot stop it". Under the gene, at match point, that floor does
/// not hold the founder and the line is not written. Version 2's cooldown
/// holds the faith war itself so the desk is reached.
#[test]
fn the_counter_floor_line_is_not_written_at_match_point() {
    let desk = |on: bool| {
        let (mut g, mut ai, mut plan, _) = match_point(100.0, 400.0);
        if on {
            ai.enable_faith_war_at_match_point();
        }
        ai.base.journal = crate::reasoning::Journal::recording();
        ai.faith_war_peace_at.insert(1, g.turn - 2);
        assert_eq!(
            ai.faith_war_cooldown(&g, 0, 1),
            Some(2),
            "fixture: cooldown"
        );
        plan.strategy = GrandStrategy::Conquest;
        plan.target_player = Some(1);
        plan.target_city = g.player_city_ids(1).first().copied();
        assert!(ai.urgent_victory_threat(&g, 1), "fixture: urgent");
        assert!(
            ai.counter_war_hopeless(&g, 0, 1),
            "fixture: under the floor"
        );
        let holds = ai.counter_floor_holds(&g, 0, 1, true);
        ai.advanced_diplomacy(&mut g, 0, &plan);
        let civ = g.players[1].civ.clone();
        let thoughts = ai.journal().since(0).thoughts;
        let held_off = thoughts
            .iter()
            .any(|thought| thought.headline == format!("Holding off war with {civ}"));
        let cannot = thoughts
            .iter()
            .any(|thought| thought.detail.contains("cannot stop it"));
        (holds, held_off, cannot, g.is_at_war(0, 1))
    };
    assert_eq!(
        desk(false),
        (true, true, true, false),
        "off: the floor line"
    );
    assert_eq!(
        desk(true),
        (false, true, false, false),
        "on: the desk holds for its own reason, not the floor"
    );
}

/// The war on record at 100 against 300 with no city of ours converting
/// (our leaning city already follows the faith): kept while the faith
/// stands at match point and a spreader stood at our cities within five
/// turns, a falling city included; released after five clear turns, and
/// never kept with the gene off.
fn match_point_war(on: bool, spreader_seen: u32) -> (Game, AdvancedAi, StrategicPlan) {
    let (mut g, mut ai, mut plan, _) = match_point(100.0, 300.0);
    if on {
        ai.enable_faith_war_at_match_point();
    }
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.turn = 90;
    ai.faith_war_before_the_flip_war = Some(FaithWar {
        rival: 1,
        opened: 70,
        spreader_seen,
    });
    plan.strategy = GrandStrategy::Conquest;
    (g, ai, plan)
}

#[test]
fn the_war_is_kept_while_the_match_point_holds() {
    let (mut g, mut ai, plan) = match_point_war(true, 90);
    ai.base.journal = crate::reasoning::Journal::recording();
    assert_eq!(ai.faith_war_match_point_keeps(&g, 0, 1), Some((READING, 0)));
    assert!(ai.faith_war_keeps_the_war(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1), "on: no peace offered");
    let civ = g.players[1].civ.clone();
    assert!(
        ai.journal().since(0).thoughts.iter().any(|thought| {
            thought.headline
            == format!("Keeping the faith war with {civ}")
            && thought.detail.starts_with(
                "faith-war-at-match-point: its faith stands at match point (75% of 4 living majors"
            )
        }),
        "the keep is journalled"
    );

    let (g, ai, _) = match_point_war(false, 90);
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1), "off: not kept");

    // A city of ours falling does not release it at match point.
    let (mut g, ai, _) = match_point_war(true, 90);
    let turn = g.turn;
    let city = g.player_city_ids(0)[1];
    let city = g.cities.get_mut(&city).unwrap();
    city.last_attacked = turn;
    city.hp = 80;
    assert!(AdvancedAi::our_cities_falling(&g, 0));
    assert!(ai.faith_war_keeps_the_war(&g, 0, 1), "falling: still kept");

    // Five clear turns: the family's exit applies.
    let (g, ai, _) = match_point_war(true, 85);
    assert_eq!(ai.faith_war_match_point_keeps(&g, 0, 1), None);
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1), "five clear turns");

    // No longer at match point: released.
    let (mut g, ai, _) = match_point_war(true, 90);
    Arc::make_mut(&mut g.observed_majority_religion).remove(&3);
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1), "match point gone");
}
