use super::*;

/// Seat p1 (holy, Ottoman) of the owner's game `hotseat-20261128T045523Z`,
/// turn 32, frame 0, trimmed to the fields the valuation reads: three cities,
/// two of them Displeased, no luxury, no strategic stockpile, 47 Gold and 2 a
/// turn. The person (Sumeria, player 0) offers Silver and Sugar.
const OTTOMAN_T32: &str = r#"{"turn":32,"frame":0,"gold":47,"gold_per_turn":2,"favor":0,"strategic_resources":{},"strategic_resource_income":{},"tourism_per_turn":0,"military":53,"world_era":1,"score":60,"cities":[{"id":65536,"name":"Istanbul","x":18,"y":18,"pop":4,"capital":true,"housing":6,"amenities":2,"amenities_needed":2,"amenities_luxuries":0,"amenities_war_weariness":0,"happiness":4,"happiness_yield_mult":0,"happiness_growth_mult":0,"growth_turns":3,"food_surplus":7,"yields":{"culture":2.19922,"faith":0,"food":15,"gold":5,"production":9,"science":4},"great_works":[],"loyalty":100,"loyalty_per_turn":28},{"id":131073,"name":"Trabzon","x":17,"y":22,"pop":3,"capital":false,"housing":5,"amenities":0,"amenities_needed":2,"amenities_luxuries":0,"amenities_war_weariness":0,"happiness":3,"happiness_yield_mult":-10,"happiness_growth_mult":-15,"growth_turns":3,"food_surplus":2,"yields":{"culture":0.808594,"faith":3.60156,"food":8,"gold":0,"production":7.20312,"science":1.35156},"great_works":[],"loyalty":100,"loyalty_per_turn":17},{"id":196610,"name":"Ankara","x":17,"y":14,"pop":1,"capital":false,"housing":5,"amenities":0,"amenities_needed":1,"amenities_luxuries":0,"amenities_war_weariness":0,"happiness":3,"happiness_yield_mult":-10,"happiness_growth_mult":-15,"growth_turns":2,"food_surplus":1,"yields":{"culture":0.269531,"faith":0,"food":3,"gold":0,"production":4.5,"science":0.453125},"great_works":[],"loyalty":100,"loyalty_per_turn":10.2812}],"rivals":[{"player":0,"civ":"CIVILIZATION_SUMERIA","score":102,"military":123,"at_war":false,"diplomatic_state":"DIPLO_STATE_FRIENDLY","gold":26,"gold_per_turn":9,"tradeable_luxuries":["RESOURCE_SILVER","RESOURCE_SUGAR"],"tradeable_strategics":[],"our_denounce_turn":-1,"their_denounce_turn":-1},{"player":2,"civ":"CIVILIZATION_PHOENICIA","score":50,"military":99,"at_war":false,"diplomatic_state":"DIPLO_STATE_FRIENDLY","gold":10.2227,"gold_per_turn":7.50391,"tradeable_luxuries":[],"tradeable_strategics":[],"our_denounce_turn":-1,"their_denounce_turn":-1},{"player":3,"civ":"CIVILIZATION_CANADA","score":44,"military":38,"at_war":false,"diplomatic_state":"DIPLO_STATE_NEUTRAL","gold":65,"gold_per_turn":8,"tradeable_luxuries":["RESOURCE_FURS"],"tradeable_strategics":{"RESOURCE_HORSES":10},"our_denounce_turn":-1,"their_denounce_turn":-1}]}"#;

