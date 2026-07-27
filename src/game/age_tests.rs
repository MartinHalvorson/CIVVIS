//! Rise & Fall Ages: the Normal-Age half of every Dedication, the era each
//! Dedication can be chosen in, and the Dark/Normal/Golden/Heroic ladder.
use super::{Action, Game};

fn two_player_game() -> Game {
    Game::new_full(2, 30, 18, 515, 300, 0, false)
}

#[test]
fn a_late_unlock_cannot_skip_world_eras() {
    let mut game = two_player_game();
    game.world_era = 2;
    game.players[0]
        .techs
        .insert("telecommunications".to_string());
    assert_eq!(
        game.era_from_progress(),
        7,
        "the fixture's leading civilization has reached Information"
    );

    for expected in 3..=7 {
        // Shipped Eras_XP1.GameEraMinimumTurns holds each era open for 40
        // standard turns, so the intervening eras arrive one after another
        // rather than all on the same turn -- but every one of them still
        // arrives, which is what this test is about.
        game.turn += 40;
        game.process_eras();
        assert_eq!(
            game.world_era, expected,
            "the world must enter every era between Medieval and Information"
        );
    }
}

#[test]
fn an_era_is_held_open_for_its_shipped_minimum() {
    // Before this floor existed the world era tracked the single most advanced
    // civilization with nothing holding it back: measured over 60 six-seat
    // games the 10th percentile gap between age transitions was ONE turn, and
    // an age nobody had turns to bank Era Score in is a Dark Age for the whole
    // table. 79% of all transitions were Dark.
    let mut game = two_player_game();
    game.world_era = 1;
    game.world_era_since = 10;
    game.players[0].techs.insert("telecommunications".to_string());

    game.turn = 40;
    game.process_eras();
    assert_eq!(
        game.world_era, 1,
        "39 turns into the era is too early for the next one"
    );

    game.turn = 50;
    game.process_eras();
    assert_eq!(game.world_era, 2, "40 turns in, the era may turn over");
    assert_eq!(
        game.world_era_since, 50,
        "and the clock restarts for the era just entered"
    );
}

#[test]
fn every_dedication_carries_both_halves_and_an_era_span() {
    let rules = crate::rules::Rules::embedded();
    assert_eq!(
        rules.dedications.len(),
        12,
        "Rise & Fall ships twelve Dedications"
    );
    for (name, spec) in rules.dedications.iter() {
        assert!(!spec.normal.is_empty(), "{name} has no Normal-Age text");
        assert!(!spec.golden.is_empty(), "{name} has no Golden-Age text");
        assert!(
            !spec.triggers.is_empty(),
            "{name} pays no Era Score in a Normal Age"
        );
        assert!(
            spec.eras.0 >= 1 && spec.eras.1 < crate::rules::ERA_NAMES.len(),
            "{name} spans {:?}, which is not a run of real eras",
            spec.eras
        );
        assert!(spec.eras.0 <= spec.eras.1, "{name} spans backwards");
    }
}

#[test]
fn dedications_are_offered_only_in_their_own_eras() {
    let mut game = two_player_game();
    game.players[0].dedication_choices = 1;

    // Classical: the early four are on offer and the late ones are not.
    game.world_era = 1;
    let classical = game.available_dedications(0);
    assert!(classical.contains(&"monumentality".to_string()));
    assert!(classical.contains(&"exodus_of_the_evangelists".to_string()));
    assert!(!classical.contains(&"automaton_warfare".to_string()));
    assert!(!classical.contains(&"wish_you_were_here".to_string()));

    // Information: the late ones are, and the Classical-only ones are gone.
    game.world_era = 7;
    let information = game.available_dedications(0);
    assert!(information.contains(&"automaton_warfare".to_string()));
    assert!(information.contains(&"wish_you_were_here".to_string()));
    assert!(!information.contains(&"exodus_of_the_evangelists".to_string()));
    assert!(!information.contains(&"monumentality".to_string()));
}

