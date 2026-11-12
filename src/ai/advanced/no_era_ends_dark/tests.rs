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
/// worth four. Off, `Banking` breaks the tie by name and takes Free Inquiry,
/// and so does the gene when the era is on course. In a Golden Age the gene
/// leaves the choice to the caller.
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

    // On course for a Normal Age the budget schedules nothing, so the record
    // alone ranks and the tie goes by name, as off.
    let mut on_course = game.clone();
    on_course.players[0].era_score = 45;
    assert!(seat(true).choose_era_dedication(&mut on_course, 0, &plan));
    assert!(on_course.players[0].dedications.contains("free_inquiry"));
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

#[test]
fn score_seat_banks_era_score_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "score-seat-banks-era-score",
        |ai| ai.score_seat_banks_era_score,
    );
}

fn score_seat(target: VictoryTarget) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_score_seat_banks_era_score();
    ai
}

/// On course for a Normal Age, `no-era-ends-dark` schedules nothing, but a
/// Score seat still takes a clause building worth its Production: one point
/// of Era Score and one building point for a 60-Production Monument. On
/// another target the Score gene does nothing.
#[test]
fn a_score_seat_schedules_the_clause_building_on_course() {
    let (mut game, cities) = medieval_board();
    let plan = quiet_plan(&game);
    game.players[0].era_score = 47;
    game.players[0]
        .dedications
        .insert("pen_brush_and_voice".to_string());
    for cid in &cities {
        game.cities.get_mut(cid).unwrap().queue = vec![building("granary")];
    }
    let granaries = heads(&game);

    let mut dark = game.clone();
    seat(true).claim_era_moments(&mut dark, 0, &plan);
    assert_eq!(heads(&dark), granaries, "no-era-ends-dark on course");

    let mut domination = game.clone();
    score_seat(VictoryTarget::Domination).claim_era_moments(&mut domination, 0, &plan);
    assert_eq!(heads(&domination), granaries, "another target");
    assert_eq!(
        score_seat(VictoryTarget::Domination).era_budget(&game, 0),
        None
    );

    let ai = score_seat(VictoryTarget::Score);
    assert!(ai.era_budget(&game, 0).is_some_and(|budget| budget.score));
    let mut score = game.clone();
    ai.claim_era_moments(&mut score, 0, &plan);
    assert_eq!(
        heads(&score)
            .into_iter()
            .filter(|head| *head == Some(monument()))
            .count(),
        1,
        "a Score seat takes one city a turn"
    );

    // A Golden Age pays no clause: the Score seat takes nothing.
    let mut golden = game.clone();
    golden.players[0].age = "golden".to_string();
    ai.claim_era_moments(&mut golden, 0, &plan);
    assert_eq!(heads(&golden), granaries, "a Golden Age");
}

/// A Great Person patronised for more than half its cost is worth the
/// +3 Moment and five score; a near recruit the +1 Moment and five score.
/// The Faith reserve is spent in the last turns before the limit.
#[test]
fn a_great_person_and_the_last_turns_are_priced_in_score() {
    let (mut game, _) = medieval_board();
    let kind = "scientist";
    let cost = game.gp_cost(0, kind);
    game.players[0].gpp.insert(kind.to_string(), 0.0);
    assert_eq!(AdvancedAi::score_patronage_points(&game, 0, kind), 8.0);
    game.players[0].gpp.insert(kind.to_string(), cost * 0.9);
    assert_eq!(AdvancedAi::score_patronage_points(&game, 0, kind), 6.0);

    let last = game.standard_duration(SCORE_FINAL_STANDARD);
    game.max_turns = 170;
    game.turn = 170 - last - 1;
    assert!(!AdvancedAi::score_final_turns(&game));
    game.turn = 170 - last;
    assert!(AdvancedAi::score_final_turns(&game));
    game.max_turns = 0;
    assert!(
        !AdvancedAi::score_final_turns(&game),
        "no limit, no last turns"
    );
}

/// A Score seat's Trader opens a route to a rival in which it holds no
/// Trading Post; once the post stands, the ordinary chooser decides.
#[test]
fn a_score_seat_routes_a_trader_to_a_rival_without_our_post() {
    let (mut game, cities) = medieval_board();
    let homes: Vec<Pos> = cities.iter().map(|cid| game.cities[cid].pos).collect();
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            !game.rules.is_water(tile)
                && homes
                    .iter()
                    .all(|home| (5..=10).contains(&game.wdist(**pos, *home)))
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    let rival = game.place_city(1, *sites.last().expect("fixture: a rival site"), None);
    game.players[1].alive = true;
    let origin = cities[0];
    assert!(
        game.can_establish_trade_route(0, origin, rival),
        "fixture: a legal route to the rival"
    );
    let strategy = GrandStrategy::Expansion;
    assert_eq!(
        seat(true).score_route_claim(&game, 0, origin, strategy),
        None,
        "off"
    );
    assert_eq!(
        score_seat(VictoryTarget::Domination).score_route_claim(&game, 0, origin, strategy),
        None,
        "another target"
    );
    let ai = score_seat(VictoryTarget::Score);
    assert_eq!(
        ai.score_route_claim(&game, 0, origin, strategy)
            .map(|(_, city)| city),
        Some(rival)
    );
    game.players[0]
        .counters
        .insert("trading_post_in:1".to_string(), 1);
    assert_eq!(ai.score_route_claim(&game, 0, origin, strategy), None);
}
