use super::*;
use crate::ai::advanced::faith_war_before_the_flip::{FaithWar, SmallWorldRace};
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use std::sync::Arc;

const FAITH: &str = "Orthodoxy";

/// Four majors. Seat 1 founded Orthodoxy and holds only its own majority
/// (its race reads 25%). Of our three cities one follows the faith and one
/// is being converted to it, so our flip is next while our majority still
/// holds out; one of its Missionaries stands two tiles from the first. We
/// are at peace, solvent, and at `ours` power against the founder's
/// `theirs`. Version 2, the armed edge and `faith-war-on-a-small-world` are
/// on.
fn own_flip(ours: f64, theirs: f64) -> (Game, AdvancedAi, StrategicPlan, u32) {
    fixture(ours, theirs, false, 3)
}

/// [`own_flip`] with `cities` of ours (2 or 3; with two, only the first
/// leans to the faith) and the world's barbarians when `barbarians`.
fn fixture(
    ours: f64,
    theirs: f64,
    barbarians: bool,
    cities: usize,
) -> (Game, AdvancedAi, StrategicPlan, u32) {
    let mut g = Game::new_full(4, 40, 24, 581144, 250, 0, barbarians);
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
    let second = g.found_city_for(0, (7, 8), None);
    if cities >= 3 {
        g.found_city_for(0, (12, 8), None);
    }
    for (rival, site) in [(1, (33, 8)), (2, (18, 8)), (3, (28, 8))] {
        g.found_city_for(rival, site, None);
        g.record_contact(0, rival);
    }
    g.players[1].religion = Some(FAITH.into());
    Arc::make_mut(&mut g.observed_majority_religion).insert(1, FAITH.into());
    let city = g.cities.get_mut(&leaning).unwrap();
    city.atheist_pressure = 0.0;
    city.pressure.insert(FAITH.into(), 100.0);
    // Converting, not converted: the faith leads the city's religions but
    // the city still follows none.
    if cities >= 3 {
        let city = g.cities.get_mut(&second).unwrap();
        city.atheist_pressure = 1_000.0;
        city.pressure.insert(FAITH.into(), 100.0);
    }
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
    ai.enable_faith_war_on_a_small_world();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    };
    assert!(!g.civ_follows_religion(0, FAITH), "our majority holds out");
    assert!(!g.civ_follows_religion(2, FAITH) && !g.civ_follows_religion(3, FAITH));
    (g, ai, plan, leaning)
}

const READING: SmallWorldRace = SmallWorldRace {
    majors: 4,
    race: 25,
    others: 0,
};

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("own-flip-faith-war-asks-no-power", |ai| {
        ai.own_flip_faith_war_asks_no_power
    });
}

/// At a quarter of the founder's power the own-flip reading is under its
/// 0.5 floor: off, the war is held on power as today; on, it opens, and the
/// opening names the gene and the ratio it waived.
#[test]
fn opens_on_our_own_flip_at_a_quarter_of_the_founders_power() {
    let (mut g, mut ai, plan, _) = own_flip(50.0, 200.0);
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None, "off: under 0.5x");
    assert_eq!(ai.own_flip_asks_no_power(&g, 0, 1), None);
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("power")
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "off: held on power");

    let (mut g, mut ai, plan, _) = own_flip(50.0, 200.0);
    ai.enable_own_flip_faith_war_asks_no_power();
    ai.base.journal = crate::reasoning::Journal::recording();
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), Some(READING));
    assert_eq!(ai.own_flip_asks_no_power(&g, 0, 1), Some(0.25));
    assert_eq!(ai.faith_war_gate(&g, 0, 1), None);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "on: opened at 50 against 200");
    let war = ai
        .faith_war_before_the_flip_war
        .expect("the family's record");
    assert_eq!((war.rival, war.opened), (1, 60));
    let civ = g.players[1].civ.clone();
    let thoughts = ai.journal().since(0).thoughts;
    let opened = thoughts
        .iter()
        .find(|thought| thought.headline == format!("Opening the faith war on {civ}"))
        .expect("the opening is journalled");
    assert!(
        opened.detail.starts_with(
            "own-flip-faith-war-asks-no-power: only its founder follows its faith and 2 of our 3 cities lean to it"
        ),
        "{}",
        opened.detail
    );
    assert!(
        opened
            .detail
            .contains("ours 50 against their steady 200 (0.25x)"),
        "{}",
        opened.detail
    );
}

