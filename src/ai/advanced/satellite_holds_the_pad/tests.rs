use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::GrandStrategy;

/// Our capital at population 10 with Rocketry and a standing Spaceport, its
/// queue led by a Builder; a peaceful plan.
fn pad_city() -> (Game, u32, StrategicPlan) {
    let mut g = Game::new_full(
        2,
        24,
        16,
        crate::rng::fixture_seed("SATELLITEHOLD", 52_337),
        250,
        0,
        false,
    );
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let capital = g.player_city_ids(0)[0];
    g.players[0].techs.insert(crate::name!("rocketry"));
    let center = g.cities[&capital].pos;
    let site = g.cities[&capital]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| {
            *pos != center
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        })
        .expect("fixture: a land tile for the pad");
    let pad = crate::name!("spaceport");
    {
        let tile = g.map.tiles.get_mut(&site).unwrap();
        tile.district = Some(pad);
        tile.improvement = None;
    }
    let city = g.cities.get_mut(&capital).unwrap();
    city.districts.insert(pad, site);
    city.pop = 10;
    city.queue.clear();
    city.queue.push(Item::Unit {
        unit: crate::name!("builder"),
    });
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, capital, plan)
}

fn satellite() -> Item {
    Item::Project {
        project: crate::name!("launch_earth_satellite"),
    }
}

#[test]
fn satellite_holds_the_pad_is_opt_in() {
    opt_in_off_in_both_controllers("satellite-holds-the-pad", |ai| ai.satellite_holds_the_pad);
}

/// Off, the Builder keeps the head; on, the Earth Satellite takes it, and the
/// next turn the hold has nothing to do.
#[test]
fn the_standing_pad_puts_the_satellite_back_at_its_head() {
    let (game, capital, plan) = pad_city();
    assert!(
        game.can_produce(0, capital, &satellite()),
        "fixture: the pad can build the Satellite"
    );

    let mut off = game.clone();
    AdvancedAi::new().claim_satellite_holds_the_pad(&mut off, 0, &plan);
    assert_ne!(off.cities[&capital].queue.first(), Some(&satellite()));

    let mut ai = AdvancedAi::new();
    ai.enable_satellite_holds_the_pad();
    let mut on = game.clone();
    ai.claim_satellite_holds_the_pad(&mut on, 0, &plan);
    assert_eq!(on.cities[&capital].queue.first(), Some(&satellite()));
    assert!(ai.satellite_hold(&on, 0).is_none(), "already at the head");
}

/// A landed Satellite or a threatened pad city leaves the queue alone.
#[test]
fn a_landed_satellite_or_a_threat_releases_the_hold() {
    let mut ai = AdvancedAi::new();
    ai.enable_satellite_holds_the_pad();

    let (mut landed, _, _) = pad_city();
    landed.players[0]
        .science_projects
        .insert("launch_earth_satellite".into());
    assert!(ai.satellite_hold(&landed, 0).is_none());

    let (mut threatened, capital, mut plan) = pad_city();
    plan.threatened_city = Some(capital);
    ai.claim_satellite_holds_the_pad(&mut threatened, 0, &plan);
    assert_ne!(
        threatened.cities[&capital].queue.first(),
        Some(&satellite())
    );
}
