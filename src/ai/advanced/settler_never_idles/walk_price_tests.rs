use super::*;

#[test]
fn an_older_exhausted_settler_prices_the_remaining_walk() {
    let mut g = Game::new_full(1, 30, 20, 91_301, 250, 0, false);
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let home = g.units[&settler].pos;
    g.found_city_for(0, home, None);
    let explored: Vec<Pos> = g.map.tiles.keys().copied().collect();
    g.players[0].explored.extend(explored);
    g.turn = 60;
    let mut ai = AdvancedAi::new();
    ai.enable_engine_repairs();
    ai.enable_settler_never_idles();
    ai.settle_sooner = false;
    ai.settler_walk_started.insert(settler, 30);
    let unpriced = ai.settler_exhaustion_target(&g, 0, settler).unwrap();
    ai.settle_sooner = true;
    let priced = ai.settler_exhaustion_target(&g, 0, settler).unwrap();
    assert_eq!(
        g.wdist(home, unpriced),
        10,
        "fixture: the unpriced fallback walks ten tiles"
    );
    assert_eq!(
        g.wdist(home, priced),
        4,
        "the older settler should take the nearer legal site"
    );
    assert!(ai.base.valid_settle_site(&g, 0, priced));
    assert!(g.route_step(settler, priced, 0).is_some());
    ai.settle_sooner = false;
    assert_eq!(ai.settler_exhaustion_target(&g, 0, settler), Some(unpriced));
}
