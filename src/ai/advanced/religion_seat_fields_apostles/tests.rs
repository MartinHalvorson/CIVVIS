use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::game::install_test_district;
use std::sync::Arc;

/// Our founder with one Holy Site city of its own faith, and one major rival
/// holding `rival_cities` cities on another faith.
fn founder(rival_cities: usize, target: VictoryTarget) -> (Game, AdvancedAi, u32) {
    let mut g = Game::new_full(2, 60, 24, 936_312, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
    }
    g.found_city_for(0, (6, 8), None);
    let home = g.player_city_ids(0)[0];
    install_test_district(&mut g, home, "holy_site");
    g.players[0].religion = Some("Buddhism".into());
    g.players[1].religion = Some("Hinduism".into());
    g.cities
        .get_mut(&home)
        .unwrap()
        .pressure
        .insert("Buddhism".into(), 500.0);
    for index in 0..rival_cities {
        g.found_city_for(1, (14 + 6 * index as i32, 8), None);
    }
    for cid in g.player_city_ids(1) {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .pressure
            .insert("Hinduism".into(), 500.0);
    }
    g.current = 0;
    g.turn = 80;
    (g, AdvancedAi::targeting(target), home)
}

fn build_temple(g: &mut Game, city: u32, building: &str) {
    g.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name::Name::new(building));
}

#[test]
fn every_gene_is_opt_in() {
    opt_in_off_in_both_controllers("religion-seat-builds-holy-sites", |ai| {
        ai.religion_seat_builds_holy_sites
    });
    opt_in_off_in_both_controllers("religion-seat-presses-the-campaign", |ai| {
        ai.religion_seat_presses_the_campaign
    });
    opt_in_off_in_both_controllers("religion-seat-races-the-prophet", |ai| {
        ai.religion_seat_races_the_prophet
    });
    opt_in_off_in_both_controllers("religion-seat-reads-foreign-faith", |ai| {
        ai.religion_seat_reads_foreign_faith
    });
    opt_in_off_in_both_controllers("religion-seat-fields-apostles", |ai| {
        ai.religion_seat_fields_apostles
    });
    opt_in_off_in_both_controllers("religion-seat-takes-the-faith-beliefs", |ai| {
        ai.religion_seat_takes_the_faith_beliefs
    });
}

/// The cap reads the shipped value off, on another target and on the
/// defensive; with the gene it rises per six unconverted major cities and
/// stops at the ceiling.
#[test]
fn the_apostle_cap_grows_with_the_cities_left_to_convert() {
    let (g, ai, _) = founder(7, VictoryTarget::Religion);
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 2, "gene off");

    let (g, mut ai, _) = founder(7, VictoryTarget::Religion);
    ai.enable_religion_seat_fields_apostles();
    assert_eq!(
        ai.religion_seat_apostle_cap(&g, 0, true, 2),
        4,
        "seven unconverted cities: 2 + ceil(7/6)"
    );
    assert_eq!(
        ai.religion_seat_apostle_cap(&g, 0, false, 0),
        0,
        "on the defensive"
    );

    let (g, mut ai, _) = founder(7, VictoryTarget::Culture);
    ai.enable_religion_seat_fields_apostles();
    assert_eq!(ai.religion_seat_apostle_cap(&g, 0, true, 2), 2, "Culture");

    let (g, mut ai, _) = founder(9, VictoryTarget::Religion);
    ai.enable_religion_seat_fields_apostles();
    let mut g = g;
    for x in 0..24 {
        g.found_city_for(1, (2 + 2 * x, 18), None);
    }
    for cid in g.player_city_ids(1) {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .pressure
            .insert("Hinduism".into(), 500.0);
    }
    assert_eq!(
        ai.religion_seat_apostle_cap(&g, 0, true, 2),
        APOSTLE_CAP_CEILING,
        "stops at the ceiling"
    );
}

