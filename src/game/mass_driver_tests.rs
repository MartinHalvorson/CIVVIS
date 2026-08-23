//! The Modified Future Era: ore on the Moon, and the mass driver that throws
//! it down.
//!
//! Everything here is about the two things that make the mechanic what it is —
//! the piles are *one shared body*, and a slug is *the same metal* whether it
//! is landed as cargo or dropped on somebody. A test that only checked "the
//! stockpile went up" would pass on a design that quietly gave every
//! civilization its own private Moon.

use super::*;
use crate::setup::FutureEra;

/// Two capitals, far enough apart to be two civilizations, on whichever
/// Future Era is asked for.
fn game_on(era: FutureEra, seed: u64) -> (Game, Vec<u32>) {
    let mut game = Game::new_with(GameOptions {
        future_era: era,
        barbarians: false,
        ..GameOptions::new(2, 24, 16, seed, 200, 0)
    });
    let mut cities = Vec::new();
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let city = game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
        cities.push(city);
    }
    (game, cities)
}

/// Put `count` drivers over the Moon for a seat, the way finishing the project
/// that many times would. The project itself is ordinary production and is
/// covered by the ruleset test below; what these tests are about is what the
/// drivers then do.
fn install_drivers(game: &mut Game, pid: usize, count: i64) {
    game.players[pid]
        .counters
        .insert("project_effect:mass_drivers".to_string(), count);
    game.players[pid]
        .science_projects
        .insert("launch_moon_landing".to_string());
    // Iron is visible from Bronze Working and uranium from Combined Arms;
    // a civilization with a driver on the Moon is long past both.
    for tech in ["bronze_working", "radio", "combined_arms"] {
        game.players[pid].techs.insert(Name::new(tech));
    }
}

#[test]
fn the_modified_future_era_is_the_only_one_with_a_moon_worth_going_back_to() {
    let (classic, _) = game_on(FutureEra::Classic, 5_101_991);
    assert!(
        classic.moon_deposits.is_empty(),
        "the classic Moon is a milestone, not a place with ore in it"
    );
    assert!(
        !classic.rules.projects.contains_key("mass_driver"),
        "and it has nothing to throw ore with"
    );

    let (modified, _) = game_on(FutureEra::Modified, 5_101_991);
    let driver = modified
        .rules
        .projects
        .get("mass_driver")
        .expect("the modified era ships the project");
    assert!(driver.repeatable, "a second driver is a second slug a turn");
    assert_eq!(driver.district.as_deref(), Some("spaceport"));
    assert_eq!(driver.tech.as_deref(), Some("offworld_mission"));
    assert_eq!(
        driver.requires,
        vec![Name::new("launch_moon_landing")],
        "you have to have got there before you can mine it"
    );

    // The three ores the Moon is made of, each inside the range the ruleset
    // gives it and each a whole number of units.
    let ores = modified.moon_ores();
    assert_eq!(
        ores,
        vec![
            Name::new("aluminum"),
            Name::new("iron"),
            Name::new("uranium")
        ],
    );
    for ore in ores {
        let spec = modified.rules.resources[&ore]
            .lunar
            .expect("a lunar ore has a deposit");
        let rolled = modified.moon_deposit(ore);
        assert!(
            (spec.min..=spec.max).contains(&rolled),
            "{ore} rolled {rolled}, outside {}..={}",
            spec.min,
            spec.max
        );
        assert_eq!(rolled, rolled.round(), "ore comes in whole units");
        assert_eq!(
            modified.rules.resources[&ore].class, "strategic",
            "the Moon holds strategic ore, which is what a stockpile takes"
        );
    }

    // The world itself is untouched by the setting: the same seed makes the
    // same map either way, because a ruleset with nothing on the Moon draws
    // no randomness for it.
    let (classic_again, _) = game_on(FutureEra::Classic, 5_101_991);
    assert_eq!(classic_again.map.tiles.len(), modified.map.tiles.len());
    for (position, tile) in &classic_again.map.tiles {
        assert_eq!(
            tile.terrain, modified.map.tiles[position].terrain,
            "the Future Era must not move the ground under the game"
        );
    }

    // And a match keeps the era it was played under across a save.
    let restored: Game = serde_json::from_str(&serde_json::to_string(&modified).unwrap()).unwrap();
    assert_eq!(restored.future_era, FutureEra::Modified);
    assert_eq!(restored.moon_deposits, modified.moon_deposits);
    assert!(restored.rules.projects.contains_key("mass_driver"));
}

