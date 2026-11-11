//! Civ AI Agents: frozen genomes with a handle, a tilt and a measured Elo.
//!
//! A **Civ AI Agent** (in code, a *player-strategy*) is one of our controllers
//! frozen at a named genome: a chassis, an optional victory contract and a
//! fixed list of gene tags. The gene screen prices behaviours one at a time;
//! this is the layer above it, the thing a person picks to play against and
//! the thing a rating can be attached to.
//!
//! The ratings are measured by `strategy_elo` (`src/bin/strategy_elo.rs`) and
//! published in `data/strategy_elo.json`, which this module embeds. The scale
//! is pinned to Civilization VI's own ladder: Prince, the rung where neither
//! side is handed anything, is [`PRINCE_ELO`], and every other rung is rated
//! by seating the [`CIV6_REFERENCE`] agent with that rung's shipped handicap
//! against a challenger playing on the human's terms. `docs/ELO.md` says what
//! that does and does not prove.
use crate::ai::{AdvancedAi, Ai, BasicAi, VictoryTarget};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// What a person is shown for one of these.
pub const CONCEPT_NAME: &str = "Civ AI Agent";

/// The Elo of Civilization VI's unhandicapped rung. Everything else is
/// measured relative to it.
pub const PRINCE_ELO: f64 = 1000.0;

/// The rung the scale is pinned to.
pub const PRINCE: &str = "prince";

/// Civilization VI's difficulty ladder, lowest first: id and display name.
pub const CIV6_RUNGS: [(&str, &str); 8] = [
    ("settler", "Settler"),
    ("chieftain", "Chieftain"),
    ("warlord", "Warlord"),
    ("prince", "Prince"),
    ("king", "King"),
    ("emperor", "Emperor"),
    ("immortal", "Immortal"),
    ("deity", "Deity"),
];

/// The agent that stands in for Firaxis's own AI in the rung tables. At
/// Prince it plays with nothing added, so its own rating *is* Prince.
pub const CIV6_REFERENCE: &str = "wheels";

/// Fewer rated tables than this and a rating is shown as provisional.
pub const PROVISIONAL_TABLES: u32 = 30;

/// The controller a strategy is built on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Chassis {
    /// `AdvancedAi` with the deployment genome (`enable_native_deployment`).
    Deployment,
    /// `AdvancedAi::new()`: the production genes alone, every opt-in off.
    Stock,
    /// `AdvancedAi::legacy()`, the first promoted hierarchical agent.
    Legacy,
    /// `BasicAi`, the scripted baseline.
    Basic,
}

/// One frozen genome a person can pick and a rating can be attached to.
#[derive(Debug)]
pub struct PlayerStrategy {
    /// Stable key: the rating ledger, the game log and `/new` all use it.
    pub id: &'static str,
    /// The gamer name the picker shows.
    pub handle: &'static str,
    /// What it leans toward, in a word or two.
    pub tilt: &'static str,
    /// One line on how it plays.
    pub blurb: &'static str,
    pub chassis: Chassis,
    /// The victory it is contracted to, when it has one.
    pub target: Option<VictoryTarget>,
    /// Gene tags switched on over the chassis.
    pub genes: &'static [&'static str],
    /// Older ids that still resolve here, so a remembered choice keeps working.
    pub aliases: &'static [&'static str],
}

