use super::*;
use crate::ai::advanced::VictoryTarget;
use crate::game::HostMenuEntry;
use crate::name::Name;
use std::sync::Arc;

/// A walled capital at war with player 1 holding a paused Campus foundation
/// with 46 of 56 banked, a Warrior at the head of its queue, and the host's
/// quotes: two turns for the Campus, three for the Warrior.
fn board() -> (Game, u32, Item, Item, AdvancedAi) {
    let mut g = Game::new_full(
        2,
        74,
        46,
        crate::rng::fixture_seed("PAUSEDDISTRICT", 46_056),
        200,
        0,
        false,
    );
    g.current = 0;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| g.units[unit].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let cid = g.player_city_ids(0)[0];
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    if let Some(barb) = g.barb_pid {
        for uid in g
            .units
            .values()
            .filter(|unit| unit.owner == barb)
            .map(|unit| unit.id)
            .collect::<Vec<_>>()
        {
            g.remove_unit(uid);
        }
    }
    g.barb_camps.clear();
    g.at_war.insert((0, 1));
    g.players[0].techs.extend([
        crate::name!("writing"),
        crate::name!("masonry"),
        crate::name!("archery"),
        crate::name!("bronze_working"),
    ]);
    let site = g
        .district_sites(cid, crate::name!("campus"))
        .into_iter()
        .next()
        .expect("the capital has a Campus site");
    let campus = Item::District {
        district: crate::name!("campus"),
        pos: site,
    };
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: campus.clone(),
        },
    )
    .unwrap();
    let warrior = Item::Unit {
        unit: Name::new("warrior"),
    };
    g.apply(
        0,
        &Action::Produce {
            city: cid,
            item: warrior.clone(),
        },
    )
    .unwrap();
    let c = g.cities.get_mut(&cid).unwrap();
    c.buildings.push(crate::name!("walls"));
    c.wall_hp = 100;
    c.hp = CITY_MAX_HP;
    c.production = 0.0;
    c.production_progress
        .insert(Game::item_progress_key(&campus), 46.0);
    assert!(
        g.map.tiles[&site].district_foundation.is_some(),
        "the Campus stands as a foundation"
    );
    assert_eq!(g.cities[&cid].queue.first(), Some(&warrior));
    Arc::make_mut(&mut g.host_buildable).insert(
        cid,
        [
            (
                Game::production_block_key(&campus),
                HostMenuEntry {
                    cost: Some(56.0),
                    turns: Some(2.0),
                },
            ),
            (
                Game::production_block_key(&warrior),
                HostMenuEntry {
                    cost: Some(20.0),
                    turns: Some(3.0),
                },
            ),
        ]
        .into(),
    );
    let mut ai = AdvancedAi::new();
    ai.enable_paused_district_finishes();
    (g, cid, campus, warrior, ai)
}

fn set_turns(g: &mut Game, cid: u32, item: &Item, turns: Option<f64>) {
    Arc::make_mut(&mut g.host_buildable)
        .get_mut(&cid)
        .unwrap()
        .get_mut(&Game::production_block_key(item))
        .unwrap()
        .turns = turns;
}

#[test]
fn a_two_turn_paused_campus_goes_back_to_the_head() {
    let (mut g, cid, campus, _, ai) = board();
    let (item, turns, banked) = ai.paused_district_finish_item(&g, 0, cid).unwrap();
    assert_eq!(
        (item.clone(), turns, banked),
        (campus.clone(), 2.0, Some(46.0))
    );
    ai.claim_paused_district_finishes(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
}

#[test]
fn the_gene_off_leaves_the_queue_alone() {
    let (mut g, cid, _, warrior, mut ai) = board();
    ai.disable_paused_district_finishes();
    assert!(ai.paused_district_finish_item(&g, 0, cid).is_none());
    ai.claim_paused_district_finishes(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&warrior));
}

#[test]
fn a_three_turn_foundation_waits() {
    let (mut g, cid, campus, warrior, ai) = board();
    set_turns(&mut g, cid, &campus, Some(3.0));
    ai.claim_paused_district_finishes(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&warrior));
}

#[test]
fn an_unquoted_foundation_waits() {
    let (mut g, cid, campus, warrior, ai) = board();
    set_turns(&mut g, cid, &campus, None);
    ai.claim_paused_district_finishes(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&warrior));
}

