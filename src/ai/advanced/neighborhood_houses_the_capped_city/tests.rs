use super::*;

/// Our one city on the first land tile with a legal Neighborhood site,
/// Urbanization adopted, its Aqueduct already standing (so the cheaper fix is
/// spent), every unit removed, the population at its housing, and Production
/// banked on every Neighborhood site so the claim finishes inside the cap.
/// Returns the game and our city.
fn capped_city() -> Option<(Game, u32)> {
    for seed in [936_231_u64, 424_243, 777_011, 31_339] {
        let mut base = Game::new_full(1, 32, 20, seed, 200, 0, false);
        for uid in base.player_unit_ids(0) {
            base.remove_unit(uid);
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
            game.players[0].civics.insert(crate::name!("urbanization"));
            let neighborhood = BasicAi::civ_district(&game, 0, "neighborhood");
            let legal: Vec<Pos> = game
                .district_sites(ours, neighborhood)
                .into_iter()
                .filter(|pos| {
                    game.can_produce(
                        0,
                        ours,
                        &Item::District {
                            district: neighborhood,
                            pos: *pos,
                        },
                    )
                })
                .collect();
            if legal.is_empty() {
                continue;
            }
            game.turn = game.standard_duration(170);
            let aqueduct = BasicAi::civ_district(&game, 0, "aqueduct");
            let center = game.cities[&ours].pos;
            let city = game.cities.get_mut(&ours).unwrap();
            city.districts.insert(aqueduct, center);
            city.queue.clear();
            for pos in &legal {
                let item = Item::District {
                    district: neighborhood,
                    pos: *pos,
                };
                city.production_progress
                    .insert(Game::item_progress_key(&item), 50.0);
            }
            let housing = game.city_housing(&game.cities[&ours]);
            game.cities.get_mut(&ours).unwrap().pop =
                (housing.ceil() as i32).max(CAPPED_NEIGHBORHOOD_MIN_POP);
            return Some((game, ours));
        }
    }
    None
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_neighborhood_houses_the_capped_city();
    ai
}

fn head(game: &Game, cid: u32) -> Option<Item> {
    game.cities[&cid].queue.first().cloned()
}

fn warrior() -> Item {
    Item::Unit {
        unit: crate::name!("warrior"),
    }
}

#[test]
fn a_capped_city_claims_its_best_housing_neighborhood() {
    let (game, city) = capped_city().expect("fixture: a legal Neighborhood site");
    let mut busy = game.clone();
    busy.cities.get_mut(&city).unwrap().queue = vec![warrior()];

    let stock = AdvancedAi::new();
    let mut off = busy.clone();
    let plan = stock.assess(&off, 0);
    assert!(stock.capped_neighborhood_item(&off, 0, city, &plan).is_none());
    stock.claim_capped_neighborhood(&mut off, 0, &plan);
    assert_eq!(head(&off, city), Some(warrior()), "off: no claim");

    let ai = armed();
    let mut on = busy.clone();
    let plan = ai.assess(&on, 0);
    let claim = ai
        .capped_neighborhood_item(&on, 0, city, &plan)
        .expect("fixture: claimable");
    // The site that houses most.
    let neighborhood = BasicAi::civ_district(&on, 0, "neighborhood");
    let best = on
        .district_sites(city, neighborhood)
        .into_iter()
        .filter(|pos| {
            on.can_produce(
                0,
                city,
                &Item::District {
                    district: neighborhood,
                    pos: *pos,
                },
            )
        })
        .map(|pos| on.district_housing(neighborhood.as_str(), pos))
        .fold(f64::MIN, f64::max);
    assert_eq!(claim.housing, best);
    ai.claim_capped_neighborhood(&mut on, 0, &plan);
    assert_eq!(head(&on, city), Some(claim.item.clone()), "the Neighborhood goes first");
    let plan = ai.assess(&on, 0);
    assert!(ai.capped_neighborhood_holds(&on, 0, city, &plan, &claim.item));
}

#[test]
fn an_unbuilt_aqueduct_room_to_grow_or_a_fresh_attack_keeps_the_queue() {
    let (game, city) = capped_city().expect("fixture: a legal Neighborhood site");
    let ai = armed();

    let mut roomy = game.clone();
    let housing = roomy.city_housing(&roomy.cities[&city]);
    roomy.cities.get_mut(&city).unwrap().pop = (housing - 2.0).floor() as i32;
    let plan = ai.assess(&roomy, 0);
    assert!(
        ai.capped_neighborhood_item(&roomy, 0, city, &plan).is_none(),
        "two housing to spare grows at full rate"
    );

    let mut attacked = game.clone();
    let turn = attacked.turn;
    attacked.cities.get_mut(&city).unwrap().last_attacked = turn;
    let plan = ai.assess(&attacked, 0);
    assert!(ai.capped_neighborhood_item(&attacked, 0, city, &plan).is_none());

    let mut small = game.clone();
    small.cities.get_mut(&city).unwrap().pop = CAPPED_NEIGHBORHOOD_MIN_POP - 1;
    let plan = ai.assess(&small, 0);
    assert!(ai.capped_neighborhood_item(&small, 0, city, &plan).is_none());

    let mut early = game.clone();
    early.players[0].civics.retain(|civic| *civic != "urbanization");
    let plan = ai.assess(&early, 0);
    assert!(
        ai.capped_neighborhood_item(&early, 0, city, &plan).is_none(),
        "no Neighborhood before Urbanization"
    );
}

#[test]
fn one_neighborhood_in_flight_per_five_cities() {
    let (mut game, city) = capped_city().expect("fixture: a legal Neighborhood site");
    let ai = armed();
    let plan = ai.assess(&game, 0);
    let claim = ai
        .capped_neighborhood_item(&game, 0, city, &plan)
        .expect("fixture: claimable");
    // One city, one Neighborhood already queued behind the Warrior.
    game.cities.get_mut(&city).unwrap().queue = vec![warrior(), claim.item.clone()];
    ai.claim_capped_neighborhood(&mut game, 0, &plan);
    assert_eq!(head(&game, city), Some(warrior()), "the one in flight is the cap");
}
