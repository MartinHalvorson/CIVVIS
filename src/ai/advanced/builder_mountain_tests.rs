//! `builders-route-around-mountains`: live Immortal 220550Z waited on oil
//! for its Artillery while its one owned oil stood unimproved: from
//! Professional Sports the board routed each Builder sent to it across a
//! mountain the host would not let it enter, and the job was retired as
//! forgotten three times.

use super::{AdvancedAi, GrandStrategy, VictoryTarget};
use crate::game::Game;
use crate::name::Name;
use crate::Pos;

/// Our city on grassland with every owned tile but `deposit` already farmed,
/// so the deposit's mine is the one Builder job. The tiles a Builder at
/// `start` would cross straight to it are mountains; the way round is open.
fn fixture() -> (Game, u32, Pos, Vec<Pos>) {
    let mut g = Game::new_full(2, 40, 24, 377_901, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    g.map.clear_rivers();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    let home = g.found_city_for(0, (8, 10), None);
    g.players[0].techs.extend(
        ["mining", "industrialization", "radio"]
            .into_iter()
            .map(Name::new),
    );
    g.players[0]
        .civics
        .insert(crate::name!("professional_sports"));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    let owned: Vec<Pos> = g.cities[&home].owned_tiles.to_vec();
    let city = g.cities[&home].pos;
    let deposit = *owned
        .iter()
        .filter(|pos| **pos != city)
        .max_by_key(|pos| (g.wdist(**pos, city), **pos))
        .expect("the city owns tiles");
    for pos in &owned {
        if *pos != city && *pos != deposit {
            g.map.tiles.get_mut(pos).unwrap().improvement = Some(crate::name!("farm"));
        }
    }
    g.map.tiles.get_mut(&deposit).unwrap().resource = Some(crate::name!("aluminum"));
    // A start two tiles past the deposit, away from the city, with every
    // tile the two share walled by mountains.
    let start = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.wdist(*pos, deposit) == 2 && g.wdist(*pos, city) > g.wdist(deposit, city))
        .min()
        .expect("a start beyond the deposit");
    let ridge: Vec<Pos> = g
        .nbrs(start)
        .into_iter()
        .filter(|pos| g.wdist(*pos, deposit) == 1)
        .collect();
    for pos in &ridge {
        let tile = g.map.tiles.get_mut(pos).unwrap();
        tile.terrain = crate::name!("mountain");
        tile.improvement = None;
    }
    let builder = g.spawn_test_unit("builder", 0, start);
    g.current = 0;
    assert!(g
        .valid_improvements(0, deposit)
        .contains(&crate::name!("mine")));
    (g, builder, deposit, ridge)
}

fn walk(g: &mut Game, ai: &mut AdvancedAi, builder: u32, turns: usize) -> Vec<Pos> {
    let mut stood = Vec::new();
    for _ in 0..turns {
        let Some(unit) = g.units.get_mut(&builder) else {
            break;
        };
        unit.moves_left = g.rules.units[unit.kind].moves;
        ai.advanced_builder_step(g, 0, builder, GrandStrategy::Conquest);
        if let Some(unit) = g.units.get(&builder) {
            stood.push(unit.pos);
        }
    }
    stood
}

#[test]
fn a_builder_walks_round_the_ridge_to_its_deposit_under_the_gene() {
    let (mut g, builder, deposit, ridge) = fixture();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_builders_route_around_mountains();
    g.observed_builders_avoid_mountains = true;
    let stood = walk(&mut g, &mut ai, builder, 6);
    assert!(
        stood.iter().all(|pos| !ridge.contains(pos)),
        "never on the ridge: {stood:?} against {ridge:?}"
    );
    assert_eq!(
        g.map.tiles[&deposit].improvement,
        Some(crate::name!("mine")),
        "the deposit is mined: {stood:?}"
    );
}

#[test]
fn without_the_rule_the_board_lets_a_builder_onto_the_ridge() {
    let (g, builder, _, ridge) = fixture();
    assert!(!g.observed_builders_avoid_mountains);
    assert!(ridge.iter().all(|pos| g.can_move(builder, *pos)));
}

#[test]
fn the_seat_sets_the_rule_from_the_gene_each_turn() {
    for gene in [false, true] {
        let (mut g, _, _, _) = fixture();
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        if gene {
            ai.enable_builders_route_around_mountains();
        }
        g.observed_builders_avoid_mountains = !gene;
        ai.take_turn_inner(&mut g, 0);
        assert_eq!(g.observed_builders_avoid_mountains, gene);
    }
}

/// The gene is a native opt-in, off in both controllers.
#[test]
fn builders_route_around_mountains_is_a_native_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "builders-route-around-mountains",
        |ai| ai.builders_route_around_mountains,
    );
}
