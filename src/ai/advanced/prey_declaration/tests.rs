use super::super::{AdvancedAi, VictoryTarget};
use crate::game::Game;

/// Two majors, each with a capital; seat 0 fields an army, seat 1 a lone
/// warrior.
fn prey_board() -> Game {
    let mut game = Game::new_full(2, 24, 16, 7_951, 300, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let capital = game.cities[&game.player_city_ids(0)[0]].pos;
    for _ in 0..6 {
        game.spawn_test_unit("swordsman", 0, capital);
    }
    game
}

#[test]
fn a_weak_major_is_prey_only_under_the_gene_and_domination() {
    let game = prey_board();
    assert!(game.military_power(1) <= super::PREY_DECLARE_RATIO * game.military_power(0));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.prey_is_declared_on_now(&game, 0, 1), "off");
    ai.enable_prey_is_declared_on();
    assert!(ai.prey_is_declared_on_now(&game, 0, 1));

    let mut other = AdvancedAi::new();
    other.enable_prey_is_declared_on();
    assert!(
        !other.prey_is_declared_on_now(&game, 0, 1),
        "no Domination target"
    );
}

#[test]
fn a_rival_near_our_power_is_not_prey() {
    let mut game = prey_board();
    let capital = game.cities[&game.player_city_ids(1)[0]].pos;
    for _ in 0..5 {
        game.spawn_test_unit("swordsman", 1, capital);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_prey_is_declared_on();
    assert!(!ai.prey_is_declared_on_now(&game, 0, 1));
}

/// `prey-waits-for-the-march`: the prey board with the army moved to the far
/// corner of our side, more than the march bar from the rival's capital, and
/// the rival's capital as the objective.
fn far_army_board() -> (Game, crate::Pos) {
    let mut game = prey_board();
    let objective = game.cities[&game.player_city_ids(1)[0]].pos;
    let far = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(**pos).is_empty()
                && game.wdist(**pos, objective) > super::one_war::OVERWHELMING_MARCH_TILES + 2
        })
        .map(|(pos, _)| *pos)
        .max_by_key(|pos| (game.wdist(*pos, objective), *pos))
        .expect("a tile far from the rival capital");
    for uid in game.player_unit_ids(0) {
        if game.units[&uid].kind == "swordsman" {
            game.units.get_mut(&uid).unwrap().pos = far;
        }
    }
    (game, objective)
}

fn march_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_prey_is_declared_on();
    ai.enable_prey_waits_for_the_march();
    ai
}

#[test]
fn a_far_army_holds_the_prey_war_until_the_march_stalls() {
    use super::PreyMarch;
    let (mut game, objective) = far_army_board();
    let median = {
        let ai = march_ai();
        ai.field_army_median_distance(&game, 0, objective)
            .expect("a field army")
    };
    assert!(median > super::one_war::OVERWHELMING_MARCH_TILES);

    let mut off = AdvancedAi::targeting(VictoryTarget::Domination);
    off.enable_prey_is_declared_on();
    assert_eq!(off.prey_march_reading(&game, 0, 1, objective), None, "off");

    let mut ai = march_ai();
    let start = game.turn;
    assert_eq!(
        ai.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Hold(median, 0))
    );
    for held in 1..super::PREY_MARCH_STALL {
        game.turn = start + held;
        assert_eq!(
            ai.prey_march_reading(&game, 0, 1, objective),
            Some(PreyMarch::Hold(median, held))
        );
    }
    game.turn = start + super::PREY_MARCH_STALL;
    assert_eq!(
        ai.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Stalled(median)),
        "a march that comes no closer releases the war"
    );
}

#[test]
fn a_closing_march_holds_until_its_patience_runs_out() {
    use super::{PreyMarch, PreyMarchHold, PREY_MARCH_PATIENCE};
    let (mut game, objective) = far_army_board();
    let mut ai = march_ai();
    game.turn = 60;
    let median = ai.field_army_median_distance(&game, 0, objective).unwrap();
    // A march that gained a tile last turn, one turn short of the patience.
    ai.prey_march_hold = Some(PreyMarchHold {
        rival: 1,
        since: game.turn - (PREY_MARCH_PATIENCE - 1),
        best: (median + 1, game.turn - 1),
        best_gun: i32::MAX,
        seen: game.turn - 1,
    });
    assert_eq!(
        ai.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Hold(median, PREY_MARCH_PATIENCE - 1)),
        "a march still closing holds"
    );
    game.turn += 1;
    assert_eq!(
        ai.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Patience(median)),
        "the hold's patience releases the war"
    );
}

