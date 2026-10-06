//! `siege-needs-a-breaker`, and the live seat's siege gun firing after it
//! moves. Live King civvis-20261003T135713Z held Kwadukuza's ring with
//! Knights and Men-at-Arms against Medieval Walls while its Bombards healed
//! or walked in circles and its Siege Tower stood seven tiles off.

use std::sync::Arc;

use super::super::ForcePosture;
use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

/// The board as the live seat sees it: every tile observed by the host.
fn mirrored(g: &mut Game) {
    g.host_observed = Arc::new(g.map.tiles.keys().copied().collect());
}

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
    // The position states its wall pool as observed; read the buildings.
    Arc::make_mut(&mut g.observed_city_max_wall_hp).remove(&cid);
    let max = g.city_max_wall_hp(&g.cities[&cid]);
    g.cities.get_mut(&cid).unwrap().wall_hp = max;
    assert_eq!(max, 200);
    (g, cid)
}

/// A land force group on the city, as the board projects one.
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

fn reducing(ai: &mut AdvancedAi, cid: u32) {
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Reduce,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
}

/// The gun two tiles out on a line to the city, stepped there from three.
fn a_step_into_range(g: &Game, cid: u32) -> (Pos, Pos) {
    let city = g.cities[&cid].pos;
    for post in at_distance(g, cid, 2) {
        if !g.line_of_sight_from(post, city) {
            continue;
        }
        if let Some(from) = g
            .nbrs(post)
            .into_iter()
            .find(|from| g.wdist(*from, city) == 3 && g.map.get(*from).is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t)))
        {
            return (from, post);
        }
    }
    panic!("the storming board has an approach");
}

#[test]
fn the_live_seat_fires_a_siege_gun_after_it_moves_and_a_native_board_does_not() {
    for (live, expect_fire) in [(false, false), (true, true)] {
        let (mut g, cid) = walled_city();
        flatten(&mut g, cid);
        g.at_war.insert((0, 1));
        if live {
            mirrored(&mut g);
        }
        let (from, post) = a_step_into_range(&g, cid);
        let gun = g.spawn_unit("catapult", 0, from);
        g.apply(0, &Action::Move { unit: gun, to: post }).unwrap();
        assert!(g.units[&gun].moved && g.units[&gun].moves_left > 0.0);
        let walls = g.cities[&cid].wall_hp;
        let target = g.cities[&cid].pos;
        let shot = g.apply(0, &Action::Ranged { unit: gun, target });
        assert_eq!(shot.is_ok(), expect_fire, "live {live}: {shot:?}");
        if expect_fire {
            assert!(g.cities[&cid].wall_hp < walls, "the shot lands on the wall");
        }
    }
}

#[test]
fn a_hostile_siege_gun_on_the_live_board_keeps_the_rule() {
    let (mut g, cid) = walled_city();
    mirrored(&mut g);
    let far = at_distance(&g, cid, 4);
    let ours = g.spawn_unit("catapult", 0, far[0]);
    let theirs = g.spawn_unit("catapult", 1, far[far.len() / 2]);
    assert!(g.siege_may_attack_after_moving(&g.units[&ours]));
    assert!(!g.siege_may_attack_after_moving(&g.units[&theirs]));
    let mut native = g.clone();
    native.host_observed = Arc::new(BTreeSet::new());
    assert!(!native.siege_may_attack_after_moving(&native.units[&ours]));
}

/// Kwadukuza, turns 113 to 122: melee on the ring, the one gun in reach
/// healing. The train falls back to Stage on the second short reading and
/// does not enter Invest again while nothing can open the walls.
#[test]
fn melee_do_not_hold_the_ring_of_walls_nothing_can_open() {
    let (mut g, cid) = medieval_city();
    let mut units: Vec<u32> = ring_of(&g, cid)
        .into_iter()
        .take(3)
        .map(|pos| g.spawn_unit("man_at_arms", 0, pos))
        .collect();
    let gun_pos = at_distance(&g, cid, 4)[0];
    let gun = g.spawn_unit("bombard", 0, gun_pos);
    g.units.get_mut(&gun).unwrap().hp = 29;
    units.push(gun);
    let group = group_on(&g, cid, &units);
    let plan = plan_against(&g, cid);

    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_positive_damage_budget();
    ai.force_groups.push(group.clone());
    reducing(&mut ai, cid);
    let mut off = ai.clone();
    ai.enable_siege_needs_a_breaker();

    let city = CityView::of(&g, cid).unwrap();
    let reading = ai.breach_reading(&g, 0, &city, &units);
    assert_eq!((reading.guns, reading.wounded_guns, reading.support), (0, 1, 0));
    assert!(!reading.at_hand(&city));

    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Reduce, "one short turn holds");
    // The hold is recorded, and the healing gun is a breaker on its way.
    assert_eq!(
        ai.siege_breaker_waits.get(&city.pos),
        Some(&BreakerWait {
            since: 30,
            last: 30,
            nearest: 4,
            nearest_turn: 30,
        })
    );
    assert!(ai.waiting_for_a_breaker(&g, 0, cid));
    assert!(!off.waiting_for_a_breaker(&g, 0, cid), "gene off");
    g.turn = 31;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage);
    g.turn = 32;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage, "no breaker, no Invest");

    // The budget counted the healing gun as firing every turn.
    off.assess_siege(&g, 0, cid, &plan, &group);
    g.turn = 33;
    off.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(off.sieges[&cid].stage, SiegeStage::Reduce);

    // A staged man-at-arms steps out of the city's reach.
    let melee = units[0];
    let view = CityView::of(&g, cid).unwrap();
    for _ in 0..4 {
        if g.units[&melee].moves_left <= 0.0 || !ai.siege_stage_step(&mut g, 0, melee, &view, &plan) {
            break;
        }
    }
    assert!(g.wdist(g.units[&melee].pos, view.pos) > CITY_STRIKE_RANGE);
}