#[test]
fn a_driver_lands_one_unit_a_turn_and_the_moon_is_that_much_lighter() {
    let (mut game, cities) = game_on(FutureEra::Modified, 77_412);
    let site = game.cities[&cities[0]].pos;
    let iron = crate::name!("iron");
    install_drivers(&mut game, 0, 2);

    // Unaimed drivers drop nothing: the order is the whole mechanism.
    let before = game.moon_deposit(iron);
    game.process_mass_drivers(0);
    assert_eq!(game.strategic_stockpile(0, iron), 0.0);
    assert_eq!(game.moon_deposit(iron), before);

    let aim = Action::AimMassDriver { site, ore: iron };
    assert!(game.legal_actions(0).contains(&aim));
    game.apply(0, &aim).unwrap();

    game.process_mass_drivers(0);
    assert_eq!(
        game.strategic_stockpile(0, iron),
        2.0,
        "one unit per driver per turn"
    );
    assert_eq!(
        game.moon_deposit(iron),
        before - 2.0,
        "and the Moon is exactly that much lighter"
    );

    // Aiming somewhere that is not yours is refused, and ground lost after the
    // fact simply stops catching anything.
    let elsewhere = game.cities[&cities[1]].pos;
    assert!(game
        .apply(
            0,
            &Action::AimMassDriver {
                site: elsewhere,
                ore: iron
            }
        )
        .is_err());
    game.players[0].mass_driver_site = Some(elsewhere);
    let held = game.moon_deposit(iron);
    game.process_mass_drivers(0);
    assert_eq!(
        game.moon_deposit(iron),
        held,
        "nobody else catches your ore"
    );
    game.players[0].mass_driver_site = Some(site);

    // A full warehouse does not spill a shared, finite resource: the ore stays
    // on the Moon for whoever can still hold it.
    let capacity = game.strategic_stockpile_capacity(0);
    game.players[0].strategic_resources.insert(iron, capacity);
    let full = game.moon_deposit(iron);
    game.process_mass_drivers(0);
    assert_eq!(game.moon_deposit(iron), full);
    assert_eq!(game.strategic_stockpile(0, iron), capacity);

    // A pile with one unit left pays out once and then is a dead body.
    game.players[0].strategic_resources.insert(iron, 0.0);
    game.moon_deposits.insert(iron, 1.0);
    game.process_mass_drivers(0);
    assert_eq!(game.strategic_stockpile(0, iron), 1.0);
    assert_eq!(game.moon_deposit(iron), 0.0);
    game.process_mass_drivers(0);
    assert_eq!(
        game.strategic_stockpile(0, iron),
        1.0,
        "an exhausted pile keeps paying nothing"
    );
    assert!(
        !game
            .legal_actions(0)
            .iter()
            .any(|action| matches!(action, Action::AimMassDriver { ore, .. } if *ore == iron)),
        "and stops being offered as somewhere to aim"
    );
}

#[test]
fn there_is_one_moon_and_everybody_draws_from_the_same_piles() {
    let (mut game, cities) = game_on(FutureEra::Modified, 214_003);
    let aluminum = crate::name!("aluminum");
    let before = game.moon_deposit(aluminum);
    assert!(before >= 4.0, "fixture needs a pile worth racing for");

    for (pid, city) in cities.iter().enumerate() {
        install_drivers(&mut game, pid, 2);
        let site = game.cities[city].pos;
        game.current = pid;
        game.apply(
            pid,
            &Action::AimMassDriver {
                site,
                ore: aluminum,
            },
        )
        .unwrap();
    }
    game.process_mass_drivers(0);
    game.process_mass_drivers(1);

    assert_eq!(game.strategic_stockpile(0, aluminum), 2.0);
    assert_eq!(game.strategic_stockpile(1, aluminum), 2.0);
    assert_eq!(
        game.moon_deposit(aluminum),
        before - 4.0,
        "two civilizations mining one Moon take four units out of it, not two \
         out of two private ones",
    );

    // Which is the whole point: what a rival has already taken is gone. Empty
    // the pile from one side and the other side's drivers have nothing left.
    game.moon_deposits.insert(aluminum, 1.0);
    game.process_mass_drivers(0);
    assert_eq!(game.moon_deposit(aluminum), 0.0);
    let stranded = game.strategic_stockpile(1, aluminum);
    game.process_mass_drivers(1);
    assert_eq!(
        game.strategic_stockpile(1, aluminum),
        stranded,
        "the second civilization arrives at an empty Moon"
    );
}

