use super::*;

#[test]
fn no_era_ends_dark_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers("no-era-ends-dark", |ai| {
        ai.no_era_ends_dark
    });
}

/// Two majors, every unit removed: our capital and a second city, two
/// citizens each, on the Medieval era's first turn (turn 30), in a Normal
/// Age with 32 Era Score against a Normal threshold of 45. 50 Production is
/// banked on each city's Monument so it finishes inside the cap.
fn medieval_board() -> (Game, Vec<u32>) {
    let mut game = Game::new_full(2, 32, 20, 936_431, 200, 0, false);
    // Rome's Trajan's Column founds every city with a Monument.
    game.players[0].civ = "Gran Colombia".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for pid in 0..2 {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=8).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(0, *sites.first().expect("fixture: a second site"), None);
    let cities = game.player_city_ids(0);
    assert_eq!(cities.len(), 2, "fixture: two cities");
    game.turn = 30;
    game.current = 0;
    game.world_era = 2;
    game.world_era_since = 30;
    game.world_era_countdown_end = None;
    let player = &mut game.players[0];
    player.age = "normal".to_string();
    player.era_score = 32;
    player.era_score_baseline = 32;
    player.normal_age_threshold = 45;
    player.golden_age_threshold = 56;
    player.dedications.clear();
    player.dedication_choices = 0;
    let monument = monument();
    for cid in &cities {
        let city = game.cities.get_mut(cid).unwrap();
        city.pop = 2;
        city.queue.clear();
        city.production_progress
            .insert(Game::item_progress_key(&monument), 50.0);
        assert!(
            game.can_produce(0, *cid, &monument),
            "fixture: every city can build its Monument"
        );
    }
    (game, cities)
}

fn monument() -> Item {
    building("monument")
}

fn building(name: &str) -> Item {
    Item::Building {
        building: crate::name::Name::new(name),
    }
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(name),
    }
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    if gene {
        ai.enable_no_era_ends_dark();
    }
    ai
}

fn quiet_plan(game: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    }
}

fn heads(game: &Game) -> Vec<Option<Item>> {
    game.player_city_ids(0)
        .into_iter()
        .map(|cid| game.cities[&cid].queue.first().cloned())
        .collect()
}

/// The target is the Normal threshold plus the margin; the usual pace and a
/// paying building at a queue head count before the shortfall. A Golden Age
/// banks nothing; off, there is no budget.
#[test]
fn the_budget_counts_the_pace_and_the_paying_heads() {
    let (mut game, cities) = medieval_board();
    assert_eq!(seat(false).era_budget(&game, 0), None, "off");
    let ai = seat(true);
    let budget = ai.era_budget(&game, 0).expect("a Medieval budget");
    assert_eq!(budget.start, 30);
    assert_eq!(budget.target, 45 + ERA_BUDGET_MARGIN);
    assert_eq!(budget.turns_left, budget.end - 30);
    assert!((budget.passive - ERA_PASSIVE_PER_ERA).abs() < 1e-9);
    assert_eq!(budget.in_hand, 0);
    assert_eq!(budget.short, 47 - 32 - 9);
    assert!(budget.banks);

    // Under Pen, Brush and Voice a Monument at a queue head is in hand.
    game.players[0]
        .dedications
        .insert("pen_brush_and_voice".to_string());
    game.cities.get_mut(&cities[0]).unwrap().queue = vec![monument()];
    let budget = ai.era_budget(&game, 0).unwrap();
    assert_eq!(budget.in_hand, 1);
    assert_eq!(budget.short, 47 - 32 - 9 - 1);

    // A Golden Age's Dedication banks nothing, so nothing is in hand.
    game.players[0].age = "golden".to_string();
    let budget = ai.era_budget(&game, 0).unwrap();
    assert!(!budget.banks);
    assert_eq!(budget.in_hand, 0);
}

/// A mirrored board (no `world_era_since`) reads its
/// era off the Online schedule; the host's countdown wins once it runs.
#[test]
fn a_mirrored_era_reads_the_schedule_until_the_countdown_runs() {
    let (mut game, _) = medieval_board();
    game.world_era_since = 0;
    let first = game.standard_duration(FIRST_ERA_STANDARD);
    let later = game.standard_duration(LATER_ERA_STANDARD);
    assert_eq!(
        AdvancedAi::era_window(&game),
        (first + later, first + 2 * later)
    );
    let late = first + 2 * later + 3;
    game.world_era_countdown_end = Some(late);
    assert_eq!(AdvancedAi::era_window(&game), (first + later, late));
    // An era the world reached early starts a whole era before its countdown.
    let early = first + later - 2;
    game.world_era_countdown_end = Some(early);
    assert_eq!(AdvancedAi::era_window(&game), (early - later, early));
}

