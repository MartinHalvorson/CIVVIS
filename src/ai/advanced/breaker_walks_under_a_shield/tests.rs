//! Focused tests for `breaker-walks-under-a-shield`.
//!
//! The seat is the live Domination seat's opening (`early-conquest-opening`,
//! `first-capital-before-the-walls`, `first-capital-strikes-the-open-window`,
//! `opening-yields-to-walls`) on Online speed at turn 45, its first capital
//! 18 tiles east behind Ancient Walls. Every test reads the board with this
//! gene off as well, which is today's behaviour.

use super::super::test_support::opt_in_off_in_both_controllers;
use super::super::{GrandStrategy, StrategicPlan, VictoryTarget};
use super::*;
use crate::game::{Action, Game};
use crate::name;
use crate::Pos;
use std::collections::BTreeSet;
use std::sync::Arc;

/// A flat 60x30 board of `capitals.len()` empires, every starting unit
/// cleared, nobody at war, Online speed, turn 45.
fn board(capitals: &[Pos]) -> Game {
    let mut game = Game::new_full(capitals.len(), 60, 30, 90_909, 1_000, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    for (pid, pos) in capitals.iter().enumerate() {
        game.found_city_for(pid, *pos, None);
    }
    for cid in game.cities.keys().copied().collect::<Vec<_>>() {
        game.cities.get_mut(&cid).unwrap().queue.clear();
    }
    game.at_war.clear();
    game.game_speed = crate::setup::GameSpeed::Online;
    game.turn = 45;
    game.current = 0;
    game
}

fn meet(game: &mut Game, rival: usize) {
    game.players[0].met.insert(rival);
    game.players[rival].met.insert(0);
}

fn at_war(game: &mut Game, a: usize, b: usize) {
    game.at_war.insert((a, b));
    game.at_war.insert((b, a));
}

/// The live seat's opening, with or without this gene.
fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_early_conquest_opening();
    ai.enable_conquest_opening_stays_near();
    ai.enable_early_conquest_stands_down();
    ai.enable_declaration_needs_production_parity();
    ai.conquest_opening_needs_the_production = false;
    ai.enable_first_capital_before_the_walls();
    ai.enable_first_capital_strikes_the_open_window();
    ai.enable_opening_yields_to_walls();
    if gene {
        ai.enable_breaker_walks_under_a_shield();
    }
    ai.base.journal = crate::reasoning::Journal::recording();
    ai
}

fn capital_of(game: &Game, pid: usize) -> u32 {
    game.player_city_ids(pid)
        .into_iter()
        .find(|cid| game.cities[cid].is_capital)
        .unwrap()
}

fn set_walls(game: &mut Game, cid: u32, hp: i32) {
    Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(cid, hp);
    game.cities.get_mut(&cid).unwrap().wall_hp = hp;
}

fn set_strength(game: &mut Game, cid: u32, strength: f64) {
    Arc::make_mut(&mut game.observed_city_strength).insert(cid, strength);
}

fn unit(game: &mut Game, kind: &str, owner: usize, pos: Pos) -> u32 {
    let uid = game.spawn_test_unit(kind, owner, pos);
    fresh(game, uid);
    uid
}

/// A new turn's movement and attack for `uid`.
fn fresh(game: &mut Game, uid: u32) {
    let moves = game.unit_max_moves(uid);
    let unit = game.units.get_mut(&uid).unwrap();
    unit.moves_left = moves;
    unit.attacks_left = 1;
    unit.moved = false;
    unit.acted = false;
    unit.fortified = false;
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 10,
        assessed_turn: 45,
        rush: false,
    }
}

fn headlines(ai: &AdvancedAi) -> Vec<(String, String)> {
    ai.base
        .journal
        .since(0)
        .thoughts
        .into_iter()
        .map(|thought| (thought.headline, thought.detail))
        .collect()
}

/// Seats 0 (us, at (6,12)), 1 (the first capital at (24,12), walled 100)
/// and 2 (to the south, walled past a Ram); the opening named on seat 1's
/// capital.
fn opened(gene: bool) -> (Game, AdvancedAi, u32) {
    let mut game = board(&[(6, 12), (24, 12), (6, 24)]);
    meet(&mut game, 1);
    let tiles: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(tiles);
    meet(&mut game, 2);
    let walled = capital_of(&game, 2);
    set_walls(&mut game, walled, 300);
    let mut ai = seat(gene);
    ai.maintain_conquest_opening(&mut game, 0);
    let capital = capital_of(&game, 1);
    let opening = ai.conquest_opening.as_ref().expect("the opening stands");
    assert_eq!((opening.target, opening.city), (1, capital));
    set_walls(&mut game, capital, 100);
    set_strength(&mut game, capital, 20.0);
    (game, ai, capital)
}

