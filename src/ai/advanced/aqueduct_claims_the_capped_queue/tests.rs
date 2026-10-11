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
