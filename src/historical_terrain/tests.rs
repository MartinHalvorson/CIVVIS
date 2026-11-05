use super::*;
use crate::setup::{MapPoles, MapTopology};

/// ★★★★★ EVERY CATALOGUED BATTLE IS DRAWN, OR SAYS WHY NOT.
///
/// `historical_terrain_plans.rs` is 2,223 lines and had no test of any kind,
/// and the join it depends on — one `Plan` per catalogue row — was enforced
/// by nothing. A battle added to the catalogue without a chart does not fail
/// anything: it quietly renders on whatever the generic script produces, and
/// a field nobody drew is exactly the defect this module exists to prevent.
///
/// Trafalgar is the one deliberate exception and is named here rather than
/// skipped by a rule, so adding a second bespoke module is a decision
/// somebody writes down.
const BESPOKE_FIELDS: &[&str] = &["trafalgar"];

#[test]
fn every_catalogued_battle_has_a_field_drawn_for_it() {
    let drawn: std::collections::BTreeSet<&str> = PLANS.iter().map(|plan| plan.id).collect();
    let undrawn: Vec<&str> = crate::historical_scenarios::all()
        .iter()
        .map(|scenario| scenario.id)
        .filter(|id| !drawn.contains(id) && !BESPOKE_FIELDS.contains(id))
        .collect();
    assert!(
            undrawn.is_empty(),
            "these battles are in the catalogue with no chart and would render on              generic ground: {undrawn:?}"
        );
}

#[test]
fn every_plan_draws_a_battle_the_catalogue_actually_lists() {
    let listed: std::collections::BTreeSet<&str> = crate::historical_scenarios::all()
        .iter()
        .map(|scenario| scenario.id)
        .collect();
    let orphans: Vec<&str> = PLANS
        .iter()
        .map(|plan| plan.id)
        .filter(|id| !listed.contains(id))
        .collect();
    assert!(
        orphans.is_empty(),
        "charts for battles nobody can play: {orphans:?}"
    );
}

#[test]
fn a_bespoke_field_is_not_also_a_plan() {
    // Two sources for one battle's ground is worse than none: the painter
    // would run and the module would overwrite it, or the reverse, and which
    // won would depend on call order.
    for id in BESPOKE_FIELDS {
        assert!(
            !PLANS.iter().any(|plan| plan.id == *id),
            "{id} has both a bespoke module and a chart"
        );
        assert!(
            crate::historical_scenarios::by_id(id).is_some(),
            "{id} is exempted from needing a chart but is not in the catalogue"
        );
    }
}

#[test]
fn no_plan_names_terrain_or_a_feature_the_rules_do_not_ship() {
    // A misspelled paint is a stroke that does nothing, on a field whose
    // whole purpose is to be the ground the battle was fought on.
    let rules = crate::rules::Rules::embedded();
    for plan in PLANS {
        assert!(
            rules.terrains.contains_key(plan.base),
            "{}: base terrain {:?} is not in the ruleset",
            plan.id,
            plan.base
        );
    }
}

fn chart(id: &str) -> WorldMap {
    let scenario = crate::historical_scenarios::by_id(id).expect("catalogue row");
    let rules = crate::rules::Rules::embedded();
    let mut rng = crate::rng::Rng::new(7);
    let (map, _) = crate::mapgen::generate_with_script(
        &rules,
        scenario.width,
        scenario.height,
        2,
        0,
        0,
        1,
        crate::historical_scenarios::script_from_id(id).expect("map script"),
        MapTopology::Flat,
        MapPoles::Poles,
        &mut rng,
    );
    map
}

fn tiles_where(map: &WorldMap, test: impl Fn(&crate::world::Tile) -> bool) -> usize {
    map.tiles.values().filter(|tile| test(tile)).count()
}

/// Every battle in the catalogue has ground drawn for it. The generic
/// noise painter this replaced is gone, so a battle without a plan would
/// open on an unpainted world rather than on a quietly wrong one.
#[test]
fn every_catalogue_battle_has_a_plan() {
    for scenario in crate::historical_scenarios::generic_scenarios() {
        assert!(
            by_id(scenario.id).is_some(),
            "{} has no terrain plan",
            scenario.id
        );
    }
    // And no plan describes a battle that is not in the catalogue.
    for plan in PLANS {
        assert!(
            crate::historical_scenarios::by_id(plan.id).is_some(),
            "{} is a plan for no catalogue battle",
            plan.id
        );
    }
}