#[test]
fn a_fit_gun_or_low_walls_are_a_breaker_and_shooters_count_by_what_they_breach() {
    let (mut g, cid) = medieval_city();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    let near = at_distance(&g, cid, 3);

    let gun = g.spawn_unit("bombard", 0, near[0]);
    let city = CityView::of(&g, cid).unwrap();
    let reading = ai.breach_reading(&g, 0, &city, &[gun]);
    assert_eq!(reading.guns, 1);
    assert!(reading.at_hand(&city));
    ai.battle_planner_recovering.insert(gun);
    let healing = ai.breach_reading(&g, 0, &city, &[gun]);
    assert_eq!((healing.guns, healing.wounded_guns), (0, 1));
    assert!(!healing.at_hand(&city));
    g.remove_unit(gun);

    // Two Crossbows chip Medieval Walls at a few points a shot: not a breach.
    let bows: Vec<u32> = near[1..3]
        .iter()
        .map(|pos| g.spawn_unit("crossbowman", 0, *pos))
        .collect();
    let city = CityView::of(&g, cid).unwrap();
    let reading = ai.breach_reading(&g, 0, &city, &bows);
    assert!(reading.shooter_walls > 0.0);
    assert!(!reading.at_hand(&city), "{reading:?}");
    // The same Crossbows against 30 points of wall are.
    g.cities.get_mut(&cid).unwrap().wall_hp = 50;
    let city = CityView::of(&g, cid).unwrap();
    assert!(ai.breach_reading(&g, 0, &city, &bows).at_hand(&city));
    // And walls a melee blow opens need no breaker at all.
    g.cities.get_mut(&cid).unwrap().wall_hp = 30;
    let city = CityView::of(&g, cid).unwrap();
    assert!(ai.breach_reading(&g, 0, &city, &[]).at_hand(&city));
}

#[test]
fn a_siege_tower_beside_a_melee_member_is_a_breaker_against_medieval_walls() {
    let (mut g, cid) = medieval_city();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    let near = at_distance(&g, cid, 3);
    let melee = g.spawn_unit("man_at_arms", 0, near[0]);
    let tower = g.spawn_unit("siege_tower", 0, near[1]);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.breach_reading(&g, 0, &city, &[melee]).support, 1);
    // Renaissance Walls are immune to it.
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("renaissance_walls"));
    g.cities.get_mut(&cid).unwrap().wall_hp = 300;
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.breach_reading(&g, 0, &city, &[melee]).support, 0);
    assert!(g.units.contains_key(&tower));
}

/// The Siege Tower walks onto a melee member of the train and links to it,
/// and leaves a carrier that is not one.
#[test]
fn a_siege_tower_joins_a_melee_member_of_the_train() {
    let (mut g, cid) = medieval_city();
    let city = g.cities[&cid].pos;
    let stand = at_distance(&g, cid, 4)[0];
    let melee = g.spawn_unit("man_at_arms", 0, stand);
    let start = g
        .wring(stand, 2)
        .into_iter()
        .find(|pos| g.wdist(*pos, city) >= 4 && g.map.get(*pos).is_some_and(|t| g.rules.is_passable(t)))
        .unwrap();
    let tower = g.spawn_unit("siege_tower", 0, start);
    // A stray warrior carries it now: not a member of the train.
    let stray = g.spawn_unit("warrior", 0, start);
    g.apply(0, &Action::LinkUnits { unit: stray, with: tower }).unwrap();
    let group = group_on(&g, cid, &[melee]);
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.force_groups.push(group);
    reducing(&mut ai, cid);

    let acted = ai.breach_support_step(&mut g, 0, tower, &plan);
    assert_eq!(acted, Some(true));
    assert_eq!(g.units[&stray].linked_to, None, "the stray lets it go");
    assert_eq!(g.units[&tower].pos, stand, "it walks onto the member");
    assert_eq!(g.units[&tower].linked_to, Some(melee));
    assert_eq!(g.units[&melee].linked_to, Some(tower));
    // Linked to a member, it rides: the carrier's orders move the pair.
    assert_eq!(ai.breach_support_step(&mut g, 0, tower, &plan), Some(false));

    // Without the gene the doctrine leaves it to the ladder.
    let mut off = AdvancedAi::new();
    off.enable_siege_train();
    assert_eq!(off.siege_doctrine_step(&mut g, 0, tower, &plan), None);
}

/// Thebes, game 43: Archers that breach 100 walls in five turns staged for
/// ten, because the damage budget reads a train with no melee taker in
/// reach as never finishing. With the gene the train invests to open the
/// walls and stays invested while they come down.
#[test]
fn shooters_open_the_walls_before_the_taker_comes() {
    let (mut g, cid) = walled_city();
    flatten(&mut g, cid);
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let bows: Vec<u32> = at_distance(&g, cid, 3)
        .into_iter()
        .take(4)
        .map(|pos| g.spawn_unit("crossbowman", 0, pos))
        .collect();
    let group = group_on(&g, cid, &bows);
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_positive_damage_budget();
    ai.force_groups.push(group.clone());
    let mut off = ai.clone();
    ai.enable_siege_needs_a_breaker();

    let city = CityView::of(&g, cid).unwrap();
    let staged: f64 = bows.iter().map(|uid| unit_power(&g, *uid)).sum();
    assert!(staged >= siege_bill(&g, 0, &city), "the bill is met");
    assert!(ai.breach_reading(&g, 0, &city, &bows).at_hand(&city));
    assert!(
        !ai.conversion_siege_ready(&g, 0, cid, &bows),
        "no taker: the budget never finishes"
    );

    off.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(off.sieges[&cid].stage, SiegeStage::Stage);
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Invest);
    for turn in 31..35 {
        g.turn = turn;
        ai.assess_siege(&g, 0, cid, &plan, &group);
        assert_ne!(ai.sieges[&cid].stage, SiegeStage::Stage, "turn {turn}");
    }
}

