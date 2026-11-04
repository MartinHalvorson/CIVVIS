use super::*;
use crate::setup::GameSpeed;

const SPEEDS: [GameSpeed; 5] = [
    GameSpeed::Online,
    GameSpeed::Quick,
    GameSpeed::Standard,
    GameSpeed::Epic,
    GameSpeed::Marathon,
];

fn game_at(speed: GameSpeed) -> Game {
    let mut game = Game::new_full(
        4,
        44,
        26,
        crate::rng::fixture_seed("ECONOMYCLOCK", 1),
        250,
        0,
        false,
    );
    game.game_speed = speed;
    game
}

fn clocked(on: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if on {
        ai.enable_economy_clock();
    } else {
        ai.disable_economy_clock();
    }
    ai
}

#[test]
fn the_economy_clock_follows_the_cost_ratio() {
    // Costs at Online are half of Standard's, so an Online turn holds two
    // Standard turns of economy, three Epic and six Marathon ones.
    assert_eq!(economy_turns(GameSpeed::Online, 40), 40);
    assert_eq!(economy_turns(GameSpeed::Quick, 40), 54);
    assert_eq!(economy_turns(GameSpeed::Standard, 40), 80);
    assert_eq!(economy_turns(GameSpeed::Epic, 40), 120);
    assert_eq!(economy_turns(GameSpeed::Marathon, 40), 240);
    assert_eq!(economy_amount(GameSpeed::Online, 100.0), 100.0);
    assert_eq!(economy_amount(GameSpeed::Standard, 100.0), 200.0);
    assert_eq!(economy_amount(GameSpeed::Marathon, 100.0), 600.0);
}

#[test]
fn online_reads_every_window_as_it_did() {
    let game = game_at(GameSpeed::Online);
    let (on, off) = (clocked(true), clocked(false));
    for n in 0..=400 {
        assert_eq!(
            on.econ_window(&game, n),
            game.standard_duration(n),
            "window {n}"
        );
        assert_eq!(on.econ_turns(&game, n), n, "turns {n}");
        assert_eq!(off.econ_window(&game, n), game.standard_duration(n));
    }
    assert_eq!(on.econ_gold(&game, 175.0), 175.0);
}

#[test]
fn off_every_speed_reads_the_old_window() {
    let off = clocked(false);
    for speed in SPEEDS {
        let game = game_at(speed);
        for n in [5, 20, 35, 60, 90, 120, 225] {
            assert_eq!(off.econ_window(&game, n), game.standard_duration(n));
            assert_eq!(off.econ_turns(&game, n), n);
        }
        assert_eq!(off.econ_gold(&game, 175.0), 175.0);
    }
}

#[test]
fn a_slower_game_holds_the_online_economy_time() {
    let on = clocked(true);
    // `early-settler-floor`'s band, written standard_duration(90): 60 turns
    // at Online (the durations table), so 120 at Standard where the old
    // reading gave 90, 180 at Epic and 360 at Marathon.
    let online = game_at(GameSpeed::Online).standard_duration(90);
    assert_eq!(online, 60);
    assert_eq!(on.econ_window(&game_at(GameSpeed::Standard), 90), 120);
    assert_eq!(on.econ_window(&game_at(GameSpeed::Epic), 90), 180);
    assert_eq!(on.econ_window(&game_at(GameSpeed::Marathon), 90), 360);
    // `settler-marches-alone-when-clear`'s raw turn 80 is 160 at Standard.
    assert_eq!(on.econ_turns(&game_at(GameSpeed::Standard), 80), 160);
    // A reserve that bought two Builders at Online buys two at Standard.
    assert_eq!(on.econ_gold(&game_at(GameSpeed::Standard), 175.0), 350.0);
    // Every slower speed holds at least as long as the old reading did.
    for speed in SPEEDS {
        let game = game_at(speed);
        for n in [5, 20, 35, 60, 90, 120, 225] {
            if speed.cost_percent() >= 100 {
                assert!(
                    on.econ_window(&game, n) >= game.standard_duration(n),
                    "{speed:?} {n}"
                );
            }
        }
    }
}

/// An Online game plays byte-identically with the gene on: every converted
/// site reads the same number there, so the deployed controller with and
/// without the gene must leave the same board behind.
#[test]
fn an_online_game_is_byte_identical_under_the_gene() {
    let play = |on: bool| {
        let mut game = game_at(GameSpeed::Online);
        let mut ais: Vec<AdvancedAi> = (0..game.players.len())
            .map(|_| {
                let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
                ai.enable_live_bridge();
                ai
            })
            .collect();
        if on {
            ais[0].enable_economy_clock();
        }
        while game.winner.is_none() && game.turn < 70 {
            let pid = game.current;
            ais[pid].take_turn(&mut game, pid);
            if game.winner.is_none() && game.current == pid {
                let _ = game.apply(pid, &Action::EndTurn);
            }
        }
        serde_json::to_string(&game).expect("the board serialises")
    };
    let (off, on) = (play(false), play(true));
    assert!(off == on, "an Online game diverged under economy-clock");
}
