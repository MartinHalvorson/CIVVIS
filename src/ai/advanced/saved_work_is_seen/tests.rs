use super::super::tests::{strategic_wonder_fixture, wonder_plan};
use super::*;

/// A Science seat whose one city sits idle with no bank of its own, and a
/// host reading of saved work on a Warrior one Production short.
fn idle_city_with_host_saved_warrior() -> (Game, u32, Item, f64) {
    let (mut g, city) = strategic_wonder_fixture(6_402, "great_library");
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let saved = g.item_cost_for_city(0, city, &warrior) - 1.0;
    let c = g.cities.get_mut(&city).unwrap();
    c.queue.clear();
    c.production = 0.0;
    c.production_progress.clear();
    let mut rows = BTreeMap::new();
    rows.insert("unit:warrior".to_string(), saved);
    let mut host = BTreeMap::new();
    host.insert(city, rows);
    g.replace_host_saved_work(host);
    (g, city, warrior, saved)
}

fn science_ai(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.base.book_pos = 4;
    if gene {
        ai.enable_saved_work_is_seen();
    }
    ai
}

#[test]
fn off_the_host_reading_never_reaches_the_bank() {
    let (mut g, city, warrior, _) = idle_city_with_host_saved_warrior();
    science_ai(false).see_host_saved_work(&mut g, 0);
    assert_eq!(g.item_invested_production(city, &warrior), 0.0);
}

#[test]
fn on_the_host_reading_enters_the_bank_and_the_resume_path_finishes_it() {
    let (mut g, city, warrior, saved) = idle_city_with_host_saved_warrior();
    let ai = science_ai(true);
    ai.see_host_saved_work(&mut g, 0);
    assert_eq!(g.item_invested_production(city, &warrior), saved);
    let plan = wonder_plan(GrandStrategy::Science, g.turn);
    let counts = ai.counts(&g, 0);
    assert!(ai.resume_city_production(&mut g, 0, city, &plan, &counts));
    assert_eq!(g.cities[&city].queue.first(), Some(&warrior));
    assert_eq!(g.cities[&city].production, saved);
}

#[test]
fn the_queue_head_and_a_rival_city_are_not_banked() {
    let (mut g, city, warrior, saved) = idle_city_with_host_saved_warrior();
    g.apply(
        0,
        &Action::Produce {
            city,
            item: warrior.clone(),
        },
    )
    .unwrap();
    science_ai(true).see_host_saved_work(&mut g, 0);
    assert!(!g.cities[&city]
        .production_progress
        .contains_key("unit:warrior"));
    // The same reading seen from another seat's turn touches nothing.
    let (mut other, city2, warrior2, _) = idle_city_with_host_saved_warrior();
    science_ai(true).see_host_saved_work(&mut other, 1);
    assert_eq!(other.item_invested_production(city2, &warrior2), 0.0);
    let _ = saved;
}

#[test]
fn the_complete_turn_resumes_the_host_saved_work() {
    let (g, city, warrior, saved) = idle_city_with_host_saved_warrior();
    let plan = wonder_plan(GrandStrategy::Science, g.turn);
    let mut board = g.clone();
    let mut ai = science_ai(true);
    ai.plan = Some(plan.clone());
    ai.plan_observed_turn(&mut board, 0);
    assert_eq!(board.cities[&city].queue.first(), Some(&warrior));
    assert_eq!(board.cities[&city].production, saved);
    let mut control = g.clone();
    let mut off = science_ai(false);
    off.plan = Some(plan);
    off.plan_observed_turn(&mut control, 0);
    assert_ne!(
        control.cities[&city].production, saved,
        "without the gene the turn cannot see the saved Warrior"
    );
}
