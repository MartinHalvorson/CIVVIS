use super::*;

fn fixture() -> (
    civvis::mirror::LiveMirror,
    civvis::mirror::StateSnapshot,
    u32,
    civvis::Pos,
) {
    let (snapshot, mut state) = tests::local_barbarian_defense_board();
    state.cities[0].districts = serde_json::from_value(serde_json::json!([
        {"type":"DISTRICT_ENCAMPMENT","x":5,"y":4,"complete":true,
         "damage":0,"max_damage":100,"wall_damage":0,"max_wall_damage":100},
        {"type":"DISTRICT_OPPIDUM","x":7,"y":4,"complete":true,
         "damage":0,"max_damage":100,"wall_damage":0,"max_wall_damage":100}
    ]))
    .unwrap();
    let mirror = civvis::mirror::LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
    let city = mirror.cid_of[&state.cities[0].id];
    (mirror, state, city, civvis::hex::offset_to_axial(7, 4))
}

#[test]
fn native_order_separates_the_source_plot_from_the_target_plot() {
    let (mirror, state, city, source) = fixture();
    let action = Action::DistrictStrike {
        city,
        source,
        target: civvis::hex::offset_to_axial(8, 4),
    };
    let order = translate(&action, &mirror, &state).unwrap();
    assert_eq!(order.kind, "district_strike");
    assert_eq!(order.subject, Some(state.cities[0].id));
    assert_eq!(order.verb.as_deref(), Some("7:4"));
    assert_eq!(order.pos, Some((8, 4)));
}

#[test]
fn native_order_requires_the_source_to_belong_to_the_mapped_city() {
    let (mirror, state, city, source) = fixture();
    assert!(translate(
        &Action::DistrictStrike {
            city,
            source: (0, 0),
            target: source
        },
        &mirror,
        &state
    )
    .is_none());
    assert!(translate(
        &Action::DistrictStrike {
            city: u32::MAX,
            source,
            target: source
        },
        &mirror,
        &state
    )
    .is_none());
}

#[test]
fn earlier_frame_oppidum_shot_does_not_spend_the_encampment_shot() {
    let (mut mirror, state, city, source) = fixture();
    let mut strikes = HostCityStrikes::default();
    let issued = IssuedOrder {
        kind: "district_strike".into(),
        subject: Some(state.cities[0].id),
        verb: Some("7:4".into()),
        pos: Some((8, 4)),
    };
    strikes.observe(state.turn, std::slice::from_ref(&issued));
    strikes.apply(&mut mirror, state.turn);
    assert!(
        mirror.game.cities[&city]
            .defending_districts
            .iter()
            .find(|d| d.pos == source)
            .unwrap()
            .struck
    );
    assert!(!mirror.game.cities[&city].encampment_struck);
    strikes.observe(state.turn, &[issued]);
    strikes.apply(&mut mirror, state.turn);
    assert_eq!(
        mirror.game.cities[&city].defending_districts[0].extra_strikes_used,
        1
    );
}

#[test]
fn new_turn_and_wrong_parent_do_not_replay_an_old_fort_shot() {
    let (mut mirror, state, city, _) = fixture();
    let mut strikes = HostCityStrikes::default();
    let issued = |subject| IssuedOrder {
        kind: "district_strike".into(),
        subject: Some(subject),
        verb: Some("7:4".into()),
        pos: Some((8, 4)),
    };
    strikes.observe(state.turn, &[issued(state.cities[0].id + 1)]);
    strikes.apply(&mut mirror, state.turn);
    assert!(!mirror.game.cities[&city].defending_districts[0].struck);
    strikes.observe(state.turn, &[issued(state.cities[0].id)]);
    strikes.apply(&mut mirror, state.turn + 1);
    assert!(!mirror.game.cities[&city].defending_districts[0].struck);
    strikes.observe(state.turn + 1, &[]);
    strikes.apply(&mut mirror, state.turn + 1);
    assert!(!mirror.game.cities[&city].defending_districts[0].struck);
}

#[test]
fn invalid_source_verbs_never_spend_a_district_shot() {
    for verb in ["7", "7:4:1", "7.0:4", "7:4x", "999999999999:4", ""] {
        assert!(district_strike_source(verb).is_none(), "{verb}");
    }
    assert_eq!(district_strike_source("7:4"), Some((7, 4)));
}
