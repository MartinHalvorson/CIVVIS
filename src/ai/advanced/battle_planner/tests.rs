use super::*;
use crate::doctrine::{build, position};
use crate::hex;

fn open_field() -> Game {
    let mut g = build(position("the_reserve").expect("known"), 3).expect("buildable");
    let seeded: Vec<u32> = (0..2).flat_map(|pid| g.player_unit_ids(pid)).collect();
    for uid in seeded {
        g.remove_unit(uid);
    }
    g
}

fn at(col: i32, row: i32) -> Pos {
    hex::offset_to_axial(col, row)
}

fn wound(g: &mut Game, uid: u32, hp: i32) {
    g.units.get_mut(&uid).expect("unit").hp = hp;
}

fn fortify(g: &mut Game, uid: u32) {
    let unit = g.units.get_mut(&uid).expect("unit");
    unit.fortified = true;
    unit.fortify_turns = 2;
}

fn conquest(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: super::super::GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

/// A force of every unit seat 0 owns, anchored on its medoid the way
/// `rebuild_force_groups` anchors it, in the given posture toward
/// `objective`.
fn force(g: &Game, objective: Pos, posture: ForcePosture) -> ForceGroup {
    let mut units = g.player_unit_ids(0);
    units.sort_unstable();
    let anchor = units
        .iter()
        .map(|uid| g.units[uid].pos)
        .min_by_key(|pos| {
            (
                units
                    .iter()
                    .map(|other| g.wdist(*pos, g.units[other].pos))
                    .sum::<i32>(),
                *pos,
            )
        })
        .expect("a force");
    ForceGroup {
        id: units[0],
        domain: super::super::ForceDomain::Land,
        units,
        anchor,
        objective,
        focus_target: None,
        posture,
        readiness: 1.0,
        local_strength_ratio: 1.0,
    }
}

fn version_two() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    ai
}

#[test]
fn live_finishing_survival_reserves_health_for_both_combat_rolls() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(9, 7));
    let victim = g.spawn_unit("scout", 1, at(10, 7));
    let reply = g.spawn_unit("warrior", 1, at(11, 7));
    wound(&mut g, ours, 60);
    wound(&mut g, victim, 2);
    let action = Action::Attack {
        unit: ours,
        target: at(10, 7),
    };
    let mut after = g.clone();
    after.apply(0, &action).expect("legal finishing attack");
    assert!(!after.units.contains_key(&victim));
    let incoming = DangerField::with_reach(&after, 0, true).danger(at(10, 7), ours);
    assert!(
        incoming > 0.0 && incoming < f64::from(after.units[&ours].hp),
        "the sampled attack survives a mean reply: hp={}, incoming={incoming}",
        after.units[&ours].hp
    );
    let mut ai = AdvancedAi::new();
    ai.enable_doomed_blow_veto();
    assert!(
        !ai.live_finishing_actions_survive(&g, 0, [&action]),
        "a favorable sampled attack and an average reply do not prove survival"
    );
    ai.enable_doomed_blow_veto_2();
    assert!(!ai.live_finishing_actions_survive(&g, 0, [&action]));
    assert!(
        ai.kill_sequence(&g, 0).is_empty(),
        "the ordinary battle pass must not restore the rejected finish"
    );
    ai.disable_doomed_blow_veto_2();
    assert!(
        ai.live_finishing_actions_survive(&g, 0, [&action]),
        "the disabled policy preserves the old choice"
    );
    ai.enable_doomed_blow_veto();
    wound(&mut g, ours, 100);
    assert!(
        ai.live_finishing_actions_survive(&g, 0, [&action]),
        "a healthy attacker can survive the same reply"
    );
    wound(&mut g, ours, 60);
    g.remove_unit(reply);
    assert!(
        ai.live_finishing_actions_survive(&g, 0, [&action]),
        "the same finish is safe without the reply"
    );
    ai.disable_doomed_blow_veto();
    assert!(ai.live_finishing_actions_survive(&g, 0, [&action]));
}

fn version_three() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_3();
    ai
}

/// File the host's reading of one pair on the board.
fn host_preview(
    g: &mut Game,
    uid: u32,
    target: Pos,
    ranged: bool,
    to_attacker: i32,
    to_defender: i32,
) {
    let mut previews = (*g.host_previews).clone();
    previews.insert(
        (uid, target, ranged),
        crate::game::HostStrikePreview {
            attacker_strength: 0.0,
            defender_strength: 0.0,
            damage_to_attacker: to_attacker,
            damage_to_defender: to_defender,
            defender_wall_damage: 0,
        },
    );
    g.host_previews = Arc::new(previews);
}

#[test]
fn version_three_ships_off_is_registered_and_turns_the_others_off() {
    let ai = AdvancedAi::new();
    assert!(!ai.battle_planner_3, "an opt-in ships off");
    assert!(ai.wanted_previews().is_empty());
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "battle_planner_3"));
    let mut on = AdvancedAi::new();
    on.enable_battle_planner();
    on.enable_battle_planner_2();
    assert_eq!((on.battle_planner, on.battle_planner_2), (false, true));
    on.enable_battle_planner_3();
    assert_eq!(
        (on.battle_planner, on.battle_planner_2, on.battle_planner_3),
        (false, false, true),
        "one version of the family plays: version three turns one and two off"
    );
    assert!(on.battle_planner_on() && on.positions_plan_on());
    on.disable_battle_planner_3();
    assert!(!on.battle_planner_3 && !on.battle_planner_on());
    super::super::test_support::opt_in_off_in_both_controllers("battle-planner-3", |ai| {
        ai.battle_planner_3
    });
}

/// A unit the siege has reserved as its taker is not spent by the kill
/// plan, not rotated, not placed: version two plans it, version three
/// leaves it to the siege train.
#[test]
fn a_reserved_taker_is_not_planned() {
    let mut g = open_field();
    let taker = g.spawn_unit("warrior", 0, at(9, 7));
    let victim = g.spawn_unit("warrior", 1, at(10, 7));
    wound(&mut g, victim, 25);
    let mut v2 = version_two();
    assert!(
        v2.kill_sequence(&g, 0)
            .iter()
            .any(|blow| blow.unit == taker),
        "version two spends the warrior on the wounded enemy"
    );
    v2.reserved_units.insert(taker);
    assert!(
        v2.kill_sequence(&g, 0)
            .iter()
            .any(|blow| blow.unit == taker),
        "version two does not read the reservation"
    );
    let mut v3 = version_three();
    v3.reserved_units.insert(taker);
    assert!(
        v3.kill_sequence(&g, 0).is_empty(),
        "the taker is the siege's"
    );
    let plan = conquest(&g);
    v3.force_groups = vec![force(&g, at(10, 7), ForcePosture::Engage)];
    assert!(!v3.plan_battle(&mut g, 0, &plan));
    assert!(g.units.contains_key(&victim), "nothing struck");
    assert_eq!(g.units[&taker].pos, at(9, 7), "not rotated, not placed");
    assert_eq!(v3.census.battle_plan_positioned, 0);
    // Released, the same unit is planned again.
    v3.reserved_units.clear();
    assert!(!v3.kill_sequence(&g, 0).is_empty());
}

/// The host's reading of a pair replaces the closed form: an archer's
/// shot the host prices at 47 plans 47, whatever the centre roll says;
/// and the pairs without a reading are what the plan asks the host for.
#[test]
fn a_host_preview_overrides_the_closed_form() {
    let mut g = open_field();
    let archer = g.spawn_unit("archer", 0, at(8, 7));
    let victim = g.spawn_unit("warrior", 1, at(10, 7));
    wound(&mut g, victim, 60);
    let v3 = version_three();
    let closed: Vec<Blow> = v3.kill_sequence(&g, 0);
    let shot = closed
        .iter()
        .find(|blow| blow.unit == archer && blow.from == at(8, 7))
        .expect("the archer shoots from its tile");
    assert!(
        (shot.expected - 47.0).abs() > 1.0,
        "the fixture's centre roll is not 47"
    );
    // Asked for: the pair, before the host has answered.
    let mut ai = version_three();
    let plan = conquest(&g);
    let mut board = g.clone();
    ai.plan_battle(&mut board, 0, &plan);
    assert!(
        ai.wanted_previews().contains(&(archer, at(10, 7), true)),
        "{:?}",
        ai.wanted_previews()
    );
    // Answered: the plan carries the host's number.
    host_preview(&mut g, archer, at(10, 7), true, 0, 47);
    let priced = v3.kill_sequence(&g, 0);
    let shot = priced
        .iter()
        .find(|blow| blow.unit == archer && blow.from == at(8, 7))
        .expect("the archer still shoots from its tile");
    assert_eq!(shot.expected, 47.0);
    let mut ai = version_three();
    ai.plan_battle(&mut g.clone(), 0, &plan);
    assert!(
        !ai.wanted_previews().contains(&(archer, at(10, 7), true)),
        "an answered pair is not asked again"
    );
    // Version two never reads the host.
    let v2 = version_two();
    let shot = v2
        .kill_sequence(&g, 0)
        .into_iter()
        .find(|blow| blow.unit == archer && blow.from == at(8, 7))
        .expect("version two shoots too");
    assert!((shot.expected - 47.0).abs() > 1.0);
}

