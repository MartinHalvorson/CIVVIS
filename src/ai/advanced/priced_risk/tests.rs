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

/// Our healthy Warrior at (16, 10), a Babylonian Line Infantry at (20, 10)
/// seen by a second Warrior of ours at (22, 10), and Babylon's city at
/// (30, 10). The Line Infantry steps once and strikes: (18, 10) is in its
/// reach, (17, 10) is not, and its blow on a Warrior always kills.
fn march() -> (Game, AdvancedAi, u32, u32, u32) {
    let mut g = field(377_310);
    let warrior = g.spawn_test_unit("warrior", 0, (16, 10));
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

/// At a strike chance of one the reading is the battle solver's own volley
/// kill chance; below one it is the mixture over which hostiles strike.
#[test]
fn the_death_chance_is_the_dice_kill_probability() {
    let a = (40.0_f32, 30.0_f32);
    let b = (36.0_f32, 30.0_f32);
    for hp in [100, 70, 45, 12] {
        let one = kill_probability(hp, &[a]);
        let both = kill_probability(hp, &[a, b]);
        assert!(
            (death_chance(hp, 0.0, &[a], 1.0) - one).abs() < 1e-12,
            "{hp}"
        );
        assert!(
            (death_chance(hp, 0.0, &[a, b], 1.0) - both).abs() < 1e-12,
            "{hp}"
        );
        let p = HOST_STRIKE_P;
        assert!(
            (death_chance(hp, 0.0, &[a], p) - p * one).abs() < 1e-12,
            "{hp}"
        );
        // Each subset of strikers, weighted by its chance.
        let mixture = p * (1.0 - p) * (one + kill_probability(hp, &[b])) + p * p * both;
        assert!(
            (death_chance(hp, 0.0, &[a, b], p) - mixture).abs() < 1e-12,
            "{hp}: {} against {mixture}",
            death_chance(hp, 0.0, &[a, b], p)
        );
    }
    // A known hazard lands first, at its planning damage.
    assert!((death_chance(100, 30.0, &[a], 1.0) - kill_probability(70, &[a])).abs() < 1e-12);
    assert_eq!(death_chance(40, 75.0, &[], HOST_STRIKE_P), 1.0);
    assert_eq!(death_chance(100, 0.0, &[], HOST_STRIKE_P), 0.0);
}

/// On the board the reading is the dice over the engine's own pair, read with
/// the unit standing on the tile: half the Line Infantry's certain kill. The
/// battle planner's field reads the same pairs at the centre of the roll.
#[test]
fn the_board_reading_is_the_kill_probability_of_the_engines_pair() {
    let (g, mut ai, warrior, hostile, _) = march();
    ai.mark_priced_risk(&g, 0, &plan());
    let mut probe = g.clone();
    probe.relocate(warrior, (18, 10));
    let (att, def) = probe
        .melee_exchange_strengths(hostile, warrior)
        .expect("the pair");
    let expected = HOST_STRIKE_P * kill_probability(100, &[(att as f32, def as f32)]);
    let risk = risk_of(&ai, warrior);
    let read = risk.death_at(&g, warrior, (18, 10));
    assert!((read - expected).abs() < 1e-9, "{read} against {expected}");
    assert!(
        (read - 0.5).abs() < 1e-9,
        "a Line Infantry always kills: {read}"
    );
    assert_eq!(risk.death_at(&g, warrior, (17, 10)), 0.0);
    assert_eq!(risk.death_at(&g, warrior, (16, 10)), 0.0);

    let mut field = DangerField::with_reach(&g, 0, true);
    let pairs = field.blow_pairs_at_hp((18, 10), warrior, 100);
    let centre: Vec<(Option<u32>, f64)> = pairs
        .iter()
        .map(|(source, pair)| (*source, pair.expected()))
        .collect();
    assert_eq!(*field.contributions((18, 10), warrior), centre);
    assert_eq!(
        pairs,
        vec![(Some(hostile), BlowPair::Strike { att, def })],
        "one strike, the engine's pair"
    );
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
/// allowance: the step beside the Line Infantry (half a death) is refused,
/// with a journal line, and the step short of its reach is taken.
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
    assert!((risk.death_at(&g, warrior, (18, 10)) - 0.5).abs() < 1e-9);
    assert!(ai.base.path_move(&mut g, 0, warrior, (17, 10)));
    assert!(
        !ai.base.path_move(&mut g, 0, warrior, (18, 10)),
        "the step beside the Line Infantry is refused"
    );
    assert_eq!(g.units[&warrior].pos, (17, 10));
    let thoughts = journal.since(0).thoughts;
    let line = thoughts
        .iter()
        .find(|thought| thought.detail.contains("risk-is-priced"))
        .expect("the refusal is journaled");
    assert!(line.headline.contains("(18, 10)"), "{}", line.headline);
    assert!(line.headline.contains("50%"), "{}", line.headline);
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
    assert_eq!(ai.base.priced_risk_cost(&g, warrior, (18, 10), 1.0), 0.0);
    assert!(ai.base.path_move(&mut g, 0, warrior, (17, 10)));
    assert!(ai.base.path_move(&mut g, 0, warrior, (18, 10)));
}

/// The same Warrior, the same tile, the same half a death: as a member of a
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
    assert!((risk.death_at(&g, warrior, (18, 10)) - 0.5).abs() < 1e-9);
    assert!(ai.base.path_move(&mut g, 0, warrior, (17, 10)));
    assert!(
        ai.base.path_move(&mut g, 0, warrior, (18, 10)),
        "a Defend member buys the risk"
    );

    // No board: a city of ours three tiles from the Line Infantry, the
    // Warrior on it.
    let mut g = field(377_312);
    let hostile = g.spawn_test_unit("line_infantry", 1, (20, 10));
    g.spawn_test_unit("warrior", 0, (22, 10));
    let near = g.found_city_for(0, (17, 10), None);
    assert_eq!(g.wdist(g.cities[&near].pos, (20, 10)), HOME_THREAT_RADIUS);
    let warrior = g.spawn_test_unit("warrior", 0, (17, 10));
    assert!(g.unit_visible_to(hostile, 0));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_risk_is_priced();
    ai.mark_priced_risk(&g, 0, &plan());
    let risk = risk_of(&ai, warrior);
    assert_eq!(risk.mission, Mission::Defend);
    let beside = risk.death_at(&g, warrior, (18, 10));
    assert!(
        beside > MARCH_ALLOWANCE && beside <= HOLD_ALLOWANCE,
        "{beside}"
    );
    assert!(ai.base.path_move(&mut g, 0, warrior, (18, 10)));
}

