use super::super::tests::{strategic_wonder_fixture, wonder_plan};
use super::*;

fn monument() -> Item {
    Item::Building {
        building: crate::name!("monument"),
    }
}

/// A Science seat's city whose head is a Monument with `left` Production to
/// go, the heads noted, then displaced by `displacer`.
fn displaced(left: f64, displacer: Item, gene: bool) -> Option<(Game, AdvancedAi, u32)> {
    let (mut g, city) = strategic_wonder_fixture(6_402, "great_library");
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.base.book_pos = 4;
    if gene {
        ai.enable_near_done_work_finishes();
    }
    let c = g.cities.get_mut(&city).unwrap();
    c.queue.clear();
    c.production = 0.0;
    c.production_progress.clear();
    if !g.can_produce(0, city, &monument()) || !g.can_produce(0, city, &displacer) {
        return None;
    }
    g.apply(
        0,
        &Action::Produce {
            city,
            item: monument(),
        },
    )
    .ok()?;
    let cost = g.item_cost_for_city(0, city, &monument());
    g.cities.get_mut(&city).unwrap().production = (cost - left).max(1.0);
    let heads = ai.near_done_heads(&g, 0);
    g.apply(
        0,
        &Action::Produce {
            city,
            item: displacer,
        },
    )
    .ok()?;
    let plan = wonder_plan(GrandStrategy::Science, g.turn);
    ai.claim_near_done_work_finishes(&mut g, 0, &plan, &heads);
    Some((g, ai, city))
}

fn builder() -> Item {
    Item::Unit {
        unit: crate::name!("builder"),
    }
}

#[test]
fn a_near_done_head_comes_back_from_an_ordinary_displacer() {
    let Some((g, _, city)) = displaced(1.0, builder(), true) else {
        panic!("fixture: the city can build a Monument and a Builder");
    };
    assert_eq!(g.cities[&city].queue.first(), Some(&monument()));
    assert!(g.item_invested_production(city, &monument()) > 0.0);
}

#[test]
fn off_the_displacer_keeps_the_queue() {
    let Some((g, _, city)) = displaced(1.0, builder(), false) else {
        panic!("fixture");
    };
    assert_eq!(g.cities[&city].queue.first(), Some(&builder()));
}

#[test]
fn a_head_far_from_done_stays_displaced() {
    let Some((g, _, city)) = displaced(10_000.0, builder(), true) else {
        panic!("fixture");
    };
    assert_eq!(g.cities[&city].queue.first(), Some(&builder()));
}

#[test]
fn an_urgent_displacer_keeps_the_queue() {
    let warrior = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let Some((g, _, city)) = displaced(1.0, warrior.clone(), true) else {
        panic!("fixture");
    };
    assert_eq!(g.cities[&city].queue.first(), Some(&warrior));
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    assert!(AdvancedAi::near_done_displacer_is_urgent(&g, &settler));
    assert!(!AdvancedAi::near_done_displacer_is_urgent(&g, &builder()));
}
