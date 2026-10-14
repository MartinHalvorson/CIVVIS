use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;

/// Seat 0 with three idle cities and the Diplomatic Service and Nationalism
/// civics (Spy capacity 2); seat 1 a met rival with one city, a Spaceport we
/// have seen, and the Moon landed.
fn board() -> (Game, Vec<u32>) {
    let mut g = Game::new_full(2, 32, 22, 79_208, 250, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = g.units[&settler].pos;
        g.found_city_for(pid, position, None);
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    for _ in 0..2 {
        let site = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                let tile = &g.map.tiles[pos];
                tile.owner_city.is_none()
                    && !g.rules.is_water(tile)
                    && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 5)
            })
            .min_by_key(|pos| (g.wdist(home, *pos), *pos))
            .expect("the fixture map has room for another city");
        g.found_city_for(0, site, None);
    }
    for civic in ["diplomatic_service", "nationalism"] {
        g.players[0].civics.insert(crate::name::Name::new(civic));
    }
    let ours = g.player_city_ids(0);
    for (i, cid) in ours.iter().enumerate() {
        let city = g.cities.get_mut(cid).unwrap();
        city.queue.clear();
        city.pop = 2 + 3 * i as i32;
    }
    g.current = 0;
    g.turn = 180;
    g.at_war.clear();
    g.record_contact(0, 1);
    for project in ["launch_earth_satellite", "launch_moon_landing"] {
        g.players[1].science_projects.insert(project.to_string());
    }
    let rival = g.player_city_ids(1)[0];
    let centre = g.cities[&rival].pos;
    let pad = g
        .wdisk(centre, 1)
        .into_iter()
        .find(|pos| {
            *pos != centre
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.district.is_none() && !g.rules.is_water(tile))
        })
        .expect("the rival city has a free land neighbour");
    let tile = g.map.tiles.get_mut(&pad).unwrap();
    tile.owner_city = Some(rival);
    tile.district = Some(crate::name!("spaceport"));
    tile.improvement = None;
    g.cities
        .get_mut(&rival)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    g.players[0].explored.insert(pad);
    (g, ours)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_science_denial_trains_spies();
    ai
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn training(g: &Game) -> BTreeSet<u32> {
    let spy = Item::Unit {
        unit: crate::name!("spy"),
    };
    g.player_city_ids(0)
        .into_iter()
        .filter(|cid| g.cities[cid].queue.first() == Some(&spy))
        .collect()
}

/// The `n` highest-Production cities of seat 0 among `among`.
fn top(g: &Game, among: &[u32], n: usize) -> BTreeSet<u32> {
    let mut ranked: Vec<(f64, u32)> = among
        .iter()
        .map(|cid| (g.city_yields(*cid).production, *cid))
        .collect();
    ranked.sort_by(|(l, lc), (r, rc)| r.total_cmp(l).then(lc.cmp(rc)));
    ranked.into_iter().take(n).map(|(_, cid)| cid).collect()
}

#[test]
fn trains_spies_is_opt_in() {
    opt_in_off_in_both_controllers("science-denial-trains-spies", |ai| {
        ai.science_denial_trains_spies
    });
}