#[test]
fn the_two_gathering_storm_dedications_exist_and_can_be_chosen() {
    let mut game = two_player_game();
    game.world_era = 7;
    game.players[0].dedication_choices = 2;
    for dedication in ["wish_you_were_here", "bodyguard_of_lies"] {
        game.apply(
            0,
            &Action::ChooseDedication {
                dedication: dedication.to_string(),
            },
        )
        .unwrap_or_else(|error| panic!("{dedication} should be choosable: {error}"));
        assert!(game.players[0].dedications.contains(dedication));
    }
}

#[test]
fn a_normal_age_dedication_still_pays_era_score() {
    let mut game = two_player_game();
    game.players[0].age = "normal".to_string();
    game.players[0]
        .dedications
        .insert("free_inquiry".to_string());
    let before = game.players[0].era_score;

    game.dedication_trigger(0, "eureka", 1);

    assert_eq!(
        game.players[0].era_score,
        before + 1,
        "Free Inquiry pays +1 Era Score per Eureka in a Normal Age"
    );
}

#[test]
fn a_dark_age_dedication_pays_the_same_score_but_not_the_golden_bonus() {
    let mut game = two_player_game();
    game.players[0].age = "dark".to_string();
    game.players[0]
        .dedications
        .insert("monumentality".to_string());
    let before = game.players[0].era_score;

    // PER_DISTRICT_CONSTRUCTED, not specialty -- see note_dedicated_building.
    game.dedication_trigger(0, "district", 1);

    assert_eq!(
        game.players[0].era_score,
        before + 1,
        "a Dark Age Dedication is how a civilization climbs out of it"
    );
    assert!(
        !game.dedication_active(0, "monumentality"),
        "but the Golden-Age half stays off"
    );

    game.players[0].age = "golden".to_string();
    assert!(game.dedication_active(0, "monumentality"));
}

#[test]
fn a_dedication_pays_only_for_its_own_trigger() {
    let mut game = two_player_game();
    game.players[0].age = "normal".to_string();
    game.players[0].dedications.insert("to_arms".to_string());
    let before = game.players[0].era_score;

    game.dedication_trigger(0, "eureka", 3);
    assert_eq!(game.players[0].era_score, before, "To Arms! is not a Eureka");

    game.dedication_trigger(0, "army_kill", 2);
    assert_eq!(
        game.players[0].era_score,
        before + 4,
        "two Army kills at +2 Era Score each"
    );
}

#[test]
fn a_heroic_age_still_grants_three_dedications() {
    let mut game = two_player_game();
    game.players[0].age = "dark".to_string();
    game.players[0].era_score = game.players[0].golden_age_threshold;
    game.players[1].era_score = 0;
    game.players[0].techs.insert("horseback_riding".to_string());
    // An era is held open for its shipped 40-turn minimum, so a fixture that
    // wants the next one has to stand far enough into this one.
    game.turn = 40;
    game.process_eras();
    assert_eq!(game.players[0].age, "heroic");
    assert_eq!(game.players[0].dedication_choices, 3);
    assert_eq!(
        game.players[1].dedication_choices, 1,
        "and every other age grants exactly one"
    );
}

#[test]
fn an_age_transition_clears_last_age_dedications() {
    let mut game = two_player_game();
    game.players[0]
        .dedications
        .insert("monumentality".to_string());
    game.players[0].techs.insert("horseback_riding".to_string());
    // An era is held open for its shipped 40-turn minimum, so a fixture that
    // wants the next one has to stand far enough into this one.
    game.turn = 40;
    game.process_eras();
    assert!(
        game.players[0].dedications.is_empty(),
        "a Dedication lasts one age"
    );
}

#[test]
fn dark_age_policy_cards_are_offered_only_inside_a_dark_age() {
    let mut game = two_player_game();
    game.world_era = 2;
    game.players[0].civics.insert("code_of_laws".to_string());

    game.players[0].age = "normal".to_string();
    let normal = game.available_policies(0);
    assert!(
        !normal.contains(&"twilight_valor".to_string()),
        "a Normal Age never sees a Dark Age card"
    );
    assert!(
        normal.contains(&"discipline".to_string()),
        "but the ordinary cards it has unlocked are still there"
    );

    game.players[0].age = "dark".to_string();
    let dark = game.available_policies(0);
    assert!(dark.contains(&"twilight_valor".to_string()));
    assert!(dark.contains(&"inquisition".to_string()));
    assert!(
        !dark.contains(&"robber_barons".to_string()),
        "Robber Barons is an Industrial-era card"
    );
    assert!(
        !dark.contains(&"automated_workforce".to_string()),
        "the Gathering Storm additions are not modelled yet"
    );
}

