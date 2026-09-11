use super::*;

fn fixture(rival_era: usize) -> Game {
    let mut g = Game::new_full(2, 30, 20, 3526, 500, 0, false);
    g.turn = 100;
    g.current = 0;
    g.record_contact(0, 1);
    g.players[0].civics.insert(crate::name!("nationalism"));
    g.players[0].denounced_since.insert(1, 50);
    g.players[0].denounced_until.insert(1, 200);
    for (pid, era) in [(0, 8), (1, rival_era)] {
        let tech = *g
            .rules
            .techs
            .iter()
            .find(|(_, spec)| spec.era == era)
            .unwrap()
            .0;
        g.players[pid].techs.clear();
        g.players[pid].techs.insert(tech);
    }
    g
}

fn colonial() -> Action {
    Action::DeclareWarWithCasusBelli {
        player: 1,
        casus_belli: "colonial_war".into(),
    }
}

#[test]
fn a_hidden_technology_tree_does_not_make_a_peer_eligible_for_colonial_war() {
    let mut g = fixture(8);
    let mut view = g.player_decision_view(0);
    assert!(
        view.players[1].techs.is_empty(),
        "private discoveries remain hidden"
    );
    assert!(g.apply(0, &colonial()).is_err());
    assert!(
        view.apply(0, &colonial()).is_err(),
        "redaction must not invent a two-era advantage over an actual peer"
    );
    assert!(!view
        .legal_actions_within(0, ActionFamilies::DIPLOMACY)
        .contains(&colonial()));
}

#[test]
fn a_real_two_era_advantage_still_allows_the_same_declaration() {
    let mut g = fixture(6);
    let mut view = g.player_decision_view(0);
    assert!(view.players[1].techs.is_empty());
    assert!(view
        .legal_actions_within(0, ActionFamilies::DIPLOMACY)
        .contains(&colonial()));
    assert!(view.apply(0, &colonial()).is_ok());
    assert!(g.apply(0, &colonial()).is_ok());
}

#[test]
fn an_authoritative_observed_era_survives_another_redaction() {
    let mut g = fixture(0);
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .entry(1)
        .or_default()
        .tech_era = Some(8);
    let view = g.player_decision_view(0);
    let mut repeated = view.player_decision_view(0);
    assert_eq!(repeated.observed_public_empire_stats[&1].tech_era, Some(8));
    assert!(repeated.players[1].techs.is_empty());
    assert!(repeated.apply(0, &colonial()).is_err());
}
