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

// ---- amphibious-siege-train-2 ---------------------------------------------

/// `armed` with version two alone, so every test below also shows version
/// two runs without version one.
fn armed_2(g: &Game, cid: u32, gun: u32, escorts: [u32; 2]) -> AdvancedAi {
    let mut ai = armed(g, cid, gun, escorts);
    ai.disable_amphibious_siege_train();
    ai.enable_amphibious_siege_train_2();
    ai
}

/// Every member steps until it stops acting, round after round, until the
/// train lets go or `rounds` pass. Returns whether it let go, the trace, and
/// each member's count of land-to-sea and sea-to-land switches. A gun ashore
/// within `far` of the target always has an escort ashore there.
fn run_crossing(
    g: &mut Game,
    ai: &mut AdvancedAi,
    gun: u32,
    escorts: [u32; 2],
    target: Pos,
    far: i32,
    rounds: usize,
) -> (bool, String, BTreeMap<u32, usize>) {
    let members = [gun, escorts[0], escorts[1]];
    let far_shore = |g: &Game, id: u32| {
        let unit = &g.units[&id];
        !g.is_embarked(unit) && g.wdist(unit.pos, target) <= far
    };
    let mut switches: BTreeMap<u32, usize> = members.iter().map(|id| (*id, 0)).collect();
    let mut trace = String::new();
    for round in 0..rounds {
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
                let before = g.is_embarked(&g.units[&uid]);
                let acted = ai.amphibious_member_step(g, 0, uid);
                if g.is_embarked(&g.units[&uid]) != before {
                    *switches.get_mut(&uid).unwrap() += 1;
                }
                if acted != Some(true) {
                    break;
                }
            }
            if far_shore(g, gun) {
                assert!(
                    escorts.iter().any(|id| far_shore(g, *id)),
                    "the gun landed alone\n{trace}"
                );
            }
        }
        if ai.amphibious_train.is_none() {
            return (true, trace, switches);
        }
        end_round(g);
    }
    (false, trace, switches)
}

#[test]
fn version_two_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.amphibious_siege_train_2);
    assert!(super::super::GENES.iter().any(|gene| gene.opt_in()
        && gene.tag == "amphibious-siege-train-2"
        && gene.field == "amphibious_siege_train_2"));
    // Off, a crossing's members are not locked.
    let (g, cid, _, _, gun, escorts) = island(2);
    let mut ai = armed(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    assert!(ai.amphibious_member(gun) && !ai.amphibious_locked(gun));
}