#[test]
fn every_dark_age_card_is_a_wildcard_with_a_cost() {
    let rules = crate::rules::Rules::embedded();
    let dark: Vec<_> = rules
        .policies
        .iter()
        .filter(|(_, spec)| spec.dark_age)
        .collect();
    assert_eq!(dark.len(), 7);
    for (name, spec) in dark {
        assert_eq!(spec.slot, "wildcard", "{name} must take a Wildcard slot");
        assert!(
            spec.civic.is_none(),
            "{name} is unlocked by an age, not a civic"
        );
        assert!(spec.eras.is_some(), "{name} needs an era span");
        assert!(
            spec.effects.values().any(|value| *value < 0.0)
                || spec
                    .effects
                    .keys()
                    .any(|key| key.starts_with("no_") || key.ends_with("_surcharge")),
            "{name} is a Dark Age card and must carry a drawback"
        );
    }
}

#[test]
fn leaving_a_dark_age_takes_the_card_back_out_of_its_slot() {
    let mut game = two_player_game();
    game.world_era = 1;
    game.players[0].age = "dark".to_string();
    game.players[0].policies.insert("twilight_valor".to_string());
    game.players[0].policies.insert("discipline".to_string());
    // Cross into the Classical era with enough Era Score for a Heroic Age.
    game.players[0].era_score = game.players[0].golden_age_threshold;
    game.players[0].techs.insert("horseback_riding".to_string());
    game.world_era = 0;
    // An era is held open for its shipped 40-turn minimum, so a fixture that
    // wants the next one has to stand far enough into this one.
    game.turn = 40;

    game.process_eras();

    assert_eq!(game.players[0].age, "heroic");
    assert!(
        !game.players[0].policies.contains("twilight_valor"),
        "the Dark Age card goes back when the Dark Age does"
    );
    assert!(
        game.players[0].policies.contains("discipline"),
        "ordinary cards stay slotted"
    );
}

#[test]
fn twilight_valor_pays_on_the_attack_and_charges_for_it() {
    let mut game = two_player_game();
    game.players[0].age = "dark".to_string();
    let position = game
        .units
        .values()
        .find(|unit| unit.owner == 0)
        .map(|unit| unit.pos)
        .unwrap();
    let warrior = game.spawn_unit("warrior", 0, position);
    // A tile nobody owns: the unit is abroad.
    let away = game
        .wdisk(position, 3)
        .into_iter()
        .find(|pos| {
            game.map.tiles[pos].owner_city.is_none()
                && !game.rules.is_water(&game.map.tiles[pos])
                && *pos != position
        })
        .unwrap();
    game.units.get_mut(&warrior).unwrap().pos = away;
    game.units.get_mut(&warrior).unwrap().hp = 50;

    let heal_before = game.unit_heal_rate(warrior);
    assert!(heal_before > 0, "a wounded unit normally heals somewhere");

    game.players[0].policies.insert("twilight_valor".to_string());
    assert_eq!(
        game.unit_heal_rate(warrior),
        0,
        "Twilight Valor stops a unit healing outside your own territory"
    );
    assert_eq!(
        game.policy_effect(0, "melee_attack_combat"),
        5.0,
        "and pays +5 Combat Strength on a melee attack for it"
    );
}

#[test]
fn isolationism_closes_the_frontier_and_pays_at_home() {
    let mut game = two_player_game();
    game.players[0].age = "dark".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.current = 0;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.cities.get_mut(&city).unwrap().pop = 4;
    assert!(game.can_produce_unit(0, city, "settler", true, 0.0));

    game.players[0].policies.insert("isolationism".to_string());
    assert!(
        !game.can_produce_unit(0, city, "settler", true, 0.0),
        "Isolationism forbids training Settlers"
    );
    // The shipped card pays +3 of all three yields on a domestic route.
    assert_eq!(game.policy_effect(0, "domestic_trade_food"), 3.0);
    assert_eq!(game.policy_effect(0, "domestic_trade_production"), 3.0);
    assert_eq!(game.policy_effect(0, "domestic_trade_gold"), 3.0);
    assert_eq!(game.policy_effect(0, "policy_trade_route_capacity"), 1.0);
}

