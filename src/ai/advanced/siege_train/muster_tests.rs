//! `stage-musters-out-of-reach`: while a train cannot yet close, its melee and
//! shooters muster out of the defenders' reach instead of waiting on the
//! staging ring. Over the 10-04/05 control runs 59% of the land soldiers we
//! lost died while their siege read Stage, 772 of them on the ring itself.

use super::tests::{plan_against, walled_city};
use super::*;

/// `walled_city` on open grassland, at war, with a hostile Swordsman two
/// tiles out on the west road and our Warrior two tiles beyond it, on the
/// staging ring and inside the Swordsman's reach.
fn ring_under_a_raider() -> (Game, u32, u32, Pos) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let raider_at = (city.0 - 2, city.1);
    let ring = (city.0 - 4, city.1);
    assert_eq!(g.wdist(ring, city), 4, "fixture: the ring tile");
    g.spawn_unit("swordsman", 1, raider_at);
    let warrior = g.spawn_unit("warrior", 0, ring);
    (g, cid, warrior, ring)
}

fn train(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_shared_danger();
    if gene {
        ai.enable_stage_musters_out_of_reach();
    }
    ai
}

fn danger(g: &Game, uid: u32, at: Pos) -> f64 {
    let mut field = super::super::battle_planner::DangerField::with_reach(g, 0, true);
    field.share(g);
    field.rotation_danger(at, uid)
}

#[test]
fn a_ring_member_under_reach_falls_back_only_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid, warrior, ring) = ring_under_a_raider();
        let limit = f64::from(g.units[&warrior].hp) * MUSTER_DANGER_SHARE;
        assert!(
            danger(&g, warrior, ring) > limit,
            "fixture: the ring reads {:.1} against {limit:.1}",
            danger(&g, warrior, ring)
        );
        let city = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        let mut ai = train(gene);
        ai.stage_muster_ready.insert(cid, false);
        ai.siege_stage_step(&mut g, 0, warrior, &city, &plan);
        let at = g.units[&warrior].pos;
        if gene {
            assert_ne!(at, ring, "the member leaves the reach");
            assert!(danger(&g, warrior, at) <= limit, "it stands under the line at {at:?}");
            assert!(g.wdist(at, city.pos) > CITY_STRIKE_RANGE);
        } else {
            assert_eq!(at, ring, "today the member holds the ring");
        }
    }
}

#[test]
fn a_ready_train_keeps_the_ordinary_stage_step() {
    let (mut g, cid, warrior, ring) = ring_under_a_raider();
    let city = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut ai = train(true);
    ai.stage_muster_ready.insert(cid, true);
    ai.siege_stage_step(&mut g, 0, warrior, &city, &plan);
    assert_eq!(g.units[&warrior].pos, ring, "a train that can close holds its ring");
}

/// `staging-gun-reads-the-shared-danger`: two hostile Warriors reach a
/// staging gun's tile beside two of our soldiers. Each Warrior strikes once,
/// at one of the three; the unshared reading charges the gun both blows, the
/// shared one their split, never under the stronger single blow. Under the
/// gene the Stage step reads the shared one and the gun never holds further
/// out than it does today.
#[test]
fn a_staging_gun_reads_the_shared_danger_only_under_the_gene() {
    let mut outcomes = Vec::new();
    for gene in [false, true] {
        let (mut g, cid) = walled_city();
        let city = g.cities[&cid].pos;
        for tile in g.map.tiles.values_mut() {
            if tile.pos != city {
                tile.terrain = crate::name!("grassland");
                tile.feature = None;
                tile.hills = false;
            }
        }
        g.map_script = crate::setup::MapScript::Pangaea;
        g.turn = 30;
        g.at_war.insert((0, 1));
        let start = (city.0 - 8, city.1);
        let gun = g.spawn_unit("catapult", 0, start);
        for pos in g
            .nbrs(start)
            .into_iter()
            .filter(|pos| g.wdist(*pos, city) == 8)
            .take(2)
            .collect::<Vec<_>>()
        {
            g.spawn_unit("warrior", 0, pos);
        }
        g.spawn_unit("warrior", 1, (start.0 + 2, start.1));
        g.spawn_unit("warrior", 1, (start.0 + 2, start.1 - 1));
        let mut field = super::super::battle_planner::DangerField::with_reach(&g, 0, true);
        field.share(&g);
        let unshared = stage_gun_danger(&mut field, start, gun, false);
        let shared = stage_gun_danger(&mut field, start, gun, true);
        assert!(
            shared < unshared && unshared > 0.0,
            "fixture: shared {shared:.1} under unshared {unshared:.1}"
        );
        let view = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        let mut ai = AdvancedAi::new();
        ai.enable_siege_train();
        ai.enable_shared_danger();
        if gene {
            ai.enable_staging_gun_reads_the_shared_danger();
        }
        ai.siege_stage_step(&mut g, 0, gun, &view, &plan);
        outcomes.push(g.wdist(g.units[&gun].pos, city));
    }
    assert!(
        outcomes[1] <= outcomes[0],
        "the shared reading never holds a gun further out: {outcomes:?}"
    );
}

