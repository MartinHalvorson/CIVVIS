use super::super::*;
use super::{GunResourceWant, GUN_RESOURCE_FUEL_LOW_TURNS, GUN_RESOURCE_FUEL_TURNS};

/// Two seats at war, our Domination campaign aimed at their city, walled 300
/// at a city strength of `defense`. We know Metal Casting, so the Bombard (55,
/// 20 Niter) is unlocked over the Trebuchet (45), and we hold no Niter and
/// earn none. None of our units stands near the target.
fn niter_case(defense: f64) -> (Game, AdvancedAi, u32, u32) {
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
    g.players[0].techs.extend(
        [
            "archery",
            "masonry",
            "engineering",
            "mining",
            "bronze_working",
            "military_engineering",
            "metal_casting",
        ]
        .map(crate::name::Name::new),
    );
    for building in ["walls", "medieval_walls", "renaissance_walls"] {
        g.cities
            .get_mut(&target)
            .unwrap()
            .buildings
            .push(crate::name::Name::new(building));
    }
    g.cities.get_mut(&target).unwrap().wall_hp = 300;
    std::sync::Arc::make_mut(&mut g.observed_city_strength).insert(target, defense);
    g.turn = 150;
    g.at_war.insert((0, 1));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.plan = Some(StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: Some(target),
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    });
    (g, ai, home, target)
}

fn niter(want: &GunResourceWant) -> (&str, &str, u32, u32, usize, bool) {
    (
        want.resource.as_str(),
        want.unit.as_str(),
        want.amount,
        want.minimum,
        want.guns,
        want.renewal,
    )
}

/// Off, nothing is wanted.
#[test]
fn off_wants_nothing() {
    let (g, ai, _, _) = niter_case(50.0);
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty());
}

/// The Trebuchet is what we can build; the Bombard waits on 20 Niter a gun.
/// At a city strength of 50 the shipped breaker count for 300 walls is three
/// guns, 60 Niter, clipped to the 50 the stockpile holds.
#[test]
fn the_bombard_asks_its_niter() {
    let (g, mut ai, home, _) = niter_case(50.0);
    ai.enable_siege_buys_the_gun_resource();
    let trebuchet = Item::Unit {
        unit: crate::name!("trebuchet"),
    };
    let bombard = Item::Unit {
        unit: crate::name!("bombard"),
    };
    assert!(g.can_produce(0, home, &trebuchet));
    assert!(!g.can_produce(0, home, &bombard), "no Niter");
    let wants = ai.siege_gun_resource_wants(&g, 0);
    assert_eq!(wants.len(), 1, "{wants:?}");
    assert_eq!(
        niter(&wants[0]),
        ("RESOURCE_NITER", "bombard", 50, 20, 3, false)
    );
    assert_eq!(wants[0].resource_id, "niter");
    assert!((wants[0].hit - 30.0 * (5.0_f64 / 25.0).exp()).abs() < 1e-9);
}

/// Under `breakers-match-the-walls` the block is sized to the guns the
/// Bombard's blow needs: ~36.6 a shot takes 300 walls and 200 health in ~8.8
/// turns with two guns, inside the default twelve, so 40 Niter.
#[test]
fn the_matched_count_sizes_the_niter() {
    let (g, mut ai, _, _) = niter_case(50.0);
    ai.enable_siege_buys_the_gun_resource();
    ai.enable_breakers_match_the_walls();
    let wants = ai.siege_gun_resource_wants(&g, 0);
    assert_eq!(
        niter(&wants[0]),
        ("RESOURCE_NITER", "bombard", 40, 20, 2, false)
    );
}

/// A Bombard that cannot take the city however many (3.3 a shot against
/// 110, under the floor) is not bought for.
#[test]
fn a_gun_that_cannot_take_the_city_asks_nothing() {
    let (g, mut ai, _, _) = niter_case(110.0);
    ai.enable_siege_buys_the_gun_resource();
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty());
    ai.enable_breakers_match_the_walls();
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty());
}

