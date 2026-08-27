use std::collections::BTreeSet;

use serde::Deserialize;

/// One seat in today's ranking. Unknown keys are rejected so a column added
/// upstream (the sheet has several waiting empty) must be added here
/// deliberately rather than riding in unvalidated.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Nation {
    rank: usize,
    nation: String,
    leader: String,
    latitude: f64,
    longitude: f64,
    bias: String,
    wonders: u32,
}

#[derive(Deserialize)]
struct NationsToday {
    #[serde(rename = "_source")]
    _source: String,
    #[serde(rename = "_note")]
    _note: String,
    date: String,
    roster: Vec<Nation>,
}

#[derive(Deserialize)]
struct CityState {
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Deserialize)]
struct CityStates {
    roster: Vec<CityState>,
}

/// The ordering is the content: rank r must be the roster's r-th entry, every
/// nation must appear once, and every seat must carry a leader and a place on
/// the globe, because map generation seats the file top to bottom.
#[test]
fn todays_ranking_is_contiguous_named_and_on_the_globe() {
    let doc: NationsToday =
        serde_json::from_str(include_str!("../../data/nations_today.json")).unwrap();

    assert!(!doc.roster.is_empty(), "an empty ranking seats nobody");
    let mut nations = BTreeSet::new();
    for (index, seat) in doc.roster.iter().enumerate() {
        assert_eq!(
            seat.rank,
            index + 1,
            "rank {} sits at roster position {}",
            seat.rank,
            index
        );
        assert!(
            nations.insert(seat.nation.as_str()),
            "{} is ranked twice",
            seat.nation
        );
        assert!(
            !seat.nation.trim().is_empty(),
            "rank {} has no nation",
            seat.rank
        );
        assert!(
            !seat.leader.trim().is_empty(),
            "{} has no leader",
            seat.nation
        );
        assert!(
            (-90.0..=90.0).contains(&seat.latitude),
            "{} sits at latitude {}",
            seat.nation,
            seat.latitude
        );
        assert!(
            (-180.0..=180.0).contains(&seat.longitude),
            "{} sits at longitude {}",
            seat.nation,
            seat.longitude
        );
        let _ = seat.wonders;
    }
}

/// A nation past the civilization cut seats as a city-state of its `bias`
/// type, so every bias must name a type `data/city_states.json` actually has —
/// the same vocabulary check the engine would otherwise fail at seating time.
#[test]
fn every_bias_is_a_real_city_state_type() {
    let doc: NationsToday =
        serde_json::from_str(include_str!("../../data/nations_today.json")).unwrap();
    let city_states: CityStates =
        serde_json::from_str(include_str!("../../data/city_states.json")).unwrap();
    let types: BTreeSet<&str> = city_states
        .roster
        .iter()
        .map(|entry| entry.kind.as_str())
        .collect();

    for seat in &doc.roster {
        assert!(
            types.contains(seat.bias.as_str()),
            "{} has bias {:?}, which no city-state type spells",
            seat.nation,
            seat.bias
        );
    }
}

/// The file is rewritten whole each day; the date stamp is what says which
/// day's ranking a checkout is holding.
#[test]
fn the_ranking_is_dated() {
    let doc: NationsToday =
        serde_json::from_str(include_str!("../../data/nations_today.json")).unwrap();
    let parts: Vec<&str> = doc.date.split('-').collect();
    assert_eq!(parts.len(), 3, "date {:?} is not YYYY-MM-DD", doc.date);
    assert_eq!(parts[0].len(), 4);
    assert!(parts
        .iter()
        .all(|part| part.chars().all(|c| c.is_ascii_digit())));
}
