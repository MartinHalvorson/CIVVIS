use super::*;
use crate::game::{HostResearchQuote, HostUnitFacts, HostUnitUpgrade, HostUnitUpgradeResource};

/// Two majors who have met: our founded capital (population 4, idle, no
/// Walls), the rival's city 9-12 tiles off, every unit removed, everything
/// within nine tiles of the capital in sight, the rival at 3 times our power
/// with a Warrior five tiles from the capital, and Animal Husbandry and
/// Mining known (Archery and Masonry both open). Returns the game and our
/// capital.
fn watched() -> (Game, u32) {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a Settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital");
    for pid in [0, 1] {
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
            (9..=12).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(1, sites[0], None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 4;
    city.queue.clear();
    game.turn = 12;
    game.host_observed = Arc::new(game.wdisk(home, 9).into_iter().collect());
    game.players[0]
        .techs
        .extend([crate::name!("animal_husbandry"), crate::name!("mining")]);
    game.players[0].research = None;
    game.players[0].gold = 90.0;
    game.players[0].gold_per_turn = 6.0;
    power(&mut game, 0, 50.0);
    power(&mut game, 1, 150.0);
    let at = ground(&game, capital, 5);
    game.spawn_unit("warrior", 1, at);
    (game, capital)
}

fn power(game: &mut Game, pid: usize, value: f64) {
    Arc::make_mut(&mut game.observed_military_power).insert(pid, value);
}

/// A land tile exactly `gap` tiles from the capital not yet holding a unit.
fn ground(game: &Game, capital: u32, gap: i32) -> Pos {
    let home = game.cities[&capital].pos;
    let mut tiles: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            game.wdist(**pos, home) == gap
                && !game.rules.is_water(tile)
                && !game.units.values().any(|unit| unit.pos == **pos)
        })
        .map(|(pos, _)| *pos)
        .collect();
    tiles.sort();
    tiles[0]
}

/// One of our Slingers `gap` tiles from the capital, with the host's Archer
/// upgrade on offer at 30 Gold.
fn slinger(game: &mut Game, capital: u32, gap: i32) -> u32 {
    let at = if gap == 0 {
        game.cities[&capital].pos
    } else {
        ground(game, capital, gap)
    };
    let uid = game.spawn_test_unit("slinger", 0, at);
    Arc::make_mut(&mut game.host_unit_facts).insert(
        uid,
        HostUnitFacts {
            maintenance: Some(0.0),
            upgrade: Some(HostUnitUpgrade {
                to: Some(crate::name!("archer")),
                cost: Some(30.0),
                resources: Some(HostUnitUpgradeResource {
                    resource: None,
                    cost: 0.0,
                }),
                blocked: None,
            }),
            ..Default::default()
        },
    );
    uid
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_capital_archers_before_the_surprise();
    ai
}

/// The live armed pair: the walls gene beside this one.
fn armed_with_walls() -> AdvancedAi {
    let mut ai = armed();
    ai.enable_capital_walls_before_the_surprise();
    ai
}

fn expansion(game: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    }
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn walls() -> Item {
    Item::Building {
        building: crate::name!("walls"),
    }
}

fn archery() -> Option<Name> {
    Some(crate::name!("archery"))
}