/// Niter in the stockpile, Niter arriving within five turns, or a Bombard
/// already fielded: nothing to buy.
#[test]
fn stock_income_or_the_gun_in_hand_asks_nothing() {
    let (mut g, mut ai, _, _) = niter_case(50.0);
    ai.enable_siege_buys_the_gun_resource();
    g.players[0]
        .strategic_resources
        .insert(crate::name!("niter"), 20.0);
    assert!(
        ai.siege_gun_resource_wants(&g, 0).is_empty(),
        "stock covers a gun"
    );

    let (mut g, mut ai, _, _) = niter_case(50.0);
    ai.enable_siege_buys_the_gun_resource();
    let mut income = BTreeMap::new();
    income.insert(crate::name!("niter"), 4.0);
    std::sync::Arc::make_mut(&mut g.observed_strategic_income_adjustments).insert(0, income);
    assert!(
        ai.siege_gun_resource_wants(&g, 0).is_empty(),
        "income covers a gun"
    );

    let (mut g, mut ai, home, _) = niter_case(50.0);
    ai.enable_siege_buys_the_gun_resource();
    let pos = g.cities[&home].pos;
    g.spawn_unit("bombard", 0, pos);
    assert!(
        ai.siege_gun_resource_wants(&g, 0).is_empty(),
        "the gun is in hand"
    );
}

/// No standing walls, no Domination target, or a target we neither fight
/// nor plan for: nothing is asked.
#[test]
fn only_the_walled_campaign_target_asks() {
    let (mut g, mut ai, _, target) = niter_case(50.0);
    ai.enable_siege_buys_the_gun_resource();
    g.cities.get_mut(&target).unwrap().wall_hp = 0;
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty(), "breached");
    let (mut g, mut ai, _, _) = niter_case(50.0);
    ai.enable_siege_buys_the_gun_resource();
    g.at_war.clear();
    ai.plan.as_mut().unwrap().target_player = None;
    assert!(
        ai.siege_gun_resource_wants(&g, 0).is_empty(),
        "neither war nor plan"
    );
    let (g, mut ai, _, _) = niter_case(50.0);
    ai.enable_siege_buys_the_gun_resource();
    ai.plan.as_mut().unwrap().target_city = None;
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty(), "no target");
}

/// Oil is upkeep: an Artillery fielded with no Oil and none coming asks ten
/// turns of its fuel, and stops asking once three turns stand in stock.
#[test]
fn fielded_artillery_renews_its_oil() {
    let (mut g, mut ai, home, _) = niter_case(70.0);
    ai.enable_siege_buys_the_gun_resource();
    g.players[0].techs.insert(crate::name!("steel"));
    let pos = g.cities[&home].pos;
    g.spawn_unit("artillery", 0, pos);
    let wants = ai.siege_gun_resource_wants(&g, 0);
    assert_eq!(wants.len(), 1, "{wants:?}");
    assert_eq!(
        niter(&wants[0]),
        (
            "RESOURCE_OIL",
            "artillery",
            GUN_RESOURCE_FUEL_TURNS as u32,
            GUN_RESOURCE_FUEL_LOW_TURNS as u32,
            1,
            true
        )
    );
    g.players[0]
        .strategic_resources
        .insert(crate::name!("oil"), GUN_RESOURCE_FUEL_LOW_TURNS);
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty());
}

/// No sale gives away what the guns want: a live want's Niter, a fielded
/// Bombard's Niter, a queued Artillery's Oil. Off, nothing is held.
#[test]
fn the_guns_resources_are_held_from_sale() {
    let (mut g, mut ai, home, _) = niter_case(50.0);
    assert!(ai.siege_gun_resources_held(&g, 0, &[]).is_empty(), "off");
    ai.enable_siege_buys_the_gun_resource();
    let wants = ai.siege_gun_resource_wants(&g, 0);
    let held = ai.siege_gun_resources_held(&g, 0, &wants);
    assert_eq!(
        held.into_iter().collect::<Vec<_>>(),
        vec!["RESOURCE_NITER".to_string()],
        "the live want's"
    );
    assert!(
        ai.siege_gun_resources_held(&g, 0, &[]).is_empty(),
        "no gun, no want"
    );
    let pos = g.cities[&home].pos;
    g.spawn_unit("bombard", 0, pos);
    g.cities.get_mut(&home).unwrap().queue.push(Item::Unit {
        unit: crate::name!("artillery"),
    });
    g.spawn_unit("trebuchet", 0, pos);
    assert_eq!(
        ai.siege_gun_resources_held(&g, 0, &[])
            .into_iter()
            .collect::<Vec<_>>(),
        vec!["RESOURCE_NITER".to_string(), "RESOURCE_OIL".to_string()],
        "the fielded Bombard's and the queued Artillery's; the Trebuchet needs none"
    );
}