/// Every Civ AI Agent, flagship first.
///
/// A row is frozen: change what an agent *is* by adding a row under a new id,
/// not by editing one, or its recorded games stop describing it.
pub const ROSTER: &[PlayerStrategy] = &[
    PlayerStrategy {
        id: "jack",
        handle: "JackOfAllTrades",
        tilt: "Balanced",
        blurb: "Our deployment genome. Reads the table and takes whichever victory is open.",
        chassis: Chassis::Deployment,
        target: None,
        genes: &[],
        aliases: &["advanced_evolved"],
    },
    PlayerStrategy {
        id: "siege",
        handle: "SiegeTheDay",
        tilt: "Aggressive · Domination",
        blurb: "Builds an army early and comes for your capital.",
        chassis: Chassis::Deployment,
        target: Some(VictoryTarget::Domination),
        genes: &[],
        aliases: &[],
    },
    PlayerStrategy {
        id: "moonshot",
        handle: "MoonshotMaven",
        tilt: "Science",
        blurb: "Campuses first, then a straight line to the launch pad.",
        chassis: Chassis::Deployment,
        target: Some(VictoryTarget::Science),
        genes: &[],
        aliases: &[],
    },
    PlayerStrategy {
        id: "tourist",
        handle: "TouristTrap",
        tilt: "Culture",
        blurb: "Theatres, wonders and Great Works until your people holiday in its cities.",
        chassis: Chassis::Deployment,
        target: Some(VictoryTarget::Culture),
        genes: &[],
        aliases: &[],
    },
    PlayerStrategy {
        id: "holy",
        handle: "HolyRoller",
        tilt: "Religion",
        blurb: "Founds a faith and sends Apostles until yours is gone.",
        chassis: Chassis::Deployment,
        target: Some(VictoryTarget::Religion),
        genes: &[],
        aliases: &[],
    },
    PlayerStrategy {
        id: "swing",
        handle: "SwingVote",
        tilt: "Diplomacy",
        blurb: "Banks Favor, courts city-states and wins the World Congress.",
        chassis: Chassis::Deployment,
        target: Some(VictoryTarget::Diplomacy),
        genes: &[],
        aliases: &[],
    },
    PlayerStrategy {
        id: "forward",
        handle: "ForwardSettle",
        tilt: "Expansion",
        blurb: "Keeps founding cities for as long as one more still pays.",
        chassis: Chassis::Deployment,
        target: None,
        genes: &["no-city-limit"],
        aliases: &[],
    },
    PlayerStrategy {
        id: "stock",
        handle: "ByTheBook",
        tilt: "Balanced · lighter",
        blurb: "The same planner with none of the promoted extras switched on.",
        chassis: Chassis::Stock,
        target: None,
        genes: &[],
        aliases: &["advanced"],
    },
    PlayerStrategy {
        id: "oldguard",
        handle: "OldGuard",
        tilt: "Classic",
        blurb: "Our first hierarchical agent, kept exactly as it was.",
        chassis: Chassis::Legacy,
        target: None,
        genes: &[],
        aliases: &["advanced_v1"],
    },
    PlayerStrategy {
        id: "wheels",
        handle: "TrainingWheels",
        tilt: "Casual",
        blurb: "A simple scripted opponent. The stand-in for Civilization VI's own AI.",
        chassis: Chassis::Basic,
        target: None,
        genes: &[],
        aliases: &["basic"],
    },
];

impl PlayerStrategy {
    /// The agent this row describes. Panics on a gene tag the registry does
    /// not hold; `every_roster_row_builds` keeps that from shipping.
    pub fn build(&self) -> Box<dyn Ai + Send> {
        match self.chassis {
            Chassis::Basic => Box::new(BasicAi::new()),
            Chassis::Legacy => Box::new(AdvancedAi::legacy()),
            Chassis::Stock | Chassis::Deployment => {
                let mut ai = match self.target {
                    Some(target) => AdvancedAi::targeting(target),
                    None => AdvancedAi::new(),
                };
                if self.chassis == Chassis::Deployment {
                    ai.enable_native_deployment();
                }
                for tag in self.genes {
                    let gene = crate::ai::gene(tag)
                        .unwrap_or_else(|| panic!("{}: unknown gene `{tag}`", self.id));
                    (gene.enable)(&mut ai);
                }
                Box::new(ai)
            }
        }
    }
}

/// A strategy by id, handle or an older id.
pub fn find(name: &str) -> Option<&'static PlayerStrategy> {
    ROSTER
        .iter()
        .find(|row| row.id == name || row.handle == name || row.aliases.contains(&name))
}

/// The ledger key of a Civilization VI rung's rivals.
pub fn rung_key(rung: &str) -> String {
    format!("civ6:{rung}")
}

/// One entrant's published rating.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub struct Rating {
    pub elo: f64,
    /// Bootstrap standard deviation, in Elo.
    #[serde(default)]
    pub sd: f64,
    /// Rated tables it sat at.
    #[serde(default)]
    pub games: u32,
    #[serde(default)]
    pub wins: u32,
}

/// `data/strategy_elo.json` as `strategy_elo` writes it.
#[derive(Debug, Default, Deserialize)]
pub struct RatingFile {
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub tables: u32,
    /// Strategy ids and `civ6:<rung>` keys.
    #[serde(default)]
    pub ratings: BTreeMap<String, Rating>,
}

