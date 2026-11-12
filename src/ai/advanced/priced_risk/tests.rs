//! `risk-is-priced`: the reading, the price, the allowance by mission, and
//! the gene's reach into the pathed moves.
use super::*;
use crate::ai::advanced::battle_solver::dice::kill_probability;
use crate::ai::advanced::objective_board::TaskForce;
use crate::ai::advanced::siege_train::{Siege, SiegeStage};
use crate::ai::advanced::{ForceDomain, ForceGroup, GrandStrategy, VictoryTarget};
use crate::reasoning::Journal;

/// A grassland field at war: our city at (6, 10), nothing else of anyone's.
fn field(seed: u64) -> Game {
    let mut g = Game::new_full(2, 40, 24, seed, 500, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
        tile.river_edges = [false; 6];
        tile.riverside = false;
    }
    g.found_city_for(0, (6, 10), None);
    g.current = 0;
    g.at_war.insert((0, 1));
    g.players[0].explored.extend(g.map.tiles.keys().copied());
    g
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: 0,
        rush: false,
    }
}

/// Our healthy Warrior at (17, 10), a Babylonian Line Infantry at (20, 10)
/// seen by a second Warrior of ours at (22, 10), and Babylon's city at
/// (30, 10). The Line Infantry steps once and strikes: (18, 10) is in its
/// reach, (17, 10) is not, (19, 10) it strikes without moving, and its blow
/// on a Warrior always kills.
fn march() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = field(377_310);
    let warrior = g.spawn_test_unit("warrior", 0, (17, 10));
    let hostile = g.spawn_test_unit("line_infantry", 1, (20, 10));
    g.spawn_test_unit("warrior", 0, (22, 10));
    let city = g.found_city_for(1, (30, 10), None);
    assert!(g.unit_visible_to(hostile, 0));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_risk_is_priced();
    (g, ai, warrior, hostile, city)
}

fn group(units: Vec<u32>, objective: Pos, posture: ForcePosture) -> ForceGroup {
    ForceGroup {
        id: units[0],
        domain: ForceDomain::Land,
        units,
        anchor: objective,
        objective,
        focus_target: None,
        posture,
        readiness: 1.0,
        local_strength_ratio: 1.0,
    }
}

fn siege(stage: SiegeStage, turn: u32) -> Siege {
    Siege {
        stage,
        taker: None,
        entered: turn,
        assessed: turn,
        posts: Default::default(),
        short_since: None,
    }
}

fn risk_of(ai: &AdvancedAi, uid: u32) -> &UnitRisk {
    ai.base
        .priced_risk
        .get(&uid)
        .expect("the frame read this unit")
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    crate::ai::advanced::test_support::opt_in_off_in_both_controllers("risk-is-priced", |ai| {
        ai.risk_is_priced
    });
}

/// With every blow certain the reading is the battle solver's own volley
/// kill chance; below that it is the mixture over which sources land.
#[test]
fn the_death_chance_is_the_dice_kill_probability() {
    let a = (40.0_f32, 30.0_f32);
    let b = (36.0_f32, 30.0_f32);
    let with = |blow: (f32, f32), p: f64| (blow.0, blow.1, p);
    for hp in [100, 70, 45, 12] {
        let one = kill_probability(hp, &[a]);
        let both = kill_probability(hp, &[a, b]);
        assert!(
            (death_chance(hp, 0.0, &[with(a, 1.0)]) - one).abs() < 1e-12,
            "{hp}"
        );
        assert!(
            (death_chance(hp, 0.0, &[with(a, 1.0), with(b, 1.0)]) - both).abs() < 1e-12,
            "{hp}"
        );
        let (p, q) = (0.43, 0.11);
        assert!(
            (death_chance(hp, 0.0, &[with(a, p)]) - p * one).abs() < 1e-12,
            "{hp}"
        );
        // Each subset of sources, weighted by its chance.
        let mixture =
            p * (1.0 - q) * one + (1.0 - p) * q * kill_probability(hp, &[b]) + p * q * both;
        let read = death_chance(hp, 0.0, &[with(a, p), with(b, q)]);
        assert!(
            (read - mixture).abs() < 1e-12,
            "{hp}: {read} against {mixture}"
        );
    }
    // A known hazard lands first, at its planning damage.
    assert!((death_chance(100, 30.0, &[with(a, 1.0)]) - kill_probability(70, &[a])).abs() < 1e-12);
    assert_eq!(death_chance(40, 75.0, &[]), 1.0);
    assert_eq!(death_chance(100, 0.0, &[]), 0.0);
}