/// A breaker counts as coming while it comes nearer, or while a city of
/// ours within reach is about to finish one; a gun that stands off for
/// `BREAKER_STALL_TURNS`, or one barely started, does not. Live King
/// civvis-20261004T025448Z held Napata "for a wall-breaker on its way" for
/// sixteen turns on a Catapult that never came nearer than eight tiles.
#[test]
fn a_breaker_is_coming_only_while_it_comes_nearer_or_is_nearly_built() {
    let (mut g, cid) = medieval_city();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    let pos = g.cities[&cid].pos;
    let stall = g.standard_duration(BREAKER_STALL_TURNS);
    let wait = |nearest: i32, nearest_turn: u32| BreakerWait {
        since: 30,
        last: 30 + stall + 1,
        nearest,
        nearest_turn,
    };
    g.turn = 30 + stall + 1;
    ai.siege_breaker_waits.insert(pos, wait(8, 30 + stall));
    assert!(ai.waiting_for_a_breaker(&g, 0, cid), "it came nearer lately");
    ai.siege_breaker_waits.insert(pos, wait(8, 30));
    assert!(!ai.waiting_for_a_breaker(&g, 0, cid), "it stood off");

    // A Catapult nearly built in a city of ours within reach is coming; one
    // barely started is not.
    let site = at_distance(&g, cid, 10)
        .into_iter()
        .find(|p| g.city_at(*p).is_none())
        .expect("land for a city of ours");
    let own = g.found_city_for(0, site, None);
    let catapult = crate::game::Item::Unit {
        unit: crate::name!("catapult"),
    };
    let cost = g.item_cost_for_city(0, own, &catapult);
    g.cities.get_mut(&own).unwrap().queue = vec![catapult];
    g.cities.get_mut(&own).unwrap().production = cost - 1.0;
    assert!(ai.waiting_for_a_breaker(&g, 0, cid), "nearly built");
    g.cities.get_mut(&own).unwrap().production = 0.0;
    assert!(!ai.waiting_for_a_breaker(&g, 0, cid), "barely started");
}

/// A train that dwarfs its bill gives its shooters twice as long to breach:
/// two Crossbows chip 200 walls too slowly for a train of their own size,
/// and fast enough beside four Knights of the same train.
#[test]
fn a_dominant_train_gives_its_shooters_longer_to_breach() {
    let (mut g, cid) = medieval_city();
    let near = at_distance(&g, cid, 3);
    let bows: Vec<u32> = near[..2]
        .iter()
        .map(|pos| g.spawn_unit("crossbowman", 0, *pos))
        .collect();
    let plan = plan_against(&g, cid);
    let city = CityView::of(&g, cid).unwrap();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    let shooters = ai.breach_reading(&g, 0, &city, &bows).shooter_walls;
    assert!(
        shooters * SHOOTER_BREACH_TURNS < f64::from(city.wall_hp)
            && shooters * SHOOTER_BREACH_TURNS_DOMINANT >= f64::from(city.wall_hp),
        "the fixture sits between the two horizons: {shooters:.1} a turn"
    );

    let small = group_on(&g, cid, &bows);
    let mut lone = ai.clone();
    lone.force_groups.push(small.clone());
    lone.assess_siege(&g, 0, cid, &plan, &small);
    assert!(lone.siege_breaker_waits.contains_key(&city.pos), "no breaker");

    let mut units = bows.clone();
    for pos in at_distance(&g, cid, 8).into_iter().take(4) {
        units.push(g.spawn_unit("knight", 0, pos));
    }
    let strength: f64 = units.iter().map(|uid| unit_power(&g, *uid)).sum();
    assert!(strength >= DOMINANT_BILL_SHARE * siege_bill(&g, 0, &city));
    let big = group_on(&g, cid, &units);
    let mut dominant = ai.clone();
    dominant.force_groups.push(big.clone());
    dominant.assess_siege(&g, 0, cid, &plan, &big);
    assert!(
        !dominant.siege_breaker_waits.contains_key(&city.pos),
        "the shooters are the breaker"
    );
}

/// `siege-counts-posted-shooters`: live King civvis-20261004T122037Z (game
/// 62) read Yaroslavl at "shooters 6-16 wall a turn" off the archers within
/// five tiles while its walls fell 16 points in twelve turns: the range band
/// held few free tiles, and a shooter with a hostile unit in reach shoots that
/// first while the walls stand.
#[test]
fn only_shooters_with_a_firing_post_and_a_clear_reach_count_toward_the_walls() {
    let (mut g, cid) = medieval_city();
    mirrored(&mut g);
    let center = g.cities[&cid].pos;
    // Water beside the city and mountain two out, but for two firing tiles.
    let open: Vec<Pos> = at_distance(&g, cid, 2)[..2].to_vec();
    for pos in g.wring(center, 1) {
        g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast");
    }
    for pos in g.wring(center, 2) {
        if !open.contains(&pos) {
            g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
        }
    }
    let bows: Vec<u32> = at_distance(&g, cid, 3)[..5]
        .iter()
        .map(|pos| g.spawn_unit("crossbowman", 0, *pos))
        .collect();
    let city = CityView::of(&g, cid).unwrap();
    let per_shot = wall_damage_per_shot(&g, bows[0], cid);
    assert!(per_shot > 0.0);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    let every_bow = ai.breach_reading(&g, 0, &city, &bows).shooter_walls;
    assert!((every_bow - 5.0 * per_shot).abs() < 1e-6, "{every_bow} vs {per_shot}");

    ai.enable_siege_counts_posted_shooters();
    let (posted, walls) = posted_shooters(&g, 0, cid, &bows);
    assert_eq!((posted.len(), walls.len()), (2, 2), "two firing tiles, two posts");
    let counted = ai.breach_reading(&g, 0, &city, &bows).shooter_walls;
    assert!((counted - 2.0 * per_shot).abs() < 1e-6, "{counted} vs {per_shot}");

    // A defender within reach of one post draws that bow's shot.
    let posts = siege_posts(&g, 0, &city, &bows, None);
    let first = posts[posted.iter().next().unwrap()];
    let other = posts[posted.iter().nth(1).unwrap()];
    let guard = g
        .wring(first, 1)
        .into_iter()
        .filter(|pos| {
            g.wdist(*pos, other) > 2
                && g.map.get(*pos).is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
                && g.unit_ids_at(*pos).is_empty()
                && *pos != center
        })
        .min()
        .or_else(|| {
            g.wring(first, 1).into_iter().find(|pos| {
                g.map.get(*pos).is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
                    && g.unit_ids_at(*pos).is_empty()
            })
        })
        .expect("a passable tile beside the post");
    g.spawn_unit("warrior", 1, guard);
    let (_, walls) = posted_shooters(&g, 0, cid, &bows);
    assert!(walls.len() < 2, "the defender draws a shot: {walls:?}");
    let drawn = ai.breach_reading(&g, 0, &city, &bows).shooter_walls;
    assert!(drawn < counted, "{drawn} < {counted}");
}

