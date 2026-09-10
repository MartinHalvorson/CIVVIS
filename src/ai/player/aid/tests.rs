use super::*;

fn opportunity() -> Opportunity<'static> {
    Opportunity {
        kind: "EMERGENCY_SEND_AID",
        target: 1,
        turns_left: 2,
        begun: true,
        member: true,
        ours: Some(50.0),
        leader: Some(200.0),
        peaceful_met_target: true,
        project_finishes: false,
    }
}

#[test]
fn exact_finish_line_budget_and_project_guard_are_shared() {
    let mut opportunities = [opportunity()];
    assert_eq!(choose(251.0, &opportunities).unwrap().1, 151);
    assert!(choose(250.0, &opportunities).is_none());
    opportunities[0].ours = Some(201.0);
    assert!(
        choose(1000.0, &opportunities).is_none(),
        "a standing lead does not need another gift on a replan"
    );
    opportunities[0].ours = Some(50.0);
    opportunities[0].project_finishes = true;
    assert!(choose(251.0, &opportunities).is_none());
}

#[test]
fn incomplete_unsafe_or_out_of_window_facts_cannot_spend() {
    for alter in [
        |op: &mut Opportunity<'_>| op.ours = None,
        |op: &mut Opportunity<'_>| op.leader = Some(f64::NAN),
        |op: &mut Opportunity<'_>| op.ours = Some(-1.0),
        |op: &mut Opportunity<'_>| op.leader = Some(250.0),
        |op: &mut Opportunity<'_>| op.member = false,
        |op: &mut Opportunity<'_>| op.begun = false,
        |op: &mut Opportunity<'_>| op.peaceful_met_target = false,
        |op: &mut Opportunity<'_>| op.turns_left = -1,
        |op: &mut Opportunity<'_>| op.turns_left = FINISH_WINDOW + 1,
    ] {
        let mut ops = [opportunity()];
        alter(&mut ops[0]);
        assert!(choose(1000.0, &ops).is_none());
    }
    assert!(choose(f64::INFINITY, &[opportunity()]).is_none());
}

#[test]
fn overlapping_requests_choose_deadline_then_cost_then_target() {
    let mut ops = [opportunity(), opportunity(), opportunity()];
    ops[0].turns_left = 1;
    ops[0].target = 4;
    ops[1].turns_left = 1;
    ops[1].target = 3;
    assert_eq!(choose(500.0, &ops).unwrap().0.target, 3);
    ops[0].leader = Some(100.0);
    assert_eq!(choose(500.0, &ops).unwrap().0.target, 4);
    ops[2].turns_left = 0;
    assert_eq!(choose(500.0, &ops).unwrap().0.target, 1);
}

#[test]
fn native_planning_logs_the_same_gift_without_mutating_its_source() {
    let mut game = Game::new_full(3, 24, 16, 41, 20, 0, false);
    for pid in 0..3 {
        let unit = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|id| game.units[id].kind == "settler")
            .unwrap();
        game.current = pid;
        game.apply(pid, &Action::FoundCity { unit }).unwrap();
    }
    game.current = 0;
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    game.players[0].gold = 251.0;
    game.competition = Some(crate::game::Competition {
        kind: "EMERGENCY_SEND_AID".into(),
        target: Some(1),
        ends: game.turn + 2,
        // The recipient is not a scoring member. A third major holds the lead.
        scores: [(0, 50.0), (2, 200.0)].into(),
    });
    let mut view = game.player_decision_view(0);
    let mut queued = view.clone();
    let cid = queued.player_city_ids(0)[0];
    queued.cities.get_mut(&cid).unwrap().queue = vec![Item::Project {
        project: crate::name!("send_aid"),
    }];
    queued.cities.get_mut(&cid).unwrap().production = 1_000_000.0;
    plan_native(&mut queued, 0);
    assert!(
        queued.log.is_empty(),
        "a finishing own-city aid project must suppress the gift"
    );
    plan_native(&mut view, 0);
    let (_, action) = view
        .log
        .since(0)
        .next()
        .expect("an affordable gift must be logged");
    assert!(
        matches!(action, Action::Trade { player: 1, offer, request } if offer.gold == 151.0 && request.is_empty())
    );
    assert_eq!(game.players[0].gold, 251.0);
    assert_eq!(view.players[0].gold, 100.0);
}
