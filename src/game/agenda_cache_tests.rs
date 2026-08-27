use super::*;

#[test]
fn batched_agenda_metrics_preserve_every_pairwise_stance() {
    let mut game = Game::new_with(GameOptions::new(6, 32, 22, 90_003, 80, 0));
    let majors: Vec<usize> = game
        .players
        .iter()
        .filter(|player| player.alive && !player.is_minor && !player.is_barbarian)
        .map(|player| player.id)
        .collect();
    let mut expected = Vec::new();
    for observer in majors.iter().copied() {
        if game.agenda_of(observer).is_none() {
            continue;
        }
        for subject in majors
            .iter()
            .copied()
            .filter(|subject| *subject != observer)
        {
            let opinion = game.agenda_opinion(observer, subject);
            let stance = if opinion >= 15.0 {
                1
            } else if opinion <= -15.0 {
                -1
            } else {
                0
            };
            expected.push((observer, subject, stance));
        }
    }

    game.process_agendas();

    for (observer, subject, stance) in expected {
        assert_eq!(
            game.players[observer]
                .agenda_view
                .get(&subject)
                .copied()
                .unwrap_or(0),
            stance,
            "cached agenda metric changed {observer}'s stance toward {subject}"
        );
    }
}