/// The first two Apostles come first; after that, one for one.
#[test]
fn apostles_lead_for_two_and_then_alternate() {
    let (g, mut ai, _) = founder(7, VictoryTarget::Religion);
    ai.enable_religion_seat_fields_apostles();
    let leads = |apostles, missionaries| {
        ai.religion_seat_leads_with_apostles(&g, 0, true, apostles, missionaries, 4)
    };
    assert!(leads(0, 3), "the first");
    assert!(leads(1, 0), "the second");
    assert!(leads(2, 2), "two and two: the Apostle");
    assert!(!leads(2, 1), "the Missionary's turn");
    assert!(!leads(4, 9), "at the cap");
    assert!(!ai.religion_seat_leads_with_apostles(&g, 0, false, 0, 0, 4));
}

/// The Faith waits only when a city of ours can field an Apostle and the bank
/// cannot pay for one; the Khmer Prasat counts as the Temple.
#[test]
fn the_bank_waits_only_for_an_apostle_a_city_can_field() {
    let (mut g, mut ai, home) = founder(7, VictoryTarget::Religion);
    ai.enable_religion_seat_fields_apostles();
    g.players[0].faith = 10.0;
    assert!(
        !ai.religion_seat_saving_for_an_apostle(&g, 0, "Buddhism", 80.0),
        "no Temple yet: the early Missionaries are unchanged"
    );
    assert_eq!(ai.religion_seat_apostle_reserve(&g, 0), 0.0);

    build_temple(&mut g, home, "prasat");
    assert!(AdvancedAi::city_can_field_an_apostle(
        &g,
        &g.cities[&home],
        "Buddhism"
    ));
    assert!(
        ai.religion_seat_saving_for_an_apostle(&g, 0, "Buddhism", 80.0),
        "short of the price: save"
    );
    assert!(ai.religion_seat_apostle_reserve(&g, 0) > 0.0);

    g.players[0].faith = 5_000.0;
    assert!(
        !ai.religion_seat_saving_for_an_apostle(&g, 0, "Buddhism", 80.0),
        "the bank covers it"
    );
}

/// A Temple in a city our faith has lost fields nothing.
#[test]
fn a_city_on_another_faith_cannot_field_an_apostle() {
    let (mut g, _, home) = founder(1, VictoryTarget::Religion);
    build_temple(&mut g, home, "temple");
    assert!(AdvancedAi::city_can_field_an_apostle(
        &g,
        &g.cities[&home],
        "Buddhism"
    ));
    let city = g.cities.get_mut(&home).unwrap();
    city.pressure.clear();
    city.pressure.insert("Hinduism".into(), 500.0);
    assert!(!AdvancedAi::city_can_field_an_apostle(
        &g,
        &g.cities[&home],
        "Buddhism"
    ));
}

/// The belief gene reads the lane, and the founding takes Pilgrimage first
/// where the shipped order takes Tithe.
#[test]
fn a_religion_seat_founds_on_pilgrimage_first() {
    let (g, mut ai, _) = founder(1, VictoryTarget::Religion);
    assert!(!ai.founder_takes_the_faith_beliefs(&g), "gene off");
    ai.enable_religion_seat_takes_the_faith_beliefs();
    assert!(ai.founder_takes_the_faith_beliefs(&g));

    let (g, mut ai, _) = founder(1, VictoryTarget::Culture);
    ai.enable_religion_seat_takes_the_faith_beliefs();
    assert!(!ai.founder_takes_the_faith_beliefs(&g), "a Culture seat");

    let (g, mut ai, _) = founder(1, VictoryTarget::Religion);
    assert_eq!(ai.base.founder_belief_order(&g)[0], "tithe", "shipped");
    ai.base.founder_takes_the_faith_beliefs = true;
    let order = ai.base.founder_belief_order(&g);
    assert_eq!(&order[..2], ["pilgrimage", "lay_ministry"]);
    assert_eq!(
        order.len(),
        g.rules.beliefs.founder.len(),
        "every founder belief stays reachable"
    );
}

