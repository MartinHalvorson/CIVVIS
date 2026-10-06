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