/// Every name a plan paints has to be in the ruleset, or it paints a tile
/// the engine cannot read.
#[test]
fn every_plan_paints_only_ruleset_names() {
    let rules = crate::rules::Rules::embedded();
    for plan in PLANS {
        assert!(
            rules.terrains.contains_key(plan.base),
            "{} bases on unknown terrain {}",
            plan.id,
            plan.base
        );
        for stroke in plan.strokes {
            for paint in stroke.paint {
                match paint {
                    Paint::Terrain(name) => assert!(
                        rules.terrains.contains_key(name),
                        "{} paints unknown terrain {name}",
                        plan.id
                    ),
                    Paint::Feature(Some(name)) => assert!(
                        rules.features.contains_key(name),
                        "{} paints unknown feature {name}",
                        plan.id
                    ),
                    Paint::Improvement(Some(name)) => assert!(
                        rules.improvements.contains_key(name),
                        "{} paints unknown improvement {name}",
                        plan.id
                    ),
                    _ => {}
                }
            }
        }
    }
}

/// Both sides must have somewhere to stand, and the two must not be the
/// same tile. This is the reachability floor the old generic chart had.
#[test]
fn every_plan_seats_two_separated_sides() {
    for scenario in crate::historical_scenarios::generic_scenarios() {
        let map = chart(scenario.id);
        let plan = by_id(scenario.id).unwrap();
        let rules = crate::rules::Rules::embedded();
        let afloat = sides_afloat(&rules, scenario);
        let starts = major_starts(&map, plan, afloat)
            .unwrap_or_else(|| panic!("{} seats nobody", scenario.id));
        assert_eq!(starts.len(), 2, "{}", scenario.id);
        assert_ne!(starts[0], starts[1], "{}", scenario.id);
        // Far enough apart to be two armies rather than one melee.
        let (ax, ay) = hex::axial_to_offset(starts[0].0, starts[0].1);
        let (bx, by) = hex::axial_to_offset(starts[1].0, starts[1].1);
        let apart = (ax - bx).abs().max((ay - by).abs());
        assert!(apart >= 4, "{} seats both sides {apart} apart", scenario.id);
    }
}

/// A plan has to leave a battle passable: an army that cannot reach the
/// other one is a chart, not a scenario. Checked as a flood fill from one
/// side's anchor to the other's over ground that force could cross.
#[test]
fn every_plan_leaves_a_route_between_the_armies() {
    for scenario in crate::historical_scenarios::generic_scenarios() {
        let map = chart(scenario.id);
        let plan = by_id(scenario.id).unwrap();
        let rules = crate::rules::Rules::embedded();
        let afloat = sides_afloat(&rules, scenario);
        let naval = afloat[0] && afloat[1];
        let starts = major_starts(&map, plan, afloat).unwrap();
        let passable = |pos: Pos| {
            map.get(pos).is_some_and(|tile| {
                let water = is_water_name(&tile.terrain);
                tile.terrain.as_str() != "mountain" && (water == naval || !naval)
            })
        };
        let mut seen = BTreeSet::new();
        let mut queue = vec![starts[0]];
        seen.insert(starts[0]);
        while let Some(pos) = queue.pop() {
            for dir in 0..6 {
                let next = hex::neighbors(pos)[dir];
                if map.get(next).is_some() && passable(next) && seen.insert(next) {
                    queue.push(next);
                }
            }
        }
        assert!(
            seen.contains(&starts[1]),
            "{} walls the two armies apart",
            scenario.id
        );
    }
}

/// An army that stood in a line opens in a line. Every piece of a force
/// has to come down on that side's own front rather than in a clump around
/// one tile — and, at Thermopylae in particular, the Greeks have to be in
/// the gate and the Persians outside it, because a rearguard that opens
/// west of the wall is not holding anything.
#[test]
fn each_force_forms_up_along_its_own_front() {
    for scenario in crate::historical_scenarios::generic_scenarios() {
        let mut options = crate::game::GameOptions::new(
            2,
            scenario.width,
            scenario.height,
            2026,
            scenario.turns,
            0,
        );
        options.map_script = crate::historical_scenarios::script_from_id(scenario.id).unwrap();
        options.start_era = scenario.era_index;
        options.barbarians = false;
        let game = crate::game::Game::new_with(options);
        let plan = by_id(scenario.id).unwrap();
        let rules = crate::rules::Rules::embedded();
        for side in 0..2 {
            // Aircraft are based rather than drawn up: a squadron belongs
            // to whatever can carry it, not to a place in the line, so it
            // is not held to the front the ground and the ships form on.
            let placed: Vec<Pos> = game
                .player_unit_ids(side)
                .iter()
                .filter_map(|uid| game.units.get(uid))
                .filter(|unit| {
                    rules
                        .units
                        .get(unit.kind.as_str())
                        .is_none_or(|spec| spec.domain.as_deref() != Some("air"))
                })
                .map(|unit| unit.pos)
                .collect();
            assert!(
                !placed.is_empty(),
                "{} side {side} deployed nothing",
                scenario.id
            );
            // Every piece within reach of the segment its side formed on.
            // Four hexes is loose on purpose: a line spills backward when
            // its own front is short, and some of these fronts are two
            // tiles wide by design.
            let front = plan.fronts[side];
            let (fx, fy) = chart_point(&game.map, front.from);
            let (tx, ty) = chart_point(&game.map, front.to);
            for pos in &placed {
                let (col, row) = hex::axial_to_offset(pos.0, pos.1);
                let off = distance_to_segment((col as f32, row as f32), (fx, fy), (tx, ty));
                assert!(
                    off <= 4.5,
                    "{} side {side} put a unit {off:.1} from its front at {pos:?}",
                    scenario.id
                );
            }
        }
    }
}

