use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn theaters_without_writing_slots() -> (Game, Vec<u32>, Item) {
    let (mut game, cities) = theater_less_empire();
    let amphitheater = Item::Building {
        building: crate::name!("amphitheater"),
    };
    for cid in &cities {
        crate::game::install_test_district(&mut game, *cid, "theater_square");
        game.cities.get_mut(cid).unwrap().queue = vec![unit("warrior")];
        assert!(game.can_produce(0, *cid, &amphitheater));
    }
    (game, cities, amphitheater)
}

#[test]
fn theaters_without_slots_claim_exactly_one_first_amphitheater() {
    let (mut game, cities, amphitheater) = theaters_without_writing_slots();
    let ai = seat(VictoryTarget::Culture, true);
    let plan = ai.assess(&game, 0);
    ai.claim_culture_theaters(&mut game, 0, &plan);
    let claimed: Vec<_> = cities
        .iter()
        .filter(|cid| game.cities[cid].queue.first() == Some(&amphitheater))
        .copied()
        .collect();
    assert_eq!(
        claimed.len(),
        1,
        "a Theater district is not yet a Writing slot"
    );
    assert!(
        ai.culture_theater_holds(&game, 0, claimed[0], &plan, &amphitheater),
        "the delegated production scorer must retain the new building"
    );
    let queues: Vec<_> = cities
        .iter()
        .map(|cid| game.cities[cid].queue.clone())
        .collect();
    ai.claim_culture_theaters(&mut game, 0, &plan);
    assert_eq!(
        queues,
        cities
            .iter()
            .map(|cid| game.cities[cid].queue.clone())
            .collect::<Vec<_>>(),
        "a queued Amphitheater does not claim another city"
    );
}

#[test]
fn first_writing_slots_do_not_override_survival_settlers_or_finished_work() {
    let (mut game, cities, amphitheater) = theaters_without_writing_slots();
    let ai = seat(VictoryTarget::Culture, true);
    game.cities.get_mut(&cities[0]).unwrap().queue = vec![unit("settler")];
    game.cities.get_mut(&cities[1]).unwrap().queue = vec![Item::Building {
        building: crate::name!("walls"),
    }];
    let mut plan = ai.assess(&game, 0);
    plan.threatened_city = Some(cities[2]);
    ai.claim_culture_theaters(&mut game, 0, &plan);
    assert!(cities
        .iter()
        .all(|cid| !game.cities[cid].queue.contains(&amphitheater)));
    plan.threatened_city = None;
    let cost = game.item_cost_for_city(0, cities[2], &unit("warrior"));
    game.cities.get_mut(&cities[2]).unwrap().production = cost - 1.0;
    ai.claim_culture_theaters(&mut game, 0, &plan);
    assert!(
        cities
            .iter()
            .all(|cid| !game.cities[cid].queue.contains(&amphitheater)),
        "the remaining eligible city finishes its current unit first"
    );
}

#[test]
fn other_lanes_and_disabled_gene_leave_routine_production_alone() {
    let (game, cities, _) = theaters_without_writing_slots();
    for ai in [
        seat(VictoryTarget::Science, true),
        seat(VictoryTarget::Culture, false),
    ] {
        let mut other = game.clone();
        claim(&ai, &mut other);
        assert!(cities
            .iter()
            .all(|cid| other.cities[cid].queue == vec![unit("warrior")]));
    }
}

#[test]
fn completed_writing_slots_admit_one_fast_followup_through_recovery() {
    for version_two in [false, true] {
        let (mut game, cities, amphitheater) = theaters_without_writing_slots();
        game.cities
            .get_mut(&cities[0])
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
        Arc::make_mut(&mut game.observed_city_yield_adjustments)
            .get_mut(&cities[2])
            .unwrap()
            .production = 30.0;
        let mut ai = seat(VictoryTarget::Culture, true);
        if version_two {
            ai.enable_culture_seat_theater_first_2();
        }
        let mut plan = ai.assess(&game, 0);
        plan.strategy = GrandStrategy::Recovery;
        ai.claim_culture_theaters(&mut game, 0, &plan);
        assert_eq!(game.cities[&cities[2]].queue.first(), Some(&amphitheater));
        assert_eq!(
            game.cities[&cities[1]].queue.first(),
            Some(&unit("warrior"))
        );
        assert!(ai.culture_theater_holds(&game, 0, cities[2], &plan, &amphitheater));
        ai.claim_culture_theaters(&mut game, 0, &plan);
        assert_eq!(
            game.cities[&cities[1]].queue.first(),
            Some(&unit("warrior")),
            "one queued followup at a time"
        );
        game.cities
            .get_mut(&cities[2])
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
        game.cities.get_mut(&cities[2]).unwrap().queue.clear();
        ai.claim_culture_theaters(&mut game, 0, &plan);
        assert_eq!(
            game.cities[&cities[1]].queue.first(),
            Some(&amphitheater),
            "the next Theater is completed after the first followup stands"
        );
    }
}

