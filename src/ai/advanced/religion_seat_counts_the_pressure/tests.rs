use super::*;
use crate::game::ObservedCityFaith;

fn rows(pop: i32, own: f64, others: &[f64], none: f64) -> FlipRows {
    FlipRows {
        pop,
        own,
        others: others.to_vec(),
        none,
    }
}

/// City-turns the host showed in civvis-20261110T031523Z, our faith Buddhism:
/// its `GetMajorityReligion()` beside the rows' lifetime pressure.
#[test]
fn a_flip_holds_exactly_where_the_host_showed_a_majority() {
    // Sedeinga t31, t32: one Citizen of two.
    assert!(!rows(2, 80.0, &[], 100.0).holds());
    assert!(rows(2, 100.0, &[], 100.0).holds());
    // Astrakhan t33: level on followers with Orthodoxy, the greater pressure.
    assert!(rows(2, 294.0, &[90.0], 50.0).holds());
    // Jerusalem t36, t39: two of four either side of the religionless half.
    assert!(!rows(4, 148.0, &[], 200.0).holds());
    assert!(rows(4, 202.0, &[24.0], 200.0).holds());
    // Meroe t39: three of six.
    assert!(rows(6, 470.0, &[24.0], 350.0).holds());
    // Solikamsk t46: 52.6% of the pressure seats one Citizen of three.
    assert!(!rows(3, 198.0, &[78.0], 100.0).holds());
    // Jerusalem t45: two of five.
    assert!(!rows(5, 242.0, &[74.0, 10.0], 200.0).holds());
    // Kumasi t48: Orthodoxy out-presses us.
    assert!(!rows(6, 256.0, &[308.0], 300.0).holds());
    // No pressure of ours is no majority, whatever the rest.
    assert!(!rows(4, 0.0, &[], 200.0).holds());
}

/// One charge takes a young city, an entrenched one takes many, and eviction
/// is what shortens it: Missionary 200 and 10%, Apostle 220 and 25%, a
/// Proselytizer's 50%.
#[test]
fn charges_follow_the_spread_arithmetic() {
    // Two Citizens following none: 200 against 100 seats one each, and the
    // religionless half does not out-press us.
    assert_eq!(rows(2, 0.0, &[], 100.0).charges(200.0, 0.10), 1);
    // Already ours costs nothing.
    assert_eq!(rows(6, 470.0, &[24.0], 350.0).charges(200.0, 0.10), 0);

    let entrenched = rows(8, 0.0, &[3_000.0], 400.0);
    let missionary = entrenched.charges(200.0, 0.10);
    let apostle = entrenched.charges(220.0, 0.25);
    let proselytizer = entrenched.charges(220.0, 0.50);
    assert!(missionary > apostle && apostle > proselytizer);
    assert!(missionary > 5, "{missionary}");
    assert!(proselytizer <= 4, "{proselytizer}");

    // Past the cap reads as out of reach, not as a number to plan on.
    assert_eq!(
        rows(10, 0.0, &[60_000.0], 500.0).charges(200.0, 0.10),
        CHARGE_CAP + 1
    );
}

