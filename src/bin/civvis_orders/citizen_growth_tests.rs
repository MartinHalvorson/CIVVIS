use super::super::Order;
use super::*;
use civvis::mirror::{Snapshot, StateSnapshot, TilesChunk};

fn observation() -> (Snapshot, StateSnapshot) {
    let chunk: TilesChunk = serde_json::from_value(serde_json::json!({
        "turn":60, "width":44, "height":26, "plots":[
            {"x":13,"y":10,"oc":65536,"yl":[2,2,0,0,0,0]},
            {"x":12,"y":10,"oc":65536,"yl":[1,4,3,0,0,0]},
            {"x":13,"y":9,"oc":65536,"yl":[1,2,0,0,0,0]},
            {"x":12,"y":11,"oc":65536,"yl":[3,1,0,0,0,0]}
        ]
    }))
    .unwrap();
    let state = serde_json::from_value(serde_json::json!({"turn":60,"cities":[{
        "id":65536,"x":13,"y":10,"pop":2,"housing":6,
        "food_surplus":0,"overall_growth_mult":1.19922,
        "damage":0,"wall_damage":0,
        "food_favored":false,"food_disfavored":false,"food_focus_managed":false,
        "worked":[
            {"x":13,"y":10,"yields":{"food":2,"production":2}},
            {"x":12,"y":10,"yields":{"food":1,"production":4}},
            {"x":13,"y":9,"yields":{"food":1,"production":2}}
        ]
    }]}))
    .unwrap();
    (Snapshot::from_chunks(&[chunk]), state)
}

fn orders(snapshot: &Snapshot, state: &StateSnapshot) -> Vec<Order> {
    let mut proposed = Vec::new();
    super::super::append_citizen_growth_orders(snapshot, state, &mut proposed);
    proposed
}

#[test]
fn stalled_small_city_favors_an_observed_food_improvement() {
    let (snapshot, state) = observation();
    let proposed = orders(&snapshot, &state);
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].kind, "city_focus");
    assert_eq!(proposed[0].subject, Some(65536));
    assert_eq!(proposed[0].verb.as_deref(), Some("FAVOR_FOOD"));
}

#[test]
fn unknown_preferences_and_growth_metrics_do_not_invent_a_stall() {
    let (snapshot, state) = observation();
    for change in 0..6 {
        let mut altered = state.clone();
        let city = &mut altered.cities[0];
        match change {
            0 => city.food_favored = None,
            1 => city.food_disfavored = None,
            2 => city.food_focus_managed = None,
            3 => city.food_surplus = -1.0,
            4 => city.overall_growth_mult = -1.0,
            _ => city.housing = None,
        }
        assert!(orders(&snapshot, &altered).is_empty(), "case {change}");
    }
    let legacy: mirror::StateCity = serde_json::from_str(r#"{"x":1,"y":1}"#).unwrap();
    assert_eq!(legacy.food_favored, None);
    assert_eq!(legacy.food_disfavored, None);
    assert_eq!(legacy.food_focus_managed, None);
}

#[test]
fn current_unworked_workable_food_is_required() {
    let (snapshot, state) = observation();
    for quote in [
        serde_json::json!({"x":12,"y":11,"oc":65536,"yl":[1,4,0,0,0,0]}),
        serde_json::json!({"x":12,"y":11,"oc":2,"yl":[3,1,0,0,0,0]}),
        serde_json::json!({"x":12,"y":11,"oc":65536,"d":"DISTRICT_CAMPUS","yl":[3,1,0,0,0,0]}),
        serde_json::json!({"x":12,"y":11,"oc":65536,"p":true,"yl":[3,1,0,0,0,0]}),
        serde_json::json!({"x":12,"y":11,"oc":65536,"i":true,"yl":[3,1,0,0,0,0]}),
        serde_json::json!({"x":12,"y":11,"oc":65536,"yl":[3]}),
    ] {
        let chunk = serde_json::from_value(serde_json::json!({
            "turn":60,"width":44,"height":26,"plots":[quote]
        }))
        .unwrap();
        assert!(orders(&Snapshot::from_chunks(&[chunk]), &state).is_empty());
    }
    let mut stale = state;
    stale.turn += 1;
    assert!(orders(&snapshot, &stale).is_empty());
}

#[test]
fn growth_focus_preserves_defense_housing_and_existing_preferences() {
    let (snapshot, state) = observation();
    for change in 0..8 {
        let mut altered = state.clone();
        let city = &mut altered.cities[0];
        match change {
            0 => city.damage = 1.0,
            1 => city.wall_damage = 1.0,
            2 => city.housing = Some(3.0),
            3 => city.food_surplus = 0.25,
            4 => city.food_disfavored = Some(true),
            5 => city.food_favored = Some(true),
            6 => city.pop = 5,
            _ => altered.turn = 101,
        }
        assert!(orders(&snapshot, &altered).is_empty(), "case {change}");
    }
}

#[test]
fn an_owned_growth_preference_persists_then_releases() {
    let (snapshot, mut state) = observation();
    state.cities[0].food_favored = Some(true);
    state.cities[0].food_focus_managed = Some(true);
    state.cities[0].food_surplus = 3.0;
    assert!(
        orders(&snapshot, &state).is_empty(),
        "positive surplus does not oscillate the focus"
    );
    for change in 0..4 {
        let mut altered = state.clone();
        match change {
            0 => altered.cities[0].housing = Some(3.0),
            1 => altered.cities[0].pop = 6,
            2 => altered.cities[0].damage = 1.0,
            _ => altered.turn = 130,
        }
        let proposed = orders(&snapshot, &altered);
        assert_eq!(proposed.len(), 1, "case {change}");
        assert_eq!(proposed[0].verb.as_deref(), Some("RELEASE_FOOD"));
        altered.cities[0].food_focus_managed = Some(false);
        assert!(
            orders(&snapshot, &altered).is_empty(),
            "do not release a preference set elsewhere"
        );
    }
}

#[test]
fn requests_are_not_verified_until_the_native_preference_changes() {
    let (_, mut after) = observation();
    let mut order = IssuedOrder {
        kind: "city_focus".into(),
        subject: Some(65536),
        verb: Some("FAVOR_FOOD".into()),
        pos: None,
    };
    after.cities[0].food_focus_managed = Some(true);
    assert!(matches!(verify(&order, &after), Verdict::Failed(_)));
    after.cities[0].food_favored = Some(true);
    assert!(matches!(verify(&order, &after), Verdict::Verified));
    order.verb = Some("RELEASE_FOOD".into());
    assert!(matches!(verify(&order, &after), Verdict::Failed(_)));
    after.cities[0].food_favored = Some(false);
    assert!(
        matches!(verify(&order, &after), Verdict::Failed(_)),
        "release ownership also needs readback"
    );
    after.cities[0].food_focus_managed = Some(false);
    assert!(matches!(verify(&order, &after), Verdict::Verified));
    after.cities[0].food_favored = None;
    assert!(matches!(verify(&order, &after), Verdict::Unverifiable));
}
