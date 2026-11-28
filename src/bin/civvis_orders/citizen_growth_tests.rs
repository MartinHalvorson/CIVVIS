use super::super::Order;
use super::*;
use civvis::mirror::{Snapshot, StateSnapshot, TilesChunk};

fn observation() -> (Snapshot, StateSnapshot) {
    observation_at(60)
}

fn observation_at(turn: u32) -> (Snapshot, StateSnapshot) {
    let chunk: TilesChunk = serde_json::from_value(serde_json::json!({
        "turn":turn, "width":44, "height":26, "plots":[
            {"x":13,"y":10,"oc":65536,"yl":[2,2,0,0,0,0]},
            {"x":12,"y":10,"oc":65536,"yl":[1,4,3,0,0,0]},
            {"x":13,"y":9,"oc":65536,"yl":[1,2,0,0,0,0]},
            {"x":12,"y":11,"oc":65536,"yl":[3,1,0,0,0,0]}
        ]
    }))
    .unwrap();
    let state = serde_json::from_value(serde_json::json!({"turn":turn,"cities":[{
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
    orders_widened(snapshot, state, false)
}

fn orders_widened(snapshot: &Snapshot, state: &StateSnapshot, widened: bool) -> Vec<Order> {
    let mut proposed = Vec::new();
    super::super::append_citizen_growth_orders(snapshot, state, widened, false, &mut proposed);
    proposed
}

/// `settler-pump-leans-on-production` on, `food-focus-grows-the-small-city`
/// off.
fn orders_pump(snapshot: &Snapshot, state: &StateSnapshot) -> Vec<Order> {
    let mut proposed = Vec::new();
    super::super::append_citizen_growth_orders(snapshot, state, false, true, &mut proposed);
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

/// `food-focus-grows-the-small-city`: a city growing at +2 a turn is not
/// stalled, so only the widened request favours its Food. Above +2, at six
/// citizens, or below zero, neither does.
#[test]
fn the_widened_request_favors_a_slow_small_city() {
    let (snapshot, state) = observation();
    let mut slow = state.clone();
    slow.cities[0].food_surplus = 2.0;
    slow.cities[0].pop = 5;
    slow.cities[0].housing = Some(9.0);
    assert!(orders(&snapshot, &slow).is_empty(), "off: +2 is no stall");
    let proposed = orders_widened(&snapshot, &slow, true);
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].verb.as_deref(), Some("FAVOR_FOOD"));
    for (surplus, pop) in [(2.5, 5), (2.0, 6), (-1.0, 3)] {
        let mut other = state.clone();
        other.cities[0].food_surplus = surplus;
        other.cities[0].pop = pop;
        other.cities[0].housing = Some(9.0);
        assert!(
            orders_widened(&snapshot, &other, true).is_empty(),
            "surplus {surplus}, pop {pop}"
        );
    }
    // The stalled city is still served with the gene on.
    assert_eq!(orders_widened(&snapshot, &state, true).len(), 1);
}

/// `food-focus-grows-the-small-city` v2: the settler pump keeps its
/// Production. A slow small city training a Settler gets no widened request,
/// and a growing focused city releases when a Settler starts; a stalled one
/// (zero surplus) keeps #4003's focus.
#[test]
fn the_widened_request_spares_the_settler_pump() {
    let (snapshot, state) = observation();
    let mut pump = state.clone();
    pump.cities[0].food_surplus = 2.0;
    pump.cities[0].pop = 3;
    pump.cities[0].producing = Some("UNIT_SETTLER".into());
    assert!(orders_widened(&snapshot, &pump, true).is_empty());
    pump.cities[0].producing = Some("UNIT_WARRIOR".into());
    assert_eq!(orders_widened(&snapshot, &pump, true).len(), 1);

    let mut focused = state.clone();
    focused.cities[0].food_favored = Some(true);
    focused.cities[0].food_focus_managed = Some(true);
    focused.cities[0].food_surplus = 3.0;
    focused.cities[0].producing = Some("UNIT_SETTLER".into());
    let proposed = orders_widened(&snapshot, &focused, true);
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].verb.as_deref(), Some("RELEASE_FOOD"));
    assert!(orders(&snapshot, &focused).is_empty(), "off: no release");
    focused.cities[0].food_surplus = 0.0;
    assert!(
        orders_widened(&snapshot, &focused, true).is_empty(),
        "a stalled city keeps its focus"
    );
}

/// `food-focus-grows-the-small-city` v3: the capital is the settler pump for
/// the whole opening, whatever it is training. It gets no widened request
/// and releases a growing focus through turn `CAPITAL_PUMP_UNTIL`; a stalled
/// capital keeps #4003's focus, and afterwards it is a city like any other.
#[test]
fn the_widened_request_spares_the_capital_in_the_opening() {
    let (snapshot, state) = observation();
    let mut capital = state.clone();
    capital.cities[0].capital = true;
    capital.cities[0].food_surplus = 2.0;
    capital.cities[0].pop = 3;
    capital.cities[0].producing = Some("UNIT_WARRIOR".into());
    assert!(
        orders_widened(&snapshot, &capital, true).is_empty(),
        "the opening capital gets no widened request"
    );
    let mut town = capital.clone();
    town.cities[0].capital = false;
    assert_eq!(orders_widened(&snapshot, &town, true).len(), 1);

    capital.cities[0].food_surplus = 0.0;
    assert_eq!(
        orders_widened(&snapshot, &capital, true).len(),
        1,
        "a stalled capital keeps the zero-surplus request"
    );

    let mut focused = capital.clone();
    focused.cities[0].food_favored = Some(true);
    focused.cities[0].food_focus_managed = Some(true);
    focused.cities[0].food_surplus = 3.0;
    let proposed = orders_widened(&snapshot, &focused, true);
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].verb.as_deref(), Some("RELEASE_FOOD"));
    assert!(orders(&snapshot, &focused).is_empty(), "off: no release");

    let (later, state) = observation_at(CAPITAL_PUMP_UNTIL + 1);
    let mut grown = state.clone();
    grown.cities[0].capital = true;
    grown.cities[0].food_surplus = 2.0;
    grown.cities[0].pop = 3;
    let proposed = orders_widened(&later, &grown, true);
    assert_eq!(proposed.len(), 1, "past the opening it is served again");
    assert_eq!(proposed[0].verb.as_deref(), Some("FAVOR_FOOD"));
}

