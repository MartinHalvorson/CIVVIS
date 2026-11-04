use super::*;
use crate::ai::advanced::near_rival_deterrence::DETERRENCE_START_STANDARD;

/// `players` majors (met when two), our one city placed on the first land
/// tile whose Aqueduct has a legal site, every unit removed, Pottery, the
/// Wheel and Engineering known, the Granary built, the population at its
/// housing, and Production banked on the Aqueduct so it finishes inside the
/// cap. With two players the rival's city stands 5-10 tiles away. `turn` is in
/// standard turns. Returns the game, our city and the Aqueduct item.
fn capped_city(players: usize, turn: u32) -> Option<(Game, u32, Item)> {
    for seed in [936_221_u64, 424_242, 777_001, 31_337, 12_345] {
        let mut base = Game::new_full(players, 32, 20, seed, 200, 0, false);
        for pid in 0..players {
            for uid in base.player_unit_ids(pid) {
                base.remove_unit(uid);
            }
        }
        let mut sites: Vec<Pos> = base
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| !base.rules.is_water(tile))
            .map(|(pos, _)| *pos)
            .collect();
        sites.sort();
        for site in sites {
            let mut game = base.clone();
            let ours = game.place_city(0, site, None);
            for tech in [
                "pottery",
                "the_wheel",
                "engineering",
                "masonry",
                "bronze_working",
            ] {
                game.players[0].techs.insert(Name::new(tech));
            }
            let aqueduct = BasicAi::civ_district(&game, 0, "aqueduct");
            let Some(pos) = game
                .district_sites(ours, aqueduct)
                .into_iter()
                .filter(|pos| {
                    game.can_produce(
                        0,
                        ours,
                        &Item::District {
                            district: aqueduct,
                            pos: *pos,
                        },
                    )
                })
                .min()
            else {
                continue;
            };
            if players > 1 {
                let home = game.cities[&ours].pos;
                let mut rival_sites: Vec<Pos> = game
                    .map
                    .tiles
                    .iter()
                    .filter(|(pos, tile)| {
                        (5..=10).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
                    })
                    .map(|(pos, _)| *pos)
                    .collect();
                rival_sites.sort();
                let Some(rival) = rival_sites.first().copied() else {
                    continue;
                };
                game.place_city(1, rival, None);
                game.players[0].met.insert(1);
                game.players[1].met.insert(0);
            }
            game.turn = game.standard_duration(turn);
            let item = Item::District {
                district: aqueduct,
                pos,
            };
            let city = game.cities.get_mut(&ours).unwrap();
            city.buildings.push(crate::name!("granary"));
            city.queue.clear();
            city.production_progress
                .insert(Game::item_progress_key(&item), 40.0);
            let housing = game.city_housing(&game.cities[&ours]);
            game.cities.get_mut(&ours).unwrap().pop =
                (housing.ceil() as i32).max(CAPPED_AQUEDUCT_MIN_POP);
            return Some((game, ours, item));
        }
    }
    None
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_aqueduct_claims_the_capped_queue();
    ai
}

fn head(game: &Game, cid: u32) -> Option<Item> {
    game.cities[&cid].queue.first().cloned()
}