/// A luxury is spare only above its first copy; a strategic only above
/// what our queued units cost and the reserve; the guns' own resource never.
#[test]
fn barter_spares_keep_the_last_copy_and_the_units_needs() {
    let (mut g, mut ai, home, _) = niter_case(50.0);
    let none = BTreeSet::new();
    assert!(ai.siege_barter_spares(&g, 0, &none).is_empty(), "off");
    ai.enable_siege_buys_the_gun_resource();
    let center = g.cities[&home].pos;
    let tiles: Vec<Pos> = g.cities[&home]
        .owned_tiles
        .iter()
        .copied()
        .filter(|pos| *pos != center)
        .take(3)
        .collect();
    for (index, pos) in tiles.iter().enumerate() {
        let tile = g.map.tiles.get_mut(pos).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = Some(if index < 2 {
            crate::name!("silk")
        } else {
            crate::name!("dyes")
        });
        tile.improvement = Some(crate::name!("plantation"));
    }
    assert_eq!(g.connected_resource_count(0, "silk"), 2);
    assert_eq!(g.connected_resource_count(0, "dyes"), 1);
    for (resource, stock) in [("coal", 30.0), ("iron", 25.0), ("niter", 40.0)] {
        g.players[0]
            .strategic_resources
            .insert(crate::name::Name::new(resource), stock);
    }
    g.cities.get_mut(&home).unwrap().queue.push(Item::Unit {
        unit: crate::name!("swordsman"),
    });
    let held: BTreeSet<String> = ["RESOURCE_NITER".to_string()].into_iter().collect();
    let spares = ai.siege_barter_spares(&g, 0, &held);
    let read: Vec<_> = spares
        .iter()
        .map(|spare| (spare.resource.as_str(), spare.amount, spare.luxury))
        .collect();
    // Silk's second copy; Coal above the reserve of ten; Iron all wanted by
    // the queued Swordsman (20) and the reserve; Niter held for the guns;
    // the one Dyes copy stays.
    assert_eq!(
        read,
        vec![("RESOURCE_SILK", 1, true), ("RESOURCE_COAL", 20, false)]
    );
}

/// Two cities of ours at peace with Advanced Flight held and no Aluminum: the
/// Domination air readiness wants its launch wing (the readiness fixture's
/// board, without its Aluminum).
fn air_case() -> (Game, AdvancedAi) {
    let mut g = Game::new_full(2, 40, 24, 371500, 2000, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(0, (12, 12), None);
    g.found_city_for(1, (24, 12), None);
    for tech in g.rules.tech_ancestors["advanced_flight"].clone() {
        g.players[0].techs.insert(crate::name::Name::new(&tech));
    }
    g.players[0].techs.insert(crate::name!("advanced_flight"));
    g.players[0].gold = 1000.0;
    g.players[0].gold_per_turn = 30.0;
    g.at_war.clear();
    g.current = 0;
    g.turn = 150;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_air_surge_2();
    (g, ai)
}

/// No Aluminum, no income: the goal reads no Bombers and the wing asks the
/// stock that lifts it to the launch wing; bought, the goal rises and the
/// ask goes. Off, nothing is asked.
#[test]
fn the_air_wing_buys_its_aluminum() {
    let (mut g, mut ai) = air_case();
    assert!(ai.air_wing_wants_bombers(&g, 0));
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 0);
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty(), "off");
    ai.enable_siege_buys_the_gun_resource();
    let (metal, needed) = AdvancedAi::air_surge_launch_stock(&g, 0, false).expect("a metal Bomber");
    assert_eq!(metal, crate::name!("aluminum"));
    let wants = ai.siege_gun_resource_wants(&g, 0);
    assert_eq!(wants.len(), 1, "{wants:?}");
    assert_eq!(wants[0].resource, "RESOURCE_ALUMINUM");
    assert_eq!(wants[0].amount, needed.ceil() as u32);
    assert_eq!(wants[0].minimum, wants[0].amount, "the whole wing or nothing");
    assert_eq!(wants[0].guns, super::super::air_surge::AIR_SURGE_LAUNCH_BOMBERS);

    // The bought block is read back: the goal fields the launch wing.
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), f64::from(wants[0].amount));
    assert_eq!(
        AdvancedAi::air_surge_bomber_goal(&g, 0),
        super::super::air_surge::AIR_SURGE_LAUNCH_BOMBERS
    );
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty(), "bought, no ask");
}

