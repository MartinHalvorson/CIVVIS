use super::*;
use crate::ai::advanced::{GrandStrategy, VictoryTarget};

/// Two majors at war on open grassland, one city each, nothing else.
fn board() -> Game {
    let mut g = Game::new_full(2, 40, 24, 936302, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
    g.found_city_for(0, (10, 12), None);
    g.found_city_for(1, (26, 12), None);
    g.record_contact(0, 1);
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    g.current = 0;
    g.turn = 180;
    g
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

fn agent() -> AdvancedAi {
    AdvancedAi::targeting(VictoryTarget::Domination)
}

/// ⭐ Each claimant, posed on its own: `owner_of` names it, and a unit no
/// claim holds has no owner.
#[test]
fn owner_of_names_each_claim_on_a_posed_board() {
    let mut g = board();
    let enemy_city = g.city_at((26, 12)).expect("the enemy city");
    let our_city = g.city_at((10, 12)).expect("our city");
    let unit = |g: &mut Game, at: Pos| g.spawn_test_unit("warrior", 0, at);
    let mut checks: Vec<(Owner, AdvancedAi, u32)> = Vec::new();

    let uid = unit(&mut g, (14, 4));
    let mut ai = agent();
    ai.reserved_units.insert(uid);
    checks.push((Owner::SiegeTaker, ai, uid));

    let uid = unit(&mut g, (14, 5));
    let mut ai = agent();
    ai.amphibious_siege_train = true;
    ai.amphibious_train = Some(amphibious_siege_train::AmphibiousTrain {
        target: enemy_city,
        target_pos: (26, 12),
        port: our_city,
        port_pos: (10, 12),
        landing: (25, 12),
        lane: Vec::new(),
        members: BTreeSet::from([uid]),
        phase: amphibious_siege_train::AmphibiousPhase::Assemble,
        formed: g.turn,
        ford: false,
        progress: BTreeMap::new(),
        progress_turn: g.turn,
    });
    checks.push((Owner::Amphibious, ai, uid));

    let keeper = unit(&mut g, (10, 12));
    g.spawn_test_unit("warrior", 1, (12, 12));
    let mut ai = agent();
    ai.centre_keeps_its_garrison = true;
    checks.push((Owner::CentreKeeper, ai, keeper));

    let uid = unit(&mut g, (11, 13));
    let mut ai = agent();
    ai.defend_force_holds_the_city = true;
    ai.defend_hold_plan.turn = g.turn;
    ai.defend_hold_plan.cities.push(our_city);
    ai.defend_hold_plan.posts.insert(
        uid,
        defend_force_holds_the_city::HoldPost {
            city: our_city,
            at: (11, 13),
            role: defend_force_holds_the_city::HoldRole::Ring,
        },
    );
    checks.push((Owner::DefendPost, ai, uid));

    let uid = unit(&mut g, (14, 7));
    let settler = g.spawn_test_unit("settler", 0, (14, 7));
    let mut ai = agent();
    ai.settler_guards.insert(settler, uid);
    checks.push((Owner::SettlerGuard, ai, uid));

    let uid = unit(&mut g, (14, 8));
    let builder = g.spawn_test_unit("builder", 0, (14, 8));
    let mut ai = agent();
    ai.builder_support.insert(
        builder,
        civilian_coordination::BuilderSupport::posed(uid, (14, 8)),
    );
    checks.push((Owner::CivilianGuard, ai, uid));

    let uid = unit(&mut g, (20, 12));
    let mut ai = agent();
    ai.siege_train = true;
    ai.force_groups.push(ForceGroup {
        id: 1,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: (20, 12),
        objective: (26, 12),
        focus_target: None,
        posture: ForcePosture::Engage,
        readiness: 1.0,
        local_strength_ratio: 1.0,
    });
    ai.sieges.insert(
        enemy_city,
        siege_train::Siege {
            stage: siege_train::SiegeStage::Invest,
            taker: None,
            entered: g.turn,
            assessed: g.turn,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    checks.push((Owner::SiegeMember, ai, uid));

    let uid = unit(&mut g, (14, 10));
    let mut ai = agent();
    ai.battle_planner_ordered.insert(uid);
    checks.push((Owner::BattlePlan, ai, uid));

    let uid = unit(&mut g, (14, 14));
    let mut ai = agent();
    ai.battle_planner_recovering.insert(uid);
    checks.push((Owner::Recovering, ai, uid));

    let uid = unit(&mut g, (14, 15));
    let mut ai = agent();
    ai.base.recovering_units.insert(uid);
    checks.push((Owner::Recovering, ai, uid));

    let uid = unit(&mut g, (14, 16));
    let mut ai = agent();
    ai.wonder_clearance.insert(
        settler,
        wonder_clearance::Clearance {
            site: (15, 16),
            until: g.turn + 3,
            units: vec![uid],
        },
    );
    checks.push((Owner::WonderClearance, ai, uid));

    let uid = unit(&mut g, (14, 17));
    let mut ai = agent();
    ai.base.camp_bounty_claims.insert((16, 17), (g.turn, uid));
    checks.push((Owner::CampBounty, ai, uid));

    let uid = unit(&mut g, (14, 18));
    let mut ai = agent();
    ai.base.retainers_garrison_claims.insert(our_city, uid);
    checks.push((Owner::RetainersGarrison, ai, uid));

    let free = unit(&mut g, (14, 19));
    for (owner, ai, uid) in &checks {
        assert_eq!(ai.owner_of(&g, 0, *uid), Some(*owner), "{owner:?}");
        assert_eq!(
            ai.owner_of(&g, 0, free),
            None,
            "{owner:?} holds no other unit"
        );
    }
    let posed: BTreeSet<Owner> = checks.iter().map(|(owner, _, _)| *owner).collect();
    assert_eq!(posed.len(), Owner::ALL.len(), "every claimant is posed");
}

/// The precedence is `Owner::ALL`'s order: the siege's taker outranks the
/// battle plan, which outranks a unit out to heal.
#[test]
fn the_most_binding_claim_owns_the_unit() {
    let mut g = board();
    let uid = g.spawn_test_unit("warrior", 0, (14, 12));
    let mut ai = agent();
    ai.battle_planner_recovering.insert(uid);
    assert_eq!(ai.owner_of(&g, 0, uid), Some(Owner::Recovering));
    ai.battle_planner_ordered.insert(uid);
    assert_eq!(ai.owner_of(&g, 0, uid), Some(Owner::BattlePlan));
    ai.reserved_units.insert(uid);
    assert_eq!(ai.owner_of(&g, 0, uid), Some(Owner::SiegeTaker));
    for (index, owner) in Owner::ALL.iter().enumerate() {
        assert_eq!(
            *owner as usize, index,
            "ALL lists {owner:?} in declaration order"
        );
    }
}

/// The siege's taker beside a one-hit enemy, and the immediate-kill pass,
/// which does not read `reserved_units`.
fn taker_beside_a_kill(gene: bool) -> (Game, AdvancedAi, u32, u32) {
    let mut g = board();
    let taker = g.spawn_test_unit("warrior", 0, (14, 12));
    let victim = g.spawn_test_unit("warrior", 1, (15, 12));
    g.units.get_mut(&victim).unwrap().hp = 1;
    let mut ai = agent();
    ai.reserved_units.insert(taker);
    if gene {
        ai.enable_one_owner_per_unit();
    }
    ai.open_unit_orders(&g, 0);
    let plan = plan();
    ai.owned_pass(&mut g, 0, OrderStep::ImmediateKills, |ai, g| {
        ai.prioritize_immediate_kills(g, 0, &plan, &BTreeSet::new())
    });
    (g, ai, taker, victim)
}

/// ⭐ A pre-pass crossing: counted with the gene off, refused with it on.
#[test]
fn a_pre_pass_crossing_is_counted_off_and_refused_on() {
    let (g, ai, taker, victim) = taker_beside_a_kill(false);
    assert!(
        !g.units.contains_key(&victim),
        "off: the immediate kill lands"
    );
    let claims = ai.strategy_census().claims;
    assert_eq!(
        claims.of(Owner::SiegeTaker, OrderStep::ImmediateKills),
        [1, 0, 0],
        "{:?}",
        claims.map()
    );
    assert_eq!(ai.last_order_step(taker), Some(OrderStep::ImmediateKills));

    let (g, ai, taker, victim) = taker_beside_a_kill(true);
    assert!(g.units.contains_key(&victim), "on: the taker is not spent");
    let claims = ai.strategy_census().claims;
    assert_eq!(
        claims.of(Owner::SiegeTaker, OrderStep::ImmediateKills),
        [0, 0, 1],
        "{:?}",
        claims.map()
    );
    let unit = &g.units[&taker];
    assert!(
        unit.moves_left > 0.0 && unit.attacks_left > 0,
        "the hold lasts only the pass"
    );
}

/// A unit out to heal, at full health, beside a one-hit enemy: the ladder
/// strikes with it, which is not a step that acts for a recovering unit.
fn recovering_beside_a_kill(gene: bool) -> (Game, AdvancedAi, u32, u32) {
    let mut g = board();
    let uid = g.spawn_test_unit("warrior", 0, (14, 12));
    let victim = g.spawn_test_unit("warrior", 1, (15, 12));
    g.units.get_mut(&victim).unwrap().hp = 1;
    let mut ai = agent();
    ai.battle_planner_recovering.insert(uid);
    if gene {
        ai.enable_one_owner_per_unit();
    }
    ai.open_unit_orders(&g, 0);
    ai.credited_military_step(&mut g, 0, uid, &plan(), false);
    (g, ai, uid, victim)
}

/// ⭐ A ladder crossing: counted with the gene off; with it on the ladder's
/// order is refused and the owner's own step, or a hold, keeps the unit.
#[test]
fn a_ladder_crossing_is_counted_off_and_refused_on() {
    let (g, ai, uid, victim) = recovering_beside_a_kill(false);
    let claims = ai.strategy_census().claims;
    let [crossed, deliberate, refused] = claims.of_owner(Owner::Recovering);
    assert_eq!(
        (crossed, deliberate, refused),
        (1, 0, 0),
        "{:?}",
        claims.map()
    );
    assert!(!g.units.contains_key(&victim), "off: the strike lands");
    let step = ai.last_order_step(uid).expect("ordered");
    assert!(!Owner::Recovering.acts_for(step), "{step:?}");

    let (g, ai, uid, victim) = recovering_beside_a_kill(true);
    let claims = ai.strategy_census().claims;
    assert_eq!(
        claims.of_owner(Owner::Recovering),
        [0, 0, 1],
        "{:?}",
        claims.map()
    );
    assert!(g.units.contains_key(&victim), "on: no strike");
    assert!(
        ai.last_order_step(uid)
            .is_none_or(|step| Owner::Recovering.acts_for(step)),
        "{:?}",
        ai.last_order_step(uid)
    );
}

/// A ranged shot from a recovering unit's own tile is a deliberate crossing
/// (`ranged-kills-are-free`): counted apart, never refused.
#[test]
fn a_shot_from_its_post_is_deliberate() {
    let mut g = board();
    let archer = g.spawn_test_unit("archer", 0, (14, 12));
    let victim = g.spawn_test_unit("warrior", 1, (15, 12));
    g.units.get_mut(&victim).unwrap().hp = 1;
    let mut ai = agent();
    ai.battle_planner_recovering.insert(archer);
    ai.open_unit_orders(&g, 0);
    g.apply(
        0,
        &Action::Ranged {
            unit: archer,
            target: (15, 12),
        },
    )
    .unwrap();
    ai.credit_orders(&g, OrderStep::ImmediateKills);
    assert_eq!(
        ai.strategy_census()
            .claims
            .of(Owner::Recovering, OrderStep::ImmediateKills),
        [0, 1, 0]
    );
    assert_eq!(
        deliberate_crossing(
            &g,
            archer,
            Owner::Recovering,
            OrderStep::BattlePlanner,
            true
        ),
        Some(Deliberate::ShotFromItsPost)
    );
    g.units.get_mut(&archer).unwrap().hp = 30;
    assert_eq!(
        deliberate_crossing(&g, archer, Owner::SiegeMember, OrderStep::Healing, false),
        Some(Deliberate::WoundedBesiegerHeals)
    );
    g.units.get_mut(&archer).unwrap().hp = 90;
    assert_eq!(
        deliberate_crossing(&g, archer, Owner::SiegeMember, OrderStep::Healing, false),
        None,
        "a healthy besieger keeps its post"
    );
}

/// With the gene off a pre-pass runs once, as it always did; with it on
/// and nothing to cross, it runs on the rehearsal and once for real.
#[test]
fn off_the_pass_runs_once_and_unchanged() {
    for (gene, runs) in [(false, 1), (true, 2)] {
        let mut g = board();
        g.spawn_test_unit("warrior", 0, (14, 12));
        let mut ai = agent();
        if gene {
            ai.enable_one_owner_per_unit();
        }
        ai.open_unit_orders(&g, 0);
        let mut calls = 0;
        ai.owned_pass(&mut g, 0, OrderStep::Hunts, |_, _| calls += 1);
        assert_eq!(calls, runs, "gene {gene}");
    }
}

/// Every turn counts our military units and the ones no step ordered.
#[test]
fn unordered_unit_turns_are_counted_at_the_end_of_the_turn() {
    let mut g = board();
    let moved = g.spawn_test_unit("warrior", 0, (14, 12));
    g.spawn_test_unit("warrior", 0, (14, 16));
    let mut ai = agent();
    ai.open_unit_orders(&g, 0);
    g.apply(
        0,
        &Action::Move {
            unit: moved,
            to: (14, 11),
        },
    )
    .unwrap();
    ai.credit_orders(&g, OrderStep::Staging);
    ai.close_unit_orders(&g, 0);
    let claims = ai.strategy_census().claims;
    assert_eq!((claims.unit_turns, claims.unordered_unit_turns), (2, 1));
}
