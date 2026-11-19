//! `culture-theater-buildings-pass-the-claims`: unit tests.
use super::*;
use crate::rules::Yields;
use std::sync::Arc;

const TAG: &str = "culture-theater-buildings-pass-the-claims:";

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn head(g: &Game, cid: u32) -> Option<Item> {
    g.cities[&cid].queue.first().cloned()
}

fn plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    }
}

/// Set `cid`'s Production to `production` a turn.
fn set_production(g: &mut Game, cid: u32, production: f64) {
    Arc::make_mut(&mut g.observed_city_yield_adjustments).remove(&cid);
    let base = g.city_yields(cid).production;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            production: production - base,
            ..Default::default()
        },
    );
}

/// One city with a Theater Square (and its Amphitheater when `amphitheater`),
/// Drama and Poetry and Humanism known, making `production` a turn, with a
/// Campus district at the head of its queue and Gold in the bank.
fn board(amphitheater: bool, production: f64) -> (Game, u32, Item) {
    let mut g = Game::new_full(
        1,
        44,
        26,
        crate::rng::fixture_seed("CULTURETHEATERBUILDINGS", 1),
        250,
        0,
        false,
    );
    g.players[0].civ = "Gran Colombia".to_string();
    g.players[0].gold = 1_000.0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    crate::game::install_test_district(&mut g, cid, "theater_square");
    for civic in ["code_of_laws", "drama_poetry", "humanism"] {
        g.players[0].civics.insert(Name::new(civic));
    }
    if amphitheater {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
    }
    let pos = g.cities[&cid].pos;
    let campus = Item::District {
        district: crate::name!("campus"),
        pos,
    };
    g.cities.get_mut(&cid).unwrap().queue = vec![campus.clone()];
    set_production(&mut g, cid, production);
    (g, cid, campus)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_culture_theater_buildings_pass_the_claims();
    }
    ai
}

/// Run the claim with a recording journal and return its thoughts' details.
fn claim(ai: &mut AdvancedAi, g: &mut Game) -> Vec<String> {
    let journal = crate::reasoning::Journal::recording();
    journal.begin_turn(g.turn, 0);
    ai.attach_journal(journal.handle());
    let plan = plan(g);
    ai.claim_culture_theater_buildings(g, 0, &plan);
    journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| thought.detail)
        .collect()
}

#[test]
fn it_is_an_opt_in_off_by_default() {
    assert!(
        !AdvancedAi::targeting(VictoryTarget::Culture).culture_theater_buildings_pass_the_claims
    );
}

#[test]
fn the_museum_goes_ahead_of_a_campus() {
    let (mut g, cid, _) = board(true, 20.0);
    let museum = building("art_museum");
    assert!(g.can_produce(0, cid, &museum));
    let mut ai = seat(VictoryTarget::Culture, true);
    assert_eq!(
        ai.culture_theater_building_next(&g, 0, cid),
        Some(museum.clone())
    );
    let lines = claim(&mut ai, &mut g);
    assert_eq!(head(&g, cid), Some(museum));
    assert!(lines.iter().any(|line| line.starts_with(TAG)), "{lines:?}");
    assert!(
        lines.iter().any(|line| line.contains("the Campus")),
        "{lines:?}"
    );
}

#[test]
fn the_amphitheater_comes_before_the_museum() {
    let (mut g, cid, _) = board(false, 20.0);
    let mut ai = seat(VictoryTarget::Culture, true);
    claim(&mut ai, &mut g);
    assert_eq!(head(&g, cid), Some(building("amphitheater")));
}

#[test]
fn a_builder_keeps_its_queue() {
    let (mut g, cid, _) = board(true, 20.0);
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.cities.get_mut(&cid).unwrap().queue = vec![builder.clone()];
    let mut ai = seat(VictoryTarget::Culture, true);
    let lines = claim(&mut ai, &mut g);
    assert_eq!(head(&g, cid), Some(builder));
    assert!(lines.is_empty(), "{lines:?}");
}

#[test]
fn a_slow_museum_off_or_another_lane_moves_nothing() {
    let (mut g, cid, campus) = board(true, 1.0);
    let mut ai = seat(VictoryTarget::Culture, true);
    claim(&mut ai, &mut g);
    assert_eq!(head(&g, cid), Some(campus.clone()), "a slow Museum waits");

    let (mut g, cid, campus) = board(true, 20.0);
    let mut ai = seat(VictoryTarget::Culture, false);
    claim(&mut ai, &mut g);
    assert_eq!(head(&g, cid), Some(campus), "off is inert");

    let (mut g, cid, campus) = board(true, 20.0);
    let mut ai = seat(VictoryTarget::Science, true);
    claim(&mut ai, &mut g);
    assert_eq!(
        head(&g, cid),
        Some(campus),
        "a Science seat keeps its Campus"
    );

    let (mut g, cid, campus) = board(true, 20.0);
    let mut ai = seat(VictoryTarget::Culture, true);
    ai.lane_lost = true;
    claim(&mut ai, &mut g);
    assert_eq!(
        head(&g, cid),
        Some(campus),
        "a released lane keeps its Campus"
    );
}
