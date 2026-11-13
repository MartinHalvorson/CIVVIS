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
pub const CIV6_REFERENCE: &str = "stock";

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
        blurb: "The planner with no promoted extras. Our stand-in for Civilization VI's own AI.",
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
        blurb: "A simple scripted opponent for a first game.",
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

/// The roster by published Elo, strongest first. An agent nothing has rated
/// ranks below every rated one, and equal ratings keep roster order.
pub fn ranked() -> Vec<&'static PlayerStrategy> {
    let file = published();
    let elo = |row: &PlayerStrategy| {
        file.ratings
            .get(row.id)
            .map_or(f64::NEG_INFINITY, |rating| rating.elo)
    };
    let mut rows: Vec<&'static PlayerStrategy> = ROSTER.iter().collect();
    rows.sort_by(|a, b| elo(b).total_cmp(&elo(a)));
    rows
}

/// Keeps the deal apart from every other stream drawn from a game's seed.
const TABLE_DEAL_STREAM: u64 = 0x7ab1_e0f5_ea75_d0a1;

/// Who plays a watched table of `seats` majors: the strongest agents, one to
/// a seat, dealt round the table by `seed`.
///
/// The deal is what keeps an agent from always drawing the first seat, and
/// with it the first move and the first start position. It is a stream of
/// its own, never the game's, so seating a table cannot move the world it
/// is played on. A table larger than the roster seats the ranking again from
/// the top.
pub fn table(seats: usize, seed: u64) -> Vec<&'static PlayerStrategy> {
    let ranked = ranked();
    let mut table: Vec<&'static PlayerStrategy> =
        (0..seats).map(|seat| ranked[seat % ranked.len()]).collect();
    let mut rng = crate::rng::Rng::new(seed ^ TABLE_DEAL_STREAM);
    for index in (1..table.len()).rev() {
        table.swap(index, rng.below(index + 1));
    }
    table
}

/// What a seat says about the agent playing it: its handle, its tilt, and
/// the rating it sat down with when `strategy_elo` has measured one.
pub fn seat_json(row: &PlayerStrategy) -> Value {
    let file = published();
    let rating = file.ratings.get(row.id);
    json!({
        "ai_agent": row.id,
        "ai_username": row.handle,
        "ai_strat_label": row.tilt,
        "ai_elo": rating.map(|r| r.elo.round() as i64),
        "ai_elo_sd": rating.map(|r| r.sd.round() as i64),
        "ai_elo_tables": rating.map_or(0, |r| r.games),
        "ai_elo_provisional": rating.is_none_or(|r| r.games < PROVISIONAL_TABLES),
        "ai_civ6_level": rating.and_then(|r| nearest_rung(file, r.elo)),
    })
}

/// The lane our own seat plays in a mirrored game of Civilization VI, as the
/// live decider is launched with it (`civvis-orders --victory`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveLane {
    /// `civvis`: no contract, the agent follows whichever victory is open.
    Adaptive,
    /// Contracted to one victory for the whole game.
    Target(VictoryTarget),
}

impl LiveLane {
    /// A lane by the id the decider takes. An id it would refuse is no lane.
    pub fn from_id(id: &str) -> Option<LiveLane> {
        if id == "civvis" {
            return Some(LiveLane::Adaptive);
        }
        id.parse().ok().map(LiveLane::Target)
    }
}

/// What a seat says when Civilization VI's own AI is playing it: every rival
/// of a mirrored game (operator, 2026-11-13: "for validation games, the
/// player should just be something like "Civ 6 Deity AI" or whatever the
/// level is"). `difficulty` is the rung id the host reported. Nothing of ours
/// plays the seat, so it carries no agent id and no rating: the published
/// rung ratings were measured on our stand-in for that AI, not on it.
pub fn civ6_ai_seat_json(difficulty: &str) -> Value {
    let rung = CIV6_RUNGS.iter().find(|(id, _)| *id == difficulty);
    json!({
        "ai_username": match rung {
            Some((_, name)) => format!("Civ 6 {name} AI"),
            None => "Civ 6 AI".to_string(),
        },
        "ai_strat_label": "Civilization VI's own AI",
    })
}

