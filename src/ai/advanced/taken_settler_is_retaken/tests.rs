//! `taken-settler-is-retaken`: a Settler taken from us is remembered by its
//! captor, and the hostile standing on it is struck for it.
//!
//! The boards copy the live one of `hotseat-20261128T031456Z` p1 turn 12
//! (`events.jsonl` line 290): our Warrior with both moves beside the
//! person's Scout (strength 10, full health) and the Settler it took from us,
//! stacked on one tile.

use super::*;
use crate::doctrine::{build, position};
use crate::game::{Game, HostStrikePreview};
use crate::hex;
use crate::Pos;
use std::collections::BTreeMap;
use std::sync::Arc;

/// The battle planner's own test field: an arena, so at war by rule.
fn open_field() -> Game {
    let mut g = build(position("the_reserve").expect("known"), 3).expect("buildable");
    let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
    for uid in seeded {
        g.remove_unit(uid);
    }
    g
}

/// A plain world of two empires at peace, for the readings that ask whether
/// a war is on (an arena is at war whatever `at_war` says).
fn world() -> Game {
    let mut g = Game::new_full(2, 36, 22, 7_311, 1_000, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.at_war.clear();
    g.turn = 60;
    g
}

fn at(col: i32, row: i32) -> Pos {
    hex::offset_to_axial(col, row)
}

/// The host's reading of one melee pair, as a `preview` event files it.
fn host_preview(g: &mut Game, uid: u32, target: Pos, to_attacker: i32, to_defender: i32) {
    let mut previews = (*g.host_previews).clone();
    previews.insert(
        (uid, target, false),
        HostStrikePreview {
            attacker_strength: 0.0,
            defender_strength: 0.0,
            damage_to_attacker: to_attacker,
            damage_to_defender: to_defender,
            defender_wall_damage: 0,
        },
    );
    g.host_previews = Arc::new(previews);
}

/// The live seat's battle planner, with or without the gene.
fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_3();
    if gene {
        ai.enable_taken_settler_is_retaken();
    }
    ai
}

/// Turn 11: our Warrior on (9, 7) and our Settler on (10, 7). Turn 12: the
/// rebuilt host board no longer carries our Settler, and the person's Scout
/// stands on (10, 7) with a Settler of theirs. Returns the board, our
/// Warrior, the Scout and the captive.
fn taken_in(mut g: Game, ai: &mut AdvancedAi) -> (Game, u32, u32, u32) {
    let warrior = g.spawn_unit("warrior", 0, at(9, 7));
    let settler = g.spawn_unit("settler", 0, at(10, 7));
    ai.observe_taken_civilians(&g, 0);
    g.remove_unit(settler);
    let scout = g.spawn_unit("scout", 1, at(10, 7));
    let captive = g.spawn_unit("settler", 1, at(10, 7));
    g.turn += 1;
    ai.observe_taken_civilians(&g, 0);
    (g, warrior, scout, captive)
}

fn taken_board(ai: &mut AdvancedAi) -> (Game, u32, u32, u32) {
    taken_in(open_field(), ai)
}

fn strikes(ai: &AdvancedAi, g: &Game, unit: u32, target: Pos) -> bool {
    ai.kill_sequence(g, 0)
        .iter()
        .any(|blow| blow.unit == unit && blow.target == target)
}

#[test]
fn taken_settler_is_retaken_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.taken_settler_is_retaken, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "taken_settler_is_retaken"));
    super::super::test_support::opt_in_off_in_both_controllers("taken-settler-is-retaken", |ai| {
        ai.taken_settler_is_retaken
    });
}

