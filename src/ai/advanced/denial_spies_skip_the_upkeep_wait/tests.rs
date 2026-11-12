use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::upkeep_waits_for_the_income::UPKEEP_FROM_STANDARD_TURN;
use crate::ai::advanced::VictoryTarget;

/// Seat 0 with three idle cities, Spy capacity 2 (Diplomatic Service and
/// Nationalism), 100 Gold at -10 a turn, past the upkeep gate's first turn;
/// seat 1 a met rival with one city, a Spaceport we have seen and the Moon
/// landed: the shape of live 081824Z at turn 180.
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
    g.turn = g.standard_duration(UPKEEP_FROM_STANDARD_TURN) + 5;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.players[0].gold = 100.0;
    g.players[0].gold_per_turn = -10.0;
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

fn spy() -> Item {
    Item::Unit {
        unit: crate::name!("spy"),
    }
}

/// The live arming the gene joins: the Domination seat's upkeep gate and the
/// denial's Spy claim.
fn stock() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_upkeep_waits_for_the_income();
    ai.enable_science_denial_trains_spies();
    ai
}

fn skips() -> AdvancedAi {
    let mut ai = stock();
    ai.enable_denial_spies_skip_the_upkeep_wait();
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

fn waits(ai: &AdvancedAi, g: &Game, cid: u32, item: &Item) -> bool {
    ai.upkeep_waits_for(g, 0, cid, item, false, &plan())
}

#[test]
fn denial_spies_skip_the_upkeep_wait_is_opt_in() {
    opt_in_off_in_both_controllers("denial-spies-skip-the-upkeep-wait", |ai| {
        ai.denial_spies_skip_the_upkeep_wait
    });
}

#[test]
fn a_wanted_spy_against_a_racer_past_its_moon_skips_the_wait() {
    let (g, ours) = board();
    assert!(
        waits(&stock(), &g, ours[0], &spy()),
        "stock: -14 projected against 100 Gold, so the Spy waits (081824Z)"
    );
    assert!(!waits(&skips(), &g, ours[0], &spy()), "the gene: it trains");
    assert_eq!(
        skips().denial_spy_upkeep_racer(&g, 0, ours[0], &spy()),
        Some(1),
        "the racer it answers"
    );
}

#[test]
fn other_upkeep_still_waits() {
    let (g, ours) = board();
    let tank = Item::Unit {
        unit: crate::name!("tank"),
    };
    assert!(waits(&skips(), &g, ours[0], &tank));
    assert_eq!(skips().denial_spy_upkeep_racer(&g, 0, ours[0], &tank), None);
}

#[test]
fn the_waiver_needs_a_racer_past_its_moon_short_of_the_launch() {
    let (mut g, ours) = board();
    g.players[1]
        .science_projects
        .remove("launch_moon_landing");
    assert!(
        waits(&skips(), &g, ours[0], &spy()),
        "the Earth Satellite alone: no decisive racer"
    );
    let (mut g, ours) = board();
    for project in ["launch_mars_colony", "exoplanet_expedition"] {
        g.players[1].science_projects.insert(project.to_string());
    }
    assert!(
        waits(&skips(), &g, ours[0], &spy()),
        "after the launch no Disrupt Rocketry stops it"
    );
    g.players[1]
        .science_projects
        .remove("exoplanet_expedition");
    assert!(!waits(&skips(), &g, ours[0], &spy()), "Mars, short of the launch");
}

#[test]
fn the_treasury_must_carry_the_spys_own_upkeep() {
    let (mut g, ours) = board();
    let upkeep = AdvancedAi::item_added_upkeep(&g, &spy());
    assert!(upkeep > 0.0, "the fixture Spy costs upkeep");
    g.players[0].gold = UPKEEP_RUNWAY_TURNS * upkeep - 1.0;
    assert!(waits(&skips(), &g, ours[0], &spy()), "090241Z's empty treasury");
    g.players[0].gold = UPKEEP_RUNWAY_TURNS * upkeep;
    assert!(!waits(&skips(), &g, ours[0], &spy()));
}

#[test]
fn a_spy_the_denial_no_longer_wants_waits() {
    let (mut g, ours) = board();
    // A Spy at the head of another city's queue fills one of the two.
    g.cities.get_mut(&ours[1]).unwrap().queue.push(spy());
    assert!(!waits(&skips(), &g, ours[0], &spy()), "one of two: still wanted");
    g.cities.get_mut(&ours[2]).unwrap().queue.push(spy());
    assert!(
        waits(&skips(), &g, ours[0], &spy()),
        "two in training against two needed"
    );
    let mut off = skips();
    off.disable_science_denial_trains_spies();
    let (g, ours) = board();
    assert!(waits(&off, &g, ours[0], &spy()), "no denial claim: no waiver");
}

#[test]
fn the_claim_keeps_its_spy_and_names_the_racer() {
    let headline = "keeps its Spy against";
    let run = |mut ai: AdvancedAi| {
        let (mut g, _) = board();
        ai.base.journal = crate::reasoning::Journal::recording();
        ai.claim_queues_for_denial_spies(&mut g, 0, &plan());
        let queued: Vec<u32> = g
            .player_city_ids(0)
            .into_iter()
            .filter(|cid| g.cities[cid].queue.first() == Some(&spy()))
            .collect();
        assert_eq!(queued.len(), 1, "the claim queues one Spy");
        let preempts = ai.upkeep_wait_preempts(&g, 0, queued[0], &spy(), &plan());
        let lines = ai
            .base
            .journal
            .since(0)
            .thoughts
            .iter()
            .filter(|thought| thought.headline.contains(headline))
            .map(|thought| thought.detail.clone())
            .collect::<Vec<_>>();
        (preempts, lines)
    };
    let (preempts, lines) = run(stock());
    assert!(preempts, "stock: the governor sets the Spy aside");
    assert!(lines.is_empty());
    let (preempts, lines) = run(skips());
    assert!(!preempts, "the gene: the Spy keeps its queue");
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("denial-spies-skip-the-upkeep-wait: "),
        "{}",
        lines[0]
    );
    assert!(lines[0].contains("landed 2 space projects"), "{}", lines[0]);
}
