//! `bombers-open-the-siege-walls`: a strike bomber over a walled city our
//! ground train besieges opens its walls before pillage or distant strikes,
//! but still strikes a hostile threatening a city of ours first.

use super::super::{GrandStrategy, StrategicPlan, VictoryTarget};
use super::*;

/// A walled enemy city, a city of ours far from it, and a bomber in reach of
/// both; `hostile_near_home` places a 1-hp Warrior beside our city, else
/// away from both cities.
fn bomber_over_a_siege(hostile_near_home: bool) -> (Game, u32, u32, Pos, Pos) {
    let mut g = Game::new_full(2, 24, 16, 71_031, 120, 0, false);
    g.at_war.insert((0, 1));
    for unit in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(unit);
    }
    let mut land: Vec<Pos> = g
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        .map(|(pos, _)| *pos)
        .collect();
    land.sort_unstable();
    let enemy_at = land[land.len() / 2];
    let enemy = g.found_city_for(1, enemy_at, None);
    {
        let city = g.cities.get_mut(&enemy).unwrap();
        city.buildings.push(crate::name!("walls"));
    }
    let wall_max = g.city_max_wall_hp(&g.cities[&enemy]);
    g.cities.get_mut(&enemy).unwrap().wall_hp = wall_max;
    assert!(wall_max > 0, "fixture: walls stand");
    let range = g.rules.units["bomber"].range;
    let home_at = *land
        .iter()
        .find(|pos| {
            let d = g.wdist(**pos, enemy_at);
            (6..=range - 2).contains(&d)
        })
        .expect("fixture: a home site");
    g.found_city_for(0, home_at, None);
    let base = *land
        .iter()
        .find(|pos| {
            g.city_at(**pos).is_none()
                && g.wdist(**pos, enemy_at) <= range - 1
                && g.wdist(**pos, home_at) <= range - 1
                && g.wdist(**pos, home_at) >= 2
        })
        .expect("fixture: a base in reach of both cities");
    let hostile_at = *land
        .iter()
        .find(|pos| {
            g.city_at(**pos).is_none()
                && **pos != base
                && g.wdist(**pos, base) <= range
                && if hostile_near_home {
                    g.wdist(**pos, home_at) == 1
                } else {
                    g.wdist(**pos, enemy_at) >= 5 && g.wdist(**pos, home_at) >= 5
                }
        })
        .expect("fixture: a hostile tile");
    // A soldier of ours near the besieged city, the train's eye on it: an
    // Archer, so the capital-finisher credit (a melee body within three
    // tiles of a Domination capital) does not price the city strike.
    let near = *land
        .iter()
        .find(|pos| {
            g.city_at(**pos).is_none()
                && **pos != base
                && **pos != hostile_at
                && g.wdist(**pos, enemy_at) == 2
                && g.wdist(**pos, hostile_at) >= 2
        })
        .expect("fixture: a tile near the siege");
    g.spawn_test_unit("archer", 0, near);
    let bomber = g.spawn_test_unit("bomber", 0, base);
    let warrior = g.spawn_test_unit("warrior", 1, hostile_at);
    g.units.get_mut(&warrior).unwrap().hp = 1;
    (g, bomber, enemy, enemy_at, hostile_at)
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 0,
        rush: false,
    }
}

fn besieging(ai: &mut AdvancedAi, g: &Game, cid: u32) {
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Invest,
            taker: None,
            entered: g.turn,
            assessed: g.turn,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
}

#[test]
fn a_bomber_opens_the_siege_walls_before_a_distant_kill_only_under_the_gene() {
    for gene in [false, true] {
        let (g, bomber, enemy, enemy_at, hostile_at) = bomber_over_a_siege(false);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        besieging(&mut ai, &g, enemy);
        if gene {
            ai.enable_bombers_open_the_siege_walls();
        }
        let legal = g.legal_doctrine_actions(0, bomber);
        let city_strike = Action::AirStrike {
            unit: bomber,
            target: enemy_at,
        };
        assert!(
            legal.contains(&city_strike),
            "fixture: the strike on the city is legal; legal = {legal:?}"
        );
        let city_value = ai.air_strike_value(&g, 0, bomber, enemy_at, &plan());
        let unit_value = ai.air_strike_value(&g, 0, bomber, hostile_at, &plan());
        assert!(
            city_value > 0.0 && unit_value > city_value,
            "fixture: city {city_value:.1}, distant kill {unit_value:.1}"
        );
        let choice = ai.advanced_air_action(&g, 0, bomber, &plan());
        let expected = if gene { enemy_at } else { hostile_at };
        assert_eq!(
            choice,
            Some(Action::AirStrike {
                unit: bomber,
                target: expected,
            }),
            "gene {gene}"
        );
    }
}

#[test]
fn a_hostile_beside_our_city_still_comes_before_the_walls() {
    let (g, bomber, enemy, _, hostile_at) = bomber_over_a_siege(true);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    besieging(&mut ai, &g, enemy);
    ai.enable_bombers_open_the_siege_walls();
    assert_eq!(
        ai.advanced_air_action(&g, 0, bomber, &plan()),
        Some(Action::AirStrike {
            unit: bomber,
            target: hostile_at,
        })
    );
}