/// A founder beside a rival of three cities the host has shown rows for, with
/// one full-health Missionary. Returns the game, the Missionary and the
/// rival's cities ordered young, middling, entrenched.
fn founder() -> (Game, u32, Vec<u32>) {
    let mut game = Game::new_full(2, 40, 24, 733_100, 250, 0, false);
    for uid in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(uid);
    }
    game.current = 0;
    let mut land: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| {
            game.rules.is_passable(tile) && !game.rules.is_water(tile) && tile.owner_city.is_none()
        })
        .map(|(pos, _)| *pos)
        .collect();
    land.sort();
    let mut sites: Vec<Pos> = Vec::new();
    for pos in land {
        if sites.len() == 4 {
            break;
        }
        if sites.iter().all(|site| game.wdist(pos, *site) >= 4) {
            sites.push(pos);
        }
    }
    assert_eq!(sites.len(), 4, "fixture: room for four cities");
    let home = game.place_city(0, sites[0], None);
    let rival: Vec<u32> = sites[1..]
        .iter()
        .map(|site| game.place_city(1, *site, None))
        .collect();
    game.players[0].religion = Some("Buddhism".to_string());
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    let faith = |religion: Option<&str>, pressure: f64| ObservedCityFaith {
        religion: religion.map(str::to_string),
        followers: 0,
        pressure,
        per_turn: 0.0,
    };
    let shown = std::sync::Arc::make_mut(&mut game.observed_city_faiths);
    for (cid, pop, hinduism) in [
        (rival[0], 2, 0.0),
        (rival[1], 4, 700.0),
        (rival[2], 9, 4_000.0),
    ] {
        let mut rows = vec![faith(None, 50.0 * f64::from(pop))];
        if hinduism > 0.0 {
            rows.push(faith(Some("Hinduism"), hinduism));
        }
        shown.insert(cid, rows);
    }
    for (cid, pop) in [(rival[0], 2), (rival[1], 4), (rival[2], 9)] {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = pop;
        city.pressure.clear();
    }
    let pos = game.cities[&home].pos;
    let missionary = game.spawn_test_unit("missionary", 0, pos);
    {
        let unit = game.units.get_mut(&missionary).unwrap();
        unit.religion = Some("Buddhism".to_string());
        unit.charges = 3;
        unit.hp = 100;
    }
    (game, missionary, rival)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_religion_seat_counts_the_pressure();
    }
    ai
}

/// ★ A rival of three cities owes our majority two. The two cheapest are its
/// campaign with one to spare, so all three are in it here; the price falls
/// with the charges each needs and the shipped population weight is gone.
#[test]
fn the_plan_pays_the_cheapest_cities_a_rival_still_owes() {
    let (game, missionary, rival) = founder();
    let on = seat(VictoryTarget::Religion, true);
    let plan = on.religion_seat_pressure_plan(&game, 0, missionary, "Buddhism");
    assert_eq!(plan.len(), 3);

    let needed = |cid: u32| {
        FlipRows::observed(&game, &game.cities[&cid], "Buddhism")
            .unwrap()
            .charges(200.0, 0.10)
    };
    assert_eq!(needed(rival[0]), 1);
    assert!(needed(rival[1]) > needed(rival[0]));
    assert!(needed(rival[2]) > needed(rival[1]));

    let shipped =
        |cid: u32| game.cities[&cid].pop * 12 + i32::from(game.cities[&cid].is_capital) * 18;
    let young = plan[&rival[0]] + shipped(rival[0]);
    let middling = plan[&rival[1]] + shipped(rival[1]);
    let entrenched = plan[&rival[2]] + shipped(rival[2]);
    assert_eq!(young, CAMPAIGN_BONUS - CHARGE_COST + SOLO_FLIP_BONUS);
    assert!(young > middling && middling > entrenched);
    // The entrenched city of nine reads below the young city of two even
    // after the shipped score pays it for its size.
    assert!(plan[&rival[2]] + 9 * 12 < plan[&rival[0]] + 2 * 12);
}