/// What our own seat in a mirrored game says: the strategy the live decider
/// is actually playing (same instruction: "where we control the player ai
/// strategy, then we should list out the actual strategy being used"). The
/// live genome is no roster row, so it borrows no handle and no rating.
/// `None` is a follower that has not said which lane the decider is on.
pub fn live_seat_json(lane: Option<LiveLane>) -> Value {
    let (handle, tilt) = match lane {
        None => ("CIVVIS Advanced AI".to_string(), "Live agent".to_string()),
        Some(LiveLane::Adaptive) => (
            "CIVVIS Adaptive".to_string(),
            "Live agent · whichever victory is open".to_string(),
        ),
        Some(LiveLane::Target(target)) => {
            let id = target.as_str();
            let lane = format!("{}{}", id[..1].to_ascii_uppercase(), &id[1..]);
            (
                format!("CIVVIS {lane}"),
                format!("Live agent · {lane} victory"),
            )
        }
    };
    json!({ "ai_username": handle, "ai_strat_label": tilt })
}

/// The ledger key of a Civilization VI rung's rivals.
pub fn rung_key(rung: &str) -> String {
    format!("civ6:{rung}")
}

/// One entrant's published rating.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
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
    /// `"lower"` when it has never lost a table and `"upper"` when it has
    /// never won one: the fit then only bounds the rating from one side.
    #[serde(default)]
    pub bound: String,
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
    /// Rung id to its Elo on the ladder a person is shown: the measured rung
    /// ratings after [`monotone_ladder`].
    #[serde(default)]
    pub ladder: BTreeMap<String, f64>,
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
        .filter_map(|(id, name)| Some(((file.ladder.get(*id)? - elo).abs(), *name)))
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
                "bound": rating.map(|r| r.bound.as_str()),
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
                "elo": file.ladder.get(*id).map(|elo| elo.round()),
                "measured_elo": rating.map(|r| r.elo.round()),
                "elo_sd": rating.map(|r| r.sd.round()),
                "bound": rating.map(|r| r.bound.as_str()),
                "rated_tables": rating.map_or(0, |r| r.games),
            })
        })
        .collect::<Vec<_>>())
}

