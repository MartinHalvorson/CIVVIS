use super::*;

#[test]
fn short_sight_arc_is_lazy_directed_and_reused() {
    let globe = build(3);
    assert!(globe.short_sight_arcs.get().is_none());
    let from = globe.positions().next().unwrap();
    let to = globe.ring(from, 2)[0];
    let first = globe.short_sight_arc(from, to, 2).unwrap();
    assert!(std::ptr::eq(
        first,
        globe.short_sight_arc(from, to, 2).unwrap()
    ));
    let reverse = globe.short_sight_arc(to, from, 2).unwrap();
    assert!(!std::ptr::eq(first, reverse));
    assert_eq!(first.positions[0], from);
    assert_eq!(first.positions[first.len - 1], to);
    assert_eq!(reverse.positions[0], to);
    assert_eq!(reverse.positions[reverse.len - 1], from);
}

#[test]
fn unsupported_or_misidentified_short_sight_arc_does_not_poison_cache() {
    let globe = build(2);
    let from = globe.positions().next().unwrap();
    let to = globe.ring(from, 3)[0];
    for steps in [-1, 0, 1, 4, 200] {
        assert!(globe.short_sight_arc(from, to, steps).is_none());
    }
    assert!(globe.short_sight_arcs.get().is_none());
    assert!(globe.short_sight_arc((-999, -999), to, 3).is_none());
    assert!(globe.short_sight_arc(from, (-999, -999), 3).is_none());
    assert!(globe.short_sight_arc(from, to, 2).is_none());
    assert!(globe.short_sight_arc(from, to, 3).is_some());
}

#[test]
fn short_sight_arc_budget_falls_back_without_allocating_rows() {
    let globe = build(2);
    assert!(globe.short_sight_arc_rows(0).is_none());
    let rows = globe.short_sight_arc_rows(SHORT_SIGHT_ARC_BYTES).unwrap();
    assert_eq!(rows.len(), globe.len());
    assert!(rows.iter().all(|row| row.get().is_none()));
    globe.short_sight_arcs.set(None).ok().unwrap();
    let from = globe.positions().next().unwrap();
    assert!(globe
        .short_sight_arc(from, globe.ring(from, 2)[0], 2)
        .is_none());
}