/// Seat p3 (moonshot, Canada) of the same game and turn: two Content cities
/// served by its only luxury (one Furs), 12 Horses rising 2 a turn, 73 Gold
/// and 8 a turn.
const CANADA_T32: &str = r#"{"turn":32,"frame":0,"gold":73,"gold_per_turn":8,"favor":0,"luxury_counts":{"RESOURCE_FURS":1},"strategic_resources":{"RESOURCE_HORSES":12},"strategic_resource_income":{"RESOURCE_HORSES":2},"tourism_per_turn":0,"military":32,"world_era":1,"score":44,"cities":[{"id":65536,"name":"Ottawa","x":18,"y":8,"pop":4,"capital":true,"housing":7,"amenities":3,"amenities_needed":2,"amenities_luxuries":1,"amenities_war_weariness":0,"happiness":4,"happiness_yield_mult":0,"happiness_growth_mult":0,"growth_turns":2,"food_surplus":5,"yields":{"culture":2.19922,"faith":0,"food":13,"gold":8,"production":10,"science":4},"great_works":[],"loyalty":100,"loyalty_per_turn":28},{"id":131073,"name":"Halifax","x":13,"y":7,"pop":2,"capital":false,"housing":5,"amenities":1,"amenities_needed":1,"amenities_luxuries":1,"amenities_war_weariness":0,"happiness":4,"happiness_yield_mult":0,"happiness_growth_mult":0,"growth_turns":1,"food_surplus":3,"yields":{"culture":0.597656,"faith":0,"food":7,"gold":0,"production":3,"science":1},"great_works":[],"loyalty":100,"loyalty_per_turn":20}],"rivals":[{"player":0,"civ":"CIVILIZATION_SUMERIA","score":102,"military":123,"at_war":false,"diplomatic_state":"DIPLO_STATE_UNFRIENDLY","gold":26,"gold_per_turn":9,"tradeable_luxuries":["RESOURCE_SILVER","RESOURCE_SUGAR"],"tradeable_strategics":[],"our_denounce_turn":-1,"their_denounce_turn":-1},{"player":1,"civ":"CIVILIZATION_OTTOMAN","score":60,"military":53,"at_war":false,"diplomatic_state":"DIPLO_STATE_FRIENDLY","gold":2.5,"gold_per_turn":2,"tradeable_luxuries":[],"tradeable_strategics":[],"our_denounce_turn":-1,"their_denounce_turn":-1}]}"#;

/// A real board as the decider reads it, at the game's speed and length
/// (`config.json`: `GAMESPEED_ONLINE`, 250 turns).
fn board(json: &str) -> StateSnapshot {
    let mut state: StateSnapshot = serde_json::from_str(json).expect("a real state record parses");
    state.seat.speed = "GAMESPEED_ONLINE".to_string();
    state.seat.max_turns = 250;
    state
}

fn ottoman() -> Position {
    Position::from_board(&board(OTTOMAN_T32), Some(VictoryTarget::Religion))
}

fn canada() -> Position {
    Position::from_board(&board(CANADA_T32), Some(VictoryTarget::Science))
}

fn rival(state: &StateSnapshot, player: usize) -> StateRival {
    state
        .rivals
        .iter()
        .find(|rival| rival.player == player)
        .cloned()
        .expect("the rival is on the board")
}

fn getting(items: Vec<Item>) -> Deal {
    Deal {
        give: Vec::new(),
        get: items,
    }
}

fn giving(items: Vec<Item>) -> Deal {
    Deal {
        give: items,
        get: Vec::new(),
    }
}

fn silver() -> Item {
    Item::Luxury("RESOURCE_SILVER".to_string())
}

fn sugar() -> Item {
    Item::Luxury("RESOURCE_SUGAR".to_string())
}

fn furs() -> Item {
    Item::Luxury("RESOURCE_FURS".to_string())
}

#[test]
fn the_happiness_table_is_the_one_the_host_applies_on_real_boards() {
    let mut seen = 0;
    for json in [OTTOMAN_T32, CANADA_T32] {
        for city in &board(json).cities {
            let mood = mood((city.amenities - city.amenities_needed).round() as i32);
            assert_eq!(mood.yield_pct, city.happiness_yield_mult, "{}", city.name);
            assert_eq!(mood.growth_pct, city.happiness_growth_mult, "{}", city.name);
            seen += 1;
        }
    }
    assert_eq!(seen, 5);
    // The rest of the Gathering Storm table, level by level.
    let table = [
        (-7, -100.0, -40.0),
        (-5, -100.0, -30.0),
        (-3, -30.0, -20.0),
        (-1, -15.0, -10.0),
        (2, 0.0, 0.0),
        (3, 10.0, 10.0),
        (5, 20.0, 20.0),
    ];
    for (score, growth, yields) in table {
        assert_eq!(
            (mood(score).growth_pct, mood(score).yield_pct),
            (growth, yields),
            "score {score}"
        );
    }
    assert_eq!(
        [1, 2, 3, 4].map(amenities_needed),
        [1, 1, 2, 2],
        "the live boards' needs"
    );
}

