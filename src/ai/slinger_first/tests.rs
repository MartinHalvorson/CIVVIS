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
    ai.opening_ranged_support = true;
    ai.scout_first_opening = true;
    ai.recon_replacement = true;
    (g, ai, capital)
}

fn unit(name: &str) -> Item {
    Item::Unit { unit: name.into() }
}

#[test]
fn first_slinger_replaces_the_scout_slot_then_releases_the_settler() {
    let (mut g, mut ai, capital) = opening();
    g.cities.get_mut(&capital).unwrap().pop = 2;
    ai.capital_settler_after_completion = true;
    ai.rapid_city_expansion_2 = true;
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue, vec![unit("slinger")]);
    assert_eq!(ai.book_pos, 1);
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue, vec![unit("slinger")]);
    assert_eq!(ai.book_pos, 1);
    g.cities.get_mut(&capital).unwrap().queue.clear();
    let home = g.cities[&capital].pos;
    g.spawn_test_unit("slinger", 0, home);
    assert!(ai.recon_replacement);
    assert!(
        ai.recon_is_the_missing_arm(&g, 0),
        "later recon behavior remains enabled"
    );
    ai.cities(&mut g, 0);
    assert_eq!(g.cities[&capital].queue, vec![unit("settler")]);
}

#[test]
fn off_preserves_the_existing_scout_overrides() {
    for scout_override in [false, true] {
        let (mut g, mut ai, capital) = opening();
        ai.opening_ranged_support = false;
        ai.scout_first_opening = scout_override;
        ai.cities(&mut g, 0);
        assert_eq!(g.cities[&capital].queue, vec![unit("scout")]);
        assert_eq!(ai.book_pos, 1);
    }
    assert!(!BasicAi::new().opening_ranged_support);
}

#[test]
fn existing_ranged_support_does_not_gain_another_slinger() {
    for name in ["slinger", "archer"] {
        let (mut g, mut ai, capital) = opening();
        let home = g.cities[&capital].pos;
        g.spawn_test_unit(name, 0, home);
        assert!(!ai.play_slinger_first_opening(&mut g, 0, capital));
        assert!(g.cities[&capital].queue.is_empty());
        assert_eq!(ai.book_pos, 0);
    }
}

#[test]
fn first_slinger_never_preempts_or_restarts_a_played_opening() {
    for case in 0..7 {
        let (mut g, mut ai, capital) = opening();
        match case {
            0 => g
                .cities
                .get_mut(&capital)
                .unwrap()
                .queue
                .push(unit("builder")),
            1 => ai.skip_opening_book(),
            2 => g.cities.get_mut(&capital).unwrap().is_capital = false,
            3 => ai.minor = true,
            4 => ai.barb = true,
            5 => ai.opening_ranged_support = false,
            6 => g.cities.get_mut(&capital).unwrap().owner = 1,
            _ => unreachable!(),
        }
        let before = g.cities[&capital].queue.clone();
        let book_pos = ai.book_pos;
        assert!(
            !ai.play_slinger_first_opening(&mut g, 0, capital),
            "case {case}"
        );
        assert_eq!(g.cities[&capital].queue, before, "case {case}");
        assert_eq!(ai.book_pos, book_pos, "case {case}");
    }
}

#[test]
fn support_requires_a_nearby_melee_defender() {
    for absent in [false, true] {
        let (mut g, mut ai, capital) = opening();
        let home = g.cities[&capital].pos;
        let uid = g.player_unit_ids(0)[0];
        if absent {
            g.remove_unit(uid);
        } else {
            let far = *g
                .map
                .tiles
                .keys()
                .find(|pos| g.wdist(home, **pos) > 5)
                .unwrap();
            g.units.get_mut(&uid).unwrap().pos = far;
        }
        assert!(!ai.play_slinger_first_opening(&mut g, 0, capital));
        assert!(g.cities[&capital].queue.is_empty());
    }
}

#[test]
fn an_established_multi_city_empire_does_not_take_an_opening_slot() {
    let (mut g, mut ai, capital) = opening();
    let mut other = g.cities[&capital].clone();
    other.id += 100;
    other.is_capital = false;
    g.cities.insert(other.id, other);
    assert!(!ai.play_slinger_first_opening(&mut g, 0, capital));
    assert_eq!(ai.book_pos, 0);
}

#[test]
fn opening_ranged_support_is_an_opt_in_off_in_both_controllers() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers(
        "opening-ranged-support",
        |ai| ai.opening_ranged_support(),
    );
}
