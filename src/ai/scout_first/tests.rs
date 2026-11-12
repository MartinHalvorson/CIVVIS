use super::*;

fn opening() -> (Game, BasicAi, u32) {
    let mut g = Game::new_full(2, 24, 16, 91_780, 250, 0, false);
    g.current = 0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = g.player_city_ids(0)[0];
    let home = g.cities[&capital].pos;
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.spawn_test_unit("warrior", 0, home);
    g.players[0].explored.clear();
    g.players[0].explored.insert(home);
    let mut ai = BasicAi::new();
    ai.scout_first_opening = true;
    ai.recon_replacement = false;
    ai.w.open0 = 1.0; // Warrior: the current live game's opener.
    (g, ai, capital)
}

fn scout() -> Item {
    Item::Unit {
        unit: crate::name!("scout"),
    }
}

#[test]
fn scout_first_opening_replaces_one_slot_and_then_releases_the_settler() {
    let (g, mut ai, capital) = opening();
    let mut control = g.clone();
    let mut baseline = BasicAi::new();
    baseline.w.open0 = 1.0;
    baseline.cities(&mut control, 0);
    assert_eq!(
        control.cities[&capital].queue[0],
        Item::Unit {
            unit: crate::name!("warrior")
        }
    );

    let mut g = g;
    g.cities.get_mut(&capital).unwrap().pop = 2;
    ai.capital_settler_after_completion = true;
    ai.rapid_city_expansion_2 = true;
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue[0], scout());
    assert_eq!(ai.book_pos, 1);
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue, vec![scout()]);
    assert_eq!(ai.book_pos, 1);

    // Model completion, then exercise the same production dispatcher.
    g.cities.get_mut(&capital).unwrap().queue.clear();
    let home = g.cities[&capital].pos;
    g.spawn_test_unit("scout", 0, home);
    assert!(ai.has_practical_settle_site(&g, 0));
    ai.cities(&mut g, 0);
    assert_eq!(
        g.cities[&capital].queue[0],
        Item::Unit {
            unit: crate::name!("settler")
        }
    );
}

#[test]
fn scout_first_opening_yields_to_visible_pressure_but_not_hidden_units() {
    let (mut g, mut ai, capital) = opening();
    let home = g.cities[&capital].pos;
    let enemy = g
        .nbrs(home)
        .into_iter()
        .find(|p| g.map.tiles.contains_key(p))
        .unwrap();
    g.spawn_test_unit("warrior", 1, enemy);
    g.at_war.insert((0, 1));
    g.players[0].turn_visible.insert(enemy);
    assert!(!ai.play_scout_first_opening(&mut g, 0, capital));
    assert_eq!(ai.book_pos, 0);
    let mut threatened = g.clone();
    ai.cities(&mut threatened, 0);
    assert_ne!(threatened.cities[&capital].queue[0], scout());
    ai.book_pos = 0;
    g.players[0].turn_visible.remove(&enemy);
    assert!(ai.play_scout_first_opening(&mut g, 0, capital));
}

#[test]
fn scout_first_opening_is_bounded_and_does_not_duplicate_or_preempt() {
    for case in 0..7 {
        let (mut g, mut ai, capital) = opening();
        let home = g.cities[&capital].pos;
        match case {
            0 => {
                g.spawn_test_unit("scout", 0, home);
            }
            1 => {
                g.cities.get_mut(&capital).unwrap().queue.push(scout());
            }
            2 => {
                ai.skip_opening_book();
            }
            3 => {
                g.players[0].explored.extend(g.map.tiles.keys().copied());
            }
            4 => {
                for uid in g.player_unit_ids(0) {
                    g.remove_unit(uid);
                }
            }
            5 => {
                ai.scout_first_opening = false;
            }
            6 => {
                g.cities.get_mut(&capital).unwrap().is_capital = false;
            }
            _ => unreachable!(),
        }
        let before = g.cities[&capital].queue.clone();
        assert!(
            !ai.play_scout_first_opening(&mut g, 0, capital),
            "case {case}"
        );
        assert_eq!(g.cities[&capital].queue, before, "case {case}");
    }
}

// ------------------------------------------------ opening-scout-comes-back

fn settler() -> Item {
    Item::Unit {
        unit: crate::name!("settler"),
    }
}

/// The opening past its scripted Scout, with `opening-scout-comes-back` on.
fn comes_back() -> (Game, BasicAi, u32) {
    let (g, mut ai, capital) = opening();
    ai.scout_first_opening = false;
    ai.book_pos = 2;
    ai.opening_scout_comes_back = true;
    (g, ai, capital)
}

