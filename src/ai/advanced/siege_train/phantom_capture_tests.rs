//! `phantom-capture-keeps-the-siege`: see `advanced/phantom_capture.rs`.
use super::super::ForcePosture;
use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

fn group(g: &Game, uid: u32, cid: u32) -> ForceGroup {
    ForceGroup {
        id: uid,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: g.units[&uid].pos,
        objective: g.cities[&cid].pos,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    }
}

/// A warrior on the ring of a wall-less enemy city whose siege reads Take
/// with the warrior as its taker, assessed this turn. `host_health` is what
/// the host reported when the frame opened; the board holds the city at one,
/// as if this frame's own volley had landed.
fn taking(gene: bool, host_health: i32) -> (Game, AdvancedAi, u32, u32) {
    let (mut g, cid) = walled_city();
    let ring = ring_of(&g, cid)
        .into_iter()
        .find(|p| g.city_at(*p).is_none() && !g.rules.is_water(g.map.get(*p).unwrap()))
        .unwrap();
    let warrior = g.spawn_unit("warrior", 0, ring);
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.wall_hp = 0;
        city.hp = host_health;
    }
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    if gene {
        ai.enable_phantom_capture_keeps_the_siege();
    }
    // The frame opens on the host's board ...
    ai.observe_frame_cities(&g);
    // ... and the board's own strike drops the city to one.
    g.cities.get_mut(&cid).unwrap().hp = 1;
    ai.force_groups = vec![group(&g, warrior, cid)];
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Take,
            taker: Some(warrior),
            entered: g.turn,
            assessed: g.turn,
            posts: Default::default(),
            short_since: None,
        },
    );
    ai.reserved_units.insert(warrior);
    (g, ai, cid, warrior)
}

#[test]
fn a_capture_on_this_frames_own_damage_commits_no_siege_state() {
    let (mut g, mut ai, cid, warrior) = taking(true, 60);
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(g.cities[&cid].owner, 0, "the board still plans the capture");
    let siege = &ai.sieges[&cid];
    assert_eq!(
        siege.stage,
        SiegeStage::Take,
        "no Hold before the host takes it"
    );
    assert_eq!(siege.taker, Some(warrior), "the taker stays the siege's");
    assert!(
        ai.reserved_units.contains(&warrior),
        "the taker stays reserved"
    );
    assert_eq!(
        ai.census.siege_captures, 0,
        "no capture is counted on the board"
    );
    assert!(
        ai.capture_is_provisional(cid),
        "its keep/raze line waits for the host"
    );
}

#[test]
fn without_the_gene_the_same_capture_commits_as_before() {
    let (mut g, mut ai, cid, warrior) = taking(false, 60);
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Hold);
    assert_eq!(ai.sieges[&cid].taker, None);
    assert!(!ai.reserved_units.contains(&warrior));
    assert_eq!(ai.census.siege_captures, 1);
    assert!(!ai.capture_is_provisional(cid));
}

#[test]
fn a_capture_the_host_already_allows_commits_and_remembers_its_siege() {
    // The host itself reported the city at one: the blow stands alone.
    let (mut g, mut ai, cid, warrior) = taking(true, 1);
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Hold);
    assert_eq!(ai.census.siege_captures, 1);
    assert_eq!(
        ai.phantom_held_from(cid),
        Some((SiegeStage::Take, Some(warrior)))
    );
    assert!(!ai.capture_is_provisional(cid));
}

#[test]
fn the_same_frames_assessment_leaves_a_board_only_capture_alone() {
    let (mut g, mut ai, cid, warrior) = taking(true, 60);
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(g.cities[&cid].owner, 0);
    // A later pass of the same frame re-reads the siege on the board that
    // now shows the city as ours.
    ai.sieges.get_mut(&cid).unwrap().assessed = g.turn - 1;
    let force = group(&g, warrior, cid);
    ai.assess_siege(&g, 0, cid, &plan, &force);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Take);
    assert_eq!(ai.census.siege_captures, 0);
}

#[test]
fn the_next_host_board_settles_a_real_capture() {
    let (mut g, mut ai, cid, warrior) = taking(true, 60);
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    // The next frame's host board reports the city as ours.
    ai.observe_frame_cities(&g);
    g.turn += 1;
    let force = group(&g, warrior, cid);
    ai.assess_siege(&g, 0, cid, &plan, &force);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Hold);
    assert_eq!(
        ai.census.siege_captures, 1,
        "counted once, when the host confirms"
    );
}