/// Under `declaration-waits-for-the-breach` a held declaration is held on
/// every offensive path: the overwhelming, road-opening and moving-on paths
/// read the cleared `staged` as an unstaged army, so game 206 declared two
/// turns into its hold at Eindhoven. Off, the hold bars only the staged war.
#[test]
fn a_held_declaration_bars_the_unstaged_paths_under_the_gene() {
    let mut ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    assert!(!ai.hold_bars_the_unstaged_paths(true), "off");
    ai.enable_declaration_waits_for_the_breach();
    assert!(ai.hold_bars_the_unstaged_paths(true), "on and held");
    assert!(!ai.hold_bars_the_unstaged_paths(false), "on, no hold");
}

/// Under `declaration-waits-for-the-breach` the declaration hold's patience
/// is a hard cap: one turn the reading passes keeps the clock, and only
/// `DECLARATION_HOLD_RESET` unheld turns in a row restart it. Live Emperor
/// civvis-20261006T065830Z (game 204) held on Eger from turn 137 to 148
/// across a ten-turn patience. Off, a passing turn restarts the clock.
#[test]
fn the_declaration_hold_keeps_its_clock_through_a_passing_turn() {
    let (mut g, cid) = medieval_city();
    let pos = g.cities[&cid].pos;
    let patience = g.standard_duration(DECLARATION_BREAKER_PATIENCE);
    let reset = g.standard_duration(DECLARATION_HOLD_RESET);
    let mut on = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    on.enable_declaration_waits_for_the_breach();
    let mut off = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    for ai in [&mut on, &mut off] {
        g.turn = 100;
        assert!(ai.declaration_hold_patience(&g, Some(pos), true, true));
        g.turn = 101;
        assert!(!ai.declaration_hold_patience(&g, Some(pos), false, false));
    }
    g.turn = 100 + patience;
    assert!(
        !on.declaration_hold_patience(&g, Some(pos), true, true),
        "on: the clock kept through the passing turn runs out"
    );
    assert!(
        off.declaration_hold_patience(&g, Some(pos), true, true),
        "off: the passing turn restarted it"
    );
    // A run of unheld turns restarts it under the gene too.
    let mut on = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    on.enable_declaration_waits_for_the_breach();
    g.turn = 100;
    assert!(on.declaration_hold_patience(&g, Some(pos), true, true));
    for turn in 101..=100 + reset {
        g.turn = turn;
        assert!(!on.declaration_hold_patience(&g, Some(pos), false, false));
    }
    g.turn = 101 + reset;
    assert!(on.declaration_hold_patience(&g, Some(pos), true, true));
    g.turn = 100 + patience;
    assert!(
        on.declaration_hold_patience(&g, Some(pos), true, true),
        "restarted after {reset} unheld turns"
    );
}