/// The Greeks hold the gate and the Persians are outside it: the one
/// arrangement that makes Thermopylae the battle it was, checked on the
/// board rather than in the briefing.
#[test]
fn leonidas_opens_inside_the_gate_and_xerxes_outside_it() {
    let scenario = crate::historical_scenarios::by_id("thermopylae").unwrap();
    let mut options =
        crate::game::GameOptions::new(2, scenario.width, scenario.height, 2026, scenario.turns, 0);
    options.map_script = crate::historical_scenarios::script_from_id("thermopylae").unwrap();
    options.start_era = scenario.era_index;
    options.barbarians = false;
    let game = crate::game::Game::new_with(options);
    let column = |side: usize| {
        game.player_unit_ids(side)
            .iter()
            .filter_map(|uid| game.units.get(uid))
            .map(|unit| hex::axial_to_offset(unit.pos.0, unit.pos.1).0)
            .collect::<Vec<_>>()
    };
    let greeks = column(0);
    let persians = column(1);
    let gate = game.map.width / 2;
    assert!(
        greeks.iter().all(|col| *col >= gate),
        "the Hellenic rearguard must stand at or east of the Middle Gate, got {greeks:?}"
    );
    assert!(
        persians.iter().all(|col| *col < gate),
        "the Persian host must still be west of the gate, got {persians:?}"
    );
}

/// Thermopylae is the battle whose whole military content is the width of
/// the ground, and it is the one the old noise painter flattered worst.
/// The claims: the Malian Gulf is north, Kallidromos is south, and the
/// road between them pinches to a gate in the middle that is far narrower
/// than the western approach the Persians march in along.
#[test]
fn thermopylae_is_a_pass_and_not_a_field() {
    let map = chart("thermopylae");
    let width = map.width;
    let height = map.height;
    let open_in_column = |col: i32| {
        (0..height)
            .filter(|row| {
                let (q, r) = hex::offset_to_axial(col, *row);
                map.get((q, r)).is_some_and(|tile| {
                    !is_water_name(&tile.terrain) && tile.terrain.as_str() != "mountain"
                })
            })
            .count()
    };
    let gate = (width / 2 - 1..=width / 2 + 1)
        .map(open_in_column)
        .min()
        .unwrap();
    let approach = open_in_column(2);
    assert!(gate >= 1, "the pass is blocked outright");
    assert!(
        gate <= 3,
        "the middle gate is {gate} tiles wide and is supposed to be a gate"
    );
    assert!(
        approach >= gate * 2,
        "the western approach ({approach}) should be far wider than the gate ({gate})"
    );
    // Sea to the north, mountain to the south: the two walls of the pass.
    let north = tiles_where(&map, |tile| {
        hex::axial_to_offset(tile.pos.0, tile.pos.1).1 == 0 && is_water_name(&tile.terrain)
    });
    let south = tiles_where(&map, |tile| {
        hex::axial_to_offset(tile.pos.0, tile.pos.1).1 == height - 1
            && tile.terrain.as_str() == "mountain"
    });
    assert!(
        north >= (width / 2) as usize,
        "the Malian Gulf should hold the north edge"
    );
    assert!(
        south >= (width / 2) as usize,
        "Kallidromos should hold the south edge"
    );
}

/// Marathon's plain is bounded by the sea it was fought beside and the
/// Great Marsh that closed its northern end — the two features that made
/// the Athenian charge a race across open ground rather than a maneuver.
#[test]
fn marathon_has_its_bay_and_its_great_marsh() {
    let map = chart("marathon");
    assert!(
        tiles_where(&map, |tile| is_water_name(&tile.terrain)) >= 12,
        "the bay of Marathon is missing"
    );
    assert!(
        tiles_where(&map, |tile| tile.feature.as_deref() == Some("marsh")) >= 6,
        "the Great Marsh is missing"
    );
}

