use super::{AdvancedAi, RaidPrize, VictoryTarget, SETTLER_PRIZE};
use crate::game::Game;
use crate::Pos;
use std::sync::Arc;

/// A two-major board with six visible enemy mines inside a warrior's
/// two-turn reach.  It is deliberately a pillage-only opportunity: the
/// score guard, rather than a missing prize or power gate, must decide it.
fn pillage_raid_board() -> Game {
    let mut game = Game::new_full(2, 28, 18, 8_131, 250, 0, false);
    let mut capitals = Vec::new();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| game.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = game.units[&settler].pos;
        capitals.push(game.found_city_for(pid, position, None));
        game.remove_unit(settler);
    }
    for pid in 0..2 {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }

    let ours = game.cities[&capitals[0]].pos;
    let theirs = game.cities[&capitals[1]].pos;
    let second_city = game
        .map
        .tiles
        .iter()
        .find_map(|(position, tile)| {
            (game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(*position, ours) >= 3
                && game.wdist(*position, theirs) >= 3)
                .then_some(*position)
        })
        .expect("fixture has a legal second-city tile");
    game.found_city_for(0, second_city, None);

    let positions: Vec<Pos> = game.map.tiles.keys().copied().collect();
    for position in positions {
        let water = {
            let tile = game.map.tiles.get(&position).unwrap();
            game.rules.is_water(tile)
        };
        if !water {
            let tile = game.map.tiles.get_mut(&position).unwrap();
            tile.terrain = crate::name!("plains");
            tile.feature = None;
            tile.hills = false;
            tile.wonder = None;
        }
        game.players[0].explored.insert(position);
    }
    game.record_contact(0, 1);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.turn = 30;
    game.current = 0;

    let enemy_city = game.player_city_ids(1)[0];
    let mines: Vec<Pos> = game.cities[&enemy_city]
        .owned_tiles
        .iter()
        .copied()
        .filter(|position| {
            *position != theirs
                && game
                    .map
                    .tiles
                    .get(position)
                    .is_some_and(|tile| !game.rules.is_water(tile))
        })
        .take(6)
        .collect();
    assert_eq!(mines.len(), 6, "fixture capital owns six usable mine tiles");
    for position in mines {
        game.map.tiles.get_mut(&position).unwrap().improvement = Some(crate::name!("mine"));
    }
    let warrior_at = game
        .map
        .tiles
        .iter()
        .find_map(|(position, tile)| {
            (game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.units_at(*position).is_empty()
                && game.wdist(*position, theirs) == 2)
                .then_some(*position)
        })
        .expect("fixture has a nearby warrior post");
    game.spawn_test_unit("warrior", 0, warrior_at);
    game.world_era = 2;
    game
}

#[test]
fn pillage_only_raids_do_not_challenge_a_score_leader() {
    let pillage = [RaidPrize::Pillage {
        pos: (0, 0),
        value: 125.0,
    }];
    assert!(
        !AdvancedAi::raid_prizes_can_challenge_score_leader(272, 479, &pillage),
        "three pillage tiles must not reopen the 272-to-479 live-run loss"
    );
    assert!(
        AdvancedAi::raid_prizes_can_challenge_score_leader(479, 272, &pillage),
        "a score lead still permits a bounded pillage raid"
    );

    let decisive_settler = [RaidPrize::Settler {
        pos: (0, 0),
        value: SETTLER_PRIZE,
    }];
    assert!(
        AdvancedAi::raid_prizes_can_challenge_score_leader(272, 479, &decisive_settler),
        "a capturable Settler remains worth a bounded opportunity against a leader"
    );
}

#[test]
fn pillage_raid_selection_respects_the_public_score_lead() {
    let mut game = pillage_raid_board();
    let mut ai = AdvancedAi::new();
    ai.enable_opportunistic_war();
    ai.enable_raid_pillage_prizes();

    Arc::make_mut(&mut game.observed_score).insert(0, 272);
    Arc::make_mut(&mut game.observed_score).insert(1, 479);
    assert!(
        ai.raid_opportunity(&game, 0).is_none(),
        "a three-mine raid must not reopen the 272-to-479 score-leader loss"
    );

    Arc::make_mut(&mut game.observed_score).insert(0, 479);
    Arc::make_mut(&mut game.observed_score).insert(1, 272);
    assert!(
        ai.raid_opportunity(&game, 0).is_some(),
        "the same legal, power-safe pillage raid remains available at score parity or better"
    );
}

