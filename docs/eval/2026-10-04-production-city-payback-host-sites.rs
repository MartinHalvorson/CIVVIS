use civvis::{
    game::{Game, Item},
    mirror,
};
use std::path::Path;
fn main() {
    let input = std::env::args().nth(1).expect("events path");
    let path = Path::new(&input);
    let snapshot = mirror::snapshot_from_events_at(path, Some(100)).unwrap();
    let state = mirror::state_from_events(path, Some(100)).unwrap();
    let g: Game = mirror::rebuild_from_state(&snapshot, &state, 4, 1, 650, 6).game;
    println!(
        "turn={} income={} apprenticeship={}",
        g.turn,
        g.players[0].gold_per_turn,
        g.players[0]
            .techs
            .contains(&civvis::name!("apprenticeship"))
    );
    for cid in g.player_city_ids(0) {
        let c = &g.cities[&cid];
        let worked = g.city_citizen_plan(cid).worked_tiles;
        let mut net = Vec::new();
        for forecast in g.district_adjacency_calculator(cid) {
            if forecast.family != "industrial_zone" {
                continue;
            }
            for site in forecast.sites {
                let item = Item::District {
                    district: forecast.district,
                    pos: site.pos,
                };
                if !g.can_produce(0, cid, &item) {
                    continue;
                }
                let displaced = if worked.contains(&site.pos) {
                    g.workable_tile_yields(site.pos).production
                } else {
                    0.0
                };
                net.push((
                    site.yields.production - displaced,
                    site.yields.production,
                    displaced,
                    site.pos,
                ));
            }
        }
        net.sort_by(|a, b| b.0.total_cmp(&a.0).then_with(|| a.3.cmp(&b.3)));
        println!(
            "{} pop={} production={:.3} amenity_surplus={} legal_sites={} best_net={:?}",
            c.name,
            c.pop,
            g.city_yields(cid).production,
            g.city_amenity_surplus(c),
            net.len(),
            net.first()
        );
    }
}
