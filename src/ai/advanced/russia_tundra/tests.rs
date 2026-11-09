use super::*;
use crate::name::Name;

fn land(g: &Game) -> Vec<Pos> {
    g.map
        .tiles
        .keys()
        .copied()
        .filter(|pos| {
            g.map
                .get(*pos)
                .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
        })
        .collect()
}

fn board(civ: &str) -> Game {
    let mut g = Game::new_full(1, 40, 26, 936_221, 200, 0, false);
    g.players[0].civ = civ.into();
    g
}

fn gene() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_russia_settles_the_tundra();
    ai
}

/// A dry plot with six dry neighbours, so a forecast can work the whole ring.
fn inland_plot(g: &Game) -> Pos {
    land(g)
        .into_iter()
        .find(|pos| {
            let neighbours = g.nbrs(*pos);
            neighbours.len() == 6
                && neighbours.iter().all(|at| {
                    g.map
                        .get(*at)
                        .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
                })
        })
        .expect("the board has an inland plot")
}

fn paint(g: &mut Game, positions: &[Pos], terrain: &str) {
    for pos in positions {
        let tile = g.map.tiles.get_mut(pos).unwrap();
        tile.terrain = Name::new(terrain);
        tile.feature = None;
        tile.resource = None;
        tile.hills = false;
    }
}

/// The plot pays what the shipped modifiers say, hills or not.
#[test]
fn a_russian_seat_reads_one_faith_and_one_production_on_tundra() {
    let mut g = board("Russia");
    let pos = inland_plot(&g);
    paint(&mut g, &[pos], "tundra");
    let ai = gene();
    let flat = ai.mother_russia_tile_yields(&g, 0, &g.map.tiles[&pos]);
    assert_eq!((flat.faith, flat.production), (1.0, 1.0));
    g.map.tiles.get_mut(&pos).unwrap().hills = true;
    let hills = ai.mother_russia_tile_yields(&g, 0, &g.map.tiles[&pos]);
    assert_eq!((hills.faith, hills.production), (1.0, 1.0));
    paint(&mut g, &[pos], "grassland");
    let warm = ai.mother_russia_tile_yields(&g, 0, &g.map.tiles[&pos]);
    assert_eq!((warm.faith, warm.production), (0.0, 0.0));
}

/// Off, and for any other civilization, nothing is added anywhere.
#[test]
fn only_a_russian_seat_with_the_gene_gains_anything() {
    let off = AdvancedAi::new();
    let on = gene();
    let mut russia = board("Russia");
    let mut korea = board("Korea");
    let pos = inland_plot(&russia);
    let ring: Vec<Pos> = russia.nbrs(pos).into_iter().collect();
    paint(&mut russia, &ring, "tundra");
    let pos_k = inland_plot(&korea);
    let ring_k: Vec<Pos> = korea.nbrs(pos_k).into_iter().collect();
    paint(&mut korea, &ring_k, "tundra");
    assert_eq!(off.mother_russia_prefilter_credit(&russia, 0, pos), 0.0);
    assert_eq!(on.mother_russia_prefilter_credit(&korea, 0, pos_k), 0.0);
    assert_eq!(
        off.settlement_static_value_uncached(&korea, 0, pos_k),
        on.settlement_static_value_uncached(&korea, 0, pos_k),
        "another civilization's score is untouched"
    );
}

/// The forecast is where the seat is chosen: a ring of Tundra must be worth
/// more to the gene than to the seat without it, by more than rounding.
#[test]
fn a_tundra_ring_raises_the_forecast_and_the_prefilter() {
    let mut g = board("Russia");
    let pos = inland_plot(&g);
    let ring: Vec<Pos> = g.nbrs(pos).into_iter().collect();
    paint(&mut g, &ring, "tundra");
    let (off, on) = (AdvancedAi::new(), gene());
    let lift = on.settlement_static_value_uncached(&g, 0, pos)
        - off.settlement_static_value_uncached(&g, 0, pos);
    assert!(lift > 10.0, "forecast lift {lift}");
    let shortlist = on.settlement_prefilter_score_for(&g, 0, pos)
        - off.settlement_prefilter_score_for(&g, 0, pos);
    assert!(shortlist > 0.0, "prefilter lift {shortlist}");
}

/// The founding plot takes no credit: what a City Center keeps of the
/// modifier has not been read from the host.
#[test]
fn the_founding_plot_itself_earns_no_credit() {
    let mut g = board("Russia");
    let pos = inland_plot(&g);
    let ring: Vec<Pos> = g.nbrs(pos).into_iter().collect();
    paint(&mut g, &ring, "grassland");
    paint(&mut g, &[pos], "grassland");
    let (off, on) = (AdvancedAi::new(), gene());
    let before = on.settlement_static_value_uncached(&g, 0, pos)
        - off.settlement_static_value_uncached(&g, 0, pos);
    paint(&mut g, &[pos], "tundra");
    let after = on.settlement_static_value_uncached(&g, 0, pos)
        - off.settlement_static_value_uncached(&g, 0, pos);
    assert_eq!(before, 0.0);
    assert_eq!(after, 0.0, "only the worked ring is credited");
}