#[test]
fn a_new_rival_or_a_lapsed_hold_starts_a_fresh_clock() {
    use super::{PreyMarch, PreyMarchHold, PREY_MARCH_PATIENCE, PREY_MARCH_STALL};
    let (mut game, objective) = far_army_board();
    let mut ai = march_ai();
    game.turn = 60;
    let median = ai.field_army_median_distance(&game, 0, objective).unwrap();
    let old = PreyMarchHold {
        rival: 2,
        since: game.turn - PREY_MARCH_PATIENCE,
        best: (median, game.turn - PREY_MARCH_PATIENCE),
        best_gun: i32::MAX,
        seen: game.turn - 1,
    };
    ai.prey_march_hold = Some(old);
    assert_eq!(
        ai.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Hold(median, 0)),
        "another rival's hold is not this one's"
    );
    ai.prey_march_hold = Some(PreyMarchHold {
        rival: 1,
        seen: game.turn - PREY_MARCH_STALL - 1,
        ..old
    });
    assert_eq!(
        ai.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Hold(median, 0)),
        "a hold not read for longer than the stall restarts"
    );
}

#[test]
fn an_army_within_the_bar_passes_and_clears_the_hold() {
    use super::PreyMarch;
    let (mut game, objective) = far_army_board();
    let mut ai = march_ai();
    assert!(matches!(
        ai.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Hold(..))
    ));
    let near = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && (3..=5).contains(&game.wdist(**pos, objective))
        })
        .map(|(pos, _)| *pos)
        .min()
        .expect("a tile beside the rival capital");
    for uid in game.player_unit_ids(0) {
        if game.units[&uid].kind == "swordsman" {
            game.units.get_mut(&uid).unwrap().pos = near;
        }
    }
    let reading = ai.prey_march_reading(&game, 0, 1, objective);
    assert!(
        matches!(reading, Some(PreyMarch::Within(m)) if m <= super::one_war::OVERWHELMING_MARCH_TILES)
    );
    assert_eq!(ai.prey_march_hold, None);
}

#[test]
fn the_march_gene_ships_off_and_is_registered() {
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.prey_waits_for_the_march);
    assert!(super::super::genes::GENES
        .iter()
        .any(|gene| gene.tag == "prey-waits-for-the-march" && gene.opt_in()));
}

/// A flat board: our two cities on the west, the prey's one city 16 tiles
/// east of our nearest, a Formal War on it available, and our army of six
/// swordsmen at a position of the test's choosing.
fn declaration_board(
    army_at: impl Fn(crate::Pos) -> crate::Pos,
) -> (Game, super::super::StrategicPlan) {
    use super::super::{GrandStrategy, StrategicPlan};
    let mut g = Game::new_full(2, 40, 24, 3431, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.owner_city = None;
    }
    g.found_city_for(0, (3, 8), None);
    g.found_city_for(0, (2, 15), None);
    let objective = (19, 8);
    let target = g.found_city_for(1, objective, None);
    g.spawn_test_unit("warrior", 1, objective);
    let stand = army_at(objective);
    for _ in 0..6 {
        g.spawn_test_unit("swordsman", 0, stand);
    }
    g.turn = 150;
    g.current = 0;
    g.at_war.clear();
    for pid in 0..2 {
        g.players[pid].civ = "Rome".into();
        g.players[pid].met.extend([0, 1]);
        g.players[pid].gold = 1000.0;
        g.players[pid].gold_per_turn = 20.0;
    }
    g.players[0].denounced_since.insert(1, 140);
    g.players[0].denounced_until.insert(1, 200);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan)
}

#[test]
fn the_declaration_step_holds_a_prey_war_for_the_march_and_opens_it_near() {
    let far = |_| (4, 9);
    let (mut g, plan) = declaration_board(far);
    let objective = g.cities[&plan.target_city.unwrap()].pos;
    let mut off = AdvancedAi::targeting(VictoryTarget::Domination);
    off.enable_prey_is_declared_on();
    assert!(off.prey_is_declared_on_now(&g, 0, 1));
    assert!(
        off.field_army_median_distance(&g, 0, objective).unwrap()
            > super::one_war::OVERWHELMING_MARCH_TILES
    );
    let mut off_game = g.clone();
    off.advanced_diplomacy(&mut off_game, 0, &plan);
    assert!(
        off_game.is_at_war(0, 1),
        "off, the prey waiver declares at once"
    );

    let mut on = march_ai();
    on.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "on, the far army holds the war");
    assert!(on.prey_march_hold.is_some_and(|hold| hold.rival == 1));

    let near = |objective: crate::Pos| (objective.0 - 5, objective.1);
    let (mut g, plan) = declaration_board(near);
    let mut on = march_ai();
    on.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        g.is_at_war(0, 1),
        "an army within the bar declares as before"
    );
}

fn march_2_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_prey_is_declared_on();
    ai.enable_prey_waits_for_the_march_2();
    ai
}

/// The prey board with the army moved to stand `distance` tiles from the
/// rival's capital, and that capital as the objective.
fn army_at_distance(distance: i32) -> (Game, crate::Pos) {
    let mut game = prey_board();
    let objective = game.cities[&game.player_city_ids(1)[0]].pos;
    let stand = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(**pos, objective) == distance
        })
        .map(|(pos, _)| *pos)
        .min()
        .expect("a land tile at that distance");
    for uid in game.player_unit_ids(0) {
        if game.units[&uid].kind == "swordsman" {
            game.units.get_mut(&uid).unwrap().pos = stand;
        }
    }
    (game, objective)
}