/// Seat p3's capital on turn 11 of `hotseat-20261128T031456Z`, as its first
/// `state` frame exported it (the fields the rule reads): four citizens, a
/// Settler half built at 4 Production a turn on a Food surplus of 7, no
/// citizen preference set. It started the 40-Production Settler on turn 7
/// and was still training it on turn 16.
fn cree_capital_turn_11() -> (Snapshot, StateSnapshot) {
    let state: StateSnapshot = serde_json::from_value(serde_json::json!({"turn":11,"cities":[{
        "id":65536,"name":"Mikisiw-Wac\u{ee}hk","x":14,"y":13,"pop":4,"capital":true,
        "production":4,"production_cost":40,"production_progress":20,
        "producing":"UNIT_SETTLER","food_surplus":7,"housing":7,
        "damage":0,"wall_damage":0,"overall_growth_mult":1,
        "food_favored":false,"food_disfavored":false,"food_focus_managed":false
    }]}))
    .unwrap();
    let chunk: TilesChunk = serde_json::from_value(serde_json::json!({
        "turn":11, "width":44, "height":26, "plots":[]
    }))
    .unwrap();
    (Snapshot::from_chunks(&[chunk]), state)
}

#[test]
fn the_slow_settler_pump_asks_for_a_production_focus() {
    let (snapshot, state) = cree_capital_turn_11();
    assert!(
        orders(&snapshot, &state).is_empty(),
        "off, the Cree capital is asked nothing: +7 Food is no stall"
    );
    assert!(orders_widened(&snapshot, &state, true).is_empty());
    let proposed = orders_pump(&snapshot, &state);
    assert_eq!(proposed.len(), 1);
    assert_eq!(proposed[0].kind, "city_focus");
    assert_eq!(proposed[0].subject, Some(65536));
    assert_eq!(proposed[0].verb.as_deref(), Some("FAVOR_PRODUCTION"));
    // Exactly at both edges it still asks.
    let mut edge = state.clone();
    edge.cities[0].food_surplus = SETTLER_PUMP_MIN_FOOD;
    edge.cities[0].production = SETTLER_PUMP_MAX_PRODUCTION;
    assert_eq!(orders_pump(&snapshot, &edge).len(), 1);
}