/// A blow the host says kills the attacker is vetoed, however the closed
/// form prices it and whatever the kill is worth.
#[test]
fn a_preview_predicted_death_is_vetoed() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(9, 7));
    let victim = g.spawn_unit("warrior", 1, at(10, 7));
    wound(&mut g, victim, 25);
    let v3 = version_three();
    assert!(
        v3.kill_sequence(&g, 0)
            .iter()
            .any(|blow| blow.unit == ours && !blow.ranged),
        "the closed form finishes the wounded warrior"
    );
    host_preview(&mut g, ours, at(10, 7), false, 100, 60);
    assert!(
        v3.kill_sequence(&g, 0).is_empty(),
        "the host says the attacker dies: vetoed"
    );
    // A survivable host reading plans the blow with the host's damage.
    host_preview(&mut g, ours, at(10, 7), false, 12, 60);
    let blows = v3.kill_sequence(&g, 0);
    let blow = blows
        .iter()
        .find(|blow| blow.unit == ours)
        .expect("the survivable blow is planned");
    assert_eq!(blow.expected, 60.0);
    assert!(blow.finishes);
}

#[test]
fn version_two_ships_off_is_registered_and_turns_version_one_off() {
    let ai = AdvancedAi::new();
    assert!(!ai.battle_planner_2, "an opt-in ships off");
    assert!(!ai.battle_planner_on());
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "battle_planner_2"));
    let mut on = AdvancedAi::new();
    on.enable_battle_planner();
    assert!((on.battle_planner, on.battle_planner_2) == (true, false));
    on.enable_battle_planner_2();
    assert_eq!(
        (on.battle_planner, on.battle_planner_2),
        (false, true),
        "one version of the family plays: version two turns version one off"
    );
    assert!(on.battle_planner_on());
    on.disable_battle_planner_2();
    assert!(!on.battle_planner_2 && !on.battle_planner_on());
    super::super::test_support::opt_in_off_in_both_controllers("battle-planner-2", |ai| {
        ai.battle_planner_2
    });
    // And version one alone leaves the positions plan out: nothing is
    // laid out, nothing is counted.
    let mut g = open_field();
    g.spawn_unit("warrior", 0, at(9, 6));
    g.spawn_unit("archer", 0, at(8, 6));
    g.spawn_unit("warrior", 1, at(12, 6));
    let mut v1 = AdvancedAi::new();
    v1.enable_battle_planner();
    v1.force_groups = vec![force(&g, at(12, 6), ForcePosture::Advance)];
    let plan = conquest(&g);
    v1.plan_battle(&mut g, 0, &plan);
    assert_eq!(v1.census.battle_plan_slots, 0);
    assert_eq!(v1.census.battle_plan_positioned, 0);
}

/// A fortified enemy warrior two tiles off, a hill on the tile between:
/// the warrior takes the front slot on the hill and the archer ends two
/// tiles from the enemy beside the warrior and behind it. The slots are
/// laid against the enemy, not the anchor. Planned with no unit armed
/// and played by `apply_position_plan`: through `plan_battle` this
/// archer has a shot to offer and is the ladder's — the next test.
#[test]
fn the_warrior_takes_the_hill_in_front_and_the_archer_stands_two_behind_it() {
    let mut g = open_field();
    let enemy_tile = at(12, 6);
    let hill = at(11, 6);
    g.map.tiles.get_mut(&hill).expect("a tile").hills = true;
    let warrior = g.spawn_unit("warrior", 0, at(10, 6));
    let archer = g.spawn_unit("archer", 0, at(9, 6));
    let enemy = g.spawn_unit("warrior", 1, enemy_tile);
    fortify(&mut g, enemy);
    let mut ai = version_two();
    let group = force(&g, enemy_tile, ForcePosture::Advance);
    let mut field = DangerField::new(&g, 0);
    let plan = ai
        .position_plan(&g, 0, &group, &mut field, &BTreeSet::new())
        .expect("an advancing force of two is planned");
    assert_eq!(plan.contacts, 1);
    let front: Vec<&Slot> = plan
        .slots
        .iter()
        .filter(|slot| slot.role == SlotRole::Front)
        .collect();
    assert_eq!(front.len(), 1, "{:?}", plan.slots);
    assert_eq!(front[0].tile, hill, "the front slot is the hill");
    let shooter: Vec<&Slot> = plan
        .slots
        .iter()
        .filter(|slot| slot.role == SlotRole::Shooter)
        .collect();
    assert_eq!(shooter.len(), 1, "{:?}", plan.slots);
    assert_eq!(g.wdist(shooter[0].tile, enemy_tile), 2, "at range");
    assert_eq!(g.wdist(shooter[0].tile, hill), 1, "behind the front slot");
    let placed: BTreeMap<u32, Pos> = plan
        .placements
        .iter()
        .map(|placement| (placement.unit, placement.tile))
        .collect();
    assert_eq!(placed[&warrior], hill);
    assert_eq!(placed[&archer], shooter[0].tile);
    assert_eq!(
        plan.placements[0].unit, warrior,
        "the front moves first: {:?}",
        plan.placements
    );
    assert!(plan.pace_floor.is_none(), "in contact, nothing is paced");

    let (placed, paced) = ai.apply_position_plan(&mut g, 0, &mut field, &plan);
    assert_eq!((placed, paced), (2, 0));
    let (w, a) = (g.units[&warrior].pos, g.units[&archer].pos);
    assert_eq!(w, hill, "the warrior stands on the hill");
    assert_eq!(
        g.wdist(a, enemy_tile),
        2,
        "the archer ends two from the enemy"
    );
    assert_eq!(g.wdist(a, w), 1, "beside the warrior");
    assert!(
        g.wdist(w, enemy_tile) < g.wdist(a, enemy_tile),
        "and behind it"
    );
    assert!(ai.battle_planner_claims(warrior) && ai.battle_planner_claims(archer));
    assert!(g.units.contains_key(&enemy));
}

/// Through `plan_battle`: an archer already standing beside the warrior
/// on a tile two from the enemy with line of sight has a blow to offer —
/// whether the kill plan spends it or declines it, the positions plan
/// leaves it to the ladder, as version one would. The warrior, which
/// cannot close and strike in one turn, is placed on the hill in front.
#[test]
fn a_unit_with_a_blow_to_offer_is_left_to_the_ladder() {
    let mut g = open_field();
    let enemy_tile = at(12, 6);
    let hill = at(11, 6);
    g.map.tiles.get_mut(&hill).expect("a tile").hills = true;
    let warrior = g.spawn_unit("warrior", 0, at(10, 6));
    let stand = g
        .wdisk(enemy_tile, 2)
        .into_iter()
        .filter(|tile| {
            g.wdist(*tile, enemy_tile) == 2
                && g.wdist(*tile, at(10, 6)) == 1
                && g.line_of_sight_from(*tile, enemy_tile)
        })
        .min()
        .expect("a firing tile beside the warrior");
    let archer = g.spawn_unit("archer", 0, stand);
    let enemy = g.spawn_unit("warrior", 1, enemy_tile);
    fortify(&mut g, enemy);
    let mut ai = version_two();
    let mut field = DangerField::new(&g, 0);
    let (blows, armed, _, _) = ai.kill_sequence_in(&g, 0, &mut field);
    assert!(armed.contains(&archer), "the archer has a shot to offer");
    assert!(
        !armed.contains(&warrior),
        "the warrior cannot close and strike"
    );
    ai.force_groups = vec![force(&g, enemy_tile, ForcePosture::Advance)];
    let strategic = conquest(&g);
    let struck = ai.plan_battle(&mut g, 0, &strategic);
    assert_eq!(struck, !blows.is_empty());
    assert_eq!(g.units[&warrior].pos, hill, "the warrior is placed");
    assert!(ai.battle_planner_claims(warrior));
    if blows.is_empty() {
        assert!(
            !ai.battle_planner_claims(archer),
            "a declined shooter is the ladder's, not the plan's"
        );
        assert_eq!(g.units[&archer].pos, stand, "and has not been moved");
    }
    assert_eq!(ai.census.battle_plan_slots, 1, "one slot: the front's");
    assert_eq!(ai.census.battle_plan_positioned, 1);
}