/// `unwalled-target-declares-into-the-strike`: the declaration turn's blows
/// on a city with no walls count only what can reach it this turn — not a
/// Warrior three tiles out, but a Horseman — at most one melee per land tile
/// beside it; a walled city reads none (the breach readings decide it).
/// Live Emperor civvis-20261006T061618Z (game 201) declared on Poland with
/// nobody near Bydgoszcz, and it stood behind 400 walls the next turn.
#[test]
fn the_first_strike_counts_what_can_hit_an_unwalled_city_this_turn() {
    let (mut g, cid) = medieval_city();
    g.at_war.remove(&(0, 1));
    g.cities.get_mut(&cid).unwrap().buildings.clear();
    Arc::make_mut(&mut g.observed_city_max_wall_hp).remove(&cid);
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    assert_eq!(g.city_max_wall_hp(&g.cities[&cid]), 0, "fixture: no walls");
    let ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    assert_eq!(ai.first_strike_blows(&g, 0, cid), Some(0.0), "nobody in reach");
    let three = at_distance(&g, cid, 3);
    g.spawn_test_unit("warrior", 0, three[0]);
    assert_eq!(
        ai.first_strike_blows(&g, 0, cid),
        Some(0.0),
        "a Warrior three tiles out cannot strike this turn"
    );
    g.spawn_test_unit("horseman", 0, three[1]);
    let one = ai.first_strike_blows(&g, 0, cid).unwrap();
    assert!(one > 0.0, "a Horseman three tiles out strikes this turn");
    for pos in three.iter().skip(2).take(8) {
        g.spawn_test_unit("horseman", 0, *pos);
    }
    let many = ai.first_strike_blows(&g, 0, cid).unwrap();
    let slots = g
        .wring(g.cities[&cid].pos, 1)
        .into_iter()
        .filter(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .count();
    assert!(
        (many - one * slots as f64).abs() < 1e-6,
        "one Horseman per land tile beside the city: {many:.1} vs {slots} x {one:.1}"
    );
    let city = g.cities.get_mut(&cid).unwrap();
    city.buildings = vec![crate::name!("walls")];
    Arc::make_mut(&mut g.observed_city_max_wall_hp).remove(&cid);
    assert_eq!(ai.first_strike_blows(&g, 0, cid), None, "walls: the breach decides");
}

/// `declaration-waits-for-the-breach`: one gun on the ring satisfies the
/// breaker hold but not the breach. Live Emperor civvis-20261006T044739Z
/// (game 194) declared on Vietnam with Dong Hoi behind 200 walls, seven of
/// ten staged and one gun fit, reading 18.0 turns against 8.7 endurance; the
/// walls stood at 300 by turn 89 and no city fell. More guns shorten the
/// reading; a city a melee blow opens needs none.
#[test]
fn one_gun_is_at_hand_but_short_of_the_breach() {
    let (mut g, cid) = medieval_city();
    g.at_war.remove(&(0, 1));
    let mut ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.enable_siege_positive_damage_budget();
    ai.enable_siege_budget_counts_what_fires();
    let near = at_distance(&g, cid, 3);
    for pos in near.iter().take(2) {
        g.spawn_test_unit("man_at_arms", 0, *pos);
    }
    g.spawn_test_unit("catapult", 0, near[2]);
    assert!(
        ai.declaration_breaker_at_hand(&g, 0, cid),
        "one gun satisfies the breaker hold"
    );
    let (turns, endurance, guns) = ai
        .declaration_breach_reading(&g, 0, cid)
        .expect("a walled city reads a budget");
    assert_eq!(guns, 1);
    assert!(
        turns > endurance * DECLARATION_BREACH_SHARE,
        "one catapult against 200 walls is short of the breach: {turns:.1} turns vs {endurance:.1}"
    );
    let far = at_distance(&g, cid, 7);
    for pos in far.iter().take(3) {
        g.spawn_test_unit("bombard", 0, *pos);
    }
    let (more, _, guns) = ai.declaration_breach_reading(&g, 0, cid).unwrap();
    assert_eq!(guns, 4, "guns within the muster's reach count");
    assert!(more < turns, "more guns, a shorter breach: {more:.1} vs {turns:.1}");
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    assert_eq!(
        ai.declaration_breach_reading(&g, 0, cid),
        None,
        "an open city waits for no gun"
    );
}

/// `declaration-waits-for-the-breaker`: a walled objective is breachable
/// before the war only with a siege gun, or a ram or tower that works on its
/// walls, on its ring. Live King civvis-20261005T053701Z (game 103) declared
/// on the Maori with Opango's Catapult 17 tiles out; the walls doubled first.
#[test]
fn a_walled_city_needs_a_breaker_on_its_ring_before_the_war() {
    let (mut g, cid) = walled_city();
    let ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    assert!(g.cities[&cid].wall_hp > 0, "fixture: walls stand");
    assert!(
        !ai.declaration_breaker_at_hand(&g, 0, cid),
        "nothing on the ring"
    );
    let far = at_distance(&g, cid, STAGING_FAR + 3)[0];
    let gun = g.spawn_test_unit("catapult", 0, far);
    assert!(
        !ai.declaration_breaker_at_hand(&g, 0, cid),
        "a gun on the road is not at hand"
    );
    let near = at_distance(&g, cid, STAGING_FAR - 1)[0];
    g.units.get_mut(&gun).unwrap().pos = near;
    assert!(
        ai.declaration_breaker_at_hand(&g, 0, cid),
        "a gun on the ring"
    );
    g.remove_unit(gun);
    // A ram works on Ancient Walls.
    g.spawn_test_unit("battering_ram", 0, near);
    assert!(
        ai.declaration_breaker_at_hand(&g, 0, cid),
        "a ram that opens these walls"
    );
    // Bare walls need no breaker at all.
    let (mut bare, bare_city) = walled_city();
    bare.cities.get_mut(&bare_city).unwrap().wall_hp = 0;
    assert!(ai.declaration_breaker_at_hand(&bare, 0, bare_city));
}

/// `declaration-waits-for-the-breaker`: the hold has patience. Ten standard
/// turns from the first held turn on a tile, through turns the army reads
/// unstaged, then the declaration goes ahead; a breaker's arrival or a new
/// objective starts a fresh clock.
#[test]
fn the_declaration_holds_for_the_breaker_only_so_long() {
    let (mut g, _) = walled_city();
    let mut ai = AdvancedAi::targeting(super::super::VictoryTarget::Domination);
    let (here, there) = ((10, 10), (20, 10));
    let patience = g.standard_duration(DECLARATION_BREAKER_PATIENCE);
    assert!(ai.declaration_hold_patience(&g, Some(here), true, true));
    g.turn += 1;
    assert!(
        !ai.declaration_hold_patience(&g, Some(here), true, false),
        "an unstaged turn holds nothing"
    );
    g.turn += patience - 2;
    assert!(
        ai.declaration_hold_patience(&g, Some(here), true, true),
        "the clock ran through it"
    );
    g.turn += 1;
    assert!(
        !ai.declaration_hold_patience(&g, Some(here), true, true),
        "patience spent"
    );
    assert!(
        ai.declaration_hold_patience(&g, Some(there), true, true),
        "a new objective"
    );
    assert!(!ai.declaration_hold_patience(&g, Some(there), false, true));
    assert!(
        ai.declaration_hold_patience(&g, Some(there), true, true),
        "a breaker that left starts a fresh clock"
    );
}

/// A second walled city of the same enemy `distance` tiles from `cid`, on
/// the flattened board, with Ancient Walls at full strength.
fn second_walled_city(g: &mut Game, cid: u32, distance: i32) -> u32 {
    let site = at_distance(g, cid, distance)[0];
    let other = g.found_city_for(1, site, None);
    let city = g.cities.get_mut(&other).unwrap();
    city.buildings = vec![crate::name!("walls")];
    Arc::make_mut(&mut g.observed_city_max_wall_hp).remove(&other);
    let max = g.city_max_wall_hp(&g.cities[&other]);
    assert!(max > 0);
    g.cities.get_mut(&other).unwrap().wall_hp = max;
    other
}

/// Tiles exactly `a` from city `first` and `b` from city `second`.
fn between(g: &Game, first: u32, second: u32, a: i32, b: i32) -> Vec<Pos> {
    let (one, two) = (g.cities[&first].pos, g.cities[&second].pos);
    let mut tiles: Vec<Pos> = g
        .wring(one, a)
        .into_iter()
        .filter(|pos| {
            g.wdist(*pos, two) == b
                && g.unit_ids_at(*pos).is_empty()
                && g.city_at(*pos).is_none()
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .collect();
    tiles.sort_unstable();
    tiles
}

/// Live King civvis-20261005T051413Z (game 102): three healthy Bombards of
/// Uruk's force stood four and five tiles from Toronto, whose own force had
/// four units staged and no gun; Toronto's reading said "0 gun(s) fit".
/// Under `breach-counts-nearby-guns` a fit gun at a walled city's ring is
/// that city's breaker whatever row it serves: Toronto reads three guns and
/// its damage budget is ready; the other siege no longer counts them.
#[test]
fn guns_of_another_siege_at_this_ring_are_this_sieges_breakers_under_the_gene() {
    let (mut g, toronto) = medieval_city();
    let uruk = second_walled_city(&mut g, toronto, 9);
    let melee: Vec<u32> = at_distance(&g, toronto, 3)
        .into_iter()
        .take(4)
        .map(|pos| g.spawn_unit("man_at_arms", 0, pos))
        .collect();
    let guns: Vec<u32> = between(&g, toronto, uruk, 4, 5)
        .into_iter()
        .chain(between(&g, toronto, uruk, 4, 6))
        .take(3)
        .map(|pos| g.spawn_unit("bombard", 0, pos))
        .collect();
    assert_eq!(guns.len(), 3);
    let toronto_group = group_on(&g, toronto, &melee);
    let uruk_group = group_on(&g, uruk, &guns);
    let plan = plan_against(&g, toronto);
    let view = CityView::of(&g, toronto).unwrap();

    let mut off = AdvancedAi::new();
    off.enable_siege_train();
    off.enable_siege_positive_damage_budget();
    off.enable_siege_needs_a_breaker();
    off.force_groups = vec![toronto_group.clone(), uruk_group.clone()];
    let mut on = off.clone();
    on.enable_breach_counts_nearby_guns();

    let force_off = off.siege_force(&g, 0, &view, &plan, &toronto_group);
    let force_on = on.siege_force(&g, 0, &view, &plan, &toronto_group);
    assert!(guns.iter().all(|gun| !force_off.contains(gun)));
    assert!(guns.iter().all(|gun| force_on.contains(gun)));
    assert_eq!(off.breach_reading(&g, 0, &view, &force_off).guns, 0);
    let reading = on.breach_reading(&g, 0, &view, &force_on);
    assert_eq!(reading.guns, 3);
    assert!(reading.at_hand(&view));
    assert!(!off.conversion_siege_ready(&g, 0, toronto, &force_off));
    assert!(on.conversion_siege_ready(&g, 0, toronto, &force_on));
    // Uruk's own roster gives them up to the siege whose ring they stand at.
    let uruk_view = CityView::of(&g, uruk).unwrap();
    let uruk_on = on.siege_force(&g, 0, &uruk_view, &plan, &uruk_group);
    assert!(guns.iter().all(|gun| !uruk_on.contains(gun)));
    assert_eq!(off.siege_force(&g, 0, &uruk_view, &plan, &uruk_group), guns);
    // The assessment advances Toronto out of Stage under the gene only.
    off.assess_siege(&g, 0, toronto, &plan, &toronto_group);
    on.assess_siege(&g, 0, toronto, &plan, &toronto_group);
    assert_eq!(off.sieges[&toronto].stage, SiegeStage::Stage);
    assert_ne!(on.sieges[&toronto].stage, SiegeStage::Stage);
}

/// A gun beyond `STAGING_FAR` of the walls, or one too hurt to fire, is no
/// city's nearby breaker.
#[test]
fn a_far_or_unfit_gun_is_no_nearby_breaker() {
    let (mut g, toronto) = medieval_city();
    let uruk = second_walled_city(&mut g, toronto, 12);
    let far = g.spawn_unit("bombard", 0, at_distance(&g, toronto, STAGING_FAR + 1)[0]);
    let near = at_distance(&g, toronto, 3);
    let hurt = g.spawn_unit("bombard", 0, near[0]);
    g.units.get_mut(&hurt).unwrap().hp = 29;
    let fit = g.spawn_unit("bombard", 0, near[1]);
    let plan = plan_against(&g, toronto);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.enable_breach_counts_nearby_guns();
    ai.force_groups = vec![
        group_on(&g, toronto, &[fit]),
        group_on(&g, uruk, &[far, hurt]),
    ];
    assert_eq!(ai.nearby_gun_city(&g, 0, &plan, far), None);
    assert_eq!(ai.nearby_gun_city(&g, 0, &plan, hurt), None);
    assert_eq!(ai.nearby_gun_city(&g, 0, &plan, fit), Some(toronto));
    ai.disable_breach_counts_nearby_guns();
    assert_eq!(ai.nearby_gun_city(&g, 0, &plan, fit), None, "gene off");
}

/// One gun serves one city: the nearer of two walled sieges; at an equal
/// distance the one its own force is on, then the lower city id.
#[test]
fn one_gun_serves_the_nearer_siege_and_ties_go_to_its_own_then_the_lower_id() {
    let (mut g, first) = medieval_city();
    let second = second_walled_city(&mut g, first, 7);
    let gun = g.spawn_unit("bombard", 0, between(&g, first, second, 3, 4)[0]);
    let plan = plan_against(&g, first);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.enable_breach_counts_nearby_guns();
    let screen_first = g.spawn_unit("man_at_arms", 0, at_distance(&g, first, 2)[0]);
    let screen_second = g.spawn_unit("man_at_arms", 0, at_distance(&g, second, 2)[0]);
    let group_first = group_on(&g, first, &[screen_first]);
    let group_second = group_on(&g, second, &[screen_second, gun]);
    ai.force_groups = vec![group_first.clone(), group_second.clone()];
    assert_eq!(
        ai.nearby_gun_city(&g, 0, &plan, gun),
        Some(first),
        "the nearer city"
    );
    let (view_first, view_second) = (
        CityView::of(&g, first).unwrap(),
        CityView::of(&g, second).unwrap(),
    );
    assert!(ai
        .siege_force(&g, 0, &view_first, &plan, &group_first)
        .contains(&gun));
    assert!(!ai
        .siege_force(&g, 0, &view_second, &plan, &group_second)
        .contains(&gun));

    // Equidistant: its own force's city, then the lower id.
    let (mut g, first) = medieval_city();
    let second = second_walled_city(&mut g, first, 6);
    let tile = between(&g, first, second, 3, 3)[0];
    let gun = g.spawn_unit("bombard", 0, tile);
    let screen_first = g.spawn_unit("man_at_arms", 0, at_distance(&g, first, 2)[0]);
    let screen_second = g.spawn_unit("man_at_arms", 0, at_distance(&g, second, 2)[0]);
    let plan = plan_against(&g, first);
    ai.force_groups = vec![
        group_on(&g, first, &[screen_first]),
        group_on(&g, second, &[screen_second, gun]),
    ];
    assert_eq!(
        ai.nearby_gun_city(&g, 0, &plan, gun),
        Some(second),
        "its own force's city"
    );
    ai.force_groups = vec![
        group_on(&g, first, &[screen_first]),
        group_on(&g, second, &[screen_second]),
    ];
    assert_eq!(
        ai.nearby_gun_city(&g, 0, &plan, gun),
        Some(first.min(second)),
        "neither: the lower id"
    );
}

/// Under `breach-counts-nearby-guns` a gun the battle planner still holds
/// recovering counts as a breaker once back at `ROTATE_HP`; below it, it is
/// still a wounded gun. Off, any recovering gun is a wounded one.
#[test]
fn a_recovering_gun_at_rotate_hp_counts_as_a_breaker_under_the_gene() {
    let (mut g, cid) = medieval_city();
    let near = at_distance(&g, cid, 3);
    let gun = g.spawn_unit("bombard", 0, near[0]);
    g.units.get_mut(&gun).unwrap().hp = 60;
    let city = CityView::of(&g, cid).unwrap();
    let mut off = AdvancedAi::new();
    off.enable_siege_train();
    off.enable_siege_needs_a_breaker();
    off.battle_planner_recovering.insert(gun);
    let mut on = off.clone();
    on.enable_breach_counts_nearby_guns();
    let read_off = off.breach_reading(&g, 0, &city, &[gun]);
    assert_eq!((read_off.guns, read_off.wounded_guns), (0, 1));
    let read_on = on.breach_reading(&g, 0, &city, &[gun]);
    assert_eq!((read_on.guns, read_on.wounded_guns), (1, 0));
    assert!(read_on.at_hand(&city));
    // Still the battle planner's: counting is not posting.
    assert!(!on.siege_member_fit(&g, gun));
    g.units.get_mut(&gun).unwrap().hp = 45;
    let low = on.breach_reading(&g, 0, &city, &[gun]);
    assert_eq!((low.guns, low.wounded_guns), (0, 1), "below ROTATE_HP");
}

/// `medieval_city` with three Men-at-Arms staged three tiles out and a
/// Bomber based `distance` tiles from the city.
fn bombed_city(distance: i32) -> (Game, u32, Vec<u32>, u32) {
    let (mut g, cid) = medieval_city();
    let near = at_distance(&g, cid, 3);
    let melee: Vec<u32> = near
        .iter()
        .take(3)
        .map(|pos| g.spawn_unit("man_at_arms", 0, *pos))
        .collect();
    let bomber = g.spawn_unit("bomber", 0, at_distance(&g, cid, distance)[0]);
    (g, cid, melee, bomber)
}

/// Live King civvis-20261005T060002Z (game 104): Bombers took Sparta's walls
/// from 400 to 153 while the breach reading said "0 gun(s) fit ... nothing
/// to open the walls". Under `breach-reads-the-air` a Bomber in strike range
/// is wall damage a turn: the reading has a breach at hand and the damage
/// budget a finite count of turns. Off, neither changes.
#[test]
fn a_bomber_in_strike_range_is_wall_damage_under_the_gene() {
    let (g, cid, melee, bomber) = bombed_city(6);
    assert!(g.wdist(g.units[&bomber].pos, g.cities[&cid].pos) <= g.unit_attack_range(bomber));
    let city = CityView::of(&g, cid).unwrap();
    let mut off = AdvancedAi::new();
    off.enable_siege_train();
    off.enable_siege_needs_a_breaker();
    let mut on = off.clone();
    on.enable_breach_reads_the_air();

    assert_eq!(off.air_breach_walls(&g, 0, cid), 0.0);
    let expected = expected_damage(
        effective_strength(g.unit_ranged_attack_strength(&g.units[&bomber]), 100),
        g.city_strength(cid),
    );
    let air = on.air_breach_walls(&g, 0, cid);
    assert!(
        air > 0.0 && (air - expected).abs() < 1e-9,
        "{air} vs {expected}"
    );

    let mut read_off = off.breach_reading(&g, 0, &city, &melee);
    read_off.air_walls = off.air_breach_walls(&g, 0, cid);
    let mut read_on = on.breach_reading(&g, 0, &city, &melee);
    read_on.air_walls = on.air_breach_walls(&g, 0, cid);
    assert_eq!(
        read_off,
        off.breach_reading(&g, 0, &city, &melee),
        "off: the same reading"
    );
    assert!(!read_off.at_hand(&city));
    assert!(air * read_on.horizon >= f64::from(city.wall_hp));
    assert!(read_on.at_hand(&city), "{read_on:?}");

    let budget = |ai: &AdvancedAi| ai.conversion_siege_budget(&g, 0, cid, &melee).unwrap().0;
    assert!(budget(&off).is_infinite(), "off: no wall damage, no budget");
    assert!(
        budget(&on).is_finite(),
        "on: the Bomber brings the walls down"
    );
}

/// Under `siege-budget-counts-what-fires` the melee blow is the last one, not
/// one every turn, so a train of Men-at-Arms under a Bomber read its fire on
/// the city short of the city's heal and the budget infinite, though the
/// Bomber strikes every turn: live King game 181 read Rome "inf turns" for
/// twelve turns under 41-63 aircraft wall a turn. Under
/// `air-fire-counts-on-the-city` the Bomber's blows count on the city too.
#[test]
fn a_bomber_s_fire_counts_on_the_city_under_the_gene() {
    let (g, cid, melee, _) = bombed_city(6);
    let mut off = AdvancedAi::new();
    off.enable_siege_train();
    off.enable_siege_needs_a_breaker();
    off.enable_breach_reads_the_air();
    off.enable_siege_budget_counts_what_fires();
    let mut on = off.clone();
    on.enable_air_fire_counts_on_the_city();
    let air = on.air_breach_walls(&g, 0, cid);
    assert!(
        air > 20.0,
        "fixture: the Bomber out-fires the city's heal ({air:.1})"
    );
    let budget = |ai: &AdvancedAi| ai.conversion_siege_budget(&g, 0, cid, &melee).unwrap();
    assert!(
        budget(&off).0.is_infinite(),
        "off: no fire on the city but the last blow"
    );
    let (turns, endurance) = budget(&on);
    assert!(
        turns.is_finite() && turns > 1.0,
        "on: the Bomber brings the city down too ({turns:.1} turns / {endurance:.1})"
    );
    assert_eq!(
        budget(&off).1,
        endurance,
        "the air wing changes the fire, not the train's endurance"
    );
}

/// A Bomber beyond its strike range of the city counts for nothing.
#[test]
fn a_bomber_out_of_strike_range_is_no_breaker() {
    let (g, cid, _, bomber) = bombed_city(12);
    assert!(g.wdist(g.units[&bomber].pos, g.cities[&cid].pos) > g.unit_attack_range(bomber));
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.enable_breach_reads_the_air();
    assert_eq!(ai.air_breach_walls(&g, 0, cid), 0.0);
}

/// Walls the air wing has brought below half are a breach at hand, however
/// little a turn it adds now; above half, a weak wing is not, and below half
/// with no aircraft in range is no breach either.
#[test]
fn walls_the_air_has_brought_below_half_are_a_breach_at_hand() {
    let (mut g, cid, melee, _) = bombed_city(6);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    let read = |g: &Game, air: f64| {
        let city = CityView::of(g, cid).unwrap();
        let mut reading = ai.breach_reading(g, 0, &city, &melee);
        reading.air_walls = air;
        (reading.at_hand(&city), city)
    };
    g.cities.get_mut(&cid).unwrap().wall_hp = 150;
    let (high, city) = read(&g, 5.0);
    assert_eq!(city.wall_max, 200);
    const { assert!(5.0 * SHOOTER_BREACH_TURNS < 150.0) };
    assert!(!high, "above half, a weak wing");
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    assert!(read(&g, 5.0).0, "at half, the same wing");
    assert!(!read(&g, 0.0).0, "at half, no aircraft");
}

/// One sortie serves one siege: a Bomber in range of two walled cities we
/// besiege counts for the one the wing struck this frame, else the nearer.
/// Live G104: a Bomber struck Sparta every turn from 222 while Pharsalos, a
/// second siege, stood nearer it.
#[test]
fn a_bomber_in_range_of_two_sieges_counts_for_the_nearer() {
    let (mut g, first) = medieval_city();
    let second = second_walled_city(&mut g, first, 7);
    let tile = between(&g, first, second, 3, 4)[0];
    let bomber = g.spawn_unit("bomber", 0, tile);
    assert!(g.wdist(tile, g.cities[&second].pos) <= g.unit_attack_range(bomber));
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.enable_breach_reads_the_air();
    g.turn = 30;
    for cid in [first, second] {
        reducing(&mut ai, cid);
        ai.sieges.get_mut(&cid).unwrap().assessed = 30;
    }
    assert!(ai.air_breach_walls(&g, 0, first) > 0.0);
    assert_eq!(ai.air_breach_walls(&g, 0, second), 0.0);
    // The wing's volley this frame went to the farther city: it serves that.
    let far = g.cities[&second].pos;
    ai.air_city_assault = Some(crate::ai::advanced::AirCityAssault {
        target: far,
        cavalry: None,
        spot: far,
        moved_to_spot: false,
        aircraft: vec![bomber],
    });
    assert_eq!(ai.air_breach_walls(&g, 0, first), 0.0);
    assert!(ai.air_breach_walls(&g, 0, second) > 0.0);
    ai.air_city_assault = None;
    // With no siege at the nearer city it serves the other.
    ai.sieges.remove(&first);
    assert!(ai.air_breach_walls(&g, 0, second) > 0.0);
}