/// The watch is recorded with this gene alone, and the threat reads at 2.5
/// times our power on an unwalled original capital only.
#[test]
fn the_threat_reads_the_watch_at_two_and_a_half_times_on_an_unwalled_capital() {
    let (game, capital) = watched();

    let mut stock = AdvancedAi::new();
    stock.observe_turn_start_hostiles(&game, 0);
    assert!(stock.capital_watch_sightings.is_empty(), "off: no watch");
    assert!(stock.capital_archers_threat(&game, 0).is_none());

    let mut ai = armed();
    assert!(
        ai.capital_archers_threat(&game, 0).is_none(),
        "no sighting yet"
    );
    ai.observe_turn_start_hostiles(&game, 0);
    assert_eq!(ai.capital_watch_sightings.get(&1), Some(&game.turn));
    let (rival, ratio) = ai
        .capital_archers_threat(&game, 0)
        .expect("3.0x at the door");
    assert_eq!(rival, 1);
    assert!((ratio - 3.0).abs() < 1e-9);
    assert!(
        !ai.capital_walls_before_the_surprise
            && ai.capital_walls_research_threat(&game, 0).is_none(),
        "the walls gene stays off"
    );

    let mut level = game.clone();
    power(&mut level, 1, 120.0);
    assert!(
        ai.capital_archers_threat(&level, 0).is_none(),
        "2.4x: under the bar"
    );

    let mut later = game.clone();
    later.turn +=
        later.standard_duration(capital_walls_before_the_surprise::CAPITAL_WATCH_STANDARD) + 1;
    assert!(
        ai.capital_archers_threat(&later, 0).is_none(),
        "stale sighting"
    );

    let mut walled = game.clone();
    walled
        .cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert!(walled.city_max_wall_hp(&walled.cities[&capital]) > 0);
    assert!(
        ai.capital_archers_threat(&walled, 0).is_none(),
        "the Walls stand: the gene is done"
    );
}

/// The research leg: Archery while a Slinger stands at the capital or is in
/// training there; not without one, not once Archery is known.
#[test]
fn archery_is_the_goal_while_a_slinger_waits_at_the_capital() {
    let (mut game, capital) = watched();
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert_eq!(
        ai.capital_archers_research_goal(&game, 0),
        None,
        "no Slinger: Masonry keeps its place"
    );

    let mut training = game.clone();
    training
        .apply(
            0,
            &Action::Produce {
                city: capital,
                item: unit("slinger"),
            },
        )
        .unwrap();
    assert_eq!(ai.capital_archers_research_goal(&training, 0), archery());

    let mut far = game.clone();
    slinger(&mut far, capital, CAPITAL_ARCHERS_REACH_TILES + 1);
    assert_eq!(
        ai.capital_archers_research_goal(&far, 0),
        None,
        "nine tiles off: not the capital's shooter"
    );

    slinger(&mut game, capital, 1);
    assert_eq!(ai.capital_archers_research_goal(&game, 0), archery());
    assert_eq!(
        AdvancedAi::new().capital_archers_research_goal(&game, 0),
        None,
        "off"
    );
    let reason = ai.capital_archers_research_reason(&game, 0).unwrap();
    assert!(
        reason.starts_with("capital-archers-before-the-surprise: Archery before Masonry"),
        "{reason}"
    );

    game.players[0].techs.insert(crate::name!("archery"));
    assert_eq!(ai.capital_archers_research_goal(&game, 0), None);
}

/// The research picker itself: with the walls gene alone the free slot goes
/// to Masonry, with both it goes to Archery, and once Archery is known the
/// walls gene's Masonry is next.
#[test]
fn the_free_research_slot_takes_archery_then_masonry() {
    let (mut game, capital) = watched();
    slinger(&mut game, capital, 1);
    let plan = expansion(&game);

    let mut walls_only = AdvancedAi::new();
    walls_only.enable_capital_walls_before_the_surprise();
    walls_only.observe_turn_start_hostiles(&game, 0);
    let mut before = game.clone();
    walls_only.advanced_research(&mut before, 0, &plan);
    assert_eq!(before.players[0].research.as_deref(), Some("masonry"));

    let mut ai = armed_with_walls();
    ai.observe_turn_start_hostiles(&game, 0);
    let mut first = game.clone();
    ai.advanced_research(&mut first, 0, &plan);
    assert_eq!(first.players[0].research.as_deref(), Some("archery"));

    let mut second = game.clone();
    second.players[0].techs.insert(crate::name!("archery"));
    ai.advanced_research(&mut second, 0, &plan);
    assert_eq!(second.players[0].research.as_deref(), Some("masonry"));
}

