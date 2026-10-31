use super::*;

/// Two majors: our founded capital at population 2 with an idle queue, a
/// standing Holy Site beside it, Astrology known, no religion founded, no
/// Prophet points, and eight turns left to the Prayer deadline.
fn praying_capital() -> (Game, u32) {
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
    let home = game.cities[&capital].pos;
    let site = game
        .wdisk(home, 1)
        .into_iter()
        .filter(|pos| *pos != home)
        .find(|pos| {
            game.map
                .tiles
                .get(pos)
                .is_some_and(|tile| !game.rules.is_water(tile))
        })
        .expect("fixture: a land tile beside the capital");
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 2;
    city.queue.clear();
    city.districts.insert(crate::name!("holy_site"), site);
    game.turn = game.standard_duration(PRAYER_DEADLINE_STANDARD) - 8;
    (game, capital)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_religion_seat_prays_for_the_prophet();
    }
    ai
}

fn is_prayers(item: Option<&Item>) -> bool {
    matches!(item, Some(Item::Project { project }) if *project == "holy_site_prayers")
}

/// A Religion seat's idle Holy Site capital prays; off, or for a Culture
/// seat, it does not.
#[test]
fn a_religion_seat_short_of_its_prophet_prays() {
    let (game, capital) = praying_capital();

    let mut on = game.clone();
    let ai = seat(VictoryTarget::Religion, true);
    let plan = ai.assess(&on, 0);
    ai.claim_religion_seat_prayers(&mut on, 0, &plan);
    assert!(
        is_prayers(on.cities[&capital].queue.first()),
        "the capital's head: {:?}",
        on.cities[&capital].queue.first()
    );

    for ai in [
        seat(VictoryTarget::Religion, false),
        seat(VictoryTarget::Culture, true),
    ] {
        let mut off = game.clone();
        let plan = ai.assess(&off, 0);
        ai.claim_religion_seat_prayers(&mut off, 0, &plan);
        assert!(off.cities[&capital].queue.is_empty());
    }
}

/// Points that cover the price, a founded religion, a Prophet in hand or a
/// busy queue ask for no Prayers.
#[test]
fn it_stops_once_the_prophet_is_paid_for() {
    let (game, capital) = praying_capital();
    let ai = seat(VictoryTarget::Religion, true);

    let mut paid = game.clone();
    let cost = paid.gp_cost(0, "prophet");
    paid.players[0].gpp.insert("prophet".to_string(), cost);
    let plan = ai.assess(&paid, 0);
    assert!(ai.religion_seat_prayer_city(&paid, 0, &plan).is_none());

    let mut founded = game.clone();
    founded.players[0].religion = Some("buddhism".to_string());
    let plan = ai.assess(&founded, 0);
    assert!(ai.religion_seat_prayer_city(&founded, 0, &plan).is_none());

    let mut pending = game.clone();
    pending.players[0].prophet_pending = true;
    let plan = ai.assess(&pending, 0);
    assert!(ai.religion_seat_prayer_city(&pending, 0, &plan).is_none());

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
    ai.claim_religion_seat_prayers(&mut busy, 0, &plan);
    assert_eq!(busy.cities[&capital].queue.first(), Some(&warrior));
}

/// A seat whose points and rate reach the price by the deadline does not pray.
#[test]
fn a_seat_on_pace_for_its_prophet_does_not_pray() {
    let (mut game, capital) = praying_capital();
    let ai = seat(VictoryTarget::Religion, true);
    let cost = game.gp_cost(0, "prophet");
    let rate = game
        .great_person_points_per_turn(0)
        .get("prophet")
        .copied()
        .unwrap_or(0.0);
    game.players[0]
        .gpp
        .insert("prophet".to_string(), (cost - 8.0 * rate).max(0.0) + 0.5);
    let plan = ai.assess(&game, 0);
    if rate > 0.0 {
        assert!(ai.religion_seat_prayer_city(&game, 0, &plan).is_none());
    }
    game.players[0].gpp.insert("prophet".to_string(), 0.0);
    let plan = ai.assess(&game, 0);
    assert_eq!(ai.religion_seat_prayer_city(&game, 0, &plan), Some(capital));
}