#[test]
fn version_two_holds_an_army_version_one_would_pass() {
    use super::PreyMarch;
    let (game, objective) = army_at_distance(8);
    let mut v1 = march_ai();
    assert_eq!(
        v1.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Within(8))
    );
    let mut v2 = march_2_ai();
    assert_eq!(
        v2.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Hold(8, 0)),
        "8 tiles is over version two's bar of 6"
    );
    assert_eq!(v2.prey_march_bar(), super::PREY_MARCH_TILES_2);
    assert_eq!(v2.prey_march_tag(), "prey-waits-for-the-march-2");
}

#[test]
fn version_two_asks_a_gun_only_once_one_is_buildable() {
    use super::PreyMarch;
    let (mut game, objective) = army_at_distance(5);
    let mut v2 = march_2_ai();
    assert_eq!(
        v2.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Within(5)),
        "no siege gun is buildable yet, so the gun leg is not asked"
    );
    game.players[0].techs.insert(crate::name!("engineering"));
    assert_eq!(
        v2.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::HoldForGun(i32::MAX, 0)),
        "a buildable gun that is not fielded holds the war"
    );
    let far_gun = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(**pos, objective) == super::PREY_MARCH_GUN_TILES + 3
        })
        .map(|(pos, _)| *pos)
        .min()
        .unwrap();
    let gun = game.spawn_test_unit("catapult", 0, far_gun);
    assert!(matches!(
        v2.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::HoldForGun(d, _)) if d == super::PREY_MARCH_GUN_TILES + 3
    ));
    let near_gun = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.wdist(**pos, objective) == 3
        })
        .map(|(pos, _)| *pos)
        .min()
        .unwrap();
    game.units.get_mut(&gun).unwrap().pos = near_gun;
    let reading = v2.prey_march_reading(&game, 0, 1, objective);
    assert!(
        matches!(reading, Some(PreyMarch::Within(_))),
        "a gun within the bar opens the war: {reading:?}"
    );
    assert_eq!(v2.prey_march_hold, None);
}

#[test]
fn a_closing_gun_is_progress_for_the_stall_release() {
    use super::PreyMarch;
    let (mut game, objective) = army_at_distance(5);
    game.players[0].techs.insert(crate::name!("engineering"));
    let at = |game: &Game, d: i32| {
        game.map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && game.wdist(**pos, objective) == d
            })
            .map(|(pos, _)| *pos)
            .min()
            .unwrap()
    };
    let gun = game.spawn_test_unit("catapult", 0, at(&game, 14));
    let mut v2 = march_2_ai();
    game.turn = 60;
    // The army stands still within the bar; the gun gains a tile a turn.
    for (step, d) in [14, 13, 12, 11, 10].into_iter().enumerate() {
        game.turn = 60 + step as u32;
        let pos = at(&game, d);
        game.units.get_mut(&gun).unwrap().pos = pos;
        let reading = v2.prey_march_reading(&game, 0, 1, objective);
        assert!(
            matches!(reading, Some(PreyMarch::HoldForGun(g, _)) if g == d),
            "turn {}: {reading:?}",
            game.turn
        );
    }
    // Then it stops: the stall clock runs from its last gain.
    for wait in 1..super::PREY_MARCH_STALL {
        game.turn = 64 + wait;
        assert!(matches!(
            v2.prey_march_reading(&game, 0, 1, objective),
            Some(PreyMarch::HoldForGun(..))
        ));
    }
    game.turn = 64 + super::PREY_MARCH_STALL;
    assert!(matches!(
        v2.prey_march_reading(&game, 0, 1, objective),
        Some(PreyMarch::Stalled(_))
    ));
}

#[test]
fn version_two_keeps_version_one_in_the_declaration_step() {
    let far = |_| (4, 9);
    let (mut g, plan) = declaration_board(far);
    let mut on = march_2_ai();
    on.advanced_diplomacy(&mut g, 0, &plan);
    assert!(!g.is_at_war(0, 1), "version two holds the far army too");
    assert!(on.prey_march_hold.is_some_and(|hold| hold.rival == 1));

    let near = |objective: crate::Pos| (objective.0 - 4, objective.1);
    let (mut g, plan) = declaration_board(near);
    let mut on = march_2_ai();
    on.advanced_diplomacy(&mut g, 0, &plan);
    assert!(
        g.is_at_war(0, 1),
        "an army within 6 tiles with no gun buildable declares"
    );
}

#[test]
fn the_march_gene_family_ships_off_and_is_registered() {
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.prey_waits_for_the_march_2);
    assert!(super::super::genes::GENES
        .iter()
        .any(|gene| gene.tag == "prey-waits-for-the-march-2" && gene.opt_in()));
}