fn quotes(game: &mut Game, current: &str) {
    game.players[0].research = Some(current.into());
    game.players[0].research_progress = 5.0;
    game.set_host_research_prerequisites(BTreeMap::from([
        (
            crate::name!("archery"),
            vec![crate::name!("animal_husbandry")],
        ),
        (crate::name!("masonry"), vec![crate::name!("mining")]),
    ]))
    .unwrap();
    let quote = |cost: f64, progress: f64| HostResearchQuote {
        cost: Some(cost),
        progress: Some(progress),
    };
    game.host_research_quotes = Arc::new(BTreeMap::from([(
        0,
        BTreeMap::from([
            (Name::new(current), quote(40.0, 5.0)),
            (crate::name!("archery"), quote(25.0, 0.0)),
            (crate::name!("masonry"), quote(40.0, 0.0)),
        ]),
    )]));
    game.host_research_eligibility = Arc::new(BTreeMap::from([(
        0,
        BTreeMap::from([
            (crate::name!("archery"), true),
            (crate::name!("masonry"), true),
        ]),
    )]));
}

/// Live 093657Z, turn 14: Archery was 14 of 25 in when the walls gene's
/// Masonry interrupted it. With this gene Archery keeps the slot; another
/// technology in progress gives way to Archery; a Masonry in progress
/// finishes.
#[test]
fn archery_in_progress_is_not_interrupted_and_interrupts_others() {
    let (mut game, capital) = watched();
    slinger(&mut game, capital, 1);
    let plan = expansion(&game);

    let mut walls_only = AdvancedAi::new();
    walls_only.enable_capital_walls_before_the_surprise();
    walls_only.observe_turn_start_hostiles(&game, 0);
    let mut ai = armed_with_walls();
    ai.observe_turn_start_hostiles(&game, 0);

    let mut underway = game.clone();
    quotes(&mut underway, "archery");
    let mut interrupted = underway.clone();
    walls_only.advanced_research(&mut interrupted, 0, &plan);
    assert_eq!(
        interrupted.players[0].research.as_deref(),
        Some("masonry"),
        "the walls gene alone takes the slot from Archery"
    );
    assert_eq!(ai.capital_archers_research_switch(&underway, 0), None);
    ai.advanced_research(&mut underway, 0, &plan);
    assert_eq!(underway.players[0].research.as_deref(), Some("archery"));

    let mut other = game.clone();
    quotes(&mut other, "pottery");
    assert_eq!(ai.capital_archers_research_switch(&other, 0), archery());
    ai.advanced_research(&mut other, 0, &plan);
    assert_eq!(other.players[0].research.as_deref(), Some("archery"));

    let mut masonry = game.clone();
    quotes(&mut masonry, "masonry");
    assert_eq!(ai.capital_archers_research_switch(&masonry, 0), None);
    ai.advanced_research(&mut masonry, 0, &plan);
    assert_eq!(masonry.players[0].research.as_deref(), Some("masonry"));

    let mut off = game.clone();
    quotes(&mut off, "pottery");
    assert_eq!(
        AdvancedAi::new().capital_archers_research_switch(&off, 0),
        None
    );
}

/// The upgrade leg: every Slinger at the capital upgrades the turn the host
/// offers it, at peace and at war, down to the upkeep reserve; one beyond
/// the reach, a treasury short of the price, and the gene off leave it.
#[test]
fn the_capitals_slingers_take_the_archer_upgrade_at_once() {
    let (mut game, capital) = watched();
    game.players[0].techs.insert(crate::name!("archery"));
    let first = slinger(&mut game, capital, 0);
    let second = slinger(&mut game, capital, 2);
    let away = slinger(&mut game, capital, CAPITAL_ARCHERS_REACH_TILES + 1);
    game.players[0].gold = 75.0;
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);

    let mut off = game.clone();
    assert_eq!(AdvancedAi::new().capital_archers_upgrade(&mut off, 0), 0);
    assert_eq!(off.units[&first].kind, "slinger");

    let mut poor = game.clone();
    poor.players[0].gold = 29.0;
    assert_eq!(ai.capital_archers_upgrade(&mut poor, 0), 0);

    for at_war in [false, true] {
        let mut now = game.clone();
        if at_war {
            now.at_war.insert((0, 1));
            now.at_war.insert((1, 0));
        }
        assert_eq!(ai.capital_archers_upgrade(&mut now, 0), 2);
        assert_eq!(now.units[&first].kind, "archer");
        assert_eq!(now.units[&second].kind, "archer");
        assert_eq!(now.units[&away].kind, "slinger");
        assert_eq!(now.players[0].gold, 15.0);
    }

    // The Walls standing end the other legs, not a waiting Slinger's upgrade.
    let mut walled = game.clone();
    walled
        .cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert!(ai.capital_archers_threat(&walled, 0).is_none());
    assert_eq!(ai.capital_archers_upgrade(&mut walled, 0), 2);
}

