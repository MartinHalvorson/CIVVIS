use super::*;
use crate::game::{Action, Game};

/// An air unit the envelopes count: military, with an attack.
fn strike_aircraft(g: &Game) -> String {
    g.rules
        .units
        .iter()
        .find(|(_, spec)| {
            spec.domain.as_deref() == Some("air")
                && spec.class == "military"
                && (spec.is_melee_capable() || spec.has_ranged_attack())
        })
        .map(|(name, _)| name.as_str().to_string())
        .expect("the ruleset has a strike aircraft")
}

#[test]
fn an_air_envelope_is_kept_while_its_base_and_range_stand() {
    let mut g = Game::new_full(2, 24, 16, 5_151, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.cities.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.owner_city = None;
    }
    g.record_contact(0, 1);
    g.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
    let positions: Vec<Pos> = g.map.tiles.keys().copied().collect();
    let base = positions[positions.len() / 2];
    let kind = strike_aircraft(&g);
    let flier = g.spawn_test_unit(&kind, 1, base);
    let walker = g.spawn_test_unit("warrior", 0, positions[2]);
    let ai = BasicAi::new();
    let reach_of = |envelopes: &AttackEnvelopes| {
        envelopes
            .iter()
            .find(|(id, _)| *id == flier)
            .map(|(_, reach)| std::sync::Arc::clone(reach))
            .expect("the aircraft has an envelope")
    };
    let first = reach_of(&ai.enemy_attack_envelopes(&g, 0));
    assert_eq!(*first, EnvelopeReach::from_tiles(g.attack_reach(flier)));

    // A soldier of the asker's steps, and a tile inside the disk is written:
    // neither is anything the disk reads.
    let step = g
        .nbrs(positions[2])
        .into_iter()
        .find(|pos| g.unit_ids_at(*pos).is_empty())
        .unwrap();
    g.units.get_mut(&walker).unwrap().pos = step;
    let inside = g.nbrs(base).into_iter().next().unwrap();
    g.map.tiles.get_mut(&inside).unwrap().terrain = crate::name!("mountain");
    let kept = reach_of(&ai.enemy_attack_envelopes(&g, 0));
    assert!(
        std::sync::Arc::ptr_eq(&first, &kept),
        "nothing the disk reads moved"
    );
    assert_eq!(*kept, EnvelopeReach::from_tiles(g.attack_reach(flier)));

    // Flying a patrol moves the disk's centre. (The soldier steps again: the
    // whole-board key in front of this table does not hash a patrol, so a
    // board that changed nothing else is answered before any envelope is
    // looked at. In play a patrol is flown on its owner's turn and never
    // arrives alone.)
    {
        let unit = g.units.get_mut(&flier).unwrap();
        unit.air_patrol = true;
        unit.air_patrol_pos = Some(inside);
    }
    let back = g
        .nbrs(step)
        .into_iter()
        .find(|pos| g.unit_ids_at(*pos).is_empty())
        .unwrap();
    g.units.get_mut(&walker).unwrap().pos = back;
    let moved = reach_of(&ai.enemy_attack_envelopes(&g, 0));
    assert!(!std::sync::Arc::ptr_eq(&kept, &moved), "the base moved");
    assert_eq!(*moved, EnvelopeReach::from_tiles(g.attack_reach(flier)));
}
