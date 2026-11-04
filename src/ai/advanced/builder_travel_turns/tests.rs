use super::*;

const STRATEGY: GrandStrategy = GrandStrategy::Science;

/// Two cities six hexes apart on flat grassland, Iron beside the second, and a
/// Builder at `from` beside or in the first. `here` (beside the first city)
/// carries middling work: the resource whose job there falls in the window the
/// gene is for — the Iron beats it at `0.7` a hex but not per Builder-turn —
/// with the widest margin either way. The values come from the scorer itself,
/// so the fixture keeps meaning what it says if the scorer is retuned.
fn two_cities(from: Pos) -> (Game, u32, Pos, Pos) {
    let mut g = Game::new_full(2, 24, 16, 91_515, 150, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.river_edges = [false; 6];
        tile.resource = None;
        tile.improvement = None;
    }
    g.current = 0;
    for site in [(5, 5), (11, 5)] {
        let founder = g.spawn_test_unit("settler", 0, site);
        g.apply(0, &Action::FoundCity { unit: founder }).unwrap();
    }
    for city in g.player_city_ids(0) {
        g.cities.get_mut(&city).unwrap().pop = 2;
    }
    for tech in [
        "mining",
        "bronze_working",
        "animal_husbandry",
        "irrigation",
        "pottery",
    ] {
        g.players[0].techs.insert(Name::new(tech));
    }
    let here = (5, 4);
    let far = (12, 5);
    assert!(g
        .player_city_ids(0)
        .iter()
        .any(|city| g.cities[city].owned_tiles.contains(&far)));
    g.map.tiles.get_mut(&far).unwrap().resource = Some(crate::name!("iron"));
    let builder = g.spawn_test_unit("builder", 0, from);

    let ai = with_the_gene();
    let travel = ai.builder_travel(&g, builder);
    assert!(travel.priced_in_turns());
    let (near, distant) = (g.wdist(from, here), g.wdist(from, far));
    let resources: Vec<Name> = g.rules.resources.keys().copied().collect();
    let mut chosen: Option<(f64, Name, bool)> = None;
    for resource in resources {
        for hills in [false, true] {
            let tile = g.map.tiles.get_mut(&here).unwrap();
            tile.resource = Some(resource);
            tile.hills = hills;
            let (Some(local), Some(remote)) = (best_value(&ai, &g, here), best_value(&ai, &g, far))
            else {
                continue;
            };
            let untreated_gap = BuilderTravel::LEGACY.score(remote, distant)
                - BuilderTravel::LEGACY.score(local, near);
            let treated_gap = travel.score(local, near) - travel.score(remote, distant);
            let margin = untreated_gap.min(treated_gap);
            if margin > 0.0 && chosen.is_none_or(|(best, _, _)| margin > best) {
                chosen = Some((margin, resource, hills));
            }
        }
    }
    let (_, resource, hills) =
        chosen.expect("some resource's work beside the first city falls in the window");
    let tile = g.map.tiles.get_mut(&here).unwrap();
    tile.resource = Some(resource);
    tile.hills = hills;
    (g, builder, here, far)
}

fn with_the_gene() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_builder_prices_travel_in_turns();
    ai
}

/// The best value a tile's work is worth, before any travel price.
fn best_value(ai: &AdvancedAi, g: &Game, pos: Pos) -> Option<f64> {
    let shortfall = g.map.tiles[&pos].owner_city.map_or(0.0, |city| {
        ai.city_production_foundation_shortfall(g, 0, city)
    });
    ai.worthwhile_improvements(g, 0, pos, STRATEGY)
        .into_iter()
        .map(|improvement| {
            ai.production_foundation_improvement_value(g, 0, pos, &improvement, STRATEGY, shortfall)
        })
        .max_by(f64::total_cmp)
}

#[test]
fn the_untreated_price_is_the_old_formula_bit_for_bit() {
    for value in [0.0, 6.6, 94.0, -3.25, 1e-9] {
        for distance in 0..9 {
            assert_eq!(
                BuilderTravel::LEGACY.score(value, distance).to_bits(),
                (value - distance as f64 * 0.7).to_bits(),
                "value {value} at {distance}"
            );
        }
    }
}

#[test]
fn turns_count_the_working_turn_and_every_whole_turn_of_walking() {
    let two = BuilderTravel::in_turns(2.0);
    let turns: Vec<f64> = (0..6).map(|distance| two.turns(distance)).collect();
    assert_eq!(turns, vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0]);
    assert_eq!(two.score(30.0, 5), 10.0);
    // A unit with no movement reading is priced as one move a turn.
    assert_eq!(BuilderTravel::in_turns(0.0).turns(3), 4.0);
}

#[test]
fn a_worthless_job_still_looks_worse_further_away() {
    let two = BuilderTravel::in_turns(2.0);
    assert!(two.score(-4.0, 0) > two.score(-4.0, 4));
    assert!(two.score(0.0, 0) >= two.score(0.0, 4));
}