fn queue(game: &mut Game, cid: u32, item: Item) {
    game.cities.get_mut(&cid).unwrap().queue = vec![item];
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

/// Past the opening floors (standard turn 130).
const LATE: u32 = 130;

/// The blocked case: a capped city training a routine Warrior puts its
/// Aqueduct first; off, nothing moves; with the Granary unbuilt the Granary's
/// claim owns the city; with the Aqueduct built nothing moves either.
#[test]
fn a_capped_city_claims_its_aqueduct_over_a_routine_unit() {
    let Some((game, city, aqueduct)) = capped_city(1, LATE) else {
        panic!("fixture: a city with a legal Aqueduct site");
    };
    let mut busy = game.clone();
    queue(&mut busy, city, unit("warrior"));

    let stock = AdvancedAi::new();
    let mut off = busy.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_capped_aqueduct(&mut off, 0, &plan);
    assert_eq!(head(&off, city), Some(unit("warrior")), "off: no claim");

    let ai = armed();
    let mut on = busy.clone();
    let plan = ai.assess(&on, 0);
    assert!(
        ai.capped_aqueduct_item(&on, 0, city, &plan).is_some(),
        "fixture: claimable"
    );
    ai.claim_capped_aqueduct(&mut on, 0, &plan);
    assert_eq!(
        head(&on, city),
        Some(aqueduct.clone()),
        "the Aqueduct goes first"
    );
    let plan = ai.assess(&on, 0);
    assert!(
        ai.capped_aqueduct_holds(&on, 0, city, &plan, &aqueduct),
        "the claim holds"
    );

    let mut no_granary = busy.clone();
    no_granary
        .cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .retain(|b| *b != "granary");
    let plan = ai.assess(&no_granary, 0);
    ai.claim_capped_aqueduct(&mut no_granary, 0, &plan);
    assert_eq!(
        head(&no_granary, city),
        Some(unit("warrior")),
        "the Granary comes first"
    );
}

/// A Settler, a Builder, the faith defence's Shrine, the science and culture
/// items, a freshly attacked city and a city with room to grow keep their
/// queues.
#[test]
fn settlers_faith_science_culture_and_unbound_cities_keep_their_queues() {
    let Some((game, city, _aqueduct)) = capped_city(1, LATE) else {
        panic!("fixture: a city with a legal Aqueduct site");
    };
    let ai = armed();
    for keep in [
        unit("settler"),
        unit("builder"),
        building("shrine"),
        building("monument"),
        building("library"),
    ] {
        let mut kept = game.clone();
        queue(&mut kept, city, keep.clone());
        let plan = ai.assess(&kept, 0);
        ai.claim_capped_aqueduct(&mut kept, 0, &plan);
        assert_eq!(head(&kept, city), Some(keep.clone()), "{keep:?} stays");
    }

    let mut attacked = game.clone();
    queue(&mut attacked, city, unit("warrior"));
    let turn = attacked.turn;
    attacked.cities.get_mut(&city).unwrap().last_attacked = turn;
    let plan = ai.assess(&attacked, 0);
    ai.claim_capped_aqueduct(&mut attacked, 0, &plan);
    assert_eq!(
        head(&attacked, city),
        Some(unit("warrior")),
        "an attacked city keeps its unit"
    );

    let mut roomy = game.clone();
    queue(&mut roomy, city, unit("warrior"));
    let housing = roomy.city_housing(&roomy.cities[&city]);
    if housing >= CAPPED_AQUEDUCT_MIN_POP as f64 + 2.0 {
        roomy.cities.get_mut(&city).unwrap().pop = housing.floor() as i32 - 2;
        let plan = ai.assess(&roomy, 0);
        ai.claim_capped_aqueduct(&mut roomy, 0, &plan);
        assert_eq!(
            head(&roomy, city),
            Some(unit("warrior")),
            "room to grow: no claim"
        );
    }
}

/// At war with a major the military unit keeps its queue.
#[test]
fn at_war_a_unit_keeps_its_queue() {
    let Some((mut game, city, _aqueduct)) = capped_city(2, LATE) else {
        panic!("fixture: two majors and a city with a legal Aqueduct site");
    };
    game.at_war.insert((0, 1));
    let ai = armed();
    queue(&mut game, city, unit("warrior"));
    let plan = ai.assess(&game, 0);
    ai.claim_capped_aqueduct(&mut game, 0, &plan);
    assert_eq!(head(&game, city), Some(unit("warrior")), "a war unit stays");
}

/// The deterrent leaves the claimed Aqueduct alone: with
/// `near-rival-deterrence` armed and a neighbour at peace out-gunning us
/// inside its window, the city keeps its uninvested Aqueduct under the gene.
#[test]
fn the_deterrent_leaves_a_claimed_aqueduct_alone() {
    let Some((mut game, city, aqueduct)) = capped_city(2, DETERRENCE_START_STANDARD + 50) else {
        panic!("fixture: two majors and a city with a legal Aqueduct site");
    };
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(0, 100.0);
    std::sync::Arc::make_mut(&mut game.observed_military_power).insert(1, 200.0);
    // Uninvested, so the deterrent may displace it without the gene.
    game.cities
        .get_mut(&city)
        .unwrap()
        .production_progress
        .remove(&Game::item_progress_key(&aqueduct));
    queue(&mut game, city, aqueduct.clone());

    let mut both = armed();
    both.enable_near_rival_deterrence();
    let plan = both.assess(&game, 0);
    assert!(
        both.capped_aqueduct_holds(&game, 0, city, &plan, &aqueduct),
        "fixture: the claimed Aqueduct holds"
    );
    let mut on = game.clone();
    both.claim_deterrence_unit(&mut on, 0, &plan);
    assert_eq!(head(&on, city), Some(aqueduct), "on: the Aqueduct stays");
}

/// Registered as a version-less opt-in, off in the stock controller.
#[test]
fn the_gene_is_an_opt_in_off_by_default() {
    let gene = crate::ai::advanced::genes::GENES
        .iter()
        .find(|gene| gene.tag == "aqueduct-claims-the-capped-queue")
        .expect("registered");
    assert!(matches!(gene.kind, crate::ai::advanced::genes::Kind::OptIn));
    assert!(!AdvancedAi::new().aqueduct_claims_the_capped_queue);
}

/// A wall breaker the siege reservation queued keeps its queue: a reserved
/// Trebuchet at the head of a capped city is never deferred for the
/// Aqueduct, though a routine Warrior there is (see the first test).
#[test]
fn a_reserved_trebuchet_keeps_its_queue() {
    let Some((mut game, city, _aqueduct)) = capped_city(1, LATE) else {
        panic!("fixture: a city with a legal Aqueduct site");
    };
    queue(&mut game, city, unit("trebuchet"));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    assert!(
        ai.capped_aqueduct_item(&game, 0, city, &plan).is_some(),
        "fixture: claimable"
    );
    ai.claim_capped_aqueduct(&mut game, 0, &plan);
    assert_eq!(
        head(&game, city),
        Some(unit("trebuchet")),
        "the reserved Trebuchet is never displaced"
    );
}

fn capped_city_with_recovery() -> (Game, u32, Item) {
    let (mut game, city, aqueduct) =
        capped_city(1, LATE).expect("fixture: a capped city with a legal Aqueduct");
    game.players[0].techs.insert(crate::name!("currency"));
    queue(&mut game, city, unit("warrior"));
    let ai = armed();
    let recovery = ai
        .base
        .economic_recovery_item(&game, 0, city, ai.counts(&game, 0).traders)
        .expect("fixture: an ordinary recovery build is available");
    assert_ne!(recovery, aqueduct);
    assert!(game.can_produce(0, city, &recovery));
    assert!(game.can_produce(0, city, &aqueduct));
    (game, city, aqueduct)
}

#[test]
fn a_depleted_capped_city_uses_its_available_recovery_build() {
    let (base, city, _) = capped_city_with_recovery();
    let ai = armed();
    let mut outcomes = Vec::new();
    for (gold, income) in [(33.0, -0.546875), (11.0, -10.8438), (0.0, -11.8438)] {
        let mut game = base.clone();
        game.players[0].gold = gold;
        game.players[0].gold_per_turn = income;
        let plan = ai.assess(&game, 0);
        assert!(ai.capped_aqueduct_item(&game, 0, city, &plan).is_some());
        let recovery = ai
            .base
            .economic_recovery_item(&game, 0, city, ai.counts(&game, 0).traders)
            .expect("fixture: this budget still has a recovery choice");
        assert!(game.can_produce(0, city, &recovery));
        eprintln!("qualified capped recovery budget {gold}/{income}: {recovery:?}");
        ai.claim_capped_aqueduct(&mut game, 0, &plan);
        outcomes.push((head(&game, city), Some(recovery)));
    }
    assert!(
        outcomes.iter().all(|(actual, expected)| actual == expected),
        "all three depleted budgets use the available recovery choice: {outcomes:?}"
    );
}

#[test]
fn funded_and_boundary_capped_cities_keep_their_housing_build() {
    let (base, city, aqueduct) = capped_city_with_recovery();
    let reserve = 100.0 + 25.0 * base.player_city_ids(0).len() as f64;
    let ai = armed();
    for (gold, income) in [
        (reserve, -11.8438),
        (250.0, -11.8438),
        (0.0, -0.5),
        (0.0, 0.0),
    ] {
        let mut game = base.clone();
        game.players[0].gold = gold;
        game.players[0].gold_per_turn = income;
        let plan = ai.assess(&game, 0);
        assert!(ai
            .base
            .economic_recovery_item(&game, 0, city, ai.counts(&game, 0).traders)
            .is_some());
        ai.claim_capped_aqueduct(&mut game, 0, &plan);
        assert_eq!(head(&game, city), Some(aqueduct.clone()), "{gold}/{income}");
        assert!(ai.capped_aqueduct_holds(&game, 0, city, &plan, &aqueduct));
    }
}

#[test]
fn a_depleted_capped_city_keeps_housing_without_a_recovery_option() {
    let (mut game, city, aqueduct) =
        capped_city(1, LATE).expect("fixture: a capped city with a legal Aqueduct");
    let ai = armed();
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -11.8438;
    queue(&mut game, city, unit("warrior"));
    assert!(ai
        .base
        .economic_recovery_item(&game, 0, city, ai.counts(&game, 0).traders)
        .is_none());
    let plan = ai.assess(&game, 0);
    assert!(ai.capped_aqueduct_item(&game, 0, city, &plan).is_some());
    ai.claim_capped_aqueduct(&mut game, 0, &plan);
    assert_eq!(head(&game, city), Some(aqueduct.clone()));
    assert!(ai.capped_aqueduct_holds(&game, 0, city, &plan, &aqueduct));
}

#[test]
fn a_depleted_aqueduct_hold_yields_to_an_available_recovery_build() {
    let (mut game, city, aqueduct) = capped_city_with_recovery();
    let ai = armed();
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -11.8438;
    queue(&mut game, city, aqueduct.clone());
    let plan = ai.assess(&game, 0);
    assert!(ai.capped_aqueduct_item(&game, 0, city, &plan).is_some());
    assert!(ai
        .base
        .economic_recovery_item(&game, 0, city, ai.counts(&game, 0).traders)
        .is_some());
    assert!(!ai.capped_aqueduct_holds(&game, 0, city, &plan, &aqueduct));
}

#[test]
fn an_inactive_capped_reservation_leaves_the_recovery_queue_alone() {
    let (mut game, city, aqueduct) = capped_city_with_recovery();
    let ai = AdvancedAi::new();
    game.players[0].gold = 0.0;
    game.players[0].gold_per_turn = -11.8438;
    assert!(ai
        .base
        .economic_recovery_item(&game, 0, city, ai.counts(&game, 0).traders)
        .is_some());
    let plan = ai.assess(&game, 0);
    ai.claim_capped_aqueduct(&mut game, 0, &plan);
    assert_eq!(head(&game, city), Some(unit("warrior")));
    assert!(!ai.capped_aqueduct_holds(&game, 0, city, &plan, &aqueduct));
}

fn capped_city_with_due_library_and_recovery() -> (Game, u32, Item) {
    let (mut game, city, aqueduct) = capped_city_with_recovery();
    let mut ai = armed();
    ai.enable_campus_building_claims_the_queue();
    game.players[0].techs.insert(crate::name!("writing"));
    game.cities.get_mut(&city).unwrap().pop = 7;
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        city,
        crate::rules::Yields {
            production: 12.0,
            ..Default::default()
        },
    );
    let old_recovery = ai
        .base
        .economic_recovery_item(&game, 0, city, ai.counts(&game, 0).traders)
        .expect("fixture: a recovery build before installing the Campus");
    let Item::District {
        pos: aqueduct_pos, ..
    } = aqueduct
    else {
        panic!("fixture: Aqueduct district");
    };
    let Item::District {
        pos: recovery_pos, ..
    } = old_recovery
    else {
        panic!("fixture: Commercial Hub recovery");
    };
    let campus = crate::name!("campus");
    let campus_pos = game
        .district_sites(city, campus)
        .into_iter()
        .filter(|pos| *pos != aqueduct_pos && *pos != recovery_pos)
        .find(|pos| {
            game.can_produce(
                0,
                city,
                &Item::District {
                    district: campus,
                    pos: *pos,
                },
            )
        })
        .expect("fixture: a separate legal Campus site");
    game.map.tiles.get_mut(&campus_pos).unwrap().district = Some(campus);
    game.cities
        .get_mut(&city)
        .unwrap()
        .districts
        .insert(campus, campus_pos);
    game.players[0].gold = 105.0;
    game.players[0].gold_per_turn = -0.746094;
    let plan = ai.assess(&game, 0);
    assert!(ai.capped_aqueduct_item(&game, 0, city, &plan).is_some());
    let recovery = ai
        .base
        .economic_recovery_item(&game, 0, city, ai.counts(&game, 0).traders)
        .expect("fixture: recovery remains legal with the standing Campus");
    assert!(matches!(&recovery, Item::District { district, .. } if *district == "commercial_hub"));
    assert_eq!(
        BasicAi::campus_building_item(&game, 0, city),
        Some(building("library"))
    );
    (game, city, recovery)
}