/// Whether a land soldier or a city of ours holds the breaker's tile.
fn shielded(game: &Game, ram: u32) -> bool {
    let pos = game.units[&ram].pos;
    game.city_at(pos)
        .is_some_and(|cid| game.cities[&cid].owner == 0)
        || game.unit_ids_at(pos).iter().any(|id| {
            *id != ram
                && game.units[id].owner == 0
                && game.rules.units[game.units[id].kind].class == "military"
        })
}

/// One unit's turn the way `advance_unit_serial` plays it: this gene's step,
/// else the shipped staging step, until it stops acting.
fn play_unit(ai: &mut AdvancedAi, game: &mut Game, uid: u32) {
    for _ in 0..8 {
        if game
            .units
            .get(&uid)
            .is_none_or(|unit| unit.moves_left <= 0.0)
        {
            return;
        }
        let acted = ai
            .shielded_breaker_step(game, 0, uid, &plan())
            .or_else(|| ai.campaign_staging_step(game, 0, uid, &plan()))
            .unwrap_or(false);
        if !acted {
            return;
        }
    }
}

// ----------------------------------------------------------------- registry

#[test]
fn breaker_walks_under_a_shield_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("breaker-walks-under-a-shield", |ai| {
        ai.breaker_walks_under_a_shield
    });
}

// ------------------------------------------------------------------- off

/// Off, nothing of this gene reads the board: no breaker is named, no unit
/// moves first, the formation pass and the staging road are as shipped, and
/// no memory is kept.
#[test]
fn off_the_gene_names_nothing_and_moves_nothing() {
    let (mut game, mut ai, _) = opened(false);
    let ram = unit(&mut game, "battering_ram", 0, (6, 12));
    let sword = unit(&mut game, "swordsman", 0, (10, 12));
    ai.maintain_conquest_opening(&mut game, 0);
    let before = game.clone();
    assert_eq!(ai.shielded_breaker(&game, 0), None);
    assert!(!ai.breaker_waits_for_its_shield(&game, 0, ram));
    assert_eq!(ai.shield_moves_first(&game, 0), None);
    assert!(!ai.shield_carries_the_breaker(&game, 0, sword));
    assert_eq!(ai.shielded_breaker_step(&mut game, 0, ram, &plan()), None);
    assert_eq!(ai.shielded_breaker_step(&mut game, 0, sword, &plan()), None);
    ai.breaker_shield_watch(&game, 0);
    ai.breaker_shield_note(&game, 0);
    assert!(ai.breaker_shield_memory.is_none());
    assert_eq!(game.units[&ram].pos, before.units[&ram].pos);
    assert_eq!(game.units[&sword].pos, before.units[&sword].pos);
    // The breaker walks alone to the rally as shipped
    // (`first-capital-strikes-the-open-window`).
    assert!(ai
        .conquest_staging_objective(&game, 0, ram, &plan())
        .is_some());

    // On, the same board names the ram and the swordsman.
    let (mut game, mut ai, _) = opened(true);
    let ram = unit(&mut game, "battering_ram", 0, (6, 12));
    let sword = unit(&mut game, "swordsman", 0, (10, 12));
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.shielded_breaker(&game, 0), Some(ram));
    assert!(ai.breaker_waits_for_its_shield(&game, 0, ram));
    assert_eq!(ai.shield_moves_first(&game, 0), Some(sword));
}

// ------------------------------------------------------------- 1. the march