/// Three warriors and two archers against a pair of enemy warriors: every
/// slot is a distinct tile, every placement is a distinct tile, and after
/// the moves every unit stands on its own tile. Everyone stands three or
/// more tiles from the enemy, so no blow is legal this turn — a melee
/// unit that steps beside an enemy has spent its movement, and the
/// archers cannot close to range — and the positions plan places all.
#[test]
fn two_units_never_receive_the_same_tile() {
    let mut g = open_field();
    for (col, row) in [(8, 5), (8, 6), (8, 7)] {
        g.spawn_unit("warrior", 0, at(col, row));
    }
    g.spawn_unit("archer", 0, at(6, 6));
    g.spawn_unit("archer", 0, at(6, 7));
    let first = g.spawn_unit("warrior", 1, at(11, 6));
    let second = g.spawn_unit("warrior", 1, at(11, 7));
    fortify(&mut g, first);
    fortify(&mut g, second);
    let mut ai = version_two();
    let group = force(&g, at(11, 6), ForcePosture::Engage);
    let mut field = DangerField::new(&g, 0);
    let plan = ai
        .position_plan(&g, 0, &group, &mut field, &BTreeSet::new())
        .expect("planned");
    let slot_tiles: BTreeSet<Pos> = plan.slots.iter().map(|slot| slot.tile).collect();
    assert_eq!(slot_tiles.len(), plan.slots.len(), "{:?}", plan.slots);
    assert_eq!(plan.slots.len(), 5, "one slot per unit: {:?}", plan.slots);
    let placed: BTreeSet<Pos> = plan.placements.iter().map(|p| p.tile).collect();
    assert_eq!(placed.len(), plan.placements.len(), "{:?}", plan.placements);
    let units: BTreeSet<u32> = plan.placements.iter().map(|p| p.unit).collect();
    assert_eq!(units.len(), plan.placements.len(), "one slot per unit");
    assert!(plan
        .slots
        .iter()
        .filter(|slot| slot.role == SlotRole::Front)
        .all(|slot| slot.depth == 1));
    ai.force_groups = vec![group];
    let strategic = conquest(&g);
    assert!(!ai.plan_battle(&mut g, 0, &strategic), "no blow is legal");
    assert_eq!(ai.census.battle_plan_kills, 0);
    let standing: BTreeSet<Pos> = g
        .player_unit_ids(0)
        .iter()
        .map(|uid| g.units[uid].pos)
        .collect();
    assert_eq!(
        standing.len(),
        5,
        "{:?} / {:?}",
        g.player_unit_ids(0)
            .iter()
            .map(|uid| (uid, g.units[uid].pos, g.units[uid].hp))
            .collect::<Vec<_>>(),
        plan.placements
    );
    assert_eq!(ai.census.battle_plan_positioned, 5, "everyone is placed");
}

/// A spearman and a horseman on the approach, the enemy far off: the
/// horseman ends no more than the spearman's pace plus one closer to the
/// objective than the spearman stood, though it could ride four.
#[test]
fn the_horseman_is_paced_to_the_spearman_on_the_approach() {
    let mut g = open_field();
    let spearman = g.spawn_unit("spearman", 0, at(4, 6));
    let horseman = g.spawn_unit("horseman", 0, at(5, 6));
    let enemy = at(19, 6);
    g.spawn_unit("warrior", 1, enemy);
    g.spawn_unit("warrior", 1, at(20, 6));
    let mut ai = version_two();
    let group = force(&g, enemy, ForcePosture::Advance);
    let mut field = DangerField::new(&g, 0);
    let plan = ai
        .position_plan(&g, 0, &group, &mut field, &BTreeSet::new())
        .expect("planned");
    assert_eq!(plan.contacts, 0, "no enemy within the band");
    let spear_start = g.wdist(at(4, 6), enemy);
    assert_eq!(
        plan.pace_floor,
        Some((spear_start - (2 + BODY_PACE_SLACK), 2)),
        "the floor is the slowest member's distance less its pace and the slack"
    );
    assert_eq!(plan.placements.len(), 2);
    ai.force_groups = vec![group];
    let strategic = conquest(&g);
    ai.plan_battle(&mut g, 0, &strategic);
    let horse_end = g.wdist(g.units[&horseman].pos, enemy);
    let spear_end = g.wdist(g.units[&spearman].pos, enemy);
    assert!(
        horse_end >= spear_start - 3,
        "paced: the horseman ends at {horse_end}, the spearman started at {spear_start}"
    );
    assert!(
        horse_end < g.wdist(at(5, 6), enemy),
        "and still advances ({horse_end})"
    );
    assert!(
        spear_end < spear_start,
        "the spearman marches ({spear_end})"
    );
    assert_eq!(ai.census.battle_plan_paced, 1);
    assert_eq!(ai.census.battle_plan_positioned, 2);
}

/// On a board that heals, a 55-hit-point warrior — above the rotation's
/// bar, below the heal slot's — takes a zero-danger tile and fortifies
/// while its healthy friend takes the front slot; it is then in the
/// recovery the rotation keeps.
#[test]
fn a_wounded_unit_gets_a_heal_slot() {
    let mut g = open_field();
    g.tactics.heal = true;
    let enemy_tile = at(12, 6);
    let hill = at(11, 6);
    g.map.tiles.get_mut(&hill).expect("a tile").hills = true;
    let healthy = g.spawn_unit("warrior", 0, at(10, 6));
    let hurt = g.spawn_unit("warrior", 0, at(8, 6));
    let enemy = g.spawn_unit("warrior", 1, enemy_tile);
    fortify(&mut g, enemy);
    wound(&mut g, hurt, 55);
    assert!(
        danger(&g, 0, at(8, 6), hurt) <= NO_DANGER,
        "out of reach where it stands"
    );
    let mut ai = version_two();
    let group = force(&g, enemy_tile, ForcePosture::Advance);
    let mut field = DangerField::new(&g, 0);
    let plan = ai
        .position_plan(&g, 0, &group, &mut field, &BTreeSet::new())
        .expect("planned");
    let heal: Vec<&Placement> = plan
        .placements
        .iter()
        .filter(|placement| placement.role.is_none())
        .collect();
    assert_eq!(heal.len(), 1, "{:?}", plan.placements);
    assert_eq!(heal[0].unit, hurt);
    assert!(
        danger(&g, 0, heal[0].tile, hurt) <= NO_DANGER,
        "a heal slot is a tile the field reads as zero"
    );
    assert_eq!(
        plan.slots
            .iter()
            .filter(|slot| slot.role == SlotRole::Front)
            .count(),
        1,
        "the healthy warrior alone wants a front slot: {:?}",
        plan.slots
    );
    ai.force_groups = vec![group];
    let strategic = conquest(&g);
    ai.plan_battle(&mut g, 0, &strategic);
    let now = &g.units[&hurt];
    assert_eq!(now.pos, heal[0].tile);
    assert!(now.fortified, "it fortifies on its heal slot");
    assert!(
        ai.battle_planner_recovering.contains(&hurt),
        "and enters the recovery"
    );
    assert!(ai.battle_planner_claims(hurt));
    assert_eq!(
        g.units[&healthy].pos, hill,
        "the healthy warrior holds the front"
    );
    assert_eq!(ai.census.battle_plan_positioned, 2);
    assert_eq!(
        ai.census.battle_plan_rotations, 0,
        "the rotation did not fire"
    );
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.battle_planner, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "battle_planner"));
    let mut on = AdvancedAi::new();
    on.enable_battle_planner();
    assert!(on.battle_planner);
    on.disable_battle_planner();
    assert!(!on.battle_planner);
    super::super::test_support::opt_in_off_in_both_controllers("battle-planner", |ai| {
        ai.battle_planner
    });
}

/// Off, the plan reads nothing and orders nothing, whatever the board.
#[test]
fn off_the_plan_is_empty() {
    let mut g = open_field();
    g.spawn_unit("archer", 0, at(9, 6));
    g.spawn_unit("archer", 0, at(9, 8));
    let victim = g.spawn_unit("warrior", 1, at(10, 7));
    wound(&mut g, victim, 40);
    let mut ai = AdvancedAi::new();
    let plan = conquest(&g);
    assert!(!ai.plan_battle(&mut g, 0, &plan));
    assert!(g.units.contains_key(&victim), "nothing struck");
    assert!(ai.battle_planner_ordered.is_empty());
}

/// Two archers and a warrior beside a wounded, fortified enemy warrior
/// that no single blow — and no pair of them — finishes with margin:
/// the plan spends all three, the two shots first and the melee blow
/// last, and the victim is gone from the board.
#[test]
fn two_archers_and_a_warrior_finish_the_wounded_fortified_enemy_melee_last() {
    let mut g = open_field();
    let archer_a = g.spawn_unit("archer", 0, at(9, 6));
    let archer_b = g.spawn_unit("archer", 0, at(9, 8));
    let warrior = g.spawn_unit("warrior", 0, at(9, 7));
    let victim = g.spawn_unit("warrior", 1, at(10, 7));
    fortify(&mut g, victim);
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner();
    // Find a wound where two shots cannot finish with margin but three
    // blows can: the plan then has to sequence all three.
    let mut chosen = None;
    for hp in (40..=90).rev() {
        wound(&mut g, victim, hp);
        let blows = ai.kill_sequence(&g, 0);
        if blows.len() == 3 && blows.iter().any(|blow| blow.finishes) {
            chosen = Some((hp, blows));
            break;
        }
    }
    let (hp, blows) = chosen.expect("some wound needs exactly three blows");
    wound(&mut g, victim, hp);
    assert!(
        blows.iter().all(|blow| blow.target == at(10, 7)),
        "{blows:?}"
    );
    assert!(
        blows[0].ranged && blows[1].ranged,
        "the shots go first: {blows:?}"
    );
    assert!(
        !blows[2].ranged && blows[2].unit == warrior,
        "the melee blow is last: {blows:?}"
    );
    assert!(blows[2].finishes && !blows[0].finishes, "{blows:?}");
    let shooters: BTreeSet<u32> = blows.iter().map(|blow| blow.unit).collect();
    assert_eq!(
        shooters,
        [archer_a, archer_b, warrior].into_iter().collect()
    );
    let plan = conquest(&g);
    assert!(ai.plan_battle(&mut g, 0, &plan), "a blow landed");
    assert!(
        !g.units.contains_key(&victim),
        "the victim is finished on the real board"
    );
    assert_eq!(ai.census.battle_plan_kills, 1);
    assert_eq!(ai.census.battle_plan_verified_kills, 1);
    assert_eq!(ai.census.battle_plan_dropped_blows, 0);
    for uid in [archer_a, archer_b, warrior] {
        assert!(
            ai.battle_planner_claims(uid),
            "the ladder leaves a planned striker alone"
        );
    }
}

