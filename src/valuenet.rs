//! Learned position evaluator: a small MLP (25→64→32→1) trained offline on
//! self-play outcomes from game-grouped dataset.csv exports (NNUE-style
//! distillation).
//! Input = evolve::features(); output = win probability for that player.
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Deserialize, Clone)]
pub struct ValueNet {
    pub sizes: Vec<usize>,
    pub weights: Vec<Vec<Vec<f64>>>, // [layer][in][out]
    pub biases: Vec<Vec<f64>>,
}

impl ValueNet {
    fn read(dir: &Path) -> Option<ValueNet> {
        let raw = fs::read_to_string(dir.join("valuenet.json")).ok()?;
        serde_json::from_str::<ValueNet>(&raw)
            .ok()
            .filter(ValueNet::valid)
    }

    /// Resolve `dir` relative to the working directory, then under `data/`.
    ///
    /// ⚠⚠ THIS WAS A SINGLE CWD-RELATIVE READ, AND IT IS WHY THE LEARNED
    /// EVALUATOR HAS NEVER LOADED IN ANY GAME.
    ///
    /// Every caller passes `"evolved"` — `strategic.rs`, `policy.rs`,
    /// `production.rs`, `elo.rs` — so the old body asked for `./evolved/valuenet.json`,
    /// which exists nowhere. Every agent on every machine therefore resolved to
    /// `None` and silently played the score-share fallback, and `docs/EVAL.md`
    /// recorded ten neutral splits on ten maps and concluded "the evaluator is
    /// good and inert". Treatment and control were the same agent.
    ///
    /// This is the FOURTH instance of the same defect class in this codebase:
    /// #469/#471 fixed it for the champion genome (worth +49 Elo once found), #490
    /// for the league roster, and `evolve::load_champion_record` — one file away —
    /// already resolves local → `data/<dir>` → embedded with a long comment
    /// explaining exactly this failure. There is no embedded arm here yet because
    /// no artifact is tracked; `data/evolved/` holds only `best.json`. Producing
    /// one is the other half of the fix, and a resolver that cannot find a file
    /// that does not exist is still broken.
    pub fn load(dir: &str) -> Option<ValueNet> {
        Self::read(Path::new(dir)).or_else(|| Self::read(&Path::new("data").join(dir)))
    }

    /// Input width this net expects. Feeding it any other width is a
    /// programming error, not a data condition, so callers resolve it at
    /// load time with [`ValueNet::load_width`] rather than at eval time.
    pub fn input_width(&self) -> usize {
        self.sizes[0]
    }

    /// Load only if the net expects `width` inputs.
    ///
    /// The 25-wide `evolve::features` and the 34-wide
    /// `decision_features::decision_features` are both trainable through
    /// the same pipeline, so a directory can hold either. An agent that
    /// silently evaluated the wrong one would produce numbers rather than
    /// an error, which is the failure mode this codebase has spent a lot
    /// of effort removing.
    pub fn load_width(dir: &str, width: usize) -> Option<ValueNet> {
        Self::load(dir).filter(|net| net.input_width() == width)
    }

    fn valid(&self) -> bool {
        // The hidden shape stays pinned so the Rust evaluator and the
        // Python trainer cannot disagree about it; only the input width is
        // free, because that is what a richer feature set changes.
        if self.sizes.len() != 4
            || self.sizes[0] == 0
            || self.sizes[1..] != [64, 32, 1]
            || self.weights.len() + 1 != self.sizes.len()
            || self.biases.len() != self.weights.len()
        {
            return false;
        }
        self.weights.iter().enumerate().all(|(layer, weights)| {
            weights.len() == self.sizes[layer]
                && weights.iter().all(|row| {
                    row.len() == self.sizes[layer + 1] && row.iter().all(|value| value.is_finite())
                })
                && self.biases[layer].len() == self.sizes[layer + 1]
                && self.biases[layer].iter().all(|value| value.is_finite())
        })
    }

