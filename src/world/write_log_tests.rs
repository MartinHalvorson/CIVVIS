use super::*;

fn grid() -> TileGrid {
    TileGrid::new(8, 6)
}

#[test]
fn an_unwritten_grid_reports_nothing_written() {
    let g = grid();
    assert_eq!(g.writes_since(g.write_chain()), Some(Vec::new()));
}

#[test]
fn single_tile_writes_are_listed_in_order_and_move_the_epoch() {
    let mut g = grid();
    let (a, b) = (*g.keys().next().unwrap(), *g.keys().nth(9).unwrap());
    let (chain, epoch) = (g.write_chain(), g.epoch());
    g.get_mut(&a).unwrap().hills = true;
    g.get_mut(&b);
    g.get_mut(&a);
    assert_eq!(g.epoch(), epoch + 3, "every route still moves the epoch");
    assert_eq!(g.writes_since(chain), Some(vec![a, b, a]));
    let middle = {
        let mut probe = grid();
        probe.get_mut(&a);
        probe.write_chain()
    };
    assert_eq!(
        g.writes_since(middle),
        Some(vec![b, a]),
        "the same history from the same start reaches the same chain"
    );
}

#[test]
fn a_whole_map_route_cannot_be_told() {
    let mut g = grid();
    let a = *g.keys().next().unwrap();
    let chain = g.write_chain();
    g.get_mut(&a);
    let _ = g.values_mut();
    assert_eq!(g.writes_since(chain), None);
    let chain = g.write_chain();
    for (_, tile) in &mut g {
        tile.hills = false;
    }
    assert_eq!(g.writes_since(chain), None, "the mutable iterator too");
}

#[test]
fn another_branch_of_a_cloned_map_cannot_be_told() {
    let mut trunk = grid();
    let (a, b) = (*trunk.keys().next().unwrap(), *trunk.keys().nth(3).unwrap());
    let mut branch = trunk.clone();
    branch.get_mut(&a);
    trunk.get_mut(&b);
    assert_eq!(
        trunk.epoch(),
        branch.epoch(),
        "the epochs agree and mean nothing"
    );
    assert_eq!(trunk.writes_since(branch.write_chain()), None);
    assert_eq!(branch.writes_since(trunk.write_chain()), None);
}

#[test]
fn a_reader_further_behind_than_the_log_is_told_nothing_is_known() {
    let mut g = grid();
    let a = *g.keys().next().unwrap();
    let chain = g.write_chain();
    for _ in 0..WRITE_LOG + 1 {
        g.get_mut(&a);
    }
    assert_eq!(g.writes_since(chain), None);
    let recent = g.write_chain();
    g.get_mut(&a);
    assert_eq!(g.writes_since(recent), Some(vec![a]));
}

#[test]
fn differing_positions_names_only_the_tiles_whose_content_changed() {
    let base = grid();
    let mut g = base.clone();
    assert_eq!(
        g.differing_positions(&base),
        Some(Vec::new()),
        "a grid still sharing its tiles differs nowhere"
    );
    let (a, b) = (*g.keys().next().unwrap(), *g.keys().nth(9).unwrap());
    g.get_mut(&a).unwrap().hills = true;
    g.get_mut(&b);
    assert_eq!(
        g.differing_positions(&base),
        Some(vec![a]),
        "a tile opened for writing but left as it was has not changed"
    );
    assert_eq!(
        TileGrid::new(9, 6).differing_positions(&base),
        None,
        "a grid of another map is not compared"
    );
}

#[test]
fn sort_positions_is_sort_then_dedup() {
    let g = TileGrid::new(20, 12);
    let keys: Vec<Pos> = g.keys().copied().collect();
    // Long enough for the bit set, out of order and with repeats.
    let mut long: Vec<Pos> = keys.iter().rev().step_by(2).copied().collect();
    long.extend(keys.iter().step_by(3).copied());
    long.extend(keys.iter().step_by(3).copied());
    assert!(long.len() >= 96, "the grid gives the bit set something to do");
    let mut expected = long.clone();
    expected.sort_unstable();
    expected.dedup();
    g.sort_positions(&mut long);
    assert_eq!(long, expected);
    // Short, and with a position off the map: the comparison sort.
    let mut short = vec![keys[5], keys[1], keys[5]];
    g.sort_positions(&mut short);
    assert_eq!(short, vec![keys[1], keys[5]]);
    let mut off = long.clone();
    off.push((-99, -99));
    let mut sorted_off = off.clone();
    sorted_off.sort_unstable();
    sorted_off.dedup();
    g.sort_positions(&mut off);
    assert_eq!(off, sorted_off);
}