/// The Gold leg: the upgrade bills of up to two Slingers at the capital are
/// held from every purchase through `purchase_upkeep_floor`, and the
/// peacetime Trader, Builder and tile purchases wait.
#[test]
fn the_upgrade_bills_are_held_and_the_chest_waits() {
    let (mut game, capital) = watched();
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert_eq!(ai.capital_archers_gold_held(&game, 0), 0.0, "no Slinger");
    assert!(ai.capital_archers_hold_the_chest(&game, 0));
    let stock = AdvancedAi::new();
    assert!(!stock.capital_archers_hold_the_chest(&game, 0), "off");

    slinger(&mut game, capital, 1);
    let bill = game
        .unit_upgrade_price_in_formation(0, crate::name!("slinger"), crate::name!("archer"), 0)
        .expect("fixture: a Slinger upgrades to an Archer")
        .0;
    assert!(bill > 0.0);
    assert_eq!(ai.capital_archers_gold_held(&game, 0), bill);
    assert_eq!(ai.purchase_upkeep_floor(&game, 0, 0.0), bill);
    assert_eq!(stock.capital_archers_gold_held(&game, 0), 0.0);
    assert_eq!(stock.purchase_upkeep_floor(&game, 0, 0.0), 0.0);

    slinger(&mut game, capital, 2);
    slinger(&mut game, capital, 3);
    assert_eq!(
        ai.capital_archers_gold_held(&game, 0),
        2.0 * bill,
        "two bills at most"
    );

    let mut level = game.clone();
    power(&mut level, 1, 100.0);
    assert_eq!(ai.capital_archers_gold_held(&level, 0), 0.0, "2.0x");
    assert!(!ai.capital_archers_hold_the_chest(&level, 0));

    let mut walled = game.clone();
    walled
        .cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert_eq!(ai.capital_archers_gold_held(&walled, 0), 0.0);
    assert!(!ai.capital_archers_hold_the_chest(&walled, 0));
}

/// The production leg: with one shooter at home the capital's Settler gives
/// way to a second one and the claim holds it; two at home, three owned, a
/// Settler two turns from done, a soldier in training and the gene off leave
/// the queue alone.
#[test]
fn the_capital_trains_its_second_shooter_ahead_of_the_settler() {
    let (mut game, capital) = watched();
    slinger(&mut game, capital, 1);
    let settler = unit("settler");
    game.apply(
        0,
        &Action::Produce {
            city: capital,
            item: settler.clone(),
        },
    )
    .unwrap();
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    assert_eq!(game.cities[&capital].hp, CITY_MAX_HP);

    let mut off = game.clone();
    AdvancedAi::new().claim_capital_archers_before_the_surprise(&mut off, 0);
    assert_eq!(off.cities[&capital].queue.first(), Some(&settler));

    let mut claimed = game.clone();
    ai.claim_capital_archers_before_the_surprise(&mut claimed, 0);
    assert_eq!(
        claimed.cities[&capital].queue.first(),
        Some(&unit("slinger"))
    );
    assert!(ai.capital_archers_hold(&claimed, 0, capital, &unit("slinger")));
    assert!(!ai.capital_archers_hold(&claimed, 0, capital, &settler));
    assert!(!AdvancedAi::new().capital_archers_hold(&claimed, 0, capital, &unit("slinger")));

    // With Archery the trainee is the Archer.
    let mut later = game.clone();
    later.players[0].techs.insert(crate::name!("archery"));
    ai.claim_capital_archers_before_the_surprise(&mut later, 0);
    assert_eq!(later.cities[&capital].queue.first(), Some(&unit("archer")));

    let mut two = game.clone();
    slinger(&mut two, capital, 2);
    ai.claim_capital_archers_before_the_surprise(&mut two, 0);
    assert_eq!(two.cities[&capital].queue.first(), Some(&settler));

    let mut afield = game.clone();
    slinger(&mut afield, capital, 6);
    slinger(&mut afield, capital, 7);
    ai.claim_capital_archers_before_the_surprise(&mut afield, 0);
    assert_eq!(
        afield.cities[&capital].queue.first(),
        Some(&settler),
        "three owned: no fourth"
    );

    let mut nearly = game.clone();
    let cost = nearly.item_remaining_cost_for_city(0, capital, &settler);
    let production = nearly.city_yields(capital).production.max(0.1);
    nearly.cities.get_mut(&capital).unwrap().production = cost - 1.5 * production;
    ai.claim_capital_archers_before_the_surprise(&mut nearly, 0);
    assert_eq!(
        nearly.cities[&capital].queue.first(),
        Some(&settler),
        "a Settler two turns from done finishes first"
    );

    let mut soldier = game.clone();
    soldier
        .apply(
            0,
            &Action::Produce {
                city: capital,
                item: unit("warrior"),
            },
        )
        .unwrap();
    ai.claim_capital_archers_before_the_surprise(&mut soldier, 0);
    assert_eq!(
        soldier.cities[&capital].queue.first(),
        Some(&unit("warrior"))
    );

    let mut level = game.clone();
    power(&mut level, 1, 100.0);
    ai.claim_capital_archers_before_the_surprise(&mut level, 0);
    assert_eq!(level.cities[&capital].queue.first(), Some(&settler));
}