/// The captive is remembered by its captor and identity, whether the board
/// drops our unit and shows theirs (a rebuilt host board) or keeps the unit
/// under its new owner (a native board). With the gene off nothing is noted.
#[test]
fn a_settler_taken_beside_its_warrior_is_remembered_by_its_captor() {
    let mut ai = seat(true);
    let (g, _, _, captive) = taken_board(&mut ai);
    assert_eq!(
        ai.taken_civilians.keys().copied().collect::<Vec<_>>(),
        vec![(1, i64::from(captive))]
    );
    let settler = captive_value(&g, crate::name!("settler"));
    assert_eq!(
        settler,
        g.rules.units["settler"].cost + objective_board::SETTLER_PREMIUM
    );
    assert_eq!(ai.taken_civilian_value_at(&g, 0, at(10, 7)), settler);
    assert_eq!(ai.taken_civilian_value_at(&g, 0, at(9, 7)), 0.0);
    assert!(ai.holds_a_taken_civilian(&g, 0, at(10, 7)));

    // A native board hands the same unit to its captor.
    let mut native = seat(true);
    let mut g = open_field();
    g.spawn_unit("warrior", 0, at(9, 7));
    let settler = g.spawn_unit("settler", 0, at(10, 7));
    native.observe_taken_civilians(&g, 0);
    g.units.get_mut(&settler).expect("the settler").owner = 1;
    g.spawn_unit("scout", 1, at(10, 7));
    g.turn += 1;
    native.observe_taken_civilians(&g, 0);
    assert_eq!(
        native.taken_civilians.keys().copied().collect::<Vec<_>>(),
        vec![(1, i64::from(settler))]
    );

    let mut off = seat(false);
    let (g, _, _, _) = taken_board(&mut off);
    assert!(off.taken_civilians.is_empty() && off.taken_civilian_watch.is_empty());
    assert_eq!(off.taken_civilian_value_at(&g, 0, at(10, 7)), 0.0);
}

/// What is not a capture: a Settler that founded where it stood, a rival
/// Settler that was already in sight, a rival at peace with us, and one too
/// far from where ours was last seen.
#[test]
fn a_founding_a_known_settler_a_peaceful_rival_and_a_far_one_are_not_captures() {
    // Founded: our city stands where the Settler stood.
    let mut ai = seat(true);
    let mut g = open_field();
    let settler = g.spawn_unit("settler", 0, at(10, 7));
    ai.observe_taken_civilians(&g, 0);
    g.remove_unit(settler);
    g.found_city_for(0, at(10, 7), None);
    g.spawn_unit("settler", 1, at(11, 7));
    g.turn += 1;
    ai.observe_taken_civilians(&g, 0);
    assert!(ai.taken_civilians.is_empty(), "a founding is not a capture");

    // Already in sight before ours left the board.
    let mut ai = seat(true);
    let mut g = open_field();
    let settler = g.spawn_unit("settler", 0, at(10, 7));
    g.spawn_unit("settler", 1, at(11, 7));
    ai.observe_taken_civilians(&g, 0);
    g.remove_unit(settler);
    g.turn += 1;
    ai.observe_taken_civilians(&g, 0);
    assert!(
        ai.taken_civilians.is_empty(),
        "their own Settler, seen before"
    );

    // At peace: nobody could have taken it. The same capture at war counts.
    let mut ai = seat(true);
    taken_in(world(), &mut ai);
    assert!(
        ai.taken_civilians.is_empty(),
        "a rival at peace takes nothing"
    );
    let mut at_war = world();
    at_war.at_war.insert((0, 1));
    let mut ai = seat(true);
    let (_, _, _, captive) = taken_in(at_war, &mut ai);
    assert_eq!(
        ai.taken_civilians.keys().copied().collect::<Vec<_>>(),
        vec![(1, i64::from(captive))]
    );

    // Beyond the reach of the tile ours was last seen on.
    let mut ai = seat(true);
    let mut g = open_field();
    let settler = g.spawn_unit("settler", 0, at(6, 7));
    ai.observe_taken_civilians(&g, 0);
    g.remove_unit(settler);
    let far = g.spawn_unit("settler", 1, at(12, 7));
    assert!(g.wdist(at(6, 7), g.units[&far].pos) > TAKEN_REACH);
    g.turn += 1;
    ai.observe_taken_civilians(&g, 0);
    assert!(ai.taken_civilians.is_empty(), "too far to be ours");
}