#[test]
fn only_a_slow_settler_pump_with_food_to_spare_and_no_preference_asks() {
    let (snapshot, state) = cree_capital_turn_11();
    for change in 0..9 {
        let mut altered = state.clone();
        let city = &mut altered.cities[0];
        match change {
            0 => city.producing = Some("UNIT_WARRIOR".into()),
            1 => city.producing = None,
            2 => city.production = 5.5,
            3 => city.food_surplus = 3.5,
            4 => city.food_favored = Some(true),
            5 => city.food_favored = None,
            6 => city.production_favored = Some(true),
            7 => city.production = f64::NAN,
            _ => city.food_surplus = f64::NAN,
        }
        assert!(orders_pump(&snapshot, &altered).is_empty(), "case {change}");
    }
}

#[test]
fn an_owned_production_focus_holds_until_the_settler_is_done() {
    let (snapshot, mut state) = cree_capital_turn_11();
    {
        let city = &mut state.cities[0];
        city.production_favored = Some(true);
        city.production_focus_managed = Some(true);
        // The focus traded Food for Production: past both thresholds now.
        city.production = 8.0;
        city.food_surplus = 2.0;
    }
    assert!(
        orders_pump(&snapshot, &state).is_empty(),
        "while the Settler trains the focus stays and is not asked again"
    );
    for change in 0..3 {
        let mut altered = state.clone();
        let city = &mut altered.cities[0];
        match change {
            0 => city.producing = Some("BUILDING_MONUMENT".into()),
            1 => city.producing = None,
            _ => city.food_surplus = -1.0,
        }
        let proposed = orders_pump(&snapshot, &altered);
        assert_eq!(proposed.len(), 1, "case {change}");
        assert_eq!(proposed[0].verb.as_deref(), Some("RELEASE_PRODUCTION"));
        assert!(orders(&snapshot, &altered).is_empty(), "off: no release");
        altered.cities[0].production_focus_managed = Some(false);
        assert!(
            orders_pump(&snapshot, &altered).is_empty(),
            "a Production preference set elsewhere is never released: case {change}"
        );
    }
}

#[test]
fn a_production_focus_is_read_back_from_its_own_pair() {
    let (_, mut after) = cree_capital_turn_11();
    let mut order = IssuedOrder {
        kind: "city_focus".into(),
        subject: Some(65536),
        verb: Some("FAVOR_PRODUCTION".into()),
        pos: None,
    };
    // Today's mod takes no Production focus and exports no readback for it.
    assert_eq!(
        verify(&order, &after),
        Verdict::Failed("production_focus_not_exported".into())
    );
    after.cities[0].production_favored = Some(false);
    after.cities[0].production_focus_managed = Some(false);
    assert_eq!(
        verify(&order, &after),
        Verdict::Failed("production_focus_unchanged".into())
    );
    after.cities[0].production_favored = Some(true);
    after.cities[0].production_focus_managed = Some(true);
    assert_eq!(verify(&order, &after), Verdict::Verified);
    order.verb = Some("RELEASE_PRODUCTION".into());
    assert!(matches!(verify(&order, &after), Verdict::Failed(_)));
    after.cities[0].production_favored = Some(false);
    after.cities[0].production_focus_managed = Some(false);
    assert_eq!(verify(&order, &after), Verdict::Verified);
    // A Food order is still read from the Food pair.
    order.verb = Some("FAVOR_FOOD".into());
    assert_eq!(
        verify(&order, &after),
        Verdict::Failed("food_focus_unchanged".into())
    );
}