/// The host's odds that the Line Infantry lands its blow on `uid` at `tile`
/// on the march board, read from the fit directly: it strikes at all, and it
/// picks `uid` over the other Warrior (at (22, 10), two tiles off).
fn host_odds(g: &Game, hostile: u32, uid: u32, tile: Pos) -> (f64, f64, f64) {
    let mut probe = g.clone();
    probe.relocate(uid, tile);
    let (att, def) = probe
        .melee_exchange_strengths(hostile, uid)
        .expect("the pair");
    let other = g
        .unit_ids_at((22, 10))
        .first()
        .copied()
        .expect("the second Warrior");
    let (att2, def2) = g
        .melee_exchange_strengths(hostile, other)
        .expect("its pair");
    let distance = g.wdist((20, 10), tile);
    let mine = target_utility(&TargetFeatures {
        hp: 100,
        dealt: expected_damage(att, def) as f32,
        shooter: false,
        distance: distance as u8,
        still: distance <= 1,
    });
    let theirs = target_utility(&TargetFeatures {
        hp: 100,
        dealt: expected_damage(att2, def2) as f32,
        shooter: false,
        distance: 2,
        still: false,
    });
    let strike = strike_probability(&StrikeFeatures {
        class: HostClass::Melee,
        owner: Owner::Major,
        hp: 100,
        home: false,
        edge: ((att - def) as f32).max((att2 - def2) as f32),
        targets: 2,
        still: distance <= 1,
    });
    let p = f64::from(strike) / (1.0 + f64::from(theirs - mine).exp());
    (p, att, def)
}

/// On the board the reading is the dice over the engine's own pair, read with
/// the unit standing on the tile, at the host's fitted odds: stepping beside
/// the Line Infantry (it strikes without moving, and picks the unit in front
/// of it) reads about 0.39; two tiles off (it must move, and has two Warriors
/// to choose from) about 0.05. The battle planner's field reads the same
/// pairs at the centre of the roll.
#[test]
fn the_board_reading_is_the_kill_probability_of_the_engines_pair_at_the_hosts_odds() {
    let (g, mut ai, warrior, hostile, _) = march();
    ai.mark_priced_risk(&g, 0, &plan());
    let risk = risk_of(&ai, warrior);
    for (tile, low, high) in [((19, 10), 0.3, 0.5), ((18, 10), 0.02, MARCH_ALLOWANCE)] {
        let (p, att, def) = host_odds(&g, hostile, warrior, tile);
        let expected = p * kill_probability(100, &[(att as f32, def as f32)]);
        let read = risk.death_at(&g, warrior, tile);
        assert!(
            (read - expected).abs() < 1e-6,
            "{tile:?}: {read} against {expected}"
        );
        assert!(read > low && read < high, "{tile:?}: {read}");
    }
    assert_eq!(risk.death_at(&g, warrior, (17, 10)), 0.0);
    assert_eq!(risk.death_at(&g, warrior, (16, 10)), 0.0);

    let (att, def) = {
        let mut probe = g.clone();
        probe.relocate(warrior, (18, 10));
        probe
            .melee_exchange_strengths(hostile, warrior)
            .expect("the pair")
    };
    let mut field = DangerField::with_reach(&g, 0, true);
    let pairs = field.blow_pairs_at_hp((18, 10), warrior, 100);
    let centre: Vec<(Option<u32>, f64)> = pairs
        .iter()
        .map(|(source, pair)| (*source, pair.expected()))
        .collect();
    assert_eq!(*field.contributions((18, 10), warrior), centre);
    assert_eq!(
        pairs,
        [(Some(hostile), BlowPair::Strike { att, def })],
        "one strike, the engine's pair"
    );
}