#[test]
fn robber_barons_costs_amenities_everywhere_it_pays() {
    let mut game = two_player_game();
    game.players[0].age = "dark".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.current = 0;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    let before = game.city_local_amenities(&game.cities[&city]);

    game.players[0].policies.insert("robber_barons".to_string());
    assert_eq!(
        game.city_local_amenities(&game.cities[&city]),
        before - 2,
        "-2 Amenities in every city is what the Gold and Production cost"
    );
}

#[test]
fn a_building_pays_the_dedication_its_yields_name_not_its_slots() {
    // PER_CULTURE_BUILDING_CONSTRUCTED and PER_SCIENCE_BUILDING_CONSTRUCTED are
    // named the same way as each other, so they must be read the same way: a
    // building that yields that yield. CIVVIS read the Culture one as "has a
    // Great Work slot", and the two sets differ in both directions -- a
    // Monument yields Culture with no slot, a Temple has a slot and no Culture.
    let mut game = two_player_game();
    let rules = crate::rules::Rules::embedded();
    let culture = |name: &str| rules.buildings[name].yields.culture > 0.0;
    let slotted = |name: &str| {
        rules.buildings[name]
            .great_work_slots
            .values()
            .any(|slots| *slots > 0)
    };
    assert!(culture("monument") && !slotted("monument"));
    assert!(slotted("temple") && !culture("temple"));

    game.players[0]
        .dedications
        .insert("pen_brush_and_voice".to_string());
    let score = |game: &Game| game.players[0].era_score;
    let before = score(&game);
    let monument = rules.buildings["monument"].clone();
    game.note_dedicated_building(0, "monument", &monument);
    assert_eq!(score(&game), before + 1, "a Monument yields Culture");
    let temple = rules.buildings["temple"].clone();
    game.note_dedicated_building(0, "temple", &temple);
    assert_eq!(score(&game), before + 1, "a Temple does not");
}

#[test]
fn heartbeat_of_steam_pays_one_era_score_per_industrial_building() {
    // ADJUST_PLAYER_ERA_SCORE_PER_INDUSTRIAL_BUILDING_CONSTRUCTED is 1. Every
    // other building-shaped dedication trigger ships 1 as well; only Religious
    // conversions and Army kills pay 2.
    let rules = crate::rules::Rules::embedded();
    assert_eq!(rules.dedications["heartbeat_of_steam"].triggers["industrial_building"], 1);
    assert_eq!(rules.dedications["exodus_of_the_evangelists"].triggers["city_converted"], 2);
    assert_eq!(rules.dedications["to_arms"].triggers["army_kill"], 2);

    let mut game = two_player_game();
    game.players[0]
        .dedications
        .insert("heartbeat_of_steam".to_string());
    let before = game.players[0].era_score;
    let factory = rules.buildings["factory"].clone();
    game.note_dedicated_building(0, "factory", &factory);
    assert_eq!(game.players[0].era_score, before + 1);
}

#[test]
fn era_score_moments_pay_what_the_moments_table_ships() {
    // Moments carries an EraScore per moment, and several of CIVVIS' awards had
    // drifted from it. The first-in-world variants are separate rows, which is
    // why the plain ones are worth less than CIVVIS was paying.
    let mut game = two_player_game();
    let score = |game: &Game, pid: usize| game.players[pid].era_score;

    // MOMENT_PANTHEON_FOUNDED 1, _FIRST_IN_WORLD 2.
    let before = score(&game, 0);
    game.players[0].faith = 1_000.0;
    game.do_choose_pantheon(0, "god_of_the_forge").unwrap();
    assert_eq!(score(&game, 0) - before, 2, "the world's first Pantheon");
    let before = score(&game, 1);
    game.players[1].faith = 1_000.0;
    game.do_choose_pantheon(1, "divine_spark").unwrap();
    assert_eq!(score(&game, 1) - before, 1, "the second is worth one");

    // MOMENT_BARBARIAN_CAMP_DESTROYED is 2, and its window is ANCIENT through
    // MEDIEVAL -- the only Moment CIVVIS models that stops paying.
    game.world_era = 0;
    assert_eq!(game.barbarian_camp_era_score(), 2);
    game.world_era = 2;
    assert_eq!(game.barbarian_camp_era_score(), 2, "Medieval still pays");
    game.world_era = 3;
    assert_eq!(game.barbarian_camp_era_score(), 0, "the Renaissance does not");
}

