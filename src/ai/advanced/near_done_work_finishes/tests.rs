use super::super::tests::{strategic_wonder_fixture, wonder_plan};
use super::*;

/// The first ordinary building and the first non-urgent displacer the
/// fixture city can produce.
fn ordinary_items(g: &Game, city: u32) -> (Item, Item) {
    let menu = g.producible_items(0, city);
    let head = menu
        .iter()
        .find(|item| {
            matches!(item, Item::Building { .. })
                && !AdvancedAi::near_done_displacer_is_urgent(g, item)
        })
        .cloned()
        .expect("fixture: an ordinary building is producible");
    let displacer = menu
        .iter()
        .find(|item| {
            **item != head
                && matches!(item, Item::Unit { .. })
                && !AdvancedAi::near_done_displacer_is_urgent(g, item)
        })
        .or_else(|| {
            menu.iter().find(|item| {
                **item != head
                    && matches!(item, Item::Building { .. })
                    && !AdvancedAi::near_done_displacer_is_urgent(g, item)
            })
        })
        .cloned()
        .expect("fixture: a non-urgent displacer is producible");
    (head, displacer)
}

/// A Science seat's city whose head has `left` Production to go, the heads
/// noted, then displaced by `displacer` (the fixture's ordinary one when
/// `None`). Returns the board, the city, the head and the displacer.
fn displaced(left: f64, displacer: Option<Item>, gene: bool) -> (Game, u32, Item, Item) {
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
    let (head, ordinary) = ordinary_items(&g, city);
    let displacer = displacer.unwrap_or(ordinary);
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
        heads.len(),
        usize::from(gene),
        "heads are noted only with the gene"
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
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let (g, city, _, displacer) = displaced(1.0, Some(warrior.clone()), true);
    assert_eq!(displacer, warrior);
    assert_eq!(g.cities[&city].queue.first(), Some(&warrior));
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    assert!(AdvancedAi::near_done_displacer_is_urgent(&g, &settler));
}