/// With Masonry known the walls gene's claim owns the capital's queue and
/// this gene's claim, which runs after it, leaves the Walls there.
#[test]
fn the_walls_claim_keeps_the_queue_once_masonry_is_known() {
    let (mut game, capital) = watched();
    game.players[0].techs.insert(crate::name!("masonry"));
    slinger(&mut game, capital, 1);
    game.apply(
        0,
        &Action::Produce {
            city: capital,
            item: unit("settler"),
        },
    )
    .unwrap();
    let mut ai = armed_with_walls();
    ai.observe_turn_start_hostiles(&game, 0);
    ai.claim_capital_walls_before_the_surprise(&mut game, 0);
    assert_eq!(game.cities[&capital].queue.first(), Some(&walls()));
    ai.claim_capital_archers_before_the_surprise(&mut game, 0);
    assert_eq!(game.cities[&capital].queue.first(), Some(&walls()));
}

/// The purchase leg: with Archery known and no shooter at home the Gold above
/// the floor buys an Archer into the capital at once; a shooter in training
/// counts, two at home, a short treasury, the Walls standing, a ratio under
/// the bar and the gene off buy nothing.
#[test]
fn the_capital_buys_its_missing_shooter_above_the_held_bills() {
    let (mut game, capital) = watched();
    game.players[0].techs.insert(crate::name!("archery"));
    game.players[0].gold = 400.0;
    let home = game.cities[&capital].pos;
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);
    let shooters_home = |game: &Game| {
        game.units
            .values()
            .filter(|unit| unit.owner == 0 && unit.pos == home && unit.kind == "archer")
            .count()
    };
    let price = game
        .unit_purchase_cost(0, capital, "archer", "gold")
        .expect("fixture: an Archer for sale");
    assert!(price > 0.0 && price < 400.0, "{price}");

    let mut off = game.clone();
    assert!(!AdvancedAi::new().capital_archers_purchase(&mut off, 0));
    assert_eq!(shooters_home(&off), 0);

    let mut bought = game.clone();
    assert!(ai.capital_archers_purchase(&mut bought, 0));
    assert_eq!(shooters_home(&bought), 1);
    assert!((bought.players[0].gold - (400.0 - price)).abs() < 1e-6);

    // One at home and one in training make two: nothing more is bought.
    let mut training = bought.clone();
    training
        .apply(
            0,
            &Action::Produce {
                city: capital,
                item: unit("archer"),
            },
        )
        .unwrap();
    assert!(!ai.capital_archers_purchase(&mut training, 0));

    let mut two = game.clone();
    slinger(&mut two, capital, 1);
    slinger(&mut two, capital, 2);
    assert!(!ai.capital_archers_purchase(&mut two, 0), "two at home");

    let mut poor = game.clone();
    poor.players[0].gold = price - 1.0;
    assert!(!ai.capital_archers_purchase(&mut poor, 0));

    // A waiting Slinger's upgrade bill is held from the purchase.
    let mut held = game.clone();
    slinger(&mut held, capital, 1);
    let bill = ai.capital_archers_gold_held(&held, 0);
    assert!(bill > 0.0);
    held.players[0].gold = price + bill - 1.0;
    assert!(
        !ai.capital_archers_purchase(&mut held, 0),
        "the bill is kept"
    );
    held.players[0].gold = price + bill + 1.0;
    assert!(ai.capital_archers_purchase(&mut held, 0));

    let mut level = game.clone();
    power(&mut level, 1, 120.0);
    assert!(!ai.capital_archers_purchase(&mut level, 0), "2.4x");

    let mut walled = game.clone();
    walled
        .cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert!(
        !ai.capital_archers_purchase(&mut walled, 0),
        "the Walls stand"
    );
}