/// The ratings this build shipped with. An unreadable file is no ratings, and
/// `the_published_ratings_parse` is what keeps one from shipping.
pub fn published() -> &'static RatingFile {
    static FILE: std::sync::OnceLock<RatingFile> = std::sync::OnceLock::new();
    FILE.get_or_init(|| {
        serde_json::from_str(include_str!("../data/strategy_elo.json")).unwrap_or_default()
    })
}

/// The Civilization VI rung whose rivals rate nearest `elo`, by display name.
pub fn nearest_rung(file: &RatingFile, elo: f64) -> Option<&'static str> {
    CIV6_RUNGS
        .iter()
        .filter_map(|(id, name)| {
            let rating = file.ratings.get(&rung_key(id))?;
            Some(((rating.elo - elo).abs(), *name))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, name)| name)
}

/// The picker's rows: every agent with its handle, tilt and rating.
pub fn roster_json() -> Value {
    let file = published();
    json!(ROSTER
        .iter()
        .map(|row| {
            let rating = file.ratings.get(row.id);
            json!({
                "name": row.id,
                "username": row.handle,
                "label": format!("{} — {}", row.tilt, row.blurb),
                "tilt": row.tilt,
                "blurb": row.blurb,
                "elo": rating.map(|r| r.elo.round()),
                "elo_sd": rating.map(|r| r.sd.round()),
                "rated_tables": rating.map_or(0, |r| r.games),
                "civ6_level": rating.and_then(|r| nearest_rung(file, r.elo)),
                "provisional": rating.is_none_or(|r| r.games < PROVISIONAL_TABLES),
            })
        })
        .collect::<Vec<_>>())
}

/// Civilization VI's ladder on our scale, lowest rung first. A rung nothing
/// has measured yet carries a null Elo rather than a guess.
pub fn civ6_ladder_json() -> Value {
    let file = published();
    json!(CIV6_RUNGS
        .iter()
        .enumerate()
        .map(|(order, (id, name))| {
            let rating = file.ratings.get(&rung_key(id));
            json!({
                "id": id,
                "name": name,
                "level": order + 1,
                "elo": rating.map(|r| r.elo.round()),
                "elo_sd": rating.map(|r| r.sd.round()),
                "rated_tables": rating.map_or(0, |r| r.games),
            })
        })
        .collect::<Vec<_>>())
}

/// One finished table: the entrant in each major seat and the seat that won.
#[derive(Clone, Debug)]
pub struct Table {
    pub seats: Vec<usize>,
    pub winner: usize,
}

/// Fit every entrant's strength to who won each table, on the Elo scale
/// (unanchored: only differences mean anything).
///
/// The model is the one `elo::win_shares` already states: a seat wins its
/// table with probability proportional to `10^(elo / 400)`. That makes a
/// two-seat table the ordinary Elo expectation, and it lets a table hold the
/// same entrant in several seats, which is how a rung's three rivals are
/// rated as one thing. The fit is the maximum-likelihood one (Hunter's MM
/// iteration), so it does not depend on the order the games were played in,
/// which a running K-factor update would.
///
/// Each entrant also plays two imaginary head-to-head games against an
/// average player, one won and one lost. Without them an entrant that has
/// never won, or never lost, has no finite rating.
pub fn fit(entrants: usize, tables: &[Table]) -> Vec<f64> {
    let mut wins = vec![1.0_f64; entrants];
    for table in tables {
        wins[table.seats[table.winner]] += 1.0;
    }
    let mut strength = vec![1.0_f64; entrants];
    for _ in 0..5000 {
        let mut exposure: Vec<f64> = strength.iter().map(|s| 2.0 / (s + 1.0)).collect();
        for table in tables {
            let total: f64 = table.seats.iter().map(|seat| strength[*seat]).sum();
            for seat in &table.seats {
                exposure[*seat] += 1.0 / total;
            }
        }
        let mut moved = 0.0_f64;
        for index in 0..entrants {
            let next = wins[index] / exposure[index];
            moved = moved.max((next / strength[index]).ln().abs());
            strength[index] = next;
        }
        if moved < 1e-10 {
            break;
        }
    }
    strength.iter().map(|s| 400.0 * s.log10()).collect()
}

