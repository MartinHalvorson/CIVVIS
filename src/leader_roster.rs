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
    /// Where [`entry`] looks a name up: a table of positions in `leaders`
    /// plus one, zero for an empty slot, addressed by [`civ_slot`] and
    /// probed forward. Built by [`index_by_civ`].
    by_civ: Vec<u32>,
}

/// The slot a name's search starts at in a table of `mask + 1` slots.
///
/// Three bytes and the length, multiplied once: the first, the last and
/// the middle byte tell the roster's names apart nearly as well as reading
/// all of them does (nine names in ten sit in the slot their search starts
/// at, which a test holds it to), and a search that starts in a crowded
/// slot is still a correct search.
fn civ_slot(civ: &str, mask: usize) -> usize {
    let bytes = civ.as_bytes();
    let Some((&first, &last)) = bytes.first().zip(bytes.last()) else {
        return 0;
    };
    let word = u64::from(first)
        | u64::from(last) << 8
        | u64::from(bytes[bytes.len() / 2]) << 16
        | (bytes.len() as u64) << 24;
    (word.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 40) as usize & mask
}

/// The table [`entry`] searches: a power of two of slots, at least four
/// times the roster, so a search always meets an empty slot and seldom
/// takes a second step. Names go in in roster order, which leaves the first
/// record of a name first on its path, as reading the roster would find it.
fn index_by_civ(leaders: &[LeaderRosterEntry]) -> Vec<u32> {
    let slots = (leaders.len() * 4).next_power_of_two().max(16);
    let mask = slots - 1;
    let mut by_civ = vec![0_u32; slots];
    for (position, leader) in leaders.iter().enumerate() {
        let mut slot = civ_slot(&leader.civ, mask);
        while by_civ[slot] != 0 {
            slot = (slot + 1) & mask;
        }
        by_civ[slot] = position as u32 + 1;
    }
    by_civ
}

/// [`entry`] over any roster and its [`index_by_civ`] table.
fn find_by_civ<'a>(
    leaders: &'a [LeaderRosterEntry],
    by_civ: &[u32],
    civ: &str,
) -> Option<&'a LeaderRosterEntry> {
    let mask = by_civ.len() - 1;
    let mut slot = civ_slot(civ, mask);
    loop {
        let position = by_civ[slot];
        if position == 0 {
            return None;
        }
        let leader = &leaders[position as usize - 1];
        if leader.civ == civ {
            return Some(leader);
        }
        slot = (slot + 1) & mask;
    }
}

static ROSTER: OnceLock<LeaderRoster> = OnceLock::new();

/// The complete data-backed roster, including retained non-selectable records.
pub fn all() -> &'static [LeaderRosterEntry] {
    &roster().leaders
}

/// Find a record by the exact civilization identity carried in a setup/save.
///
/// ★ THIS IS ASKED MILLIONS OF TIMES A GAME, AND IT READ THE ROSTER EVERY
/// TIME. `Game::has_ability`, `civ_effect` and `owns_civ_unique` all begin
/// by asking whether the seat is a Civilization VI identity
/// ([`uses_civ6_mechanics`]), which is this lookup, and the rules ask one of
/// the three for almost every yield, strength and movement term. It walked
/// the roster comparing names until one matched, so what it cost depended on
/// where the seat's name stood: next to nothing for the first six names,
/// which is who `civvis simulate` seats, and 2.6% of a game for the shuffled
/// civilizations a gene screen seats (`docs/SIMULATOR_PERFORMANCE.md`,
/// 2026-11-15). The roster is read once and never changes, so a name is
/// looked up in a table built when it is read, at the same price wherever
/// it stands.
pub fn entry(civ: &str) -> Option<&'static LeaderRosterEntry> {
    let roster = roster();
    find_by_civ(&roster.leaders, &roster.by_civ, civ)
}

