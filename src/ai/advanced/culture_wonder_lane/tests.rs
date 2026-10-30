use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

/// Three developed cities of six citizens with the ancient and classical
/// wonder techs and civics in hand, production 40 / 30 / 20, idle queues.
fn wonder_ready_empire() -> (Game, Vec<u32>) {
    let mut game = Game::new_full(
        1,
        44,
        26,
        crate::rng::fixture_seed("CULTUREWONDER", 1),
        250,
        0,
        false,
    );
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
    game.turn = 80;
    let techs: Vec<Name> = game.rules.techs.keys().copied().collect();
    let civics: Vec<Name> = game.rules.civics.keys().copied().collect();
    for tech in techs {
        if game.rules.techs[&tech].era <= 1 {
            game.players[0].techs.insert(tech);
        }
    }
    for civic in civics {
        if game.rules.civics[&civic].era <= 1 {
            game.players[0].civics.insert(civic);
        }
    }
    let cities = game.player_city_ids(0);
    for (rank, cid) in cities.iter().enumerate() {
        let city = game.cities.get_mut(cid).unwrap();
        city.pop = 6;
        city.queue.clear();
        for building in ["monument", "granary"] {
            if !city.buildings.iter().any(|built| *built == building) {
                city.buildings.push(Name::new(building));
            }
        }
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                production: 40.0 - 10.0 * rank as f64,
                ..Default::default()
            },
        );
    }
    let mut ranked = cities;
    ranked.sort_by(|a, b| {
        game.city_yields(*b)
            .production
            .total_cmp(&game.city_yields(*a).production)
            .then(a.cmp(b))
    });
    (game, ranked)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_culture_seat_wonder_lane();
    }
    ai
}

fn is_wonder(item: Option<&Item>) -> bool {
    matches!(item, Some(Item::Wonder { .. }))
}

fn claim(ai: &AdvancedAi, game: &mut Game) {
    let plan = ai.assess(game, 0);
    ai.claim_lane_wonders(game, 0, &plan);
}

/// Three cities open one lane: the strongest takes a wonder, the others keep
/// their queues, and the claim holds. Off, or for a Domination seat, nothing.
#[test]
fn the_strongest_culture_city_takes_one_wonder_lane() {
    let (game, ranked) = wonder_ready_empire();
    let ai = seat(VictoryTarget::Culture, true);
    assert!(
        ai.lane_wonder_item(&game, 0, ranked[0], VictoryTarget::Culture)
            .is_some(),
        "fixture: the strongest city can finish a wonder in time"
    );

    for (other, why) in [
        (seat(VictoryTarget::Culture, false), "off"),
        (seat(VictoryTarget::Domination, true), "a Domination seat"),
    ] {
        let mut kept = game.clone();
        claim(&other, &mut kept);
        for cid in &ranked {
            assert!(kept.cities[cid].queue.is_empty(), "{why}: no claim");
        }
    }

    let mut on = game.clone();
    claim(&ai, &mut on);
    let head = on.cities[&ranked[0]].queue.first().cloned();
    assert!(
        is_wonder(head.as_ref()),
        "the strongest city takes the lane"
    );
    for cid in &ranked[1..] {
        assert!(
            !is_wonder(on.cities[cid].queue.first()),
            "one lane for three cities"
        );
    }
    let plan = ai.assess(&on, 0);
    assert!(ai.lane_wonder_holds(&on, 0, ranked[0], &plan, head.as_ref().unwrap()));
    assert!(!seat(VictoryTarget::Culture, false).lane_wonder_holds(
        &on,
        0,
        ranked[0],
        &plan,
        head.as_ref().unwrap()
    ));

    // A second pass finds the lane in flight and opens no other.
    let before = on.clone();
    claim(&ai, &mut on);
    for cid in &ranked {
        assert_eq!(
            on.cities[cid].queue.first(),
            before.cities[cid].queue.first(),
            "the lane is full"
        );
    }
}

/// The Culture arm prices what a wonder pays the Culture victory: a wonder
/// with Great Work slots outranks one with none at the same tourism.
#[test]
fn great_work_slots_raise_a_wonders_culture_value() {
    let (game, _) = wonder_ready_empire();
    let ai = seat(VictoryTarget::Culture, true);
    let value = |name: &str| {
        ai.wonder_lane_value(
            VictoryTarget::Culture,
            &game.rules.wonders[&Name::new(name)],
        )
        .unwrap()
    };
    let slotted = game
        .rules
        .wonders
        .iter()
        .find(|(_, spec)| spec.great_work_slots.values().sum::<i32>() > 0)
        .map(|(name, _)| name.to_string())
        .expect("a wonder with Great Work slots");
    let bare = game
        .rules
        .wonders
        .iter()
        .find(|(_, spec)| {
            spec.great_work_slots.is_empty()
                && spec.effects.get("tourism").is_none()
                && spec.yields.culture <= 0.0
                && spec.great_person_points.is_empty()
        })
        .map(|(name, _)| name.to_string())
        .expect("a wonder with no culture payload");
    assert_eq!(value(&bare), 2.0, "every wonder earns two tourism");
    assert!(value(&slotted) > value(&bare));
    assert!(ai
        .wonder_lane_value(
            VictoryTarget::Domination,
            &game.rules.wonders[&Name::new(&slotted)]
        )
        .is_none());
}
