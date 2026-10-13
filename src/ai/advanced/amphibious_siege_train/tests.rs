use super::*;

/// `the_storming`'s board with its army removed: a walled city of player 1.
fn walled_city() -> (Game, u32) {
    let mut g =
        crate::doctrine::build(crate::doctrine::position("the_storming").expect("known"), 3)
            .expect("buildable");
    let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
    for uid in seeded {
        g.remove_unit(uid);
    }
    let cid = *g.cities.keys().next().expect("the position states a city");
    assert_eq!(g.cities[&cid].owner, 1);
    (g, cid)
}

/// The target island: `walled_city`'s city of player 1 on land out to two
/// tiles (out to `island` tiles), a Coast ring out to six, and our land
/// beyond it with our port seven tiles west of the city. Shipbuilding, war,
/// a Catapult and two Spearmen on our side, in the target's force group.
pub(in crate::ai::advanced) fn island(island: i32) -> (Game, u32, Pos, Pos, u32, [u32; 2]) {
    let (mut g, cid) = walled_city();
    let target = g.cities[&cid].pos;
    for pos in g.map.tiles.keys().copied().collect::<Vec<_>>() {
        let distance = g.wdist(pos, target);
        let tile = g.map.tiles.get_mut(&pos).unwrap();
        tile.terrain = if (island + 1..=6).contains(&distance) {
            crate::name!("coast")
        } else {
            crate::name!("grassland")
        };
        tile.feature = None;
        tile.hills = false;
    }
    g.players[0].techs.insert(crate::name!("shipbuilding"));
    g.at_war.insert((0, 1));
    let port = (target.0 - 7, target.1);
    g.found_city_for(0, port, None);
    let gun = g.spawn_unit("catapult", 0, (target.0 - 8, target.1));
    let escorts = [
        g.spawn_unit("spearman", 0, (target.0 - 8, target.1 + 1)),
        g.spawn_unit("spearman", 0, (target.0 - 9, target.1)),
    ];
    (g, cid, target, port, gun, escorts)
}

pub(in crate::ai::advanced) fn armed(
    g: &Game,
    cid: u32,
    gun: u32,
    escorts: [u32; 2],
) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_amphibious_siege_train();
    let objective = g.cities[&cid].pos;
    ai.force_groups = vec![ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![gun, escorts[0], escorts[1]],
        anchor: objective,
        objective,
        focus_target: None,
        posture: ForcePosture::Muster,
        readiness: 0.0,
        local_strength_ratio: 1.0,
    }];
    ai
}

fn end_round(g: &mut Game) {
    assert!(g.apply(0, &Action::EndTurn).is_ok());
    assert!(g.apply(1, &Action::EndTurn).is_ok());
    assert_eq!(g.current, 0);
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.amphibious_siege_train && ai.amphibious_train.is_none());
    assert!(super::super::GENES.iter().any(|gene| gene.opt_in()
        && gene.tag == "amphibious-siege-train"
        && gene.field == "amphibious_siege_train"));
    let mut on = AdvancedAi::new();
    on.enable_amphibious_siege_train();
    assert!(on.amphibious_siege_train);
    on.disable_amphibious_siege_train();
    assert!(!on.amphibious_siege_train && on.amphibious_train.is_none());
}

