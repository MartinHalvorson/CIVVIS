use super::*;

/// Two majors at war, one founded city each, our seat on the Domination lane
/// with a Conquest plan on the rival's city. That city stands behind
/// Renaissance Walls (300, no Ram or Tower works on them) at a strength of
/// `defense`; we know Engineering (the Catapult) and Masonry, and field no
/// unit. Returns the game, the seat, its plan, our city and the target.
fn walled_war(defense: f64) -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new_full(2, 40, 26, 79_301, 500, 0, false);
    g.current = 0;
    let settlers: Vec<_> = g
        .units
        .values()
        .filter(|u| u.kind == "settler")
        .map(|u| (u.owner, u.pos))
        .collect();
    for (owner, pos) in settlers {
        g.found_city_for(owner, pos, None);
    }
    let home = g.player_city_ids(0)[0];
    let target = g.player_city_ids(1)[0];
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.players[0]
        .techs
        .extend(["archery", "masonry", "engineering"].map(Name::new));
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 30.0;
    {
        let city = g.cities.get_mut(&target).unwrap();
        city.buildings
            .extend(["walls", "medieval_walls", "renaissance_walls"].map(Name::new));
        city.wall_hp = 300;
    }
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(target, defense);
    g.turn = 125;
    g.at_war.insert((0, 1));
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, home, target)
}

fn armed(mut ai: AdvancedAi) -> AdvancedAi {
    ai.enable_hopeless_army_yields_the_queue();
    ai
}

fn head_is_siege(g: &Game, cid: u32) -> bool {
    matches!(
        g.cities[&cid].queue.first(),
        Some(Item::Unit { unit }) if g.rules.units[unit].siege
    )
}

/// The reading: a Catapult (35) against a city of 70 strikes ~7.4 a shot,
/// and six take ~16 turns to breach 300 walls and take the city, so the
/// walls are hopeless; against 50 (~16.5 a shot) four finish in ~10, and
/// with Military Engineering's Trebuchet (45, ~11.0) five finish in ~12, so
/// neither is; breached walls never are; with no gun at all they are.
#[test]
fn the_reading_names_only_walls_nothing_we_can_field_opens() {
    let (g, _, _, _, target) = walled_war(70.0);
    let read = AdvancedAi::walls_nothing_breaches(&g, 0, target).expect("catapults cannot");
    assert_eq!((read.walls, read.city), (300, target));
    let hit = read.hit.expect("a buildable catapult");
    assert!(
        (hit - 30.0 * (-35.0_f64 / 25.0).exp()).abs() < 1e-9,
        "{hit}"
    );

    let (g, _, _, _, target) = walled_war(50.0);
    assert_eq!(AdvancedAi::walls_nothing_breaches(&g, 0, target), None);

    let (mut g, _, _, _, target) = walled_war(70.0);
    g.players[0]
        .techs
        .insert(crate::name!("military_engineering"));
    assert_eq!(
        AdvancedAi::walls_nothing_breaches(&g, 0, target),
        None,
        "the right-tier gun makes the siege viable"
    );

    let (mut g, _, _, _, target) = walled_war(70.0);
    g.cities.get_mut(&target).unwrap().wall_hp = 0;
    assert_eq!(AdvancedAi::walls_nothing_breaches(&g, 0, target), None);

    let (mut g, _, _, _, target) = walled_war(50.0);
    g.players[0].techs.remove(&crate::name!("engineering"));
    let read = AdvancedAi::walls_nothing_breaches(&g, 0, target).expect("no gun at all");
    assert_eq!(read.hit, None);
}

/// The walled-assault reservation: off, the idle city lays a Catapult down
/// for Gondar-like walls; on, nothing is reserved; on, against walls the
/// Catapult opens, the reservation stands.
#[test]
fn hopeless_walls_reserve_no_gun() {
    let (mut off, ai, plan, home, _) = walled_war(70.0);
    assert!(ai
        .reserve_delegated_domination_siege(&mut off, 0, &plan)
        .is_some());
    assert!(head_is_siege(&off, home), "off: the gun is reserved");

    let (mut on, ai, plan, home, _) = walled_war(70.0);
    let ai = armed(ai);
    assert!(ai.hopeless_offensive(&on, 0, &plan).is_some());
    assert!(ai
        .reserve_delegated_domination_siege(&mut on, 0, &plan)
        .is_none());
    assert!(on.cities[&home].queue.is_empty(), "on: the queue is left");

    let (mut viable, ai, plan, home, _) = walled_war(50.0);
    let ai = armed(ai);
    assert!(ai
        .reserve_delegated_domination_siege(&mut viable, 0, &plan)
        .is_some());
    assert!(
        head_is_siege(&viable, home),
        "a gun that breaches is reserved"
    );
}

/// A displaced gun's queue: off, it is taken back from the Monument; on, the
/// Monument keeps it while the walls are hopeless.
#[test]
fn hopeless_walls_take_no_queue_back() {
    let catapult = Item::Unit {
        unit: crate::name!("catapult"),
    };
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for gene in [false, true] {
        let (mut g, ai, plan, home, _) = walled_war(70.0);
        let ai = if gene { armed(ai) } else { ai };
        g.cities.get_mut(&home).unwrap().queue = vec![monument.clone()];
        ai.restore_wall_breaker_queues(&mut g, 0, &plan, &[(home, catapult.clone())], None);
        let expected = if gene { &monument } else { &catapult };
        assert_eq!(g.cities[&home].queue.first(), Some(expected), "gene {gene}");
    }
}

