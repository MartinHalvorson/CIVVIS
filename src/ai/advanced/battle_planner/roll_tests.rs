//! `slots-survive-the-roll`: the positions plan reads the top of the roll,
//! not its centre, when it asks whether a slot or an end tile removes a unit.

use super::*;
use crate::hex;

fn open_field() -> Game {
    let mut g = crate::doctrine::build(
        crate::doctrine::position("the_reserve").expect("fixture"),
        3,
    )
    .expect("buildable");
    for pid in 0..2 {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    g
}

fn at(col: i32, row: i32) -> Pos {
    hex::offset_to_axial(col, row)
}

fn version_two(roll: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    if roll {
        ai.enable_slots_survive_the_roll();
    }
    ai
}

#[test]
fn slots_survive_the_roll_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.slots_survive_the_roll, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "slots_survive_the_roll"));
    super::super::test_support::opt_in_off_in_both_controllers("slots-survive-the-roll", |ai| {
        ai.slots_survive_the_roll
    });
}

/// A slot beside an enemy warrior, our warrior wounded to a health between
/// the enemy's centre blow and the top of its roll there: version two
/// assigns it, the gene refuses it.
#[test]
fn a_slot_the_top_of_the_roll_removes_the_unit_on_is_never_assigned() {
    let slot = at(10, 6);
    let mut found = None;
    for hp in 1..=100 {
        let mut g = open_field();
        let ours = g.spawn_unit("warrior", 0, at(8, 6));
        g.spawn_unit("warrior", 1, at(11, 6));
        g.units.get_mut(&ours).expect("ours").hp = hp;
        let mut field = DangerField::new(&g, 0);
        let centre = field.danger(slot, ours);
        let top = field.upper_danger(slot, ours);
        if centre > 0.0 && centre < f64::from(hp) && top >= f64::from(hp) {
            found = Some((g, ours, hp, centre, top));
            break;
        }
    }
    let (g, ours, hp, centre, top) =
        found.expect("some health sits between the centre blow and the top of its roll");
    let reachable = g.reachable(ours);
    let mut field = DangerField::new(&g, 0);
    let off = version_two(false).slot_cost(&g, &mut field, ours, slot, &reachable);
    let on = version_two(true).slot_cost(&g, &mut field, ours, slot, &reachable);
    assert!(
        off < UNASSIGNABLE,
        "version two prices the slot at {off}: centre {centre} < {hp} hp"
    );
    assert_eq!(
        on, UNASSIGNABLE,
        "the top of the roll ({top}) meets {hp} hp: never assigned"
    );
}

/// A unit far short of its slot, which lies past an enemy warrior: version
/// two ends it on the reachable tile nearest the slot, inside the warrior's
/// reach, where the warrior's blow removes it; the gene ends it on the
/// nearest tile the top of the roll does not remove it on.
#[test]
fn a_unit_short_of_its_slot_ends_where_the_roll_does_not_remove_it() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(4, 6));
    g.spawn_unit("warrior", 1, at(9, 6));
    g.units.get_mut(&ours).expect("ours").hp = 20;
    let slot = at(13, 6);
    let plan = PositionPlan {
        group: ours,
        target: slot,
        contacts: 1,
        slots: Vec::new(),
        placements: Vec::new(),
        pace_floor: None,
    };
    let reserved = BTreeSet::new();
    let mut field = DangerField::new(&g, 0);
    let (off, _) =
        version_two(false).position_end_tile(&g, &mut field, ours, slot, &reserved, &plan);
    let (on, _) = version_two(true).position_end_tile(&g, &mut field, ours, slot, &reserved, &plan);
    assert!(
        field.upper_danger(off, ours) >= 20.0,
        "version two walks to {off:?}, nearest the slot, where the roll removes it"
    );
    assert!(
        field.upper_danger(on, ours) < 20.0,
        "the gene ends at {on:?}, which the roll does not remove it on"
    );
    assert!(
        g.reachable(ours).contains(&on) || on == g.units[&ours].pos,
        "a tile it can reach this turn"
    );
    // Among such tiles, still the nearest the slot.
    let mut safe: Vec<Pos> = vec![g.units[&ours].pos];
    safe.extend(g.reachable(ours));
    let best = safe
        .into_iter()
        .filter(|tile| field.upper_danger(*tile, ours) < 20.0)
        .map(|tile| g.wdist(tile, slot))
        .min()
        .expect("a safe tile");
    assert_eq!(g.wdist(on, slot), best);
}
