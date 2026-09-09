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