#[test]
fn without_a_walled_siege_the_ordinary_choice_stands() {
    let (g, bomber, _, _, hostile_at) = bomber_over_a_siege(false);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_bombers_open_the_siege_walls();
    assert_eq!(
        ai.advanced_air_action(&g, 0, bomber, &plan()),
        Some(Action::AirStrike {
            unit: bomber,
            target: hostile_at,
        })
    );
}

#[test]
fn a_siege_whose_train_is_far_off_leaves_the_ordinary_choice() {
    let (mut g, bomber, enemy, enemy_at, hostile_at) = bomber_over_a_siege(false);
    let near: Vec<u32> = g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "archer")
        .map(|unit| unit.id)
        .collect();
    for uid in near {
        g.remove_unit(uid);
    }
    assert!(g
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind != "bomber")
        .all(|unit| g.wdist(unit.pos, enemy_at) > MUSTER_FAR));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    besieging(&mut ai, &g, enemy);
    ai.enable_bombers_open_the_siege_walls();
    assert_eq!(
        ai.advanced_air_action(&g, 0, bomber, &plan()),
        Some(Action::AirStrike {
            unit: bomber,
            target: hostile_at,
        })
    );
}

/// `wall-sortie-skips-the-encampment` fixture: an enemy Encampment two tiles
/// from our city, in the bomber's reach, at the 20 health and 0 walls of live
/// G433's, which healed between strikes so each sortie read a little damage.
fn encampment_beside_home(g: &mut Game, bomber: u32, enemy: u32) -> Pos {
    let home = g.player_city_ids(0)[0];
    let home_at = g.cities[&home].pos;
    let base = g.units[&bomber].pos;
    let range = g.rules.units["bomber"].range;
    let camp = g
        .wring(home_at, 2)
        .into_iter()
        .find(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
                && g.units_at(*pos).is_empty()
                && g.city_at(*pos).is_none()
                && *pos != base
                && g.wdist(*pos, base) <= range
        })
        .expect("fixture: an Encampment tile beside our city");
    {
        let tile = g.map.tiles.get_mut(&camp).unwrap();
        tile.district = Some(crate::name!("encampment"));
        tile.owner_city = Some(enemy);
    }
    g.cities.get_mut(&enemy).unwrap().encampment_hp = 20;
    assert!(g.encampment_at(camp).is_some(), "fixture: the Encampment stands");
    camp
}

/// `wall-sortie-skips-the-encampment`: an enemy Encampment two tiles from a
/// city of ours, with no soldier on it, takes the wall sortie's guard slot
/// with the gene off; with it on, the bomber opens the besieged city's walls.
#[test]
fn a_bare_encampment_beside_our_city_does_not_outrank_the_walls_under_the_gene() {
    for gene in [false, true] {
        let (mut g, bomber, enemy, enemy_at, _) = bomber_over_a_siege(false);
        let camp = encampment_beside_home(&mut g, bomber, enemy);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        besieging(&mut ai, &g, enemy);
        ai.enable_bombers_open_the_siege_walls();
        if gene {
            ai.enable_wall_sortie_skips_the_encampment();
        }
        let legal = g.legal_doctrine_actions(0, bomber);
        let camp_strike = Action::AirStrike { unit: bomber, target: camp };
        assert!(legal.contains(&camp_strike), "fixture: the Encampment is a legal strike");
        assert!(
            ai.air_strike_value(&g, 0, bomber, camp, &plan()) > 0.0,
            "fixture: the Encampment strike is worth something"
        );
        let choice = ai.siege_wall_sortie(&g, 0, bomber, &legal, &plan());
        let expected = if gene { enemy_at } else { camp };
        assert_eq!(
            choice,
            Some(Action::AirStrike { unit: bomber, target: expected }),
            "gene {gene}"
        );
    }
}

/// `wall-sortie-skips-the-encampment`: a hostile soldier standing in that
/// Encampment beside our city is still the guard's target under the gene.
#[test]
fn a_soldier_in_the_encampment_beside_our_city_still_takes_the_guard_slot() {
    let (mut g, bomber, enemy, _, _) = bomber_over_a_siege(false);
    let camp = encampment_beside_home(&mut g, bomber, enemy);
    let soldier = g.spawn_test_unit("warrior", 1, camp);
    g.units.get_mut(&soldier).unwrap().hp = 1;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    besieging(&mut ai, &g, enemy);
    ai.enable_bombers_open_the_siege_walls();
    ai.enable_wall_sortie_skips_the_encampment();
    let legal = g.legal_doctrine_actions(0, bomber);
    let choice = ai.siege_wall_sortie(&g, 0, bomber, &legal, &plan());
    assert!(
        matches!(choice, Some(Action::AirStrike { target, .. } | Action::PriorityTarget { target, .. }) if target == camp),
        "the soldier in the Encampment is struck first: {choice:?}"
    );
}