/// `domination-target-gets-its-breaker` lays no breaker down for walls
/// nothing we field opens.
#[test]
fn hopeless_walls_claim_no_breaker() {
    for gene in [false, true] {
        let (mut g, mut ai, plan, home, _) = walled_war(70.0);
        ai.enable_domination_target_gets_its_breaker();
        let ai = if gene { armed(ai) } else { ai };
        let claim = ai.claim_domination_breaker(&mut g, 0, &plan);
        assert_eq!(claim.is_some(), !gene, "gene {gene}");
        assert_eq!(head_is_siege(&g, home), !gene, "gene {gene}");
    }
}

/// The scorer: at war on hopeless walls, the land army is priced at one
/// garrison a city, so a Warrior is worth less than off; the same city under
/// threat (named by the plan, or attacked last turn) prices it as off.
#[test]
fn a_hopeless_siege_buys_no_body_but_a_threatened_city_still_does() {
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let (mut g, off, plan, home, _) = walled_war(70.0);
    let pos = g.cities[&home].pos;
    g.spawn_unit("archer", 0, pos);
    let on = armed(off.clone());
    let counts = off.counts(&g, 0);

    assert_eq!(on.hopeless_army_target(&g, 0, &plan, false, 1, 4), 1);
    assert_eq!(off.hopeless_army_target(&g, 0, &plan, false, 1, 4), 4);
    assert_eq!(on.hopeless_army_target(&g, 0, &plan, true, 1, 4), 4);

    let value_off = off.production_value(&g, 0, home, &warrior, &plan, &counts);
    let value_on = on.production_value(&g, 0, home, &warrior, &plan, &counts);
    assert!(value_on < value_off, "on {value_on} off {value_off}");

    let threatened = StrategicPlan {
        threatened_city: Some(home),
        ..plan.clone()
    };
    assert_eq!(
        on.production_value(&g, 0, home, &warrior, &threatened, &counts),
        off.production_value(&g, 0, home, &warrior, &threatened, &counts),
        "a threatened city keeps its army target"
    );
    let mut attacked = g.clone();
    attacked.cities.get_mut(&home).unwrap().last_attacked = attacked.turn - 1;
    assert_eq!(
        on.production_value(&attacked, 0, home, &warrior, &plan, &counts),
        off.production_value(&attacked, 0, home, &warrior, &plan, &counts),
        "a city under fire keeps its army target"
    );

    let (g, off, plan, home, _) = walled_war(50.0);
    let on = armed(off.clone());
    let counts = off.counts(&g, 0);
    assert_eq!(
        on.production_value(&g, 0, home, &warrior, &plan, &counts),
        off.production_value(&g, 0, home, &warrior, &plan, &counts),
        "breachable walls keep the offensive's target"
    );
}

/// The taker arm (live Deity civvis-20261112T043248Z, Smolensk): Ancient
/// Walls of 100 and a buildable Ram, no gun, and a city strength 30 over our
/// best melee taker: the city's return blow removes the taker, so the walls
/// are hopeless; the same board with a Knight in the field (a taker within
/// `ONE_BLOW_GAP` of the city) is not.
#[test]
fn a_ram_opens_nothing_for_a_taker_the_city_removes() {
    let (mut g, _, plan, _, target) = walled_war(50.0);
    g.players[0].techs.remove(&crate::name!("engineering"));
    {
        let city = g.cities.get_mut(&target).unwrap();
        city.buildings.retain(|building| {
            !matches!(building.as_str(), "medieval_walls" | "renaissance_walls")
        });
        city.wall_hp = 100;
    }
    assert!(g.city_allows_siege_support(target, "battering_ram"));
    assert!(g.can_produce(
        0,
        g.player_city_ids(0)[0],
        &Item::Unit {
            unit: crate::name!("battering_ram")
        }
    ));
    let best = AdvancedAi::best_melee_taker(&g, 0).expect("a buildable melee unit");
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(target, best + 30.0);

    let read = AdvancedAi::walls_nothing_breaches(&g, 0, target).expect("the taker dies");
    assert!(read.breach_support);
    assert_eq!((read.hit, read.taker), (None, Some(best)));
    assert!(
        read.describe().contains("removes the taker"),
        "{}",
        read.describe()
    );
    let ai = armed(AdvancedAi::targeting(VictoryTarget::Domination));
    assert!(ai.hopeless_offensive(&g, 0, &plan).is_some());

    let mut knight = g.clone();
    let pos = knight.cities[&knight.player_city_ids(0)[0]].pos;
    knight.spawn_unit("knight", 0, pos);
    assert!(AdvancedAi::best_melee_taker(&knight, 0).is_some_and(|taker| taker > best + 5.0));
    assert_eq!(
        AdvancedAi::walls_nothing_breaches(&knight, 0, target),
        None,
        "a taker within reach of the city makes the Ram's breach viable"
    );
    assert!(ai.hopeless_offensive(&knight, 0, &plan).is_none());
}

/// On another lane the gene reads nothing.
#[test]
fn another_lane_reads_nothing() {
    let (g, _, plan, _, _) = walled_war(70.0);
    let mut ai = armed(AdvancedAi::targeting(VictoryTarget::Science));
    assert!(ai.hopeless_offensive(&g, 0, &plan).is_none());
    ai.disable_hopeless_army_yields_the_queue();
    assert!(ai.hopeless_offensive(&g, 0, &plan).is_none());
}
