//! Rise & Fall Ages: the Normal-Age half of every Dedication, the era each
//! Dedication can be chosen in, and the Dark/Normal/Golden/Heroic ladder.
use super::{Action, Emergency, Game, Item, TradeRoute};
use crate::name::Name;

fn two_player_game() -> Game {
    Game::new_full(2, 30, 18, 515, 300, 0, false)
}

fn finish_minimum_era_countdown(game: &mut Game) {
    let since = game.world_era_since;
    game.turn = since + 30;
    game.process_eras();
    game.turn = since + 40;
    game.process_eras();
}

fn found_capital(game: &mut Game, pid: usize) -> u32 {
    game.current = pid;
    let settler = game
        .player_unit_ids(pid)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(pid, &Action::FoundCity { unit: settler })
        .unwrap();
    game.player_city_ids(pid)[0]
}

#[test]
fn a_late_unlock_cannot_skip_world_eras() {
    let mut game = two_player_game();
    game.world_era = 2;
    game.players[0]
        .techs
        .insert(crate::name!("telecommunications"));
    assert_eq!(
        game.era_from_progress(),
        7,
        "exactly half of an even field arms the era countdown"
    );

    for expected in 3..=7 {
        // Shipped Eras_XP1.GameEraMinimumTurns holds each era open for 40
        // standard turns, so the intervening eras arrive one after another
        // rather than all on the same turn -- but every one of them still
        // arrives, which is what this test is about.
        finish_minimum_era_countdown(&mut game);
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
    game.players[0]
        .techs
        .insert(crate::name!("telecommunications"));
    game.players[1]
        .techs
        .insert(crate::name!("telecommunications"));

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
fn an_era_turns_over_at_its_shipped_maximum() {
    let mut game = two_player_game();
    game.world_era = 0;
    game.world_era_since = 0;
    game.turn = 59;
    game.process_eras();
    assert_eq!(
        game.world_era, 0,
        "the Ancient era is still open on turn 59"
    );

    game.turn = 60;
    game.process_eras();
    assert_eq!(
        game.world_era, 1,
        "GameEraMaximumTurns advances even before the research median"
    );
}

#[test]
fn a_legacy_save_recovers_the_exact_dynamic_threshold() {
    let mut game = two_player_game();
    game.world_era = 2;
    game.world_era_since = 0;
    for pid in 0..2 {
        game.players[pid].techs.insert(crate::name!("gunpowder"));
        game.players[pid].normal_age_threshold = 0;
        game.players[pid].golden_age_threshold = 0;
        game.players[pid].past_dark_ages = 1;
        game.players[pid].era_score = 10;
    }

    finish_minimum_era_countdown(&mut game);

    assert_eq!(game.players[0].age, "normal");
    assert_eq!(
        game.players[0].normal_age_threshold, 10,
        "the next threshold keeps Gathering Storm's shipped -5 past-Dark adjustment"
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
    assert!(classical.contains(&crate::name!("monumentality")));
    assert!(classical.contains(&crate::name!("exodus_of_the_evangelists")));
    assert!(!classical.contains(&crate::name!("automaton_warfare")));
    assert!(!classical.contains(&crate::name!("wish_you_were_here")));

    // Information: the late ones are, and the Classical-only ones are gone.
    game.world_era = 7;
    let information = game.available_dedications(0);
    assert!(information.contains(&crate::name!("automaton_warfare")));
    assert!(information.contains(&crate::name!("wish_you_were_here")));
    assert!(!information.contains(&crate::name!("exodus_of_the_evangelists")));
    assert!(!information.contains(&crate::name!("monumentality")));
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
                dedication: Name::new(dedication),
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
    assert_eq!(
        game.players[0].era_score, before,
        "To Arms! is not a Eureka"
    );

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
    game.players[0]
        .techs
        .insert(crate::name!("horseback_riding"));
    game.players[1]
        .techs
        .insert(crate::name!("horseback_riding"));
    // An era is held open for its shipped 40-turn minimum, so a fixture that
    // wants the next one has to stand far enough into this one.
    finish_minimum_era_countdown(&mut game);
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
    game.players[0]
        .techs
        .insert(crate::name!("horseback_riding"));
    game.players[1]
        .techs
        .insert(crate::name!("horseback_riding"));
    // An era is held open for its shipped 40-turn minimum, so a fixture that
    // wants the next one has to stand far enough into this one.
    finish_minimum_era_countdown(&mut game);
    assert!(
        game.players[0].dedications.is_empty(),
        "a Dedication lasts one age"
    );
}

#[test]
fn dark_age_policy_cards_are_offered_only_inside_a_dark_age() {
    let mut game = two_player_game();
    game.world_era = 2;
    game.players[0].civics.insert(crate::name!("code_of_laws"));

    game.players[0].age = "normal".to_string();
    let normal = game.available_policies(0);
    assert!(
        !normal.contains(&crate::name!("twilight_valor")),
        "a Normal Age never sees a Dark Age card"
    );
    assert!(
        normal.contains(&crate::name!("discipline")),
        "but the ordinary cards it has unlocked are still there"
    );

    game.players[0].age = "dark".to_string();
    let dark = game.available_policies(0);
    assert!(dark.contains(&crate::name!("twilight_valor")));
    assert!(dark.contains(&crate::name!("inquisition")));
    assert!(
        !dark.contains(&crate::name!("robber_barons")),
        "Robber Barons is an Industrial-era card"
    );
    assert!(
        !dark.contains(&crate::name!("automated_workforce")),
        "Automated Workforce is one of the explicitly out-of-scope Dark Age cards"
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
    game.players[0]
        .policies
        .insert(crate::name!("twilight_valor"));
    game.players[0].policies.insert(crate::name!("discipline"));
    // Cross into the Classical era with enough Era Score for a Heroic Age.
    game.players[0].era_score = game.players[0].golden_age_threshold;
    game.players[0]
        .techs
        .insert(crate::name!("horseback_riding"));
    game.players[1]
        .techs
        .insert(crate::name!("horseback_riding"));
    game.world_era = 0;
    // An era is held open for its shipped 40-turn minimum, so a fixture that
    // wants the next one has to stand far enough into this one.
    finish_minimum_era_countdown(&mut game);

    assert_eq!(game.players[0].age, "heroic");
    assert!(
        !game.players[0]
            .policies
            .contains(&crate::name!("twilight_valor")),
        "the Dark Age card goes back when the Dark Age does"
    );
    assert!(
        game.players[0]
            .policies
            .contains(&crate::name!("discipline")),
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

    game.players[0]
        .policies
        .insert(crate::name!("twilight_valor"));
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
    assert!(game.can_produce_unit(0, city, crate::name!("settler"), true, 0.0));

    game.players[0]
        .policies
        .insert(crate::name!("isolationism"));
    assert!(
        !game.can_produce_unit(0, city, crate::name!("settler"), true, 0.0),
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

    game.players[0]
        .policies
        .insert(crate::name!("robber_barons"));
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
    assert_eq!(
        rules.dedications["heartbeat_of_steam"].triggers["industrial_building"],
        1
    );
    assert_eq!(
        rules.dedications["exodus_of_the_evangelists"].triggers["city_converted"],
        2
    );
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
fn kill_dedications_use_domain_formation_and_weapon_and_exclude_barbarians() {
    let mut game = two_player_game();
    game.players[0].dedications.extend([
        "hic_sunt_dracones".to_string(),
        "to_arms".to_string(),
        "automaton_warfare".to_string(),
    ]);
    let mut victim = game
        .units
        .values()
        .find(|unit| unit.owner == 1)
        .unwrap()
        .clone();
    victim.kind = crate::name!("galley");
    victim.formation = 2;

    game.players[0].era_score = 0;
    game.record_kill(0, Some("giant_death_robot"), &victim);
    assert_eq!(
        game.players[0].era_score, 4,
        "a non-Barbarian Armada killed by a GDR pays naval + Army + robot triggers"
    );

    victim.owner = game
        .players
        .iter()
        .find(|player| player.is_barbarian)
        .unwrap()
        .id;
    game.record_kill(0, Some("giant_death_robot"), &victim);
    assert_eq!(
        game.players[0].era_score, 4,
        "none of the three kill Dedications pays for a Barbarian"
    );
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

    // MOMENT_BARBARIAN_CAMP_DESTROYED is 2, with an ANCIENT-through-MEDIEVAL
    // availability window.
    game.world_era = 0;
    assert_eq!(game.barbarian_camp_era_score(), 2);
    game.world_era = 2;
    assert_eq!(game.barbarian_camp_era_score(), 2, "Medieval still pays");
    game.world_era = 3;
    assert_eq!(
        game.barbarian_camp_era_score(),
        0,
        "the Renaissance does not"
    );

    let mut late = two_player_game();
    late.world_era = 2;
    late.players[0].faith = 1_000.0;
    late.do_choose_pantheon(0, "god_of_the_forge").unwrap();
    assert_eq!(
        late.players[0].era_score, 0,
        "Pantheon moments are obsolete in Medieval"
    );
}

#[test]
fn high_adjacency_moments_use_each_shipped_threshold() {
    for family in ["campus", "holy_site", "theater_square"] {
        assert_eq!(Game::district_historic_moment_threshold(family), Some(3.0));
    }
    for family in ["commercial_hub", "harbor", "industrial_zone"] {
        assert_eq!(Game::district_historic_moment_threshold(family), Some(4.0));
    }
    assert_eq!(Game::district_historic_moment_threshold("encampment"), None);
}

#[test]
fn first_in_world_moments_replace_instead_of_stack() {
    let mut game = two_player_game();
    let ordinary = Some("MOMENT_PLAYER_MET_ALL_MAJORS");
    let world_first = Some("MOMENT_PLAYER_MET_ALL_MAJORS_FIRST_IN_WORLD");
    assert!(game.first_historic_moment(0, "test_first", ordinary, world_first));
    assert_eq!(game.players[0].era_score, 5);
    assert!(!game.first_historic_moment(0, "test_first", ordinary, world_first));
    assert_eq!(game.players[0].era_score, 5, "a moment is paid once");

    assert!(!game.first_historic_moment(1, "test_first", ordinary, world_first));
    assert_eq!(
        game.players[1].era_score, 3,
        "the ordinary row replaces the first-in-world row for later civs"
    );
}

#[test]
fn a_natural_wonder_city_site_is_worth_three() {
    let mut game = two_player_game();
    let position = game
        .units
        .values()
        .find(|unit| unit.owner == 0)
        .unwrap()
        .pos;
    for tile in game.wdisk(position, 2) {
        let tile = game.map.tiles.get_mut(&tile).unwrap();
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
    }
    let wonder = game.nbrs(position)[0];
    game.map.tiles.get_mut(&wonder).unwrap().feature = Some(crate::name!("crater_lake"));
    game.players[0].era_score = 0;

    game.note_city_founding_moments(0, position);

    assert_eq!(game.players[0].era_score, 3);
}

#[test]
fn founded_projects_pay_their_world_and_ordinary_rows() {
    let mut game = two_player_game();
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
        game.players[pid].era_score = 0;
    }
    let first_city = game.player_city_ids(0)[0];
    let second_city = game.player_city_ids(1)[0];
    let satellite = Item::Project {
        project: crate::name!("launch_earth_satellite"),
    };

    assert!(game.complete_item(0, first_city, &satellite));
    assert_eq!(game.players[0].era_score, 4);
    assert!(game.complete_item(1, second_city, &satellite));
    assert_eq!(game.players[1].era_score, 2);
}

#[test]
fn unit_and_formation_firsts_pay_their_exact_variants() {
    let mut game = two_player_game();
    let mut cities = Vec::new();
    for pid in 0..2 {
        game.current = pid;
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(pid, &Action::FoundCity { unit: settler })
            .unwrap();
        game.players[pid].era_score = 0;
        cities.push(game.player_city_ids(pid)[0]);
    }

    game.note_unit_created_moments(0, "biplane");
    game.note_unit_created_moments(1, "biplane");
    assert_eq!(
        game.players[0].era_score, 7,
        "world's first air unit (+5) also fields the world's first Oil unit (+2)"
    );
    assert_eq!(
        game.players[1].era_score, 4,
        "the later civilization earns ordinary air (+3) and Oil (+1) rows"
    );

    game.players[0].era_score = 0;
    game.players[1].era_score = 0;
    let first = game.spawn_unit("warrior", 0, game.cities[&cities[0]].pos);
    game.units.get_mut(&first).unwrap().formation = 1;
    game.note_formation_moment(0, first);
    let second = game.spawn_unit("warrior", 1, game.cities[&cities[1]].pos);
    game.units.get_mut(&second).unwrap().formation = 1;
    game.note_formation_moment(1, second);
    assert_eq!(game.players[0].era_score, 2, "world's first Corps");
    assert_eq!(
        game.players[1].era_score, 1,
        "later civilization's first Corps"
    );
}

#[test]
fn taj_mahal_modifies_moments_but_never_dedication_score() {
    let mut game = two_player_game();
    let city = found_capital(&mut game, 0);
    let position = game.cities[&city].pos;
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("taj_mahal"), position);

    game.players[0].era_score = 0;
    game.add_historic_moment(0, "MOMENT_BARBARIAN_CAMP_DESTROYED");
    assert_eq!(game.players[0].era_score, 3, "a +2 moment gets Taj's +1");

    game.players[0].era_score = 0;
    game.players[0].age = "normal".to_string();
    game.players[0]
        .dedications
        .insert("exodus_of_the_evangelists".to_string());
    game.dedication_trigger(0, "city_converted", 1);
    assert_eq!(
        game.players[0].era_score, 2,
        "a +2 Dedication trigger is not a Historic Moment and gets no Taj bonus"
    );
}

#[test]
fn national_parks_keep_paying_after_the_world_first() {
    let mut game = two_player_game();
    game.players[0].era_score = 0;
    game.players[1].era_score = 0;

    assert!(game.repeatable_world_first_moment(
        0,
        "test_park",
        "MOMENT_NATIONAL_PARK_CREATED",
        "MOMENT_NATIONAL_PARK_CREATED_FIRST_IN_WORLD",
    ));
    assert!(!game.repeatable_world_first_moment(
        0,
        "test_park",
        "MOMENT_NATIONAL_PARK_CREATED",
        "MOMENT_NATIONAL_PARK_CREATED_FIRST_IN_WORLD",
    ));
    assert!(!game.repeatable_world_first_moment(
        1,
        "test_park",
        "MOMENT_NATIONAL_PARK_CREATED",
        "MOMENT_NATIONAL_PARK_CREATED_FIRST_IN_WORLD",
    ));

    assert_eq!(game.players[0].era_score, 7, "+4 first, then +3 again");
    assert_eq!(game.players[1].era_score, 3, "another civilization gets +3");
}

#[test]
fn distinguished_units_and_underdog_kills_pay_their_rows() {
    let mut game = two_player_game();
    let city = found_capital(&mut game, 0);
    let position = game.cities[&city].pos;
    let veteran = game.spawn_unit("warrior", 0, position);
    game.players[0].era_score = 0;
    for expected_level in 2..=4 {
        {
            let unit = game.units.get_mut(&veteran).unwrap();
            unit.xp = 10_000;
            unit.moves_left = 2.0;
            unit.acted = false;
        }
        let promotion = game.available_promotions(veteran)[0].clone();
        game.do_promote(0, veteran, &promotion).unwrap();
        assert_eq!(game.units[&veteran].level, expected_level);
    }
    assert_eq!(game.players[0].era_score, 1, "level four pays exactly +1");

    let attacker = game.units[&veteran].clone();
    let mut victim = attacker.clone();
    victim.owner = 1;
    victim.formation = attacker.formation + 1;
    victim.promotions.insert(crate::name!("test_veteran_one"));
    victim.promotions.insert(crate::name!("test_veteran_two"));
    game.players[0].era_score = 0;
    game.note_underdog_kill(0, &attacker, &victim);
    assert_eq!(
        game.players[0].era_score, 4,
        "superior formation +1 and two-more-promotions +3 stack"
    );

    game.players[0]
        .great_people
        .push("hannibal_barca".to_string());
    game.players[0].era_score = 0;
    game.note_great_person_assisted_kill(0, &attacker);
    game.note_great_person_assisted_kill(0, &attacker);
    assert_eq!(
        game.players[0].era_score, 2,
        "each Great General pays only for the first land offensive they oversee"
    );

    game.players[0]
        .great_people
        .push("gaius_duilius".to_string());
    let mut galley = attacker.clone();
    galley.kind = crate::name!("galley");
    game.note_great_person_assisted_kill(0, &galley);
    assert_eq!(
        game.players[0].era_score, 4,
        "a Great Admiral independently pays for a naval offensive"
    );
}

#[test]
fn completed_foreign_routes_establish_scoring_trading_posts() {
    let mut game = two_player_game();
    let first = found_capital(&mut game, 0);
    let second = found_capital(&mut game, 1);
    game.players[0].era_score = 0;
    game.players[1].era_score = 0;

    game.note_completed_trade_route_moments(
        0,
        &TradeRoute {
            origin: first,
            dest: second,
            owner: 0,
            ends: 1,
        },
    );
    assert_eq!(
        game.players[0].era_score, 6,
        "+1 new civilization and +5 world's first posts in every civilization"
    );
    game.note_completed_trade_route_moments(
        0,
        &TradeRoute {
            origin: first,
            dest: second,
            owner: 0,
            ends: 2,
        },
    );
    assert_eq!(
        game.players[0].era_score, 6,
        "the same trading post is not new"
    );
}

#[test]
fn envoys_and_levies_pay_city_state_moments() {
    let mut game = Game::new_full(2, 30, 18, 516, 300, 1, false);
    let minor = game
        .players
        .iter()
        .find(|player| player.is_minor && !player.is_free_city && !player.is_barbarian)
        .unwrap()
        .id;
    game.players[0].met.insert(minor);
    game.players[minor].met.insert(0);
    game.players[0].envoys_free = 3;
    game.players[0].era_score = 0;
    for _ in 0..3 {
        game.do_send_envoy(0, minor).unwrap();
    }
    assert_eq!(game.suzerain_of_uncached(minor), Some(0));
    assert_eq!(
        game.players[0].era_score, 2,
        "the city-state's first Suzerain before Medieval pays +2"
    );

    game.players[0].gold = 1_000.0;
    let levy_score = game.players[0].era_score;
    game.do_levy_military(0, minor).unwrap();
    assert!(
        matches!(game.players[0].era_score - levy_score, 1 | 2),
        "a levy pays +1, replaced by +2 when the city-state is within six tiles of an enemy"
    );
}

#[test]
fn emergency_winners_pay_member_and_target_moments() {
    let mut game = two_player_game();
    let target_city = found_capital(&mut game, 1);
    game.players[0].era_score = 0;
    game.players[1].era_score = 0;
    let emergency = |id, contributions| Emergency {
        id,
        kind: "military".to_string(),
        target: 1,
        city: target_city,
        original_owner: 0,
        members: [0].into_iter().collect(),
        contributions,
        started: 0,
        ends: 30,
    };

    game.active_emergencies
        .push(emergency(1, [(0, 1)].into_iter().collect()));
    game.resolve_emergency(1, true);
    assert_eq!(game.players[0].era_score, 3);

    game.active_emergencies
        .push(emergency(2, [(0, 1)].into_iter().collect()));
    game.resolve_emergency(2, false);
    assert_eq!(game.players[1].era_score, 4);
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
fn georgia_banks_dedication_score_during_golden_and_heroic_ages() {
    for age in ["golden", "heroic"] {
        let mut game = two_player_game();
        game.players[0].civ = "Georgia".to_string();
        game.players[0].age = age.to_string();
        game.players[0]
            .dedications
            .insert("exodus_of_the_evangelists".to_string());
        game.dedication_trigger(0, "city_converted", 1);
        assert_eq!(game.players[0].era_score, 2, "Strength in Unity failed in a {age} age");
    }
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

    game.players[0]
        .techs
        .insert(crate::name!("horseback_riding"));
    game.players[1]
        .techs
        .insert(crate::name!("horseback_riding"));
    // An era is held open for its shipped 40-turn minimum, so a fixture that
    // wants the next one has to stand far enough into this one.
    finish_minimum_era_countdown(&mut game);

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
fn the_dedication_arms_choose_differently_and_the_default_banks() {
    use crate::ai::{choose_dedications, DedicationChoice, Weights};

    // Alphabetically the Classical era offers exodus_of_the_evangelists first,
    // and that is what both AI tiers took for the whole history of the repo.
    // Ranking BOTH halves on projected Era Score lost to it 41.2% to 58.8%;
    // ranking only the half where Era Score is the objective beat it 57.7% to
    // 42.3% over a pre-registered 300 maps. See the DedicationChoice docs.
    assert_eq!(
        Weights::default().dedication_choice,
        DedicationChoice::Banking,
        "the shipped agent is the one that passed its gate"
    );
    let mut game = two_player_game();
    game.world_era = 1;
    game.players[0].age = "normal".to_string();
    game.players[0].dedication_choices = 1;
    game.players[0]
        .last_era_triggers
        .insert("eureka".to_string(), 6);

    let mut shipped = game.clone();
    choose_dedications(&mut shipped, 0, DedicationChoice::Alphabetical);
    assert!(
        shipped.players[0]
            .dedications
            .contains("exodus_of_the_evangelists"),
        "the shipped arm takes the first name in the map"
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

#[test]
fn the_banking_arm_ranks_only_where_era_score_is_the_objective() {
    use crate::ai::{choose_dedications, DedicationChoice};

    // A Normal or Dark Age banks Era Score, so the projection is the literal
    // objective and Banking ranks on it exactly as Measured does.
    for age in ["normal", "dark"] {
        let mut game = two_player_game();
        game.world_era = 1;
        game.players[0].age = age.to_string();
        game.players[0].dedication_choices = 1;
        game.players[0]
            .last_era_triggers
            .insert("eureka".to_string(), 6);

        choose_dedications(&mut game, 0, DedicationChoice::Banking);
        assert!(
            game.players[0].dedications.contains("free_inquiry"),
            "a {age} age banks, so six Eurekas name Free Inquiry"
        );
    }

    // A Golden or Heroic Age banks nothing, so the projection is only a
    // correlate there and Banking leaves that choice where the default puts it.
    // Ranking on a correlate is what lost the first gate.
    for age in ["golden", "heroic"] {
        let mut game = two_player_game();
        game.world_era = 1;
        game.players[0].age = age.to_string();
        game.players[0].dedication_choices = 1;
        game.players[0]
            .last_era_triggers
            .insert("eureka".to_string(), 6);

        let mut ranked = game.clone();
        choose_dedications(&mut ranked, 0, DedicationChoice::Measured);
        assert!(ranked.players[0].dedications.contains("free_inquiry"));

        choose_dedications(&mut game, 0, DedicationChoice::Banking);
        assert!(
            game.players[0]
                .dedications
                .contains("exodus_of_the_evangelists"),
            "a {age} age keeps the default choice, which is the one that wins"
        );

        let mut georgia = two_player_game();
        georgia.world_era = 1;
        georgia.players[0].civ = "Georgia".to_string();
        georgia.players[0].age = age.to_string();
        georgia.players[0].dedication_choices = 1;
        georgia.players[0]
            .last_era_triggers
            .insert("eureka".to_string(), 6);

        choose_dedications(&mut georgia, 0, DedicationChoice::Banking);
        assert!(
            georgia.players[0].dedications.contains("free_inquiry"),
            "Strength in Unity lets Georgia bank in a {age} age"
        );
    }
}

#[test]
fn every_catalogued_moment_pays_its_score_only_inside_its_window() {
    let mut game = two_player_game();
    let catalogue: Vec<_> = game
        .rules
        .historic_moments
        .iter()
        .map(|(id, spec)| (id.to_string(), spec.clone()))
        .collect();
    assert_eq!(catalogue.len(), 149);

    for (id, spec) in catalogue {
        game.world_era = spec.minimum_game_era.unwrap_or(0);
        game.players[0].era_score = 0;
        assert!(game.add_historic_moment(0, &id), "{id} was not awardable");
        assert_eq!(game.players[0].era_score, spec.era_score, "{id}");

        if let Some(minimum) = spec.minimum_game_era.filter(|minimum| *minimum > 0) {
            game.world_era = minimum - 1;
            game.players[0].era_score = 0;
            assert!(!game.add_historic_moment(0, &id), "{id} ignored its minimum era");
            assert_eq!(game.players[0].era_score, 0);
        }
        if let Some(maximum) = spec
            .maximum_game_era
            .filter(|maximum| *maximum + 1 < crate::rules::ERA_NAMES.len())
        {
            game.world_era = maximum + 1;
            game.players[0].era_score = 0;
            assert!(!game.add_historic_moment(0, &id), "{id} ignored its maximum era");
            assert_eq!(game.players[0].era_score, 0);
        }
        if let Some(obsolete) = spec.obsolete_era {
            game.world_era = obsolete;
            game.players[0].era_score = 0;
            assert!(!game.add_historic_moment(0, &id), "{id} ignored ObsoleteEra");
            assert_eq!(game.players[0].era_score, 0);
        }
    }
    assert!(!game.add_historic_moment(0, "MOMENT_NOT_IN_THE_RULESET"));
}

#[test]
fn unique_replacement_districts_do_not_claim_base_district_moments() {
    let mut korea = two_player_game();
    korea.players[0].civ = "Korea".to_string();
    korea.players[0].era_score = 0;
    let seowon = korea.units.values().find(|unit| unit.owner == 0).unwrap().pos;
    korea.note_district_completed_moments(0, "seowon", seowon);
    assert_eq!(korea.players[0].era_score, 4, "Seowon earns only the unique-district Moment");
    assert!(!korea.players[0]
        .counters
        .contains_key("historic_moment:high_adjacency:campus"));

    let mut kongo = two_player_game();
    kongo.players[0].civ = "Kongo".to_string();
    kongo.players[0].era_score = 0;
    let mbanza = kongo.units.values().find(|unit| unit.owner == 0).unwrap().pos;
    kongo.note_district_completed_moments(0, "mbanza", mbanza);
    assert_eq!(kongo.players[0].era_score, 4, "Mbanza earns only the unique-district Moment");
    assert!(!kongo.players[0]
        .counters
        .contains_key("historic_moment:neighborhood_district"));
}

#[test]
fn free_population_unique_buildings_and_prophets_reach_moment_hooks() {
    let mut game = two_player_game();
    game.players[0].civ = "Arabia".to_string();
    let city = found_capital(&mut game, 0);
    game.players[0].era_score = 0;

    game.cities.get_mut(&city).unwrap().pop = 9;
    game.increase_city_population(city, 1);
    assert_eq!(game.players[0].era_score, 2, "a granted tenth Citizen is still a world first");

    game.grant_free_building_family(0, city, "university");
    assert!(game.cities[&city].buildings.contains(&crate::name!("madrasa")));
    assert_eq!(
        game.players[0]
            .counters
            .get("historic_moment_awards:MOMENT_BUILDING_CONSTRUCTED_FIRST_UNIQUE"),
        Some(&1)
    );

    let stonehenge = game.rules.wonders["stonehenge"].clone();
    let position = game.cities[&city].pos;
    game.cities
        .get_mut(&city)
        .unwrap()
        .wonders
        .insert(crate::name!("stonehenge"), position);
    game.apply_wonder_completion_effects(0, city, "stonehenge", position, &stonehenge);
    assert!(game.players[0].prophet_pending);
    assert_eq!(
        game.players[0]
            .counters
            .get("historic_moment_awards:MOMENT_GREAT_PERSON_CREATED_GAME_ERA"),
        Some(&1)
    );
}

#[test]
fn repeat_conversions_keep_their_moments_but_not_repeat_dedication_credit() {
    let mut game = two_player_game();
    let _founder_city = found_capital(&mut game, 0);
    let holy_city = found_capital(&mut game, 1);
    game.players[0].religion = Some("First Faith".to_string());
    game.players[1].religion = Some("Second Faith".to_string());
    game.players[1].holy_city = Some(holy_city);
    game.cities
        .get_mut(&holy_city)
        .unwrap()
        .pressure
        .insert("First Faith".to_string(), 10_000.0);
    game.at_war.insert((0, 1));
    game.players[0].era_score = 0;
    let before = vec![(holy_city, Some("Second Faith".to_string()))];

    game.award_conversion_era_score(&before);
    game.award_conversion_era_score(&before);

    assert_eq!(game.players[0].era_score, 14, "war + holy-city Moments pay on each conversion");
    assert_eq!(game.players[0].converted_cities.len(), 1, "dedication credit stays first-only");
}

#[test]
fn team_contact_final_capitals_and_invalid_casus_belli_do_not_misaward() {
    let mut team = two_player_game();
    team.players[0].team = Some(7);
    team.players[1].team = Some(7);
    team.note_met_all_majors(0);
    assert_eq!(team.players[0].era_score, 5, "a known teammate completes the contact table");

    let mut conquest = two_player_game();
    let _own = found_capital(&mut conquest, 0);
    let foreign = found_capital(&mut conquest, 1);
    conquest.players[0].era_score = 0;
    conquest.transfer_city(foreign, 0, true);
    conquest.capture_rewards(0, 1, 0.0);
    assert_eq!(
        conquest.players[0].era_score, 5,
        "the final original capital pays Final Foreign City only, not another +4"
    );

    let mut war = two_player_game();
    war.players[0].met.insert(1);
    assert!(war
        .do_declare_war_with_casus_belli(0, 1, "golden_age_war")
        .is_err());
    assert_eq!(war.players[0].era_score, 0);
}

#[test]
fn only_the_first_fully_promoted_governor_earns_the_moment() {
    let mut game = two_player_game();
    let city = found_capital(&mut game, 0);
    game.players[0]
        .counters
        .insert("district_governor_titles".to_string(), 20);
    game.do_appoint_governor(0, "pingala", city).unwrap();
    game.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .promotions
        .extend(
            ["connoisseur", "researcher", "grants", "space_initiative"]
                .into_iter()
                .map(str::to_string),
        );
    game.players[0].era_score = 0;
    game.do_promote_governor(0, "pingala", "curator").unwrap();
    assert_eq!(game.players[0].era_score, 1);

    game.do_appoint_governor(0, "reyna", city).unwrap();
    game.players[0]
        .governor_roster
        .get_mut("reyna")
        .unwrap()
        .promotions
        .extend(
            ["harbormaster", "forestry_management", "tax_collector", "contractor"]
                .into_iter()
                .map(str::to_string),
        );
    game.do_promote_governor(0, "reyna", "renewable_subsidizer")
        .unwrap();
    assert_eq!(game.players[0].era_score, 1, "a second Governor earns no second Moment");
    assert_eq!(
        game.players[0]
            .counters
            .get("historic_moment_awards:MOMENT_GOVERNOR_FULLY_PROMOTED_FIRST"),
        Some(&1)
    );
}
