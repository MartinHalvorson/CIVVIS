use super::*;

#[test]
fn domination_seat_raises_its_culture_is_opt_in() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "domination-seat-raises-its-culture",
        |ai| ai.domination_seat_raises_its_culture,
    );
}

/// Two majors, every unit removed: our capital and a second city, two
/// citizens each, turn 30, and 50 Production banked on each city's Monument
/// so it finishes inside the cap. Returns the game, the two cities and the
/// Monument item.
fn two_cities() -> (Game, Vec<u32>, Item) {
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
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
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
    (game, cities, monument)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_domination_seat_raises_its_culture();
    }
    ai
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(name),
    }
}

fn building(name: &str) -> Item {
    Item::Building {
        building: crate::name::Name::new(name),
    }
}

fn district(game: &Game, cid: u32, name: &str) -> Item {
    let home = game.cities[&cid].pos;
    let pos = game
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| game.wdist(*pos, home) == 1)
        .min()
        .expect("fixture: a neighbouring tile");
    Item::District {
        district: crate::name::Name::new(name),
        pos,
    }
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

/// `heads` at the head of the two cities' queues; the claim runs once.
fn claim_over(ai: &AdvancedAi, game: &Game, plan: &StrategicPlan, heads: [Item; 2]) -> Game {
    let (cities, mut board) = (game.player_city_ids(0), game.clone());
    for (cid, head) in cities.iter().zip(heads) {
        board.cities.get_mut(cid).unwrap().queue = vec![head];
    }
    ai.claim_domination_monuments(&mut board, 0, plan);
    board
}

/// At peace, a Domination seat's Monument goes ahead of a routine soldier
/// and a Granary and the claim holds; off, or for a Culture seat, nothing
/// moves.
#[test]
fn a_domination_seat_puts_its_monument_ahead_of_a_soldier_and_a_granary() {
    let (game, cities, monument) = two_cities();
    let plan = quiet_plan(&game);
    let heads = [unit("warrior"), building("granary")];
    for (ai, why) in [
        (seat(VictoryTarget::Domination, false), "off"),
        (seat(VictoryTarget::Culture, true), "a Culture seat"),
    ] {
        let kept = claim_over(&ai, &game, &plan, heads.clone());
        for (cid, head) in cities.iter().zip(&heads) {
            assert_eq!(kept.cities[cid].queue.first(), Some(head), "{why}");
        }
        assert!(!ai.domination_monument_holds(&kept, 0, cities[0], &plan, &monument));
    }
    let ai = seat(VictoryTarget::Domination, true);
    let on = claim_over(&ai, &game, &plan, heads);
    for cid in &cities {
        assert_eq!(on.cities[cid].queue.first(), Some(&monument));
        assert!(ai.domination_monument_holds(&on, 0, *cid, &plan, &monument));
    }
}

/// An Aqueduct waits for the Monument too.
#[test]
fn an_aqueduct_waits_for_the_monument() {
    let (game, cities, monument) = two_cities();
    let plan = quiet_plan(&game);
    let ai = seat(VictoryTarget::Domination, true);
    let aqueduct = district(&game, cities[0], "aqueduct");
    let on = claim_over(&ai, &game, &plan, [aqueduct, unit("warrior")]);
    assert_eq!(on.cities[&cities[0]].queue.first(), Some(&monument));
}

/// A Settler, a Builder, a Trader, Walls, a Campus, a Library, a Commercial
/// Hub, a Holy Site, a siege gun and a soldier finishing this turn all keep
/// their queue.
#[test]
fn the_kept_heads_keep_their_queue() {
    let (game, cities, _) = two_cities();
    let plan = quiet_plan(&game);
    let ai = seat(VictoryTarget::Domination, true);
    let mut finishing = game.clone();
    let warrior = unit("warrior");
    let cost = finishing.item_cost_for_city(0, cities[1], &warrior);
    finishing
        .cities
        .get_mut(&cities[1])
        .unwrap()
        .production_progress
        .insert(Game::item_progress_key(&warrior), cost - 0.01);
    let kept = claim_over(&ai, &finishing, &plan, [unit("settler"), warrior.clone()]);
    assert_eq!(
        kept.cities[&cities[0]].queue.first(),
        Some(&unit("settler"))
    );
    assert_eq!(
        kept.cities[&cities[1]].queue.first(),
        Some(&warrior),
        "a soldier finishing this turn keeps its queue"
    );
    let pairs = [
        [unit("builder"), unit("trader")],
        [building("walls"), building("library")],
        [
            district(&game, cities[0], "campus"),
            district(&game, cities[1], "commercial_hub"),
        ],
        [
            district(&game, cities[0], "holy_site"),
            unit("battering_ram"),
        ],
    ];
    for heads in pairs {
        let kept = claim_over(&ai, &game, &plan, heads.clone());
        for (cid, head) in cities.iter().zip(&heads) {
            assert_eq!(kept.cities[cid].queue.first(), Some(head), "{head:?}");
        }
    }
}