/// A 20-hit-point warrior beside a healthy fortified swordsman: the
/// return would kill it and it finishes nothing, so no blow is planned
/// and the unit is not spent.
#[test]
fn a_suicidal_attack_is_vetoed() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(9, 7));
    let enemy = g.spawn_unit("swordsman", 1, at(10, 7));
    fortify(&mut g, enemy);
    wound(&mut g, ours, 20);
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner();
    let (att, def) = g.melee_exchange_strengths(ours, enemy).expect("a pair");
    assert!(
        expected_damage(def, att) >= 20.0,
        "the fixture's return blow is lethal: {}",
        expected_damage(def, att)
    );
    assert!(ai.kill_sequence(&g, 0).is_empty());
    // And a healthy warrior in the same place is not suicidal but is not
    // a kill either: the exchange is a loss, so nothing is planned.
    wound(&mut g, ours, 100);
    assert!(ai.kill_sequence(&g, 0).is_empty());
}

/// A 30-hit-point warrior beside a healthy enemy steps to a tile the
/// enemy cannot strike next turn and fortifies; the healthy warrior on
/// the same board does neither.
///
/// Beside, not two tiles off: stepping into a tile adjacent to an enemy
/// ends a move here (zone of control), so a melee unit two tiles away
/// cannot close and strike in one turn and the field reads zero there —
/// which is the engine's fact, and the reason the fixture is adjacent.
#[test]
fn a_wounded_unit_rotates_to_a_zero_danger_tile_and_fortifies_while_a_healthy_one_does_not() {
    let mut g = open_field();
    g.tactics.heal = true;
    let hurt = g.spawn_unit("warrior", 0, at(10, 6));
    let healthy = g.spawn_unit("warrior", 0, at(9, 8));
    let enemy = g.spawn_unit("warrior", 1, at(11, 6));
    wound(&mut g, hurt, 30);
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner();
    let before = danger(&g, 0, at(10, 6), hurt);
    assert!(
        before > 10.0,
        "the wounded unit is exposed where it stands: {before}"
    );
    let plan = conquest(&g);
    ai.plan_battle(&mut g, 0, &plan);
    let after = &g.units[&hurt];
    assert_ne!(after.pos, at(10, 6), "it left the exposed tile");
    assert!(after.fortified, "and fortified where it went");
    assert!(g.units.contains_key(&enemy));
    let now = danger(&g, 0, after.pos, hurt);
    assert!(now <= NO_DANGER, "its new tile is out of reach: {now}");
    assert!(ai.battle_planner_recovering.contains(&hurt));
    assert_eq!(ai.census.battle_plan_rotations, 1);
    let fresh = &g.units[&healthy];
    assert_eq!(fresh.pos, at(9, 8), "the healthy unit is not rotated");
    assert!(!fresh.fortified);
    assert!(!ai.battle_planner_recovering.contains(&healthy));
}

/// The danger field at a tile in range of two enemy archers is exactly
/// the sum of their expected shots at our unit standing there.
#[test]
fn the_danger_field_sums_the_archers_expected_damage() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let left = g.spawn_unit("archer", 1, at(8, 6));
    let right = g.spawn_unit("archer", 1, at(12, 6));
    let tile = at(10, 6);
    for archer in [left, right] {
        assert!(g.wdist(g.units[&archer].pos, tile) <= 2, "in range");
        assert!(g.attack_reach(archer).contains(&tile));
    }
    let expected: f64 = [left, right]
        .into_iter()
        .map(|archer| {
            let (att, def) = g
                .ranged_strike_strengths(archer, ours, tile)
                .expect("a pair");
            expected_damage(att, def)
        })
        .sum();
    assert!(expected > 0.0);
    let read = danger(&g, 0, tile, ours);
    assert!((read - expected).abs() < 1e-9, "{read} v {expected}");
    // The same unit fortified reads the same: the field prices it
    // unfortified, as it would stand after moving there.
    fortify(&mut g, ours);
    let read = danger(&g, 0, tile, ours);
    assert!((read - expected).abs() < 1e-9, "{read} v {expected}");
    // And a tile beyond both archers' reach reads zero.
    let far = at(10, 12);
    assert!([left, right]
        .iter()
        .all(|archer| !g.attack_reach(*archer).contains(&far)));
    assert_eq!(danger(&g, 0, far, ours), 0.0);
}

/// `shared-danger`: two archers strike once each, so with two of our
/// units inside both reaches each unit reads one archer's worth, never
/// less than the strongest single shot. The gene ships off.
#[test]
fn shared_danger_splits_each_blow_among_the_units_in_its_reach() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let left = g.spawn_unit("archer", 1, at(8, 6));
    let right = g.spawn_unit("archer", 1, at(12, 6));
    let tile = at(10, 6);
    let full = danger(&g, 0, tile, ours);
    let mut field = DangerField::new(&g, 0);
    field.share(&g);
    let alone = field.rotation_danger(tile, ours);
    assert!(
        (alone - full).abs() < 1e-9,
        "one unit in reach takes both: {alone} v {full}"
    );

    let friend = *g
        .wdisk(tile, 2)
        .iter()
        .find(|pos| {
            **pos != tile
                && [left, right].iter().all(|archer| {
                    g.attack_reach(*archer).contains(pos) && g.units[archer].pos != **pos
                })
        })
        .expect("a second tile inside both reaches");
    g.spawn_unit("warrior", 0, friend);
    let full = danger(&g, 0, tile, ours);
    let mut field = DangerField::new(&g, 0);
    field.share(&g);
    let shared = field.rotation_danger(tile, ours);
    let strongest = [left, right]
        .into_iter()
        .map(|archer| {
            let (att, def) = g
                .ranged_strike_strengths(archer, ours, tile)
                .expect("a pair");
            expected_damage(att, def)
        })
        .fold(0.0_f64, f64::max);
    assert!(
        (shared - (full / 2.0).max(strongest)).abs() < 1e-9,
        "{shared} v {full}"
    );
    assert!(shared < full);
    assert!(!AdvancedAi::new().shared_danger, "an opt-in ships off");
    super::super::test_support::opt_in_off_in_both_controllers("shared-danger", |ai| {
        ai.shared_danger
    });
}

/// `strike-reach` is an opt-in that ships off and is registered.
#[test]
fn strike_reach_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.strike_reach, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "strike_reach"));
    let mut on = AdvancedAi::new();
    on.enable_strike_reach();
    assert!(on.strike_reach);
    on.disable_strike_reach();
    assert!(!on.strike_reach);
    super::super::test_support::opt_in_off_in_both_controllers("strike-reach", |ai| {
        ai.strike_reach
    });
}

/// A melee enemy two tiles off: the movement flood writes zero movement
/// into the tile beside us, so `attack_reach` — and the field on it —
/// read zero where we stand; the strike reach knows the unit keeps its
/// movement for the blow when our zone of control stops it, and reads
/// the blow. Three tiles off, both read zero.
#[test]
fn a_melee_enemy_two_tiles_off_is_no_danger_to_the_flood_and_a_blow_to_the_strike_reach() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("warrior", 1, at(12, 6));
    let tile = at(10, 6);
    assert_eq!(g.wdist(g.units[&enemy].pos, tile), 2);
    assert!(g.unit_max_moves(enemy) >= 2.0);
    assert!(
        !g.attack_reach(enemy).contains(&tile),
        "the flood stops at our zone of control"
    );
    assert_eq!(danger(&g, 0, tile, ours), 0.0);
    let read = strike_danger(&g, 0, tile, ours);
    let (att, def) = g.melee_exchange_strengths(enemy, ours).expect("a pair");
    assert!(
        read > 0.0,
        "the strike reach prices the closing blow: {read}"
    );
    assert!(
        (read - expected_damage(att, def)).abs() < 1e-9,
        "{read} v {}",
        expected_damage(att, def)
    );
    // Beyond one move and a blow, nothing.
    let far = at(9, 6);
    assert_eq!(g.wdist(g.units[&enemy].pos, far), 3);
    assert_eq!(strike_danger(&g, 0, far, ours), 0.0);
    assert_eq!(danger(&g, 0, far, ours), 0.0);
}

/// The same board, played: a 30-hit-point warrior two tiles from the
/// enemy holds where it stands under the flood's reading — its own tile
/// reads zero — and under `strike-reach` steps to a tile the closing blow
/// cannot follow it to.
#[test]
fn with_strike_reach_the_wounded_unit_steps_out_of_the_closing_blow() {
    let mut g = open_field();
    g.tactics.heal = true;
    let hurt = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("warrior", 1, at(12, 6));
    wound(&mut g, hurt, 30);
    let plan = conquest(&g);
    let mut flood = g.clone();
    let mut v1 = AdvancedAi::new();
    v1.enable_battle_planner();
    v1.plan_battle(&mut flood, 0, &plan);
    assert_eq!(
        flood.units[&hurt].pos,
        at(10, 6),
        "the flood reads its tile as safe, so it holds"
    );
    assert!(flood.units[&hurt].fortified);
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner();
    ai.enable_strike_reach();
    ai.plan_battle(&mut g, 0, &plan);
    let after = &g.units[&hurt];
    assert_ne!(
        after.pos,
        at(10, 6),
        "it left the tile the enemy can close on"
    );
    assert!(
        g.wdist(after.pos, g.units[&enemy].pos) >= 3,
        "beyond one move and a blow"
    );
    assert!(after.fortified);
    assert!(strike_danger(&g, 0, after.pos, hurt) <= NO_DANGER);
    assert_eq!(ai.census.strike_reach_widened, 1);
    assert_eq!(ai.census.battle_plan_rotations, 1);
}