/// A threatened city of ours still holds the war, under the gene's tag.
#[test]
fn a_threatened_city_still_holds_it() {
    let (mut g, mut ai, plan, leaning) = own_flip(50.0, 200.0);
    ai.enable_own_flip_faith_war_asks_no_power();
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
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("threatened")
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "a threatened city of ours");
    let civ = g.players[1].civ.clone();
    assert!(
        ai.journal()
            .since(0)
            .thoughts
            .iter()
            .any(
                |thought| thought.headline == format!("Holding the faith war on {civ}")
                    && thought
                        .detail
                        .starts_with("own-flip-faith-war-asks-no-power: ")
                    && thought.detail.contains("a city of ours is threatened")
            ),
        "the hold is journalled under the gene's tag"
    );
}

/// The treasury reserve and the income gate stay on the gene's war.
#[test]
fn the_income_gate_still_holds_it() {
    let (mut g, mut ai, plan, _) = own_flip(50.0, 200.0);
    ai.enable_own_flip_faith_war_asks_no_power();
    g.players[0].gold_per_turn = -3.0;
    assert_eq!(
        ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate),
        Some("income")
    );
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "negative income");
}

/// The shipped own-flip thresholds are unchanged, and the shipped reading
/// still asks them; under the gene the reading applies from two cities, and
/// one leaning city opens it only while a spreader of the faith stands at
/// our cities. At half the founder's power or more the shipped reading
/// stands and the gene is not what opens it.
#[test]
fn the_shipped_thresholds_stay_and_one_leaning_city_needs_a_spreader() {
    assert_eq!(
        (
            SMALL_WORLD_OWN_CITIES,
            SMALL_WORLD_OWN_LEANING,
            SMALL_WORLD_OWN_SHARE
        ),
        (3, 2, 50)
    );
    assert!((SMALL_WORLD_OWN_POWER - 0.5).abs() < 1e-9);
    assert_eq!((OWN_FLIP_CITIES, OWN_FLIP_LEANING_WITH_A_SPREADER), (2, 1));

    let one_of_three = |on: bool, spreader: bool| {
        let (mut g, mut ai, _, _) = own_flip(50.0, 200.0);
        if on {
            ai.enable_own_flip_faith_war_asks_no_power();
        }
        let second = g
            .player_city_ids(0)
            .into_iter()
            .find(|cid| g.cities[cid].pos == (7, 8))
            .unwrap();
        g.cities.get_mut(&second).unwrap().pressure.clear();
        if !spreader {
            let missionary = g
                .units
                .values()
                .find(|unit| unit.kind == "missionary")
                .map(|unit| unit.id)
                .unwrap();
            g.units.get_mut(&missionary).unwrap().pos = (24, 8);
        }
        (
            ai.faith_war_small_world(&g, 0, 1),
            ai.own_flip_reading(&g, 0, 1),
        )
    };
    assert_eq!(one_of_three(false, true), (None, false), "off: as today");
    assert_eq!(
        one_of_three(true, true),
        (Some(READING), true),
        "with a spreader"
    );
    assert_eq!(one_of_three(true, false), (None, false), "without one");

    // Two leaning of five (40%): only with a spreader at our cities.
    let two_of_five = |spreader: bool| {
        let (mut g, mut ai, _, _) = own_flip(50.0, 200.0);
        ai.enable_own_flip_faith_war_asks_no_power();
        g.found_city_for(0, (2, 14), None);
        g.found_city_for(0, (8, 14), None);
        assert_eq!(g.player_city_ids(0).len(), 5);
        if !spreader {
            let missionary = g
                .units
                .values()
                .find(|unit| unit.kind == "missionary")
                .map(|unit| unit.id)
                .unwrap();
            g.units.get_mut(&missionary).unwrap().pos = (24, 8);
        }
        ai.faith_war_small_world(&g, 0, 1)
    };
    assert_eq!(two_of_five(false), None, "two of five, no spreader");
    assert_eq!(two_of_five(true), Some(READING), "two of five, a spreader");

    let (g, mut ai, _, _) = own_flip(120.0, 200.0);
    ai.enable_own_flip_faith_war_asks_no_power();
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), Some(READING));
    assert!(ai.own_flip_reading(&g, 0, 1));
    assert_eq!(
        ai.own_flip_asks_no_power(&g, 0, 1),
        None,
        "the shipped reading stands at 0.6x"
    );

    // Without `faith-war-on-a-small-world` the gene reads nothing.
    let (g, mut ai, _, _) = own_flip(50.0, 200.0);
    ai.enable_own_flip_faith_war_asks_no_power();
    ai.disable_faith_war_on_a_small_world();
    assert!(!ai.own_flip_reading(&g, 0, 1));
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None);
}