/// A catapult at its post in an active siege, one tile out of a Line
/// Infantry's step-and-blow, reads half a death and keeps its post; the same
/// gun while its siege is still in Stage steps out. Version one of
/// `guns-stay-out-of-reach` (armed live) steps it out either way, and with
/// both genes on version one leaves it to this one.
#[test]
fn an_active_siege_shooter_keeps_its_post() {
    let post = |stage: SiegeStage, priced: bool, v1: bool| -> (Game, AdvancedAi, u32) {
        let mut g = field(377_313);
        let gun = g.spawn_test_unit("catapult", 0, (18, 10));
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
        ai.mark_gun_lethal_tiles(&g, 0);
        ai.guns_step_out_of_reach(&mut g, 0);
        ai.mark_priced_risk(&g, 0, &plan());
        ai.priced_risk_guns_step_out(&mut g, 0);
        (g, ai, gun)
    };
    let (g, ai, gun) = post(SiegeStage::Reduce, true, false);
    assert!(ai.active_siege_member(&g, 0, gun));
    let risk = risk_of(&ai, gun);
    assert_eq!(risk.mission, Mission::SiegePost);
    let here = risk.death_at(&g, gun, (18, 10));
    assert!(here > MARCH_ALLOWANCE && here <= HOLD_ALLOWANCE, "{here}");
    assert_eq!(g.units[&gun].pos, (18, 10), "the post is kept");

    let (g, ai, gun) = post(SiegeStage::Stage, true, false);
    assert_eq!(risk_of(&ai, gun).mission, Mission::Staging);
    let pos = g.units[&gun].pos;
    assert_ne!(pos, (18, 10), "a staging gun steps out");
    assert!(risk_of(&ai, gun).death_at(&g, gun, pos) <= MARCH_ALLOWANCE);

    // Version one alone steps the active post out; with this gene on it
    // marks nothing for the gun and the post is kept.
    let (g, _, gun) = post(SiegeStage::Reduce, false, true);
    assert_ne!(g.units[&gun].pos, (18, 10));
    let (g, ai, gun) = post(SiegeStage::Reduce, true, true);
    assert!(ai.base.gun_lethal_tiles.is_empty());
    assert_eq!(g.units[&gun].pos, (18, 10));
}

