//! SCRATCH — how often is the garrison-walls danger gate open, and what does
//! the doctrine cost, in native all-lanes games?
use civvis::ai::{run_game, AdvancedAi};
use civvis::game::{Game, GameOptions, VictoryConditions};
use civvis::setup::MapScript;

fn game(seed: u64) -> Game {
    Game::new_with(GameOptions {
        speed: "online".to_string(),
        map_script: MapScript::Pangaea,
        randomize_civs: true,
        victory_conditions: VictoryConditions::default(),
        ..GameOptions::new(4, 60, 38, seed, 250, 6)
    })
}

#[test]
#[ignore]
fn scratch_walls_gate() {
    let handles: Vec<_> = (1..=6u64)
        .map(|seed| {
            std::thread::spawn(move || {
                let mut out = String::new();
                for on in [false, true] {
                    let mut g = game(seed);
                    let mut ais: Vec<AdvancedAi> = (0..g.players.len())
                        .map(|pid| {
                            let mut ai = AdvancedAi::new();
                            if pid == 0 {
                                ai.enable_engine_repairs();
                                if !on {
                                    ai.disable_garrison_walls();
                                }
                            }
                            ai
                        })
                        .collect();
                    run_game(&mut g, &mut ais);
                    let walls = g
                        .cities
                        .values()
                        .filter(|c| {
                            c.owner == 0
                                && c.buildings.iter().any(|b| b.as_str() == "walls")
                        })
                        .count();
                    let cities = g.player_city_ids(0).len();
                    out += &format!(
                        "seed {seed} walls={on:5} → walls {walls}/{cities} score {} win {} victory {:?} t{}\n",
                        g.score(0),
                        g.winner == Some(0),
                        g.victory_type,
                        g.reported_turn()
                    );
                }
                out
            })
        })
        .collect();
    for h in handles {
        print!("{}", h.join().unwrap());
    }
}