/// The keeper leg: the best shooter on the capital's City Center keeps it
/// and fortifies (Walls standing or not); with the centre empty the nearest
/// shooter within reach walks home; a melee soldier on the centre, a
/// shooter out of reach, a lapsed watch and the gene off leave the units to
/// their own steps.
#[test]
fn the_best_shooter_keeps_the_capitals_centre() {
    let (mut game, capital) = watched();
    game.players[0].techs.insert(crate::name!("archery"));
    let home = game.cities[&capital].pos;
    let mut ai = armed();
    ai.observe_turn_start_hostiles(&game, 0);

    let mut kept = game.clone();
    let sling = slinger(&mut kept, capital, 0);
    assert_eq!(ai.capital_archers_keeper(&kept, 0), Some((sling, home)));
    assert_eq!(AdvancedAi::new().capital_archers_keeper(&kept, 0), None);
    let near = ground(&kept, capital, 1);
    let archer = kept.spawn_test_unit("archer", 0, near);
    assert_eq!(
        ai.capital_archers_keeper(&kept, 0),
        Some((sling, home)),
        "the one on the centre keeps it"
    );
    assert_eq!(ai.capital_archers_keeper_step(&mut kept, 0, archer), None);
    assert_eq!(
        ai.capital_archers_keeper_step(&mut kept, 0, sling),
        Some(false)
    );
    assert_eq!(kept.units[&sling].pos, home, "it holds the centre");

    let mut walled = kept.clone();
    walled
        .cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("walls"));
    assert_eq!(
        ai.capital_archers_keeper(&walled, 0),
        Some((sling, home)),
        "the Walls do not end the keeper"
    );

    let mut called = game.clone();
    let start = ground(&called, capital, 3);
    let walker = called.spawn_test_unit("archer", 0, start);
    let farther = ground(&called, capital, 5);
    called.spawn_test_unit("archer", 0, farther);
    assert_eq!(ai.capital_archers_keeper(&called, 0), Some((walker, home)));
    assert_eq!(
        ai.capital_archers_keeper_step(&mut called, 0, walker),
        Some(true)
    );
    assert!(
        called.wdist(called.units[&walker].pos, home) < 3,
        "it walks home"
    );

    let mut held = game.clone();
    held.spawn_test_unit("warrior", 0, home);
    let beside = ground(&held, capital, 1);
    held.spawn_test_unit("archer", 0, beside);
    assert_eq!(
        ai.capital_archers_keeper(&held, 0),
        None,
        "a soldier of ours holds the centre already"
    );

    let mut afar = game.clone();
    let out = ground(&afar, capital, CAPITAL_ARCHERS_REACH_TILES + 1);
    afar.spawn_test_unit("archer", 0, out);
    assert_eq!(ai.capital_archers_keeper(&afar, 0), None, "out of reach");

    let mut later = kept.clone();
    later.turn +=
        later.standard_duration(capital_walls_before_the_surprise::CAPITAL_WATCH_STANDARD) + 1;
    assert_eq!(
        ai.capital_archers_keeper(&later, 0),
        None,
        "the watch lapsed"
    );
}