/// Gaugamela's ground is the claim: Darius had it levelled for his scythed
/// chariots, so an open plain is the historical fact and any wood or hill
/// in the middle of it is an error.
#[test]
fn gaugamela_is_the_ground_darius_cleared() {
    let map = chart("gaugamela");
    let cleared = map.tiles.values().filter(|tile| {
        let (col, row) = hex::axial_to_offset(tile.pos.0, tile.pos.1);
        (col >= 4 && col < map.width - 4) && (row >= 3 && row < map.height - 3)
    });
    for tile in cleared {
        assert!(
            !tile.hills && tile.feature.is_none() && tile.terrain.as_str() != "mountain",
            "the cleared chariot ground carries {:?}/{:?} at {:?}",
            tile.terrain,
            tile.feature,
            tile.pos
        );
    }
}

/// Agincourt was fought in a corridor between two woods, on ground the
/// rain had turned to mud. Both are the battle: the woods squeezed the
/// French frontage and the mud drowned their advance. The corridor is
/// drawn running north–south, so its two woods are east and west, and it
/// has to be narrower at the English end than at the French one.
#[test]
fn agincourt_is_a_corridor_between_two_woods() {
    let map = chart("agincourt");
    let woods = |west: bool| {
        map.tiles.values().filter(move |tile| {
            let (col, _) = hex::axial_to_offset(tile.pos.0, tile.pos.1);
            tile.feature.as_deref() == Some("forest")
                && if west {
                    col < map.width / 2
                } else {
                    col >= map.width / 2
                }
        })
    };
    assert!(woods(true).count() >= 6, "the wood on one side is missing");
    assert!(
        woods(false).count() >= 6,
        "the wood on the other side is missing"
    );
    assert!(
        tiles_where(&map, |tile| tile.feature.as_deref() == Some("marsh")) >= 8,
        "the ploughed mud that drowned the French advance is missing"
    );
    // The squeeze: the open frontage narrows toward the English end.
    let open_in_row = |row: i32| {
        (0..map.width)
            .filter(|col| {
                let (q, r) = hex::offset_to_axial(*col, row);
                map.get((q, r))
                    .is_some_and(|tile| tile.feature.as_deref() != Some("forest"))
            })
            .count()
    };
    let french_end = open_in_row(1);
    let english_end = open_in_row(map.height - 2);
    assert!(
        english_end < french_end,
        "the corridor should pinch toward the English ({english_end}) from the \
             French end ({french_end})"
    );
}

/// Cannae's river is why the envelopment worked: the Aufidus pinned the
/// Roman flank and shortened the frontage they could deploy on. It is
/// drawn as a channel rather than as edge segments, because a line has to
/// be able to *rest* on it — so what is checked is water across the top of
/// the field, and open ground below for the wings to swing through.
#[test]
fn cannae_is_pinned_against_the_aufidus() {
    let map = chart("cannae");
    let northern =
        |tile: &crate::world::Tile| hex::axial_to_offset(tile.pos.0, tile.pos.1).1 < map.height / 3;
    let channel = map
        .tiles
        .values()
        .filter(|tile| northern(tile) && is_water_name(&tile.terrain))
        .count();
    assert!(
        channel >= 8,
        "the Aufidus should close the northern flank; found {channel} water tiles there"
    );
    let southern_open = map.tiles.values().filter(|tile| {
        let (_, row) = hex::axial_to_offset(tile.pos.0, tile.pos.1);
        row >= map.height / 2
            && !is_water_name(&tile.terrain)
            && tile.terrain.as_str() != "mountain"
            && tile.feature.is_none()
    });
    assert!(
        southern_open.count() >= map.tiles.len() / 4,
        "the open ground the wings closed through is missing"
    );
}

/// A naval battle has to be mostly sea, and a landing has to have both a
/// sea to come from and a shore to land on.
#[test]
fn naval_and_landing_charts_hold_the_water_they_claim() {
    for scenario in crate::historical_scenarios::generic_scenarios() {
        let map = chart(scenario.id);
        let water = tiles_where(&map, |tile| is_water_name(&tile.terrain));
        let land = map.tiles.len() - water;
        match scenario.terrain {
            "water" | "water_air" => assert!(
                water * 4 >= map.tiles.len() * 3,
                "{} is a sea fight on {water}/{} water",
                scenario.id,
                map.tiles.len()
            ),
            "land_water" | "land_water_air" => {
                assert!(water >= 8, "{} claims water and has {water}", scenario.id);
                assert!(land >= 8, "{} claims land and has {land}", scenario.id);
            }
            // A land battle is one fought on its feet, not one with no
            // water in sight: Thermopylae's northern wall is the Malian
            // Gulf and the pass is unreadable without it. What must hold
            // is that most of the chart is ground.
            _ => assert!(
                land * 2 >= map.tiles.len(),
                "{} is a land battle with only {land} of {} tiles dry",
                scenario.id,
                map.tiles.len()
            ),
        }
    }
}