/// A walled enemy city strikes a unit within two tiles of it at the city
/// rate, shared over our units there.
#[test]
fn a_walled_city_strikes_at_the_city_rate_shared() {
    let mut g = field(377_310);
    let enemy = g.found_city_for(1, (24, 10), None);
    g.cities.get_mut(&enemy).unwrap().wall_hp = 100;
    let warrior = g.spawn_test_unit("warrior", 0, (21, 10));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_risk_is_priced();
    ai.mark_priced_risk(&g, 0, &plan());
    let read = |ai: &AdvancedAi| {
        let tile = &risk_of(ai, warrior).tiles[&(22, 10)];
        let blow = tile
            .blows
            .iter()
            .find(|blow| blow.source == Source::City(enemy))
            .expect("the city's strike");
        blow.p
    };
    assert!((read(&ai) - CITY_STRIKE_P).abs() < 1e-12);
    // A second unit of ours within two of the city halves it.
    g.spawn_test_unit("warrior", 0, (23, 11));
    ai.mark_priced_risk(&g, 0, &plan());
    assert!((read(&ai) - CITY_STRIKE_P / 2.0).abs() < 1e-12);
}

/// The price: cost at the hit points, a premium per promotion, and a share
/// of that against a much stronger enemy.
#[test]
fn the_price_is_cost_at_health_with_promotions_and_obsolescence() {
    let mut g = field(377_311);
    let warrior = g.spawn_test_unit("warrior", 0, (16, 10));
    assert_eq!(unit_price(&g, warrior, 20.0), 40.0);
    g.units.get_mut(&warrior).unwrap().hp = 50;
    assert_eq!(unit_price(&g, warrior, 20.0), 20.0);
    g.units
        .get_mut(&warrior)
        .unwrap()
        .promotions
        .insert(crate::name!("battlecry"));
    assert_eq!(
        unit_price(&g, warrior, 20.0),
        20.0 + PROMOTION_PREMIUM * 40.0
    );
    // Against a Line Infantry (65) a Warrior (20) is obsolete.
    assert_eq!(
        unit_price(&g, warrior, 65.0),
        (20.0 + PROMOTION_PREMIUM * 40.0) * OBSOLETE_PRICE_SHARE
    );
    // Well within the gap it is not.
    assert_eq!(
        unit_price(&g, warrior, 20.0 + OBSOLETE_STRENGTH_GAP - 5.0),
        20.0 + PROMOTION_PREMIUM * 40.0
    );
}

/// A healthy Warrior of a siege still in Stage marches on the march
/// allowance: two tiles from the Line Infantry (about 0.05) it walks on, the
/// step beside it (about 0.39) is refused, with a journal line.
#[test]
fn a_healthy_warrior_will_not_march_beside_a_line_infantry_while_staging() {
    let (mut g, mut ai, warrior, _, city) = march();
    ai.enable_siege_train();
    let target = g.cities[&city].pos;
    ai.force_groups = vec![group(vec![warrior], target, ForcePosture::Muster)];
    ai.sieges.insert(city, siege(SiegeStage::Stage, g.turn));
    assert!(ai.staging_siege_member(&g, 0, warrior));
    let journal = Journal::recording();
    ai.base.journal = journal.handle();
    ai.mark_priced_risk(&g, 0, &plan());
    let risk = risk_of(&ai, warrior);
    assert_eq!(risk.mission, Mission::Staging);
    assert_eq!(risk.allowance, MARCH_ALLOWANCE);
    let beside = risk.death_at(&g, warrior, (19, 10));
    assert!(beside > MARCH_ALLOWANCE, "{beside}");
    assert!(ai.base.path_move(&mut g, 0, warrior, (18, 10)));
    assert!(
        !ai.base.path_move(&mut g, 0, warrior, (19, 10)),
        "the step beside the Line Infantry is refused"
    );
    assert_eq!(g.units[&warrior].pos, (18, 10));
    let thoughts = journal.since(0).thoughts;
    let line = thoughts
        .iter()
        .find(|thought| thought.detail.contains("risk-is-priced"))
        .expect("the refusal is journaled");
    assert!(line.headline.contains("(19, 10)"), "{}", line.headline);
    assert!(
        line.detail.contains("staging allowance 0.10"),
        "{}",
        line.detail
    );

    // The gene off: nothing is read and the same steps are taken.
    let (mut g, mut ai, warrior, _, city) = march();
    ai.disable_risk_is_priced();
    ai.enable_siege_train();
    let target = g.cities[&city].pos;
    ai.force_groups = vec![group(vec![warrior], target, ForcePosture::Muster)];
    ai.sieges.insert(city, siege(SiegeStage::Stage, g.turn));
    ai.mark_priced_risk(&g, 0, &plan());
    assert!(ai.base.priced_risk.is_empty());
    assert_eq!(ai.base.priced_risk_cost(&g, warrior, (19, 10), 1.0), 0.0);
    assert!(ai.base.path_move(&mut g, 0, warrior, (18, 10)));
    assert!(ai.base.path_move(&mut g, 0, warrior, (19, 10)));
}