#[test]
fn a_real_board_reads_into_a_position() {
    let ottoman = ottoman();
    assert_eq!(ottoman.cities.len(), 3);
    assert_eq!(ottoman.gold, 47.0);
    assert_eq!(ottoman.gold_per_turn, 2.0);
    assert!(ottoman.luxuries.is_empty());
    assert!(
        ottoman.luxuries_known,
        "no luxury count is exported and no city has a luxury Amenity: none is held"
    );
    assert_eq!(ottoman.turns_left, 250 - 32);
    // The Displeased city's yields are read back to their unmodified base.
    let trabzon = &ottoman.cities[1];
    assert_eq!(trabzon.score, -2);
    let shown = 7.20312 * ottoman.weights.production
        + 1.35156 * ottoman.weights.science
        + 0.808594 * ottoman.weights.culture
        + 3.60156 * ottoman.weights.faith;
    assert!((trabzon.base_nonfood_gold - shown / 0.9).abs() < 1e-9);
    let canada = canada();
    assert_eq!(canada.luxuries.get("RESOURCE_FURS"), Some(&1));
    assert_eq!(canada.cities[0].luxury_amenities, Some(1));
}

#[test]
fn per_turn_gold_is_discounted_by_the_game_speed() {
    let online = ottoman();
    assert!((online.annuity(DEAL_TURNS) - 19.600).abs() < 0.01);
    let mut state = board(OTTOMAN_T32);
    state.seat.speed = "GAMESPEED_STANDARD".to_string();
    let standard = Position::from_board(&state, None);
    assert!((standard.annuity(DEAL_TURNS) - 24.016).abs() < 0.01);
    // A Gold a turn is worth less than 30 Gold now, and more than one.
    let per_turn = online.value_of_getting(&Item::GoldPerTurn(1), None);
    assert!(per_turn > 1.0 && per_turn < 30.0, "{per_turn}");
    assert_eq!(online.value_of_getting(&Item::Gold(25), None), 25.0);
}

#[test]
fn a_seat_cannot_promise_what_it_does_not_have() {
    let ottoman = ottoman();
    assert_eq!(ottoman.cost_of_giving(&Item::Gold(47), None), 47.0);
    assert_eq!(ottoman.cost_of_giving(&Item::Gold(48), None), f64::INFINITY);
    assert!(ottoman
        .cost_of_giving(&Item::GoldPerTurn(2), None)
        .is_finite());
    assert_eq!(
        ottoman.cost_of_giving(&Item::GoldPerTurn(3), None),
        f64::INFINITY,
        "more Gold a turn than the income"
    );
    assert_eq!(
        ottoman.cost_of_giving(&Item::Gold(30), None)
            + ottoman.cost_of_giving(&Item::Gold(30), None),
        60.0
    );
    assert_eq!(
        ottoman.net(&giving(vec![Item::Gold(30), Item::Gold(30)]), None),
        f64::NEG_INFINITY,
        "two lumps that together exceed the treasury"
    );
    assert_eq!(ottoman.cost_of_giving(&silver(), None), f64::INFINITY);
    assert_eq!(ottoman.cost_of_giving(&Item::Favor(1), None), f64::INFINITY);
    assert_eq!(
        canada().cost_of_giving(&Item::Strategic("RESOURCE_HORSES".into(), 13), None),
        f64::INFINITY
    );
    assert_eq!(
        ottoman.net(&getting(vec![Item::Gold(-5)]), None),
        f64::NEG_INFINITY,
        "a negative amount is no deal"
    );
}

#[test]
fn a_first_luxury_is_worth_its_amenities_and_a_duplicate_nothing() {
    // The Ottomans run two Displeased cities and hold no luxury: Silver from
    // the person lifts Ankara to Content now and keeps Istanbul Content as it
    // grows.
    let first = ottoman().value_of_getting(&silver(), None);
    assert!(first > 100.0 && first < 200.0, "{first}");
    // Canada already holds Furs: another copy changes nothing.
    assert_eq!(canada().value_of_getting(&furs(), None), 0.0);
}