/// A blow the plain planner declines — 25 dealt on a full-health Scout for
/// 34 taken — is planned when the Scout stands on our Settler, and is still
/// declined when the Settler on its tile is the person's own.
#[test]
fn the_captor_is_struck_for_the_settler_it_holds() {
    let mut off = seat(false);
    let (mut g, warrior, _, _) = taken_board(&mut off);
    host_preview(&mut g, warrior, at(10, 7), 34, 25);
    assert!(
        !strikes(&off, &g, warrior, at(10, 7)),
        "without the gene the wound is worth less than the blow back"
    );
    let mut on = seat(true);
    let (mut g, warrior, _, _) = taken_board(&mut on);
    host_preview(&mut g, warrior, at(10, 7), 34, 25);
    assert!(
        strikes(&on, &g, warrior, at(10, 7)),
        "with it the wound carries a quarter of the Settler"
    );

    // The person's own Settler under its own Scout is not ours to free.
    let mut own = seat(true);
    let mut g = open_field();
    let warrior = g.spawn_unit("warrior", 0, at(9, 7));
    own.observe_taken_civilians(&g, 0);
    g.spawn_unit("scout", 1, at(10, 7));
    g.spawn_unit("settler", 1, at(10, 7));
    g.turn += 1;
    own.observe_taken_civilians(&g, 0);
    assert!(own.taken_civilians.is_empty());
    host_preview(&mut g, warrior, at(10, 7), 34, 25);
    assert!(!strikes(&own, &g, warrior, at(10, 7)));
}

/// The live reading: 32 dealt, 28 taken (`events.jsonl` line 294). The seat
/// fortified there; with the gene the blow is planned at the host's damage.
#[test]
fn the_live_blow_on_the_captor_is_planned() {
    let mut on = seat(true);
    let (mut g, warrior, scout, _) = taken_board(&mut on);
    host_preview(&mut g, warrior, at(10, 7), 28, 32);
    let blows = on.kill_sequence(&g, 0);
    let blow = blows
        .iter()
        .find(|blow| blow.unit == warrior)
        .expect("the Warrior strikes the Scout");
    assert_eq!(blow.target, at(10, 7));
    assert_eq!(blow.defender, scout);
    assert_eq!(blow.expected, 32.0);
    assert!(!blow.ranged);
}

/// A captive is forgotten once its captor is at peace with us, and after
/// `TAKEN_MEMORY_TURNS` standard turns.
#[test]
fn a_captive_is_forgotten_at_peace_and_after_its_time() {
    let mut ai = seat(true);
    let (mut g, _, _, _) = taken_board(&mut ai);
    assert_eq!(ai.taken_civilians.len(), 1);
    g.turn += g.standard_duration(TAKEN_MEMORY_TURNS) - 1;
    ai.observe_taken_civilians(&g, 0);
    assert_eq!(ai.taken_civilians.len(), 1, "still inside its time");
    g.turn += 1;
    ai.observe_taken_civilians(&g, 0);
    assert!(ai.taken_civilians.is_empty(), "past its time");

    let mut at_war = world();
    at_war.at_war.insert((0, 1));
    let mut ai = seat(true);
    let (mut g, _, _, _) = taken_in(at_war, &mut ai);
    assert_eq!(ai.taken_civilians.len(), 1);
    g.at_war.clear();
    g.turn += 1;
    ai.observe_taken_civilians(&g, 0);
    assert!(ai.taken_civilians.is_empty(), "the captor made peace");
}

/// A rebuilt board re-keys the watch and keeps what it dropped for the next
/// turn's reading.
#[test]
fn a_rebuilt_board_carries_the_watch() {
    let mut ai = seat(true);
    let mut g = open_field();
    let kept = g.spawn_unit("builder", 0, at(4, 7));
    let dropped = g.spawn_unit("settler", 0, at(10, 7));
    ai.observe_taken_civilians(&g, 0);
    let map: BTreeMap<u32, u32> = [(kept, 900)].into_iter().collect();
    ai.remap_taken_civilian_watch(&map);
    assert_eq!(
        ai.taken_civilian_watch.keys().copied().collect::<Vec<_>>(),
        vec![900]
    );
    assert_eq!(
        ai.taken_civilian_vanished,
        vec![(at(10, 7), g.units[&dropped].kind)]
    );
    ai.forget_taken_civilians();
    assert!(ai.taken_civilian_watch.is_empty() && ai.taken_civilian_vanished.is_empty());
}
