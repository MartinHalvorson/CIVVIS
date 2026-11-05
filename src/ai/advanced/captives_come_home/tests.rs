use super::*;

#[test]
fn captives_come_home_wants_the_slots_only_when_the_network_is_short() {
    let g = Game::new(3, 32, 22, 71, 250, 0);
    let mut ai = AdvancedAi::new();
    assert!(
        !ai.captives_come_home_wanted(&g, 0, 0, 3),
        "inert with the gene off"
    );
    ai.enable_captives_come_home();
    assert!(ai.captives_come_home_enabled());
    assert!(
        ai.captives_come_home_wanted(&g, 0, 0, 3),
        "none of three held"
    );
    assert!(
        ai.captives_come_home_wanted(&g, 0, 1, 3),
        "one of three is under half"
    );
    assert!(
        !ai.captives_come_home_wanted(&g, 0, 2, 3),
        "two of three held and no racer: the slots can wait"
    );
    assert!(
        !ai.captives_come_home_wanted(&g, 0, 3, 3),
        "a full network wants nothing"
    );
    assert!(
        !ai.captives_come_home_wanted(&g, 0, 0, 0),
        "no capacity, no captive to want"
    );
}

#[test]
fn version_two_carries_version_one_and_pays_in_favor() {
    let g = Game::new(3, 32, 22, 71, 250, 0);
    let mut ai = AdvancedAi::new();
    assert!(!ai.captives_come_home_pays_favor());
    ai.enable_captives_come_home_2();
    assert!(ai.captives_come_home_enabled(), "v2 carries v1's ask");
    assert!(ai.captives_come_home_wanted(&g, 0, 0, 3));
    assert!(ai.captives_come_home_pays_favor());
}