/// Once a rival owes nothing more, its campaign is the one spare: the
/// cheapest open city stays a target and the dearest is charged.
#[test]
fn a_city_outside_the_campaign_is_charged() {
    let (mut game, missionary, rival) = founder();
    let hold = |game: &mut Game, cid: u32| {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = 3;
        city.atheist_pressure = 0.0;
        city.pressure.clear();
        city.pressure.insert("Buddhism".to_string(), 100.0);
    };
    // Two more rival cities already ours by the host's majority, and the
    // middling one converted: three of five held.
    for _ in 0..2 {
        let site = game
            .map
            .tiles
            .iter()
            .filter(|(pos, tile)| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(**pos, city.pos) >= 4)
            })
            .map(|(pos, _)| *pos)
            .min()
            .expect("fixture: room for another city");
        let cid = game.place_city(1, site, None);
        hold(&mut game, cid);
    }
    hold(&mut game, rival[1]);
    let on = seat(VictoryTarget::Religion, true);
    let plan = on.religion_seat_pressure_plan(&game, 0, missionary, "Buddhism");
    assert_eq!(plan.len(), 2, "a city that is ours is not priced");
    let shipped =
        |cid: u32| game.cities[&cid].pop * 12 + i32::from(game.cities[&cid].is_capital) * 18;
    assert_eq!(
        plan[&rival[0]] + shipped(rival[0]),
        CAMPAIGN_BONUS - CHARGE_COST + SOLO_FLIP_BONUS,
        "the spare is the cheapest open city"
    );
    assert_eq!(plan[&rival[2]] + shipped(rival[2]), -OUTSIDE_CAMPAIGN);

    // A campaign city at the cap still reads above one outside it.
    assert!(CAMPAIGN_BONUS - CHARGE_CAP as i32 * CHARGE_COST > -OUTSIDE_CAMPAIGN);
}

/// Off, on another target, for another faith's spreader, or with no rows,
/// the plan is empty and the shipped score stands.
#[test]
fn the_plan_is_empty_without_the_gene_the_target_the_faith_or_the_rows() {
    let (game, missionary, _) = founder();
    assert!(seat(VictoryTarget::Religion, false)
        .religion_seat_pressure_plan(&game, 0, missionary, "Buddhism")
        .is_empty());
    assert!(seat(VictoryTarget::Culture, true)
        .religion_seat_pressure_plan(&game, 0, missionary, "Buddhism")
        .is_empty());
    assert!(seat(VictoryTarget::Religion, true)
        .religion_seat_pressure_plan(&game, 0, missionary, "Hinduism")
        .is_empty());
    let mut bare = game.clone();
    std::sync::Arc::make_mut(&mut bare.observed_city_faiths).clear();
    assert!(seat(VictoryTarget::Religion, true)
        .religion_seat_pressure_plan(&bare, 0, missionary, "Buddhism")
        .is_empty());
}

/// A Translator's charge is worth three abroad, so the entrenched city costs
/// it fewer charges than it costs an unpromoted Apostle, and each charge
/// saved is paid for: the promoted unit is drawn to where its promotion
/// counts.
#[test]
fn a_promoted_apostle_is_paid_for_the_charges_it_saves() {
    let (mut game, _, rival) = founder();
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
    let mut apostle = |game: &mut Game, promotion: Option<&str>| {
        let uid = game.spawn_test_unit("apostle", 0, home);
        let unit = game.units.get_mut(&uid).unwrap();
        unit.religion = Some("Buddhism".to_string());
        unit.charges = 3;
        unit.hp = 100;
        if let Some(promotion) = promotion {
            unit.promotions.insert(Name::new(promotion));
        }
        uid
    };
    let plain = apostle(&mut game, None);
    let translator = apostle(&mut game, Some("translator"));
    let on = seat(VictoryTarget::Religion, true);
    let rows = FlipRows::observed(&game, &game.cities[&rival[2]], "Buddhism").unwrap();
    let reference = rows.charges(220.0, 0.25);
    let translated = rows.charges(660.0, 0.25);
    assert!(translated < reference, "{translated} against {reference}");

    let shipped =
        |cid: u32| game.cities[&cid].pop * 12 + i32::from(game.cities[&cid].is_capital) * 18;
    let price = |uid: u32| {
        on.religion_seat_pressure_plan(&game, 0, uid, "Buddhism")[&rival[2]] + shipped(rival[2])
    };
    let solo = |needed: u32| if needed <= 3 { SOLO_FLIP_BONUS } else { 0 };
    assert_eq!(
        price(plain),
        CAMPAIGN_BONUS - reference as i32 * CHARGE_COST + solo(reference)
    );
    assert_eq!(
        price(translator),
        CAMPAIGN_BONUS - translated as i32 * CHARGE_COST
            + solo(translated)
            + (reference - translated) as i32 * ADVANTAGE_BONUS
    );
    assert!(price(translator) > price(plain));
}