/// Two cities of ours, one following the faith, its Missionary at ours, at
/// a quarter of the founder's power (civvis-20261112T142558Z's shape at
/// t44): off, held as today; on, the war opens on the gene's reading.
#[test]
fn opens_from_two_cities_on_one_leaning_city_with_a_spreader() {
    let (mut g, mut ai, plan, _) = fixture(50.0, 200.0, false, 2);
    assert!(!g.civ_follows_religion(0, FAITH), "one of two holds out");
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None, "off: no reading");
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "off: held as today");

    let (mut g, mut ai, plan, _) = fixture(50.0, 200.0, false, 2);
    ai.enable_own_flip_faith_war_asks_no_power();
    ai.base.journal = crate::reasoning::Journal::recording();
    assert!(ai.own_flip_reading(&g, 0, 1));
    assert_eq!(ai.own_flip_asks_no_power(&g, 0, 1), Some(0.25));
    assert_eq!(ai.faith_war_gate(&g, 0, 1), None);
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(g.is_at_war(0, 1), "on: opened from two cities");
    let civ = g.players[1].civ.clone();
    let thoughts = ai.journal().since(0).thoughts;
    let opened = thoughts
        .iter()
        .find(|thought| thought.headline == format!("Opening the faith war on {civ}"))
        .expect("the opening is journalled");
    assert!(
        opened.detail.starts_with(
            "own-flip-faith-war-asks-no-power: only its founder follows its faith and 1 of our 2 cities lean to it (from 2 cities, 1 of its spreader(s) at ours;"
        ),
        "{}",
        opened.detail
    );

    // Without a spreader at our cities one leaning city of two reads nothing.
    let (mut g, mut ai, _, _) = fixture(50.0, 200.0, false, 2);
    ai.enable_own_flip_faith_war_asks_no_power();
    let missionary = g
        .units
        .values()
        .find(|unit| unit.kind == "missionary")
        .map(|unit| unit.id)
        .unwrap();
    g.units.get_mut(&missionary).unwrap().pos = (24, 8);
    assert!(!ai.own_flip_reading(&g, 0, 1));
}

