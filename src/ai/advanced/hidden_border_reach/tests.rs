use super::*;

/// One major with its capital founded and every unit removed; the capital's
/// population as asked. Returns the game and the capital's position.
fn capital(pop: i32) -> (Game, Pos) {
    let mut game = Game::new_full(1, 32, 20, 936_431, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let cid = game.player_city_ids(0)[0];
    game.cities.get_mut(&cid).unwrap().pop = pop;
    let pos = game.cities[&cid].pos;
    (game, pos)
}

/// A site `distance` tiles from the capital, with an unseen rival border plot
/// two tiles past it, so the unresolved-border arm reads the site.
fn site_beside_an_unseen_border(game: &mut Game, home: Pos, distance: i32) -> Pos {
    let site = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == distance)
        .min()
        .expect("fixture: a site at that distance");
    let border = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, site) == 2 && game.wdist(*pos, home) > distance)
        .min()
        .expect("fixture: a border plot past the site");
    game.unseen_major_borders.insert(border);
    assert!(AdvancedAi::beside_unresolved_major_border(game, site));
    site
}

fn frontier(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_frontier_loyalty();
    if gene {
        ai.enable_hidden_border_yields_to_our_pressure();
    }
    ai
}

/// Our pressure is population times closeness over our cities within nine
/// tiles, and nothing past nine.
#[test]
fn own_pressure_is_population_times_closeness() {
    let (game, home) = capital(6);
    let at = |distance: i32| {
        game.map
            .tiles
            .keys()
            .copied()
            .find(|pos| game.wdist(*pos, home) == distance)
            .unwrap()
    };
    assert_eq!(AdvancedAi::own_loyalty_pressure(&game, 0, at(5)), 30);
    assert_eq!(AdvancedAi::own_loyalty_pressure(&game, 0, at(9)), 6);
    assert_eq!(AdvancedAi::own_loyalty_pressure(&game, 0, at(10)), 0);
}

/// Beside an unseen rival border, a site our six-citizen capital presses from
/// five tiles is settled with the gene on; off, or from a two-citizen capital,
/// the veto stands.
#[test]
fn the_hidden_border_veto_yields_to_our_pressure() {
    let (mut game, home) = capital(6);
    let site = site_beside_an_unseen_border(&mut game, home, 5);
    let veto = |ai: &AdvancedAi, game: &Game| {
        ai.settle_site_frontier_loyalty_verdict(game, 0, site)
            .is_some_and(|why| why.contains("border whose city may be hidden"))
    };
    assert!(veto(&frontier(false), &game), "off: the veto stands");
    assert!(!veto(&frontier(true), &game), "our pressure 30: it yields");

    let (mut small, home) = capital(2);
    let site_small = site_beside_an_unseen_border(&mut small, home, 5);
    assert!(
        frontier(true)
            .settle_site_frontier_loyalty_verdict(&small, 0, site_small)
            .is_some_and(|why| why.contains("border whose city may be hidden")),
        "our pressure 10: the veto stands"
    );
}

/// The founding telemetry reads our pressure only where this gene lifted the
/// veto: Some at the yielded site, None with the gene off or away from an
/// unseen border.
#[test]
fn the_founding_note_marks_only_a_site_the_gene_allowed() {
    let (mut game, home) = capital(6);
    let site = site_beside_an_unseen_border(&mut game, home, 5);
    assert_eq!(
        frontier(true).hidden_border_yield_before_founding(&game, 0, site),
        Some(30)
    );
    assert_eq!(
        frontier(false).hidden_border_yield_before_founding(&game, 0, site),
        None
    );
    game.unseen_major_borders.clear();
    assert_eq!(
        frontier(true).hidden_border_yield_before_founding(&game, 0, site),
        None,
        "no unseen border: the old veto would have passed it anyway"
    );
}
