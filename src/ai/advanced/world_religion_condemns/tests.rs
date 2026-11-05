use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::CongressEffect;

/// Our Warrior beside a Missionary of `faith` at peace, seat 2 its owner.
fn fixture(faith: &str, adjacent: bool) -> (Game, AdvancedAi, u32, u32) {
    let mut g = Game::new_full(3, 36, 26, 374_600, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (5, 5), None);
    g.found_city_for(2, (23, 13), None);
    g.players[2].religion = Some(faith.into());
    g.current = 0;
    let guard = g.spawn_test_unit("warrior", 0, (6, 5));
    let missionary = g.spawn_test_unit("missionary", 2, if adjacent { (7, 5) } else { (6, 5) });
    g.units.get_mut(&missionary).unwrap().religion = Some(faith.into());
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_world_religion_condemns_the_faith();
    (g, ai, guard, missionary)
}

fn ban(g: &mut Game, faith: &str) {
    let expires = g.turn + 20;
    g.active_congress_effects.push(CongressEffect {
        resolution: "world_religion".into(),
        outcome: "B".into(),
        target: faith.into(),
        expires,
    });
}

#[test]
fn a_soldier_condemns_a_banned_faith_at_peace() {
    for adjacent in [false, true] {
        let (mut g, mut ai, guard, missionary) = fixture("Orthodoxy", adjacent);
        ban(&mut g, "Orthodoxy");
        assert!(!g.is_at_war(0, 2), "the fixture is at peace");
        let favor = g.players[0].favor;
        assert!(ai.condemn_step(&mut g, 0, guard));
        assert!(!g.units.contains_key(&missionary), "the Missionary is gone");
        assert_eq!(g.players[0].favor, favor + 25, "option B pays 25 Favor");
    }
}

#[test]
fn another_faith_stands_at_peace() {
    let (mut g, mut ai, guard, missionary) = fixture("Orthodoxy", false);
    ban(&mut g, "Taoism");
    assert!(!ai.condemn_step(&mut g, 0, guard));
    assert!(g.units.contains_key(&missionary));
}

#[test]
fn no_ban_leaves_the_peace_alone() {
    let (mut g, mut ai, guard, missionary) = fixture("Orthodoxy", false);
    assert!(!ai.condemn_step(&mut g, 0, guard));
    assert!(g.units.contains_key(&missionary));
}

#[test]
fn off_the_ban_is_not_read() {
    let (mut g, _, guard, missionary) = fixture("Orthodoxy", false);
    ban(&mut g, "Orthodoxy");
    let mut stock = AdvancedAi::targeting(VictoryTarget::Science);
    assert!(!stock.condemn_step(&mut g, 0, guard));
    assert!(g.units.contains_key(&missionary));
}

#[test]
fn the_gene_is_opt_in() {
    opt_in_off_in_both_controllers("world-religion-condemns-the-faith", |ai| {
        ai.world_religion_condemns_the_faith
    });
}