/// The standard deviation of each entrant's Elo relative to `anchor`, from
/// refitting `rounds` resamples of the tables drawn with replacement.
pub fn bootstrap_sd(
    entrants: usize,
    tables: &[Table],
    anchor: usize,
    rounds: usize,
    seed: u64,
) -> Vec<f64> {
    if tables.is_empty() || rounds < 2 {
        return vec![0.0; entrants];
    }
    let mut rng = crate::rng::Rng::new(seed);
    let mut sum = vec![0.0_f64; entrants];
    let mut squares = vec![0.0_f64; entrants];
    for _ in 0..rounds {
        let sample: Vec<Table> = (0..tables.len())
            .map(|_| tables[rng.below(tables.len())].clone())
            .collect();
        let elo = fit(entrants, &sample);
        for index in 0..entrants {
            let relative = elo[index] - elo[anchor];
            sum[index] += relative;
            squares[index] += relative * relative;
        }
    }
    let n = rounds as f64;
    (0..entrants)
        .map(|index| {
            let mean = sum[index] / n;
            (squares[index] / n - mean * mean).max(0.0).sqrt()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_roster_row_builds() {
        let mut ids = BTreeSet::new();
        let mut handles = BTreeSet::new();
        for row in ROSTER {
            assert!(ids.insert(row.id), "duplicate id {}", row.id);
            assert!(handles.insert(row.handle), "duplicate handle {}", row.handle);
            for tag in row.genes {
                assert!(crate::ai::gene(tag).is_some(), "{}: no gene {tag}", row.id);
            }
            let _ = row.build();
        }
        assert!(find(CIV6_REFERENCE).is_some());
    }

    #[test]
    fn an_older_id_still_finds_its_agent() {
        assert_eq!(find("advanced").map(|row| row.id), Some("stock"));
        assert_eq!(find("basic").map(|row| row.id), Some("wheels"));
        assert_eq!(find("TouristTrap").map(|row| row.id), Some("tourist"));
        assert!(find("nobody").is_none());
    }

    #[test]
    fn the_published_ratings_parse() {
        let file: RatingFile = serde_json::from_str(include_str!("../data/strategy_elo.json"))
            .expect("data/strategy_elo.json parses");
        for key in file.ratings.keys() {
            let known = ROSTER.iter().any(|row| row.id == key)
                || CIV6_RUNGS.iter().any(|(id, _)| &rung_key(id) == key);
            assert!(known, "rating for an entrant nobody registered: {key}");
        }
        if let Some(prince) = file.ratings.get(&rung_key(PRINCE)) {
            assert!((prince.elo - PRINCE_ELO).abs() < 0.5);
        }
    }

    /// Two entrants, one winning three head-to-head games in four: the fit
    /// must put them the Elo distance apart that predicts three in four once
    /// the two imaginary games each are counted (4 wins to 2).
    #[test]
    fn a_head_to_head_record_fits_its_elo_expectation() {
        let mut tables = Vec::new();
        for game in 0..40 {
            tables.push(Table {
                seats: vec![0, 1],
                winner: usize::from(game % 4 == 3),
            });
        }
        let elo = fit(2, &tables);
        let expected = crate::elo::expected(elo[0], elo[1]);
        assert!((expected - 0.75).abs() < 0.03, "expected {expected}");
    }

    /// One challenger against three seats of the same rival, winning a
    /// quarter of the tables, is exactly as strong as that rival.
    #[test]
    fn three_seats_of_one_rival_are_rated_as_one_entrant() {
        let tables: Vec<Table> = (0..80)
            .map(|game| Table {
                seats: vec![0, 1, 1, 1],
                winner: game % 4,
            })
            .collect();
        let elo = fit(2, &tables);
        assert!((elo[0] - elo[1]).abs() < 15.0, "{elo:?}");
        let sd = bootstrap_sd(2, &tables, 1, 50, 7);
        assert_eq!(sd[1], 0.0);
        assert!(sd[0] > 0.0 && sd[0] < 200.0, "{sd:?}");
    }

    #[test]
    fn an_entrant_that_never_won_still_has_a_finite_rating() {
        let tables: Vec<Table> = (0..20)
            .map(|_| Table {
                seats: vec![0, 1],
                winner: 0,
            })
            .collect();
        let elo = fit(2, &tables);
        assert!(elo.iter().all(|value| value.is_finite()));
        assert!(elo[0] > elo[1] + 300.0);
    }
}