/// The readiness test reads the mustered train as it would stand on the
/// ring. Live King game 168 (civvis-20261005T222327Z) armed the gene with a
/// test that counted the mustered bill but read the breakers and the damage
/// budget within five tiles: with the whole train at the muster line
/// neither saw anyone, `no_breaker` held, and the sieges sat in Stage 172
/// siege-turns against 13 in Invest and Reduce.
fn mustered_train(with_breaker: bool) -> (Game, u32, AdvancedAi, ForceGroup, StrategicPlan) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let spots = super::tests::at_distance(&g, cid, 7);
    let mut units: Vec<u32> = spots
        .iter()
        .take(3)
        .map(|pos| g.spawn_unit("modern_armor", 0, *pos))
        .collect();
    if with_breaker {
        units.push(g.spawn_unit("catapult", 0, spots[3]));
    }
    let group = ForceGroup {
        id: units[0],
        domain: ForceDomain::Land,
        units: units.clone(),
        anchor: g.units[&units[0]].pos,
        objective: city,
        focus_target: None,
        posture: super::super::ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.enable_siege_positive_damage_budget();
    ai.enable_stage_musters_out_of_reach();
    ai.force_groups.push(group.clone());
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Stage,
            taker: None,
            entered: 29,
            assessed: 29,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    (g, cid, ai, group, plan)
}

#[test]
fn a_gathered_train_with_a_breaker_reads_ready_to_close() {
    let (g, cid, mut ai, group, plan) = mustered_train(true);
    let city = CityView::of(&g, cid).unwrap();
    assert!(city.wall_hp > 0, "fixture: walls stand");
    let force = ai.siege_force(&g, 0, &city, &plan, &group);
    assert!(
        force.iter().all(|uid| g.wdist(g.units[uid].pos, city.pos) > STAGING_FAR),
        "fixture: the whole train musters beyond the ring"
    );
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(
        ai.stage_muster_ready.get(&cid).copied(),
        Some(true),
        "a mustered train that meets the bill with a breaker closes"
    );
}

/// A close is a commitment for `MUSTER_COMMIT_TURNS`: while the train
/// still meets the bill with a breaker it keeps closing whatever its budget
/// reads, so a reading that wavers at its line cannot pull the guns back
/// before they reach the ring. A lost bill or a breaker hold ends it at
/// once, and so does the window.
#[test]
fn a_close_holds_through_its_commitment_window() {
    assert!(muster_commitment_holds(Some(30), 34, 5, true, false));
    assert!(!muster_commitment_holds(Some(30), 35, 5, true, false), "the window ends");
    assert!(!muster_commitment_holds(Some(30), 31, 5, false, false), "the bill is lost");
    assert!(!muster_commitment_holds(Some(30), 31, 5, true, true), "a breaker hold");
    assert!(!muster_commitment_holds(None, 31, 5, true, false), "never closed");
}

/// The turn a train closes is recorded once, and a train that stays closed
/// keeps that turn; a train that holds forgets it.
#[test]
fn a_close_records_its_turn_and_a_hold_forgets_it() {
    let (mut g, cid, mut ai, group, plan) = mustered_train(true);
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.stage_muster_ready.get(&cid).copied(), Some(true));
    assert_eq!(ai.stage_muster_closed.get(&cid).copied(), Some(30));
    g.turn = 31;
    ai.sieges.get_mut(&cid).unwrap().assessed = 30;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.stage_muster_ready.get(&cid).copied(), Some(true));
    assert_eq!(
        ai.stage_muster_closed.get(&cid).copied(),
        Some(30),
        "still closed: the commitment counts from the first close"
    );

    let (g, cid, mut ai, group, plan) = mustered_train(false);
    ai.stage_muster_closed.insert(cid, 29);
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(
        ai.stage_muster_ready.get(&cid).copied(),
        Some(false),
        "a breaker hold ends the commitment"
    );
    assert_eq!(ai.stage_muster_closed.get(&cid), None);
}

