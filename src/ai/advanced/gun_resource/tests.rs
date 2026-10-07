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