#[test]
fn the_gene_is_off_by_default_and_the_last_charge_keeps_the_old_price() {
    let (mut g, builder, _, _) = two_cities((5, 4));
    let untreated = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(untreated.builder_travel(&g, builder), BuilderTravel::LEGACY);
    let ai = with_the_gene();
    assert!(g.units[&builder].charges > 1);
    assert!(ai.builder_travel(&g, builder).priced_in_turns());
    g.units.get_mut(&builder).unwrap().charges = 1;
    assert_eq!(ai.builder_travel(&g, builder), BuilderTravel::LEGACY);
}

#[test]
fn the_fixture_is_the_hop_the_gene_is_for() {
    let (g, builder, here, far) = two_cities((5, 4));
    let ai = with_the_gene();
    let local = best_value(&ai, &g, here).unwrap();
    let remote = best_value(&ai, &g, far).unwrap();
    let distance = g.wdist(here, far);
    let travel = ai.builder_travel(&g, builder);
    assert!(
        BuilderTravel::LEGACY.score(remote, distance) > local,
        "untreated: {remote:.1} - 0.7 x {distance} must beat {local:.1} here"
    );
    assert!(
        travel.score(remote, distance) <= local,
        "per turn: {remote:.1} over {} turns must not beat {local:.1} here",
        travel.turns(distance)
    );
}

#[test]
fn untreated_the_builder_leaves_its_work_for_the_far_city() {
    let (mut g, builder, here, far) = two_cities((5, 4));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    let charges = g.units[&builder].charges;
    assert!(ai.advanced_builder_step(&mut g, 0, builder, STRATEGY));
    assert_ne!(g.units[&builder].pos, here);
    assert_eq!(g.units[&builder].charges, charges);
    assert_eq!(ai.builder_targets.get(&builder), Some(&far));
}

#[test]
fn with_charges_to_spare_the_builder_finishes_the_work_here() {
    let (mut g, builder, here, _) = two_cities((5, 4));
    let mut ai = with_the_gene();
    let charges = g.units[&builder].charges;
    assert!(ai.advanced_builder_step(&mut g, 0, builder, STRATEGY));
    assert_eq!(g.units[&builder].pos, here);
    assert!(g.map.tiles[&here].improvement.is_some());
    assert_eq!(g.units[&builder].charges, charges - 1);
}

#[test]
fn on_its_last_charge_the_builder_still_walks_to_the_better_job() {
    let (mut g, builder, here, far) = two_cities((5, 4));
    g.units.get_mut(&builder).unwrap().charges = 1;
    let mut ai = with_the_gene();
    assert!(ai.advanced_builder_step(&mut g, 0, builder, STRATEGY));
    assert_ne!(g.units[&builder].pos, here);
    assert!(g.map.tiles[&here].improvement.is_none());
    assert_eq!(ai.builder_targets.get(&builder), Some(&far));
}

#[test]
fn with_nothing_here_the_sweep_takes_its_own_citys_work_first() {
    // The Builder stands in the first city's centre, where there is no work.
    let (mut g, builder, here, far) = two_cities((5, 5));
    assert!(with_the_gene()
        .worthwhile_improvements(&g, 0, (5, 5), STRATEGY)
        .is_empty());

    let mut untreated = AdvancedAi::targeting(VictoryTarget::Science);
    let mut board = g.clone();
    assert!(untreated.advanced_builder_step(&mut board, 0, builder, STRATEGY));
    assert_eq!(untreated.builder_targets.get(&builder), Some(&far));

    let mut ai = with_the_gene();
    assert!(ai.advanced_builder_step(&mut g, 0, builder, STRATEGY));
    assert_eq!(ai.builder_targets.get(&builder), Some(&here));
}

/// civvis-20261031T031456Z (Kongo, Immortal): the opening Builder stood from
/// t17 and made its first improvement at t34. Two of its `more_useful_builder_job`
/// switches, read back from why.log (`after travel` = value − 0.7 × wdist) with
/// the Builder's position, 2 moves and 2 charges from the state rows.
#[test]
fn the_live_kongo_builder_stays_for_the_long_walk_and_takes_the_short_one() {
    let two = BuilderTravel::in_turns(2.0);
    // t27, axial (5, 25) → (11, 18), 7 hexes: "adds 23.3 after travel, versus
    // 12.6 here". It walked, under escort holds t28-34.
    let value = 23.3 + 0.7 * 7.0;
    assert!(BuilderTravel::LEGACY.score(value, 7) > 12.6);
    assert!(
        two.score(value, 7) < 12.6,
        "4 Builder-turns: {:.2} a turn",
        two.score(value, 7)
    );
    // t33, axial (9, 20) → (11, 20), 2 hexes: "adds 42.0 after travel, versus
    // 19.1 here". Two turns for more than twice the work still pays.
    let value = 42.0 + 0.7 * 2.0;
    assert!(two.score(value, 2) > 19.1);
}