/// The same Warrior, the same tile, the same death chance: as a member of a
/// Defend row it closes on the attacker. And without a board, a Warrior
/// standing by a city of ours the Line Infantry stands at is defending it
/// and closes too.
#[test]
fn a_defend_member_closes_on_the_attacker_beside_its_city_at_the_same_risk() {
    let (mut g, mut ai, warrior, _, _) = march();
    let home = g.city_at((6, 10)).expect("our city");
    ai.objective_board_state.forces.push(TaskForce {
        id: 1,
        objective_key: ObjectiveKey::Defend(home),
        domain: ForceDomain::Land,
        units: vec![warrior],
        rally: (6, 10),
        doctrine_state: ForcePosture::Hold,
        aimed_at: (6, 10),
        formed: g.turn,
    });
    ai.mark_priced_risk(&g, 0, &plan());
    let risk = risk_of(&ai, warrior);
    assert_eq!(risk.mission, Mission::Defend);
    let beside = risk.death_at(&g, warrior, (19, 10));
    assert!(
        beside > MARCH_ALLOWANCE && beside <= HOLD_ALLOWANCE,
        "{beside}"
    );
    assert!(ai.base.path_move(&mut g, 0, warrior, (18, 10)));
    assert!(
        ai.base.path_move(&mut g, 0, warrior, (19, 10)),
        "a Defend member buys the risk"
    );

    // No board: a city of ours three tiles from the Line Infantry, the
    // Warrior beside it.
    let mut g = field(377_312);
    let hostile = g.spawn_test_unit("line_infantry", 1, (20, 10));
    g.spawn_test_unit("warrior", 0, (22, 10));
    let near = g.found_city_for(0, (17, 10), None);
    assert_eq!(g.wdist(g.cities[&near].pos, (20, 10)), HOME_THREAT_RADIUS);
    let warrior = g.spawn_test_unit("warrior", 0, (18, 10));
    assert!(g.unit_visible_to(hostile, 0));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_risk_is_priced();
    ai.mark_priced_risk(&g, 0, &plan());
    let risk = risk_of(&ai, warrior);
    assert_eq!(risk.mission, Mission::Defend);
    let beside = risk.death_at(&g, warrior, (19, 10));
    assert!(
        beside > MARCH_ALLOWANCE && beside <= HOLD_ALLOWANCE,
        "{beside}"
    );
    assert!(ai.base.path_move(&mut g, 0, warrior, (19, 10)));
}