/// `bombers-fly-on-a-small-stock`: the ask is the small stock that trains
/// the launch wing and fuels it a few turns, not the grace-window block; that
/// bank lifts the seat's goal to the launch wing (the grace-window goal still
/// reads none) and the ask goes. Off, the seat's goal is the grace-window goal.
#[test]
fn a_small_stock_flies_the_launch_wing() {
    let (mut g, mut ai) = air_case();
    ai.enable_siege_buys_the_gun_resource();
    let full = ai.siege_gun_resource_wants(&g, 0);
    assert_eq!(full.len(), 1, "{full:?}");
    ai.enable_bombers_fly_on_a_small_stock();
    let wants = ai.siege_gun_resource_wants(&g, 0);
    assert_eq!(wants.len(), 1, "{wants:?}");
    let (_, small) = AdvancedAi::air_surge_launch_stock(&g, 0, true).expect("a metal Bomber");
    assert_eq!(wants[0].amount, small.ceil() as u32);
    assert_eq!(wants[0].minimum, wants[0].amount);
    assert!(wants[0].amount < full[0].amount, "{wants:?} against {full:?}");
    let launch = super::super::air_surge::AIR_SURGE_LAUNCH_BOMBERS;
    let aluminum = crate::name!("aluminum");
    // One short of the small stock flies nothing and still asks.
    g.players[0]
        .strategic_resources
        .insert(aluminum, f64::from(wants[0].amount - 1));
    assert_eq!(ai.air_wing_bomber_goal(&g, 0), 0);
    assert!(!ai.siege_gun_resource_wants(&g, 0).is_empty());
    // The small lot bought: the seat fields the launch wing while the
    // grace-window goal still reads none, and the ask goes.
    g.players[0]
        .strategic_resources
        .insert(aluminum, f64::from(wants[0].amount));
    assert_eq!(ai.air_wing_bomber_goal(&g, 0), launch);
    assert_eq!(AdvancedAi::air_surge_bomber_goal(&g, 0), 0);
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty(), "bought, no ask");
    ai.disable_bombers_fly_on_a_small_stock();
    assert_eq!(ai.air_wing_bomber_goal(&g, 0), 0, "off: the grace-window bank");
    assert!(!ai.siege_gun_resource_wants(&g, 0).is_empty(), "off: the block is asked");
}

/// The Bomber more than three technologies away asks nothing yet.
#[test]
fn a_distant_bomber_asks_no_aluminum() {
    let (mut g, mut ai) = air_case();
    ai.enable_siege_buys_the_gun_resource();
    let ancestors = g.rules.tech_ancestors["advanced_flight"].clone();
    for tech in ancestors.iter().take(3) {
        g.players[0].techs.remove(&crate::name::Name::new(tech));
    }
    g.players[0].techs.remove(&crate::name!("advanced_flight"));
    assert!(AdvancedAi::air_surge_missing_techs(&g, 0) > super::AIR_WING_RESOURCE_TECHS);
    assert!(ai.siege_gun_resource_wants(&g, 0).is_empty());
}

/// While the wing wants Bombers its Aluminum is held from sale and from the
/// barter, bought or not. Off, nothing is held.
#[test]
fn the_wings_aluminum_is_held_from_sale() {
    let (mut g, mut ai) = air_case();
    assert!(ai.siege_gun_resources_held(&g, 0, &[]).is_empty(), "off");
    ai.enable_siege_buys_the_gun_resource();
    g.players[0]
        .strategic_resources
        .insert(crate::name!("aluminum"), 60.0);
    let held = ai.siege_gun_resources_held(&g, 0, &[]);
    assert!(held.contains("RESOURCE_ALUMINUM"), "{held:?}");
    let spares = ai.siege_barter_spares(&g, 0, &held);
    assert!(
        spares.iter().all(|spare| spare.resource != "RESOURCE_ALUMINUM"),
        "never bartered away"
    );
}