/// A siege unit behind a mountain — a shooter that fires only from its
/// own tile, so a sidestep cannot open the line: no line of sight, so
/// the flood and the strike reach on a native board both read zero — the
/// engine's own `do_ranged` refuses that shot there. On the mirrored
/// board the host's rule is the player's visibility, and the strike
/// reach reads the shot.
#[test]
fn on_the_mirrored_board_a_shooter_needs_no_line_of_sight_of_its_own() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(10, 6));
    let archer = g.spawn_unit("catapult", 1, at(8, 6));
    let tile = at(10, 6);
    g.map.tiles.get_mut(&at(9, 6)).expect("a tile").terrain = "mountain".into();
    assert!(g.rules.units[g.units[&archer].kind].siege);
    assert!(
        !g.unit_has_line_of_sight_from(archer, at(8, 6), tile),
        "the mountain blocks the line"
    );
    assert!(!g.attack_reach(archer).contains(&tile));
    assert_eq!(danger(&g, 0, tile, ours), 0.0);
    assert_eq!(
        strike_danger(&g, 0, tile, ours),
        0.0,
        "a native board keeps the engine's rule"
    );
    // The live seat, shown the board by the host.
    let observed: BTreeSet<Pos> = g.map.tiles.keys().copied().collect();
    g.host_observed = Arc::new(observed);
    let read = strike_danger(&g, 0, tile, ours);
    assert!(read > 0.0, "the mirrored board reads the shot: {read}");
    assert_eq!(danger(&g, 0, tile, ours), 0.0, "the flood still does not");
}

/// `safest-stand` is an opt-in that ships off and is registered.
#[test]
fn safest_stand_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.safest_stand, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "safest_stand"));
    let mut on = AdvancedAi::new();
    on.enable_safest_stand();
    assert!(on.safest_stand);
    on.disable_safest_stand();
    assert!(!on.safest_stand);
    super::super::test_support::opt_in_off_in_both_controllers("safest-stand", |ai| {
        ai.safest_stand
    });
}

/// A wounded warrior that cannot leave its tile — one movement point
/// left, mountains on five sides and an enemy warrior on the sixth: no
/// zero-danger tile is in reach, so the rotation as shipped skips it and
/// the ladder has it; under `safest-stand` it holds under fire and
/// fortifies, is the plan's and is recovering.
#[test]
fn with_no_tile_out_of_reach_the_wounded_unit_holds_under_fire_and_fortifies() {
    let mut g = open_field();
    g.tactics.heal = true;
    let hurt = g.spawn_unit("warrior", 0, at(10, 6));
    let enemy = g.spawn_unit("warrior", 1, at(11, 6));
    wound(&mut g, hurt, 45);
    for pos in g.nbrs(at(10, 6)) {
        if pos != at(11, 6) {
            g.map.tiles.get_mut(&pos).expect("a tile").terrain = "mountain".into();
        }
    }
    g.units.get_mut(&hurt).expect("unit").moves_left = 1.0;
    assert!(g.reachable(hurt).is_empty(), "boxed in");
    let here = danger(&g, 0, at(10, 6), hurt);
    assert!(
        here > 0.0 && here < 45.0,
        "in the warrior's reach, not dead: {here}"
    );
    let plan = conquest(&g);
    let mut shipped = g.clone();
    let mut v2 = version_two();
    v2.plan_battle(&mut shipped, 0, &plan);
    assert!(
        !shipped.units[&hurt].fortified,
        "as shipped the rotation skips it"
    );
    assert!(!v2.battle_planner_ordered.contains(&hurt));
    assert!(g.units.contains_key(&enemy));
    let mut ai = version_two();
    ai.enable_safest_stand();
    ai.plan_battle(&mut g, 0, &plan);
    let now = &g.units[&hurt];
    assert_eq!(now.pos, at(10, 6));
    assert!(now.fortified, "it holds under fire and fortifies");
    assert!(ai.battle_planner_ordered.contains(&hurt));
    assert!(ai.battle_planner_recovering.contains(&hurt));
    assert_eq!(ai.census.battle_plan_fallbacks, 1);
    assert_eq!(ai.census.battle_plan_rotations, 0, "it did not move");
}

/// With every tile in reach under some blow, the wounded unit takes the
/// least dangerous one. Two archers whose reaches overlap on the
/// swordsman's tile and part of its ring; the tile only one of them can
/// shoot is where it steps.
#[test]
fn with_no_tile_out_of_reach_the_wounded_unit_takes_the_least_dangerous_one() {
    let mut g = open_field();
    g.tactics.heal = true;
    let hurt = g.spawn_unit("swordsman", 0, at(11, 6));
    g.spawn_unit("archer", 1, at(13, 6));
    g.spawn_unit("archer", 1, at(12, 8));
    wound(&mut g, hurt, 45);
    g.units.get_mut(&hurt).expect("unit").moves_left = 1.0;
    // Wall off every tile in reach that lies beyond both archers, so no
    // zero-danger tile exists.
    let mut walls = Vec::new();
    {
        let mut field = DangerField::new(&g, 0);
        for tile in g.reachable(hurt) {
            if field.danger(tile, hurt) <= NO_DANGER {
                walls.push(tile);
            }
        }
    }
    for pos in walls {
        g.map.tiles.get_mut(&pos).expect("a tile").terrain = "mountain".into();
    }
    let mut field = DangerField::new(&g, 0);
    let mut tiles = vec![at(11, 6)];
    tiles.extend(g.reachable(hurt));
    assert!(tiles.len() > 1, "something is still in reach");
    let readings: Vec<(Pos, f64)> = tiles
        .iter()
        .map(|tile| (*tile, field.danger(*tile, hurt)))
        .collect();
    assert!(
        readings.iter().all(|(_, d)| *d > NO_DANGER),
        "no tile out of reach: {readings:?}"
    );
    let here = field.danger(at(11, 6), hurt);
    let least = readings
        .iter()
        .map(|(_, d)| *d)
        .fold(f64::INFINITY, f64::min);
    assert!(
        least < here - 1.0,
        "a strictly better tile exists: {least} v {here} ({readings:?})"
    );
    assert!(least < 45.0, "and it is survivable: {least}");
    let plan = conquest(&g);
    let mut ai = version_two();
    ai.enable_safest_stand();
    ai.plan_battle(&mut g, 0, &plan);
    let now = &g.units[&hurt];
    assert_ne!(now.pos, at(11, 6), "it moved");
    assert!(now.fortified);
    let mut after = DangerField::new(&g, 0);
    let there = after.danger(now.pos, hurt);
    assert!(
        (there - least).abs() < 1e-6,
        "it stands on the least dangerous tile: {there} v {least}"
    );
    assert_eq!(ai.census.battle_plan_fallbacks, 1);
    assert_eq!(ai.census.battle_plan_rotations, 1);
    assert!(ai.battle_planner_recovering.contains(&hurt));
}

/// A stand the field says would still remove the unit is no stand: the
/// ladder keeps it. The boxed-in warrior again, at ten hit points.
#[test]
fn a_stand_that_would_still_remove_the_unit_is_left_to_the_ladder() {
    let mut g = open_field();
    g.tactics.heal = true;
    let hurt = g.spawn_unit("warrior", 0, at(10, 6));
    g.spawn_unit("warrior", 1, at(11, 6));
    wound(&mut g, hurt, 10);
    for pos in g.nbrs(at(10, 6)) {
        if pos != at(11, 6) {
            g.map.tiles.get_mut(&pos).expect("a tile").terrain = "mountain".into();
        }
    }
    g.units.get_mut(&hurt).expect("unit").moves_left = 1.0;
    assert!(g.reachable(hurt).is_empty());
    assert!(
        danger(&g, 0, at(10, 6), hurt) >= 10.0,
        "lethal where it stands"
    );
    let plan = conquest(&g);
    let mut ai = version_two();
    ai.enable_safest_stand();
    ai.plan_battle(&mut g, 0, &plan);
    assert!(
        !ai.battle_planner_ordered.contains(&hurt),
        "left to the ladder"
    );
    assert_eq!(ai.census.battle_plan_fallbacks, 0);
}

/// `doomed-blow-veto` is an opt-in that ships off and is registered.
#[test]
fn doomed_blow_veto_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.doomed_blow_veto, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "doomed_blow_veto"));
    let mut on = AdvancedAi::new();
    on.enable_doomed_blow_veto();
    assert!(on.doomed_blow_veto);
    on.disable_doomed_blow_veto();
    assert!(!on.doomed_blow_veto);
    super::super::test_support::opt_in_off_in_both_controllers("doomed-blow-veto", |ai| {
        ai.doomed_blow_veto
    });
}

#[test]
fn reply_damage_uses_remaining_hp_without_poisoning_the_field_cache() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(10, 4));
    g.spawn_unit("warrior", 1, at(11, 4));
    let mut field = DangerField::new(&g, 0);
    let healthy = field.danger(at(10, 4), ours);
    let wounded = field.danger_without(at(10, 4), ours, 40, &BTreeSet::new());
    assert!(wounded > healthy, "wounds reduce the defender's strength");
    assert_eq!(field.probe.units[&ours].hp, 100);
    assert_eq!(field.danger(at(10, 4), ours), healthy);
    wound(&mut g, ours, 40);
    assert_eq!(DangerField::new(&g, 0).danger(at(10, 4), ours), wounded);
}