#[test]
fn science_target_does_not_open_an_opportunistic_raid() {
    let game = pillage_raid_board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_opportunistic_war();
    ai.enable_raid_pillage_prizes();

    assert!(
        ai.raid_opportunity(&game, 0).is_none(),
        "an explicit Science lane must not trade its research race for a raid"
    );
}

#[test]
fn mountains_between_the_army_and_prizes_do_not_justify_a_war() {
    let mut game = pillage_raid_board();
    let mut ai = AdvancedAi::new();
    ai.enable_opportunistic_war();
    ai.enable_raid_pillage_prizes();
    assert!(ai.raid_opportunity(&game, 0).is_some());
    let warrior = game.player_unit_ids(0)[0];
    let here = game.units[&warrior].pos;
    for position in game.nbrs(here) {
        game.map.tiles.get_mut(&position).unwrap().terrain = crate::name!("mountain");
    }
    // The old straight-line valuation still clears the declaration bar.
    let prizes = ai.raid_prizes_against(&game, 0, 1, &ai.raid_strikers(&game, 0));
    assert!(prizes.iter().map(|prize| prize.value()).sum::<f64>() >= super::RAID_WAR_MIN_VALUE);
    assert!(ai.raid_opportunity(&game, 0).is_none());
    assert!(!game.is_at_war(0, 1));
}

#[test]
fn hypothetical_raid_opens_closed_borders_without_changing_the_real_board() {
    let game = pillage_raid_board();
    let mut ai = AdvancedAi::new();
    ai.enable_opportunistic_war();
    ai.enable_raid_pillage_prizes();
    let warrior = game.player_unit_ids(0)[0];
    let prizes = ai.raid_prizes_against(&game, 0, 1, &ai.raid_strikers(&game, 0));
    assert!(prizes
        .iter()
        .any(|prize| { game.route_distance(warrior, prize.pos(), 0).is_none() }));
    let treasury = game.players[0].gold;
    let position = game.units[&warrior].pos;
    let opportunity = ai
        .raid_opportunity(&game, 0)
        .expect("war opens access to the mines");
    assert!(!opportunity.prizes.is_empty());
    assert!(!game.is_at_war(0, 1));
    assert_eq!(game.players[0].gold, treasury);
    assert_eq!(game.units[&warrior].pos, position);
    assert!(ai.raid_war.is_none());
}

#[test]
fn a_visible_settler_behind_an_impassable_ring_is_not_a_war_opportunity() {
    let mut game = pillage_raid_board();
    let mut ai = AdvancedAi::new();
    ai.enable_opportunistic_war();
    let warrior = game.player_unit_ids(0)[0];
    let here = game.units[&warrior].pos;
    let position = game
        .wdisk(here, 2)
        .into_iter()
        .find(|position| {
            game.wdist(here, *position) == 2
                && game.city_at(*position).is_none()
                && game
                    .map
                    .get(*position)
                    .is_some_and(|tile| !game.rules.is_water(tile) && game.rules.is_passable(tile))
                && game.player_can_see(0, *position)
                && game.player_city_ids(0).iter().any(|cid| {
                    game.wdist(game.cities[cid].pos, *position) <= super::RAID_SETTLER_HOME_RADIUS
                })
        })
        .expect("a visible land tile near home");
    game.spawn_test_unit("settler", 1, position);
    assert!(
        ai.raid_opportunity(&game, 0).is_some(),
        "the open route offers a settler"
    );
    // Isolate the soldier rather than the prize: its existing sight still
    // supplies the candidate, but no legal first step can leave this tile.
    for neighbor in game.nbrs(here) {
        game.map.tiles.get_mut(&neighbor).unwrap().terrain = crate::name!("mountain");
    }
    let observer =
        game.nbrs(position)
            .into_iter()
            .find(|neighbor| {
                game.wdist(here, *neighbor) > 1
                    && game.city_at(*neighbor).is_none()
                    && game.map.get(*neighbor).is_some_and(|tile| {
                        !game.rules.is_water(tile) && game.rules.is_passable(tile)
                    })
            })
            .expect("an observer outside the ring");
    game.spawn_test_unit("builder", 0, observer);
    // Civilian vision keeps the target visible without adding a striker.
    let prizes = ai.raid_prizes_against(&game, 0, 1, &ai.raid_strikers(&game, 0));
    assert!(matches!(prizes.as_slice(), [RaidPrize::Settler { .. }]));
    assert!(ai.raid_opportunity(&game, 0).is_none());
}