/// A tied record (two Eurekas against two Inspirations) goes to the clause
/// the era can build for: two Monument-less cities make Pen, Brush and Voice
/// worth four. Off, `Banking` breaks the tie by name and takes Free Inquiry.
/// In a Golden Age the gene leaves the choice to the caller.
#[test]
fn the_dedication_follows_what_the_era_can_build() {
    let (mut game, _) = medieval_board();
    let plan = quiet_plan(&game);
    {
        let player = &mut game.players[0];
        player.dedication_choices = 1;
        player.last_era_triggers.insert("eureka".to_string(), 2);
        player
            .last_era_triggers
            .insert("inspiration".to_string(), 2);
    }

    let mut off = game.clone();
    assert!(!seat(false).choose_era_dedication(&mut off, 0, &plan));
    crate::ai::choose_dedications(&mut off, 0, crate::ai::DedicationChoice::Banking);
    assert!(off.players[0].dedications.contains("free_inquiry"));

    let mut on = game.clone();
    assert!(seat(true).choose_era_dedication(&mut on, 0, &plan));
    assert!(on.players[0].dedications.contains("pen_brush_and_voice"));
    assert_eq!(on.players[0].dedication_choices, 0);

    let mut golden = game.clone();
    golden.players[0].age = "golden".to_string();
    assert!(!seat(true).choose_era_dedication(&mut golden, 0, &plan));
    assert!(golden.players[0].dedications.is_empty());
}

/// Short of its target under Pen, Brush and Voice, one city a turn puts its
/// Monument ahead of a Granary, the cheapest first; a Settler keeps its
/// queue. Off, in a Golden Age, or on course, nothing moves.
#[test]
fn a_short_era_schedules_the_clauses_cheapest_building() {
    let (mut game, cities) = medieval_board();
    let plan = quiet_plan(&game);
    game.players[0]
        .dedications
        .insert("pen_brush_and_voice".to_string());
    for cid in &cities {
        game.cities.get_mut(cid).unwrap().queue = vec![building("granary")];
    }
    let granaries = heads(&game);

    let mut off = game.clone();
    seat(false).claim_era_moments(&mut off, 0, &plan);
    assert_eq!(heads(&off), granaries, "off");

    let mut golden = game.clone();
    golden.players[0].age = "golden".to_string();
    seat(true).claim_era_moments(&mut golden, 0, &plan);
    assert_eq!(heads(&golden), granaries, "a Golden Age");

    let mut on_course = game.clone();
    on_course.players[0].era_score = 45;
    seat(true).claim_era_moments(&mut on_course, 0, &plan);
    assert_eq!(heads(&on_course), granaries, "on course");

    let ai = seat(true);
    let mut on = game.clone();
    ai.claim_era_moments(&mut on, 0, &plan);
    let claimed = heads(&on)
        .into_iter()
        .filter(|head| *head == Some(monument()))
        .count();
    assert_eq!(claimed, 1, "one city a turn");
    ai.claim_era_moments(&mut on, 0, &plan);
    assert_eq!(
        heads(&on),
        vec![Some(monument()); 2],
        "the next turn's city"
    );

    let mut settlers = game.clone();
    for cid in &cities {
        settlers.cities.get_mut(cid).unwrap().queue = vec![unit("settler")];
    }
    seat(true).claim_era_moments(&mut settlers, 0, &plan);
    assert_eq!(heads(&settlers), vec![Some(unit("settler")); 2], "Settlers");
}

/// Two paying buildings at queue heads are the cap: with both first cities
/// on their Monuments, a third city keeps its Granary.
#[test]
fn two_paying_heads_are_the_cap() {
    let (mut game, cities) = medieval_board();
    let plan = quiet_plan(&game);
    let homes: Vec<Pos> = cities.iter().map(|cid| game.cities[cid].pos).collect();
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            !game.rules.is_water(tile)
                && homes
                    .iter()
                    .all(|home| (5..=9).contains(&game.wdist(**pos, *home)))
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    let third = game.place_city(0, *sites.first().expect("fixture: a third site"), None);
    // Founding may award its own Moments; the board stays at 32.
    game.players[0].era_score = 32;
    {
        let city = game.cities.get_mut(&third).unwrap();
        city.pop = 2;
        city.queue = vec![building("granary")];
        city.production_progress
            .insert(Game::item_progress_key(&monument()), 50.0);
    }
    assert!(
        game.can_produce(0, third, &monument()),
        "fixture: a third Monument"
    );
    game.players[0]
        .dedications
        .insert("pen_brush_and_voice".to_string());
    for cid in &cities {
        game.cities.get_mut(cid).unwrap().queue = vec![monument()];
    }
    let ai = seat(true);
    let budget = ai.era_budget(&game, 0).unwrap();
    assert_eq!(budget.in_hand, 2);
    assert!(budget.short > 0, "still short with both in hand");
    let mut after = game.clone();
    ai.claim_era_moments(&mut after, 0, &plan);
    assert_eq!(
        after.cities[&third].queue.first(),
        Some(&building("granary")),
        "the cap holds the third city"
    );
    // One paying head fewer, and the third city takes its Monument.
    let mut freed = game.clone();
    freed.cities.get_mut(&cities[0]).unwrap().queue = vec![building("granary")];
    ai.claim_era_moments(&mut freed, 0, &plan);
    assert!(
        heads(&freed)
            .iter()
            .filter(|head| **head == Some(monument()))
            .count()
            >= 2,
        "below the cap a city is claimed"
    );
}