#[test]
fn followup_writing_slots_respect_slow_builds_survival_debt_and_remaining_clock() {
    for why in ["slow", "attacked", "debt", "clock", "survival"] {
        let (mut game, cities, amphitheater) = theaters_without_writing_slots();
        game.cities
            .get_mut(&cities[0])
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
        let ai = seat(VictoryTarget::Culture, true);
        let mut plan = ai.assess(&game, 0);
        plan.threatened_city = Some(cities[1]);
        match why {
            "slow" => {
                Arc::make_mut(&mut game.observed_city_yield_adjustments)
                    .get_mut(&cities[2])
                    .unwrap()
                    .production = -100.0
            }
            "attacked" => game.cities.get_mut(&cities[2]).unwrap().last_attacked = game.turn - 1,
            "debt" => {
                game.players[0].gold_per_turn = -10.0;
                game.players[0].gold = 0.0;
            }
            "clock" => game.turn = game.max_turns - 10,
            "survival" => game.cities.get_mut(&cities[2]).unwrap().queue = vec![unit("settler")],
            _ => unreachable!(),
        }
        ai.claim_culture_theaters(&mut game, 0, &plan);
        assert!(
            cities
                .iter()
                .all(|cid| !game.cities[cid].queue.contains(&amphitheater)),
            "{why}"
        );
    }
}

#[test]
fn a_queued_followup_releases_its_hold_when_threatened() {
    let (mut game, cities, amphitheater) = theaters_without_writing_slots();
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    let ai = seat(VictoryTarget::Culture, true);
    let mut plan = ai.assess(&game, 0);
    game.cities.get_mut(&cities[1]).unwrap().queue = vec![amphitheater.clone()];
    assert!(ai.culture_theater_holds(&game, 0, cities[1], &plan, &amphitheater));
    plan.threatened_city = Some(cities[1]);
    assert!(!ai.culture_theater_holds(&game, 0, cities[1], &plan, &amphitheater));
    plan.threatened_city = None;
    game.cities.get_mut(&cities[1]).unwrap().last_attacked = game.turn - 1;
    assert!(!ai.culture_theater_holds(&game, 0, cities[1], &plan, &amphitheater));
}

/// Three cities of four citizens with Drama and Poetry and Writing in hand,
/// production 10, no district but the centre: every city has a free slot and
/// can build its first Theater Square and its first Campus.
fn theater_less_empire() -> (Game, Vec<u32>) {
    let mut game = Game::new_full(
        1,
        44,
        26,
        crate::rng::fixture_seed("CULTURETHEATER", 1),
        250,
        0,
        false,
    );
    // A plain Theater Square: Greece's Acropolis needs hills the fixture
    // does not promise, and Rome founds with a Monument.
    game.players[0].civ = "Gran Colombia".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    while game.player_city_ids(0).len() < 3 {
        let pos = game
            .map
            .tiles
            .values()
            .filter(|tile| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && tile.district.is_none()
                    && tile.wonder.is_none()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, tile.pos) >= 5)
                    && game
                        .cities
                        .values()
                        .any(|city| game.wdist(city.pos, tile.pos) <= 8)
            })
            .map(|tile| tile.pos)
            .min()
            .expect("room for three cities");
        game.found_city_for(0, pos, None);
    }
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    game.game_speed = GameSpeed::Online;
    game.turn = 70;
    // Forest, jungle and marsh sites need their removal technologies.
    for tech in [
        "writing",
        "mining",
        "bronze_working",
        "irrigation",
        "pottery",
        "animal_husbandry",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.players[0].civics.insert(Name::new("drama_poetry"));
    let cities = game.player_city_ids(0);
    for cid in &cities {
        game.cities.get_mut(cid).unwrap().pop = 4;
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                // Low enough that a routine Warrior does not finish this turn.
                production: 10.0,
                ..Default::default()
            },
        );
        assert!(
            BasicAi::first_theater_item(&game, 0, *cid).is_some(),
            "fixture: the first Theater Square is buildable"
        );
    }
    (game, cities)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_culture_seat_theater_first();
    }
    ai
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn is_theater(game: &Game, item: Option<&Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "theater_square")
}

