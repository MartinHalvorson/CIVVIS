use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;

/// Seat 0 with three idle cities and the Diplomatic Service and Nationalism
/// civics (Spy capacity 2); seat 1 a met rival with one city. With `racing`
/// the rival has landed the Satellite and the Moon from a Spaceport we have
/// seen (a decisive racer); without, it has neither Rocketry nor a project.
fn board(racing: bool) -> (Game, Vec<u32>) {
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
    g.turn = 120;
    g.at_war.clear();
    g.record_contact(0, 1);
    if racing {
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
    }
    (g, ours)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_science_denial_trains_spies();
    if gene {
        ai.enable_denial_spies_fill_every_slot();
    }
    ai
}

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn spy() -> Item {
    Item::Unit {
        unit: crate::name!("spy"),
    }
}

fn training(g: &Game) -> BTreeSet<u32> {
    g.player_city_ids(0)
        .into_iter()
        .filter(|cid| g.cities[cid].queue.first() == Some(&spy()))
        .collect()
}

/// Seat 0's cities by Production, best first (the lower id on a tie).
fn by_production(g: &Game, among: &[u32]) -> Vec<u32> {
    let mut ranked: Vec<(f64, u32)> = among
        .iter()
        .map(|cid| (g.city_yields(*cid).production, *cid))
        .collect();
    ranked.sort_by(|(l, lc), (r, rc)| r.total_cmp(l).then(lc.cmp(rc)));
    ranked.into_iter().map(|(_, cid)| cid).collect()
}

/// A host Spy menu: the Spy offered, at `turns`, in each of `cities`, and an
/// empty menu (no Spy) in every other city of seat 0.
fn host_menus(g: &mut Game, offers: &[(u32, f64)]) {
    let key = Game::production_block_key(&spy());
    let mut menus = BTreeMap::new();
    for cid in g.player_city_ids(0) {
        let mut menu = BTreeMap::new();
        if let Some((_, turns)) = offers.iter().find(|(city, _)| *city == cid) {
            menu.insert(
                key.clone(),
                crate::game::HostMenuEntry {
                    cost: None,
                    turns: Some(*turns),
                },
            );
        }
        menus.insert(cid, menu);
    }
    g.host_buildable = std::sync::Arc::new(menus);
}

#[test]
fn fill_every_slot_is_opt_in() {
    opt_in_off_in_both_controllers("denial-spies-fill-every-slot", |ai| {
        ai.denial_spies_fill_every_slot
    });
}

#[test]
fn the_slots_fill_before_any_rival_takes_rocketry() {
    let (g, ours) = board(false);
    assert_eq!(g.spy_capacity(0), 2, "fixture capacity");
    assert!(
        seat(VictoryTarget::Culture, false)
            .denial_spy_racers(&g, 0)
            .is_empty(),
        "no rival on the denial board"
    );
    let mut off = g.clone();
    seat(VictoryTarget::Culture, false).claim_queues_for_denial_spies(
        &mut off,
        0,
        &plan(GrandStrategy::Culture),
    );
    assert!(training(&off).is_empty(), "off: no racer, no Spy");
    let mut on = g.clone();
    let ai = seat(VictoryTarget::Culture, true);
    assert_eq!(
        ai.denial_training_racers(&g, 0),
        BTreeSet::from([1]),
        "the met rival stands in"
    );
    ai.claim_queues_for_denial_spies(&mut on, 0, &plan(GrandStrategy::Culture));
    // A native board has no host menu: the whole shortfall at once.
    let ranked = by_production(&g, &ours);
    assert_eq!(
        training(&on),
        ranked.iter().take(2).copied().collect::<BTreeSet<u32>>(),
        "both slots, the two best cities"
    );
}

#[test]
fn no_slot_no_stand_in() {
    let (mut g, _) = board(false);
    g.players[0].civics.clear();
    assert_eq!(g.spy_capacity(0), 0);
    let ai = seat(VictoryTarget::Culture, true);
    assert!(ai.denial_training_racers(&g, 0).is_empty());
    ai.claim_queues_for_denial_spies(&mut g, 0, &plan(GrandStrategy::Culture));
    assert!(training(&g).is_empty());
}

