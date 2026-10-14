use super::*;
use std::collections::HashSet;

/// A cleared two-player board with a rival Orthodox Apostle on a land tile
/// and our Buddhist Missionary on a land tile its strike envelope reaches,
/// chosen so a single step leaves the envelope. Returns the game, our unit,
/// the rival and the envelope (every tile the rival can strike).
fn chased() -> Option<(Game, u32, u32, HashSet<Pos>)> {
    let mut game = Game::new_full(2, 32, 20, 4_242, 200, 0, false);
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let mut land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| game.rules.is_passable(tile) && !game.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    for rival_at in land.iter().copied() {
        let mut probe = game.clone();
        let rival = probe.spawn_unit("apostle", 1, rival_at);
        probe.units.get_mut(&rival)?.religion = Some("orthodoxy".into());
        let mut stands: HashSet<Pos> = probe.threat_reach(rival).into_iter().collect();
        stands.insert(rival_at);
        let envelope: HashSet<Pos> = land
            .iter()
            .copied()
            .filter(|pos| {
                stands
                    .iter()
                    .any(|from| *from != *pos && probe.wdist(*from, *pos) <= 1)
            })
            .collect();
        let edge = land.iter().copied().find(|pos| {
            envelope.contains(pos)
                && !stands.contains(pos)
                && probe
                    .nbrs(*pos)
                    .into_iter()
                    .any(|next| land.contains(&next) && !envelope.contains(&next))
        });
        if let Some(ours_at) = edge {
            let mine = probe.spawn_unit("missionary", 0, ours_at);
            probe.units.get_mut(&mine)?.religion = Some("buddhism".into());
            return Some((probe, mine, rival, envelope));
        }
    }
    None
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_spreader_steps_out_of_the_apostle();
    ai
}

/// Off, nothing is read; on, the rival that would win the fight is a threat.
#[test]
fn a_winning_rival_apostle_is_read_only_with_the_gene() {
    let Some((game, mine, _rival, _)) = chased() else {
        panic!("fixture: a land tile at the edge of the Apostle's reach");
    };
    assert!(AdvancedAi::new()
        .theological_threats(&game, 0, mine)
        .is_empty());
    assert_eq!(armed().theological_threats(&game, 0, mine).len(), 1);
}

/// On, the threatened Missionary steps out of the envelope; off, the flee
/// reads nothing and the caller goes on with its turn.
#[test]
fn the_missionary_steps_out_of_the_apostles_reach() {
    let Some((game, mine, _rival, envelope)) = chased() else {
        panic!("fixture: a land tile at the edge of the Apostle's reach");
    };
    let mut off = game.clone();
    assert_eq!(
        AdvancedAi::new().religious_unit_evades_apostles(&mut off, 0, mine),
        None
    );

    let mut on = game.clone();
    let acted = armed().religious_unit_evades_apostles(&mut on, 0, mine);
    assert_eq!(acted, Some(true), "on: the flee acts");
    let after = on.units[&mine].pos;
    assert!(
        !envelope.contains(&after),
        "on: the new tile is out of reach ({after:?})"
    );
}

/// The march never steps into the envelope toward a target beyond it.
#[test]
fn the_march_does_not_walk_into_the_reach() {
    let Some((mut game, mine, rival, envelope)) = chased() else {
        panic!("fixture: a land tile at the edge of the Apostle's reach");
    };
    // Walk ours one step out first, then aim it at the rival's own tile.
    let ai = armed();
    assert_eq!(
        ai.religious_unit_evades_apostles(&mut game, 0, mine),
        Some(true)
    );
    game.units.get_mut(&mine).unwrap().moves_left = 4.0;
    let target = game.units[&rival].pos;
    let before = game.units[&mine].pos;
    let _ = ai.religious_step_toward_range(&mut game, 0, mine, target, 1);
    let after = game.units[&mine].pos;
    assert!(
        after == before || !envelope.contains(&after),
        "on: the step stays out of reach ({before:?} -> {after:?})"
    );
}

/// The probe reads the shipped preview on the unit's own tile, and a tile of
/// ours beside our own unit of the faith reads it stronger, never weaker.
#[test]
fn the_probe_preview_matches_the_shipped_one_in_place() {
    let Some((game, mine, rival, _)) = chased() else {
        panic!("fixture: a land tile at the edge of the Apostle's reach");
    };
    let here = game.units[&mine].pos;
    assert_eq!(
        game.theological_exchange_preview_with_defender_at(rival, mine, here),
        game.theological_exchange_preview(rival, here),
    );
    // Off its tile the unit does not count itself as a neighbour of its faith.
    let beside = game
        .nbrs(here)
        .into_iter()
        .find(|pos| game.units_at(*pos).is_empty())
        .unwrap();
    let mut alone = game.clone();
    alone.remove_unit(mine);
    let ghost = alone.spawn_unit("missionary", 0, beside);
    alone.units.get_mut(&ghost).unwrap().religion = Some("buddhism".into());
    assert_eq!(
        game.theological_exchange_preview_with_defender_at(rival, mine, beside),
        alone.theological_exchange_preview(rival, beside),
    );
}
