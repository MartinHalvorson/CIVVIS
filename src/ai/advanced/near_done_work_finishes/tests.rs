use super::super::tests::{strategic_wonder_fixture, wonder_plan};
use super::*;

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(name),
    }
}

/// The fixture city trains both (`production_commitment`'s fixture checks
/// the same pair): a Warrior head and an ordinary (non-urgent) Builder.
fn ordinary_items(_g: &Game, _city: u32) -> (Item, Item) {
    (unit("warrior"), unit("builder"))
}

/// A Science seat's city whose head has `left` Production to go, the heads
/// noted, then displaced by `displacer` (the fixture's ordinary one when
/// `None`). Returns the board, the city, the head and the displacer.
fn displaced(left: f64, displacer: Option<Item>, gene: bool) -> (Game, u32, Item, Item) {
    displaced_with(left, None, displacer, gene)
}

fn displaced_with(
    left: f64,
    head: Option<Item>,
    displacer: Option<Item>,
    gene: bool,
) -> (Game, u32, Item, Item) {
    let (mut g, city) = strategic_wonder_fixture(6_402, "great_library");
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.base.book_pos = 4;
    if gene {
        ai.enable_near_done_work_finishes();
    }
    {
        let c = g.cities.get_mut(&city).unwrap();
        c.queue.clear();
        c.production = 0.0;
        c.production_progress.clear();
    }
    let (ordinary_head, ordinary) = ordinary_items(&g, city);
    let head = head.unwrap_or(ordinary_head);
    let displacer = displacer.unwrap_or(ordinary);
    assert!(
        g.can_produce(0, city, &head),
        "fixture: {head:?} is producible"
    );
    assert!(
        g.can_produce(0, city, &displacer),
        "fixture: {displacer:?} is producible"
    );
    g.apply(
        0,
        &Action::Produce {
            city,
            item: head.clone(),
        },
    )
    .expect("fixture: the head starts");
    let cost = g.item_cost_for_city(0, city, &head);
    g.cities.get_mut(&city).unwrap().production = (cost - left).max(1.0);
    let heads = ai.near_done_heads(&g, 0);
    assert_eq!(
        heads.get(&city),
        gene.then_some(&head),
        "the head is noted only with the gene"
    );
    g.apply(
        0,
        &Action::Produce {
            city,
            item: displacer.clone(),
        },
    )
    .expect("fixture: the displacer starts");
    let plan = wonder_plan(GrandStrategy::Science, g.turn);
    ai.claim_near_done_work_finishes(&mut g, 0, &plan, &heads);
    (g, city, head, displacer)
}

#[test]
fn a_near_done_head_comes_back_from_an_ordinary_displacer() {
    let (g, city, head, _) = displaced(1.0, None, true);
    assert_eq!(g.cities[&city].queue.first(), Some(&head));
    assert!(g.item_invested_production(city, &head) > 0.0);
}

#[test]
fn off_the_displacer_keeps_the_queue() {
    let (g, city, _, displacer) = displaced(1.0, None, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&displacer));
}

#[test]
fn a_head_far_from_done_stays_displaced() {
    let (g, city, _, displacer) = displaced(10_000.0, None, true);
    assert_eq!(g.cities[&city].queue.first(), Some(&displacer));
}

#[test]
fn an_urgent_displacer_keeps_the_queue() {
    let warrior = unit("warrior");
    let (g, city, _, displacer) =
        displaced_with(1.0, Some(unit("builder")), Some(warrior.clone()), true);
    assert_eq!(displacer, warrior);
    assert_eq!(g.cities[&city].queue.first(), Some(&warrior));
    assert!(AdvancedAi::near_done_displacer_is_urgent(
        &g,
        &unit("settler")
    ));
    assert!(!AdvancedAi::near_done_displacer_is_urgent(
        &g,
        &unit("builder")
    ));
}
