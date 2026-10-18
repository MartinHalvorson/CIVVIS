use super::*;

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_denial_target_leads_the_fronts();
    ai
}

#[test]
fn the_countered_rival_at_war_leads_the_fronts() {
    let ai = armed();
    assert_eq!(
        ai.denial_target_front(Some((2, GrandStrategy::Conquest)), &[1, 2]),
        Some(2)
    );
}

#[test]
fn off_the_chain_is_unchanged() {
    let ai = AdvancedAi::new();
    assert_eq!(
        ai.denial_target_front(Some((2, GrandStrategy::Conquest)), &[1, 2]),
        None
    );
}

#[test]
fn a_rival_we_are_not_fighting_opens_no_war() {
    let ai = armed();
    assert_eq!(
        ai.denial_target_front(Some((3, GrandStrategy::Conquest)), &[1, 2]),
        None
    );
}

#[test]
fn a_counter_not_answered_by_conquest_leaves_the_fronts() {
    let ai = armed();
    assert_eq!(
        ai.denial_target_front(Some((2, GrandStrategy::Culture)), &[1, 2]),
        None
    );
    assert_eq!(ai.denial_target_front(None, &[1, 2]), None);
}
