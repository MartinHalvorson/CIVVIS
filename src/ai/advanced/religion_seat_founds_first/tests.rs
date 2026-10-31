use super::*;

/// Two majors: our founded capital at population 2 with an idle queue,
/// Astrology and the feature-clearing techs known, no religion founded.
fn lone_capital() -> (Game, u32) {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
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
    city.queue.clear();
    game.turn = 12;
    (game, capital)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_religion_seat_founds_first();
    }
    ai
}

fn is_holy_site(game: &Game, item: Option<&Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "holy_site")
}

/// A one-city Religion seat starts its Holy Site in the idle capital; off, or
/// for a Culture seat, it does not.
#[test]
fn a_lone_religion_capital_starts_its_holy_site() {
    let (game, capital) = lone_capital();

    let mut on = game.clone();
    let ai = seat(VictoryTarget::Religion, true);
    let plan = ai.assess(&on, 0);
    ai.claim_religion_seat_holy_site(&mut on, 0, &plan);
    assert!(
        is_holy_site(&on, on.cities[&capital].queue.first()),
        "the capital's head: {:?}",
        on.cities[&capital].queue.first()
    );

    for ai in [
        seat(VictoryTarget::Religion, false),
        seat(VictoryTarget::Culture, true),
    ] {
        let mut off = game.clone();
        let plan = ai.assess(&off, 0);
        ai.claim_religion_seat_holy_site(&mut off, 0, &plan);
        assert!(off.cities[&capital].queue.is_empty());
    }
}

/// A busy queue keeps its item, and a founded seat, a Prophet in hand or a
/// site already queued asks for nothing.
#[test]
fn it_never_takes_a_busy_queue_or_a_settled_founding() {
    let (game, capital) = lone_capital();
    let ai = seat(VictoryTarget::Religion, true);

    let mut busy = game.clone();
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    busy.cities
        .get_mut(&capital)
        .unwrap()
        .queue
        .push(warrior.clone());
    let plan = ai.assess(&busy, 0);
    ai.claim_religion_seat_holy_site(&mut busy, 0, &plan);
    assert_eq!(busy.cities[&capital].queue.first(), Some(&warrior));

    let mut founded = game.clone();
    founded.players[0].religion = Some("buddhism".to_string());
    let plan = ai.assess(&founded, 0);
    assert!(ai.religion_seat_holy_site(&founded, 0, &plan).is_none());

    let mut pending = game.clone();
    pending.players[0].prophet_pending = true;
    let plan = ai.assess(&pending, 0);
    assert!(ai.religion_seat_holy_site(&pending, 0, &plan).is_none());

    let mut queued = game.clone();
    let plan = ai.assess(&queued, 0);
    ai.claim_religion_seat_holy_site(&mut queued, 0, &plan);
    let plan = ai.assess(&queued, 0);
    assert!(
        ai.religion_seat_holy_site(&queued, 0, &plan).is_none(),
        "a queued site asks for no second one"
    );
}