/// A muster stand lies out of every ranged and city strike, so the train
/// can gather nine or ten tiles out: its bill reads over the same radius as
/// its budget and breakers. Live Emperor game 210 held Pella's train at "0
/// strength within 8 tiles" for ten turns while its budget within 10 read
/// ready and the walls fell to 0.
#[test]
fn a_train_gathered_nine_tiles_out_meets_its_bill() {
    let (mut g, cid, mut ai, group, plan) = mustered_train(true);
    let spots = super::tests::at_distance(&g, cid, MUSTER_FAR + 1);
    for (uid, pos) in group.units.iter().zip(spots) {
        g.units.get_mut(uid).unwrap().pos = pos;
    }
    assert!(
        group
            .units
            .iter()
            .all(|uid| g.wdist(g.units[uid].pos, g.cities[&cid].pos) > MUSTER_FAR),
        "fixture: the whole train stands past the old bill line"
    );
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(
        ai.stage_muster_ready.get(&cid).copied(),
        Some(true),
        "the train nine tiles out meets its bill and closes"
    );
}

/// `invest-keeps-its-cavalry`: the turn's sieges are assessed before the
/// raid planner reads them, so a muster that closes this turn already reads
/// closed. Live Emperor game 228 sent three Horsemen out to raid beside
/// Ngulu Mapu the turn its muster closed. Off, the record stays last turn's.
#[test]
fn the_raids_read_this_turns_siege_under_the_gene() {
    for gene in [false, true] {
        let (g, cid, mut ai, _, plan) = mustered_train(true);
        let before = ai.sieges[&cid].assessed;
        assert!(before < g.turn, "fixture: last turn's record");
        if gene {
            ai.enable_invest_keeps_its_cavalry();
        }
        ai.assess_sieges_before_the_raids(&g, 0, &plan);
        if gene {
            assert_eq!(ai.sieges[&cid].assessed, g.turn);
            assert_eq!(ai.stage_muster_ready.get(&cid).copied(), Some(true));
        } else {
            assert_eq!(ai.sieges[&cid].assessed, before);
        }
    }
}

/// `weak-target-skips-the-muster`: a gunless train holds at the muster
/// line, but under the gene, against a target whose military is under
/// `MUSTER_WEAK_TARGET_SHARE` of ours, it closes. Live Emperor game 330 held
/// before Cartagena 12-16 tiles out with Spain at 105 power against 1,419.
#[test]
fn a_train_against_a_weak_target_skips_the_muster_under_the_gene() {
    let (g, cid, mut ai, group, plan) = mustered_train(false);
    let owner = g.cities[&cid].owner;
    assert!(
        g.military_power(owner) <= MUSTER_WEAK_TARGET_SHARE * g.military_power(0),
        "fixture: the target is weak ({} vs {})",
        g.military_power(owner),
        g.military_power(0)
    );
    let mut on = ai.clone();
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.stage_muster_ready.get(&cid).copied(), Some(false), "off: holds");
    on.enable_weak_target_skips_the_muster();
    on.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(on.stage_muster_ready.get(&cid).copied(), Some(true), "on: closes");
}

#[test]
fn a_gathered_train_without_a_breaker_holds_however_long_it_waits() {
    let (mut g, cid, mut ai, group, plan) = mustered_train(false);
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(
        ai.stage_muster_ready.get(&cid).copied(),
        Some(false),
        "nothing opens the walls: the train holds at the muster line"
    );
    g.turn = 29 + g.standard_duration(16) + 1;
    ai.sieges.get_mut(&cid).unwrap().assessed = g.turn - 1;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(
        ai.stage_muster_ready.get(&cid).copied(),
        Some(false),
        "a gunless train never closes on walls it cannot open"
    );
}

