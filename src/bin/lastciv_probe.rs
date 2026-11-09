//! PROBE ONLY, never committed: does a played-on stock world reach one civ?
use civvis::game::PlayOnMode;
use civvis::server::{probe_stock_params, Session};
use std::collections::BTreeMap;
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u64 = args[1].parse().unwrap();
    let cap: u32 = args[2].parse().unwrap();
    let wall_s: u64 = args[3].parse().unwrap();
    let mode = PlayOnMode::parse(args.get(4).map(String::as_str).unwrap_or("indefinite")).unwrap();
    let mut s = Session::new(probe_stock_params(seed));
    let t0 = Instant::now();
    while !s.game.is_finished() {
        s.step_quietly();
    }
    println!(
        "{{\"event\":\"first_result\",\"seed\":{seed},\"turn\":{},\"winner\":{:?},\"type\":{:?},\"secs\":{}}}",
        s.game.turn, s.game.winner, s.game.victory_type, t0.elapsed().as_secs()
    );
    assert!(s.play_on(mode));
    let mut last = 0u32;
    let mut last_change_turn = s.game.turn;
    let mut last_cities: BTreeMap<usize, usize> = BTreeMap::new();
    loop {
        s.step_quietly();
        let g = &s.game;
        let majors: Vec<usize> = g
            .players
            .iter()
            .filter(|p| p.alive && !p.is_minor && !p.is_barbarian)
            .map(|p| p.id)
            .collect();
        let mut cities: BTreeMap<usize, usize> = BTreeMap::new();
        for c in g.cities.values() {
            if majors.contains(&c.owner) {
                *cities.entry(c.owner).or_default() += 1;
            }
        }
        if cities != last_cities {
            last_change_turn = g.turn;
            last_cities = cities.clone();
        }
        let done = majors.len() <= 1 || g.is_finished();
        if g.turn >= last + 25 || done || g.turn >= cap {
            last = g.turn;
            let mil: Vec<usize> = majors
                .iter()
                .map(|p| g.units.values().filter(|u| u.owner == *p && g.rules.units.get(&u.kind).is_some_and(|k| k.strength > 0.0)).count())
                .collect();
            let mut wars = 0;
            for a in &majors {
                for b in &majors {
                    if a < b && g.is_at_war(*a, *b) {
                        wars += 1;
                    }
                }
            }
            println!(
                "{{\"event\":\"tick\",\"seed\":{seed},\"turn\":{},\"alive\":{:?},\"cities\":{:?},\"mil\":{:?},\"wars\":{wars},\"since_change\":{},\"finished\":{},\"winner\":{:?},\"type\":{:?},\"secs\":{}}}",
                g.turn, majors, cities.values().collect::<Vec<_>>(), mil, g.turn - last_change_turn, g.is_finished(), g.winner, g.victory_type, t0.elapsed().as_secs()
            );
        }
        if done || g.turn >= cap || t0.elapsed().as_secs() > wall_s {
            break;
        }
    }
}