/// The live mirror often never reads a paused foundation's banked work (see
/// the module note); the foundation and the host's quote are the signal.
#[test]
fn a_foundation_whose_work_the_mirror_did_not_read_still_finishes() {
    let (mut g, cid, campus, _, ai) = board();
    g.cities
        .get_mut(&cid)
        .unwrap()
        .production_progress
        .remove(&Game::item_progress_key(&campus));
    let (item, turns, banked) = ai.paused_district_finish_item(&g, 0, cid).unwrap();
    assert_eq!((item, turns, banked), (campus.clone(), 2.0, None));
    ai.claim_paused_district_finishes(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&campus));
}

#[test]
fn a_city_with_no_foundation_on_the_board_waits() {
    let (mut g, cid, campus, warrior, ai) = board();
    let Item::District { pos, .. } = campus else {
        unreachable!()
    };
    g.map.tiles.get_mut(&pos).unwrap().district_foundation = None;
    ai.claim_paused_district_finishes(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&warrior));
}

#[test]
fn any_city_or_wall_damage_leaves_the_queue_to_the_defence() {
    for wall in [false, true] {
        let (mut g, cid, _, warrior, ai) = board();
        let c = g.cities.get_mut(&cid).unwrap();
        if wall {
            c.wall_hp -= 10;
        } else {
            c.hp -= 10;
        }
        ai.claim_paused_district_finishes(&mut g, 0);
        assert_eq!(g.cities[&cid].queue.first(), Some(&warrior));
    }
}

#[test]
fn a_head_the_host_finishes_this_turn_keeps_its_turn() {
    let (mut g, cid, _, warrior, ai) = board();
    set_turns(&mut g, cid, &warrior, Some(1.0));
    ai.claim_paused_district_finishes(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&warrior));
}

#[test]
fn an_attacker_in_reach_leaves_the_queue_to_the_defence() {
    let (mut g, cid, _, warrior, ai) = board();
    let center = g.cities[&cid].pos;
    let post = g.cities[&cid]
        .owned_tiles
        .iter()
        .copied()
        .find(|pos| {
            g.wdist(center, *pos) == 1
                && g.map.tiles[pos].district_foundation.is_none()
                && !g.rules.is_water(&g.map.tiles[pos])
        })
        .unwrap();
    let tile = g.map.tiles.get_mut(&post).unwrap();
    tile.terrain = crate::name!("plains");
    tile.feature = None;
    tile.hills = false;
    let attacker = g.spawn_test_unit("line_infantry", 1, post);
    g.units.get_mut(&attacker).unwrap().moves_left = 0.0;
    assert!(AdvancedAi::imminent_city_attack(
        &g,
        0,
        cid,
        &g.player_vision_frame(0)
    ));
    ai.claim_paused_district_finishes(&mut g, 0);
    assert_eq!(g.cities[&cid].queue.first(), Some(&warrior));
}

/// The zero-damage war handoff: with the gene on, the defence redirect keeps
/// a paused foundation the city is finishing and leaves an undamaged city
/// that holds one; with it off, the Campus is paused for a defender.
#[test]
fn the_war_handoff_keeps_the_finishing_foundation() {
    for on in [true, false] {
        let (mut g, cid, campus, _, mut ai) = board();
        ai.enable_garrison_under_fire();
        ai.victory_planning = true;
        ai.victory_target = Some(VictoryTarget::Domination);
        if !on {
            ai.disable_paused_district_finishes();
        }
        // The city is finishing the Campus: it is the head, work active.
        g.apply(
            0,
            &Action::Produce {
                city: cid,
                item: campus.clone(),
            },
        )
        .unwrap();
        g.cities.get_mut(&cid).unwrap().production = 46.0;
        g.cities
            .get_mut(&cid)
            .unwrap()
            .production_progress
            .remove(&Game::item_progress_key(&campus));
        assert!(ai
            .preemptive_major_war_defense_item(&g, 0, cid, Some(cid), true, false)
            .is_some());
        ai.redirect_unsafe_city_queue_for_defense(&mut g, 0, Some(cid));
        if on {
            assert_eq!(
                g.cities[&cid].queue.first(),
                Some(&campus),
                "finish the two-turn Campus behind undamaged walls"
            );
        } else {
            assert_ne!(g.cities[&cid].queue.first(), Some(&campus));
        }
    }
}