#[test]
fn the_best_city_trains_one_spy_at_a_time_up_to_the_pads_plus_one() {
    let (g, ours) = board();
    assert_eq!(g.spy_capacity(0), 2, "fixture capacity");
    let mut off = g.clone();
    AdvancedAi::targeting(VictoryTarget::Domination).claim_queues_for_denial_spies(
        &mut off,
        0,
        &plan(),
    );
    assert!(training(&off).is_empty(), "off: no claim");
    let mut on = g.clone();
    armed().claim_queues_for_denial_spies(&mut on, 0, &plan());
    // One pad plus one is two, but the host trains one Spy at a time.
    assert_eq!(training(&on), top(&g, &ours, 1));
    armed().claim_queues_for_denial_spies(&mut on, 0, &plan());
    assert_eq!(training(&on).len(), 1, "a Spy in training holds the claim");
    // It trains: the queue empties and an agent is held; the second follows.
    let best = *training(&on).iter().next().unwrap();
    on.cities.get_mut(&best).unwrap().queue.clear();
    let id = on.next_id;
    on.next_id += 1;
    on.spies.insert(
        id,
        crate::game::Spy {
            id,
            owner: 0,
            level: 0,
            promotions: BTreeSet::new(),
            city: Some(best),
            ready_turn: on.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    armed().claim_queues_for_denial_spies(&mut on, 0, &plan());
    assert_eq!(training(&on), top(&g, &ours, 1), "the best city again");
    // Two held: the need is met.
    on.cities.get_mut(&best).unwrap().queue.clear();
    let id = on.next_id;
    on.next_id += 1;
    let mut second = on.spies[&(id - 1)].clone();
    second.id = id;
    on.spies.insert(id, second);
    armed().claim_queues_for_denial_spies(&mut on, 0, &plan());
    assert!(training(&on).is_empty(), "two held meet the need");
}

#[test]
fn no_racer_no_claim() {
    let (mut g, _) = board();
    g.players[1].science_projects.remove("launch_moon_landing");
    armed().claim_queues_for_denial_spies(&mut g, 0, &plan());
    assert!(
        training(&g).is_empty(),
        "one project landed is not the decisive race"
    );
}

#[test]
fn walls_settlers_and_threatened_cities_keep_their_queues() {
    let (mut g, ours) = board();
    let ranked = top(&g, &ours, 3);
    let mut by_rank: Vec<u32> = ranked.iter().copied().collect();
    by_rank.sort_by(|l, r| {
        g.city_yields(*r)
            .production
            .total_cmp(&g.city_yields(*l).production)
            .then(l.cmp(r))
    });
    let (first, second, third) = (by_rank[0], by_rank[1], by_rank[2]);
    // The best city is building Walls; the second is under attack.
    g.cities.get_mut(&first).unwrap().queue = vec![Item::Building {
        building: crate::name!("walls"),
    }];
    g.cities.get_mut(&second).unwrap().last_attacked = g.turn;
    armed().claim_queues_for_denial_spies(&mut g, 0, &plan());
    assert_eq!(training(&g), BTreeSet::from([third]));
    assert_eq!(
        g.cities[&first].queue.first(),
        Some(&Item::Building {
            building: crate::name!("walls")
        }),
        "the Walls stay"
    );
}

#[test]
fn a_routine_build_is_displaced_and_a_held_spy_counts() {
    let (mut g, ours) = board();
    let best = *top(&g, &ours, 1).iter().next().unwrap();
    g.cities.get_mut(&best).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("builder"),
    }];
    // One Spy already held: one more is needed.
    let id = g.next_id;
    g.next_id += 1;
    g.spies.insert(
        id,
        crate::game::Spy {
            id,
            owner: 0,
            level: 0,
            promotions: BTreeSet::new(),
            city: Some(ours[0]),
            ready_turn: g.turn,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        },
    );
    armed().claim_queues_for_denial_spies(&mut g, 0, &plan());
    assert_eq!(
        training(&g),
        BTreeSet::from([best]),
        "the Builder is displaced"
    );
}

/// The board with the rival only holding Rocketry: no space project landed
/// and no Spaceport standing.
fn rocketry_only() -> (Game, Vec<u32>) {
    let (mut g, ours) = board();
    g.players[1].science_projects.clear();
    let rival = g.player_city_ids(1)[0];
    let pads = g
        .cities
        .get_mut(&rival)
        .unwrap()
        .districts
        .remove(&crate::name!("spaceport"))
        .unwrap_or_default();
    for pad in pads {
        g.map.tiles.get_mut(&pad).unwrap().district = None;
    }
    g.players[1].techs.insert(crate::name!("rocketry"));
    (g, ours)
}

#[test]
fn spies_from_rocketry_is_opt_in() {
    opt_in_off_in_both_controllers("space-denial-spies-from-rocketry", |ai| {
        ai.space_denial_spies_from_rocketry
    });
}

/// `space-denial-spies-from-rocketry`: a rival that only holds Rocketry is
/// no decisive racer (the launcher war still waits), but the spy network
/// already trains against it.
#[test]
fn a_rocketry_rival_starts_the_spy_network_but_not_the_war() {
    let (g, ours) = rocketry_only();
    let stock = armed();
    assert!(
        stock.decisive_space_racers(&g, 0).is_empty(),
        "fixture: not decisive"
    );
    let mut off = g.clone();
    stock.claim_queues_for_denial_spies(&mut off, 0, &plan());
    assert!(
        training(&off).is_empty(),
        "off: Rocketry alone trains nothing"
    );

    let mut early = armed();
    early.enable_space_denial_spies_from_rocketry();
    assert!(
        early.decisive_space_racers(&g, 0).is_empty(),
        "the war's racers are unchanged"
    );
    assert_eq!(early.denial_spy_racers(&g, 0), BTreeSet::from([1]));
    let mut on = g.clone();
    early.claim_queues_for_denial_spies(&mut on, 0, &plan());
    assert_eq!(training(&on), top(&g, &ours, 1), "one Spy in the best city");
}

/// Under the gene the network trains to capacity even with no pad explored:
/// stock needs the explored pads plus one (here one), the gene needs two.
#[test]
fn spies_from_rocketry_trains_to_capacity() {
    let (g, _ours) = rocketry_only();
    let racers = BTreeSet::from([1]);
    assert_eq!(g.spy_capacity(0), 2, "fixture capacity");
    assert_eq!(
        armed().denial_spies_wanted(&g, 0, &racers),
        1,
        "stock: no pad seen, one"
    );
    let mut early = armed();
    early.enable_space_denial_spies_from_rocketry();
    assert_eq!(
        early.denial_spies_wanted(&g, 0, &racers),
        2,
        "gene: our capacity"
    );
}