#[test]
fn the_only_copy_costs_its_amenities_and_a_spare_almost_nothing() {
    let canada = canada();
    let only = canada.cost_of_giving(&furs(), None);
    // Halifax falls Displeased then Unhappy as it grows, Ottawa four turns
    // sooner.
    assert!(only > 30.0, "{only}");
    let mut state = board(CANADA_T32);
    state.luxury_counts = Some(BTreeMap::from([("RESOURCE_FURS".to_string(), 2.0)]));
    let spare =
        Position::from_board(&state, Some(VictoryTarget::Science)).cost_of_giving(&furs(), None);
    assert_eq!(spare, LUXURY_SPARE_GOLD);
    assert!(only > 5.0 * spare);
}

#[test]
fn unhappy_cities_pay_more_for_a_luxury_than_content_ones() {
    let unhappy = ottoman();
    let mut content = ottoman();
    for city in &mut content.cities {
        // Ecstatic with room for every citizen the deal's thirty turns can
        // grow: one Amenity more changes nothing.
        city.score = 10;
    }
    let paid = unhappy.value_of_getting(&silver(), None);
    let spare = content.value_of_getting(&silver(), None);
    assert!(paid > spare + 50.0, "{paid} vs {spare}");
}

#[test]
fn two_luxuries_into_the_same_cities_are_worth_less_than_twice_one() {
    let ottoman = ottoman();
    let one = ottoman.value_of_getting(&silver(), None);
    let two = ottoman.net(&getting(vec![silver(), sugar()]), None);
    assert!(two > one, "the second type still helps: {two} vs {one}");
    assert!(
        two < 2.0 * one,
        "but it lands on cities the first lifted: {two} vs {one}"
    );
    assert_eq!(
        ottoman.net(&getting(vec![silver(), silver()]), None),
        one,
        "two copies of one type are one type"
    );
}

#[test]
fn a_luxury_count_the_board_cannot_read_trades_nothing() {
    let mut state = board(CANADA_T32);
    state.luxury_counts = None;
    let blind = Position::from_board(&state, Some(VictoryTarget::Science));
    assert!(
        !blind.luxuries_known,
        "Halifax has a luxury Amenity of unknown type"
    );
    assert_eq!(blind.value_of_getting(&silver(), None), 0.0);
    assert_eq!(blind.cost_of_giving(&furs(), None), f64::INFINITY);
}

#[test]
fn strategics_fill_the_need_first_and_nothing_past_the_cap() {
    // The Ottomans hold no Horses and earn none; their era's army needs a few.
    let ottoman = ottoman();
    let horses = |amount| Item::Strategic("RESOURCE_HORSES".to_string(), amount);
    let five = ottoman.value_of_getting(&horses(5), None);
    assert_eq!(five, 5.0 * STRATEGIC_NEEDED_GOLD);
    let ten = ottoman.value_of_getting(&horses(10), None);
    assert_eq!(
        ten,
        five + 5.0 * STRATEGIC_IDLE_GOLD,
        "past the need, idle units"
    );
    let mut full = ottoman.clone();
    full.strategics
        .insert("RESOURCE_HORSES".to_string(), STRATEGIC_CAP_DEFAULT);
    assert_eq!(
        full.value_of_getting(&horses(10), None),
        0.0,
        "no room past the cap"
    );
    // Canada's 12 Horses rise 2 a turn: in thirty turns the cap wastes more
    // than the whole stockpile, so the stockpile goes for nothing.
    let canada = canada();
    assert_eq!(canada.cost_of_giving(&horses(10), None), 0.0);
    // A seat that will need what it gives pays the need price and more.
    let mut needy = canada.clone();
    needy
        .strategic_income
        .insert("RESOURCE_HORSES".to_string(), 0.0);
    needy
        .strategic_needs
        .insert("RESOURCE_HORSES".to_string(), 12.0);
    assert_eq!(
        needy.cost_of_giving(&horses(10), None),
        10.0 * STRATEGIC_NEEDED_GOLD * STRATEGIC_NEED_GIVE_PREMIUM
    );
    assert!(needy.cost_of_giving(&horses(10), None) > needy.value_of_getting(&horses(10), None));
}