/// A city of one citizen, the plan's threatened city and a city attacked
/// this turn claim nothing.
#[test]
fn small_threatened_and_attacked_cities_claim_nothing() {
    let (mut game, cities, _) = two_cities();
    game.cities.get_mut(&cities[0]).unwrap().pop = 1;
    game.cities.get_mut(&cities[1]).unwrap().last_attacked = game.turn;
    let ai = seat(VictoryTarget::Domination, true);
    let plan = quiet_plan(&game);
    let kept = claim_over(&ai, &game, &plan, [unit("warrior"), unit("warrior")]);
    for cid in &cities {
        assert_eq!(kept.cities[cid].queue.first(), Some(&unit("warrior")));
    }
    let (game, cities, _) = two_cities();
    let mut threatened = quiet_plan(&game);
    threatened.threatened_city = Some(cities[0]);
    let kept = claim_over(&ai, &game, &threatened, [unit("warrior"), unit("warrior")]);
    assert_eq!(
        kept.cities[&cities[0]].queue.first(),
        Some(&unit("warrior"))
    );
    assert!(ai
        .domination_monument_item(&kept, 0, cities[0], &threatened)
        .is_none());
}

/// At war with a major and with no hostile near, a soldier keeps its queue
/// but a Granary still waits for the Monument.
#[test]
fn at_war_a_soldier_keeps_its_queue_and_a_granary_waits() {
    let (mut game, cities, monument) = two_cities();
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    let plan = quiet_plan(&game);
    let ai = seat(VictoryTarget::Domination, true);
    let on = claim_over(&ai, &game, &plan, [unit("warrior"), building("granary")]);
    assert_eq!(on.cities[&cities[0]].queue.first(), Some(&unit("warrior")));
    assert_eq!(on.cities[&cities[1]].queue.first(), Some(&monument));
}

/// An idle queue takes the Monument unless a Settler is due there.
#[test]
fn an_idle_queue_takes_the_monument_unless_a_settler_is_due() {
    let (game, cities, monument) = two_cities();
    let plan = quiet_plan(&game);
    let ai = seat(VictoryTarget::Domination, true);
    let mut idle = game.clone();
    ai.claim_domination_monuments(&mut idle, 0, &plan);
    let settlers = ai.counts(&game, 0).settlers;
    for cid in &cities {
        let due = ai.base.settler_due(&game, 0, *cid, cities.len(), settlers);
        let expected = (!due).then_some(&monument);
        assert_eq!(
            idle.cities[cid].queue.first(),
            expected,
            "settler due {due}"
        );
    }
}

/// A built Monument is not claimed again.
#[test]
fn a_built_monument_is_not_claimed_again() {
    let (mut game, cities, _) = two_cities();
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("monument"));
    let ai = seat(VictoryTarget::Domination, true);
    let plan = quiet_plan(&game);
    assert!(ai
        .domination_monument_item(&game, 0, cities[0], &plan)
        .is_none());
    assert!(ai
        .domination_monument_item(&game, 0, cities[1], &plan)
        .is_some());
}

/// Two cities and no units, so a second Governor has a seat.
fn two_city_board(titles: i64) -> Game {
    let mut game = Game::new_full(2, 32, 20, 366900, 500, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.found_city_for(0, (6, 8), None);
    game.found_city_for(0, (12, 8), None);
    game.players[0]
        .counters
        .insert("district_governor_titles".into(), titles);
    game
}

fn titles(game: &mut Game, ai: &AdvancedAi, plan: &StrategicPlan, count: i64) {
    game.players[0]
        .counters
        .insert("district_governor_titles".into(), count);
    ai.strategic_governors(game, 0, plan);
    assert_eq!(game.governor_titles_available(0), 0);
}

fn pingala_seat(gene: bool) -> AdvancedAi {
    let mut ai = seat(VictoryTarget::Domination, gene);
    ai.enable_domination_seat_appoints_pingala_first();
    ai
}

/// With the gene, Pingala's third title is Connoisseur and Magnus is the
/// fourth; without it Magnus is the third.
#[test]
fn connoisseur_is_the_third_title_and_magnus_the_fourth() {
    let mut game = two_city_board(2);
    let plan = quiet_plan(&game);
    let ai = pingala_seat(true);
    titles(&mut game, &ai, &plan, 2);
    assert!(ai.domination_pingala_owes_connoisseur(&game, 0, plan.strategy));
    titles(&mut game, &ai, &plan, 3);
    let pingala = &game.players[0].governor_roster["pingala"];
    assert!(
        pingala.promotions.contains("researcher") && pingala.promotions.contains("connoisseur"),
        "{:?}",
        pingala.promotions
    );
    assert!(!game.players[0].governor_roster.contains_key("magnus"));
    assert!(!ai.domination_pingala_owes_connoisseur(&game, 0, plan.strategy));
    titles(&mut game, &ai, &plan, 4);
    assert!(game.players[0].governor_roster.contains_key("magnus"));
    assert_eq!(
        game.players[0].governor_roster["pingala"].promotions.len(),
        2
    );

    let mut game = two_city_board(2);
    let off = pingala_seat(false);
    titles(&mut game, &off, &plan, 2);
    titles(&mut game, &off, &plan, 3);
    assert!(game.players[0].governor_roster.contains_key("magnus"));
    assert_eq!(
        game.players[0].governor_roster["pingala"].promotions.len(),
        1
    );
}

/// A Recovery plan keeps its Victor ahead of Connoisseur.
#[test]
fn a_recovery_plan_keeps_victor_ahead_of_connoisseur() {
    let mut game = two_city_board(2);
    let plan = quiet_plan(&game);
    let ai = pingala_seat(true);
    titles(&mut game, &ai, &plan, 2);
    let recovery = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        ..plan
    };
    assert!(!ai.domination_pingala_owes_connoisseur(&game, 0, recovery.strategy));
    titles(&mut game, &ai, &recovery, 3);
    assert!(game.players[0].governor_roster.contains_key("victor"));
    assert_eq!(
        game.players[0].governor_roster["pingala"].promotions.len(),
        1
    );
}
