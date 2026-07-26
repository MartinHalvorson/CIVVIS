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
    pub fn load(dir: &str) -> Option<ValueNet> {
        let raw = fs::read_to_string(Path::new(dir).join("valuenet.json")).ok()?;
        serde_json::from_str::<ValueNet>(&raw)
            .ok()
            .filter(ValueNet::valid)
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
        let raw = std::fs::read_to_string("evolved/valuenet_fixture.json")
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
