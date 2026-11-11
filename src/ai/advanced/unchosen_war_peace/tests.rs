use super::super::*;
use super::{UNCHOSEN_WAR_OURS_TURNS, UNCHOSEN_WAR_PEACE_AGE};

/// Two majors in contact and at peace, each with a capital, at turn 30, and
/// a plan that still wants four cities.
fn two_majors() -> (Game, StrategicPlan) {
    let mut g = Game::new_full(2, 24, 16, 7_971, 300, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| g.units[unit].kind == "settler")
            .expect("every major starts with a settler");
        g.found_city_for(pid, g.units[&settler].pos, None);
        g.remove_unit(settler);
    }
    g.record_contact(0, 1);
    g.turn = 30;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, plan)
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_unchosen_war_takes_the_first_peace();
    ai
}

/// One frame of observation at the board's turn, then `turns` more turns.
fn watch(ai: &mut AdvancedAi, g: &mut Game, turns: u32) {
    ai.observe_unchosen_wars(g, 0);
    for _ in 0..turns {
        g.turn += 1;
        ai.observe_unchosen_wars(g, 0);
    }
}

#[test]
fn a_war_declared_on_us_is_offered_peace_from_the_host_age() {
    let (mut g, plan) = two_majors();
    let mut ai = armed();
    ai.observe_unchosen_wars(&g, 0);
    g.turn += 1;
    g.at_war.insert((0, 1));
    let declared = g.turn;
    watch(&mut ai, &mut g, UNCHOSEN_WAR_PEACE_AGE - 1);
    assert_eq!(
        ai.unchosen_war_peace_since(&g, 0, 1, &plan),
        None,
        "one turn short of the age the host signs at"
    );
    g.turn += 1;
    ai.observe_unchosen_wars(&g, 0);
    assert_eq!(ai.unchosen_war_peace_since(&g, 0, 1, &plan), Some(declared));
    assert_eq!(
        AdvancedAi::targeting(VictoryTarget::Domination).unchosen_war_peace_since(&g, 0, 1, &plan),
        None,
        "off"
    );
}

#[test]
fn a_war_this_seat_declared_is_not_offered() {
    let (mut g, plan) = two_majors();
    let mut ai = armed();
    ai.observe_unchosen_wars(&g, 0);
    let start = g.log.len();
    g.log.push(0, Action::DeclareWar { player: 1 });
    g.at_war.insert((0, 1));
    ai.note_frame_declarations(&g, 0, start);
    g.turn += 1;
    watch(
        &mut ai,
        &mut g,
        UNCHOSEN_WAR_PEACE_AGE + UNCHOSEN_WAR_OURS_TURNS,
    );
    assert_eq!(ai.unchosen_war_peace_since(&g, 0, 1, &plan), None);
}

#[test]
fn a_war_already_running_when_the_record_began_is_never_counted() {
    let (mut g, plan) = two_majors();
    g.at_war.insert((0, 1));
    let mut ai = armed();
    watch(&mut ai, &mut g, UNCHOSEN_WAR_PEACE_AGE + 2);
    assert_eq!(ai.unchosen_war_peace_since(&g, 0, 1, &plan), None);
}

#[test]
fn the_offer_stands_down_once_the_empire_is_built_or_the_war_ends() {
    let (mut g, mut plan) = two_majors();
    let mut ai = armed();
    ai.observe_unchosen_wars(&g, 0);
    g.turn += 1;
    g.at_war.insert((0, 1));
    watch(&mut ai, &mut g, UNCHOSEN_WAR_PEACE_AGE);
    assert!(ai.unchosen_war_peace_since(&g, 0, 1, &plan).is_some());
    plan.desired_cities = g.player_city_ids(0).len();
    assert_eq!(
        ai.unchosen_war_peace_since(&g, 0, 1, &plan),
        None,
        "the plan holds every city it wants"
    );
    plan.desired_cities = 4;
    g.at_war.remove(&(0, 1));
    ai.observe_unchosen_wars(&g, 0);
    g.at_war.insert((0, 1));
    assert_eq!(
        ai.unchosen_war_peace_since(&g, 0, 1, &plan),
        None,
        "a war that ended starts its count again"
    );
}

#[test]
fn a_war_our_roster_can_win_is_kept() {
    let (mut g, plan) = two_majors();
    let mut ai = armed();
    ai.observe_unchosen_wars(&g, 0);
    g.turn += 1;
    g.at_war.insert((0, 1));
    watch(&mut ai, &mut g, UNCHOSEN_WAR_PEACE_AGE);
    assert!(ai.unchosen_war_peace_since(&g, 0, 1, &plan).is_some());
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    for _ in 0..24 {
        g.spawn_test_unit("swordsman", 0, home);
    }
    assert!(
        ai.war_policy_siege_against(&g, 0, 1).0,
        "fixture: the roster meets the bill"
    );
    assert_eq!(ai.unchosen_war_peace_since(&g, 0, 1, &plan), None);
}

