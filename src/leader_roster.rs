//! The people and civilizations offered by the game setup screen.
//!
//! This is deliberately distinct from `data/civs.json`.  That file describes
//! the Civilization VI rules which the engine can simulate; this roster says
//! who is available to lead a match and where they belong on a True Start
//! Earth map.  Only the `civ6` tier is allowed to use the ruleset's unique
//! mechanics.  Historical and contemporary entries are neutral identities
//! until they have a separately modelled ruleset.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::sync::OnceLock;

use crate::rules::{CivSpec, Rules};

/// The three deliberately ordered leader collections exposed by setup.
///
/// `expanded` remains a deserialization alias so saved games and older
/// browser clients retain their intended historical roster.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub enum LeaderPool {
    #[default]
    #[serde(rename = "civ6")]
    Civ6,
    #[serde(rename = "historical", alias = "expanded")]
    ExpandedHistorical,
    #[serde(rename = "today")]
    Today,
}

pub const LEADER_POOLS: [LeaderPool; 3] = [
    LeaderPool::Civ6,
    LeaderPool::ExpandedHistorical,
    LeaderPool::Today,
];

impl LeaderPool {
    pub const fn id(self) -> &'static str {
        match self {
            Self::Civ6 => "civ6",
            Self::ExpandedHistorical => "historical",
            Self::Today => "today",
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::Civ6 => "Civ 6 Leaders",
            Self::ExpandedHistorical => "Expanded Historical Figures",
            Self::Today => "Today's Leaders",
        }
    }

    pub const fn description(self) -> &'static str {
        match self {
            Self::Civ6 => {
                "Civilization VI leaders with their official abilities, civilization abilities, and unique units."
            }
            Self::ExpandedHistorical => {
                "A conservatively curated historical roster with neutral CIVVIS rules."
            }
            Self::Today => {
                "A separate, data-driven contemporary roster. It becomes selectable when its records are supplied."
            }
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "civ6" => Some(Self::Civ6),
            // `expanded` was the public setting before the historical tier
            // received a precise name.  Keeping it readable makes old saves
            // and staged successor settings forward-compatible.
            "historical" | "expanded" => Some(Self::ExpandedHistorical),
            "today" => Some(Self::Today),
            _ => None,
        }
    }

    /// Entries which may actually be selected today.  The data keeps a few
    /// retired historical identities for save/map compatibility, but never
    /// hands them out randomly or shows them in setup.
    pub fn entries(self) -> impl Iterator<Item = &'static LeaderRosterEntry> {
        roster()
            .leaders
            .iter()
            .filter(move |entry| entry.pool == self && entry.available)
    }

    pub fn is_available(self) -> bool {
        self.entries().next().is_some()
    }

    /// Never start a game from an empty future dataset.  This is also a safe
    /// fallback for a stale browser that posts `today` before the corresponding
    /// roster data is installed.
    pub fn available_or_default(self) -> Self {
        if self.is_available() {
            self
        } else {
            Default::default()
        }
    }
}

/// A true-start address in the order the setup/data contract uses: latitude,
/// then longitude, both in ordinary WGS84 degrees.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct TrueStartPoint {
    pub latitude: f64,
    pub longitude: f64,
}

/// One selectable or retained leader/civilization identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LeaderRosterEntry {
    /// The stable civilization identity that game saves and the setup payload
    /// carry.  It is also the display name until a future identity needs a
    /// dedicated localization layer.
    pub civ: String,
    pub leader: String,
    pub pool: LeaderPool,
    /// Whether the record is offered in its pool.  False retains the exact
    /// old True Start location for old saves without putting a disputed figure
    /// back into the public historical picker.
    #[serde(default = "default_available")]
    pub available: bool,
    #[serde(flatten)]
    pub true_start: TrueStartPoint,
}

fn default_available() -> bool {
    true
}

#[derive(Deserialize)]
struct RosterDocument {
    schema: String,
    leaders: Vec<LeaderRosterEntry>,
}

struct LeaderRoster {
    leaders: Vec<LeaderRosterEntry>,
}

static ROSTER: OnceLock<LeaderRoster> = OnceLock::new();

/// The complete data-backed roster, including retained non-selectable records.
pub fn all() -> &'static [LeaderRosterEntry] {
    &roster().leaders
}

/// Find a record by the exact civilization identity carried in a setup/save.
pub fn entry(civ: &str) -> Option<&'static LeaderRosterEntry> {
    all().iter().find(|entry| entry.civ == civ)
}

/// Whether a name can be submitted as an explicit leader choice.
pub fn is_selectable(civ: &str) -> bool {
    entry(civ).is_some_and(|entry| entry.available)
}

/// Whether this identity is entitled to Civilization VI-specific content.
/// This one predicate makes the rules boundary auditable rather than relying
/// on every historic entry remembering to leave an ability blank.
pub fn uses_civ6_mechanics(civ: &str) -> bool {
    entry(civ).is_some_and(|entry| entry.pool == LeaderPool::Civ6)
}

/// Where a named leader/civilization belongs on True Start Earth.
pub fn true_start(civ: &str) -> Option<TrueStartPoint> {
    entry(civ).map(|entry| entry.true_start)
}

/// The original seat-index fallback used by generic mapgen callers.  The data
/// is intentionally ordered like the legacy `CIV_NAMES` list, so existing
/// deterministic tests and callers preserve their old defaults while actual
/// games now pass their selected identities explicitly.
pub fn legacy_true_start(index: usize) -> TrueStartPoint {
    let leaders = all();
    leaders[index % leaders.len()].true_start
}

