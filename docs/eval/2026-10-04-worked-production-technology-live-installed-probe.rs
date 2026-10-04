use civvis::{game::Game, mirror};
use std::path::Path;
fn main() {
    for turn in [50, 75, 100, 125] {
        let path = format!("/tmp/civvis-prod-replay-{turn}/events.jsonl");
        let p = Path::new(&path);
        let snap = mirror::snapshot_from_events_at(p, Some(turn)).unwrap();
        let state = mirror::state_from_events(p, Some(turn)).unwrap();
        let g = mirror::rebuild_from_state(&snap, &state, 4, 1, 250, 6).game;
        let worked: Vec<_> = g
            .player_city_ids(0)
            .iter()
            .flat_map(|cid| g.city_citizen_plan(*cid).worked_tiles)
            .filter(|pos| g.map.tiles[pos].improvement.is_some() && !g.map.tiles[pos].pillaged)
            .collect();
        println!("TURN {turn}: worked improvements={}", worked.len());
        for (tech, spec) in g.rules.techs.iter() {
            if g.players[0].techs.contains(tech)
                || !spec
                    .effects
                    .iter()
                    .any(|(k, v)| k.ends_with("_production") && *v > 0.0)
            {
                continue;
            }
            let mut branch: Game = g.clone();
            branch.players[0].techs.insert(*tech);
            let gain: f64 = worked
                .iter()
                .map(|pos| {
                    (branch.modeled_tile_yields(*pos).production
                        - g.modeled_tile_yields(*pos).production)
                        .max(0.0)
                })
                .sum();
            if gain > 0.0 {
                println!("{} gain={} cost={}", tech, gain, g.tech_cost(tech.as_str()));
            }
        }
    }
}