/// Use an explicit host preview to isolate reply accounting from the
/// uncertain strike roll. The field still uses real units and geometry.
fn previewed_finisher(
    g: &Game,
    ours: u32,
    victim: u32,
    ranged: bool,
) -> (Vec<Shooter>, Vec<Target>, Vec<Candidate>) {
    let unit = &g.units[&ours];
    let enemy = &g.units[&victim];
    (
        vec![Shooter {
            uid: ours,
            pos: unit.pos,
            hp: unit.hp,
            cost: 40.0,
            value: 250.0,
            holds_walls: false,
        }],
        vec![Target {
            uid: victim,
            pos: enemy.pos,
            hp: enemy.hp,
            strength: 20.0,
            kill_value: 250.0,
            siege: false,
            captures: true,
            wall_threat: 0.0,
        }],
        vec![Candidate {
            shooter: 0,
            target: 0,
            from: unit.pos,
            ranged,
            att: 30.0,
            def_base: 20.0,
            host: Some((100.0, 0.0)),
            kill_only: false,
        }],
    )
}

#[test]
fn doomed_veto_does_not_count_the_enemy_a_ranged_finisher_removes() {
    let mut g = open_field();
    let ours = g.spawn_unit("archer", 0, at(10, 4));
    let victim = g.spawn_unit("archer", 1, at(11, 4));
    wound(&mut g, ours, 10);
    wound(&mut g, victim, 1);
    let (shooters, targets, candidates) = previewed_finisher(&g, ours, victim, true);
    let mut field = DangerField::new(&g, 0);
    assert!(field.danger(at(10, 4), ours) > 10.0);
    assert!(doomed_shooters(&shooters, &targets, &candidates, &mut field).is_empty());
    // A non-finishing shot still leaves a lethal answer on the board.
    let mut chip = candidates.clone();
    chip[0].host = Some((0.0, 0.0));
    assert!(doomed_shooters(&shooters, &targets, &chip, &mut field).contains(&ours));
    // `defenders-hold-the-walls`: a city's defender is never doomed.
    let mut holding = shooters.clone();
    holding[0].holds_walls = true;
    assert!(doomed_shooters(&holding, &targets, &chip, &mut field).is_empty());
}

#[test]
fn doomed_veto_prices_a_melee_finisher_outside_its_original_garrison() {
    let mut g = open_field();
    g.found_city_for(0, at(10, 4), Some("Refuge".to_string()));
    let ours = g.spawn_unit("warrior", 0, at(10, 4));
    let victim = g.spawn_unit("archer", 1, at(11, 4));
    g.spawn_unit("crossbowman", 1, at(12, 4));
    wound(&mut g, ours, 30);
    wound(&mut g, victim, 1);
    let (shooters, targets, candidates) = previewed_finisher(&g, ours, victim, false);
    let mut field = DangerField::new(&g, 0);
    assert_eq!(field.danger(at(10, 4), ours), 0.0);
    assert!(field.danger_without(at(11, 4), ours, 30, &BTreeSet::from([victim])) > 30.0);
    assert!(doomed_shooters(&shooters, &targets, &candidates, &mut field).contains(&ours));
}

#[test]
fn doomed_garrison_is_claimed_before_the_ladder_can_order_a_sortie() {
    let mut g = open_field();
    g.found_city_for(0, at(10, 4), Some("Refuge".to_string()));
    let ours = g.spawn_unit("warrior", 0, at(10, 4));
    let mut field = DangerField::new(&g, 0);
    let mut ai = version_two();
    ai.enable_doomed_blow_veto();
    ai.rotate_wounded(
        &mut g,
        0,
        &mut field,
        &BTreeSet::new(),
        &BTreeSet::from([ours]),
    );
    assert!(
        ai.battle_planner_claims(ours),
        "the garrison must not fall through to a sortie"
    );
    assert_eq!(g.units[&ours].pos, at(10, 4));
    assert!(g.units[&ours].fortified);
}

/// A warrior whose one blow — on an archer two tiles off along a clear
/// row — is struck from a tile the archer and a swordsman both reach: the
/// return plus the danger at the stand is over its hit points, so it is
/// doomed. The same warrior against the archer alone has a blow it
/// survives and is not.
#[test]
fn a_shooter_with_no_survivable_blow_is_doomed_and_one_with_a_survivable_blow_is_not() {
    let mut g = open_field();
    let ours = g.spawn_unit("warrior", 0, at(10, 4));
    let archer = g.spawn_unit("archer", 1, at(12, 4));
    let ai = AdvancedAi::new();
    {
        let mut field = DangerField::new(&g, 0);
        let (shooters, targets, candidates, armed) = ai.strike_candidates(&g, 0, &mut field);
        assert!(
            armed.contains(&ours),
            "the warrior has a blow on the archer"
        );
        assert!(
            candidates.iter().all(|c| c.from == at(11, 4)),
            "struck from the one stand beside it"
        );
        let doomed = doomed_shooters(&shooters, &targets, &candidates, &mut field);
        assert!(doomed.is_empty(), "the archer alone is a blow it survives");
    }
    let sword = g.spawn_unit("swordsman", 1, at(11, 5));
    assert_eq!(
        g.wdist(g.units[&sword].pos, at(11, 4)),
        1,
        "the swordsman covers the stand"
    );
    assert_eq!(
        g.wdist(g.units[&sword].pos, at(10, 4)),
        2,
        "and not the warrior's own tile"
    );
    let mut field = DangerField::new(&g, 0);
    let (shooters, targets, candidates, armed) = ai.strike_candidates(&g, 0, &mut field);
    assert!(armed.contains(&ours));
    let stand = field.danger(at(11, 4), ours);
    let home = field.danger(at(10, 4), ours);
    assert!(
        stand > 80.0,
        "the stand is nearly lethal on its own: {stand}"
    );
    assert!(
        home < 80.0,
        "the warrior is not exposed where it stands: {home}"
    );
    let doomed = doomed_shooters(&shooters, &targets, &candidates, &mut field);
    assert!(doomed.contains(&ours), "no blow it would survive");
    assert!(g.units.contains_key(&archer));
}

/// Played: with the gene off the doomed warrior is left to the ladder —
/// the plan orders nothing for it; with the gene on, safe where it stands,
/// it holds its ground and fortifies and the archer is untouched; wounded
/// as well, it is the rotation's and steps out of reach.
#[test]
fn with_the_veto_the_doomed_unit_is_rotated_instead_of_left_to_attack() {
    let mut g = open_field();
    g.tactics.heal = true;
    let ours = g.spawn_unit("warrior", 0, at(10, 4));
    let archer = g.spawn_unit("archer", 1, at(12, 4));
    g.spawn_unit("swordsman", 1, at(11, 5));
    let plan = conquest(&g);
    let mut off_board = g.clone();
    let mut off = version_two();
    off.plan_battle(&mut off_board, 0, &plan);
    assert!(
        !off.battle_planner_ordered.contains(&ours),
        "off, the ladder has it"
    );
    assert_eq!(off.census.battle_plan_doomed, 0);
    let mut ai = version_two();
    ai.enable_doomed_blow_veto();
    ai.plan_battle(&mut g, 0, &plan);
    assert_eq!(ai.census.battle_plan_doomed, 1);
    assert!(ai.battle_planner_ordered.contains(&ours), "on, the plan's");
    let now = &g.units[&ours];
    assert_eq!(
        now.pos,
        at(10, 4),
        "safe where it stands, it holds its ground"
    );
    assert!(now.fortified, "and fortifies");
    assert_eq!(ai.census.battle_plan_rotations, 0);
    assert!(g.units.contains_key(&archer), "and struck nothing");
    assert!(
        !ai.battle_planner_recovering.contains(&ours),
        "not a recovery"
    );
    // Wounded as well, the same unit is the rotation's and steps out of reach.
    let mut g2 = off_board.clone();
    wound(&mut g2, ours, 40);
    let mut hurt = version_two();
    hurt.enable_doomed_blow_veto();
    hurt.plan_battle(&mut g2, 0, &plan);
    assert!(hurt.battle_planner_ordered.contains(&ours));
    assert_ne!(
        g2.units[&ours].pos,
        at(10, 4),
        "wounded, it steps out of reach"
    );
    assert!(hurt.battle_planner_recovering.contains(&ours));
}

#[test]
fn a_recovering_garrison_stays_claimed_until_it_can_return_to_battle() {
    for hp in [57, RETURN_HP - 1, RETURN_HP] {
        let mut g = open_field();
        g.tactics.heal = true;
        let refuge = at(10, 4);
        g.found_city_for(0, refuge, Some("Refuge".to_string()));
        let ours = g.spawn_unit("warrior", 0, refuge);
        wound(&mut g, ours, hp);
        let mut ai = version_two();
        // The preceding host frame rotated this unit into the city.
        // A fresh frame restores its unspent movement before FORTIFY
        // has landed, so the per-unit ladder must still leave it alone.
        ai.battle_planner_recovering.insert(ours);
        let plan = conquest(&g);
        ai.plan_battle(&mut g, 0, &plan);
        assert_eq!(ai.battle_planner_claims(ours), hp < RETURN_HP);
        if hp < RETURN_HP {
            assert!(g.units[&ours].fortified);
            assert_eq!(g.units[&ours].pos, refuge);
            assert!(ai.battle_planner_recovering.contains(&ours));
        } else {
            assert!(!ai.battle_planner_recovering.contains(&ours));
        }
    }
}