/// Decision 1, recorded: live 230722Z t4 "slinger takes the queue before
/// scout". Only a displaced recon unit is owed, on the city's tile.
#[test]
fn a_scout_the_barbarian_defence_displaces_is_owed_on_the_city_tile() {
    let (g, mut ai, capital) = opening();
    let home = g.cities[&capital].pos;
    let slinger = Item::Unit {
        unit: crate::name!("slinger"),
    };
    ai.note_opening_scout_displaced(&g, capital, Some(&scout()));
    assert_eq!(ai.opening_scout_owed_at, None, "off: nothing is owed");

    ai.opening_scout_comes_back = true;
    ai.note_opening_scout_displaced(&g, capital, Some(&slinger));
    ai.note_opening_scout_displaced(&g, capital, Some(&settler()));
    ai.note_opening_scout_displaced(&g, capital, None);
    assert_eq!(ai.opening_scout_owed_at, None, "only a Scout is owed");

    ai.note_opening_scout_displaced(&g, capital, Some(&scout()));
    assert_eq!(ai.opening_scout_owed_at, Some(home));
}

/// Decision 1, paid: the owed Scout waits for the defender, then is the
/// next build; a recon unit that came another way clears the debt.
#[test]
fn the_owed_scout_is_the_next_build_once_the_defender_leaves_the_queue() {
    let (mut g, mut ai, capital) = comes_back();
    let home = g.cities[&capital].pos;
    let slinger = Item::Unit {
        unit: crate::name!("slinger"),
    };
    ai.opening_scout_owed_at = Some(home);
    g.cities.get_mut(&capital).unwrap().queue = vec![slinger];
    ai.cities(&mut g, 0);
    assert_ne!(g.cities[&capital].queue.first(), Some(&scout()));
    assert_eq!(ai.opening_scout_owed_at, Some(home), "still owed");

    g.cities.get_mut(&capital).unwrap().queue.clear();
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue[0], scout());
    assert_eq!(ai.opening_scout_owed_at, None, "paid");

    // Owed again, but a Scout already stands: the debt is dropped.
    let (mut g, mut ai, capital) = comes_back();
    let home = g.cities[&capital].pos;
    ai.opening_scout_owed_at = Some(home);
    g.spawn_test_unit("scout", 0, home);
    g.cities.get_mut(&capital).unwrap().queue.clear();
    ai.cities(&mut g, 0);
    assert_eq!(ai.opening_scout_owed_at, None);
}

/// Decision 2: live 223549Z lost its Scout at about turn 7 and had none
/// until turn 29. A Scout seen and then gone is replaced at once, over the
/// Settler, and only once a game.
#[test]
fn a_scout_lost_early_is_replaced_at_once_and_only_once() {
    let (mut g, mut ai, capital) = comes_back();
    let home = g.cities[&capital].pos;
    g.cities.get_mut(&capital).unwrap().queue = vec![settler()];
    let first = g.spawn_test_unit("scout", 0, home);
    ai.cities(&mut g, 0);
    assert!(ai.opening_recon_fielded, "the Scout was seen");
    assert_ne!(g.cities[&capital].queue.first(), Some(&scout()));

    g.remove_unit(first);
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue[0], scout(), "replaced at once");
    assert!(ai.opening_scout_replaced);

    g.cities.get_mut(&capital).unwrap().queue = vec![settler()];
    ai.cities(&mut g, 0);
    assert_ne!(
        g.cities[&capital].queue.first(),
        Some(&scout()),
        "one replacement a game"
    );
}

/// Decision 2's gates: no replacement once a rival's original capital is
/// charted, nor from turn `OPENING_SCOUT_LOSS_TURNS` (standard) on.
#[test]
fn no_replacement_once_a_rival_capital_is_charted_or_past_the_window() {
    let (mut g, mut ai, capital) = comes_back();
    let home = g.cities[&capital].pos;
    ai.opening_recon_fielded = true;
    let site = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.wdist(home, *pos) == 8)
        .min()
        .unwrap();
    let rival_capital = g.found_city_for(1, site, None);
    let theirs = g.cities[&rival_capital].pos;
    assert!(g.cities[&rival_capital].is_capital);

    g.players[0].explored.insert(theirs);
    g.cities.get_mut(&capital).unwrap().queue = vec![settler()];
    ai.cities(&mut g, 0);
    assert_ne!(
        g.cities[&capital].queue.first(),
        Some(&scout()),
        "a rival capital is charted"
    );

    g.players[0].explored.remove(&theirs);
    g.turn = g.standard_duration(OPENING_SCOUT_LOSS_TURNS);
    g.cities.get_mut(&capital).unwrap().queue = vec![settler()];
    ai.cities(&mut g, 0);
    assert_ne!(
        g.cities[&capital].queue.first(),
        Some(&scout()),
        "past the window"
    );

    g.turn -= 1;
    g.cities.get_mut(&capital).unwrap().queue = vec![settler()];
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue[0], scout());
}
