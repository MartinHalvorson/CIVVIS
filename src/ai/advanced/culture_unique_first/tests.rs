use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::name;

#[test]
fn culture_road_takes_the_unique_first_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("culture-road-takes-the-unique-first", |ai| {
        ai.culture_road_takes_the_unique_first
    });
}

/// One founded capital of `civ` with every prerequisite of `open` known.
fn seat(civ: &str, open: &str, seed: u64) -> (Game, u32) {
    let mut game = Game::new_full(
        1,
        24,
        16,
        crate::rng::fixture_seed("UNIQUEFIRST", seed),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    game.players[0].civ = civ.to_string();
    let mut stack: Vec<Name> = game.rules.civics[open].requires.clone();
    while let Some(known) = stack.pop() {
        if game.players[0].civics.insert(known) {
            stack.extend(game.rules.civics[known.as_str()].requires.iter().copied());
        }
    }
    let cid = game.player_city_ids(0)[0];
    (game, cid)
}

fn culture_ai(enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if enabled {
        ai.enable_culture_road_takes_the_unique_first();
    }
    ai
}

#[test]
fn sweden_reads_the_open_air_museum_at_nationalism() {
    let (g, _) = seat("Sweden", "nationalism", 51_001);
    assert_eq!(
        AdvancedAi::unique_culture_source(&g, 0),
        Some((
            UniqueSource::Improvement {
                name: name!("open_air_museum"),
                one_per_city: true
            },
            name!("nationalism")
        ))
    );
}

#[test]
fn a_civilization_with_no_unique_culture_source_changes_nothing() {
    let (g, _) = seat("Rome", "nationalism", 51_002);
    assert_eq!(AdvancedAi::unique_culture_source(&g, 0), None);
    assert_eq!(culture_ai(true).unique_first_goal(&g, 0), None);
}

#[test]
fn the_unlock_goes_first_from_the_tier_two_rung() {
    let (mut g, _) = seat("Sweden", "nationalism", 51_003);
    let ai = culture_ai(true);
    let slots = g.gov_slots(0);
    let capacity = slots.military + slots.economic + slots.diplomatic + slots.wildcard;
    if capacity < UNIQUE_FIRST_SLOTS {
        assert_eq!(
            ai.unique_first_goal(&g, 0),
            None,
            "a four-slot government climbs first"
        );
        let monarchy = g
            .rules
            .governments
            .iter()
            .find(|(_, spec)| {
                spec.slots.military
                    + spec.slots.economic
                    + spec.slots.diplomatic
                    + spec.slots.wildcard
                    >= UNIQUE_FIRST_SLOTS
            })
            .map(|(name, _)| *name)
            .expect("a six-slot government");
        g.players[0].government = Some(monarchy.as_str().to_string());
    }
    assert_eq!(ai.unique_first_goal(&g, 0), Some("nationalism"));
    assert_eq!(culture_ai(false).unique_first_goal(&g, 0), None, "off");
    g.players[0].civics.insert(name!("nationalism"));
    assert_eq!(ai.unique_first_goal(&g, 0), None, "known");
}

#[test]
fn builders_are_queued_as_nationalism_lands() {
    let (mut g, cid) = seat("Sweden", "nationalism", 51_004);
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            culture: 500.0,
            production: 20.0,
            ..Default::default()
        },
    );
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.players[0].civic = Some("nationalism".to_string());
    let ai = culture_ai(true);
    let plan = ai.assess(&g, 0);
    ai.claim_unique_source_builders(&mut g, 0, &plan);
    let builder = Item::Unit {
        unit: name!("builder"),
    };
    assert_eq!(g.cities[&cid].queue.first(), Some(&builder));
    // One Builder covers the lone city: nothing more is queued.
    ai.claim_unique_source_builders(&mut g, 0, &plan);
    assert_eq!(
        g.cities[&cid]
            .queue
            .iter()
            .filter(|item| **item == builder)
            .count(),
        1
    );
}

#[test]
fn charges_on_hand_or_a_distant_civic_queue_nothing() {
    let (mut g, cid) = seat("Sweden", "nationalism", 51_005);
    g.cities.get_mut(&cid).unwrap().queue.clear();
    g.players[0].civic = Some("nationalism".to_string());
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            culture: 1.0,
            production: 20.0,
            ..Default::default()
        },
    );
    let ai = culture_ai(true);
    let plan = ai.assess(&g, 0);
    ai.claim_unique_source_builders(&mut g, 0, &plan);
    assert!(g.cities[&cid].queue.is_empty(), "Nationalism is far off");
    let pos = g.cities[&cid].pos;
    let builder = g.spawn_unit("builder", 0, pos);
    g.units.get_mut(&builder).unwrap().charges = 3;
    g.players[0].civics.insert(name!("nationalism"));
    ai.claim_unique_source_builders(&mut g, 0, &plan);
    assert!(
        g.cities[&cid].queue.is_empty(),
        "the charges on hand cover it"
    );
}
