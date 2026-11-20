use super::*;

/// Two majors: our founded capital at population 2, Astrology known, no
/// religion founded, no Settler on the board, the queue as given.
fn lone_capital(head: Option<Item>) -> (Game, u32) {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for uid in game.player_unit_ids(0) {
        if game.units[&uid].kind == "settler" {
            game.units.remove(&uid);
        }
    }
    for tech in [
        "pottery",
        "astrology",
        "mining",
        "bronze_working",
        "irrigation",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    let capital = game.player_city_ids(0)[0];
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 2;
    city.queue = head.into_iter().collect();
    game.turn = 12;
    (game, capital)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_religion_seat_founds_first();
    if gene {
        ai.enable_religion_seat_second_city_first();
    }
    ai
}

fn settler() -> Item {
    Item::Unit {
        unit: crate::name!("settler"),
    }
}

fn prayers() -> Item {
    Item::Project {
        project: crate::name!("holy_site_prayers"),
    }
}

fn claim(ai: &AdvancedAi, game: &mut Game) {
    let mut plan = ai.assess(game, 0);
    plan.threatened_city = None;
    ai.claim_religion_seat_second_city(game, 0, &plan);
}

fn holy_site(game: &Game, capital: u32) -> Item {
    game.producible_items(0, capital)
        .into_iter()
        .find(|item| {
            matches!(item, Item::District { district, .. }
            if game.district_family(*district) == "holy_site")
        })
        .expect("fixture: the capital can place a Holy Site")
}

/// Prayers, a fresh Holy Site, a Builder and a quiet Warrior all give way to
/// the first Settler; off, or for a Culture seat, nothing changes.
#[test]
fn the_first_settler_takes_the_capital_queue() {
    let (base, capital) = lone_capital(None);
    let heads = [
        None,
        Some(prayers()),
        Some(holy_site(&base, capital)),
        Some(Item::Unit {
            unit: crate::name!("builder"),
        }),
        Some(Item::Unit {
            unit: crate::name!("warrior"),
        }),
    ];
    for head in heads {
        let (mut on, _) = lone_capital(head.clone());
        claim(&seat(VictoryTarget::Religion, true), &mut on);
        assert_eq!(
            on.cities[&capital].queue.first(),
            Some(&settler()),
            "head {head:?}"
        );
        for ai in [
            seat(VictoryTarget::Religion, false),
            seat(VictoryTarget::Culture, true),
        ] {
            let (mut off, _) = lone_capital(head.clone());
            claim(&ai, &mut off);
            assert_eq!(off.cities[&capital].queue.first(), head.as_ref());
        }
    }
}

/// A Holy Site with paid work, an item finishing this turn and Walls keep
/// the head; so does everything once a Settler or a second city exists.
#[test]
fn under_way_work_and_a_settled_seat_keep_the_queue() {
    let ai = seat(VictoryTarget::Religion, true);
    let (base, capital) = lone_capital(None);
    let site = holy_site(&base, capital);

    let (mut paid, _) = lone_capital(Some(site.clone()));
    paid.cities
        .get_mut(&capital)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&site), 6.0);
    claim(&ai, &mut paid);
    assert_eq!(paid.cities[&capital].queue.first(), Some(&site));

    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let (mut nearly, _) = lone_capital(Some(warrior.clone()));
    let cost = nearly.item_cost_for_city(0, capital, &warrior);
    nearly
        .cities
        .get_mut(&capital)
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&warrior), cost - 0.5);
    claim(&ai, &mut nearly);
    assert_eq!(nearly.cities[&capital].queue.first(), Some(&warrior));

    let (mut walking, _) = lone_capital(Some(prayers()));
    let home = walking.cities[&capital].pos;
    walking.spawn_unit("settler", 0, home);
    claim(&ai, &mut walking);
    assert_eq!(walking.cities[&capital].queue.first(), Some(&prayers()));

    let (mut small, _) = lone_capital(Some(prayers()));
    small.cities.get_mut(&capital).unwrap().pop = 1;
    claim(&ai, &mut small);
    assert_eq!(small.cities[&capital].queue.first(), Some(&prayers()));
}

/// With one religion slot left Prayers go first; Arabia never takes the
/// exception.
#[test]
fn the_last_slot_lets_the_founding_go_first_except_for_arabia() {
    let ai = seat(VictoryTarget::Religion, true);
    let (mut close, capital) = lone_capital(Some(prayers()));
    assert!(!ai.second_city_slot_guard(&close, 0), "every slot open");
    // Every slot but one is taken; the guard reads only the count.
    let rivals = close.max_religions() - 1;
    for (n, player) in close.players.iter_mut().skip(1).take(rivals).enumerate() {
        player.religion = Some(format!("faith_{n}"));
    }
    assert_eq!(close.religions_founded(), rivals, "fixture: one slot left");
    assert!(ai.second_city_slot_guard(&close, 0));
    let mut arabia = close.clone();
    claim(&ai, &mut close);
    assert_eq!(close.cities[&capital].queue.first(), Some(&prayers()));

    arabia.players[0].civ = "Arabia".to_string();
    assert!(!ai.second_city_slot_guard(&arabia, 0));
    claim(&ai, &mut arabia);
    assert_eq!(arabia.cities[&capital].queue.first(), Some(&settler()));
}

/// `religion-seat-founds-first` leaves the idle capital to its Settler while
/// the gene holds it, places the site with the gene off, and places it with
/// the gene on once one religion slot is left.
#[test]
fn founds_first_waits_for_the_settler() {
    let (mut game, _) = lone_capital(None);
    let mut plan = seat(VictoryTarget::Religion, true).assess(&game, 0);
    plan.threatened_city = None;
    assert!(seat(VictoryTarget::Religion, true)
        .religion_seat_holy_site(&game, 0, &plan)
        .is_none());
    assert!(seat(VictoryTarget::Religion, false)
        .religion_seat_holy_site(&game, 0, &plan)
        .is_some());
    let rivals = game.max_religions() - 1;
    for (n, player) in game.players.iter_mut().skip(1).take(rivals).enumerate() {
        player.religion = Some(format!("faith_{n}"));
    }
    assert!(seat(VictoryTarget::Religion, true)
        .religion_seat_holy_site(&game, 0, &plan)
        .is_some());
}