#[test]
fn a_water_locked_objective_forms_a_train() {
    let (g, cid, target, port, gun, escorts) = island(2);
    let mut ai = armed(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let train = ai.amphibious_train.clone().expect("formed");
    assert_eq!(train.target, cid);
    assert_eq!(train.port_pos, port);
    assert_eq!(g.wdist(train.landing, target), 2, "the island's shore");
    assert!(!train.lane.is_empty() && train.lane.len() <= AMPHIB_SEA_LANE_MAX);
    assert!(train
        .lane
        .iter()
        .all(|pos| g.rules.is_water(g.map.get(*pos).unwrap())));
    assert_eq!(
        train.members,
        [gun, escorts[0], escorts[1]]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(train.phase, AmphibiousPhase::Assemble);
}

#[test]
fn the_landing_prefers_three_tiles_out_of_the_citys_strike() {
    let (g, cid, target, _, gun, escorts) = island(3);
    let tiles = landing_tiles(&g, 0, gun, target);
    assert!(!tiles.is_empty());
    assert_eq!(g.wdist(tiles[0], target), 3);
    let mut ai = armed(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    assert_eq!(g.wdist(ai.amphibious_train.unwrap().landing, target), 3);
}

#[test]
fn no_train_without_the_embark_technology_or_enough_escorts() {
    let (mut g, cid, _, _, gun, escorts) = island(2);
    g.players[0].techs.clear();
    let mut ai = armed(&g, cid, gun, escorts);
    assert!(!ai.form_amphibious_train(&g, 0, cid), "no Shipbuilding");

    let (mut g, cid, _, _, gun, escorts) = island(2);
    g.remove_unit(escorts[1]);
    let mut ai = armed(&g, cid, gun, escorts);
    assert!(
        !ai.form_amphibious_train(&g, 0, cid),
        "one escort is too few"
    );
}

#[test]
fn an_enemy_warship_on_the_lane_holds_the_train_and_abandons_one_in_hand() {
    let (mut g, cid, target, _, gun, escorts) = island(2);
    let mut ai = armed(&g, cid, gun, escorts);
    let watcher = g.spawn_unit("galley", 1, (target.0 - 5, target.1));
    assert!(!ai.form_amphibious_train(&g, 0, cid), "the lane is watched");

    g.remove_unit(watcher);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    g.spawn_unit("galley", 1, (target.0 - 5, target.1));
    assert!(!ai.amphibious_train_holds(&g, 0));
    assert!(ai.amphibious_train.is_none());
    assert_eq!(ai.amphibious_member_step(&mut g, 0, gun), None);
}

#[test]
fn a_gun_never_lands_alone() {
    let (mut g, cid, target, _, gun, escorts) = island(2);
    let mut ai = armed(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let landing = ai.amphibious_train.as_ref().unwrap().landing;
    // Put the whole train at sea beside the beach.
    let mut water: Vec<Pos> = g
        .wdisk(landing, 2)
        .into_iter()
        .filter(|pos| {
            g.rules.is_water(g.map.get(*pos).unwrap())
                && g.nbrs(*pos).into_iter().any(|n| n == landing)
        })
        .collect();
    water.sort_unstable();
    assert!(water.len() >= 2, "fixture: water beside the landing");
    let more: Vec<Pos> = g
        .wdisk(landing, 2)
        .into_iter()
        .filter(|pos| g.rules.is_water(g.map.get(*pos).unwrap()) && !water.contains(pos))
        .collect();
    let spots = [water[0], water[1], more[0]];
    for uid in [gun, escorts[0], escorts[1]] {
        g.remove_unit(uid);
    }
    let gun = g.spawn_unit("catapult", 0, spots[0]);
    let escorts = [
        g.spawn_unit("spearman", 0, spots[1]),
        g.spawn_unit("spearman", 0, spots[2]),
    ];
    assert!([gun, escorts[0], escorts[1]]
        .iter()
        .all(|id| g.is_embarked(&g.units[id])));
    ai.amphibious_train.as_mut().unwrap().members =
        [gun, escorts[0], escorts[1]].into_iter().collect();
    ai.amphibious_train.as_mut().unwrap().phase = AmphibiousPhase::Sail;
    let ashore = |g: &Game, id: u32| !g.is_embarked(&g.units[&id]);

    // The gun moves first: no escort is ashore, so it stays at sea.
    let _ = ai.amphibious_member_step(&mut g, 0, gun);
    assert!(!ashore(&g, gun), "a gun never lands alone");
    // The escorts land; then the gun comes ashore beside them.
    for escort in escorts {
        for _ in 0..4 {
            if ai.amphibious_member_step(&mut g, 0, escort) != Some(true) {
                break;
            }
        }
    }
    assert!(escorts.iter().any(|id| ashore(&g, *id)), "an escort lands");
    // Then the gun comes ashore beside them, moving along the shore first
    // if an escort took its beach tile.
    for _ in 0..3 {
        end_round(&mut g);
        for _ in 0..4 {
            if ai.amphibious_member_step(&mut g, 0, gun) != Some(true) {
                break;
            }
        }
        if ashore(&g, gun) {
            break;
        }
    }
    assert!(ashore(&g, gun), "the gun lands beside its escort");
    assert!(g.wdist(g.units[&gun].pos, target) <= AMPHIB_LANDING_MAX);
}

#[test]
fn the_crossing_follows_a_rebuilt_board() {
    let (g, cid, target, port, gun, escorts) = island(2);
    let mut ai = armed(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    // The live seat's fresh board renumbers every unit: respawn the members
    // in place under new ids, and lose one escort in the rebuild.
    let mut next = g.clone();
    let mut units = BTreeMap::new();
    for old in [gun, escorts[0]] {
        let (kind, pos) = (next.units[&old].kind.to_string(), next.units[&old].pos);
        next.remove_unit(old);
        units.insert(old, next.spawn_unit(&kind, 0, pos));
    }
    next.remove_unit(escorts[1]);
    // An old target id that names nothing on the new board.
    ai.amphibious_train.as_mut().unwrap().target = cid + 1_000;
    ai.remap_siege_memory(&g, &next, &units);
    let train = ai
        .amphibious_train
        .clone()
        .expect("the train survives the rebuild");
    assert_eq!(train.target, cid, "the target follows by position");
    assert_eq!(next.cities[&train.port].pos, port);
    assert_eq!(
        train.members,
        [units[&gun], units[&escorts[0]]]
            .into_iter()
            .collect::<BTreeSet<_>>()
    );
    assert_eq!(next.wdist(train.landing, target), 2);
    // A board without the unit memory forgets the crossing.
    ai.forget_unit_memory();
    assert!(ai.amphibious_train.is_none());
}

#[test]
fn the_whole_train_crosses_and_lands_together() {
    let (mut g, cid, target, _, gun, escorts) = island(2);
    let mut ai = armed(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let landing = ai.amphibious_train.as_ref().unwrap().landing;
    let members = [gun, escorts[0], escorts[1]];
    let far_shore = |g: &Game, id: u32| {
        let unit = &g.units[&id];
        !g.is_embarked(unit) && g.wdist(unit.pos, target) <= AMPHIB_LANDING_MAX
    };
    let mut released = false;
    let mut trace = String::new();
    for round in 0..20 {
        trace.push_str(&format!(
            "r{round} {:?}: {}\n",
            ai.amphibious_train.as_ref().map(|t| t.phase),
            members
                .iter()
                .map(|id| {
                    let u = &g.units[id];
                    format!(
                        "{}@{:?}{}",
                        u.kind,
                        u.pos,
                        if g.is_embarked(u) { "~" } else { "" }
                    )
                })
                .collect::<Vec<_>>()
                .join(" ")
        ));
        for uid in members {
            for _ in 0..6 {
                if !g.units.contains_key(&uid) || g.units[&uid].moves_left <= 0.0 {
                    break;
                }
                match ai.amphibious_member_step(&mut g, 0, uid) {
                    Some(true) => continue,
                    _ => break,
                }
            }
            // A gun ashore on the far side always has an escort ashore there.
            if far_shore(&g, gun) {
                assert!(
                    escorts.iter().any(|id| far_shore(&g, *id)),
                    "the gun landed alone"
                );
            }
        }
        if ai.amphibious_train.is_none() {
            released = true;
            break;
        }
        end_round(&mut g);
    }
    assert!(
        released,
        "the crossing completes and releases its members; landing {landing:?}\n{trace}"
    );
    for uid in members {
        assert!(far_shore(&g, uid), "{uid} is ashore beside the target");
        assert!(g.wdist(g.units[&uid].pos, landing) <= 2);
    }
}
