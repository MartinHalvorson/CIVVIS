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

/// What a directory holds, distinguishing "no artifact here" from "an
/// artifact that will not load".
///
/// The difference decides whether resolution continues, so it cannot be
/// collapsed into `Option`. See [`ValueNet::load`].
enum Artifact {
    Missing,
    Invalid,
    Net(ValueNet),
}

fn read_net(dir: &Path) -> Artifact {
    let Ok(raw) = fs::read_to_string(dir.join("valuenet.json")) else {
        return Artifact::Missing;
    };
    match serde_json::from_str::<ValueNet>(&raw) {
        Ok(net) if net.valid() => Artifact::Net(net),
        _ => Artifact::Invalid,
    }
}

impl ValueNet {
    /// Read a net from `dir`, falling back to a committed snapshot under
    /// `data/`.
    ///
    /// This is the resolution [`crate::evolve::load_champion`] already uses
    /// for the genome, and it is here for the same reason. The previous body
    /// was a single read of `<dir>/valuenet.json` against the **current
    /// working directory**, so whether an agent had a learned evaluator at all
    /// depended on where its process happened to be started — and nothing in
    /// the tree is tracked at that path, so in practice every agent on every
    /// machine resolved to `None` and played the score-share fallback. That is
    /// the defect `#469`/`#471` fixed for the champion genome and `#490` fixed
    /// for the league roster, still live for the net.
    ///
    /// A local `<dir>/valuenet.json` still wins, so an in-progress training
    /// run is never shadowed by a committed snapshot.
    ///
    /// **A present-but-unloadable artifact stops resolution.** Only `Missing`
    /// continues to the next tier. Falling through on `Invalid` would hand an
    /// experimenter a *different* net than the one they placed while
    /// `elo::builtin_provenance` reported a net found — the silent substitution
    /// this repository keeps having to undo.
    ///
    /// There is deliberately **no embedded tier**. `include_str!` needs the
    /// artifact at compile time, and whether any particular net should ship is
    /// a strength question with its own paired run, not a consequence of fixing
    /// a path. The live path does not need one today: the spectator supervisor
    /// runs with a checkout root as its working directory, so `data/` resolves
    /// there. A binary copied somewhere without the tree still gets `None`, and
    /// that is the remaining tier to add if and when an artifact is committed.
    pub fn load(dir: &str) -> Option<ValueNet> {
        Self::load_under(Path::new(""), dir)
    }

    /// [`ValueNet::load`] with an explicit base directory.
    ///
    /// Production passes `""`, which `Path::join` leaves as the bare relative
    /// path the old body used. Tests pass a temporary base so the fallback
    /// order can be exercised without a process-wide `chdir`, which would make
    /// them unsafe to run in parallel with everything else in this crate.
    fn load_under(base: &Path, dir: &str) -> Option<ValueNet> {
        match read_net(&base.join(dir)) {
            Artifact::Net(net) => Some(net),
            Artifact::Invalid => None,
            Artifact::Missing => match read_net(&base.join("data").join(dir)) {
                Artifact::Net(net) => Some(net),
                _ => None,
            },
        }
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
    ///
    /// The width filter is applied **after** resolution, so this deliberately
    /// does not go looking for a differently-shaped net in the next tier. A run
    /// seats `strategic` (25-wide) and `policy_wide` (34-wide) from the same
    /// directory name; width-shopping across tiers would let those two agents
    /// read two different artifacts in one run, and the resulting numbers would
    /// be filed under one provenance line.
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
    use super::{read_net, Artifact, ValueNet};
    use std::path::{Path, PathBuf};

    /// A structurally valid net of the requested input width, as JSON.
    ///
    /// `ValueNet` derives only `Deserialize`, so a test fixture has to be
    /// written out rather than serialized.
    fn net_json(width: usize) -> String {
        let plane = |rows: usize, cols: usize| {
            let row = format!("[{}]", vec!["0.0"; cols].join(","));
            format!("[{}]", vec![row; rows].join(","))
        };
        let bias = |n: usize| format!("[{}]", vec!["0.0"; n].join(","));
        format!(
            "{{\"sizes\":[{width},64,32,1],\"weights\":[{},{},{}],\"biases\":[{},{},{}]}}",
            plane(width, 64),
            plane(64, 32),
            plane(32, 1),
            bias(64),
            bias(32),
            bias(1)
        )
    }

    /// A fresh base directory, so these can run beside every other test in the
    /// crate without a process-wide `chdir`.
    fn base(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "civvis-valuenet-{tag}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    fn place(dir: &Path, body: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join("valuenet.json"), body).unwrap();
    }

    /// Preserve parity with a Python training artifact when one is present.
    ///
    /// The net and its fixture must come from the **same tier**.
    /// `ValueNet::load` falls back to `data/`, so resolving the net with the
    /// fallback while reading the fixture from a fixed path would compare a
    /// committed net against a local fixture and report a parity failure that
    /// is really a mismatch of artifacts. `read_net` is the exact-tier read.
    #[test]
    fn matches_training_fixture() {
        #[derive(serde::Deserialize)]
        struct Fix {
            input: Vec<f32>,
            output: f64,
        }
        for dir in ["evolved", "data/evolved"] {
            let Ok(raw) = std::fs::read_to_string(format!("{dir}/valuenet_fixture.json")) else {
                continue;
            };
            let Artifact::Net(net) = read_net(Path::new(dir)) else {
                panic!("{dir} ships a parity fixture but no loadable net beside it");
            };
            let fix: Fix = serde_json::from_str(&raw).unwrap();
            let got = net.eval(&fix.input);
            assert!(
                (got - fix.output).abs() < 1e-4,
                "{dir}: rust {got} vs python {}",
                fix.output
            );
        }
    }

    /// An in-progress training run must not be shadowed by a committed net.
    #[test]
    fn a_local_net_wins_over_the_committed_snapshot() {
        let base = base("local-wins");
        place(&base.join("evolved"), &net_json(25));
        place(&base.join("data").join("evolved"), &net_json(34));
        let net = ValueNet::load_under(&base, "evolved").expect("the local net resolves");
        assert_eq!(net.input_width(), 25, "the local net must win");
    }

    /// The defect this resolution exists for: a process whose working
    /// directory holds no `evolved/` still gets the committed net.
    #[test]
    fn the_committed_snapshot_answers_when_the_working_directory_has_none() {
        let base = base("data-tier");
        place(&base.join("data").join("evolved"), &net_json(25));
        let net = ValueNet::load_under(&base, "evolved").expect("the data/ tier resolves");
        assert_eq!(net.input_width(), 25);
    }

    /// A present-but-unloadable artifact stops resolution. Falling through
    /// would hand back a net the experimenter did not place while
    /// `elo::builtin_provenance` reported one found.
    #[test]
    fn an_unloadable_local_net_does_not_fall_through() {
        let base = base("invalid-stops");
        place(&base.join("evolved"), "{ not json");
        place(&base.join("data").join("evolved"), &net_json(25));
        assert!(
            ValueNet::load_under(&base, "evolved").is_none(),
            "an invalid local artifact must not be silently replaced"
        );

        // Structurally parseable but the wrong shape is the same condition:
        // `valid` rejects it, so it is the experimenter's broken file, not an
        // absent one.
        place(&base.join("evolved"), &net_json(0));
        assert!(ValueNet::load_under(&base, "evolved").is_none());
    }

    #[test]
    fn absent_in_every_tier_is_none() {
        let base = base("absent");
        std::fs::create_dir_all(&base).unwrap();
        assert!(ValueNet::load_under(&base, "evolved").is_none());
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