/// The measured rung ratings made non-decreasing up the ladder.
///
/// Each rung's handicap contains the one below it, so a higher rung cannot
/// truly be easier; when two measure the wrong way round, that is noise, and
/// showing it would tell a person King is harder than Emperor. Neighbours out
/// of order are pooled to their weighted mean (isotonic regression), with the
/// `pin` rung held where it is because it defines the scale. Rungs the
/// tables cannot tell apart therefore show the same Elo, which is the honest
/// reading of them.
pub fn monotone_ladder(elo: &[f64], weight: &[f64], pin: usize) -> Vec<f64> {
    // Blocks of (weighted sum, weight, rungs); a pinned block outweighs all.
    let mut blocks: Vec<(f64, f64, usize)> = Vec::new();
    for (index, value) in elo.iter().enumerate() {
        let w = if index == pin {
            1e12
        } else {
            weight[index].max(1.0)
        };
        blocks.push((value * w, w, 1));
        while blocks.len() > 1 {
            let (last, before) = (blocks[blocks.len() - 1], blocks[blocks.len() - 2]);
            if before.0 / before.1 <= last.0 / last.1 {
                break;
            }
            blocks.truncate(blocks.len() - 2);
            blocks.push((before.0 + last.0, before.1 + last.1, before.2 + last.2));
        }
    }
    blocks
        .iter()
        .flat_map(|(sum, w, rungs)| std::iter::repeat_n(sum / w, *rungs))
        .collect()
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
            assert!(
                handles.insert(row.handle),
                "duplicate handle {}",
                row.handle
            );
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

    /// A watched table seats the strongest agents, each exactly once, and
    /// the seed decides only who sits where.
    #[test]
    fn a_table_seats_the_top_rated_agents_once_each() {
        let ranked = ranked();
        assert_eq!(ranked.len(), ROSTER.len());
        let file = published();
        let rated: Vec<f64> = ranked
            .iter()
            .filter_map(|row| file.ratings.get(row.id).map(|rating| rating.elo))
            .collect();
        assert!(
            rated.windows(2).all(|pair| pair[0] >= pair[1]),
            "strongest first: {rated:?}"
        );
        let top: BTreeSet<&str> = ranked.iter().take(4).map(|row| row.id).collect();
        let mut first_seats = BTreeSet::new();
        for seed in 0..40_u64 {
            let dealt: Vec<&str> = table(4, seed).iter().map(|row| row.id).collect();
            let seated: BTreeSet<&str> = dealt.iter().copied().collect();
            assert_eq!(seated, top, "seed {seed} seated somebody else");
            let again: Vec<&str> = table(4, seed).iter().map(|row| row.id).collect();
            assert_eq!(dealt, again, "the same seed deals the same table");
            first_seats.insert(dealt[0]);
        }
        assert_eq!(
            first_seats, top,
            "every agent draws the first seat sometimes"
        );
        // More seats than agents: everybody sits before anybody sits twice.
        let crowded = table(ROSTER.len() + 2, 7);
        let distinct: BTreeSet<&str> = crowded.iter().map(|row| row.id).collect();
        assert_eq!(distinct.len(), ROSTER.len());
        assert!(table(0, 7).is_empty());
    }

    #[test]
    fn a_seat_carries_its_agents_handle_and_published_rating() {
        let seat = seat_json(find("jack").expect("the flagship"));
        assert_eq!(seat["ai_username"], json!("JackOfAllTrades"));
        assert_eq!(seat["ai_agent"], json!("jack"));
        let published = published()
            .ratings
            .get("jack")
            .map(|r| r.elo.round() as i64);
        assert_eq!(seat["ai_elo"].as_i64(), published);
    }

    #[test]
    fn a_mirrored_rival_is_named_for_civilization_vis_own_ai() {
        let deity = civ6_ai_seat_json("deity");
        assert_eq!(deity["ai_username"], json!("Civ 6 Deity AI"));
        assert_eq!(deity["ai_strat_label"], json!("Civilization VI's own AI"));
        // Nothing of ours is in the seat: no roster id and no rating.
        assert!(deity["ai_agent"].is_null());
        assert!(deity["ai_elo"].is_null());
        for (id, name) in CIV6_RUNGS {
            assert_eq!(
                civ6_ai_seat_json(id)["ai_username"],
                json!(format!("Civ 6 {name} AI"))
            );
        }
        // A level the ladder does not hold still says whose AI it is.
        assert_eq!(civ6_ai_seat_json("")["ai_username"], json!("Civ 6 AI"));
    }

    #[test]
    fn our_mirrored_seat_names_the_lane_it_plays() {
        // Every id the live decider accepts names a lane.
        for (id, handle) in [
            ("science", "CIVVIS Science"),
            ("culture", "CIVVIS Culture"),
            ("religious", "CIVVIS Religious"),
            ("diplomatic", "CIVVIS Diplomatic"),
            ("domination", "CIVVIS Domination"),
            ("score", "CIVVIS Score"),
            ("civvis", "CIVVIS Adaptive"),
        ] {
            let lane = LiveLane::from_id(id).unwrap_or_else(|| panic!("{id} is a lane"));
            let seat = live_seat_json(Some(lane));
            assert_eq!(seat["ai_username"], json!(handle));
            assert!(seat["ai_elo"].is_null(), "the live genome is unrated");
        }
        assert_eq!(
            live_seat_json(LiveLane::from_id("diplomatic"))["ai_strat_label"],
            json!("Live agent · Diplomatic victory")
        );
        assert_eq!(LiveLane::from_id(""), None);
        assert_eq!(
            live_seat_json(None)["ai_username"],
            json!("CIVVIS Advanced AI")
        );
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
        // The ladder a person is shown climbs, and Prince is where it is pinned.
        let ladder: Vec<f64> = CIV6_RUNGS
            .iter()
            .filter_map(|(id, _)| file.ladder.get(*id).copied())
            .collect();
        assert!(ladder.is_empty() || ladder.len() == CIV6_RUNGS.len());
        assert!(
            ladder.windows(2).all(|pair| pair[0] <= pair[1]),
            "{ladder:?}"
        );
        if let Some(prince) = file.ladder.get(PRINCE) {
            assert!((prince - PRINCE_ELO).abs() < 0.5);
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
    fn a_ladder_measured_out_of_order_is_pooled_around_the_pin() {
        // Two low rungs measured above the pinned one, and the top two the
        // wrong way round.
        let ladder = monotone_ladder(
            &[1100.0, 950.0, 1000.0, 1200.0, 1500.0, 1400.0],
            &[30.0, 30.0, 0.0, 30.0, 30.0, 30.0],
            2,
        );
        assert!(
            ladder.windows(2).all(|pair| pair[0] <= pair[1]),
            "{ladder:?}"
        );
        assert!((ladder[2] - 1000.0).abs() < 1e-6, "the pin does not move");
        assert!((ladder[0] - 1000.0).abs() < 1e-6 && (ladder[1] - 1000.0).abs() < 1e-6);
        assert_eq!(ladder[3], 1200.0);
        assert!((ladder[4] - 1450.0).abs() < 1e-6 && (ladder[5] - 1450.0).abs() < 1e-6);
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