#[test]
fn a_wonder_is_worth_more_while_it_is_still_current() {
    // MOMENT_BUILDING_CONSTRUCTED_GAME_ERA_WONDER is 4 and _PAST_ERA_WONDER is
    // 3. CIVVIS paid a flat 3 for every wonder.
    let rules = crate::rules::Rules::embedded();
    // pyramids unlock in the Ancient era, big_ben in the Industrial.
    let mut game = two_player_game();
    assert_eq!(game.wonder_era("pyramids"), 0);
    assert_eq!(game.wonder_era("big_ben"), 4);

    game.world_era = 0;
    assert!(game.wonder_era("pyramids") >= game.world_era, "current era");
    game.world_era = 4;
    assert!(game.wonder_era("pyramids") < game.world_era, "long past");
    let _ = rules;
}

#[test]
fn a_golden_age_pays_its_bonus_and_banks_nothing() {
    // Every COMMEMORATION_*_QUEST modifier hangs off
    // PLAYER_ELIGIBLE_FOR_COMMEMORATION_QUEST, a TEST_ANY set whose only two
    // members are an inverted REQUIREMENT_PLAYER_HAS_GOLDEN_AGE and a
    // REQUIREMENT_PLAYER_ALWAYS_ALLOWED_COMMEMORATION_QUEST nothing in the
    // shipped data grants. So the two halves are exclusive, and a Golden Age
    // cannot finance its own successor.
    let mut game = two_player_game();
    game.players[0]
        .dedications
        .insert("free_inquiry".to_string());

    for age in ["golden", "heroic"] {
        game.players[0].age = age.to_string();
        game.players[0].era_score = 0;
        game.dedication_trigger(0, "eureka", 4);
        assert_eq!(
            game.players[0].era_score, 0,
            "a {age} Age banks no Era Score"
        );
        assert!(
            game.dedication_active(0, "free_inquiry"),
            "it is paid in the Golden-Age half instead"
        );
    }

    game.players[0].age = "normal".to_string();
    game.players[0].era_score = 0;
    game.dedication_trigger(0, "eureka", 4);
    assert_eq!(
        game.players[0].era_score, 4,
        "and a Normal Age banks, which is the whole trade"
    );
}

#[test]
fn the_trigger_tally_counts_behaviour_not_what_was_paid_for() {
    // The tally is what `projected_dedication_score` reads, so it has to be
    // the behaviour itself: kept whether or not the trigger was dedicated, and
    // kept through a Golden Age that pays nothing for it.
    let mut game = two_player_game();
    game.players[0].age = "golden".to_string();
    game.dedication_trigger(0, "eureka", 3);
    game.dedication_trigger(0, "district", 2);

    assert_eq!(game.players[0].era_triggers.get("eureka"), Some(&3));
    assert_eq!(game.players[0].era_triggers.get("district"), Some(&2));
    assert_eq!(
        game.players[0].era_score, 0,
        "counted, but not paid, and no Dedication was even held"
    );
}

