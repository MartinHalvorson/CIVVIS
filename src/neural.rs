//! NeuralAi: an experimental value-net-guided war decision on top of BasicAi.
//!
//! It clones peace and war branches, rolls each forward with scripted AIs, and
//! lets a compatible 25-feature state-value net judge unresolved endpoints.
//! No value net ships with CIVVIS; the `neural` builtin factory therefore
//! returns champion-weight `BasicAi` in a normal checkout. This module is a
//! research surface, not the production major-civilization controller.
use crate::ai::{Ai, BasicAi, Weights};
use crate::evolve::features;
use crate::game::{Action, Game};
use crate::valuenet::ValueNet;

pub struct NeuralAi {
    base: BasicAi,
    net: ValueNet,
    every: u32,   // reconsider war every N turns
    horizon: u32, // rollout depth in game rounds
}

impl NeuralAi {
    pub fn new(mut w: Weights, net: ValueNet) -> NeuralAi {
        w.war_ratio = 99.0; // the scripted layer never declares; the net does
        NeuralAi {
            base: BasicAi::with_weights(w),
            net,
            every: 4,
            horizon: 12,
        }
    }

    /// Win probability after playing `horizon` rounds out with default AIs.
    fn rollout(&self, g: &Game, pid: usize) -> f64 {
        let mut sim = g.clone();
        let mut ais = BasicAi::fleet(&sim);
        let stop = sim.turn + self.horizon;
        while sim.winner.is_none() && sim.turn < stop {
            let p = sim.current;
            ais[p].take_turn(&mut sim, p);
            if sim.winner.is_none() && sim.current == p {
                let _ = sim.apply(p, &Action::EndTurn);
            }
        }
        match sim.winner {
            Some(w) if w == pid => 1.0,
            Some(_) => 0.0,
            None => self.net.eval(&features(&sim, pid)),
        }
    }

    fn war_candidates(g: &Game, pid: usize) -> Vec<usize> {
        g.players
            .iter()
            .filter(|other| {
                other.id != pid
                    && other.alive
                    && !other.is_minor
                    && !other.is_barbarian
                    && g.has_met(pid, other.id)
            })
            .map(|other| other.id)
            .collect()
    }

    fn consider_war(&mut self, g: &mut Game, pid: usize) {
        if !g.turn.is_multiple_of(self.every) || g.player_city_ids(pid).len() < 2 {
            return;
        }
        let others = Self::war_candidates(g, pid);
        if others.is_empty() || others.iter().any(|o| g.is_at_war(pid, *o)) {
            return;
        }
        let peace = self.rollout(g, pid);
        let mut best: Option<(f64, usize)> = None;
        for o in others {
            let mut sim = g.clone();
            if sim.apply(pid, &Action::DeclareWar { player: o }).is_err() {
                continue;
            }
            let v = self.rollout(&sim, pid);
            if best.map(|(b, _)| v > b).unwrap_or(true) {
                best = Some((v, o));
            }
        }
        if let Some((v, o)) = best {
            if v > peace + 0.03 {
                let _ = g.apply(pid, &Action::DeclareWar { player: o });
            }
        }
    }
}

impl Ai for NeuralAi {
    fn take_turn(&mut self, g: &mut Game, pid: usize) {
        if !g.players[pid].is_minor && !g.players[pid].is_barbarian && g.winner.is_none() {
            self.consider_war(g, pid);
        }
        self.base.take_turn(g, pid);
    }

    /// The scripted layer underneath does the day-to-day reasoning, so the
    /// observer's log reads from there. The rollouts above run over clones,
    /// whose journals are silent by construction.
    fn attach_journal(&mut self, journal: crate::reasoning::Journal) {
        self.base.attach_journal(journal);
    }
}

#[cfg(test)]
mod tests {
    use super::NeuralAi;
    use crate::ai::{Ai, BasicAi, Weights};
    use crate::game::{Action, Game};
    use crate::valuenet::ValueNet;

    /// The value net only judges war; the scripted layer underneath does the
    /// day-to-day reasoning. An attached journal has to reach that layer, or
    /// a seated `neural` agent is completely blank in the observer's panel.
    #[test]
    fn an_attached_journal_reaches_the_scripted_layer() {
        let journal = crate::reasoning::Journal::recording();
        let net = ValueNet {
            sizes: vec![25, 1],
            weights: vec![vec![vec![0.0]; 25]],
            biases: vec![vec![0.0]],
        };
        let mut agent = NeuralAi::new(Weights::default(), net);
        agent.attach_journal(journal.handle());
        let mut g = Game::new(2, 24, 16, 8_100, 90, 0);
        let mut others = BasicAi::fleet(&g);
        while g.winner.is_none() && g.turn <= 12 {
            let pid = g.current;
            if pid == 0 {
                agent.take_turn(&mut g, pid);
            } else {
                others[pid].take_turn(&mut g, pid);
            }
            if g.winner.is_none() && g.current == pid {
                let _ = g.apply(pid, &Action::EndTurn);
            }
        }
        assert!(
            journal.since(0).thoughts.iter().any(|t| t.player == 0),
            "the seat run by the neural wrapper never recorded a thought"
        );
    }

    #[test]
    fn war_candidates_include_only_met_major_civilizations() {
        let mut game = Game::new_full(3, 24, 16, 90_735, 120, 0, false);
        for player in 0..game.players.len() {
            game.players[player].met.clear();
        }
        game.record_contact(0, 2);

        assert_eq!(NeuralAi::war_candidates(&game, 0), vec![2]);
    }
}