/// A catapult at its post in an active siege, beside a Line Infantry, reads
/// about 0.36 and keeps its post; the same gun while its siege is still in
/// Stage steps out to a tile within the march allowance. Version one of
/// `guns-stay-out-of-reach` (armed live) steps it out either way, and with
/// both genes on version one leaves it to this one.
#[test]
fn an_active_siege_shooter_keeps_its_post() {
    let post = |stage: SiegeStage, priced: bool, v1: bool| -> (Game, AdvancedAi, u32) {
        let mut g = field(377_313);
        let gun = g.spawn_test_unit("catapult", 0, (19, 10));
        let hostile = g.spawn_test_unit("line_infantry", 1, (20, 10));
        assert!(g.unit_visible_to(hostile, 0));
        let city = g.found_city_for(1, (30, 16), None);
        g.cities.get_mut(&city).unwrap().wall_hp = 0;
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_siege_train();
        if priced {
            ai.enable_risk_is_priced();
        }
        if v1 {
            ai.enable_guns_stay_out_of_reach();
        }
        let target = g.cities[&city].pos;
        ai.force_groups = vec![group(vec![gun], target, ForcePosture::Engage)];
        ai.sieges.insert(city, siege(stage, g.turn));
        // The unit pass's order (`advanced_units`).
        ai.mark_priced_risk(&g, 0, &plan());
        ai.mark_gun_lethal_tiles(&g, 0);
        ai.guns_step_out_of_reach(&mut g, 0);
        ai.priced_risk_guns_step_out(&mut g, 0);
        (g, ai, gun)
    };
    let (g, ai, gun) = post(SiegeStage::Reduce, true, false);
    assert!(ai.active_siege_member(&g, 0, gun));
    let risk = risk_of(&ai, gun);
    assert_eq!(risk.mission, Mission::SiegePost);
    let here = risk.death_at(&g, gun, (19, 10));
    assert!(here > MARCH_ALLOWANCE && here <= HOLD_ALLOWANCE, "{here}");
    assert_eq!(g.units[&gun].pos, (19, 10), "the post is kept");

    let (g, ai, gun) = post(SiegeStage::Stage, true, false);
    assert_eq!(risk_of(&ai, gun).mission, Mission::Staging);
    let pos = g.units[&gun].pos;
    assert_ne!(pos, (19, 10), "a staging gun steps out");
    assert!(risk_of(&ai, gun).death_at(&g, gun, pos) <= MARCH_ALLOWANCE);

    // Version one alone steps the active post out; with this gene on it
    // marks nothing for the gun and the post is kept.
    let (g, _, gun) = post(SiegeStage::Reduce, false, true);
    assert_ne!(g.units[&gun].pos, (19, 10));
    let (g, ai, gun) = post(SiegeStage::Reduce, true, true);
    assert!(ai.base.gun_lethal_tiles.is_empty());
    assert_eq!(g.units[&gun].pos, (19, 10));
}

/// A hostile killed later in the frame stops counting; a unit the battle
/// planner ordered is not read at all.
#[test]
fn a_dead_hostile_and_a_planned_unit_are_left_out() {
    let (mut g, mut ai, warrior, hostile, _) = march();
    ai.mark_priced_risk(&g, 0, &plan());
    assert_eq!(risk_of(&ai, warrior).mission, Mission::March);
    assert!(risk_of(&ai, warrior).death_at(&g, warrior, (19, 10)) > MARCH_ALLOWANCE);
    g.remove_unit(hostile);
    assert_eq!(risk_of(&ai, warrior).death_at(&g, warrior, (19, 10)), 0.0);
    assert!(ai.base.path_move(&mut g, 0, warrior, (18, 10)));
    assert!(ai.base.path_move(&mut g, 0, warrior, (19, 10)));

    let (g, mut ai, warrior, _, _) = march();
    ai.battle_planner_ordered.insert(warrior);
    ai.mark_priced_risk(&g, 0, &plan());
    assert!(!ai.base.priced_risk.contains_key(&warrior));
}