#[test]
fn a_spy_in_training_no_longer_holds_the_claim() {
    let (g, ours) = board(true);
    let ranked = by_production(&g, &ours);
    // Off: one at a time, and the one in training holds the claim.
    let mut off = g.clone();
    let old = seat(VictoryTarget::Domination, false);
    old.claim_queues_for_denial_spies(&mut off, 0, &plan(GrandStrategy::Conquest));
    assert_eq!(training(&off), BTreeSet::from([ranked[0]]));
    old.claim_queues_for_denial_spies(&mut off, 0, &plan(GrandStrategy::Conquest));
    assert_eq!(training(&off).len(), 1, "off: the queued Spy waits");
    // On: the second slot trains beside the first.
    let mut on = g.clone();
    on.cities.get_mut(&ranked[0]).unwrap().queue = vec![spy()];
    seat(VictoryTarget::Domination, true).claim_queues_for_denial_spies(
        &mut on,
        0,
        &plan(GrandStrategy::Conquest),
    );
    assert_eq!(
        training(&on),
        BTreeSet::from([ranked[0], ranked[1]]),
        "on: the next best city takes the second slot"
    );
    // Both slots in training: nothing more.
    seat(VictoryTarget::Domination, true).claim_queues_for_denial_spies(
        &mut on,
        0,
        &plan(GrandStrategy::Conquest),
    );
    assert_eq!(training(&on).len(), 2, "capacity reached");
}

#[test]
fn the_host_menu_gates_the_claim_one_per_pass() {
    let (mut g, ours) = board(true);
    let ranked = by_production(&g, &ours);
    // The host offers a Spy in the two weaker cities only: the best city's
    // menu says the slots are full there (as the host lists it nowhere once
    // every slot is taken, in training, in transit or held by a captor).
    host_menus(&mut g, &[(ranked[1], 6.0), (ranked[2], 4.0)]);
    let ai = seat(VictoryTarget::Culture, true);
    ai.claim_queues_for_denial_spies(&mut g, 0, &plan(GrandStrategy::Culture));
    assert_eq!(
        training(&g),
        BTreeSet::from([ranked[2]]),
        "one per pass, where the host quotes the soonest Spy"
    );
    // The host lists the Spy nowhere: no claim, however few are held.
    let (mut full, _) = board(true);
    host_menus(&mut full, &[]);
    ai.claim_queues_for_denial_spies(&mut full, 0, &plan(GrandStrategy::Culture));
    assert!(training(&full).is_empty(), "no host room, no order");
}

#[test]
fn a_science_seat_keeps_the_old_claim() {
    let (g, ours) = board(true);
    let ranked = by_production(&g, &ours);
    let mut on = g.clone();
    on.cities.get_mut(&ranked[0]).unwrap().queue = vec![spy()];
    let ai = seat(VictoryTarget::Science, true);
    assert!(!ai.denial_spies_fill_every_slot_on(&on));
    ai.claim_queues_for_denial_spies(&mut on, 0, &plan(GrandStrategy::Science));
    assert_eq!(
        training(&on),
        BTreeSet::from([ranked[0]]),
        "a Science seat still waits"
    );
    let (bare, _) = board(false);
    assert!(
        ai.denial_training_racers(&bare, 0).is_empty(),
        "no stand-in racer off the Culture and Domination aims"
    );
}

#[test]
fn the_governor_keeps_a_spy_the_gene_trains() {
    let (mut g, ours) = board(false);
    let ranked = by_production(&g, &ours);
    g.cities.get_mut(&ranked[0]).unwrap().queue = vec![spy()];
    let culture = plan(GrandStrategy::Culture);
    assert!(
        !seat(VictoryTarget::Culture, false).denial_spy_queue_committed(
            &g,
            0,
            ranked[0],
            &culture,
            &spy()
        ),
        "off: no racer, nothing reserved"
    );
    assert!(
        seat(VictoryTarget::Culture, true).denial_spy_queue_committed(
            &g,
            0,
            ranked[0],
            &culture,
            &spy()
        ),
        "on: the stand-in racer reserves the queue"
    );
}
