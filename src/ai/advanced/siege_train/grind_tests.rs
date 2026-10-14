//! `guns-grind-the-walls`: a fit gun at the ring opens walls the melee cannot
//! yet touch even when the whole capture outlasts the train's endurance; the
//! melee keep the staging ring meanwhile. Over the 10-05 control runs 430
//! siege-turns had the force at least half staged, a fit gun and walls up,
//! and held in Stage on a budget of a median 15.7 turns against 7.5 of
//! endurance, while a damaged wall stayed where the guns left it on 933 of
//! 1011 turns.

use std::sync::Arc;

use super::super::ForcePosture;
use super::tests::{at_distance, plan_against, walled_city};
use super::*;

/// Open grassland everywhere but the city tile, so steps cost one move and
/// every line is clear.
fn flatten(g: &mut Game, cid: u32) {
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
}

/// `walled_city` behind Medieval Walls at full strength, on open ground.
fn medieval_city() -> (Game, u32) {
    let (mut g, cid) = walled_city();
    flatten(&mut g, cid);
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let city = g.cities.get_mut(&cid).unwrap();
    city.buildings = vec![crate::name!("walls"), crate::name!("medieval_walls")];
    Arc::make_mut(&mut g.observed_city_max_wall_hp).remove(&cid);
    let max = g.city_max_wall_hp(&g.cities[&cid]);
    g.cities.get_mut(&cid).unwrap().wall_hp = max;
    (g, cid)
}

fn group_on(g: &Game, cid: u32, units: &[u32]) -> ForceGroup {
    ForceGroup {
        id: units[0],
        domain: ForceDomain::Land,
        units: units.to_vec(),
        anchor: g.units[&units[0]].pos,
        objective: g.cities[&cid].pos,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    }
}

/// One Catapult and two Men-at-Arms three tiles out of Medieval Walls: the
/// bill is met and the gun is fit, but the budget (with the live seat's
/// `siege-budget-counts-what-fires`) reads the walls and the city together
/// as about 20 turns against 15 of endurance.
/// The city, the first unit (the gun when there is one), the melee line, the
/// group on the city, the plan against it, and the seat.
type Train = (
    Game,
    u32,
    u32,
    Vec<u32>,
    ForceGroup,
    StrategicPlan,
    AdvancedAi,
);

fn one_gun_train() -> Train {
    train(true)
}

fn train(with_gun: bool) -> Train {
    let (mut g, cid) = medieval_city();
    let posts = at_distance(&g, cid, 3);
    let gun = if with_gun {
        g.spawn_unit("catapult", 0, posts[0])
    } else {
        g.spawn_unit("man_at_arms", 0, posts[0])
    };
    let melee: Vec<u32> = posts[1..3]
        .iter()
        .map(|pos| g.spawn_unit("man_at_arms", 0, *pos))
        .collect();
    let mut units = vec![gun];
    units.extend(&melee);
    let group = group_on(&g, cid, &units);
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_positive_damage_budget();
    ai.enable_siege_needs_a_breaker();
    ai.enable_siege_budget_counts_what_fires();
    ai.force_groups.push(group.clone());
    (g, cid, gun, melee, group, plan, ai)
}

#[test]
fn a_fit_gun_grinds_untouched_walls_the_budget_holds() {
    let (g, cid, gun, melee, group, plan, off) = one_gun_train();
    let mut on = off.clone();
    on.enable_guns_grind_the_walls();
    let mut off = off;

    let city = CityView::of(&g, cid).unwrap();
    let mut force = vec![gun];
    force.extend(&melee);
    let staged: f64 = force.iter().map(|uid| unit_power(&g, *uid)).sum();
    assert!(staged >= siege_bill(&g, 0, &city), "the bill is met");
    assert!(
        off.breach_reading(&g, 0, &city, &force).guns > 0,
        "the gun is fit"
    );
    assert!(
        !off.conversion_siege_ready(&g, 0, cid, &force),
        "the budget reads the capture as outlasting the line"
    );

    off.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(
        off.sieges[&cid].stage,
        SiegeStage::Stage,
        "off: the train holds"
    );
    assert!(!off.grinding_sieges.contains(&cid));

    on.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(
        on.sieges[&cid].stage,
        SiegeStage::Invest,
        "on: the gun opens the walls"
    );
    assert!(on.grinding_sieges.contains(&cid));
    // A grind is a running assault: the budget does not pull it back out.
    let mut g = g;
    for turn in 31..35 {
        g.turn = turn;
        on.assess_siege(&g, 0, cid, &plan, &group);
        assert_ne!(on.sieges[&cid].stage, SiegeStage::Stage, "turn {turn}");
    }
}

#[test]
fn the_melee_keep_the_staging_ring_while_the_guns_grind() {
    let (mut g, cid, _gun, melee, group, plan, ai) = one_gun_train();
    let mut on = ai;
    on.enable_guns_grind_the_walls();
    on.assess_siege(&g, 0, cid, &plan, &group);
    assert!(on.grinding_sieges.contains(&cid));
    let city = g.cities[&cid].pos;
    // Without the grind the invested melee step for a post beside the city.
    let mut posted = on.clone();
    posted.grinding_sieges.clear();
    let mut g2 = g.clone();
    let uid = melee[0];
    posted.siege_train_step(&mut g2, 0, uid, cid, &plan, &group);
    on.siege_train_step(&mut g, 0, uid, cid, &plan, &group);
    assert!(
        g.wdist(g.units[&uid].pos, city) > 2,
        "grinding: the melee stays out of the city's reach"
    );
    assert!(
        g2.wdist(g2.units[&uid].pos, city) < g.wdist(g.units[&uid].pos, city),
        "posted: the melee closes on its post"
    );
}

#[test]
fn walls_open_to_the_melee_or_no_gun_end_the_grind() {
    let (mut g, cid, gun, _melee, group, plan, ai) = one_gun_train();
    let mut on = ai;
    on.enable_guns_grind_the_walls();
    // Walls at a tenth: the melee can swing at them, so nothing grinds.
    let max = g.cities[&cid].wall_hp;
    g.cities.get_mut(&cid).unwrap().wall_hp = max / 10;
    on.assess_siege(&g, 0, cid, &plan, &group);
    assert!(!on.grinding_sieges.contains(&cid), "open walls: no grind");

    // Walls whole but no gun in the train: nothing to grind with.
    let (g, cid, _first, _melee, group, plan, ai) = train(false);
    let _ = gun;
    let mut on = ai;
    on.enable_guns_grind_the_walls();
    on.assess_siege(&g, 0, cid, &plan, &group);
    assert!(!on.grinding_sieges.contains(&cid), "no gun: no grind");
}