/// A founder claims one Holy Site a turn in an idle city of two or more
/// citizens, and only once it holds three cities.
#[test]
fn an_idle_city_claims_a_holy_site() {
    let (mut g, mut ai, home) = founder(1, VictoryTarget::Religion);
    g.players[0].techs.insert(crate::name!("astrology"));
    g.found_city_for(0, (14, 18), None);
    let second = *g
        .player_city_ids(0)
        .iter()
        .find(|cid| **cid != home)
        .unwrap();
    for cid in g.player_city_ids(0) {
        let city = g.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.queue.clear();
    }
    let queued = |g: &Game| {
        g.player_city_ids(0)
            .iter()
            .filter(|cid| !g.cities[cid].queue.is_empty())
            .count()
    };

    ai.religion_seat_claims_a_holy_site(&mut g, 0);
    assert_eq!(queued(&g), 0, "gene off");

    ai.enable_religion_seat_builds_holy_sites();
    ai.religion_seat_claims_a_holy_site(&mut g, 0);
    assert_eq!(queued(&g), 0, "two cities: under the floor");

    g.found_city_for(0, (24, 18), None);
    let third = *g
        .player_city_ids(0)
        .iter()
        .find(|cid| **cid != home && **cid != second)
        .unwrap();
    {
        let city = g.cities.get_mut(&third).unwrap();
        city.pop = 1;
        city.queue.clear();
    }
    ai.religion_seat_claims_a_holy_site(&mut g, 0);
    assert_eq!(queued(&g), 1, "one claim a turn");
    assert!(
        matches!(g.cities[&second].queue.first(),
            Some(Item::District { district, .. }) if g.district_family(*district) == "holy_site"),
        "the town without one, not the one-citizen city"
    );
    assert!(g.cities[&home].queue.is_empty(), "already has a Holy Site");
}

/// A Holy Site city owes its Shrine and then its Temple; Faith buys them when
/// it covers the price, and otherwise the queue's head gives way.
#[test]
fn a_founder_takes_its_faith_buildings_first() {
    let (mut g, mut ai, home) = founder(1, VictoryTarget::Religion);
    ai.base.book_pos = 4;
    g.players[0].civics.insert(crate::name!("theology"));
    g.players[0].faith = 0.0;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    let head = |g: &Game| g.cities[&home].queue.first().cloned();

    ai.claim_religion_seat_faith_economy(&mut g, 0, &plan);
    assert!(head(&g).is_none(), "gene off");

    ai.enable_religion_seat_builds_holy_sites();
    ai.claim_religion_seat_faith_economy(&mut g, 0, &plan);
    assert!(
        matches!(head(&g), Some(Item::Building { building }) if building == "shrine"),
        "the Shrine comes first: {:?}",
        head(&g)
    );

    build_temple(&mut g, home, "shrine");
    g.cities.get_mut(&home).unwrap().queue.clear();
    ai.claim_religion_seat_faith_economy(&mut g, 0, &plan);
    assert!(
        matches!(head(&g), Some(Item::Building { building }) if building == "temple"),
        "then the Temple: {:?}",
        head(&g)
    );
}

/// Units and Walls keep their queues; Districts and ordinary buildings give
/// way.
#[test]
fn a_faith_building_displaces_only_ordinary_work() {
    let (g, _, _) = founder(1, VictoryTarget::Religion);
    let may = |item: Item| AdvancedAi::faith_building_may_displace(&g, Some(&item));
    assert!(AdvancedAi::faith_building_may_displace(&g, None), "idle");
    assert!(may(Item::Building {
        building: crate::name!("granary")
    }));
    assert!(!may(Item::Building {
        building: crate::name!("walls")
    }));
    assert!(!may(Item::Building {
        building: crate::name!("temple")
    }));
    assert!(!may(Item::Unit {
        unit: crate::name!("settler")
    }));
}