#[test]
fn doomed_veto_blocks_a_profitable_trade_before_the_kill_search_can_spend_it() {
    let mut g = open_field();
    g.found_city_for(0, at(10, 4), Some("Refuge".to_string()));
    let ours = g.spawn_unit("warrior", 0, at(10, 4));
    let victim = g.spawn_unit("modern_armor", 1, at(11, 4));
    wound(&mut g, ours, 50);
    wound(&mut g, victim, 1);
    let off = version_two();
    assert!(
        off.kill_sequence(&g, 0)
            .iter()
            .any(|blow| blow.unit == ours),
        "the control must choose the profitable sacrifice"
    );
    let mut ai = version_two();
    ai.enable_doomed_blow_veto();
    let mut field = DangerField::new(&g, 0);
    let (blows, armed, _, doomed) = ai.kill_sequence_in(&g, 0, &mut field);
    assert!(doomed.contains(&ours));
    assert!(!armed.contains(&ours));
    assert!(
        blows.iter().all(|blow| blow.unit != ours),
        "the veto must remove the attack before the beam can choose it"
    );
    let plan = conquest(&g);
    ai.plan_battle(&mut g, 0, &plan);
    assert!(g.units.contains_key(&victim));
    assert_eq!(g.units[&ours].pos, at(10, 4));
    assert_eq!(g.units[&ours].hp, 50);
    assert!(ai.battle_planner_claims(ours));
}
#[test]
fn survival_version_two_is_opt_in_and_excludes_version_one() {
    super::super::test_support::opt_in_off_in_both_controllers("doomed-blow-veto-2", |ai| {
        ai.doomed_blow_veto_2
    });
    let mut ai = version_two();
    ai.enable_doomed_blow_veto();
    ai.enable_doomed_blow_veto_2();
    assert!(ai.doomed_blow_veto_2 && !ai.doomed_blow_veto);
    ai.enable_doomed_blow_veto();
    assert!(ai.doomed_blow_veto && !ai.doomed_blow_veto_2);
    ai.enable_doomed_blow_veto_2();
    ai.disable_doomed_blow_veto_2();
    assert!(!ai.doomed_blow_veto && !ai.doomed_blow_veto_2);
}

#[test]
fn a_later_friendly_kill_can_rescue_an_earlier_striker_in_the_same_plan() {
    let mut g = open_field();
    let first = g.spawn_unit("archer", 0, at(10, 4));
    let second = g.spawn_unit("archer", 0, at(10, 5));
    let enemy_first = g.spawn_unit("crossbowman", 1, at(11, 4));
    let enemy_second = g.spawn_unit("crossbowman", 1, at(11, 5));
    for uid in [first, second] {
        wound(&mut g, uid, 50);
    }
    for uid in [enemy_first, enemy_second] {
        wound(&mut g, uid, 1);
    }
    let (mut shooters, mut targets, mut candidates) =
        previewed_finisher(&g, first, enemy_first, true);
    let (more_shooters, more_targets, mut more_candidates) =
        previewed_finisher(&g, second, enemy_second, true);
    shooters.extend(more_shooters);
    targets.extend(more_targets);
    more_candidates[0].shooter = 1;
    more_candidates[0].target = 1;
    candidates.extend(more_candidates);
    let mut field = DangerField::with_reach(&g, 0, true);
    assert_eq!(
        doomed_shooters(&shooters, &targets, &candidates, &mut field),
        BTreeSet::from([first, second])
    );
    let empty = BeamState::empty(targets.len());
    let partial = empty
        .extend(0, &candidates[0], &shooters, &targets, &mut field, true)
        .unwrap();
    assert!(
        !partial.survives,
        "the other crossbow still has a lethal reply"
    );
    assert!(!partial
        .terminal(&shooters, &targets, &candidates)
        .is_finite());
    let complete = partial
        .extend(1, &candidates[1], &shooters, &targets, &mut field, true)
        .unwrap();
    assert!(complete.survives, "both retaliators have now been removed");
    assert!(
        complete.score > partial.score + targets[1].kill_value,
        "removing the reply refunds the earlier exposure penalty"
    );
    let (sequence, _) = search_kill_sequence(&shooters, &targets, &candidates, &mut field, true);
    assert_eq!(
        sequence.len(),
        2,
        "the safe combination must remain discoverable"
    );
    let (incomplete, _) =
        search_kill_sequence(&shooters, &targets, &candidates[..1], &mut field, true);
    assert!(
        incomplete.is_empty(),
        "a stranded unsafe prefix is never a final plan"
    );
}

#[test]
fn survival_version_two_keeps_a_profitable_direct_sacrifice_out_of_the_battle_pass() {
    let mut g = open_field();
    g.found_city_for(0, at(10, 4), Some("Refuge".to_string()));
    let ours = g.spawn_unit("warrior", 0, at(10, 4));
    let victim = g.spawn_unit("modern_armor", 1, at(11, 4));
    wound(&mut g, ours, 50);
    wound(&mut g, victim, 1);
    assert!(!version_two().kill_sequence(&g, 0).is_empty());
    let mut ai = version_two();
    ai.enable_doomed_blow_veto_2();
    assert!(ai.kill_sequence(&g, 0).is_empty());
    let plan = conquest(&g);
    ai.plan_battle(&mut g, 0, &plan);
    assert_eq!(g.units[&ours].hp, 50);
    assert!(g.units.contains_key(&victim));
    assert!(ai.battle_planner_claims(ours));
}

#[test]
fn dropping_the_rescuing_strike_cannot_leave_an_unsafe_verified_prefix() {
    let mut g = open_field();
    let first = g.spawn_unit("archer", 0, at(10, 4));
    let second = g.spawn_unit("archer", 0, at(10, 5));
    let enemy_first = g.spawn_unit("crossbowman", 1, at(11, 4));
    let enemy_second = g.spawn_unit("crossbowman", 1, at(11, 5));
    for uid in [first, second] {
        wound(&mut g, uid, 50);
    }
    for uid in [enemy_first, enemy_second] {
        wound(&mut g, uid, 1);
    }
    let blows: Vec<Blow> = [(first, enemy_first), (second, enemy_second)]
        .into_iter()
        .map(|(ours, enemy)| Blow {
            unit: ours,
            from: g.units[&ours].pos,
            target: g.units[&enemy].pos,
            defender: enemy,
            ranged: true,
            expected: 100.0,
            finishes: true,
        })
        .collect();
    let mut ai = version_two();
    ai.enable_strike_reach();
    ai.enable_doomed_blow_veto_2();
    let plan = conquest(&g);
    assert_eq!(ai.verify_blows(&g, 0, &plan, &blows).0.len(), 2);
    g.units.get_mut(&second).unwrap().attacks_left = 0;
    let (verified, kills, dropped) = ai.verify_blows(&g, 0, &plan, &blows);
    assert!(
        verified.is_empty(),
        "the first strike depended on the rejected second strike"
    );
    assert_eq!(kills, 0);
    assert_eq!(dropped, 2);
    ai.disable_doomed_blow_veto_2();
    assert_eq!(
        ai.verify_blows(&g, 0, &plan, &blows).0.len(),
        1,
        "the old verifier keeps the unsafe prefix"
    );
}
#[test]
fn an_independent_safe_shot_survives_rejection_of_an_unsafe_combination() {
    let mut g = open_field();
    let first = g.spawn_unit("archer", 0, at(10, 4));
    let second = g.spawn_unit("archer", 0, at(10, 5));
    let independent = g.spawn_unit("archer", 0, at(3, 3));
    let enemy_first = g.spawn_unit("crossbowman", 1, at(11, 4));
    let enemy_second = g.spawn_unit("crossbowman", 1, at(11, 5));
    let distant = g.spawn_unit("warrior", 1, at(4, 3));
    for uid in [first, second] {
        wound(&mut g, uid, 50);
    }
    for uid in [enemy_first, enemy_second, distant] {
        wound(&mut g, uid, 1);
    }
    g.units.get_mut(&second).unwrap().attacks_left = 0;
    let blows: Vec<Blow> = [
        (first, enemy_first),
        (second, enemy_second),
        (independent, distant),
    ]
    .into_iter()
    .map(|(ours, enemy)| Blow {
        unit: ours,
        from: g.units[&ours].pos,
        target: g.units[&enemy].pos,
        defender: enemy,
        ranged: true,
        expected: 100.0,
        finishes: true,
    })
    .collect();
    let mut ai = version_two();
    ai.enable_strike_reach();
    ai.enable_doomed_blow_veto_2();
    let plan = conquest(&g);
    let (verified, kills, dropped) = ai.verify_blows(&g, 0, &plan, &blows);
    assert_eq!(
        verified.iter().map(|blow| blow.unit).collect::<Vec<_>>(),
        vec![independent]
    );
    assert_eq!(kills, 1);
    assert_eq!(dropped, 2);
}