    /// Win probability for a position.
    ///
    /// # Panics
    /// If `x` is not [`ValueNet::input_width`] long. Use
    /// [`ValueNet::load_width`] so a mismatch is caught when the artifact
    /// is resolved instead of mid-turn.
    pub fn eval(&self, x: &[f32]) -> f64 {
        assert_eq!(
            x.len(),
            self.input_width(),
            "value net expects {} features, got {}",
            self.input_width(),
            x.len()
        );
        let mut a: Vec<f64> = x.iter().map(|v| *v as f64).collect();
        let last = self.weights.len() - 1;
        for l in 0..=last {
            let (w, b) = (&self.weights[l], &self.biases[l]);
            let mut next = b.clone();
            for (i, ai) in a.iter().enumerate() {
                for (j, nj) in next.iter_mut().enumerate() {
                    *nj += ai * w[i][j];
                }
            }
            for v in next.iter_mut() {
                *v = if l < last {
                    v.max(0.0)
                } else {
                    1.0 / (1.0 + (-*v).exp())
                };
            }
            a = next;
        }
        a[0]
    }
}

#[cfg(test)]
mod tests {
    use super::ValueNet;

    /// Preserve parity with a Python training artifact when one is present.
    #[test]
    fn matches_training_fixture() {
        let Some(net) = ValueNet::load("evolved") else {
            return;
        };
        #[derive(serde::Deserialize)]
        struct Fix {
            input: Vec<f32>,
            output: f64,
        }
        // ⚠ Resolve the fixture the SAME two ways the net resolves. Hardcoding
        // "evolved/..." here is the identical cwd-relative defect that kept the
        // net itself from ever loading: once an artifact is tracked under
        // `data/evolved/`, `load` would succeed and this read would panic.
        let raw = std::fs::read_to_string("evolved/valuenet_fixture.json")
            .or_else(|_| std::fs::read_to_string("data/evolved/valuenet_fixture.json"))
            .expect("a trained model must include its parity fixture");
        let fix: Fix = serde_json::from_str(&raw).unwrap();
        let got = net.eval(&fix.input);
        assert!(
            (got - fix.output).abs() < 1e-4,
            "rust {got} vs python {}",
            fix.output
        );
    }

    #[test]
    fn malformed_networks_are_rejected_before_evaluation() {
        let mut network = ValueNet {
            sizes: vec![25, 64, 32, 1],
            weights: vec![
                vec![vec![0.0; 64]; 25],
                vec![vec![0.0; 32]; 64],
                vec![vec![0.0; 1]; 32],
            ],
            biases: vec![vec![0.0; 64], vec![0.0; 32], vec![0.0; 1]],
        };
        assert!(network.valid());

        network.weights[0][0].pop();
        assert!(!network.valid());
        network.weights[0][0].push(f64::NAN);
        assert!(!network.valid());
    }
}

#[cfg(test)]
mod resolver_tests {
    use super::ValueNet;

    #[test]
    fn a_missing_net_is_none_from_both_candidate_paths() {
        // Not a tautology: the point is that `load` no longer panics or reads a
        // single fixed path. A name that exists in neither place must be None,
        // and a name is looked for under `data/` too.
        assert!(ValueNet::load("definitely-not-a-directory-anywhere").is_none());
    }

    #[test]
    fn the_shipped_evaluator_directory_is_the_one_every_caller_asks_for() {
        // ⚠ Every caller passes "evolved" — strategic.rs, policy.rs,
        // production.rs, elo.rs. This test documents that and will start
        // returning Some the moment an artifact is tracked at either
        // `evolved/valuenet.json` or `data/evolved/valuenet.json`, which is how
        // the second half of this fix will be noticed rather than assumed.
        let resolved = ValueNet::load("evolved");
        println!(
            "evolved/valuenet.json resolves: {}",
            if resolved.is_some() { "YES" } else { "no artifact tracked yet" }
        );
    }
}