#[test]
fn a_dedication_is_projected_from_the_era_that_just_ended() {
    let mut game = two_player_game();
    game.players[0].age = "normal".to_string();
    game.dedication_trigger(0, "eureka", 5);
    game.dedication_trigger(0, "science_building", 2);
    game.dedication_trigger(0, "district", 1);
    assert_eq!(
        game.projected_dedication_score(0, "free_inquiry"),
        0,
        "an era in progress is not yet evidence"
    );

    game.players[0].techs.insert("horseback_riding".to_string());
    // An era is held open for its shipped 40-turn minimum, so a fixture that
    // wants the next one has to stand far enough into this one.
    game.turn = 40;
    game.process_eras();

    // Free Inquiry pays 1 per Eureka and 1 per Science building: 5 + 2.
    assert_eq!(game.projected_dedication_score(0, "free_inquiry"), 7);
    // Monumentality pays 1 per District: 1.
    assert_eq!(game.projected_dedication_score(0, "monumentality"), 1);
    // Nothing naval happened, so Hic Sunt Dracones projects nothing.
    assert_eq!(game.projected_dedication_score(0, "hic_sunt_dracones"), 0);
    assert!(
        game.players[0].era_triggers.is_empty(),
        "and the new era starts its own tally"
    );
}

#[test]
fn the_ai_dedicates_on_its_record_rather_than_alphabetically() {
    use crate::ai::{choose_dedications, DedicationChoice};

    // Alphabetically the Classical era offers exodus_of_the_evangelists first,
    // and that is what both AI tiers used to take in every game ever played.
    let mut game = two_player_game();
    game.world_era = 1;
    game.players[0].age = "normal".to_string();
    game.players[0].dedication_choices = 1;
    game.players[0]
        .last_era_triggers
        .insert("eureka".to_string(), 6);

    let mut control = game.clone();
    choose_dedications(&mut control, 0, DedicationChoice::Alphabetical);
    assert!(
        control.players[0]
            .dedications
            .contains("exodus_of_the_evangelists"),
        "the frozen control still takes the first name in the map"
    );

    choose_dedications(&mut game, 0, DedicationChoice::Measured);
    assert!(
        game.players[0].dedications.contains("free_inquiry"),
        "six Eurekas last era say Free Inquiry, not a religion this civ has not founded"
    );
    assert_eq!(game.players[0].dedication_choices, 0);
}

#[test]
fn a_heroic_age_takes_the_three_best_dedications() {
    use crate::ai::{choose_dedications, DedicationChoice};

    let mut game = two_player_game();
    game.world_era = 1;
    game.players[0].age = "heroic".to_string();
    game.players[0].dedication_choices = 3;
    for (trigger, count) in [("eureka", 9), ("district", 5), ("inspiration", 2)] {
        game.players[0]
            .last_era_triggers
            .insert(trigger.to_string(), count);
    }

    choose_dedications(&mut game, 0, DedicationChoice::Measured);

    let held = &game.players[0].dedications;
    assert_eq!(held.len(), 3, "a Heroic Age dedicates three times");
    assert!(held.contains("free_inquiry"), "9 Eurekas");
    assert!(held.contains("monumentality"), "5 Districts");
    assert!(held.contains("pen_brush_and_voice"), "2 Inspirations");
    assert!(
        !held.contains("exodus_of_the_evangelists"),
        "and the one with no record behind it is the one left out"
    );
}

#[test]
fn every_dark_age_card_spans_the_eras_the_shipped_game_offers_it_in() {
    // Policies_XP1.MinimumGameEra / MaximumGameEra, as ChronologyIndex - 1.
    let rules = crate::rules::Rules::embedded();
    for (name, first, last) in [
        ("monasticism", 1, 2),
        ("twilight_valor", 1, 3),
        ("inquisition", 1, 3),
        ("elite_forces", 1, 3),
        ("isolationism", 1, 4),
        ("letters_of_marque", 3, 5),
        ("robber_barons", 4, 6),
    ] {
        let spec = &rules.policies[name];
        assert!(spec.dark_age, "{name} is a Dark Age card");
        assert_eq!(
            spec.eras,
            Some((first, last)),
            "{name} spans the wrong eras"
        );
        assert!(!spec.offered("normal", first), "{name} needs a Dark Age");
        assert!(spec.offered("dark", first), "{name} opens at {first}");
        assert!(spec.offered("dark", last), "{name} closes after {last}");
        assert!(
            !spec.offered("dark", last + 1),
            "{name} is gone by {}",
            last + 1
        );
    }
}
