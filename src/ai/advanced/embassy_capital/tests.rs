use super::*;
use crate::game::{DiplomaticMission, Game};
use crate::{name, Pos};

fn at(col: i32, row: i32) -> Pos {
    crate::hex::offset_to_axial(col, row)
}

/// Three empires on a flat board, met and at peace, at turn 120. We know
/// Diplomatic Service and hold 300 Gold. Each rival's original capital is
/// taken off the board — what the live mirror shows of a capital the seat
/// has never seen — leaving one known city of each, and both rivals read
/// Neutral toward us.
fn board() -> Game {
    let mut g = Game::new_full(3, 48, 24, 7_204_119, 300, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, at(6, 11), None);
    for (rival, capital_col, other_col) in [(1, 28, 24), (2, 42, 38)] {
        let capital = g.found_city_for(rival, at(capital_col, 11), None);
        assert!(
            g.cities[&capital].is_capital,
            "fixture: the rival's first city is its capital"
        );
        g.found_city_for(rival, at(other_col, 11), None);
        g.record_contact(0, rival);
        g.cities.remove(&capital);
        for tile in g.map.tiles.values_mut() {
            if tile.owner_city == Some(capital) {
                tile.owner_city = None;
            }
        }
        attitude(&mut g, rival, "NEUTRAL");
    }
    g.at_war.clear();
    g.players[0].civics.insert(name!("diplomatic_service"));
    g.players[0].gold = 300.0;
    g.turn = 120;
    g.current = 0;
    g
}

fn attitude(g: &mut Game, rival: usize, state: &str) {
    g.players[rival]
        .observed_diplomatic_state
        .insert(0, state.to_string());
}

fn embassy(g: &Game, rival: usize) -> bool {
    g.diplomatic_mission_to(0, rival)
        .is_some_and(|mission| mission.kind == "embassy")
}

fn on() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_embassy_finds_the_capital();
    ai
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    assert!(!AdvancedAi::new().embassy_finds_the_capital);
    let gene = super::super::genes::GENES
        .iter()
        .find(|gene| gene.tag == "embassy-finds-the-capital")
        .expect("registered");
    let mut ai = AdvancedAi::new();
    (gene.enable)(&mut ai);
    assert!(ai.embassy_finds_the_capital);
    (gene.disable)(&mut ai);
    assert!(!ai.embassy_finds_the_capital);
}

#[test]
fn off_nothing_is_sent() {
    let mut g = board();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let gold = g.players[0].gold;
    assert!(ai.embassy_capital_candidates(&g, 0, None).is_empty());
    assert!(!ai.embassy_finds_the_capital_step(&mut g, 0, None));
    assert!(!embassy(&g, 1) && !embassy(&g, 2));
    assert_eq!(g.players[0].gold, gold);
}

#[test]
fn a_neutral_rival_whose_capital_is_unseen_is_sent_an_embassy() {
    let mut g = board();
    let mut ai = on();
    assert_eq!(ai.embassy_capital_candidates(&g, 0, None), vec![1, 2]);
    assert!(ai.embassy_finds_the_capital_step(&mut g, 0, None));
    assert!(embassy(&g, 1), "the first unseen capital's owner is asked");
    assert!(!embassy(&g, 2), "one Embassy a turn");
    assert_eq!(ai.embassy_capital_asks.get(&1), Some(&120));
}

#[test]
fn an_unfriendly_or_denouncing_rival_is_not_asked() {
    let mut g = board();
    let mut ai = on();
    attitude(&mut g, 1, "UNFRIENDLY");
    attitude(&mut g, 2, "DENOUNCED");
    assert!(ai.embassy_capital_candidates(&g, 0, None).is_empty());
    assert!(!ai.embassy_finds_the_capital_step(&mut g, 0, None));

    attitude(&mut g, 2, "FRIENDLY");
    assert!(ai.embassy_finds_the_capital_step(&mut g, 0, None));
    assert!(embassy(&g, 2) && !embassy(&g, 1));
}