/// Native 151911's journal had enough paper strength but no staged units.
/// Keep this distinction explicit while evaluating a replacement siege veto.
#[test]
fn paper_feasibility_vetoes_science_peace_with_an_entirely_distant_army() {
    let (mut g, plan) = two_majors();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.enable_unchosen_war_takes_the_first_peace();
    let journal = crate::reasoning::Journal::recording();
    ai.base.attach_journal(journal.handle());
    journal.begin_turn(g.turn, 0);
    ai.observe_unchosen_wars(&g, 0);
    g.turn += 1;
    g.at_war.insert((0, 1));
    watch(&mut ai, &mut g, UNCHOSEN_WAR_PEACE_AGE);
    assert!(ai.unchosen_war_peace_since(&g, 0, 1, &plan).is_some());
    assert!(journal
        .since(0)
        .thoughts
        .last()
        .unwrap()
        .detail
        .contains("eligible"));
    let target = g.cities[&g.player_city_ids(1)[0]].pos;
    let distant = g
        .wdisk(target, 24)
        .into_iter()
        .filter(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
        })
        .max_by_key(|pos| (g.wdist(*pos, target), *pos))
        .expect("distant land");
    assert!(g.wdist(distant, target) > 10);
    for uid in g.player_unit_ids(0) {
        g.remove_unit(uid);
    }
    for _ in 0..24 {
        g.spawn_test_unit("swordsman", 0, distant);
    }
    assert!(ai.staged_campaign_units(&g, 0, 1, target).is_empty());
    let (feasible, bill, roster) = ai.war_policy_siege_against(&g, 0, 1);
    assert!(feasible && roster >= bill);
    assert_eq!(ai.unchosen_war_peace_since(&g, 0, 1, &plan), None);
    let trace = journal.since(0);
    let detail = &trace.thoughts.last().unwrap().detail;
    assert!(detail.contains("whole-roster siege feasibility"));
    assert!(detail.contains("cities 1/4"));
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    super::super::test_support::opt_in_off_in_both_controllers(
        "unchosen-war-takes-the-first-peace",
        |ai| ai.unchosen_war_takes_the_first_peace,
    );
    let (mut g, _) = two_majors();
    let mut ai = AdvancedAi::new();
    ai.observe_unchosen_wars(&g, 0);
    g.at_war.insert((0, 1));
    ai.observe_unchosen_wars(&g, 0);
    assert!(ai.unchosen_war_since.is_empty(), "off, nothing is recorded");
}

/// `peace-waits-for-the-host-minimum`: a war declared on us holds every
/// offer until the host's minimum war length, then releases it; a war we
/// declared, one already running when the record began, and the gene off
/// never hold. It records on its own, without the first-peace gene.
#[test]
fn peace_waits_for_the_host_minimum() {
    use super::PEACE_HOST_MINIMUM_STANDARD;
    super::super::test_support::opt_in_off_in_both_controllers(
        "peace-waits-for-the-host-minimum",
        |ai| ai.peace_waits_for_the_host_minimum,
    );
    let (mut g, _) = two_majors();
    let minimum = g.standard_duration(PEACE_HOST_MINIMUM_STANDARD);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_peace_waits_for_the_host_minimum();
    let off = AdvancedAi::targeting(VictoryTarget::Culture);
    assert_eq!(ai.young_war_peace_waits(&g, 0, 1), None, "at peace");
    ai.observe_unchosen_wars(&g, 0);
    g.turn += 1;
    g.at_war.insert((0, 1));
    for age in 0..minimum {
        ai.observe_unchosen_wars(&g, 0);
        assert_eq!(ai.young_war_peace_waits(&g, 0, 1), Some(age));
        assert_eq!(off.young_war_peace_waits(&g, 0, 1), None, "off");
        g.turn += 1;
    }
    ai.observe_unchosen_wars(&g, 0);
    assert_eq!(
        ai.young_war_peace_waits(&g, 0, 1),
        None,
        "the host signs from here"
    );

    let (mut ours, _) = two_majors();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_peace_waits_for_the_host_minimum();
    ai.observe_unchosen_wars(&ours, 0);
    let start = ours.log.len();
    ours.log.push(0, Action::DeclareWar { player: 1 });
    ours.at_war.insert((0, 1));
    ai.note_frame_declarations(&ours, 0, start);
    ours.turn += 1;
    ai.observe_unchosen_wars(&ours, 0);
    assert_eq!(ai.young_war_peace_waits(&ours, 0, 1), None, "our own war");

    let (mut running, _) = two_majors();
    running.at_war.insert((0, 1));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_peace_waits_for_the_host_minimum();
    ai.observe_unchosen_wars(&running, 0);
    assert_eq!(
        ai.young_war_peace_waits(&running, 0, 1),
        None,
        "already running"
    );
}
