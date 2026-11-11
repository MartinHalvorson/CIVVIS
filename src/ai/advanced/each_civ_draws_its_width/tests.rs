use super::*;

#[test]
fn the_target_walks_two_ahead_of_the_cities_held_to_the_width() {
    // Below the shared ten the plan's own target speaks.
    assert_eq!(stepped_city_target(10, 5, 14), 10);
    assert_eq!(stepped_city_target(10, 8, 14), 10);
    // From nine held it asks for one more than ten, then walks.
    assert_eq!(stepped_city_target(10, 9, 14), 11);
    assert_eq!(stepped_city_target(10, 10, 14), 12);
    assert_eq!(stepped_city_target(10, 13, 14), 14);
    // At the width it asks for no more.
    assert_eq!(stepped_city_target(10, 14, 14), 14);
    assert_eq!(stepped_city_target(10, 12, 12), 12);
    // A width of ten or less never lifts, and a higher plan is kept.
    assert_eq!(stepped_city_target(10, 12, 10), 10);
    assert_eq!(stepped_city_target(15, 11, 13), 15);
}

#[test]
fn a_plan_below_ten_is_not_lifted_early() {
    // Outside the shared contract a plan may want six; the gene must not
    // turn it into an opening expansion driver.
    assert_eq!(stepped_city_target(6, 3, 14), 6);
    assert_eq!(stepped_city_target(6, 8, 14), 6);
}

#[test]
fn the_draw_is_fixed_per_game_and_spreads_over_the_table() {
    let len = CITY_WIDTH_DRAW.len();
    assert_eq!(
        city_width_index(77, 1, "Rome", len),
        city_width_index(77, 1, "Rome", len)
    );
    let mut seen = vec![0usize; len];
    for seed in 0..2_000u64 {
        for pid in 0..4 {
            seen[city_width_index(seed, pid, "Rome", len)] += 1;
        }
    }
    // 8,000 draws over the table's slots: each within a seventh of its
    // equal share.
    let share = 8_000 / len;
    assert!(
        seen.iter()
            .all(|&n| (share - share / 7..=share + share / 7).contains(&n)),
        "{seen:?}"
    );
    // Two majors of one world do not always draw alike.
    let differ = (0..200u64)
        .filter(|&seed| {
            city_width_index(seed, 0, "Rome", len) != city_width_index(seed, 1, "Rome", len)
        })
        .count();
    assert!(differ > 100, "{differ}");
}

#[test]
fn every_drawn_width_reaches_past_the_shared_ten() {
    assert!(CITY_WIDTH_DRAW
        .iter()
        .all(|&width| width > SHARED_CITY_TARGET_FLOOR));
}

#[test]
fn off_the_gene_leaves_the_target_and_the_cap_alone() {
    let g = Game::new_full(4, 32, 20, 7, 200, 0, false);
    let ai = AdvancedAi::new();
    assert_eq!(ai.city_width_target(&g, 0, 10, 11), 10);
    assert_eq!(ai.city_width_science_cap(&g, 0, 12), 12);
    let mut on = AdvancedAi::new();
    on.enable_each_civ_draws_its_width();
    let width = AdvancedAi::drawn_city_width(&g, 0);
    assert!(CITY_WIDTH_DRAW.contains(&width));
    assert_eq!(
        on.city_width_target(&g, 0, 10, 11),
        10.max((11 + 2).min(width))
    );
    assert_eq!(on.city_width_science_cap(&g, 0, 12), 12.max(width));
}