#[test]
fn a_slug_is_refused_in_peace_and_costs_the_metal_it_is_made_of() {
    let (mut game, cities) = game_on(FutureEra::Modified, 918_204);
    let (mine, theirs) = (cities[0], cities[1]);
    let site = game.cities[&mine].pos;
    let target = game.cities[&theirs].pos;
    let iron = crate::name!("iron");
    install_drivers(&mut game, 0, 1);
    game.players[0].explored.insert(target);
    game.cities.get_mut(&theirs).unwrap().pop = 6;
    game.cities.get_mut(&theirs).unwrap().wall_hp = 100;
    let city_name = game.cities[&theirs].name.clone();
    let defender = game.spawn_test_unit("warrior", 1, target);

    let strike = Action::MassDriverStrike { target };
    // Unaimed, so there is no ore to make a slug out of.
    assert!(game.apply(0, &strike).is_err());
    game.apply(0, &Action::AimMassDriver { site, ore: iron })
        .unwrap();
    assert!(
        game.apply(0, &strike).is_err(),
        "an empty stockpile has nothing to throw"
    );
    game.process_mass_drivers(0);
    assert_eq!(game.strategic_stockpile(0, iron), 1.0);
    assert!(
        game.apply(0, &strike).is_err(),
        "dropping a rock on a civilization you are at peace with is not an order"
    );
    game.at_war.insert(pair(0, 1));

    assert!(game.legal_actions(0).contains(&strike));
    game.apply(0, &strike).unwrap();

    assert_eq!(
        game.strategic_stockpile(0, iron),
        0.0,
        "the slug is the metal: a shot costs a unit of the aimed ore"
    );
    assert_eq!(game.players[0].counters["mass_driver_strikes"], 1);
    assert_eq!(game.players[0].mass_driver_shots, 1);
    assert!(
        game.apply(0, &strike).is_err(),
        "one driver, one shot a turn"
    );
    assert!(
        game.units.get(&defender).is_none_or(|unit| unit.hp < 100),
        "whatever is standing on the tile takes the impact"
    );
    let struck = &game.cities[&theirs];
    assert_eq!(struck.wall_hp, 0, "a slug goes through the Outer Defenses");
    assert_eq!(struck.pop, 5);
    assert!(
        game.map.tiles[&target].fallout_until < game.turn,
        "a rock is not a device: it leaves no fallout"
    );

    // It belongs to the war it was thrown in.
    let war = game
        .wars
        .get(&pair(0, 1))
        .expect("a strike is fought inside a war");
    let moment = war
        .highlights
        .iter()
        .find(|moment| moment.kind == "mass_driver_strike")
        .expect("the ledger names the strike");
    assert_eq!((moment.actor, moment.subject), (0, 1));
    assert_eq!(moment.city.as_deref(), Some(city_name.as_str()));
    assert_eq!(
        game.players[1].grievances.get(&0),
        Some(&50.0),
        "the victim holds a grievance for it — a war's worth, not a device's"
    );

    // The next turn re-arms the driver: a shot is a turn's cargo spent on a
    // target, not a separate magazine.
    game.process_mass_drivers(0);
    assert_eq!(game.players[0].mass_driver_shots, 0);
    assert_eq!(game.strategic_stockpile(0, iron), 1.0);
    assert!(game.legal_actions(0).contains(&strike));
}