#[test]
fn the_campus_claim_keeps_a_capped_citys_recovery_reservation() {
    let (mut game, city, recovery) = capped_city_with_due_library_and_recovery();
    let mut ai = armed();
    ai.enable_campus_building_claims_the_queue();
    let plan = ai.assess(&game, 0);
    eprintln!("qualified capped recovery handoff 105/-0.746094: {recovery:?}; Library legal");
    ai.claim_capped_aqueduct(&mut game, 0, &plan);
    assert_eq!(head(&game, city), Some(recovery.clone()));
    ai.claim_campus_building(&mut game, 0, &plan);
    assert_eq!(
        head(&game, city),
        Some(recovery),
        "the recovery replacement keeps the capped-city reservation"
    );
}

#[test]
fn an_unrecorded_recovery_queue_does_not_inherit_the_capped_reservation() {
    let (mut game, city, recovery) = capped_city_with_due_library_and_recovery();
    let mut ai = armed();
    ai.enable_campus_building_claims_the_queue();
    let plan = ai.assess(&game, 0);
    assert!(game
        .apply(
            0,
            &Action::Produce {
                city,
                item: recovery.clone()
            }
        )
        .is_ok());
    assert!(!ai.capped_aqueduct_holds(&game, 0, city, &plan, &recovery));
    ai.claim_campus_building(&mut game, 0, &plan);
    assert_eq!(head(&game, city), Some(building("library")));
}

#[test]
fn a_capped_recovery_reservation_releases_when_its_budget_recovers() {
    let (base, city, recovery) = capped_city_with_due_library_and_recovery();
    let reserve = 100.0 + 25.0 * base.player_city_ids(0).len() as f64;
    for (gold, income) in [(reserve, -0.746094), (105.0, -0.5)] {
        let mut game = base.clone();
        let mut ai = armed();
        ai.enable_campus_building_claims_the_queue();
        let plan = ai.assess(&game, 0);
        ai.claim_capped_aqueduct(&mut game, 0, &plan);
        assert_eq!(head(&game, city), Some(recovery.clone()));
        game.players[0].gold = gold;
        game.players[0].gold_per_turn = income;
        assert!(!ai.capped_aqueduct_holds(&game, 0, city, &plan, &recovery));
        ai.claim_campus_building(&mut game, 0, &plan);
        assert_eq!(
            head(&game, city),
            Some(building("library")),
            "{gold}/{income}"
        );
    }
}