/// The breaker is trained at home with its nearest melee body four tiles
/// out. The body walks back for it, it steps onto the body and links, and
/// from then on the pair walks the staging road as one. After every unit's
/// move the breaker stands with a soldier of ours or in our city, and it is
/// never nearer the walls than the force's nearest body.
#[test]
fn the_breaker_ends_every_move_stacked_with_its_shield() {
    let (mut game, mut ai, capital) = opened(true);
    let city = game.cities[&capital].pos;
    let ram = unit(&mut game, "battering_ram", 0, (6, 12));
    let sword = unit(&mut game, "swordsman", 0, (10, 12));
    let ahead = unit(&mut game, "warrior", 0, (14, 12));
    ai.maintain_conquest_opening(&mut game, 0);
    let force = ai.conquest_opening.as_ref().unwrap().force.clone();
    assert!(force.contains(&sword) && force.contains(&ahead));
    let start = game.wdist(game.units[&ram].pos, city);
    let mut linked_on = None;
    for turn in 0..8 {
        for uid in [ram, sword, ahead] {
            fresh(&mut game, uid);
        }
        assert_eq!(ai.shield_moves_first(&game, 0), Some(sword), "turn {turn}");
        for uid in [sword, ram] {
            play_unit(&mut ai, &mut game, uid);
            assert!(
                shielded(&game, ram),
                "turn {turn}: the ram stands alone at {:?}",
                game.units[&ram].pos
            );
            let nearest = force
                .iter()
                .filter_map(|body| game.units.get(body))
                .map(|body| game.wdist(body.pos, city))
                .min()
                .unwrap();
            assert!(
                game.wdist(game.units[&ram].pos, city) >= nearest,
                "turn {turn}: the ram walks ahead of the force"
            );
        }
        if linked_on.is_none() && AdvancedAi::breaker_carrier(&game, ram) == Some(sword) {
            linked_on = Some(turn);
        }
        if linked_on.is_some() {
            assert_eq!(
                game.units[&ram].pos, game.units[&sword].pos,
                "turn {turn}: the pair walks as one"
            );
        }
    }
    assert_eq!(linked_on, Some(0), "the body met the ram on the first turn");
    assert!(
        game.wdist(game.units[&ram].pos, city) + 6 <= start,
        "the pair marched: {} -> {}",
        start,
        game.wdist(game.units[&ram].pos, city)
    );
    let lines = headlines(&ai);
    assert!(lines.iter().any(|(head, _)| head
        .starts_with("Breaker under a shield: the battering ram follows the swordsman to")));
}

/// A breaker alone in the open with no shield in reach does not walk on by
/// itself: it steps to the soldier of ours it can reach.
#[test]
fn a_lone_breaker_steps_under_a_soldier_rather_than_on_alone() {
    let (mut game, mut ai, _) = opened(true);
    let ram = unit(&mut game, "battering_ram", 0, (12, 12));
    let archer = unit(&mut game, "archer", 0, (13, 12));
    // Not the capital's garrison: a body standing in a city keeps it.
    let sword = unit(&mut game, "swordsman", 0, (6, 16));
    ai.maintain_conquest_opening(&mut game, 0);
    assert_eq!(ai.breaker_shield(&game, 0, ram), Some(sword));
    assert_eq!(
        ai.shielded_breaker_step(&mut game, 0, ram, &plan()),
        Some(true)
    );
    assert_eq!(game.units[&ram].pos, game.units[&archer].pos);
    assert!(shielded(&game, ram));
}

// ------------------------------------------------------ 3. the assault order