fn spawn_religious(
    g: &mut Game,
    kind: &str,
    owner: usize,
    pos: crate::Pos,
    faith: &str,
    hp: i32,
) -> u32 {
    let uid = g.spawn_unit(kind, owner, pos);
    let moves = g.unit_max_moves(uid);
    let unit = g.units.get_mut(&uid).unwrap();
    unit.religion = Some(faith.into());
    unit.charges = 3;
    unit.hp = hp;
    unit.moves_left = moves;
    uid
}

/// Our Apostle takes the fight a wounded rival Missionary cannot win; with the
/// gene off, or on another lane, nothing is decided here.
#[test]
fn an_apostle_fights_a_winnable_rival_before_stepping_away() {
    let (mut g, ai, _) = founder(1, VictoryTarget::Religion);
    let apostle = spawn_religious(&mut g, "apostle", 0, (30, 12), "Buddhism", 100);
    let rival = spawn_religious(&mut g, "missionary", 1, (31, 12), "Hinduism", 20);
    assert_eq!(
        ai.religion_seat_apostle_fights_first(&mut g, 0, apostle),
        None,
        "gene off"
    );

    let (mut g, mut ai, _) = founder(1, VictoryTarget::Culture);
    ai.enable_religion_seat_presses_the_campaign();
    let apostle = spawn_religious(&mut g, "apostle", 0, (30, 12), "Buddhism", 100);
    spawn_religious(&mut g, "missionary", 1, (31, 12), "Hinduism", 20);
    assert_eq!(
        ai.religion_seat_apostle_fights_first(&mut g, 0, apostle),
        None,
        "a Culture seat"
    );

    let (mut g, mut ai, _) = founder(1, VictoryTarget::Religion);
    ai.enable_religion_seat_presses_the_campaign();
    let apostle = spawn_religious(&mut g, "apostle", 0, (30, 12), "Buddhism", 100);
    let target = spawn_religious(&mut g, "missionary", 1, (31, 12), "Hinduism", 20);
    assert_eq!(
        ai.religion_seat_apostle_fights_first(&mut g, 0, apostle),
        Some(true)
    );
    assert!(
        g.units.get(&target).is_none_or(|unit| unit.hp < 20),
        "the rival took the blow"
    );
    let _ = rival;
}

/// The corps yields only to a killing blow once the gene reads a founded
/// Religion seat.
#[test]
fn the_campaign_reads_only_lethal_threats() {
    let (g, ai, _) = founder(1, VictoryTarget::Religion);
    assert!(!ai.religion_seat_presses_on(&g, 0), "gene off");
    let (mut g, mut ai, _) = founder(1, VictoryTarget::Religion);
    ai.enable_religion_seat_presses_the_campaign();
    assert!(ai.religion_seat_presses_on(&g, 0));
    g.players[0].religion = None;
    assert!(!ai.religion_seat_presses_on(&g, 0), "not yet founded");
    let (g, mut ai, _) = founder(1, VictoryTarget::Culture);
    ai.enable_religion_seat_presses_the_campaign();
    assert!(!ai.religion_seat_presses_on(&g, 0), "a Culture seat");
}

/// A foreign city whose banner shows our faith stops being a target, and the
/// corps cap stops counting it.
#[test]
fn a_banner_showing_our_faith_ends_the_campaign_there() {
    let (mut g, mut ai, _) = founder(7, VictoryTarget::Religion);
    let rival = g.player_city_ids(1)[0];
    Arc::make_mut(&mut g.observed_foreign_city_religion).insert(rival, "Buddhism".into());
    let city = g.cities[&rival].clone();
    assert!(
        !ai.foreign_city_shows_our_faith(&g, 0, &city, "Buddhism"),
        "gene off"
    );
    ai.enable_religion_seat_reads_foreign_faith();
    assert!(ai.foreign_city_shows_our_faith(&g, 0, &city, "Buddhism"));
    assert!(
        !ai.foreign_city_shows_our_faith(&g, 0, &city, "Hinduism"),
        "another faith's spreader"
    );
    assert_eq!(
        AdvancedAi::unconverted_major_cities(&g, 0),
        6,
        "seven cities, one banner ours"
    );
    let (mut g, mut ai, _) = founder(1, VictoryTarget::Culture);
    let rival = g.player_city_ids(1)[0];
    Arc::make_mut(&mut g.observed_foreign_city_religion).insert(rival, "Buddhism".into());
    ai.enable_religion_seat_reads_foreign_faith();
    let city = g.cities[&rival].clone();
    assert!(
        !ai.foreign_city_shows_our_faith(&g, 0, &city, "Buddhism"),
        "a Culture seat"
    );
}

