use super::*;
use crate::ai::advanced::faith_war_before_the_flip::SmallWorldRace;
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
    let converting = g.found_city_for(0, (7, 8), None);
    g.found_city_for(0, (12, 8), None);
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
    let city = g.cities.get_mut(&converting).unwrap();
    city.atheist_pressure = 1_000.0;
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

/// The own-flip thresholds are the shipped ones: one leaning city of three,
/// or two of five, is no reading at any power; at half the founder's power
/// or more the shipped reading stands and the gene is not what opens it.
#[test]
fn the_own_flip_thresholds_are_unchanged() {
    assert_eq!(
        (
            SMALL_WORLD_OWN_CITIES,
            SMALL_WORLD_OWN_LEANING,
            SMALL_WORLD_OWN_SHARE
        ),
        (3, 2, 50)
    );
    assert!((SMALL_WORLD_OWN_POWER - 0.5).abs() < 1e-9);

    let (mut g, mut ai, plan, _) = own_flip(50.0, 200.0);
    ai.enable_own_flip_faith_war_asks_no_power();
    let converting = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| g.cities[cid].pos == (7, 8))
        .unwrap();
    g.cities.get_mut(&converting).unwrap().pressure.clear();
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None, "one of three");
    assert!(!ai.own_flip_reading(&g, 0, 1));
    ai.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "one of three: held as today");

    let (mut g, mut ai, _, _) = own_flip(50.0, 200.0);
    ai.enable_own_flip_faith_war_asks_no_power();
    g.found_city_for(0, (2, 14), None);
    g.found_city_for(0, (8, 14), None);
    assert_eq!(g.player_city_ids(0).len(), 5);
    assert_eq!(ai.faith_war_small_world(&g, 0, 1), None, "two of five");

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