/// civvis-20261112T160320Z's shape at t33: one leaning city of three, a
/// Missionary at ours, their steady 118 against our 112. Off, the edge
/// holds the war; on, it opens, and the line reads the ratio without the
/// own-flip floor it is not under.
#[test]
fn one_leaning_city_of_three_with_a_spreader_opens_it_at_parity() {
    let opened = |on: bool| {
        let (mut g, mut ai, plan, _) = own_flip(112.0, 118.0);
        let second = g
            .player_city_ids(0)
            .into_iter()
            .find(|cid| g.cities[cid].pos == (7, 8))
            .unwrap();
        g.cities.get_mut(&second).unwrap().pressure.clear();
        if on {
            ai.enable_own_flip_faith_war_asks_no_power();
        }
        ai.base.journal = crate::reasoning::Journal::recording();
        let gate = ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate);
        ai.advanced_diplomacy(&mut g, 0, &plan);
        let civ = g.players[1].civ.clone();
        let line = ai
            .journal()
            .since(0)
            .thoughts
            .iter()
            .find(|thought| thought.headline == format!("Opening the faith war on {civ}"))
            .map(|thought| thought.detail.clone());
        (gate, g.is_at_war(0, 1), line)
    };
    let (gate, at_war, _) = opened(false);
    assert_eq!((gate, at_war), (Some("edge"), false), "off: the edge holds");
    let (gate, at_war, line) = opened(true);
    assert_eq!((gate, at_war), (None, true), "on: opened at parity");
    let line = line.expect("the opening is journalled");
    assert!(
        line.contains("ours 112 against their steady 118 (0.95x)") && !line.contains("under the"),
        "{line}"
    );
}

/// Barbarians at a city of ours no longer hold a small-world war under the
/// gene, on the shipped reading (0.6x) or on the gene's own (0.25x); off,
/// the shipped reading is held on the threat. A major's army still holds it
/// (`a_threatened_city_still_holds_it`).
#[test]
fn a_barbarian_threat_does_not_hold_it() {
    let threatened = |on: bool, ours: f64| {
        let (mut g, mut ai, plan, leaning) = fixture(ours, 200.0, true, 3);
        if on {
            ai.enable_own_flip_faith_war_asks_no_power();
        }
        ai.base.journal = crate::reasoning::Journal::recording();
        let barbarian = g.barb_pid.expect("the world's barbarians");
        let at = g.cities[&leaning].pos;
        for step in [(1, 0), (1, 1), (0, 1), (-1, 1)] {
            g.spawn_test_unit("swordsman", barbarian, (at.0 + step.0, at.1 + step.1));
        }
        assert!(ai.threatened_city(&g, 0).is_some(), "fixture: threatened");
        assert!(!ai.threat_is_a_majors_army(&g, 0), "barbarians only");
        let gate = ai.faith_war_gate(&g, 0, 1).map(|(gate, _)| gate);
        ai.advanced_diplomacy(&mut g, 0, &plan);
        let civ = g.players[1].civ.clone();
        let waived = ai.journal().since(0).thoughts.iter().any(|thought| {
            thought.headline == format!("Waiving the threatened hold on the faith war on {civ}")
                && thought
                    .detail
                    .starts_with("own-flip-faith-war-asks-no-power: a city of ours is threatened, but by no major's army")
        });
        (gate, g.is_at_war(0, 1), waived)
    };
    assert_eq!(
        threatened(false, 120.0),
        (Some("threatened"), false, false),
        "off: held on the barbarians"
    );
    assert_eq!(threatened(true, 120.0), (None, true, true), "on, 0.6x");
    assert_eq!(threatened(true, 50.0), (None, true, true), "on, 0.25x");
}