fn claim(ai: &AdvancedAi, game: &mut Game) {
    let plan = ai.assess(game, 0);
    ai.claim_culture_theaters(game, 0, &plan);
}

/// A Culture seat's cities training Warriors put the Theater first; off, or
/// for a Domination seat, nothing moves; a Settler keeps its queue.
#[test]
fn a_culture_seat_claims_its_theaters_over_routine_units() {
    let (game, cities) = theater_less_empire();
    let mut busy = game.clone();
    for cid in &cities {
        busy.cities.get_mut(cid).unwrap().queue = vec![unit("warrior")];
    }

    for (ai, why) in [
        (seat(VictoryTarget::Culture, false), "off"),
        (seat(VictoryTarget::Domination, true), "a Domination seat"),
    ] {
        let mut kept = busy.clone();
        claim(&ai, &mut kept);
        for cid in &cities {
            assert_eq!(
                kept.cities[cid].queue.first(),
                Some(&unit("warrior")),
                "{why}: no claim"
            );
        }
    }

    let ai = seat(VictoryTarget::Culture, true);
    let mut on = busy.clone();
    claim(&ai, &mut on);
    let plan = ai.assess(&on, 0);
    for cid in &cities {
        let head = on.cities[cid].queue.first().cloned();
        assert!(is_theater(&on, head.as_ref()), "the Theater goes first");
        assert!(ai.culture_theater_holds(&on, 0, *cid, &plan, head.as_ref().unwrap()));
    }

    let mut pump = busy.clone();
    pump.cities.get_mut(&cities[0]).unwrap().queue = vec![unit("settler")];
    claim(&ai, &mut pump);
    assert_eq!(
        pump.cities[&cities[0]].queue.first(),
        Some(&unit("settler")),
        "a Settler keeps its queue"
    );
}

/// The free-slot Campus claim yields a Culture seat's city to its Theater;
/// with the gene off it takes the slot as before.
#[test]
fn the_free_slot_campus_yields_to_the_theater() {
    let (game, cities) = theater_less_empire();
    let mut on = seat(VictoryTarget::Culture, true);
    on.enable_campus_claims_the_free_slot_2();
    let plan = on.assess(&game, 0);
    assert!(on
        .free_slot_campus_item(&game, 0, cities[0], &plan)
        .is_none());

    let mut off = seat(VictoryTarget::Culture, false);
    off.enable_campus_claims_the_free_slot_2();
    let plan = off.assess(&game, 0);
    assert!(off
        .free_slot_campus_item(&game, 0, cities[0], &plan)
        .is_some());
}

fn seat_two() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_culture_seat_theater_first_2();
    ai
}

/// Version two: with no Campus the empire claims one Theater, then the rest
/// wait; two standing Campuses admit a second.
#[test]
fn version_two_balances_theaters_against_campuses() {
    let (game, cities) = theater_less_empire();
    let mut busy = game.clone();
    for cid in &cities {
        busy.cities.get_mut(cid).unwrap().queue = vec![unit("warrior")];
    }
    let ai = seat_two();
    assert!(!ai.culture_seat_theater_first, "version two turns one off");

    let mut none = busy.clone();
    claim(&ai, &mut none);
    let claimed = cities
        .iter()
        .filter(|cid| is_theater(&none, none.cities[cid].queue.first()))
        .count();
    assert_eq!(claimed, 1, "the first Theater always goes ahead");

    let mut two_campuses = busy.clone();
    for cid in &cities[1..] {
        crate::game::install_test_district(&mut two_campuses, *cid, "campus");
    }
    claim(&ai, &mut two_campuses);
    let claimed = cities
        .iter()
        .filter(|cid| is_theater(&two_campuses, two_campuses.cities[cid].queue.first()))
        .count();
    assert_eq!(claimed, 2, "two Campuses admit two Theaters");
}