/// A capture committed on the board (the host reported the city at its
/// blow) that the host never executed: Hold over an enemy city at one
/// health behind fallen walls, the force short of its abort share.
fn phantom_hold(gene: bool) -> (Game, AdvancedAi, u32, u32) {
    let (mut g, mut ai, cid, warrior) = taking(gene, 1);
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Hold);
    // The host's next board: the warrior never left its ring tile and the
    // city is still the enemy's at one health behind fallen walls.
    let (mut host, _) = walled_city();
    let back = ring_of(&host, cid)
        .into_iter()
        .find(|p| host.city_at(*p).is_none() && !host.rules.is_water(host.map.get(*p).unwrap()))
        .unwrap();
    let warrior = host.spawn_unit("warrior", 0, back);
    {
        let city = host.cities.get_mut(&cid).unwrap();
        city.wall_hp = 0;
        city.hp = 1;
    }
    // Two defenders stand off the ring: the bill outgrows one warrior.
    let posts: Vec<Pos> = at_distance(&host, cid, 3)
        .into_iter()
        .filter(|p| host.city_at(*p).is_none() && !host.rules.is_water(host.map.get(*p).unwrap()))
        .take(2)
        .collect();
    for pos in posts {
        host.spawn_unit("musketman", 1, pos);
    }
    host.map_script = crate::setup::MapScript::Pangaea;
    host.at_war.insert((0, 1));
    ai.force_groups = vec![group(&host, warrior, cid)];
    ai.observe_frame_cities(&host);
    (host, ai, cid, warrior)
}

#[test]
fn a_phantom_hold_over_a_falling_city_resumes_its_siege() {
    let (host, mut ai, cid, warrior) = phantom_hold(true);
    let plan = plan_against(&host, cid);
    let force = group(&host, warrior, cid);
    let city = CityView::of(&host, cid).unwrap();
    assert!(unit_power(&host, warrior) < ABORT_SHARE * siege_bill(&host, 0, &city));
    ai.assess_siege(&host, 0, cid, &plan, &force);
    let siege = &ai.sieges[&cid];
    assert_ne!(
        siege.stage,
        SiegeStage::Stage,
        "a falling city is finished, not regrouped for"
    );
    assert_ne!(siege.stage, SiegeStage::Hold);
    assert_eq!(siege.taker, Some(warrior), "a taker is named for the city");
    assert_eq!(
        ai.phantom_held_from(cid),
        None,
        "the remembered stage is spent"
    );
}

#[test]
fn without_the_gene_a_phantom_hold_over_a_weak_force_regroups() {
    let (host, mut ai, cid, warrior) = phantom_hold(false);
    let plan = plan_against(&host, cid);
    let force = group(&host, warrior, cid);
    ai.assess_siege(&host, 0, cid, &plan, &force);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage);
}

#[test]
fn the_resumed_siege_sends_a_real_taker_into_the_falling_city() {
    let (mut host, mut ai, cid, warrior) = phantom_hold(true);
    let plan = plan_against(&host, cid);
    let start = host.log.len();
    let _ = ai.siege_doctrine_step(&mut host, 0, warrior, &plan);
    let target = host.cities[&cid].pos;
    let ordered = host.log.since(start).any(|(seat, action)| {
        *seat == 0
            && (matches!(action, Action::Attack { unit, target: at } if *unit == warrior && *at == target)
                || matches!(action, Action::Move { unit, to } if *unit == warrior && *to == target))
    });
    assert!(ordered, "the taker is ordered onto the city tile");
    assert_eq!(host.cities[&cid].owner, 0);
}

#[test]
fn a_phantom_hold_over_a_healthy_city_still_answers_the_abort_gate() {
    let (mut host, mut ai, cid, warrior) = phantom_hold(true);
    {
        let city = host.cities.get_mut(&cid).unwrap();
        city.hp = 200;
        city.wall_hp = 100;
    }
    ai.observe_frame_cities(&host);
    let plan = plan_against(&host, cid);
    let force = group(&host, warrior, cid);
    ai.assess_siege(&host, 0, cid, &plan, &force);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage);
}

#[test]
fn a_volley_batch_is_contingent_and_nothing_is_off_the_live_adapter() {
    let (g, cid) = walled_city();
    let mut ai = AdvancedAi::new();
    ai.enable_phantom_capture_keeps_the_siege();
    let strike = Action::AirStrike {
        unit: 1,
        target: g.cities[&cid].pos,
    };
    // No frame observed: the gene reads no host and stays inert.
    assert!(!ai.capture_is_contingent(&g, cid));
    assert!(!ai.batch_capture_is_contingent(&g, cid, std::slice::from_ref(&strike)));
    ai.observe_frame_cities(&g);
    assert!(!ai.capture_is_contingent(&g, cid));
    assert!(ai.batch_capture_is_contingent(&g, cid, &[strike]));
    let mut off = AdvancedAi::new();
    off.observe_frame_cities(&g);
    assert!(
        off.phantom_frame_cities.is_none(),
        "gene off records nothing"
    );
}