#[test]
fn a_safe_alternative_does_not_license_a_more_valuable_fatal_strike() {
    let mut g = open_field();
    let ours = g.spawn_unit("archer", 0, at(10, 4));
    let valuable = g.spawn_unit("warrior", 1, at(11, 4));
    let retaliator = g.spawn_unit("crossbowman", 1, at(10, 5));
    wound(&mut g, ours, 50);
    wound(&mut g, valuable, 1);
    wound(&mut g, retaliator, 1);
    let (shooters, mut targets, mut candidates) = previewed_finisher(&g, ours, valuable, true);
    targets[0].kill_value = 1000.0;
    let (_, more_targets, mut more_candidates) = previewed_finisher(&g, ours, retaliator, true);
    more_candidates[0].target = 1;
    targets.extend(more_targets);
    candidates.extend(more_candidates);
    let mut field = DangerField::with_reach(&g, 0, true);
    assert!(
        doomed_shooters(&shooters, &targets, &candidates, &mut field).is_empty(),
        "the whole-shooter veto allows this unit because one choice survives"
    );
    assert_eq!(
        search_kill_sequence(&shooters, &targets, &candidates, &mut field, false).0,
        vec![0]
    );
    assert_eq!(
        search_kill_sequence(&shooters, &targets, &candidates, &mut field, true).0,
        vec![1],
        "version two must select the survivable alternative"
    );
}

/// Live Lv 8 Deity 045954Z t87: the rotation moved a 40-hp embarked Heavy
/// Chariot one tile, reading "danger 69 where it stood, -0 where it
/// stands"; a visible barbarian Galley three tiles off stepped into the tile
/// it left and killed it (four embarked Chariots fell to Galleys t85-98).
/// An embarked land unit within a visible Galley's reach reads its blow.
#[test]
fn an_embarked_unit_in_a_galley_reach_reads_its_blow() {
    let mut g = open_field();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("coast");
    }
    let galley = g.spawn_test_unit("galley", 1, at(10, 4));
    for (col, row) in [(10, 6), (10, 7)] {
        let ours = g.spawn_test_unit("heavy_chariot", 0, at(col, row));
        wound(&mut g, ours, 40);
        assert!(
            g.is_embarked(&g.units[&ours]),
            "a land unit on coast is embarked"
        );
        let distance = g.wdist(at(col, row), g.units[&galley].pos);
        let mut field = DangerField::new(&g, 0);
        assert!(
            field.reaches_tile(at(col, row)),
            "the galley's reach holds a tile {distance} away"
        );
        assert!(
            field.danger(at(col, row), ours) > 0.0,
            "the blow on an embarked unit {distance} from the galley is priced"
        );
        g.remove_unit(ours);
    }
}

/// The same case's geometry: the only water beside the tile the unit moves
/// to is the tile it leaves. The field is built with the unit still on that
/// tile; the reach reads through units, so the blow is priced.
#[test]
fn the_tile_an_embarked_unit_leaves_is_a_stand_for_the_galley() {
    let mut g = open_field();
    let corridor = [at(10, 4), at(10, 5), at(10, 6), at(10, 7)];
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if corridor.contains(&tile.pos) {
            crate::name!("coast")
        } else {
            crate::name!("plains")
        };
    }
    g.spawn_test_unit("galley", 1, at(10, 4));
    let ours = g.spawn_test_unit("heavy_chariot", 0, at(10, 6));
    wound(&mut g, ours, 40);
    let beside_water: Vec<Pos> = g
        .nbrs(at(10, 7))
        .into_iter()
        .filter(|pos| corridor.contains(pos))
        .collect();
    assert_eq!(
        beside_water,
        vec![at(10, 6)],
        "the left tile is the only approach"
    );
    let mut field = DangerField::new(&g, 0);
    assert!(
        field.reaches_tile(at(10, 7)),
        "the reach reads through the unit that leaves"
    );
    assert!(field.danger(at(10, 7), ours) > 0.0);
}

#[test]
fn ranged_kills_are_free_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.ranged_kills_are_free, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "ranged_kills_are_free"));
    super::super::test_support::opt_in_off_in_both_controllers("ranged-kills-are-free", |ai| {
        ai.ranged_kills_are_free
    });
}

/// Two shooters whose shots each deal 25 to a 38-hp target (a kill needs
/// 38 x KILL_MARGIN), both kill-only (recovering or doomed) and in danger
/// where they stand: together they are a plan, alone a shot is not, and the
/// danger at their stand is not charged.
#[test]
fn kill_only_shots_join_a_sequence_that_kills_and_nothing_less() {
    let mut g = open_field();
    let a = g.spawn_unit("archer", 0, at(9, 4));
    let b = g.spawn_unit("archer", 0, at(9, 5));
    let victim = g.spawn_unit("swordsman", 1, at(11, 4));
    // Three more hostiles keep both stands deadly.
    for row in [3, 5, 6] {
        g.spawn_unit("spearman", 1, at(10, row));
    }
    wound(&mut g, victim, 38);
    let shooter = |uid: u32| Shooter {
        uid,
        pos: g.units[&uid].pos,
        hp: g.units[&uid].hp,
        cost: 40.0,
        value: 60.0,
        holds_walls: false,
    };
    let shooters = vec![shooter(a), shooter(b)];
    let targets = vec![Target {
        uid: victim,
        pos: g.units[&victim].pos,
        hp: 38,
        strength: 35.0,
        kill_value: 250.0,
        siege: false,
        captures: true,
        wall_threat: 0.0,
    }];
    let shot = |index: usize, uid: u32| Candidate {
        shooter: index,
        target: 0,
        from: g.units[&uid].pos,
        ranged: true,
        att: 25.0,
        def_base: 35.0,
        host: Some((25.0, 0.0)),
        kill_only: true,
    };
    let need = f64::from(targets[0].hp) * KILL_MARGIN;
    assert!(
        50.0 >= need && 25.0 < need,
        "two shots kill at the margin, one does not"
    );
    let mut field = DangerField::new(&g, 0);
    let both = vec![shot(0, a), shot(1, b)];
    let (sequence, score) = search_kill_sequence(&shooters, &targets, &both, &mut field, false);
    assert_eq!(sequence.len(), 2, "two shots kill: {sequence:?} at {score}");
    assert!(score > 0.0);
    let alone = vec![shot(0, a)];
    let (sequence, _) = search_kill_sequence(&shooters, &targets, &alone, &mut field, false);
    assert!(sequence.is_empty(), "one shot leaves it alive: no plan");
    // Live 044106Z t49: two shots of about 20 on the 38-hp Swordsman reach
    // its health at the centre but not 38 x KILL_MARGIN. Kill-only shots
    // take no return and no new exposure, so the centre is their bar.
    let light = |index: usize, uid: u32| Candidate {
        host: Some((20.0, 0.0)),
        ..shot(index, uid)
    };
    assert!(40.0 < need, "the pair falls short of the margin");
    let both = vec![light(0, a), light(1, b)];
    let (sequence, score) = search_kill_sequence(&shooters, &targets, &both, &mut field, false);
    assert_eq!(
        sequence.len(),
        2,
        "the centre fells it: {sequence:?} at {score}"
    );
    let alone = vec![light(0, a)];
    let (sequence, _) = search_kill_sequence(&shooters, &targets, &alone, &mut field, false);
    assert!(sequence.is_empty());
}

#[test]
fn field_reads_the_tile_it_leaves_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.field_reads_the_tile_it_leaves, "an opt-in ships off");
    assert!(super::super::GENES
        .iter()
        .any(|gene| gene.opt_in() && gene.field == "field_reads_the_tile_it_leaves"));
    super::super::test_support::opt_in_off_in_both_controllers(
        "field-reads-the-tile-it-leaves",
        |ai| ai.field_reads_the_tile_it_leaves,
    );
}

/// Live 045954Z t87 on the live constructor: with `strike-reach` the field's
/// reach stops at the tile our unit holds, so the only approach to the tile
/// it steps back to reads no danger. Under the gene the hostile stepping into
/// the tile it left is priced, for that unit and no other: a unit elsewhere
/// opens nothing.
#[test]
fn the_field_reads_a_hostile_stepping_into_the_tile_the_unit_leaves() {
    let mut g = open_field();
    let corridor = [at(10, 4), at(10, 5), at(10, 6), at(10, 7)];
    for tile in g.map.tiles.values_mut() {
        tile.terrain = if corridor.contains(&tile.pos) {
            crate::name!("coast")
        } else {
            crate::name!("plains")
        };
    }
    g.spawn_test_unit("galley", 1, at(10, 4));
    let ours = g.spawn_test_unit("heavy_chariot", 0, at(10, 6));
    wound(&mut g, ours, 40);
    let other = g.spawn_test_unit("warrior", 0, at(13, 7));
    let back = at(10, 7);
    let mut off = DangerField::with_reach(&g, 0, true);
    assert!(
        !off.reaches_tile(back),
        "the strike reach stops at our tile"
    );
    assert!(
        off.danger(back, ours) <= NO_DANGER,
        "the miss the gene closes"
    );
    let mut on = DangerField::with_reach(&g, 0, true);
    on.reads_the_tile_it_leaves = true;
    assert!(
        on.danger(back, ours) > 0.0,
        "the Galley steps into the tile left"
    );
    assert!(
        on.danger(back, other) <= NO_DANGER,
        "another unit opens nothing"
    );
    assert!(
        on.danger(at(10, 6), ours) > 0.0,
        "where it stands the blow reads as it always did"
    );
}
