use super::*;

/// Two majors: our founded capital, a Settler of ours, every unit but it
/// removed, only the capital's own ring explored, and a land site beyond the
/// Loyalty reach. Returns the game, the Settler and the site.
fn fogged_frontier() -> (Game, u32, Pos) {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settlers: Vec<u32> = game
        .player_unit_ids(0)
        .into_iter()
        .filter(|uid| game.units[uid].kind == "settler")
        .collect();
    let founder = settlers[0];
    game.apply(0, &Action::FoundCity { unit: founder })
        .expect("fixture: the capital founds");
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let settler = game.spawn_unit("settler", 0, home);
    game.players[0].explored = game.wdisk(home, 2).into_iter().collect();
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.wdist(**pos, home) == FRONTIER_LOYALTY_RADIUS + 2 && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.turn = 60;
    (game, settler, sites[0])
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.frontier_loyalty = true;
    if gene {
        ai.enable_fog_yields_to_a_stranded_settler();
    }
    ai
}

fn fog_refusal(verdict: Option<String>) -> bool {
    verdict.is_some_and(|why| why.contains("beyond the empire's Loyalty reach"))
}

/// A Settler thirty turns into its walk lifts the fog clause; off, or with a
/// fresh Settler, the clause refuses the site.
#[test]
fn a_stranded_settler_lifts_the_fog_veto() {
    let (game, settler, site) = fogged_frontier();
    assert!(
        AdvancedAi::beyond_loyalty_reach(&game, 0, site),
        "fixture: the site is beyond reach on fogged ground"
    );

    let mut on = seat(true);
    on.settler_walk_started.insert(settler, game.turn - 30);
    assert!(on.fog_veto_yields(&game, 0));
    assert!(!fog_refusal(
        on.settle_site_frontier_loyalty_verdict(&game, 0, site)
    ));
    assert!(on.fog_yield_before_founding(&game, 0, site));

    let mut off = seat(false);
    off.settler_walk_started.insert(settler, game.turn - 30);
    assert!(fog_refusal(
        off.settle_site_frontier_loyalty_verdict(&game, 0, site)
    ));

    let mut fresh = seat(true);
    fresh.settler_walk_started.insert(settler, game.turn - 2);
    assert!(!fresh.fog_veto_yields(&game, 0));
    assert!(fog_refusal(
        fresh.settle_site_frontier_loyalty_verdict(&game, 0, site)
    ));
}

/// A walk clock left by a Settler that has since founded or died lifts
/// nothing.
#[test]
fn only_a_living_settler_counts() {
    let (mut game, settler, _) = fogged_frontier();
    let mut ai = seat(true);
    ai.settler_walk_started.insert(settler, game.turn - 30);
    game.remove_unit(settler);
    assert!(!ai.fog_veto_yields(&game, 0));
}