/// Version two: once the Theaters lead, the free-slot Campus keeps the slot.
#[test]
fn version_two_returns_the_slot_to_the_campus() {
    let (mut game, cities) = theater_less_empire();
    let theater = BasicAi::first_theater_item(&game, 0, cities[0]).unwrap();
    game.cities.get_mut(&cities[0]).unwrap().queue = vec![theater];
    let mut ai = seat_two();
    ai.enable_campus_claims_the_free_slot_2();
    let plan = ai.assess(&game, 0);

    assert!(ai
        .culture_theater_item(&game, 0, cities[1], &plan)
        .is_none());
    assert!(ai
        .free_slot_campus_item(&game, 0, cities[1], &plan)
        .is_some());
}

fn theaters_ready_for_art() -> (Game, Vec<u32>, Item) {
    let (mut game, cities, _) = theaters_without_writing_slots();
    game.players[0].civics.insert(crate::name!("humanism"));
    let museum = Item::Building {
        building: crate::name!("art_museum"),
    };
    for cid in &cities {
        game.cities
            .get_mut(cid)
            .unwrap()
            .buildings
            .push(crate::name!("amphitheater"));
        assert!(game.can_produce(0, *cid, &museum));
    }
    // Fill the six Writing slots and the universal Palace slot, so the
    // simulator cannot rearrange Writing to invent a spare art slot.
    game.players[0]
        .counters
        .insert("great_work:writing".to_string(), 7);
    (game, cities, museum)
}

#[test]
fn the_first_art_museum_opens_slots_and_keeps_its_claim() {
    let (game, cities, museum) = theaters_ready_for_art();
    for ai in [seat(VictoryTarget::Culture, true), seat_two()] {
        let mut claimed = game.clone();
        let plan = ai.assess(&claimed, 0);
        ai.claim_culture_theaters(&mut claimed, 0, &plan);
        let selected: Vec<_> = cities
            .iter()
            .copied()
            .filter(|cid| claimed.cities[cid].queue.first() == Some(&museum))
            .collect();
        assert_eq!(selected.len(), 1);
        assert!(ai.culture_theater_holds(&claimed, 0, selected[0], &plan, &museum));
        let queues: Vec<_> = cities
            .iter()
            .map(|cid| claimed.cities[cid].queue.clone())
            .collect();
        ai.claim_culture_theaters(&mut claimed, 0, &plan);
        assert_eq!(
            queues,
            cities
                .iter()
                .map(|cid| claimed.cities[cid].queue.clone())
                .collect::<Vec<_>>()
        );
        claimed
            .cities
            .get_mut(&selected[0])
            .unwrap()
            .buildings
            .push(crate::name!("art_museum"));
        for cid in &cities {
            claimed.cities.get_mut(cid).unwrap().queue = vec![unit("warrior")];
        }
        ai.claim_culture_theaters(&mut claimed, 0, &plan);
        assert!(cities
            .iter()
            .all(|cid| claimed.cities[cid].queue == vec![unit("warrior")]));
        assert!(claimed.can_house_additional_great_work(0, "art"));
    }
}

#[test]
fn an_archaeological_museum_does_not_replace_the_first_art_slots() {
    let (mut game, cities, museum) = theaters_ready_for_art();
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("archaeological_museum"));
    assert!(!game.can_produce(0, cities[0], &museum));
    assert!(!game.can_house_additional_great_work(0, "art"));
    claim(&seat(VictoryTarget::Culture, true), &mut game);
    assert_ne!(game.cities[&cities[0]].queue.first(), Some(&museum));
    assert_eq!(
        cities
            .iter()
            .filter(|cid| game.cities[cid].queue.first() == Some(&museum))
            .count(),
        1
    );
}

#[test]
fn the_first_art_slots_preserve_defense_and_other_lanes() {
    let (game, cities, museum) = theaters_ready_for_art();
    for ai in [
        seat(VictoryTarget::Culture, false),
        seat(VictoryTarget::Science, true),
    ] {
        let mut other = game.clone();
        claim(&ai, &mut other);
        assert!(cities
            .iter()
            .all(|cid| other.cities[cid].queue == vec![unit("warrior")]));
    }
    let ai = seat(VictoryTarget::Culture, true);
    let mut protected = game;
    protected.cities.get_mut(&cities[0]).unwrap().queue = vec![unit("settler")];
    protected.cities.get_mut(&cities[1]).unwrap().queue = vec![Item::Building {
        building: crate::name!("walls"),
    }];
    let mut plan = ai.assess(&protected, 0);
    plan.threatened_city = Some(cities[2]);
    ai.claim_culture_theaters(&mut protected, 0, &plan);
    assert!(cities
        .iter()
        .all(|cid| !protected.cities[cid].queue.contains(&museum)));
}