/// At war, walls 100: a swordsman beside the city holds while the ram rides
/// with its shield one turn out, and strikes once the ram stands beside
/// the city; the blow lands its full weight on the walls.
#[test]
fn melee_holds_until_the_ram_stands_beside_the_walls_then_strikes() {
    let (mut game, mut ai, capital) = opened(true);
    let city = game.cities[&capital].pos;
    let mut ring: Vec<Pos> = game.nbrs(city).into_iter().collect();
    ring.sort();
    let body = unit(&mut game, "swordsman", 0, ring[0]);
    let far = game
        .wring(city, 3)
        .into_iter()
        .find(|pos| game.wdist(*pos, ring[0]) > 1)
        .unwrap();
    let shield = unit(&mut game, "swordsman", 0, far);
    let ram = unit(&mut game, "battering_ram", 0, far);
    assert!(game
        .apply(
            0,
            &Action::LinkUnits {
                unit: shield,
                with: ram
            }
        )
        .is_ok());
    at_war(&mut game, 0, 1);
    {
        let opening = ai.conquest_opening.as_mut().unwrap();
        opening.declared = Some(game.turn);
        opening.force = BTreeSet::from([body, shield]);
    }
    ai.breaker_shield_watch(&game, 0);
    assert_eq!(AdvancedAi::shield_eta(&game, shield, city), 1);
    let (hp, walls) = (game.cities[&capital].hp, game.cities[&capital].wall_hp);
    assert_eq!(
        ai.shielded_breaker_step(&mut game, 0, body, &plan()),
        Some(false)
    );
    assert_eq!(
        (game.cities[&capital].hp, game.cities[&capital].wall_hp),
        (hp, walls),
        "the body holds its swing"
    );
    assert!(headlines(&ai)
        .iter()
        .any(|(head, _)| head.contains("holds the assault on")
            && head.contains("until the ram stands beside the walls (1 turn(s))")));

    // The shield carries the ram beside the city (a river crossing on the
    // generated map can end a turn's move early, so give it the turns).
    for _ in 0..3 {
        if game.wdist(game.units[&shield].pos, city) <= 1 {
            break;
        }
        fresh(&mut game, shield);
        fresh(&mut game, ram);
        for _ in 0..4 {
            if ai.shielded_breaker_step(&mut game, 0, shield, &plan()) != Some(true) {
                break;
            }
        }
    }
    assert_eq!(game.wdist(game.units[&shield].pos, city), 1);
    assert_eq!(game.units[&ram].pos, game.units[&shield].pos);

    // Now the body strikes, and the ram opens the walls to it.
    fresh(&mut game, body);
    assert_eq!(
        ai.shielded_breaker_step(&mut game, 0, body, &plan()),
        Some(true)
    );
    let after = game.cities[&capital].wall_hp;
    assert!(
        walls - after > 15,
        "a full blow on the walls: {walls} -> {after}"
    );
    assert!(game.units.contains_key(&body));
    ai.breaker_shield_note(&game, 0);
    assert!(headlines(&ai).iter().any(|(head, _)| head
        .starts_with("Breaker under a shield: the ram stands beside")
        && head.ends_with("1 bodies strike the walls")));

    // Off, the same body is not this gene's to hold or strike.
    let (mut game, mut off, _) = opened(false);
    let body = unit(&mut game, "swordsman", 0, ring[0]);
    at_war(&mut game, 0, 1);
    off.conquest_opening.as_mut().unwrap().declared = Some(game.turn);
    assert_eq!(off.shielded_breaker_step(&mut game, 0, body, &plan()), None);
}

/// With the walls down nothing waits for a breaker: a melee body beside the
/// city strikes at once.
#[test]
fn with_the_walls_down_the_assault_does_not_wait() {
    let (mut game, mut ai, capital) = opened(true);
    set_walls(&mut game, capital, 0);
    let city = game.cities[&capital].pos;
    let mut ring: Vec<Pos> = game.nbrs(city).into_iter().collect();
    ring.sort();
    let body = unit(&mut game, "swordsman", 0, ring[0]);
    at_war(&mut game, 0, 1);
    {
        let opening = ai.conquest_opening.as_mut().unwrap();
        opening.declared = Some(game.turn);
        opening.force = BTreeSet::from([body]);
    }
    let hp = game.cities[&capital].hp;
    assert_eq!(
        ai.shielded_breaker_step(&mut game, 0, body, &plan()),
        Some(true)
    );
    assert!(game.cities[&capital].hp < hp, "the city takes the blow");
}

// --------------------------------------------------- 4. the breaker is lost

/// The ram and its shield die beside the walls and a soldier of theirs
/// stands on the tile. The loss is journalled with where and to what, and
/// the opening then yields exactly as shipped (`opening-yields-to-walls`).
#[test]
fn a_lost_breaker_is_journalled_and_the_opening_yields_as_shipped() {
    let mut reasons = Vec::new();
    for gene in [false, true] {
        let (mut game, mut ai, capital) = opened(gene);
        let city = game.cities[&capital].pos;
        let tile = game
            .wring(city, 2)
            .into_iter()
            .find(|pos| game.unit_ids_at(*pos).is_empty())
            .unwrap();
        let shield = unit(&mut game, "swordsman", 0, tile);
        let ram = unit(&mut game, "battering_ram", 0, tile);
        let other = game
            .wring(city, 4)
            .into_iter()
            .find(|pos| game.unit_ids_at(*pos).is_empty())
            .unwrap();
        let body = unit(&mut game, "swordsman", 0, other);
        at_war(&mut game, 0, 1);
        {
            let opening = ai.conquest_opening.as_mut().unwrap();
            opening.declared = Some(game.turn);
            opening.force = BTreeSet::from([body, shield]);
        }
        ai.breaker_shield_watch(&game, 0);
        assert_eq!(ai.breaker_shield_memory.is_some(), gene);
        game.remove_unit(ram);
        game.remove_unit(shield);
        unit(&mut game, "warrior", 1, tile);
        game.turn += 1;
        ai.maintain_conquest_opening(&mut game, 0);
        assert!(ai.conquest_opening.is_none(), "the opening yields");
        let lines = headlines(&ai);
        let release = lines
            .iter()
            .position(|(head, _)| head.starts_with("Releasing the conquest opening"))
            .expect("released");
        reasons.push(lines[release].1.clone());
        let lost = lines.iter().position(|(head, _)| {
            head.starts_with("Breaker under a shield: the battering ram is lost before")
        });
        assert_eq!(lost.is_some(), gene);
        if let Some(lost) = lost {
            assert!(lost < release, "the loss reads before the release");
            let detail = &lines[lost].1;
            assert!(
                detail.starts_with(&format!("captured at {tile:?} by the warrior of")),
                "{detail}"
            );
            assert!(
                detail.contains("its shield, the swordsman, is gone too"),
                "{detail}"
            );
        }
    }
    assert_eq!(reasons[0], reasons[1], "released as shipped");
    assert_eq!(
        reasons[0],
        "the target walled up before the strike landed and no breaker is held"
    );
}

