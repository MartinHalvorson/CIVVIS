use super::*;

fn board() -> Game {
    let mut g = Game::new_full(2, 14, 14, 10_033_878, 500, 0, false);
    g.players[0].civics.insert(crate::name!("code_of_laws"));
    g.players[1].civics.insert(crate::name!("code_of_laws"));
    g.players[0].government = Some("chiefdom".into());
    g.players[1].government = Some("chiefdom".into());
    g
}

fn slot(card: &str) -> Action {
    Action::SlotPolicy {
        policy: Name::new(card),
    }
}

fn unslot(card: &str) -> Action {
    Action::UnslotPolicy {
        policy: Name::new(card),
    }
}

fn window(g: &mut Game, open: bool) {
    g.host_policy_windows
        .insert(0, HostPolicyWindow { turn: g.turn, open });
}

#[test]
fn a_closed_host_window_preserves_the_deck_and_keeps_card_eligibility_visible() {
    let mut g = board();
    g.apply(0, &slot("discipline")).unwrap();
    let before = g.players[0].policies.clone();
    let gold = g.players[0].gold;
    window(&mut g, false);
    assert!(g.available_policies(0).contains(&crate::name!("god_king")));
    assert!(g.apply(0, &slot("god_king")).is_err());
    assert!(g.apply(0, &unslot("discipline")).is_err());
    assert_eq!(g.players[0].policies, before);
    assert_eq!(g.players[0].gold, gold);
    assert!(
        g.apply(0, &slot("discipline")).is_ok(),
        "acknowledge an unchanged held card"
    );
    g.current = 1;
    assert!(
        g.apply(1, &slot("god_king")).is_ok(),
        "scope the restriction to its seat"
    );
}

#[test]
fn a_free_window_allows_the_whole_planned_deck_without_consuming_it_per_card() {
    let mut g = board();
    window(&mut g, true);
    g.apply(0, &slot("discipline")).unwrap();
    g.apply(0, &slot("god_king")).unwrap();
    g.apply(0, &unslot("god_king")).unwrap();
    g.apply(0, &slot("urban_planning")).unwrap();
    assert!(g.players[0].policies.contains(&crate::name!("discipline")));
    assert!(g.players[0]
        .policies
        .contains(&crate::name!("urban_planning")));
    assert!(!g.players[0].policies.contains(&crate::name!("god_king")));
}

#[test]
fn the_action_menu_does_not_offer_changes_the_host_window_would_reject() {
    let mut g = board();
    g.apply(0, &slot("discipline")).unwrap();
    window(&mut g, false);
    assert!(!g
        .legal_actions(0)
        .iter()
        .any(|a| matches!(a, Action::SlotPolicy { .. } | Action::UnslotPolicy { .. })));
    window(&mut g, true);
    assert!(g
        .legal_actions(0)
        .iter()
        .any(|a| matches!(a, Action::UnslotPolicy { .. })));
}

#[test]
fn an_old_observation_does_not_lock_forecast_turns_and_legacy_games_stay_unrestricted() {
    let mut g = board();
    assert_eq!(g.host_policy_change_window(0), None);
    assert!(serde_json::to_value(&g)
        .unwrap()
        .get("host_policy_windows")
        .is_none());
    window(&mut g, false);
    let clone = g.clone();
    assert!(clone.host_policy_change_blocked(0));
    g.turn += 1;
    assert_eq!(g.host_policy_change_window(0), None);
    g.apply(0, &slot("god_king")).unwrap();
}