/// A hostile killed later in the frame stops counting; a unit the battle
/// planner ordered is not read at all.
#[test]
fn a_dead_hostile_and_a_planned_unit_are_left_out() {
    let (mut g, mut ai, warrior, hostile, _) = march();
    ai.mark_priced_risk(&g, 0, &plan());
    assert_eq!(risk_of(&ai, warrior).mission, Mission::March);
    assert!((risk_of(&ai, warrior).death_at(&g, warrior, (18, 10)) - 0.5).abs() < 1e-9);
    g.remove_unit(hostile);
    assert_eq!(risk_of(&ai, warrior).death_at(&g, warrior, (18, 10)), 0.0);
    assert!(ai.base.path_move(&mut g, 0, warrior, (17, 10)));
    assert!(ai.base.path_move(&mut g, 0, warrior, (18, 10)));

    let (g, mut ai, warrior, _, _) = march();
    ai.battle_planner_ordered.insert(warrior);
    ai.mark_priced_risk(&g, 0, &plan());
    assert!(!ai.base.priced_risk.contains_key(&warrior));
}

/// A unit already in danger may step to a tile that lowers its death chance,
/// never to one that does not; a wound since the frame was read is re-read at
/// the unit's health.
#[test]
fn a_unit_in_danger_may_step_to_less_and_a_wound_is_re_read() {
    // Two Line Infantry two tiles either side: both reach (18, 10), only the
    // western one reaches (17, 10).
    let mut g = field(377_315);
    let warrior = g.spawn_test_unit("warrior", 0, (18, 10));
    g.spawn_test_unit("line_infantry", 1, (20, 10));
    g.spawn_test_unit("line_infantry", 1, (16, 10));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_risk_is_priced();
    ai.mark_priced_risk(&g, 0, &plan());
    let risk = risk_of(&ai, warrior).clone();
    let both = risk.death_at(&g, warrior, (18, 10));
    let one = risk.death_at(&g, warrior, (17, 10));
    assert!((both - 0.75).abs() < 1e-9, "{both}");
    assert!((one - 0.5).abs() < 1e-9, "{one}");
    assert!(
        risk.refusal(&g, warrior, (18, 10), (17, 10)).is_none(),
        "over the allowance, but lower than where it stands"
    );
    assert!(risk.refusal(&g, warrior, (17, 10), (18, 10)).is_some());

    // Two Swordsmen beside a Spearman: no single blow kills it healthy,
    // two can; wounded, the same two are likelier to.
    let mut g = field(377_314);
    let spear = g.spawn_test_unit("spearman", 0, (18, 10));
    g.spawn_test_unit("swordsman", 1, (20, 10));
    g.spawn_test_unit("swordsman", 1, (16, 10));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_risk_is_priced();
    ai.mark_priced_risk(&g, 0, &plan());
    let risk = risk_of(&ai, spear).clone();
    let healthy = risk.death_at(&g, spear, (18, 10));
    g.units.get_mut(&spear).unwrap().hp = 60;
    let wounded = risk.death_at(&g, spear, (18, 10));
    assert!(wounded > healthy, "{healthy} -> {wounded}");
}