#[test]
fn a_domination_seat_needs_its_eras_army_resources() {
    let state = board(OTTOMAN_T32);
    let siege = Position::from_board(&state, Some(VictoryTarget::Domination));
    let holy = Position::from_board(&state, Some(VictoryTarget::Religion));
    let iron = Item::Strategic("RESOURCE_IRON".to_string(), 20);
    assert!(siege.value_of_getting(&iron, None) > 2.0 * holy.value_of_getting(&iron, None));
    let oil = Item::Strategic("RESOURCE_OIL".to_string(), 20);
    assert_eq!(
        siege.value_of_getting(&oil, None),
        20.0 * STRATEGIC_IDLE_GOLD,
        "the Classical army has no use for Oil"
    );
}

#[test]
fn a_great_work_needs_a_slot_and_is_worth_most_to_a_culture_seat() {
    let state = board(OTTOMAN_T32);
    let writing = Item::GreatWork("GREATWORKOBJECT_WRITING".to_string());
    let mut culture = Position::from_board(&state, Some(VictoryTarget::Culture));
    assert_eq!(
        culture.value_of_getting(&writing, None),
        0.0,
        "nowhere to put it"
    );
    culture
        .great_work_room
        .insert("GREATWORKOBJECT_WRITING".to_string(), 1);
    let mut science = Position::from_board(&state, Some(VictoryTarget::Science));
    science
        .great_work_room
        .insert("GREATWORKOBJECT_WRITING".to_string(), 1);
    let to_culture = culture.value_of_getting(&writing, None);
    let to_science = science.value_of_getting(&writing, None);
    assert!(to_science > 0.0);
    assert!(
        to_culture > 2.0 * to_science,
        "{to_culture} vs {to_science}"
    );
    // One slot holds one work.
    assert_eq!(
        culture.net(&getting(vec![writing.clone(), writing]), None),
        to_culture
    );
}

#[test]
fn the_last_great_work_of_its_class_costs_more_than_one_of_several() {
    let mut seat = ottoman();
    let relic = Item::GreatWork("GREATWORKOBJECT_RELIC".to_string());
    assert_eq!(seat.cost_of_giving(&relic, None), f64::INFINITY);
    seat.great_works
        .insert("GREATWORKOBJECT_RELIC".to_string(), 1);
    let last = seat.cost_of_giving(&relic, None);
    seat.great_works
        .insert("GREATWORKOBJECT_RELIC".to_string(), 2);
    let one_of_two = seat.cost_of_giving(&relic, None);
    assert!(last > 0.0);
    assert!((last / one_of_two - LAST_GREAT_WORK_PREMIUM / SPARE_GREAT_WORK_DISCOUNT).abs() < 1e-9);
}

#[test]
fn favor_is_a_diplomacy_seats_votes() {
    let state = board(OTTOMAN_T32);
    let favor = Item::Favor(10);
    let diplomat = Position::from_board(&state, Some(VictoryTarget::Diplomacy));
    let general = Position::from_board(&state, None);
    assert_eq!(
        general.value_of_getting(&favor, None),
        10.0 * FAVOR_GOLD_PER_POINT
    );
    assert_eq!(
        diplomat.value_of_getting(&favor, None),
        10.0 * FAVOR_GOLD_PER_POINT * DIPLOMACY_FAVOR_PREMIUM
    );
}

#[test]
fn passage_follows_the_engines_book_and_the_partners_army() {
    let state = board(OTTOMAN_T32);
    let ottoman = ottoman();
    let person = rival(&state, 0);
    let canada = rival(&state, 3);
    assert_eq!(
        ottoman.value_of_getting(&Item::OpenBorders, Some(&person)),
        OPEN_BORDERS_BASE_GOLD
    );
    let to_the_strong = ottoman.cost_of_giving(&Item::OpenBorders, Some(&person));
    let to_the_weak = ottoman.cost_of_giving(&Item::OpenBorders, Some(&canada));
    // 123 against our 53, and 38 against it.
    assert!((to_the_strong - (9.0 + 8.0 * 123.0 / 53.0)).abs() < 1e-9);
    assert!(to_the_strong > to_the_weak);
    let culture = Position::from_board(&state, Some(VictoryTarget::Culture));
    assert!(
        culture.value_of_getting(&Item::OpenBorders, Some(&person))
            > ottoman.value_of_getting(&Item::OpenBorders, Some(&person))
    );
}

