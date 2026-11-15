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