/// When the host allows no war on the founder yet, the gene denounces it.
#[test]
fn a_founder_the_host_shields_is_denounced_first() {
    let denounced = |on: bool| {
        let (mut g, mut ai, plan, _) = own_flip(50.0, 200.0);
        if on {
            ai.enable_own_flip_faith_war_asks_no_power();
        }
        ai.base.journal = crate::reasoning::Journal::recording();
        Arc::make_mut(&mut g.host_war_blocks).insert((0, 1, g.turn));
        ai.advanced_diplomacy(&mut g, 0, &plan);
        assert!(!g.is_at_war(0, 1), "the host refuses the war");
        let civ = g.players[1].civ.clone();
        let line = ai.journal().since(0).thoughts.iter().any(|thought| {
            thought.headline == format!("Denouncing {civ} for the faith war")
                && thought
                    .detail
                    .starts_with("own-flip-faith-war-asks-no-power: ")
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

/// The declaration desk's counter-war floor does not hold the founder on
/// the gene's reading.
#[test]
fn the_counter_floor_does_not_hold_the_founder() {
    let (g, mut ai, _, _) = own_flip(50.0, 200.0);
    ai.enable_counter_war_needs_parity();
    assert!(
        ai.counter_war_hopeless(&g, 0, 1),
        "fixture: under the floor"
    );
    assert!(ai.counter_floor_holds(&g, 0, 1, true), "off: held");
    ai.enable_own_flip_faith_war_asks_no_power();
    assert!(!ai.counter_floor_holds(&g, 0, 1, true), "on: not held");
}

/// The faith war on record at turn 90, opened at 70, at 100 against 300
/// (the outmatched peace is due): no spreader at our cities since 70, no
/// city of ours converting, and our first city following the faith unless
/// `following` is false (civvis-20261112T160812Z at t74).
fn kept_war(on: bool, following: bool) -> (Game, AdvancedAi, StrategicPlan) {
    let (mut g, mut ai, mut plan, leaning) = own_flip(100.0, 300.0);
    if on {
        ai.enable_own_flip_faith_war_asks_no_power();
    }
    let second = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| g.cities[cid].pos == (7, 8))
        .unwrap();
    g.cities.get_mut(&second).unwrap().pressure.clear();
    if !following {
        let city = g.cities.get_mut(&leaning).unwrap();
        city.pressure.clear();
        city.atheist_pressure = 50.0;
    }
    let missionary = g
        .units
        .values()
        .find(|unit| unit.kind == "missionary")
        .map(|unit| unit.id)
        .unwrap();
    g.units.get_mut(&missionary).unwrap().pos = (24, 8);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.turn = 90;
    ai.faith_war_before_the_flip_war = Some(FaithWar {
        rival: 1,
        opened: 70,
        spreader_seen: 70,
    });
    plan.strategy = GrandStrategy::Conquest;
    (g, ai, plan)
}

/// A city of ours following the faith keeps the war under the gene: no
/// peace is offered and the keep is journalled; off, or with no city on the
/// faith, the outmatched peace is offered as today; a falling city of ours
/// still releases it.
#[test]
fn a_city_on_the_faith_keeps_the_faith_war() {
    let (mut g, mut ai, plan) = kept_war(true, true);
    ai.base.journal = crate::reasoning::Journal::recording();
    assert_eq!(ai.own_flip_keeps(&g, 0, 1), Some((1, 20)));
    assert!(ai.faith_war_keeps_the_war(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!ai.peace_offers.contains(&1), "on: no peace offered");
    let civ = g.players[1].civ.clone();
    assert!(
        ai.journal().since(0).thoughts.iter().any(|thought| thought.headline
            == format!("Keeping the faith war with {civ}")
            && thought.detail.starts_with(
                "own-flip-faith-war-asks-no-power: 1 of our 3 cities follow or are converting to its faith"
            )),
        "the keep is journalled"
    );

    let (mut g, mut ai, plan) = kept_war(false, true);
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1), "off: not kept");
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1), "off: the peace is offered");

    let (mut g, mut ai, plan) = kept_war(true, false);
    assert_eq!(ai.own_flip_keeps(&g, 0, 1), None, "no city on the faith");
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(ai.peace_offers.contains(&1), "the family's exit as today");

    let (mut g, ai, _) = kept_war(true, true);
    let turn = g.turn;
    let city = g.player_city_ids(0)[1];
    let city = g.cities.get_mut(&city).unwrap();
    city.last_attacked = turn;
    city.hp = 80;
    assert!(AdvancedAi::our_cities_falling(&g, 0));
    assert!(!ai.faith_war_keeps_the_war(&g, 0, 1), "falling: released");
}

/// The founder's own peace offer is refused while a city of ours follows
/// its faith under the gene, and taken off.
#[test]
fn the_founders_peace_is_refused_while_a_city_follows_its_faith() {
    let refused = |on: bool| {
        let (mut g, mut ai, plan) = kept_war(on, true);
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
    assert!(refused(true), "on: refused");
    assert!(!refused(false), "off: taken");
}
