use super::*;

/// One city of ours at population `pop` on turn `turn`, a plan that wants
/// `desired` cities, and a recording journal attached to a fresh governor.
fn fixture(pop: i32, turn: u32, desired: usize) -> (Game, AdvancedAi, Journal, u32, StrategicPlan) {
    let mut game = Game::new_full(2, 30, 20, 8_121, 120, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    let pos = game.units[&settler].pos;
    game.remove_unit(settler);
    game.found_city_for(0, pos, None);
    let cid = game.player_city_ids(0)[0];
    game.cities.get_mut(&cid).unwrap().pop = pop;
    game.current = 0;
    game.turn = turn;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: desired,
        assessed_turn: turn,
        rush: false,
    };
    let mut ai = AdvancedAi::new();
    let journal = Journal::recording();
    ai.attach_journal(journal.handle());
    (game, ai, journal, cid, plan)
}

fn settler() -> Item {
    Item::Unit {
        unit: crate::name!("settler"),
    }
}

fn monument() -> Item {
    Item::Building {
        building: crate::name!("monument"),
    }
}

fn lines(journal: &Journal) -> Vec<(String, String)> {
    journal
        .since(0)
        .thoughts
        .iter()
        .map(|thought| (thought.headline.clone(), thought.detail.clone()))
        .collect()
}

#[test]
fn an_admitted_settler_names_the_item_that_outscored_it() {
    let (game, ai, journal, cid, plan) = fixture(3, 63, 6);
    let counts = ai.counts(&game, 0);
    ai.journal_settler_admission(
        &game,
        0,
        cid,
        &plan,
        &counts,
        &[settler(), monument()],
        &[61.0, 82.0],
    );
    let lines = lines(&journal);
    assert!(
        lines
            .iter()
            .any(|(head, why)| head.starts_with("Settler held in")
                && why.starts_with("outscored by")
                && why.contains("(82 vs 61)")),
        "{lines:?}"
    );
}

#[test]
fn a_refused_settler_names_the_first_failing_gate() {
    let (game, ai, journal, cid, plan) = fixture(1, 63, 6);
    let counts = ai.counts(&game, 0);
    ai.journal_settler_admission(
        &game,
        0,
        cid,
        &plan,
        &counts,
        &[settler(), monument()],
        &[-10_000.0, 82.0],
    );
    let lines = lines(&journal);
    assert!(
        lines
            .iter()
            .any(|(_, why)| why.starts_with("population 1 is below")),
        "{lines:?}"
    );
}

#[test]
fn no_line_at_the_target_off_the_stride_or_when_the_settler_wins() {
    // At the settlement target.
    let (game, ai, journal, cid, plan) = fixture(3, 63, 1);
    let counts = ai.counts(&game, 0);
    ai.journal_settler_admission(
        &game,
        0,
        cid,
        &plan,
        &counts,
        &[settler(), monument()],
        &[61.0, 82.0],
    );
    assert!(lines(&journal).is_empty());
    // Off the stride.
    let (game, ai, journal, cid, plan) = fixture(3, 64, 6);
    let counts = ai.counts(&game, 0);
    ai.journal_settler_admission(
        &game,
        0,
        cid,
        &plan,
        &counts,
        &[settler(), monument()],
        &[61.0, 82.0],
    );
    assert!(lines(&journal).is_empty());
    // The Settler is the menu's best.
    let (game, ai, journal, cid, plan) = fixture(3, 63, 6);
    let counts = ai.counts(&game, 0);
    ai.journal_settler_admission(
        &game,
        0,
        cid,
        &plan,
        &counts,
        &[settler(), monument()],
        &[90.0, 82.0],
    );
    assert!(lines(&journal).is_empty());
}