/// [`entry`] as it was written first: the first record whose name matches,
/// found by reading the roster in order. Test-only, as what the search is
/// compared against.
#[cfg(test)]
fn entry_by_reading_the_roster(civ: &str) -> Option<&'static LeaderRosterEntry> {
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
            by_civ: index_by_civ(&document.leaders),
            leaders: document.leaders,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{CIV6_LEADER_POOL, CIV_NAMES};
    use crate::rules::Rules;

    /// The search finds the record reading the roster in order found, for
    /// every name on it, and finds nothing for a name that is not: a near
    /// miss of every length, a city-state, the wrong case.
    #[test]
    fn the_search_finds_the_record_that_reading_the_roster_found() {
        for (position, record) in all().iter().enumerate() {
            let found = entry(&record.civ).expect("every roster name is found");
            assert!(
                std::ptr::eq(found, &all()[position]),
                "{} found {}",
                record.civ,
                found.civ
            );
            assert!(std::ptr::eq(
                found,
                entry_by_reading_the_roster(&record.civ).unwrap()
            ));
            let mut near_misses = vec![
                format!("{} ", record.civ),
                format!(" {}", record.civ),
                record.civ.to_lowercase(),
                record.civ.to_uppercase(),
                record.civ.chars().skip(1).collect(),
                format!("{}a", record.civ),
            ];
            near_misses.retain(|name| *name != record.civ);
            for name in near_misses {
                assert_eq!(
                    entry(&name).map(|found| found.civ.as_str()),
                    entry_by_reading_the_roster(&name).map(|found| found.civ.as_str()),
                    "{name:?}"
                );
            }
        }
        for name in [
            "",
            "Zanzibar",
            "Auckland",
            "barbarians",
            "Free Cities",
            "\u{e9}",
        ] {
            assert!(entry_by_reading_the_roster(name).is_none(), "{name:?}");
            assert!(entry(name).is_none(), "{name:?}");
            assert!(!uses_civ6_mechanics(name));
            assert!(!is_selectable(name));
            assert!(true_start(name).is_none());
        }
        // The table holds every record once, leaves most slots empty so a
        // search for a name it does not hold ends, and seldom makes a search
        // take a second step.
        let table = &roster().by_civ;
        assert!(table.len().is_power_of_two() && table.len() >= 4 * all().len());
        let mut held: Vec<u32> = table.iter().copied().filter(|at| *at != 0).collect();
        held.sort_unstable();
        assert_eq!(held, (1..=all().len() as u32).collect::<Vec<_>>());
        let mask = table.len() - 1;
        let at_home = all()
            .iter()
            .enumerate()
            .filter(|(position, record)| table[civ_slot(&record.civ, mask)] == *position as u32 + 1)
            .count();
        assert!(
            at_home * 10 >= all().len() * 8,
            "only {at_home} of {} names sit where their search starts; the three-byte \
             slot no longer tells this roster's names apart",
            all().len()
        );
    }

    /// The table is built for whatever roster it is given: an empty one, a
    /// single record, and one that repeats a name, where the first record is
    /// the one reading the roster finds.
    #[test]
    fn the_table_answers_for_a_small_roster_and_a_repeated_name() {
        let record = |civ: &str, leader: &str| LeaderRosterEntry {
            civ: civ.to_string(),
            leader: leader.to_string(),
            pool: LeaderPool::Civ6,
            available: true,
            true_start: TrueStartPoint {
                latitude: 0.0,
                longitude: 0.0,
            },
        };
        let none: Vec<LeaderRosterEntry> = Vec::new();
        let table = index_by_civ(&none);
        assert!(find_by_civ(&none, &table, "Rome").is_none());
        assert!(find_by_civ(&none, &table, "").is_none());

        let one = vec![record("Rome", "Trajan")];
        let table = index_by_civ(&one);
        assert_eq!(find_by_civ(&one, &table, "Rome").unwrap().leader, "Trajan");
        assert!(find_by_civ(&one, &table, "Roma").is_none());

        // Same first, middle and last byte and the same length: these start
        // in one slot and are told apart by the comparison alone.
        let crowded = vec![
            record("abcde", "first"),
            record("abxde", "a different name"),
            record("abcde", "repeated"),
            record("aXcYe", "same three bytes"),
            record("", "the empty name"),
        ];
        let table = index_by_civ(&crowded);
        let mask = table.len() - 1;
        assert_eq!(civ_slot("abcde", mask), civ_slot("aXcYe", mask));
        for name in ["abcde", "abxde", "aXcYe", "", "abcdf", "abcd", "e"] {
            let read = crowded.iter().find(|entry| entry.civ == name);
            let found = find_by_civ(&crowded, &table, name);
            assert_eq!(
                found.map(|entry| entry.leader.as_str()),
                read.map(|entry| entry.leader.as_str()),
                "{name:?}"
            );
        }
        assert_eq!(
            find_by_civ(&crowded, &table, "abcde").unwrap().leader,
            "first"
        );
    }

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