/// A live rebuild carries the memory by the mirror's unit map.
#[test]
fn the_memory_follows_a_live_rebuild() {
    let (mut game, mut ai, _) = opened(true);
    let ram = unit(&mut game, "battering_ram", 0, (6, 12));
    let sword = unit(&mut game, "swordsman", 0, (7, 12));
    ai.maintain_conquest_opening(&mut game, 0);
    ai.breaker_shield_watch(&game, 0);
    let units = std::collections::BTreeMap::from([(ram, 900), (sword, 901)]);
    ai.remap_breaker_shield_memory(&units);
    let memory = ai.breaker_shield_memory.as_ref().unwrap();
    assert_eq!((memory.breaker, memory.shield), (Some(900), Some(901)));
    ai.remap_breaker_shield_memory(&std::collections::BTreeMap::new());
    assert_eq!(ai.breaker_shield_memory.as_ref().unwrap().breaker, None);
}

// ------------------------------------- the first live city-state war's shape

/// Live `civvis-20261111T072641Z`: the stone (Auckland, 9 tiles, walls 100,
/// defence 22-28, no garrison) was declared on at t36 with 4 Warriors, 3
/// Archers and the Ram; the siege train staged for 14 turns ("nothing to open
/// the walls, so the train holds outside the city's reach", "falls back to
/// the muster line") with 0 damage, and the opening yielded at t50. Here the
/// same force stands at the rally of a city-state at defence 25 behind 100
/// walls, the war is declared, and the seat plays whole turns with the siege
/// train armed: within six turns the Ram stands beside the city under its
/// shield, the melee have struck the walls, and no siege line holds the
/// opening's force outside the city's reach.
#[test]
fn the_city_state_strike_walks_in_under_the_shield_and_hits_the_walls() {
    use crate::ai::Ai;
    let mut game = Game::new_full(4, 60, 30, 5_051_001, 1_000, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.barb_camps.clear();
    game.barb_naval_camps.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    game.found_city_for(0, (6, 12), None);
    game.found_city_for(0, (6, 18), None);
    game.found_city_for(1, (30, 12), None);
    let near = game.found_city_for(2, (14, 12), None);
    let far = game.found_city_for(3, (17, 12), None);
    for minor in [2, 3] {
        game.players[minor].is_minor = true;
    }
    for cid in [near, far] {
        set_walls(&mut game, cid, 100);
    }
    for cid in game.cities.keys().copied().collect::<Vec<_>>() {
        game.cities.get_mut(&cid).unwrap().queue.clear();
    }
    for other in 1..4 {
        game.record_contact(0, other);
    }
    let tiles: Vec<Pos> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(tiles);
    game.at_war.clear();
    game.game_speed = crate::setup::GameSpeed::Online;
    game.turn = 30;
    game.current = 0;

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_early_conquest_opening();
    ai.enable_conquest_opening_stays_near();
    ai.enable_early_conquest_stands_down();
    ai.enable_declaration_needs_production_parity();
    ai.enable_domination_ignores_city_states();
    ai.enable_first_capital_before_the_walls();
    ai.enable_city_state_before_the_first_capital();
    ai.enable_opening_yields_to_walls();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.enable_stage_musters_out_of_reach();
    ai.conquest_opening_needs_the_production = false;
    ai.enable_breaker_walks_under_a_shield();
    ai.base.journal = crate::reasoning::Journal::recording();

    ai.maintain_conquest_opening(&mut game, 0);
    let rally = ai
        .conquest_opening
        .as_ref()
        .expect("the stone is named")
        .rally;
    assert_eq!(ai.conquest_opening.as_ref().unwrap().city, near);
    let city = game.cities[&near].pos;
    let mut tiles: Vec<Pos> = game.wring(rally, 1);
    tiles.extend(game.wring(rally, 2));
    let tiles: Vec<Pos> = tiles
        .into_iter()
        .filter(|pos| {
            game.city_at(*pos).is_none()
                && game.unit_ids_at(*pos).is_empty()
                && game.wdist(*pos, city) >= 2
        })
        .collect();
    let force: Vec<u32> = [
        "warrior", "warrior", "warrior", "warrior", "archer", "archer", "archer",
    ]
    .iter()
    .zip(tiles)
    .map(|(kind, pos)| unit(&mut game, kind, 0, pos))
    .collect();
    assert_eq!(force.len(), 7, "the ring had room");
    let ram = unit(&mut game, "battering_ram", 0, rally);
    ai.maintain_conquest_opening(&mut game, 0);
    assert!(ai.conquest_declaration(&mut game, 0), "the war opens");
    set_strength(&mut game, near, 25.0);
    let _ = game.apply(0, &Action::EndTurn);

    let warriors: Vec<u32> = force
        .iter()
        .copied()
        .filter(|uid| game.units[uid].kind == "warrior")
        .collect();
    let ours: Vec<u32> = game.player_city_ids(0);
    let mut beside_on = None;
    let mut most_beside = 0;
    for turn in 0..6 {
        let mut guard = 0;
        while game.current != 0 && guard < 16 {
            let seat = game.current;
            let _ = game.apply(seat, &Action::EndTurn);
            guard += 1;
        }
        assert_eq!(game.current, 0, "our turn comes round");
        ai.take_turn(&mut game, 0);
        assert!(
            game.units.contains_key(&ram),
            "turn {turn}: the ram was lost"
        );
        assert!(
            shielded(&game, ram),
            "turn {turn}: the ram ends our turn alone at {:?}",
            game.units[&ram].pos
        );
        if beside_on.is_none() && game.wdist(game.units[&ram].pos, city) <= 1 {
            beside_on = Some(turn);
        }
        most_beside = most_beside.max(
            warriors
                .iter()
                .filter_map(|uid| game.units.get(uid))
                .filter(|unit| game.wdist(unit.pos, city) <= 1)
                .count(),
        );
        // Nothing new is trained: every soldier on the board is the strike's.
        for cid in &ours {
            if let Some(home) = game.cities.get_mut(cid) {
                home.queue.clear();
            }
        }
        if game.current == 0 {
            let _ = game.apply(0, &Action::EndTurn);
        }
    }
    let lines = headlines(&ai);
    let trace: Vec<String> = lines
        .iter()
        .filter(|(head, _)| head.contains("Breaker under a shield") || head.contains("Siege of"))
        .map(|(head, detail)| format!("{head} | {detail}"))
        .collect();
    assert!(
        beside_on.is_some(),
        "the ram never stood beside the city:\n{}",
        trace.join("\n")
    );
    assert!(
        game.cities
            .get(&near)
            .is_none_or(|c| c.owner == 0 || c.wall_hp < 100),
        "the walls stand untouched:\n{}",
        trace.join("\n")
    );
    assert!(
        lines
            .iter()
            .any(|(head, _)| head.starts_with("Breaker under a shield: the ram stands beside")),
        "no strike beside the ram:\n{}",
        trace.join("\n")
    );
    assert!(
        most_beside >= 3,
        "the warriors never closed on the walls ({most_beside} beside at most):\n{}",
        trace.join("\n")
    );
    let held: Vec<&String> = trace
        .iter()
        .filter(|line| {
            line.starts_with("Siege of")
                && (line.contains("outside the city's reach")
                    || line.contains("muster")
                    || line.contains(": stage |"))
        })
        .collect();
    assert!(
        held.is_empty(),
        "the siege train held the opening's force:\n{}",
        trace.join("\n")
    );
}