/// Give a per-game rules snapshot neutral specs for non-Civ-VI identities
/// seated in that game.  A future Today entry can therefore be added to
/// `data/leader_roster.json` without a source-code change, while all direct
/// rule lookups remain safe.  Existing legacy rules rows are deliberately
/// replaced too: no agenda, effects, bias, or unique content leaks through a
/// historical name which happens to match an older modeled civilization.
pub fn install_neutral_identities(rules: &mut Rules, identities: &[String]) {
    for civ in identities {
        let Some(record) = entry(civ) else {
            continue;
        };
        if record.pool == LeaderPool::Civ6 {
            continue;
        }
        rules.civs.insert(
            record.civ.clone(),
            CivSpec {
                leader: record.leader.clone(),
                agenda: None,
                traits: Vec::new(),
                ability: "neutral_roster_identity".to_string(),
                effects: Default::default(),
                unique_unit: None,
                note: format!(
                    "{} of {} is a neutral roster identity without Civilization VI-specific mechanics.",
                    record.leader, record.civ
                ),
                start_bias: None,
            },
        );
    }
}

/// A compact browser contract: pools arrive in their prescribed order with
/// exactly the entries their picker may show.
#[derive(Serialize)]
pub struct BrowserLeaderPool<'a> {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub available: bool,
    pub leaders: Vec<&'a LeaderRosterEntry>,
}

pub fn browser_pools() -> Vec<BrowserLeaderPool<'static>> {
    LEADER_POOLS
        .into_iter()
        .map(|pool| {
            let leaders: Vec<_> = pool.entries().collect();
            BrowserLeaderPool {
                id: pool.id(),
                name: pool.name(),
                description: pool.description(),
                available: !leaders.is_empty(),
                leaders,
            }
        })
        .collect()
}

fn roster() -> &'static LeaderRoster {
    ROSTER.get_or_init(|| {
        let document: RosterDocument =
            serde_json::from_str(include_str!("../data/leader_roster.json"))
                .unwrap_or_else(|error| panic!("leader_roster.json is malformed: {error}"));
        assert_eq!(
            document.schema, "civvis.leader-roster.v1",
            "leader_roster.json has an unsupported schema"
        );
        assert!(
            !document.leaders.is_empty(),
            "leader_roster.json must retain the Civilization VI roster"
        );
        let mut names = BTreeSet::new();
        for leader in &document.leaders {
            assert!(
                !leader.civ.trim().is_empty(),
                "leader roster has a blank civilization"
            );
            assert!(
                !leader.leader.trim().is_empty(),
                "{} has a blank leader",
                leader.civ
            );
            assert!(
                (-90.0..=90.0).contains(&leader.true_start.latitude),
                "{} has an invalid true-start latitude {}",
                leader.civ,
                leader.true_start.latitude
            );
            assert!(
                (-180.0..=180.0).contains(&leader.true_start.longitude),
                "{} has an invalid true-start longitude {}",
                leader.civ,
                leader.true_start.longitude
            );
            assert!(
                names.insert(leader.civ.clone()),
                "leader roster repeats civilization {:?}",
                leader.civ
            );
        }
        LeaderRoster {
            leaders: document.leaders,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{CIV6_LEADER_POOL, CIV_NAMES};
    use crate::rules::Rules;

    #[test]
    fn roster_preserves_every_legacy_identity_and_coordinate_in_order() {
        assert!(
            all().len() >= CIV_NAMES.len(),
            "future historical or contemporary records may extend the legacy roster"
        );
        for (index, civ) in CIV_NAMES.iter().enumerate() {
            assert_eq!(all()[index].civ, *civ, "legacy identity moved at {index}");
            assert_eq!(true_start(civ), Some(legacy_true_start(index)));
        }
    }

    #[test]
    fn civ6_is_the_only_mechanical_roster_and_legacy_expanded_maps_to_history() {
        let civ6: Vec<&str> = LeaderPool::Civ6
            .entries()
            .map(|entry| entry.civ.as_str())
            .collect();
        assert_eq!(civ6, CIV6_LEADER_POOL);
        assert!(civ6.iter().all(|civ| uses_civ6_mechanics(civ)));
        assert!(LeaderPool::ExpandedHistorical
            .entries()
            .all(|entry| !uses_civ6_mechanics(&entry.civ)));
        assert_eq!(
            LeaderPool::from_id("expanded"),
            Some(LeaderPool::ExpandedHistorical)
        );
        assert_eq!(
            LeaderPool::from_id("historical"),
            Some(LeaderPool::ExpandedHistorical)
        );
        assert_eq!(
            serde_json::from_str::<LeaderPool>("\"expanded\"").unwrap(),
            LeaderPool::ExpandedHistorical
        );
        assert_eq!(
            serde_json::to_string(&LeaderPool::ExpandedHistorical).unwrap(),
            "\"historical\""
        );
    }

    #[test]
    fn historical_picker_is_curated_and_today_stays_empty_until_data_arrives() {
        assert!(LeaderPool::ExpandedHistorical.is_available());
        assert!(!LeaderPool::Today.is_available());
        assert_eq!(LeaderPool::Today.available_or_default(), LeaderPool::Civ6);
        assert!(!is_selectable("Romania"));
        assert!(is_selectable("Switzerland"));
        let pools = browser_pools();
        assert_eq!(
            pools.iter().map(|pool| pool.id).collect::<Vec<_>>(),
            vec!["civ6", "historical", "today"]
        );
        assert!(!pools[2].available);
        assert!(pools[2].leaders.is_empty());
    }

    #[test]
    fn civ6_roster_records_agree_with_the_modeled_rules_leaders() {
        let rules = Rules::embedded();
        for record in LeaderPool::Civ6.entries() {
            let spec = &rules.civs[&record.civ];
            assert_eq!(spec.leader, record.leader, "{} leader drifted", record.civ);
        }
    }
}
