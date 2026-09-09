use civvis::ai::{AdvancedAi, Ai};
use civvis::game::{Action, Game, GameOptions};
use civvis::setup::MapScript;
fn main() {
    println!("seed,version,seat,first_major_contact,met_by_30,explored_by_30,max_scouts_by_30");
    let args: Vec<String> = std::env::args().collect();
    let start: u64 = args
        .get(1)
        .and_then(|x| x.parse().ok())
        .unwrap_or(91_093_001);
    let maps: u64 = args.get(2).and_then(|x| x.parse().ok()).unwrap_or(4);
    for seed in start..start + maps {
        for version in [1, 2] {
            let mut game = Game::new_with(GameOptions {
                speed: "online".to_string(),
                map_script: MapScript::Continents,
                difficulty: "emperor".to_string(),
                randomize_civs: true,
                ..GameOptions::new(6, 74, 46, seed, 250, 9)
            });
            game.set_fog_memory(false);
            game.set_war_ledger(false);
            let mut ais: Vec<_> = (0..game.players.len())
                .map(|pid| {
                    let mut ai = AdvancedAi::new();
                    if pid < 6 {
                        ai.enable_engine_repairs();
                        if version == 1 {
                            ai.enable_early_contact_window();
                        } else {
                            ai.enable_early_contact_window_2();
                        }
                    }
                    ai
                })
                .collect();
            let mut first = [None; 6];
            let mut max_scouts = [0; 6];
            while game.turn <= 30 && game.winner.is_none() {
                for pid in 0..6 {
                    let met = (0..6)
                        .filter(|other| *other != pid && game.has_met(pid, *other))
                        .count();
                    if met > 0 && first[pid].is_none() {
                        first[pid] = Some(game.turn);
                    }
                    let scouts = game
                        .units
                        .values()
                        .filter(|u| u.owner == pid && u.kind == "scout")
                        .count();
                    max_scouts[pid] = max_scouts[pid].max(scouts);
                }
                let pid = game.current;
                ais[pid].take_turn(&mut game, pid);
                if game.winner.is_none() && game.current == pid {
                    game.apply(pid, &Action::EndTurn).expect("end turn");
                }
            }
            for pid in 0..6 {
                let met = (0..6)
                    .filter(|other| *other != pid && game.has_met(pid, *other))
                    .count();
                println!(
                    "{seed},{version},{pid},{},{met},{},{}",
                    first[pid].map_or("unmet".to_string(), |t| t.to_string()),
                    game.players[pid].explored.len(),
                    max_scouts[pid]
                );
            }
        }
    }
}