#[test]
fn friendship_is_worth_more_with_the_strong_and_ties_a_conquerors_hands() {
    let state = board(OTTOMAN_T32);
    let person = rival(&state, 0);
    let canada = rival(&state, 3);
    let holy = ottoman();
    let friendship = Item::Friendship;
    let with_strong = holy.value_of_getting(&friendship, Some(&person));
    let with_weak = holy.value_of_getting(&friendship, Some(&canada));
    assert!(with_strong > with_weak, "{with_strong} vs {with_weak}");
    assert_eq!(with_weak, FRIENDSHIP_GOLD);
    let siege = Position::from_board(&state, Some(VictoryTarget::Domination));
    assert!(
        siege.value_of_getting(&friendship, Some(&canada)) < 0.0,
        "Canada fields 38 against our 53: the Domination seat's target"
    );
    assert!(siege.value_of_getting(&friendship, Some(&person)) > 0.0);
    // The same declaration on either side of the deal, counted once.
    assert_eq!(
        holy.net(
            &Deal {
                give: vec![Item::Friendship],
                get: vec![Item::Friendship],
            },
            Some(&person)
        ),
        with_strong
    );
    let mut at_war = person.clone();
    at_war.at_war = true;
    assert_eq!(
        holy.value_of_getting(&friendship, Some(&at_war)),
        f64::NEG_INFINITY
    );
}

#[test]
fn a_delegation_is_hosted_unless_we_are_at_war_or_denounced() {
    let state = board(OTTOMAN_T32);
    let holy = ottoman();
    let person = rival(&state, 0);
    assert!(
        holy.cost_of_giving(&Item::Delegation, Some(&person)) < 0.0,
        "goodwill exceeds the cost"
    );
    assert_eq!(
        holy.value_of_getting(&Item::Delegation, Some(&person)),
        DELEGATION_GOLD - DELEGATION_FEE
    );
    let mut denounced = person.clone();
    denounced.their_denounce_turn = Some(20);
    assert_eq!(
        holy.cost_of_giving(&Item::Embassy, Some(&denounced)),
        f64::INFINITY
    );
    denounced.their_denounce_turn = Some(1);
    assert!(
        holy.cost_of_giving(&Item::Embassy, Some(&denounced))
            .is_finite(),
        "31 turns ago"
    );
}

#[test]
fn a_swap_both_seats_gain_from_is_priced_positive_by_each_from_its_own_board() {
    // The worked example: Canada's 10 spare Horses for 20 Gold from the
    // Ottomans, priced by each side from its own board alone.
    let horses = Item::Strategic("RESOURCE_HORSES".to_string(), 10);
    let canada_side = Deal {
        give: vec![horses.clone()],
        get: vec![Item::Gold(20)],
    };
    let ottoman_side = canada_side.mirrored();
    assert_eq!(ottoman_side.mirrored(), canada_side);
    assert_eq!(canada().net(&canada_side, None), 20.0);
    let ottoman_gain = ottoman().net(&ottoman_side, None);
    assert_eq!(
        ottoman_gain,
        5.0 * STRATEGIC_NEEDED_GOLD + 5.0 * STRATEGIC_IDLE_GOLD - 20.0
    );
    assert!(ottoman_gain > 0.0);
    // The same Horses for 60 Gold: the Ottomans cannot pay it, and would not.
    let dear = Deal {
        give: vec![Item::Gold(60)],
        get: vec![horses],
    };
    assert_eq!(ottoman().net(&dear, None), f64::NEG_INFINITY);
}

#[test]
fn every_item_is_priced_on_a_real_board_without_a_nan() {
    let state = board(OTTOMAN_T32);
    let person = rival(&state, 0);
    let items = [
        Item::Gold(10),
        Item::GoldPerTurn(1),
        silver(),
        Item::Strategic("RESOURCE_IRON".to_string(), 5),
        Item::Favor(5),
        Item::OpenBorders,
        Item::GreatWork("GREATWORKOBJECT_MUSIC".to_string()),
        Item::GreatWork("GREATWORK_NOT_A_CLASS".to_string()),
        Item::Delegation,
        Item::Embassy,
        Item::Friendship,
    ];
    for seat in [ottoman(), canada()] {
        for item in &items {
            for partner in [None, Some(&person)] {
                let get = seat.value_of_getting(item, partner);
                let give = seat.cost_of_giving(item, partner);
                assert!(!get.is_nan() && !give.is_nan(), "{item:?}");
                assert!(get.is_finite(), "{item:?} received is always priced");
            }
        }
    }
}