/// A hostile Archer reaches the muster member's tile: its shot is under
/// half the member's health, but a waiting body takes it every turn.
#[test]
fn a_muster_member_in_an_archer_s_reach_falls_back() {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let here = (city.0 - 7, city.1);
    // A Horseman: its four moves reach a stand beyond the Archer's reach.
    let warrior = g.spawn_unit("horseman", 0, here);
    g.spawn_unit("archer", 1, (city.0 - 5, city.1));
    let limit = f64::from(g.units[&warrior].hp) * MUSTER_DANGER_SHARE;
    let reading = danger(&g, warrior, here);
    assert!(
        reading > 0.0 && reading <= limit,
        "fixture: the Archer reaches the member under the line ({reading:.1} vs {limit:.1})"
    );
    let view = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut ai = train(true);
    ai.stage_muster_ready.insert(cid, false);
    ai.siege_stage_step(&mut g, 0, warrior, &view, &plan);
    let at = g.units[&warrior].pos;
    assert_ne!(at, here, "the member moves");
    assert_eq!(
        danger(&g, warrior, at),
        0.0,
        "the member stands out of the Archer's reach: {here:?} -> {at:?}"
    );
}

/// `muster-walks-the-road`: `walled_city` on open grassland at war, with a
/// bay of coast on every tile five to seven out from the city on its own row
/// and the rows north of it, and our Warrior eight tiles west. Every tile in
/// its reach nearer the city by straight distance is water; the dry road runs
/// south round the bay. Live Emperor civvis-20261008T192219Z (game 433) held
/// Taiyuan's train 22-25 tiles out that way from turn 183.
fn behind_a_bay() -> (Game, u32, u32, Pos) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let bay: Vec<Pos> = g
        .map
        .tiles
        .keys()
        .copied()
        .filter(|pos| (5..=7).contains(&g.wdist(*pos, city)) && pos.1 <= city.1 + 2)
        .collect();
    for pos in bay {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    }
    let start = (city.0 - 8, city.1);
    assert_eq!(g.wdist(start, city), 8, "fixture: the start");
    let warrior = g.spawn_unit("warrior", 0, start);
    (g, cid, warrior, start)
}

#[test]
fn a_member_behind_a_bay_walks_the_road_only_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid, warrior, start) = behind_a_bay();
        let city = g.cities[&cid].pos;
        assert!(
            g.reachable(warrior).into_iter().all(|pos| {
                g.wdist(pos, city) >= g.wdist(start, city)
                    || g.map.get(pos).is_some_and(|tile| g.rules.is_water(tile))
            }),
            "fixture: no dry tile in reach is nearer by straight distance"
        );
        let road = g.dry_road_steps(warrior, city, STAGING_FAR, MUSTER_ROAD_STEPS);
        let from = *road
            .get(&start)
            .expect("fixture: a dry road leads round the bay");
        let view = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        let mut ai = train(true);
        if gene {
            ai.enable_muster_walks_the_road();
        }
        ai.stage_muster_ready.insert(cid, false);
        ai.siege_stage_step(&mut g, 0, warrior, &view, &plan);
        let at = g.units[&warrior].pos;
        if gene {
            let now = *road.get(&at).expect("the member stands on the road map");
            assert!(
                now < from,
                "the member walks the road: {from} -> {now} steps at {at:?}"
            );
            assert!(g.map.get(at).is_some_and(|tile| !g.rules.is_water(tile)));
            assert!(g.wdist(at, city) > CITY_STRIKE_RANGE);
        } else {
            assert_eq!(at, start, "today the member holds at the muster line");
        }
    }
}

/// The road map leaves out water and reads no tile farther than its limit.
#[test]
fn the_dry_road_map_keeps_to_land_within_its_limit() {
    let (g, cid, warrior, start) = behind_a_bay();
    let city = g.cities[&cid].pos;
    let road = g.dry_road_steps(warrior, city, STAGING_FAR, MUSTER_ROAD_STEPS);
    assert!(road
        .keys()
        .all(|pos| g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))));
    assert!(road.get(&start).copied().unwrap() > (g.wdist(start, city) - STAGING_FAR) as usize);
    let short = g.dry_road_steps(warrior, city, STAGING_FAR, 2);
    assert!(short.values().all(|steps| *steps <= 2));
    assert!(
        !short.contains_key(&start),
        "the start lies past a two-step road"
    );
}