/// A unit already in danger may step to a tile that lowers its death chance,
/// never to one that does not; a source gone since the frame was read stops
/// counting. Read on a hand-built frame: two certain kills on one tile, one
/// on the next, each landing half the time.
#[test]
fn a_move_that_lowers_the_death_chance_is_allowed_over_the_allowance() {
    let mut g = field(377_310);
    let warrior = g.spawn_test_unit("warrior", 0, (18, 10));
    let a = g.spawn_test_unit("line_infantry", 1, (20, 10));
    let b = g.spawn_test_unit("line_infantry", 1, (26, 14));
    let kill = |source: u32| Blow {
        source: Source::Unit(source),
        att: 65.0,
        def_base: 20.0,
        p: 0.5,
    };
    let tile = |blows: Vec<Blow>| {
        let volley: Vec<(f32, f32, f64)> = blows.iter().map(|b| (b.att, b.def_base, b.p)).collect();
        TileRisk {
            hazard: 0.0,
            death: death_chance(100, 0.0, &volley),
            blows,
        }
    };
    let risk = UnitRisk {
        mission: Mission::March,
        allowance: MARCH_ALLOWANCE,
        price: 40.0,
        hp_equivalent: 100.0,
        hp: 100,
        tiles: [
            ((18, 10), tile(vec![kill(a), kill(b)])),
            ((17, 10), tile(vec![kill(a)])),
        ]
        .into_iter()
        .collect(),
    };
    let both = risk.death_at(&g, warrior, (18, 10));
    let one = risk.death_at(&g, warrior, (17, 10));
    assert!((both - 0.75).abs() < 1e-9, "{both}");
    assert!((one - 0.5).abs() < 1e-9, "{one}");
    assert!(
        risk.refusal(&g, warrior, (18, 10), (17, 10)).is_none(),
        "over the allowance, but lower than where it stands"
    );
    let refused = risk
        .refusal(&g, warrior, (17, 10), (18, 10))
        .expect("higher than where it stands");
    assert_eq!((refused.death, refused.here), (both, one));
    assert!(risk.refusal(&g, warrior, (16, 10), (17, 10)).is_some());
    // The second Line Infantry falls: the tile reads one blow, and a step
    // between two equal readings over the allowance is refused.
    g.remove_unit(b);
    assert!((risk.death_at(&g, warrior, (18, 10)) - 0.5).abs() < 1e-9);
    assert!(risk.refusal(&g, warrior, (17, 10), (18, 10)).is_some());
}

/// A wound since the frame was read is re-read at the unit's health: one
/// Swordsman's blow never kills a healthy Spearman and always kills one at
/// forty, so the reading goes from nothing to the Swordsman's odds.
#[test]
fn a_wound_since_the_frame_is_re_read() {
    let mut g = field(377_310);
    let spear = g.spawn_test_unit("spearman", 0, (16, 10));
    g.spawn_test_unit("swordsman", 1, (20, 10));
    g.spawn_test_unit("warrior", 0, (22, 10));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_risk_is_priced();
    ai.mark_priced_risk(&g, 0, &plan());
    let risk = risk_of(&ai, spear).clone();
    assert_eq!(risk.death_at(&g, spear, (18, 10)), 0.0);
    g.units.get_mut(&spear).unwrap().hp = 40;
    let wounded = risk.death_at(&g, spear, (18, 10));
    let odds = risk.tiles[&(18, 10)].blows[0].p;
    assert!(
        wounded > 0.0 && (wounded - odds).abs() < 1e-9,
        "{wounded} against {odds}"
    );
}

/// Without a board or a siege train: an engaged unit on the ring of the
/// enemy city its group is aimed at holds a siege post, the same group's
/// unit further out is engaged; a unit within the relief radius of a
/// contested city of ours is relieving it, one beyond it is marching.
#[test]
fn the_assault_ring_and_the_relief_radius_buy_the_hold_allowance() {
    let mut g = field(377_310);
    let enemy = g.found_city_for(1, (26, 10), None);
    let target = g.cities[&enemy].pos;
    let ring = g.spawn_test_unit("swordsman", 0, (24, 10));
    let back = g.spawn_test_unit("swordsman", 0, (20, 10));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_risk_is_priced();
    ai.force_groups = vec![group(vec![back, ring], target, ForcePosture::Engage)];
    let mission = |ai: &AdvancedAi, g: &Game, uid: u32, contested: &[Pos]| {
        ai.priced_risk_mission(g, 0, uid, &plan(), contested)
    };
    assert_eq!(mission(&ai, &g, ring, &[]), Mission::SiegePost);
    assert_eq!(mission(&ai, &g, back, &[]), Mission::Engaged);

    ai.force_groups.clear();
    let mut g = field(377_310);
    let home = g.found_city_for(0, (17, 10), None);
    let at = g.cities[&home].pos;
    let near = g.spawn_test_unit("warrior", 0, (12, 10));
    let far = g.spawn_test_unit("warrior", 0, (9, 10));
    assert_eq!(g.wdist(at, (12, 10)), 5);
    assert_eq!(g.wdist(at, (9, 10)), 8);
    assert_eq!(mission(&ai, &g, near, &[at]), Mission::Relieve);
    assert_eq!(mission(&ai, &g, far, &[at]), Mission::March);
}
