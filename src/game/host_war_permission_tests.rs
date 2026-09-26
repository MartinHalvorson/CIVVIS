use super::*;

fn fixture() -> Game {
    let mut game = Game::new_full(3, 24, 16, 926_3780, 250, 0, false);
    game.turn = 53;
    game.record_contact(0, 1);
    game.record_contact(0, 2);
    Arc::make_mut(&mut game.host_war_type_permissions).insert(
        (0, 1),
        ObservedWarTypes {
            turn: 53,
            permissions: BTreeMap::from([
                ("DECLARE_FORMAL_WAR".to_string(), true),
                ("DECLARE_SURPRISE_WAR".to_string(), false),
            ]),
        },
    );
    game
}

#[test]
fn native_war_type_permissions_expire_and_are_actor_target_scoped() {
    let mut game = fixture();
    assert!(game.casus_belli_available(0, 1, "formal_war"));
    assert!(game.casus_belli_available(0, 1, "formal"));
    assert!(!game.casus_belli_available(0, 2, "formal_war"));
    assert!(!game.casus_belli_available(1, 0, "formal_war"));
    assert!(game.apply(0, &Action::DeclareWar { player: 1 }).is_err());
    game.turn += 1;
    assert!(!game.casus_belli_available(0, 1, "formal_war"));
    assert_eq!(game.host_war_type_permission(0, 1, "surprise_war"), None);
    game.apply(0, &Action::DeclareWar { player: 1 }).unwrap();
}

#[test]
fn native_war_type_permissions_keep_contact_life_and_non_aggression_guards() {
    let game = fixture();
    let formal = Action::DeclareWarWithCasusBelli {
        player: 1,
        casus_belli: "formal_war".to_string(),
    };
    let mut dead = game.clone();
    dead.players[1].alive = false;
    assert!(dead.apply(0, &formal).is_err());
    let mut unmet = game.clone();
    unmet.players[0].met.remove(&1);
    assert!(!unmet.has_met(0, 1));
    assert!(unmet.apply(0, &formal).is_err());
    let mut treaty = game.clone();
    treaty.peace_treaties.insert((0, 1), 100);
    assert!(!treaty
        .legal_actions_within(0, ActionFamilies::DIPLOMACY)
        .contains(&formal));
    assert!(treaty.apply(0, &formal).is_err());
    let mut blocked = game.clone();
    Arc::make_mut(&mut blocked.host_war_blocks).insert((0, 1, game.turn));
    assert!(blocked.apply(0, &formal).is_err());
    let mut friends = game.clone();
    friends.players[0].friends_until.insert(1, 100);
    friends.players[1].friends_until.insert(0, 100);
    assert!(!friends
        .legal_actions_within(0, ActionFamilies::DIPLOMACY)
        .contains(&formal));
    assert!(friends.apply(0, &formal).is_err());
    let mut at_war = game.clone();
    at_war.apply(0, &formal).unwrap();
    assert!(at_war.apply(0, &formal).is_err());
    let restored: Game = serde_json::from_str(&serde_json::to_string(&game).unwrap()).unwrap();
    assert!(
        restored.host_war_type_permissions.is_empty(),
        "live facts are not checkpoint permissions"
    );
    assert!(!restored.casus_belli_available(0, 1, "formal_war"));
}