#[test]
fn version_two_alone_crosses_a_sea_lane() {
    let (mut g, cid, target, _, gun, escorts) = island(2);
    let mut ai = armed_2(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    assert!(
        !ai.amphibious_train.as_ref().unwrap().ford,
        "four water tiles is no ford"
    );
    let landing = ai.amphibious_train.as_ref().unwrap().landing;
    let (released, trace, switches) = run_crossing(
        &mut g,
        &mut ai,
        gun,
        escorts,
        target,
        AMPHIB_LANDING_MAX,
        20,
    );
    assert!(
        released,
        "the crossing completes; landing {landing:?}\n{trace}"
    );
    for (uid, count) in switches {
        assert!(
            g.wdist(g.units[&uid].pos, landing) <= AMPHIB_BEACH_2,
            "{uid} on the beach\n{trace}"
        );
        assert!(
            count <= 2,
            "{uid} embarked once and landed once, not {count} switches\n{trace}"
        );
    }
}

#[test]
fn the_ford_search_finds_a_strait_of_one_or_two_water_tiles() {
    for (island_radius, water) in [(5, 1), (4, 2)] {
        let (g, _, target, _, gun, _) = island(island_radius);
        let start = g.units[&gun].pos;
        let (shore, lane, landing) = ford_crossing(
            &g,
            0,
            gun,
            start,
            target,
            super::super::siege_train::STAGING_FAR,
        )
        .unwrap_or_else(|| panic!("island {island_radius}: a ford"));
        assert_eq!(lane.len(), water, "island {island_radius}");
        assert_eq!(g.wdist(shore, target), 7, "the near shore is our land");
        assert_eq!(
            g.wdist(landing, target),
            island_radius,
            "the far shore is theirs"
        );
        assert!(lane
            .iter()
            .all(|pos| g.rules.is_water(g.map.get(*pos).unwrap())));
        assert!(g.nbrs(shore).contains(&lane[0]) && g.nbrs(landing).contains(&lane[water - 1]));
    }
    // Three water tiles is no ford: version two falls back to the sea lane.
    let (g, cid, target, _, gun, escorts) = island(3);
    assert!(ford_crossing(&g, 0, gun, g.units[&gun].pos, target, 5).is_none());
    let mut ai = armed_2(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    assert!(!ai.amphibious_train.as_ref().unwrap().ford);
}

#[test]
fn a_one_tile_strait_is_forded_and_crossed_together() {
    let (mut g, cid, target, _, gun, escorts) = island(5);
    // The column south of the port, so the near shore is no city.
    for uid in [gun, escorts[0], escorts[1]] {
        g.remove_unit(uid);
    }
    let gun = g.spawn_unit("catapult", 0, (7, 11));
    let escorts = [
        g.spawn_unit("spearman", 0, (6, 11)),
        g.spawn_unit("spearman", 0, (7, 12)),
    ];
    // Version one finds no landing within three of the city.
    let mut v1 = armed(&g, cid, gun, escorts);
    assert!(
        !v1.form_amphibious_train(&g, 0, cid),
        "no landing beside water within 3"
    );
    let mut ai = armed_2(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let train = ai.amphibious_train.clone().unwrap();
    assert!(train.ford && train.lane.len() == 1);
    assert!(
        g.city_at(train.port_pos).is_none(),
        "the gathering point is the shore"
    );
    assert_eq!(g.wdist(train.port_pos, target), 7);
    assert_eq!(g.wdist(train.landing, target), 5);
    assert_eq!(train.members.len(), 3);
    // The ford follows a rebuilt board although its shore holds no city.
    let mut next = g.clone();
    let mut units = BTreeMap::new();
    for old in [gun, escorts[0], escorts[1]] {
        let (kind, pos) = (next.units[&old].kind.to_string(), next.units[&old].pos);
        next.remove_unit(old);
        units.insert(old, next.spawn_unit(&kind, 0, pos));
    }
    let mut follow = ai.clone();
    follow.remap_siege_memory(&g, &next, &units);
    let followed = follow
        .amphibious_train
        .expect("a ford survives the rebuild");
    assert!(followed.ford && followed.members.contains(&units[&gun]));
    // March to the shore, cross, and land on the far side: one embark and
    // one landing each, a gun never ashore there without an escort.
    let (released, trace, switches) = run_crossing(&mut g, &mut ai, gun, escorts, target, 5, 12);
    assert!(released, "the ford completes\n{trace}");
    for (uid, count) in switches {
        let unit = &g.units[&uid];
        assert!(
            !g.is_embarked(unit) && g.wdist(unit.pos, target) <= 5,
            "{uid} across\n{trace}"
        );
        assert_eq!(count, 2, "{uid} embarked once and landed once\n{trace}");
    }
}

#[test]
fn members_are_locked_against_combining_upgrading_and_re_tasking() {
    let (mut g, cid, _, _, gun, escorts) = island(2);
    // The escorts side by side, and spare bodies so the reserve allows a
    // combination.
    let beside = g
        .nbrs(g.units[&escorts[0]].pos)
        .into_iter()
        .filter(|pos| landable(&g, *pos) && g.unit_ids_at(*pos).is_empty())
        .min()
        .expect("free land beside the escort");
    g.remove_unit(escorts[1]);
    let escorts = [escorts[0], g.spawn_unit("spearman", 0, beside)];
    let target = g.cities[&cid].pos;
    for k in 0..6 {
        g.spawn_unit("warrior", 0, (target.0 - 12 - 2 * k, target.1 + 4));
    }
    g.players[0].civics.insert(crate::name!("nationalism"));
    let settler = g.spawn_unit("settler", 0, (target.0 - 12, target.1 - 4));
    for version in [1, 2] {
        let mut g = g.clone();
        let mut ai = if version == 1 {
            armed(&g, cid, gun, escorts)
        } else {
            armed_2(&g, cid, gun, escorts)
        };
        assert!(ai.form_amphibious_train(&g, 0, cid));
        let locked = version == 2;
        assert!(escorts
            .iter()
            .chain([&gun])
            .all(|id| ai.amphibious_locked(*id) == locked));
        // No upgrade: every Gold upgrade but the science core's asks this.
        assert_eq!(
            ai.air_resource_upgrade_preserves_wing(&g, 0, gun, crate::name!("trebuchet"), 0.0),
            !locked
        );
        // Not pooled by the objective board.
        let pool = ai.board_pool(&g, 0);
        assert_eq!(pool.contains(&escorts[0]), !locked);
        // Not bound as a settler's guard.
        ai.bind_settler_guard(&g, settler, escorts[1]);
        assert_eq!(
            ai.settler_guards.get(&settler) == Some(&escorts[1]),
            !locked
        );
        // Not combined into a Corps.
        ai.settler_guards.clear();
        ai.advanced_formations(&mut g, 0);
        let both = escorts
            .iter()
            .all(|id| g.units.get(id).is_some_and(|u| u.formation == 0));
        assert_eq!(
            both, locked,
            "version {version}: the escorts combine only when unlocked"
        );
    }
}

#[test]
fn a_member_that_does_not_close_stays_behind() {
    let (mut g, cid, _, port, gun, escorts) = island(2);
    // One escort far from the port that never moves.
    g.remove_unit(escorts[1]);
    let straggler = g.spawn_unit("spearman", 0, (port.0 - 6, port.1));
    let third = g.spawn_unit("spearman", 0, (port.0 - 1, port.1 - 1));
    let mut ai = armed_2(&g, cid, gun, [escorts[0], straggler]);
    ai.force_groups[0].units.push(third);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let train = ai.amphibious_train.clone().unwrap();
    assert!(train.members.contains(&straggler) && train.members.contains(&third));
    let near = [gun, escorts[0], third];
    let mut rounds = 0;
    while ai.amphibious_train.as_ref().unwrap().phase == AmphibiousPhase::Assemble {
        assert!(
            rounds < AMPHIB_STALL_TURNS as usize + 4,
            "the train never sails"
        );
        for uid in near {
            for _ in 0..4 {
                if ai.amphibious_member_step(&mut g, 0, uid) != Some(true) {
                    break;
                }
            }
        }
        if ai.amphibious_train.as_ref().unwrap().phase == AmphibiousPhase::Assemble {
            // Waited on until it has failed to close for two turns.
            let waited = ai.amphibious_waited_on(&g, ai.amphibious_train.as_ref().unwrap());
            assert!(
                waited.contains(&straggler)
                    || ai.amphibious_train.as_ref().unwrap().progress[&straggler].1
                        >= AMPHIB_STALL_TURNS
            );
        }
        end_round(&mut g);
        rounds += 1;
    }
    let train = ai.amphibious_train.clone().unwrap();
    assert!(
        !train.members.contains(&straggler),
        "the straggler stays behind"
    );
    assert!(!ai.amphibious_locked(straggler));
    assert!(near.iter().all(|id| train.members.contains(id)));
}

#[test]
fn the_cohesion_wait_blames_the_straggler_not_the_waiters() {
    let (mut g, cid, _, _, gun, escorts) = island(2);
    let mut ai = armed_2(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let landing = ai.amphibious_train.as_ref().unwrap().landing;
    let mut water: Vec<(i32, Pos)> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| g.rules.is_water(g.map.get(*pos).unwrap()))
        .map(|pos| (g.wdist(pos, landing), pos))
        .collect();
    water.sort_unstable();
    let at = |d: i32| {
        water
            .iter()
            .filter(move |(w, _)| *w == d)
            .map(|(_, pos)| *pos)
    };
    let (a, b) = (at(4).next().unwrap(), at(4).nth(1).unwrap());
    let (reach, far) = *water.last().unwrap();
    assert!(
        reach > 4 + AMPHIB_COHESION,
        "fixture: a straggler well behind"
    );
    for uid in [gun, escorts[0], escorts[1]] {
        g.remove_unit(uid);
    }
    let gun = g.spawn_unit("catapult", 0, a);
    let escort = g.spawn_unit("spearman", 0, b);
    let straggler = g.spawn_unit("spearman", 0, far);
    {
        let train = ai.amphibious_train.as_mut().unwrap();
        train.members = [gun, escort, straggler].into_iter().collect();
        train.phase = AmphibiousPhase::Sail;
        train.progress.clear();
    }
    // Nobody moves for three readings: the waiters are held, the straggler
    // stalls.
    for _ in 0..=AMPHIB_STALL_TURNS {
        ai.amphibious_track_progress(&g);
        end_round(&mut g);
    }
    let train = ai.amphibious_train.clone().unwrap();
    assert_eq!(train.progress[&gun].1, 0);
    assert_eq!(train.progress[&escort].1, 0);
    assert_eq!(train.progress[&straggler].1, AMPHIB_STALL_TURNS);
    assert_eq!(ai.amphibious_waited_on(&g, &train), vec![gun, escort]);
    // Version one waits on everyone.
    let mut v1 = ai.clone();
    v1.enable_amphibious_siege_train();
    v1.disable_amphibious_siege_train_2();
    assert_eq!(v1.amphibious_waited_on(&g, &train).len(), 3);
}

#[test]
fn version_two_sails_on_water_only() {
    let (mut g, cid, target, port, gun, escorts) = island(2);
    // Islets on the straight lane from the port to the island, two tiles
    // from the landing: no ford (the islets are not the target's land).
    for x in [target.0 - 4, target.0 - 5] {
        let tile = g.map.tiles.get_mut(&(x, target.1)).unwrap();
        tile.terrain = crate::name!("grassland");
    }
    let islets = g.clone();
    let mut ai = armed_2(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    assert!(!ai.amphibious_train.as_ref().unwrap().ford);
    let landing = ai.amphibious_train.as_ref().unwrap().landing;
    let zone = beach_zone(&g, landing);
    let (released, trace, switches) = run_crossing(
        &mut g,
        &mut ai,
        gun,
        escorts,
        target,
        AMPHIB_LANDING_MAX,
        24,
    );
    assert!(released, "the crossing completes from {port:?}\n{trace}");
    for (uid, count) in switches {
        assert!(
            count <= 2,
            "{uid} hopped ashore mid-crossing ({count} switches)\n{trace}"
        );
        assert!(
            zone.contains(&g.units[&uid].pos),
            "{uid} on the island's beach\n{trace}"
        );
    }
    // Version one's mixed router walks the gun onto the islets, and its
    // two-tile beach counts the islet as ashore: the gun is left there.
    let mut g = islets;
    let mut v1 = armed(&g, cid, gun, escorts);
    assert!(v1.form_amphibious_train(&g, 0, cid));
    let (released, trace, _) = run_crossing(
        &mut g,
        &mut v1,
        gun,
        escorts,
        target,
        AMPHIB_LANDING_MAX,
        24,
    );
    assert!(
        released && !zone.contains(&g.units[&gun].pos),
        "version one strands the gun\n{trace}"
    );
    // The router itself never steps ashore.
    let at_sea = g.spawn_unit("spearman", 0, (target.0 - 6, target.1));
    assert!(g.is_embarked(&g.units[&at_sea]));
    let goals: std::collections::HashSet<Pos> = [(target.0 - 3, target.1 + 1)].into();
    if let Some(step) = water_step(&g, 0, at_sea, at_sea, &goals) {
        assert!(g.rules.is_water(g.map.get(step).unwrap()));
    }
}

#[test]
fn version_two_lands_on_a_wider_beach_once_within_three() {
    let (mut g, cid, _, _, gun, escorts) = island(2);
    let mut ai = armed_2(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let landing = ai.amphibious_train.as_ref().unwrap().landing;
    let zone = beach_zone(&g, landing);
    // Water three tiles from the landing beside a beach tile two from it,
    // where no tile beside the water is the landing's own ring.
    let mut spots: Vec<(Pos, Pos)> = zone
        .iter()
        .filter(|b| g.wdist(**b, landing) == 2)
        .flat_map(|b| g.nbrs(*b).into_iter().map(move |w| (w, *b)))
        .filter(|(w, _)| {
            g.map.get(*w).is_some_and(|tile| g.rules.is_water(tile))
                && g.wdist(*w, landing) == 3
                && g.nbrs(*w)
                    .into_iter()
                    .all(|n| g.wdist(n, landing) >= 2 || !landable(&g, n))
        })
        .collect();
    spots.sort_unstable();
    let (water, _) = *spots
        .first()
        .expect("fixture: water three out beside the wide beach");
    let others: Vec<Pos> = g
        .wdisk(landing, 3)
        .into_iter()
        .filter(|pos| {
            *pos != water
                && g.rules.is_water(g.map.get(*pos).unwrap())
                && g.wdist(*pos, landing) == 3
        })
        .collect();
    for uid in [gun, escorts[0], escorts[1]] {
        g.remove_unit(uid);
    }
    let escort = g.spawn_unit("spearman", 0, water);
    let gun = g.spawn_unit("catapult", 0, others[0]);
    let second = g.spawn_unit("spearman", 0, others[1]);
    for version in [1, 2] {
        let mut g = g.clone();
        let mut ai = ai.clone();
        if version == 1 {
            ai.enable_amphibious_siege_train();
            ai.disable_amphibious_siege_train_2();
        }
        let train = ai.amphibious_train.as_mut().unwrap();
        train.members = [gun, escort, second].into_iter().collect();
        train.phase = AmphibiousPhase::Sail;
        let _ = ai.amphibious_member_step(&mut g, 0, escort);
        let unit = &g.units[&escort];
        if version == 2 {
            assert!(!g.is_embarked(unit), "version two lands within three");
            assert!(zone.contains(&unit.pos) && g.wdist(unit.pos, landing) <= AMPHIB_BEACH_2);
        } else {
            assert!(g.is_embarked(unit), "version one waits for all within two");
        }
    }
}

#[test]
fn a_ford_that_cannot_muster_falls_back_to_the_sea_lane() {
    // `island(3)`: a three-tile sea lane from the port, and a land bridge
    // on the south side that leaves a two-tile ford there, more than ten
    // tiles from the port, where only one gun of the group stands.
    let (mut g, cid, target, port, gun, escorts) = island(3);
    let bridge = (12, 10);
    assert_eq!(
        g.wdist(bridge, target),
        4,
        "fixture: the bridge is in the ring"
    );
    g.map.tiles.get_mut(&bridge).unwrap().terrain = crate::name!("grassland");
    let south = g.spawn_unit("catapult", 0, (14, 13));
    let (shore, lane, landing) =
        ford_crossing(&g, 0, south, g.units[&south].pos, target, 5).expect("the south ford");
    assert_eq!((lane.len(), landing), (2, bridge));
    assert!(escorts
        .iter()
        .all(|id| g.wdist(g.units[id].pos, shore) > AMPHIB_MUSTER_RADIUS));
    assert!(g.wdist(port, shore) > AMPHIB_MUSTER_RADIUS);
    let mut ai = armed_2(&g, cid, gun, escorts);
    ai.force_groups[0].units.insert(0, south);
    let refusal = ai
        .amphibious_ford(&g, 0, cid, target, &[south, gun, escorts[0], escorts[1]])
        .expect("a ford is found")
        .expect_err("no escort within reach of the ford");
    assert!(refusal.contains(&format!("{shore:?}")), "{refusal}");
    // Version two still ships what version one ships: the sea lane.
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let train = ai.amphibious_train.clone().unwrap();
    assert!(!train.ford, "{train:?}");
    assert!(train.members.contains(&gun) && !train.members.contains(&south));
    let mut v1 = armed(&g, cid, gun, escorts);
    v1.force_groups[0].units.insert(0, south);
    assert!(v1.form_amphibious_train(&g, 0, cid));
    assert_eq!(v1.amphibious_train.unwrap().lane, train.lane);
}

/// The ford fixture of `a_one_tile_strait_is_forded_and_crossed_together`:
/// a one-tile strait west of `island(5)`'s target, our column south of the
/// port, version two armed.
fn ford_fixture() -> (Game, u32, Pos, u32, [u32; 2]) {
    let (mut g, cid, target, _, gun, escorts) = island(5);
    for uid in [gun, escorts[0], escorts[1]] {
        g.remove_unit(uid);
    }
    let gun = g.spawn_unit("catapult", 0, (7, 11));
    let escorts = [
        g.spawn_unit("spearman", 0, (6, 11)),
        g.spawn_unit("spearman", 0, (7, 12)),
    ];
    (g, cid, target, gun, escorts)
}

#[test]
fn a_ford_is_held_only_by_a_warship_beside_it_or_within_a_turns_reach() {
    let (mut g, cid, target, gun, escorts) = ford_fixture();
    let mut ai = armed_2(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let train = ai.amphibious_train.clone().unwrap();
    assert!(train.ford && train.lane.len() == 1);
    let ford = train.lane.clone();
    let is_water = |g: &Game, pos: Pos| g.rules.is_water(g.map.get(pos).unwrap());

    // A warship three tiles off on a lake it cannot leave: the sea lane's
    // three-tile watch would abort, the ford's does not.
    let lake = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| {
            g.wdist(*pos, target) >= 9
                && g.wdist(*pos, ford[0]) == 3
                && g.unit_ids_at(*pos).is_empty()
                && g.city_at(*pos).is_none()
        })
        .min()
        .expect("our land three tiles from the ford");
    g.map.tiles.get_mut(&lake).unwrap().terrain = crate::name!("coast");
    let watcher = g.spawn_unit("galley", 1, lake);
    assert!(lane_watched(&g, 0, &ford), "the three-tile watch sees it");
    assert!(
        !ford_watched(&g, 0, &ford),
        "but it cannot reach the ford this turn"
    );
    assert!(ai.amphibious_train_holds(&g, 0), "the ford proceeds");
    let mut fresh = armed_2(&g, cid, gun, escorts);
    assert!(fresh.form_amphibious_train(&g, 0, cid));
    assert!(fresh.amphibious_train.as_ref().unwrap().ford, "and forms");
    g.remove_unit(watcher);

    // A warship beside the ford holds it, and abandons the ford in hand.
    let beside = g
        .nbrs(ford[0])
        .into_iter()
        .filter(|pos| is_water(&g, *pos) && !ford.contains(pos))
        .min()
        .expect("water beside the ford");
    let watcher = g.spawn_unit("galley", 1, beside);
    assert!(ford_watched(&g, 0, &ford));
    assert!(
        !ai.amphibious_train_holds(&g, 0),
        "abandons the ford in hand"
    );
    let mut fresh = armed_2(&g, cid, gun, escorts);
    assert!(
        !fresh.form_amphibious_train(&g, 0, cid) || !fresh.amphibious_train.as_ref().unwrap().ford,
        "no ford forms under a warship beside it"
    );
    g.remove_unit(watcher);

    // A warship three tiles off on water joined to the ford, within its
    // moves (a Galley sails three), holds it too.
    let reach = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| is_water(&g, *pos) && *pos != lake && g.wdist(*pos, ford[0]) == 3)
        .min()
        .expect("the strait's water three tiles along");
    g.spawn_unit("galley", 1, reach);
    assert!(ford_watched(&g, 0, &ford), "it reaches the ford this turn");
}

#[test]
fn version_two_keeps_the_three_tile_watch_on_a_sea_lane() {
    let (mut g, cid, _, _, gun, escorts) = island(2);
    let mut ai = armed_2(&g, cid, gun, escorts);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let train = ai.amphibious_train.clone().unwrap();
    assert!(!train.ford, "four water tiles is a sea lane");
    let lane = train.lane.clone();
    let three = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| {
            g.rules.is_water(g.map.get(*pos).unwrap())
                && lane.iter().map(|tile| g.wdist(*tile, *pos)).min() == Some(3)
        })
        .min()
        .expect("water three tiles from the lane");
    g.spawn_unit("galley", 1, three);
    assert!(
        !ai.amphibious_train_holds(&g, 0),
        "a warship within three of a sea lane still abandons it"
    );
}

#[test]
fn a_ford_train_gathers_only_units_joined_to_its_near_shore() {
    let (mut g, cid, target, gun, escorts) = ford_fixture();
    let mut probe = armed_2(&g, cid, gun, escorts);
    assert!(probe.form_amphibious_train(&g, 0, cid));
    let (shore, landing) = {
        let train = probe.amphibious_train.as_ref().unwrap();
        (train.port_pos, train.landing)
    };
    // One escort four tiles back on our land, so the gathering waits on it.
    let back = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| {
            g.wdist(*pos, shore) == 4
                && g.wdist(*pos, target) > g.wdist(shore, target)
                && landable(&g, *pos)
                && g.unit_ids_at(*pos).is_empty()
        })
        .min()
        .expect("our land four tiles behind the shore");
    g.remove_unit(escorts[1]);
    let escorts = [escorts[0], g.spawn_unit("spearman", 0, back)];
    // A gun and an escort of the group already on the target's side of the
    // strait, nearer the ford than our column (live Immortal 204616Z).
    let far_side: Vec<Pos> = std::iter::once(landing)
        .chain(g.nbrs(landing))
        .filter(|pos| g.wdist(*pos, target) <= 5 && landable(&g, *pos))
        .take(2)
        .collect();
    assert_eq!(far_side.len(), 2, "two tiles across the strait");
    let far_gun = g.spawn_unit("catapult", 0, far_side[0]);
    let far_escort = g.spawn_unit("spearman", 0, far_side[1]);
    let mut ai = armed_2(&g, cid, gun, escorts);
    let journal = crate::reasoning::Journal::recording();
    ai.attach_journal(journal.handle());
    ai.force_groups[0].units.extend([far_gun, far_escort]);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let train = ai.amphibious_train.clone().unwrap();
    assert!(train.ford);
    assert!(
        !train.members.contains(&far_gun) && !train.members.contains(&far_escort),
        "units across the strait cannot gather at the near shore: {:?}",
        train.members
    );
    assert_eq!(
        train.members,
        BTreeSet::from([gun, escorts[0], escorts[1]]),
        "the near-shore column is the train"
    );
    let (released, trace, _) = run_crossing(&mut g, &mut ai, gun, escorts, target, 5, 12);
    assert!(
        released,
        "the gathering completes and the ford is crossed\n{trace}"
    );
    // The gathering is journalled each turn it lasts.
    let gathers: Vec<String> = journal
        .since(0)
        .thoughts
        .iter()
        .filter(|thought| thought.headline.starts_with("Siege train gathers for"))
        .map(|thought| thought.detail.clone())
        .collect();
    assert!(!gathers.is_empty(), "the gathering is journalled\n{trace}");
    assert!(
        gathers[0].ends_with(&format!(
            "2 of 3 within 2 of the ford; waiting on spearman {} at d=4",
            escorts[1]
        )),
        "{gathers:?}"
    );
}

#[test]
fn a_unit_with_a_dry_road_is_not_drafted_into_a_sea_lane() {
    let (mut g, cid, target, _, gun, escorts) = island(2);
    // A gun and an escort of the group already on the target's island: they
    // have a dry road, and stand within the muster of our port.
    let theirs: Vec<Pos> = g
        .nbrs(target)
        .into_iter()
        .filter(|pos| landable(&g, *pos) && g.unit_ids_at(*pos).is_empty())
        .take(2)
        .collect();
    let road_gun = g.spawn_unit("catapult", 0, theirs[0]);
    let road_escort = g.spawn_unit("spearman", 0, theirs[1]);
    let mut v1 = armed(&g, cid, gun, escorts);
    v1.force_groups[0].units.extend([road_gun, road_escort]);
    assert!(v1.form_amphibious_train(&g, 0, cid));
    assert!(
        v1.amphibious_train
            .as_ref()
            .unwrap()
            .members
            .contains(&road_gun),
        "version one drafts the nearest units to the port, a dry road or not"
    );
    let mut ai = armed_2(&g, cid, gun, escorts);
    ai.force_groups[0].units.extend([road_gun, road_escort]);
    assert!(ai.form_amphibious_train(&g, 0, cid));
    let train = ai.amphibious_train.clone().unwrap();
    assert!(!train.ford, "a sea lane");
    assert_eq!(
        train.members,
        BTreeSet::from([gun, escorts[0], escorts[1]]),
        "only the units on the port's land, which lack a dry road"
    );
    // Without the column at the port, the train does not form at all.
    for uid in [gun, escorts[0], escorts[1]] {
        g.remove_unit(uid);
    }
    let ai = armed_2(&g, cid, road_gun, [road_escort, road_escort]);
    let reason = ai.amphibious_gate(&g, 0, cid).expect_err("no train");
    assert!(reason.contains("units with a dry road march"), "{reason}");
}

#[test]
fn a_sea_lane_landing_on_our_own_landmass_with_a_dry_road_is_refused() {
    let (mut g, cid, target, port, gun, escorts) = island(2);
    // A land bridge from our land to the target's island, away from the
    // port: the bay beside it still offers a short lane to a landing.
    for d in 3..=6 {
        g.map
            .tiles
            .get_mut(&(target.0, target.1 - d))
            .unwrap()
            .terrain = crate::name!("grassland");
    }
    assert!(
        land_joined(&g, target, None).contains(&port),
        "our port stands on the target's landmass"
    );
    let mut v1 = armed(&g, cid, gun, escorts);
    assert!(
        v1.form_amphibious_train(&g, 0, cid),
        "version one still ships across the bay: {:?}",
        v1.amphibious_gate(&g, 0, cid).err()
    );
    let landing = v1.amphibious_train.as_ref().unwrap().landing;
    assert!(land_joined(&g, port, None).contains(&landing));
    let ai = armed_2(&g, cid, gun, escorts);
    let reason = ai.amphibious_gate(&g, 0, cid).expect_err("refused");
    assert!(reason.contains("own landmass, with a dry road"), "{reason}");
}