/// A founding seat with a finished Holy Site and a queue full of Settlers.
fn racing_seat() -> (Game, AdvancedAi, u32, StrategicPlan) {
    let (mut g, ai, home) = founder(1, VictoryTarget::Religion);
    g.players[0].religion = None;
    g.players[1].religion = None;
    g.players[0].techs.insert(crate::name!("astrology"));
    g.cities.get_mut(&home).unwrap().pressure.clear();
    g.cities.get_mut(&home).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    let plan = StrategicPlan {
        strategy: GrandStrategy::Religion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, home, plan)
}

fn runs_prayers(g: &Game, city: u32) -> bool {
    matches!(g.cities[&city].queue.first(),
        Some(Item::Project { project }) if project == "holy_site_prayers")
}

/// Prayers take the head of a Holy Site city's queue while the race is open.
#[test]
fn the_race_runs_prayers_ahead_of_the_queue() {
    let (mut g, ai, home, plan) = racing_seat();
    ai.claim_religion_seat_prayers(&mut g, 0, &plan);
    assert!(!runs_prayers(&g, home), "gene off");

    let (mut g, mut ai, home, plan) = racing_seat();
    ai.enable_religion_seat_races_the_prophet();
    ai.claim_religion_seat_prayers(&mut g, 0, &plan);
    assert!(runs_prayers(&g, home), "{:?}", g.cities[&home].queue);
}

/// A threatened city, a pending Prophet, a founded faith and another lane all
/// leave the queue alone.
#[test]
fn the_race_yields_when_it_should() {
    let run = |tweak: &dyn Fn(&mut Game, &mut StrategicPlan, u32)| {
        let (mut g, mut ai, home, mut plan) = racing_seat();
        ai.enable_religion_seat_races_the_prophet();
        tweak(&mut g, &mut plan, home);
        ai.claim_religion_seat_prayers(&mut g, 0, &plan);
        runs_prayers(&g, home)
    };
    assert!(!run(&|_, plan, home| plan.threatened_city = Some(home)));
    assert!(!run(&|g, _, _| g.players[0].prophet_pending = true));
    assert!(!run(
        &|g, _, _| g.players[0].religion = Some("Buddhism".into())
    ));
    assert!(!run(&|g, _, home| {
        g.cities.get_mut(&home).unwrap().last_attacked = g.turn;
    }));

    let (mut g, _, home, plan) = racing_seat();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_religion_seat_races_the_prophet();
    ai.claim_religion_seat_prayers(&mut g, 0, &plan);
    assert!(!runs_prayers(&g, home), "a Culture seat");
}

/// With the gene on the founding site may take a queue that holds a Settler.
#[test]
fn the_race_takes_a_busy_queue_for_its_holy_site() {
    let site = |gene: bool| {
        let (mut g, mut ai, home, plan) = racing_seat();
        g.cities.get_mut(&home).unwrap().districts.clear();
        ai.enable_religion_seat_founds_first();
        if gene {
            ai.enable_religion_seat_races_the_prophet();
        }
        ai.religion_seat_holy_site(&g, 0, &plan)
            .map(|(city, _)| city)
    };
    assert_eq!(site(false), None, "off: only an idle queue");
    assert!(site(true).is_some(), "on: the Settler's queue");
}