#[test]
fn the_plans_target_is_asked_first() {
    let mut g = board();
    let mut ai = on();
    assert_eq!(ai.embassy_capital_candidates(&g, 0, Some(2)), vec![2, 1]);
    assert!(ai.embassy_finds_the_capital_step(&mut g, 0, Some(2)));
    assert!(
        embassy(&g, 2),
        "the stock cadence's denied partner is asked here"
    );
}

#[test]
fn a_favourite_holding_an_embassy_does_not_hold_back_the_next_rival() {
    let mut g = board();
    let mut ai = on();
    g.players[0].diplomatic_missions.insert(
        1,
        DiplomaticMission {
            kind: "embassy".to_string(),
            sent: 104,
        },
    );
    assert_eq!(ai.embassy_capital_candidates(&g, 0, None), vec![2]);
    assert!(ai.embassy_finds_the_capital_step(&mut g, 0, None));
    assert!(embassy(&g, 2));
}

#[test]
fn a_refused_ask_waits_out_the_bridge_retry_and_the_next_rival_goes_first() {
    let mut g = board();
    let mut ai = on();
    assert!(ai.embassy_finds_the_capital_step(&mut g, 0, None));
    assert!(embassy(&g, 1));
    // The host refused it: the next export carries no Embassy.
    g.players[0].diplomatic_missions.remove(&1);
    g.turn = 121;
    assert_eq!(
        ai.embassy_capital_candidates(&g, 0, None),
        vec![2],
        "rival 1 waits out the retry window"
    );
    assert!(ai.embassy_finds_the_capital_step(&mut g, 0, None));
    assert!(embassy(&g, 2));
    g.players[0].diplomatic_missions.remove(&2);
    for turn in 122..120 + EMBASSY_CAPITAL_ASK_TURNS {
        g.turn = turn;
        assert!(
            ai.embassy_capital_candidates(&g, 0, None).is_empty(),
            "turn {turn}: both rivals are inside the retry window"
        );
    }
    g.turn = 120 + EMBASSY_CAPITAL_ASK_TURNS;
    assert_eq!(ai.embassy_capital_candidates(&g, 0, None), vec![1]);
}

#[test]
fn a_known_capital_asks_for_nothing() {
    let mut g = board();
    let mut ai = on();
    for (rival, col) in [(1, 26), (2, 40)] {
        let id = g.found_city_for(rival, at(col, 16), None);
        let city = g.cities.get_mut(&id).unwrap();
        city.is_capital = true;
        city.original_owner = rival;
    }
    assert!(ai.embassy_capital_candidates(&g, 0, None).is_empty());
    assert!(!ai.embassy_finds_the_capital_step(&mut g, 0, None));
}

#[test]
fn war_no_diplomatic_service_or_another_lane_asks_for_nothing() {
    let mut g = board();
    let ai = on();
    g.at_war.insert((0, 1));
    assert_eq!(ai.embassy_capital_candidates(&g, 0, None), vec![2]);
    g.at_war.insert((0, 2));
    assert!(ai.embassy_capital_candidates(&g, 0, None).is_empty());

    let g = board();
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_embassy_finds_the_capital();
    assert!(science.embassy_capital_candidates(&g, 0, None).is_empty());

    let mut g = board();
    g.players[0].civics.remove(&name!("diplomatic_service"));
    assert!(ai.embassy_capital_candidates(&g, 0, None).is_empty());
}

#[test]
fn the_treasury_floor_holds_the_ask() {
    let mut g = board();
    let mut ai = on();
    g.players[0].gold = 20.0;
    assert!(!ai.embassy_finds_the_capital_step(&mut g, 0, None));
    assert!(!embassy(&g, 1));
    assert!(
        ai.embassy_capital_asks.is_empty(),
        "an unsent ask starts no retry window"
    );
}

#[test]
fn a_board_without_the_hosts_reading_asks_any_rival_not_denouncing() {
    let mut g = board();
    let ai = on();
    for rival in [1, 2] {
        g.players[rival].observed_diplomatic_state.clear();
    }
    g.players[2].denounced_until.insert(0, g.turn + 10);
    assert_eq!(ai.embassy_capital_candidates(&g, 0, None), vec![1]);
}
