use super::*;

/// `players` majors (met and at peace when two), our capital founded at
/// turn 20 at population 4 with every unit removed, a bank of `gold` and a
/// zero income. Returns the game and the capital.
fn young_empire(players: usize, gold: f64) -> Option<(Game, u32)> {
    let mut game = Game::new_full(players, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    for pid in 0..players {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    if players > 1 {
        let home = game.cities[&capital].pos;
        let mut sites: Vec<Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                (8..=12).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
            })
            .map(|(pos, _)| *pos)
            .collect();
        sites.sort();
        let site = *sites.first()?;
        game.place_city(1, site, None);
        game.players[0].met.insert(1);
        game.players[1].met.insert(0);
    }
    game.turn = 20;
    game.cities.get_mut(&capital).unwrap().pop = 4;
    game.players[0].gold = gold;
    game.players[0].gold_per_turn = 0.0;
    Some((game, capital))
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_settler();
    ai
}

fn settlers(game: &Game) -> usize {
    game.player_unit_ids(0)
        .into_iter()
        .filter(|uid| game.units[uid].kind == "settler")
        .count()
}

fn price(game: &Game, capital: u32) -> f64 {
    game.unit_purchase_cost(0, capital, "settler", "gold")
        .expect("fixture: the capital can buy a Settler")
}

/// A full bank buys the Settler with the gene on and leaves the treasury
/// untouched with it off.
#[test]
fn a_bank_that_covers_the_settler_buys_it() {
    let Some((probe, capital)) = young_empire(1, 0.0) else {
        panic!("fixture: a young empire");
    };
    let cost = price(&probe, capital);
    let Some((game, _)) = young_empire(1, cost + 5.0) else {
        panic!("fixture: a young empire");
    };

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    assert_eq!(stock.settler_fund(&mut off, 0, &plan), SettlerFund::Off, "off: no fund");
    assert_eq!(settlers(&off), 0);
    assert_eq!(off.players[0].gold, game.players[0].gold, "off: the bank is untouched");

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    assert!(plan.desired_cities > 1, "fixture: the plan wants more cities");
    assert_eq!(ai.settler_fund(&mut on, 0, &plan), SettlerFund::Bought);
    assert_eq!(settlers(&on), 1, "a Settler is bought");
    assert!(on.players[0].gold < game.players[0].gold, "the bank paid for it");
}

/// Short of the price but within the save horizon, the fund raises the
/// discretionary reserve to the Settler's price and buys nothing; with no
/// income and an empty bank it stays out of the way.
#[test]
fn a_bank_within_reach_saves_and_an_empty_one_does_not() {
    let Some((probe, capital)) = young_empire(1, 0.0) else {
        panic!("fixture: a young empire");
    };
    let cost = price(&probe, capital);
    let ai = armed();

    let Some((mut near, _)) = young_empire(1, cost - 20.0) else {
        panic!("fixture: a young empire");
    };
    near.players[0].gold_per_turn = 10.0;
    let plan = ai.assess(&near, 0);
    match ai.settler_fund(&mut near, 0, &plan) {
        SettlerFund::Saving { reserve } => {
            assert!(reserve + 1e-9 >= cost - 1.0, "the reserve is the Settler's price");
        }
        other => panic!("expected Saving, got {other:?}"),
    }
    assert_eq!(settlers(&near), 0, "nothing is bought while saving");

    let Some((mut empty, _)) = young_empire(1, 0.0) else {
        panic!("fixture: a young empire");
    };
    let plan = ai.assess(&empty, 0);
    assert_eq!(ai.settler_fund(&mut empty, 0, &plan), SettlerFund::Off);
}

/// The window shuts at war with a major, past the band turn, for a city at
/// the population floor, and with two Settlers already walking.
#[test]
fn war_the_band_turn_the_floor_and_walkers_shut_the_fund() {
    let ai = armed();
    let Some((peace, capital)) = young_empire(2, 2_000.0) else {
        panic!("fixture: two majors");
    };

    let mut war = peace.clone();
    war.at_war.insert((0, 1));
    assert!(war.is_at_war(0, 1), "fixture: at war");
    let plan = ai.assess(&war, 0);
    assert_eq!(ai.settler_fund(&mut war, 0, &plan), SettlerFund::Off, "at war");

    let mut late = peace.clone();
    late.turn = late.standard_duration(SETTLER_FUND_END_STANDARD);
    let plan = ai.assess(&late, 0);
    assert_eq!(ai.settler_fund(&mut late, 0, &plan), SettlerFund::Off, "past the band");

    let mut small = peace.clone();
    small.cities.get_mut(&capital).unwrap().pop = 1;
    let plan = ai.assess(&small, 0);
    assert_eq!(ai.settler_fund(&mut small, 0, &plan), SettlerFund::Off, "pop floor");

    let mut walking = peace.clone();
    let home = walking.cities[&capital].pos;
    walking.spawn_unit("settler", 0, home);
    walking.spawn_unit("settler", 0, home);
    let plan = ai.assess(&walking, 0);
    assert_eq!(ai.settler_fund(&mut walking, 0, &plan), SettlerFund::Off, "two walking");

    let mut open = peace.clone();
    let plan = ai.assess(&open, 0);
    assert_eq!(ai.settler_fund(&mut open, 0, &plan), SettlerFund::Bought, "at peace it buys");
}

/// The gene is registered and ships off.
#[test]
fn the_gene_is_registered_off_by_default() {
    assert!(!AdvancedAi::new().gold_buys_the_settler, "an opt-in ships off");
    let mut ai = AdvancedAi::new();
    ai.enable_gold_buys_the_settler();
    assert!(ai.gold_buys_the_settler);
    ai.disable_gold_buys_the_settler();
    assert!(!ai.gold_buys_the_settler);
    assert!(crate::ai::advanced::genes::GENES
        .iter()
        .any(|gene| gene.tag == "gold-buys-the-settler"));
}
